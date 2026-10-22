//! Bounded, in-memory edit history. Audio buffers remain shared through their Arcs.
use crate::{Result, Session};
use daw_core::Project;
use std::collections::VecDeque;

const HISTORY_LIMIT: usize = 100;
const HISTORY_BYTES: u64 = 128 * 1024 * 1024;

struct Snapshot {
    session: Session,
    model_bytes: u64,
}
impl std::ops::Deref for Snapshot {
    type Target = Session;
    fn deref(&self) -> &Session {
        &self.session
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct HistoryMemory {
    pub current_model_bytes: u64,
    pub retained_model_bytes: u64,
    pub retained_audio_bytes: u64,
    pub saved_model_bytes: u64,
}
impl HistoryMemory {
    pub fn optional_bytes(self) -> u64 {
        self.retained_model_bytes
            .saturating_add(self.retained_audio_bytes)
    }
}

pub struct SessionHistory {
    current: Snapshot,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    pending: Option<Snapshot>,
    grouping: bool,
    saved: Option<Project>,
    byte_limit: u64,
    removed_for_memory: usize,
    memory: HistoryMemory,
}

impl SessionHistory {
    pub fn new(session: &Session, saved: bool) -> Result<Self> {
        let current = snapshot(session)?;
        let mut history = Self {
            saved: saved.then(|| current.project.clone()),
            current,
            byte_limit: HISTORY_BYTES,
            removed_for_memory: 0,
            memory: HistoryMemory::default(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            grouping: false,
        };
        history.memory = history.calculate_memory();
        Ok(history)
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
            self.enforce_memory_limit();
        } else {
            self.commit(next);
        }
        Ok(())
    }

    fn commit(&mut self, next: Snapshot) {
        if next.project == self.current.project {
            self.memory = self.calculate_memory();
            return;
        }
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        self.redo.clear();
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.pop_front();
        }
        self.enforce_memory_limit();
    }

    pub fn memory_usage(&self) -> HistoryMemory {
        self.memory
    }
    fn calculate_memory(&self) -> HistoryMemory {
        fn visit(s: &Session, usage: &mut daw_media::AudioMemoryUsage) {
            for a in s.audio.values() {
                usage.audio(a);
            }
            for a in s.stretched_audio.values() {
                a.visit_memory(usage);
            }
            for a in s.sampler_audio.values() {
                a.visit_memory(usage);
            }
        }
        let mut audio = daw_media::AudioMemoryUsage::default();
        visit(&self.current, &mut audio);
        if let Some(pending) = &self.pending {
            visit(pending, &mut audio);
        }
        let essential = audio.bytes;
        for entry in self.undo.iter().chain(&self.redo) {
            visit(entry, &mut audio);
        }
        HistoryMemory {
            current_model_bytes: self.current.model_bytes
                + self.pending.as_ref().map_or(0, |p| p.model_bytes),
            retained_model_bytes: self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|s| s.model_bytes)
                .sum(),
            retained_audio_bytes: audio.bytes.saturating_sub(essential),
            saved_model_bytes: self.saved.as_ref().map_or(0, Project::memory_bytes),
        }
    }
    fn enforce_memory_limit(&mut self) {
        self.memory = self.calculate_memory();
        while self.memory.optional_bytes() > self.byte_limit {
            if self.undo.pop_front().is_none() {
                if self.redo.is_empty() {
                    break;
                }
                self.redo.remove(0);
            }
            self.removed_for_memory += 1;
            self.memory = self.calculate_memory();
        }
    }
    /// Release optional history under preparation pressure; current/pending/saved stay intact.
    pub fn release_optional_memory(&mut self) {
        self.removed_for_memory += self.undo.len() + self.redo.len();
        self.undo.clear();
        self.redo.clear();
        self.memory = self.calculate_memory();
    }
    pub fn take_memory_notice(&mut self) -> usize {
        std::mem::take(&mut self.removed_for_memory)
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
        self.saved = Some(snapshot(session)?.session.project);
        self.memory.saved_model_bytes = self.saved.as_ref().map_or(0, Project::memory_bytes);
        Ok(())
    }

