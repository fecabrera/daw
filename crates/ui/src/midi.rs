//! Application orchestration. Routing state is transient; capture lives on the service worker.
use super::*;
use crate::midi_service::{Command, Message, Service};
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
/// Actions shared by the app menu, native menu and keyboard input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportAction {
    Record,
    Panic,
}
impl TransportAction {
    pub const ALL: [Self; 2] = [Self::Record, Self::Panic];
    pub fn label(self) -> &'static str {
        match self {
            Self::Record => "Record MIDI",
            Self::Panic => "Panic",
        }
    }
}
pub(super) struct State {
    pub armed: HashSet<Id>,
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
}
impl DawUi {
    pub fn transport_action_enabled(&self, action: TransportAction) -> bool {
        match action {
            TransportAction::Record => self.midi_record_enabled(),
            TransportAction::Panic => self.midi.service.is_some(),
        }
    }
    pub fn perform_transport_action(&mut self, action: TransportAction) {
        if !self.transport_action_enabled(action) {
            return;
        }
        match action {
            TransportAction::Record => self.record_midi(),
            TransportAction::Panic => {
                if let Some(service) = &self.midi.service {
                    service.panic();
                }
            }
        }
    }
    pub(super) fn transport_menu(&mut self, ui: &mut egui::Ui) {
        for action in TransportAction::ALL {
            if ui
                .add_enabled(
                    self.transport_action_enabled(action),
                    egui::Button::new(action.label()),
                )
                .clicked()
            {
                self.perform_transport_action(action);
                ui.close();
            }
        }
    }
    pub(super) fn midi_arm(&self, track: &daw_core::Track) -> Option<Arm> {
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
        })
    }
    pub(super) fn apply_midi_arm(&mut self, id: Id, arm: Option<Arm>) {
        if let Some(arm) = arm {
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
        self.midi.service.is_some()
            && self.midi.storage_ready
            && self.midi.connected
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
        if self.midi.finalizing
            || !self.midi.connected
            || self.job.is_some()
            || !(self.media_ready() || self.midi.output_revision == Some(self.change_revision))
            || self.session.project.transport.r#loop.enabled
        {
            return;
        }
        let targets = daw_core::resolve_midi_targets(
            &self.session.project,
            &self.midi.armed,
            self.selected_track,
        );
        if targets.is_empty() {
            self.fail("Select a MIDI track or enable R before recording");
            return;
        }
        self.ensure_history();
        let signature = self.session.project.time_signature;
        let bars = u64::from(self.settings.midi.count_in_bars);
        let frames = (60.0 / f64::from(self.session.project.tempo_bpm)
            * f64::from(self.session.project.sample_rate_hz)
            * f64::from(signature.numerator)
            * 4.0
            / f64::from(signature.denominator)
            * bars as f64)
            .round() as u64;
        if self.midi_send(Command::Record(
            self.session.clone(),
            self.change_revision,
            if self.is_playing() || !self.settings.midi.count_in_enabled {
                0
            } else {
                frames
            },
        )) {
            self.midi.recording = true;
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
            service.panic();
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
            self.midi.targets.clear();
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
                Message::Recording(value) => {
                    self.midi.recording = value;
                    if !value {
                        self.midi.finalizing = false;
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
                        self.fail("The project changed during capture. The complete MIDI take is retained for recovery.");
                        continue;
                    }
                    if self.session.project != session.project {
                        let mut candidate = self.session.clone();
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
                        self.session.project = candidate.project;
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
        if self.settings.midi.device.is_some()
            && self.media_ready()
            && self.output.is_none()
            && focused
            && self.job.is_none()
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
        if targets != self.midi.targets && self.midi_send(Command::Routes(targets.clone())) {
            self.midi.targets = targets;
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
    pub(super) fn midi_dialogs(&mut self, ctx: &egui::Context) {
        if self.unsaved_prompt {
            return;
        }
        if self.midi.recovery_running {
            dialogs::Dialog::<()>::new("Project recovery", "Recovering project…", &[]).show(ctx);
            return;
        }
        if let Some(entry) = self.midi.recoveries.first().cloned() {
            let mut choices = vec![("Recover", 0), ("Discard recovery", 2)];
            if entry.original_folder.is_some() {
                choices.insert(1, ("Open last saved", 1));
            }
            let choice=dialogs::Dialog::new("Project recovery",&format!("Recover recent edits to {}?",entry.name),&choices).show_with_content(ctx,|ui|{
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
