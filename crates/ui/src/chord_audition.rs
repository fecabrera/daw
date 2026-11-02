//! Latest-selection worker previews, separate from project edits and preparation jobs.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Copy)]
enum Sound {
    Chord(daw_core::Chord),
    Note(u8),
    Beat {
        row: Id,
        transpose: i8,
        resolution: daw_core::BeatResolution,
        direct: bool,
    },
}

enum PreparedAudition {
    Samples(Vec<[f32; 2]>),
    Stream(daw_output::AuditionStream),
}

struct Request {
    session: Session,
    sound: Sound,
    track: Id,
    revision: u64,
    release: Option<Arc<AtomicBool>>,
}
struct Pending {
    receiver: mpsc::Receiver<Result<PreparedAudition, String>>,
    cancel: Arc<AtomicBool>,
    project: Id,
    track: Id,
    revision: u64,
    rate: u32,
}
#[derive(Default)]
pub(super) struct State {
    pending: Option<Pending>,
    queued: Option<Request>,
    scope: Option<(Id, Id, u64)>,
    editor: bool,
    direct: bool,
    release: Option<Arc<AtomicBool>>,
    render_cancel: Option<Arc<AtomicBool>>,
    pub(super) allow_note_commit: bool,
    #[cfg(test)]
    pub note_requests: Vec<(Id, u8)>,
    #[cfg(test)]
    pub beat_requests: Vec<(Id, Id)>,
    #[cfg(test)]
    pub requests: Vec<(Id, daw_core::Chord)>,
    #[cfg(test)]
    pub delivered: usize,
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl State {
    #[cfg(test)]
    pub(super) fn scoped(&self) -> bool {
        self.scope.is_some()
    }
    #[cfg(test)]
    pub(super) fn note_released(&self) -> bool {
        self.release
            .as_ref()
            .is_none_or(|flag| flag.load(Ordering::Acquire))
    }
    pub fn preparing(&self) -> bool {
        self.queued.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|p| !p.cancel.load(Ordering::Relaxed))
    }
    pub fn cancel(&mut self) {
        if let Some(cancel) = &self.render_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        self.release = None;
        self.allow_note_commit = false;
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
        self.queued = None;
        self.scope = None;
    }
    fn start(&mut self, request: Request) {
        let Request {
            session,
            sound,
            track,
            revision,
            release,
        } = request;
        let project = session.project.project_id;
        let rate = session.project.sample_rate_hz;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        self.render_cancel = Some(cancel.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let cancelled = || flag.load(Ordering::Relaxed);
            match sound {
                Sound::Beat {
                    row,
                    transpose,
                    resolution,
                    direct: false,
                } => {
                    let result = session
                        .audition_beat_step(track, row, transpose, resolution, &cancelled)
                        .map(PreparedAudition::Samples)
                        .map_err(|e| e.to_string());
                    let _ = sender.send(result);
                }
                Sound::Chord(chord) => {
                    let result = session
                        .audition_chord(track, chord, &cancelled)
                        .map(PreparedAudition::Samples)
                        .map_err(|e| e.to_string());
                    let _ = sender.send(result);
                }
                sound => match match sound {
                    Sound::Note(pitch) => session.held_note_audition(track, pitch, &cancelled),
                    Sound::Beat {
                        row,
                        transpose,
                        resolution,
                        ..
                    } => session
                        .beat_subtrack_audition(track, row, transpose, resolution, &cancelled),
                    Sound::Chord(_) => unreachable!(),
                } {
                    Ok(mut note) => {
                        let (mut writer, stream) = daw_output::audition_stream();
                        if sender.send(Ok(PreparedAudition::Stream(stream))).is_err() {
                            return;
                        }
                        let Some(frames) = writer.wait_for_start(&cancelled) else {
                            return;
                        };
                        note.set_chunk_frames(frames);
                        while !cancelled() {
                            note.set_chunk_frames(writer.chunk_frames());
                            let released = release
                                .as_ref()
                                .is_none_or(|flag| flag.load(Ordering::Acquire));
                            let Some(samples) = note.next_chunk(released) else {
                                break;
                            };
                            if !writer.send(samples, &cancelled) {
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                    }
                },
            }
        });
        self.pending = Some(Pending {
            receiver,
            cancel,
            project,
            track,
            revision,
            rate,
        });
    }
}
impl DawUi {
    pub(super) fn audition_chord(&mut self, chord: daw_core::Chord) {
        self.request_midi_audition(Sound::Chord(chord));
    }