    pub fn undo(&mut self, live: &Session) -> Option<Session> {
        self.finish_group();
        let previous = self.undo.pop_back()?;
        self.redo
            .push(std::mem::replace(&mut self.current, previous));
        self.enforce_memory_limit();
        Some(self.restore(live))
    }
    pub fn redo(&mut self, live: &Session) -> Option<Session> {
        self.finish_group();
        let next = self.redo.pop()?;
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        self.enforce_memory_limit();
        Some(self.restore(live))
    }
    fn restore(&self, live: &Session) -> Session {
        let mut next = self.current.session.clone();
        // Undo never changes the save destination or seeks playback. A rate change
        // retains the current time, expressed in the restored project's frame units.
        next.folder = live.folder.clone();
        // Save As copies generated stems. Restore their latest saved location by
        // stable asset ID, including Redo entries made before that copy.
        if let Some(saved) = &self.saved {
            for asset in next
                .project
                .assets
                .iter_mut()
                .filter(|a| a.source.kind == "generated")
            {
                if let Some(current) = saved.assets.iter().find(|a| a.id == asset.id) {
                    asset.source = current.source.clone();
                }
            }
        }

        let denominator = u128::from(live.project.sample_rate_hz);
        let frame = (u128::from(live.project.transport.playhead_frame)
            * u128::from(next.project.sample_rate_hz)
            + denominator / 2)
            / denominator;
        next.project.transport.playhead_frame = u64::try_from(frame).unwrap_or(u64::MAX);
        next
    }
}

fn snapshot(session: &Session) -> Result<Snapshot> {
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
    let model_bytes = daw_engine::prepared_model_bytes(
        &next.project,
        &next.audio,
        &next.stretched_audio,
        &next.sampler_audio,
    ) + next.folder.as_ref().map_or(0, |p| p.capacity() as u64)
        + (next.warnings.capacity() * std::mem::size_of::<String>()) as u64
        + next
            .warnings
            .iter()
            .map(|s| s.capacity() as u64)
            .sum::<u64>();
    Ok(Snapshot {
        session: next,
        model_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{Edit, EffectInstance};

    #[test]
    fn history_byte_limit_evicts_oldest_entries_and_keeps_current_saved_and_grouped_edits() {
        let mut session = Session::default();
        let mut history = SessionHistory::new(&session, true).unwrap();
        history.byte_limit = history.current.model_bytes * 2 + 128;
        for n in 0..20 {
            session.project.name = format!("Name {n}");
            history.record(&session).unwrap();
        }
        assert!(history.memory_usage().optional_bytes() <= history.byte_limit);
        assert!(history.take_memory_notice() > 0);
        assert_eq!(history.take_memory_notice(), 0);
        assert_eq!(history.current.project.name, "Name 19");
        history.begin_group();
        session.project.name = "Grouped final".into();
        history.record(&session).unwrap();
        history.release_optional_memory();
        assert_eq!(
            history.pending.as_ref().unwrap().project.name,
            "Grouped final"
        );
        history.finish_group();
        assert_eq!(history.current.project.name, "Grouped final");
        let prior = history.undo(&session).unwrap();
        assert_eq!(prior.project.name, "Name 19");
        assert_eq!(history.redo(&prior).unwrap().project.name, "Grouped final");
        assert!(history.saved.is_some());
    }

    #[test]
    fn history_counts_shared_audio_once_and_releases_exclusive_audio_on_eviction() {
        let mut session = Session::default();
        let samples = std::sync::Arc::new(vec![[0.0; 2]; 1024]);
        let weak = std::sync::Arc::downgrade(&samples);
        session.audio.insert(
            daw_core::Id::new_v4(),
            daw_media::AudioData {
                samples,
                peaks: std::sync::Arc::new(Vec::new()),
                sample_rate_hz: 48_000,
                metadata: daw_core::SourceMetadata {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    sample_format: "float".into(),
                    bits_per_sample: 32,
                },
            },
        );
        let mut history = SessionHistory::new(&session, true).unwrap();
        session.project.name = "Same audio".into();
        history.record(&session).unwrap();
        assert_eq!(history.memory_usage().retained_audio_bytes, 0);
        session.audio.clear();
        session.project.name = "Removed audio".into();
        history.record(&session).unwrap();
        assert_eq!(history.memory_usage().retained_audio_bytes, 8192);
        history.release_optional_memory();
        assert!(weak.upgrade().is_none());
        assert_eq!(history.memory_usage().optional_bytes(), 0);
    }

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
