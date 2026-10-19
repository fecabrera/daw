//! Bounded, in-memory edit history. Audio buffers remain shared through their Arcs.
use crate::{Result, Session};
use daw_core::Project;
use std::collections::VecDeque;

const HISTORY_LIMIT: usize = 100;

pub struct SessionHistory {
    current: Session,
    undo: VecDeque<Session>,
    redo: Vec<Session>,
    pending: Option<Session>,
    grouping: bool,
    saved: Option<Project>,
}

impl SessionHistory {
    pub fn new(session: &Session, saved: bool) -> Result<Self> {
        let current = snapshot(session)?;
        Ok(Self {
            saved: saved.then(|| current.project.clone()),
            current,
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            grouping: false,
        })
    }

    pub fn project_id(&self) -> daw_core::Id {
        self.current.project.project_id
    }

    /// Group live control updates into one transaction, including a return to the original value.
    pub fn begin_group(&mut self) {
        self.grouping = true;
    }

    pub fn finish_group(&mut self) {
        self.grouping = false;
        if let Some(next) = self.pending.take() {
            self.commit(next);
        }
    }

    /// Call only after a successful edit or prepared worker publication.
    pub fn record(&mut self, session: &Session) -> Result<()> {
        let next = snapshot(session)?;
        if self.grouping {
            self.pending = Some(next);
        } else {
            self.commit(next);
        }
        Ok(())
    }

    fn commit(&mut self, next: Session) {
        if next.project == self.current.project {
            return;
        }
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        self.redo.clear();
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.pop_front();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn is_saved(&self) -> bool {
        self.saved.as_ref() == Some(&self.pending.as_ref().unwrap_or(&self.current).project)
    }
    /// Accept the exact snapshot written by a save worker, even if newer edits exist.
    pub fn mark_saved(&mut self, session: &Session) -> Result<()> {
        self.saved = Some(snapshot(session)?.project);
        Ok(())
    }

    pub fn undo(&mut self, live: &Session) -> Option<Session> {
        self.finish_group();
        let previous = self.undo.pop_back()?;
        self.redo
            .push(std::mem::replace(&mut self.current, previous));
        Some(self.restore(live))
    }
    pub fn redo(&mut self, live: &Session) -> Option<Session> {
        self.finish_group();
        let next = self.redo.pop()?;
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        Some(self.restore(live))
    }
    fn restore(&self, live: &Session) -> Session {
        let mut next = self.current.clone();
        // Undo never changes the save destination or seeks playback. A rate change
        // retains the current time, expressed in the restored project's frame units.
        next.folder = live.folder.clone();
        let denominator = u128::from(live.project.sample_rate_hz);
        let frame = (u128::from(live.project.transport.playhead_frame)
            * u128::from(next.project.sample_rate_hz)
            + denominator / 2)
            / denominator;
        next.project.transport.playhead_frame = u64::try_from(frame).unwrap_or(u64::MAX);
        next
    }
}

fn snapshot(session: &Session) -> Result<Session> {
    let mut next = session.clone();
    next.project.transport.playhead_frame = 0;
    // History can outlive Save As. Resolve references against their original folder,
    // without touching source files or retaining a stale relative-path base.
    for asset in &mut next.project.assets {
        if asset.midi_pattern.is_none() {
            asset.source.path = session.source_path(asset)?.to_string_lossy().into_owned();
            asset.source.path_kind = "absolute".into();
        }
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{Edit, EffectInstance};

    #[test]
    fn history_groups_edits_skips_noops_and_discards_only_committed_redo() {
        let mut session = Session::default();
        let track = session.project.add_track().unwrap();
        let initial = session.project.clone();
        let mut history = SessionHistory::new(&session, true).unwrap();
        history.begin_group();
        for gain in [-1.0, -2.0, -6.0] {
            session.project.tracks[0].gain_db = gain;
            history.record(&session).unwrap();
        }
        history.finish_group();
        assert!(history.can_undo());
        session.project.transport.playhead_frame = 1234;
        session = history.undo(&session).unwrap();
        assert_eq!(session.project.tracks, initial.tracks);
        assert_eq!(session.project.transport.playhead_frame, 1234);
        session.project.transport.playhead_frame = u64::MAX - 100;
        session = history.redo(&session).unwrap();
        session = history.undo(&session).unwrap();
        assert_eq!(session.project.transport.playhead_frame, u64::MAX - 100);
        session.project.transport.playhead_frame = 1234;
        assert!(history.is_saved());
        assert!(!history.can_undo());
        history.begin_group();
        session.project.tracks[0].gain_db = -9.0;
        history.record(&session).unwrap();
        session.project.tracks[0].gain_db = 0.0;
        history.record(&session).unwrap();
        history.finish_group();
        assert!(history.can_redo());
        assert!(!history.can_undo());
        assert!(
            session
                .project
                .edit(Edit::SetTempo { bpm: f32::NAN })
                .is_err()
        );
        history.record(&session).unwrap();
        assert!(history.can_redo());
        session = history.redo(&session).unwrap();
        assert_eq!(session.project.tracks[0].gain_db, -6.0);
        history.mark_saved(&session).unwrap();
        let saved = session.clone();
        session
            .project
            .edit(Edit::SetEffects {
                track_id: track,
                effects: vec![EffectInstance::saturator()],
                compact: false,
            })
            .unwrap();
        let effect = session.project.tracks[0].effects[0].id;
        history.record(&session).unwrap();
        session = history.undo(&session).unwrap();
        assert_eq!(session.project, saved.project);
        assert!(history.is_saved());
        session = history.redo(&session).unwrap();
        assert_eq!(session.project.tracks[0].effects[0].id, effect);
        session = history.undo(&session).unwrap();
        session.project.tracks[0].muted = true;
        history.record(&session).unwrap();
        assert!(!history.can_redo());
    }

    #[test]
    fn history_is_bounded_and_save_completion_marks_its_own_snapshot() {
        let mut session = Session::default();
        let mut history = SessionHistory::new(&session, true).unwrap();
        for n in 1..=120 {
            session.project.name = format!("Name {n}");
            history.record(&session).unwrap();
        }
        let saved = session.clone();
        session.project.name = "Later edit".into();
        history.record(&session).unwrap();
        history.mark_saved(&saved).unwrap();
        assert!(!history.is_saved());
        session = history.undo(&session).unwrap();
        assert!(history.is_saved());
        let mut count = 1;
        while let Some(previous) = history.undo(&session) {
            session = previous;
            count += 1;
        }
        assert_eq!(count, HISTORY_LIMIT);
        assert_eq!(session.project.name, "Name 21");
    }
}