    pub(super) fn audition_editor_note(&mut self, pitch: u8) {
        if self.midi_editor.audition_enabled {
            self.request_midi_audition(Sound::Note(pitch));
        }
    }

    pub(super) fn audition_clip_note(&mut self, clip: Id, pitch: u8) {
        if let Some(clip) = self
            .session
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == clip)
        {
            let pitch = (i16::from(pitch) + i16::from(clip.transpose)).clamp(0, 127) as u8;
            self.audition_editor_note(pitch);
        }
    }

    pub(super) fn audition_beat_step(&mut self, track: Id, row: Id) {
        if self.midi_editor.audition_enabled {
            self.request_beat_audition(track, row, false);
        }
    }

    /// An explicit row preview works without a selected pattern or automatic audition.
    pub(super) fn audition_beat_subtrack(&mut self, track: Id, row: Id) {
        if self.editing_enabled() {
            self.request_beat_audition(track, row, true);
        }
    }

    fn request_beat_audition(&mut self, track: Id, row: Id, direct: bool) {
        if self
            .session
            .project
            .beat_subtrack(row)
            .is_none_or(|(owner, _)| owner != track)
        {
            return;
        }
        let settings = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.clips.iter().find(|c| Some(c.id) == self.selected_clip))
            .and_then(|clip| {
                self.session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .and_then(|a| a.beat_pattern.as_ref())
                    .map(|source| (clip.transpose, source.pattern.resolution))
            });
        if !direct && settings.is_none() {
            return;
        }
        let (transpose, resolution) = settings.unwrap_or((0, daw_core::BeatResolution::Sixteenth));
        self.request_midi_audition(Sound::Beat {
            row,
            transpose,
            resolution,
            direct,
        });
    }

    fn active_audition_track(&self) -> Option<Id> {
        self.active_midi_track().or_else(|| {
            (self.view_mode == ViewMode::Edit)
                .then(|| self.editor_track())
                .flatten()
                .filter(|t| t.kind == daw_core::TrackKind::Beat)
                .map(|t| t.id)
        })
    }

    fn request_midi_audition(&mut self, sound: Sound) {
        let Some(track) = (match sound {
            Sound::Beat { .. } => self.active_audition_track(),
            _ => self.active_midi_track(),
        }) else {
            return;
        };
        if let Some(output) = &self.output {
            output.stop_audition();
        }
        let state = &mut self.chords.audition;
        if let Some(cancel) = &state.render_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        state.allow_note_commit = false;
        state.release = matches!(sound, Sound::Note(_)).then(|| Arc::new(AtomicBool::new(false)));
        if let Some(pending) = &state.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
        state.scope = Some((self.session.project.project_id, track, self.change_revision));
        state.editor = !matches!(sound, Sound::Chord(_));
        state.direct = matches!(sound, Sound::Beat { direct: true, .. });
        #[cfg(test)]
        match sound {
            Sound::Chord(chord) => state.requests.push((track, chord)),
            Sound::Note(pitch) => state.note_requests.push((track, pitch)),
            Sound::Beat { row, .. } => state.beat_requests.push((track, row)),
        }
        let request = Request {
            session: self.session.clone(),
            sound,
            track,
            revision: self.change_revision,
            release: state.release.clone(),
        };
        if state.pending.is_some() {
            state.queued = Some(request);
        } else {
            state.start(request);
        }
    }

    pub(super) fn poll_chord_audition(&mut self, ctx: &egui::Context, enabled: bool) {
        if let Some(release) = &self.chords.audition.release
            && !ctx.input(|i| i.pointer.primary_down())
        {
            release.store(true, Ordering::Release);
        }
        if self.chords.audition.allow_note_commit
            && self.job.is_none()
            && let Some((project, track, _)) = self.chords.audition.scope
            && project == self.session.project.project_id
            && Some(track) == self.active_audition_track()
        {
            self.chords.audition.scope = Some((project, track, self.change_revision));
            if let Some(pending) = &mut self.chords.audition.pending {
                pending.revision = self.change_revision;
            }
            if let Some(queued) = &mut self.chords.audition.queued {
                queued.revision = self.change_revision;
            }
            if !self.midi_editor.editing_note() {
                self.chords.audition.allow_note_commit = false;
            }
        }
        let valid = enabled
            && if self.chords.audition.editor {
                self.view_mode == ViewMode::Edit
                    && (self.midi_editor.audition_enabled || self.chords.audition.direct)
            } else {
                self.chords_visible()
            }
            && ctx.input(|i| i.focused && !i.key_pressed(egui::Key::Escape))
            && self.edit_environment_available_with_job(self.chords.audition.allow_note_commit)
            && self
                .chords
                .audition
                .scope
                .is_none_or(|(project, track, revision)| {
                    project == self.session.project.project_id
                        && Some(track) == self.active_audition_track()
                        && revision == self.change_revision
                });
        if !valid {
            self.chords.audition.cancel();
            if let Some(output) = &self.output {
                output.stop_audition();
            }
        }
        let state = &mut self.chords.audition;
        if let Some(pending) = &state.pending {
            match pending.receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {}
                result => {
                    let pending = state.pending.take().unwrap();
                    if !pending.cancel.load(Ordering::Relaxed)
                        && valid
                        && state.queued.is_none()
                        && state.scope == Some((pending.project, pending.track, pending.revision))
                    {
                        match result {
                            Ok(Ok(samples)) => self.play_chord_audition(pending.rate, samples),
                            Ok(Err(error)) => self.fail(error),
                            Err(_) => self.fail("MIDI audition worker stopped"),
                        }
                    }
                }
            }
        }
        let state = &mut self.chords.audition;
        if state.pending.is_none()
            && let Some(request) = state.queued.take()
        {
            state.start(request);
        }
        if state.pending.is_some() || self.output.as_ref().is_some_and(|o| o.auditioning()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
    fn play_chord_audition(&mut self, rate: u32, samples: PreparedAudition) {
        if !self.media_ready() && self.output.is_none() {
            return;
        }
        #[cfg(test)]
        {
            let _ = rate;
            match samples {
                PreparedAudition::Samples(samples) => {
                    let _ = samples.len();
                }
                PreparedAudition::Stream(stream) => {
                    drop(stream);
                    if let Some(cancel) = &self.chords.audition.render_cancel {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
            }
            self.chords.audition.delivered += 1;
        }
        #[cfg(not(test))]
        {
            if self.output.is_none() {
                match AudioOutput::with_settings(
                    self.session.plan(),
                    self.audio_preparation.take().map(|(_, _, p)| p),
                    &self.settings.audio,
                ) {
                    Ok(output) => self.output = Some(output),
                    Err(error) => {
                        self.chords.audition.cancel();
                        self.fail(error);
                        return;
                    }
                }
            }
            if let Some(output) = &mut self.output
                && let Err(error) = match samples {
                    PreparedAudition::Samples(samples) => output.audition(rate, samples),
                    PreparedAudition::Stream(stream) => output.audition_stream(rate, stream),
                }
            {
                self.chords.audition.cancel();
                self.fail(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_beat_results_ignore_the_toggle_but_require_a_valid_editor_scope() {
        for invalid in ["none", "view", "revision", "focus", "disabled", "dialog"] {
            let mut app = DawUi::default();
            let track = app.session.project.add_beat_track().unwrap();
            app.selected_track = Some(track);
            app.view_mode = if invalid == "view" {
                ViewMode::Timeline
            } else {
                ViewMode::Edit
            };
            app.settings_open = invalid == "dialog";
            let project = app.session.project.clone();
            let (sender, receiver) = mpsc::channel();
            let flag = Arc::new(AtomicBool::new(false));
            app.chords.audition.editor = true;
            app.chords.audition.direct = true;
            app.chords.audition.scope = Some((project.project_id, track, app.change_revision));
            app.chords.audition.pending = Some(Pending {
                receiver,
                cancel: flag.clone(),
                project: project.project_id,
                track,
                revision: app.change_revision,
                rate: 48_000,
            });
            if invalid == "revision" {
                app.change_revision += 1;
            }
            let (_writer, stream) = daw_output::audition_stream();
            sender.send(Ok(PreparedAudition::Stream(stream))).unwrap();
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: invalid != "focus",
                    ..Default::default()
                },
                |_| app.poll_chord_audition(&ctx, invalid != "disabled"),
            );
            output.textures_delta.clear();
            assert!(!app.midi_editor.audition_enabled);
            assert_eq!(
                app.chords.audition.delivered,
                usize::from(invalid == "none"),
                "{invalid}"
            );
            assert_eq!(flag.load(Ordering::Relaxed), invalid != "none", "{invalid}");
            assert_eq!(app.session.project, project);
        }
    }

    #[test]
    fn editor_results_require_the_enabled_current_scope() {
        for invalid in [false, true] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            app.selected_track = Some(track);
            app.view_mode = ViewMode::Edit;
            app.midi_editor.audition_enabled = !invalid;
            let (sender, receiver) = mpsc::channel();
            let flag = Arc::new(AtomicBool::new(false));
            app.chords.audition.editor = true;
            app.chords.audition.scope =
                Some((app.session.project.project_id, track, app.change_revision));
            app.chords.audition.pending = Some(Pending {
                receiver,
                cancel: flag.clone(),
                project: app.session.project.project_id,
                track,
                revision: app.change_revision,
                rate: 48_000,
            });
            sender
                .send(Ok(PreparedAudition::Samples(vec![[0.1; 2]; 64])))
                .unwrap();
            let ctx = egui::Context::default();
            let mut output =
                ctx.run_ui(Default::default(), |_| app.poll_chord_audition(&ctx, true));
            output.textures_delta.clear();
            assert_eq!(app.chords.audition.delivered, usize::from(!invalid));
            assert_eq!(flag.load(Ordering::Relaxed), invalid);
        }
    }

    #[test]
    fn stale_worker_results_are_discarded_and_track_changes_cancel_previews() {
        let mut app = DawUi::default();
        let track = app.session.project.add_midi_track().unwrap();
        app.selected_track = Some(track);
        app.chords.visible = true;
        let ctx = egui::Context::default();
        let (sender, receiver) = mpsc::channel();
        let flag = Arc::new(AtomicBool::new(false));
        app.chords.audition.pending = Some(Pending {
            receiver,
            cancel: flag.clone(),
            project: app.session.project.project_id,
            track,
            revision: app.change_revision,
            rate: 48_000,
        });
        let chord = daw_core::Chord {
            root: 0,
            kind: daw_core::ChordKind::Major,
        };
        app.audition_chord(chord);
        app.audition_chord(daw_core::Chord {
            kind: daw_core::ChordKind::Minor,
            ..chord
        });
        assert!(flag.load(Ordering::Relaxed));
        assert_eq!(app.stop_action(), transport_controls::StopAction::Stop);
        assert_eq!(
            match app.chords.audition.queued.as_ref().unwrap().sound {
                Sound::Chord(c) => c.kind,
                _ => panic!("expected chord"),
            },
            daw_core::ChordKind::Minor
        );
        sender
            .send(Ok(PreparedAudition::Samples(vec![[1.0; 2]; 64])))
            .unwrap();
        app.poll_chord_audition(&ctx, true);
        assert_eq!(app.chords.audition.delivered, 0);
        assert!(app.chords.audition.queued.is_none());
        app.stop();
        assert!(!app.chords.audition.preparing());
        app.selected_track = None;
        app.poll_chord_audition(&ctx, true);
        assert!(app.chords.audition.scope.is_none());
        if let Some(pending) = &app.chords.audition.pending {
            assert!(pending.cancel.load(Ordering::Relaxed));
        }
    }
}
