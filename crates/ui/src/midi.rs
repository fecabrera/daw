//! Application orchestration. Routing state is transient; capture lives on the service worker.
use super::*;
use crate::dialogs::FooterAction;
use crate::midi_service::{Command, Message, Service};
use daw_output::recording::{InputChannels, InputRequest};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub device: Option<String>,
    pub channel: Option<u8>,
    pub count_in_bars: u8,
    pub count_in_enabled: bool,
    pub recording_offset_ms: i16,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            device: None,
            channel: None,
            count_in_bars: 1,
            count_in_enabled: true,
            recording_offset_ms: 0,
        }
    }
}
pub(super) struct State {
    pub armed: HashSet<Id>,
    audio_armed: HashSet<Id>,
    audio_inputs: HashMap<Id, (Option<daw_output::DeviceId>, InputChannels)>,
    failed_audio_fallback: Option<(Id, Option<daw_output::DeviceId>)>,
    pub audio_take: bool,
    pub file_after_audio_take: Option<FileAction>,
    audio_peaks: HashMap<Id, std::sync::Arc<std::sync::atomic::AtomicU32>>,
    pub recording: bool,
    pub finalizing: bool,
    pub(super) storage_ready: bool,
    pub(super) take_retained: bool,
    pub(super) recovery_running: bool,
    pub(super) service: Option<Service>,
    pub(super) devices: Vec<crate::MidiDevice>,
    pub(super) connected: bool,
    configured: Option<(Option<String>, Option<u8>, i16)>,
    output: usize,
    output_revision: Option<u64>,
    targets: Vec<Id>,
    monitoring: Vec<Id>,
    project: Option<Id>,
    recoveries: Vec<daw_project::recovery::RecoveryEntry>,
    last_snapshot: std::time::Instant,
    snapshot_revision: Option<(Id, u64)>,
    ack_pending: bool,
    focused: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            armed: HashSet::new(),
            audio_armed: HashSet::new(),
            audio_inputs: HashMap::new(),
            failed_audio_fallback: None,
            audio_take: false,
            file_after_audio_take: None,
            audio_peaks: HashMap::new(),
            recording: false,
            finalizing: false,
            storage_ready: false,
            take_retained: false,
            recovery_running: false,
            service: None,
            devices: Vec::new(),
            connected: false,
            configured: None,
            output: 0,
            output_revision: None,
            targets: Vec::new(),
            monitoring: Vec::new(),
            project: None,
            recoveries: Vec::new(),
            last_snapshot: std::time::Instant::now(),
            snapshot_revision: None,
            ack_pending: false,
            focused: true,
        }
    }
}
impl State {
    pub fn recovery_prompt(&self) -> bool {
        !self.recoveries.is_empty() || self.recovery_running
    }
    pub fn busy(&self) -> bool {
        self.recording || self.finalizing
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Arm {
    pub armed: bool,
    pub fallback: bool,
    pub editable: bool,
    pub input_peak: Option<f32>,
    pub audio: bool,
}
impl DawUi {
    pub(super) fn midi_arm(&self, track: &daw_core::Track) -> Option<Arm> {
        if track.kind == daw_core::TrackKind::Audio {
            let armed = self.midi.audio_armed.contains(&track.id);
            let peak = self.midi.audio_peaks.get(&track.id);
            return Some(Arm {
                armed,
                fallback: if self.midi.busy() {
                    !armed && peak.is_some()
                } else {
                    self.audio_fallback() == Some(track.id)
                },
                editable: !self.midi.busy(),
                audio: true,
                input_peak: peak
                    .map(|peak| f32::from_bits(peak.load(std::sync::atomic::Ordering::Relaxed))),
            });
        }
        if track.kind != daw_core::TrackKind::Midi {
            return None;
        }
        let any_armed = self
            .session
            .project
            .tracks
            .iter()
            .any(|t| t.kind == daw_core::TrackKind::Midi && self.midi.armed.contains(&t.id));
        Some(Arm {
            armed: self.midi.armed.contains(&track.id),
            fallback: !any_armed && self.selected_track == Some(track.id),
            editable: !self.midi.busy(),
            input_peak: None,
            audio: false,
        })
    }
    pub(super) fn apply_midi_arm(&mut self, id: Id, arm: Option<Arm>) {
        if let Some(arm) = arm.filter(|arm| arm.editable) {
            if arm.audio {
                if arm.armed {
                    self.midi.audio_armed.insert(id);
                } else {
                    self.midi.audio_armed.remove(&id);
                }
                return;
            }
            if arm.armed {
                self.midi.armed.insert(id);
            } else {
                self.midi.armed.remove(&id);
            }
        }
    }
    pub fn enable_midi(&mut self, backend: Box<dyn crate::MidiBackend>, recovery_root: PathBuf) {
        self.midi.service = Some(Service::new(backend, recovery_root));
    }
    pub(super) fn midi_send(&mut self, command: Command) -> bool {
        if let Some(service) = &self.midi.service {
            if let Err(error) = service.send(command) {
                self.fail(error);
                return false;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn midi_boundary(&mut self, action: daw_output::MidiAction) -> bool {
        if self.midi.busy() {
            if self.midi_send(Command::Boundary(action)) {
                self.midi.finalizing = true;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn midi_record_enabled(&self) -> bool {
        if self.midi.recording {
            return !self.midi.finalizing;
        }
        let has_audio = !self.midi.audio_armed.is_empty() || self.audio_fallback().is_some();
        self.midi.service.is_some()
            && self.midi.storage_ready
            && (self.midi.connected || has_audio && self.midi.armed.is_empty())
            && (!has_audio || self.regular_save.is_none())
            && !self.midi.finalizing
            && !self.midi.take_retained
            && !self.session.project.transport.r#loop.enabled
            && self.job.is_none()
            && self.error.is_none()
            && !self.midi.recovery_prompt()
            && !self.settings_open
            && self
                .output
                .as_ref()
                .is_some_and(|o| o.current_plan_published())
            && (self.media_ready() || self.midi.output_revision == Some(self.change_revision))
    }
    pub(super) fn record_midi(&mut self) {
        if self.midi.recording {
            self.midi_boundary(daw_output::MidiAction::Seal);
            return;
        }
        if !self.midi_record_enabled() {
            return;
        }
        let targets = daw_core::resolve_midi_targets(
            &self.session.project,
            &self.midi.armed,
            self.selected_track,
        );
        let audio = self.audio_take_request();
        if targets.is_empty() && audio.is_none() {
            self.fail("Select or arm an Audio or MIDI track before recording");
            return;
        }
        self.ensure_history();
        let clock = match self.session.project.musical_clock() {
            Ok(c) => c,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let start = self.session.project.transport.playhead_frame;
        let signature = clock.signature_at(start);
        let bars = u64::from(self.settings.midi.count_in_bars);
        let frames = (60.0 / f64::from(clock.tempo_at(start))
            * f64::from(self.session.project.sample_rate_hz)
            * f64::from(signature.numerator)
            * 4.0
            / f64::from(signature.denominator)
            * bars as f64)
            .round() as u64;
        let audio_take = audio.is_some();
        if self.midi_send(Command::Record(
            self.session.clone(),
            self.change_revision,
            if self.is_playing() || !self.settings.midi.count_in_enabled {
                0
            } else {
                frames
            },
            audio,
        )) {
            self.midi.recording = true;
            self.midi.audio_take = audio_take;
            self.ensure_history();
            if let Some(history) = &mut self.history {
                history.begin_group();
            }
        }
    }
    pub(super) fn poll_midi(&mut self, ctx: &egui::Context) {
        if self.midi.service.is_none() {
            return;
        }
        let focused = ctx.input(|i| i.focused);
        if self.midi.focused
            && !focused
            && let Some(service) = &self.midi.service
        {
            if self.midi.audio_take {
                service.focus_lost();
            } else {
                service.panic();
            }
        }
        self.midi.focused = focused;
        let project = self.session.project.project_id;
        if self.midi.project != Some(project) {
            if let Some(previous) = self.midi.project {
                self.midi_send(Command::LeaveProject(previous));
            }
            if let Some(service) = &self.midi.service {
                service.panic();
            }
            self.midi.armed.clear();
            self.midi.audio_armed.clear();
            self.midi.audio_inputs.clear();
            self.midi.failed_audio_fallback = None;
            self.midi.audio_peaks.clear();
            self.midi.targets.clear();
            self.midi.monitoring.clear();
            self.midi.project = Some(project);
            self.midi.output = 0;
        }
        self.midi.armed.retain(|id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|t| t.id == *id && t.kind == daw_core::TrackKind::Midi)
        });
        let valid_audio = |id: &Id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|t| t.id == *id && t.kind == daw_core::TrackKind::Audio)
        };
        self.midi.audio_armed.retain(valid_audio);
        self.midi.audio_inputs.retain(|id, _| valid_audio(id));
        loop {
            let message = self
                .midi
                .service
                .as_ref()
                .and_then(|s| s.rx.try_recv().ok());
            let Some(message) = message else {
                break;
            };
            match message {
                Message::StorageReady => self.midi.storage_ready = true,
                Message::TakeRetained(value) => self.midi.take_retained = value,
                Message::Devices(devices) => self.midi.devices = devices,
                Message::Connected(value) => self.midi.connected = value,
                Message::AudioPeak(value) => self.midi.audio_peaks = value.into_iter().collect(),
                Message::Recording(value) => {
                    self.midi.recording = value;
                    if !value {
                        self.midi.finalizing = false;
                        self.midi.audio_take = false;
                    }
                }
                Message::Error(error) => {
                    self.midi.recovery_running = false;
                    self.fail(error);
                }
                Message::Pitch(pitch, targets) => {
                    let mut changed = false;
                    for track in targets {
                        let pitches = self.session.live_midi_pitches.entry(track).or_default();
                        if !pitches.contains(&pitch) {
                            pitches.push(pitch);
                            pitches.sort_unstable();
                            changed = true;
                        }
                    }
                    if changed
                        && self
                            .session
                            .sampler_preparation_needed(&self.session.project)
                    {
                        self.queue_background_media();
                        let message = "Live MIDI pitch preparation pending; ready tracks still sound. Pending notes remain recordable.";
                        if !self.notices.iter().any(|m| m == message) {
                            self.notices.push(message.into());
                        }
                    }
                }
                Message::Take { base, mut session } => {
                    session.project.transport = self.session.project.transport.clone();
                    session.project.display = self.session.project.display;
                    self.midi.finalizing = false;
                    self.midi.recording = false;
                    if daw_project::recovery::digest(&self.session.project)
                        .ok()
                        .as_deref()
                        != Some(&base)
                    {
                        self.midi.take_retained = true;
                        self.fail("The project changed during capture. The take is retained for recovery.");
                        continue;
                    }
                    if self.session.project != session.project {
                        let mut candidate = self.session.clone();
                        candidate.adopt_recorded_sources(&session);
                        candidate.project = session.project;
                        let mut history = self.history.clone();
                        if let Some(h) = &mut history
                            && let Err(error) = h.record(&candidate)
                        {
                            self.midi.take_retained = true;
                            self.fail(error);
                            continue;
                        }
                        self.history = history;
                        self.session = candidate;
                        self.changed();
                        if self.session.audio_preparation_needed(&self.session.project) {
                            self.queue_background_media();
                        }
                    }
                    self.midi.ack_pending = true;
                    if let Some(action) = self.midi_after_take.take() {
                        self.request(action);
                    }
                }
                Message::Recoveries(entries) => self.midi.recoveries = entries,
                Message::Recovered(mut session) => {
                    self.midi.recovery_running = false;
                    let before = session.recovered_take_before.take();
                    self.install_loaded_session(session, false);
                    if let Some(before) = before {
                        match daw_project::SessionHistory::new(&before, false).and_then(|mut h| {
                            h.record(&self.session)?;
                            Ok(h)
                        }) {
                            Ok(history) => self.history = Some(history),
                            Err(error) => self.fail(error),
                        }
                    }
                    self.midi.ack_pending = true;
                    self.dirty = true;
                    self.queue_background_media();
                }
            }
        }
        if self.midi.ack_pending && self.midi_send(Command::Acknowledge) {
            self.midi.ack_pending = false;
        }
        if !self.midi.busy()
            && !self.midi.take_retained
            && self.error.is_none()
            && let Some(action) = self.midi.file_after_audio_take.take()
        {
            self.perform_file_action(action);
        }
        let audio_fallback = self.audio_fallback();
        // Selection persists after an error. Avoid retrying the same failed
        // automatic output open on every frame.
        if self
            .midi
            .failed_audio_fallback
            .as_ref()
            .is_some_and(|(track, device)| {
                audio_fallback != Some(*track) || self.settings.audio.output_device != *device
            })
            || self.output.is_some()
        {
            self.midi.failed_audio_fallback = None;
        }
        if (self.settings.midi.device.is_some()
            || !self.midi.audio_armed.is_empty()
            || audio_fallback.is_some() && self.midi.failed_audio_fallback.is_none())
            && self.media_ready()
            && self.output.is_none()
            && focused
            && self.job.is_none()
            && self.error.is_none()
        {
            match AudioOutput::with_settings(
                self.session.plan(),
                self.audio_preparation.take().map(|(_, _, p)| p),
                &self.settings.audio,
            ) {
                Ok(output) => self.output = Some(output),
                Err(error) => {
                    self.fail(error);
                    self.settings.midi.device = None;
                    self.midi.audio_armed.clear();
                    self.midi.failed_audio_fallback = self
                        .audio_fallback()
                        .map(|track| (track, self.settings.audio.output_device.clone()));
                }
            }
        }
        if let Some(output) = &self.output {
            if self.media_ready() && output.current_plan_published() {
                self.midi.output_revision = Some(self.change_revision);
            }
            let identity = std::sync::Arc::as_ptr(&output.status) as usize;
            if identity != self.midi.output
                && output.current_plan_published()
                && let Some(sender) = output.take_midi_sender()
                && self.midi_send(Command::Output(sender))
            {
                self.midi.output = identity;
            }
        }
        let configured = (
            self.settings.midi.device.clone(),
            self.settings.midi.channel,
            self.settings.midi.recording_offset_ms,
        );
        if self.midi.configured.as_ref() != Some(&configured)
            && (configured.0.is_none() || self.midi.output != 0)
            && self.midi_send(Command::Connect(
                configured.0.clone(),
                configured.1,
                configured.2,
            ))
        {
            self.midi.configured = Some(configured);
        }
        let targets = daw_core::resolve_midi_targets(
            &self.session.project,
            &self.midi.armed,
            self.selected_track,
        );
        let monitoring = self
            .session
            .project
            .tracks
            .iter()
            .filter(|track| {
                track.kind == daw_core::TrackKind::Midi
                    && match track.midi_options.monitor {
                        daw_core::MidiMonitor::On => true,
                        daw_core::MidiMonitor::Off => false,
                        daw_core::MidiMonitor::Auto => targets.contains(&track.id),
                    }
            })
            .map(|track| track.id)
            .collect::<Vec<_>>();
        if (targets != self.midi.targets || monitoring != self.midi.monitoring)
            && self.midi_send(Command::Routes(targets.clone(), monitoring.clone()))
        {
            self.midi.targets = targets;
            self.midi.monitoring = monitoring;
        }
        if self.dirty
            && !self.midi.busy()
            && self.midi.last_snapshot.elapsed() >= Duration::from_secs(30)
            && self.midi.snapshot_revision != Some((project, self.change_revision))
        {
            match self.session.save_snapshot(self.history.as_ref()) {
                Ok(snapshot) => {
                    if self.midi_send(Command::Snapshot(
                        snapshot,
                        self.change_revision,
                        self.session.folder.clone(),
                    )) {
                        self.midi.snapshot_revision = Some((project, self.change_revision));
                        self.midi.last_snapshot = std::time::Instant::now();
                    }
                }
                Err(error) => self.fail(error),
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn audio_fallback(&self) -> Option<Id> {
        if !self.midi.armed.is_empty() || !self.midi.audio_armed.is_empty() {
            return None;
        }
        self.selected_track.filter(|id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|track| track.id == *id && track.kind == daw_core::TrackKind::Audio)
        })
    }

    fn audio_take_request(&self) -> Option<super::midi_service::AudioTake> {
        let mut routes = Vec::new();
        let mut tracks = Vec::new();
        let device = &self.settings.audio.input_device;
        let fallback = self.audio_fallback();
        for track in &self.session.project.tracks {
            if track.kind != daw_core::TrackKind::Audio
                || (!self.midi.audio_armed.contains(&track.id) && fallback != Some(track.id))
            {
                continue;
            }
            let route = self
                .midi
                .audio_inputs
                .get(&track.id)
                .filter(|(input, _)| input == device)
                .map_or(InputChannels::default(), |(_, route)| *route);
            let index = routes.iter().position(|r| *r == route).unwrap_or_else(|| {
                let index = routes.len();
                routes.push(route);
                index
            });
            tracks.push((track.id, index));
        }
        (!tracks.is_empty()).then(|| super::midi_service::AudioTake {
            input: InputRequest {
                device: device.clone(),
                routes,
            },
            tracks,
            midi_targets: self
                .session
                .project
                .tracks
                .iter()
                .filter(|t| t.kind == daw_core::TrackKind::Midi && self.midi.armed.contains(&t.id))
                .map(|t| t.id)
                .collect(),
        })
    }

    pub(super) fn audio_recording_input(&mut self, ui: &mut egui::Ui, track: Id) {
        if self.audio_devices.catalog.is_none()
            && !self.audio_devices.busy()
            && self.audio_devices.error.is_none()
        {
            self.refresh_audio_devices();
        }
        let device = &self.settings.audio.input_device;
        let channels = self
            .audio_devices
            .catalog
            .as_ref()
            .and_then(|catalog| {
                let id = device.as_ref().or(catalog.default_input.as_ref())?;
                catalog.inputs.iter().find(|input| &input.id == id)
            })
            .and_then(|input| {
                input
                    .configurations
                    .iter()
                    .map(|config| config.channels)
                    .max()
            })
            .unwrap_or(1);
        let selected = self
            .midi
            .audio_inputs
            .get(&track)
            .filter(|(input, _)| input == device)
            .map_or(InputChannels::default(), |(_, route)| *route);
        let label = |route: InputChannels| {
            if route.stereo {
                format!(
                    "Stereo {}–{}",
                    u32::from(route.first) + 1,
                    u32::from(route.first) + 2
                )
            } else {
                format!("Mono {}", u32::from(route.first) + 1)
            }
        };
        let mut next = selected;
        ui.add_enabled_ui(!self.midi.busy(), |ui| {
            crate::menus::Dropdown::from_id_salt(("audio-recording-input", track))
                .selected_text(label(selected))
                .show_ui(ui, |ui| {
                    for first in 0..channels {
                        let route = InputChannels {
                            first,
                            stereo: false,
                        };
                        ui.selectable_value(&mut next, route, label(route));
                        if first + 1 < channels {
                            let route = InputChannels {
                                first,
                                stereo: true,
                            };
                            ui.selectable_value(&mut next, route, label(route));
                        }
                    }
                });
        });
        if next != selected {
            self.midi.audio_inputs.insert(track, (device.clone(), next));
        }
    }
    pub(super) fn midi_dialogs(&mut self, ctx: &egui::Context, surface: Option<&mut egui::Ui>) {
        if self.project_prompt_kind() != Some(crate::project_prompt::ProjectPromptKind::Recovery) {
            return;
        }
        if self.midi.recovery_running {
            dialogs::Dialog::<()>::new("Project recovery", "Recovering project…", &[])
                .show_message(ctx, surface);
            return;
        }
        if let Some(entry) = self.midi.recoveries.first().cloned() {
            let mut choices = vec![
                FooterAction::accept("Recover", 0),
                FooterAction::deny("Discard recovery", 2),
            ];
            if entry.original_folder.is_some() {
                choices.insert(1, FooterAction::cancel("Open last saved", 1));
            }
            let choice=dialogs::Dialog::new("Project recovery",&format!("Recover recent edits to {}?",entry.name),&choices).show_surface(ctx,surface,|ui|{
                if let Some(warning)=&entry.warning {ui.label(warning);}
                rows::advice(ui,"Recovery keeps the last manual save. Use Save As to put recovered work in your project folder.");
            });
            if let Some(choice) = choice {
                if choice == 2 {
                    if self.midi_send(Command::Discard(entry)) {
                        self.midi.recoveries.remove(0);
                    }
                } else {
                    self.midi.recoveries.remove(0);
                    self.request(Action::Recovery(Box::new(entry), choice == 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod recording_tests {
    use super::*;

    #[test]
    fn selected_audio_follows_selection_until_a_track_is_explicitly_armed() {
        let mut app = DawUi::default();
        let a = app.session.project.add_track().unwrap();
        let b = app.session.project.add_track().unwrap();
        let midi = app.session.project.add_midi_track().unwrap();
        let before = app.session.project.clone();
        let arm_for = |app: &DawUi, id| {
            let track = app
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == id)
                .unwrap();
            app.midi_arm(track).unwrap()
        };

        app.select_track(a, false);
        let mut arm = arm_for(&app, a);
        assert!(arm.fallback && !arm.armed);
        assert_eq!(app.audio_take_request().unwrap().tracks, [(a, 0)]);
        arm.armed = true;
        app.apply_midi_arm(a, Some(arm));

        app.select_track(b, false);
        assert!(!arm_for(&app, b).fallback);
        assert_eq!(app.audio_take_request().unwrap().tracks, [(a, 0)]);
        let mut arm = arm_for(&app, a);
        assert!(arm.armed && !arm.fallback);
        arm.armed = false;
        app.apply_midi_arm(a, Some(arm));

        assert!(arm_for(&app, b).fallback);
        assert_eq!(app.audio_take_request().unwrap().tracks, [(b, 0)]);
        app.midi.recording = true;
        app.midi.audio_peaks.insert(b, std::sync::Arc::default());
        app.select_track(a, false);
        assert!(!arm_for(&app, a).fallback);
        assert!(arm_for(&app, b).fallback);
        assert_eq!(arm_for(&app, b).input_peak, Some(0.0));
        app.midi.recording = false;
        app.select_track(b, false);

        app.midi.armed.insert(midi);
        assert!(!arm_for(&app, b).fallback);
        assert!(app.audio_take_request().is_none());
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }

    #[test]
    fn audio_arms_share_routes_and_mixed_take_requires_explicit_midi_arm() {
        let mut app = DawUi::default();
        let a = app.session.project.add_track().unwrap();
        let b = app.session.project.add_track().unwrap();
        let c = app.session.project.add_midi_track().unwrap();
        app.select_track(c, false);
        let before = app.session.project.clone();
        assert!(app.audio_take_request().is_none());
        for id in [a, b] {
            let track = app
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == id)
                .unwrap();
            let mut arm = app.midi_arm(track).unwrap();
            assert!(arm.audio && !arm.fallback && !arm.armed);
            arm.armed = true;
            app.apply_midi_arm(id, Some(arm));
        }
        let take = app.audio_take_request().unwrap();
        assert_eq!(take.input.routes, [InputChannels::default()]);
        assert_eq!(take.tracks, [(a, 0), (b, 0)]);
        assert!(take.midi_targets.is_empty());
        assert_eq!(
            daw_core::resolve_midi_targets(&app.session.project, &app.midi.armed, Some(c)),
            [c]
        );
        app.midi.armed.insert(c);
        assert_eq!(app.audio_take_request().unwrap().midi_targets, [c]);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
}
