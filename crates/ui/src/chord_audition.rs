//! Latest-selection worker previews, separate from project edits and preparation jobs.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

struct Request {
    session: Session,
    chord: daw_core::Chord,
    track: Id,
    revision: u64,
}
struct Pending {
    receiver: mpsc::Receiver<Result<Vec<[f32; 2]>, String>>,
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
    #[cfg(test)]
    pub requests: Vec<(Id, daw_core::Chord)>,
    #[cfg(test)]
    pub delivered: usize,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl State {
    pub fn preparing(&self) -> bool {
        self.queued.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|p| !p.cancel.load(Ordering::Relaxed))
    }
    pub fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
        self.queued = None;
        self.scope = None;
    }
    fn start(&mut self, request: Request) {
        let Request {
            session,
            chord,
            track,
            revision,
        } = request;
        let project = session.project.project_id;
        let rate = session.project.sample_rate_hz;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = session
                .audition_chord(track, chord, &|| flag.load(Ordering::Relaxed))
                .map_err(|e| e.to_string());
            let _ = sender.send(result);
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
        let Some(track) = self.selected_track.filter(|id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|t| t.id == *id && t.kind == daw_core::TrackKind::Midi)
        }) else {
            return;
        };
        if let Some(output) = &self.output {
            output.stop_audition();
        }
        let state = &mut self.chords.audition;
        if let Some(pending) = &state.pending {
            pending.cancel.store(true, Ordering::Relaxed);
        }
        state.scope = Some((self.session.project.project_id, track, self.change_revision));
        #[cfg(test)]
        state.requests.push((track, chord));
        let request = Request {
            session: self.session.clone(),
            chord,
            track,
            revision: self.change_revision,
        };
        if state.pending.is_some() {
            state.queued = Some(request);
        } else {
            state.start(request);
        }
    }

    pub(super) fn poll_chord_audition(&mut self, ctx: &egui::Context, enabled: bool) {
        let valid = enabled
            && self.view_mode == ViewMode::Timeline
            && self.chords.visible
            && ctx.input(|i| i.focused && !i.key_pressed(egui::Key::Escape))
            && self.edit_environment_available()
            && self
                .chords
                .audition
                .scope
                .is_none_or(|(project, track, revision)| {
                    project == self.session.project.project_id
                        && Some(track) == self.selected_track
                        && revision == self.change_revision
                });
        if !valid {
            let state = &mut self.chords.audition;
            if let Some(pending) = &state.pending {
                pending.cancel.store(true, Ordering::Relaxed);
            }
            state.queued = None;
            state.scope = None;
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
                            Err(_) => self.fail("Chord audition worker stopped"),
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
    fn play_chord_audition(&mut self, rate: u32, samples: Vec<[f32; 2]>) {
        #[cfg(test)]
        {
            let _ = (rate, samples);
            self.chords.audition.delivered += 1;
        }
        #[cfg(not(test))]
        {
            if self.output.is_none() {
                match AudioOutput::with_preparation(
                    self.session.plan(),
                    self.audio_preparation.take().map(|(_, _, p)| p),
                ) {
                    Ok(output) => self.output = Some(output),
                    Err(error) => {
                        self.fail(error);
                        return;
                    }
                }
            }
            if let Some(output) = &mut self.output
                && let Err(error) = output.audition(rate, samples)
            {
                self.fail(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            app.chords.audition.queued.as_ref().unwrap().chord.kind,
            daw_core::ChordKind::Minor
        );
        sender.send(Ok(vec![[1.0; 2]; 64])).unwrap();
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
