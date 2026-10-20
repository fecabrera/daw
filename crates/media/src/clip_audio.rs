//! Shared clip transform identity and lookup. Preparation stays on host workers.
use crate::{AudioData, PreparedStretch, Result};
use daw_core::{Clip, ClipStretch, Id, StretchEngine};
use std::collections::HashMap;

/// Session-scoped identity. PreparedStretch also checks source allocation, rate,
/// engine/revision and pitch revision before an entry can be reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClipAudioKey {
    pub asset_id: Id,
    pub stretch: ClipStretch,
    pub transpose: i8,
}
pub type ClipAudioCache = HashMap<ClipAudioKey, PreparedStretch>;

impl ClipAudioKey {
    pub fn for_clip(clip: &Clip) -> Option<Self> {
        if clip.stretch.is_none() && clip.transpose == 0 {
            return None;
        }
        Some(Self {
            asset_id: clip.asset_id,
            stretch: clip
                .stretch
                .unwrap_or_else(|| ClipStretch::new(1, 1).unwrap()),
            transpose: clip.transpose,
        })
    }
    fn pitch(self) -> Result<timestretch::Pitch> {
        timestretch::Pitch::from_semitones(f64::from(self.transpose))
            .map_err(|error| crate::Error(error.to_string()))
    }
    pub fn matches(
        self,
        prepared: &PreparedStretch,
        source: &AudioData,
        engine: StretchEngine,
    ) -> bool {
        self.pitch()
            .is_ok_and(|pitch| prepared.matches_pitch(source, self.stretch, engine, pitch))
    }
    pub fn prepare(
        self,
        source: &AudioData,
        engine: StretchEngine,
        cancelled: &impl Fn() -> bool,
    ) -> Result<PreparedStretch> {
        PreparedStretch::with_pitch_cancellable(
            source,
            self.stretch,
            engine,
            self.pitch()?,
            cancelled,
        )
    }
}

/// Missing/stale transformed audio is silent; never substitute the original pitch.
pub fn clip_audio<'a>(
    clip: &Clip,
    originals: &'a HashMap<Id, AudioData>,
    prepared: &'a ClipAudioCache,
    engine: StretchEngine,
) -> Option<&'a AudioData> {
    let source = originals.get(&clip.asset_id)?;
    match ClipAudioKey::for_clip(clip) {
        Some(key) => prepared
            .get(&key)
            .filter(|audio| key.matches(audio, source, engine))
            .map(|audio| &audio.audio),
        None => Some(source),
    }
}
