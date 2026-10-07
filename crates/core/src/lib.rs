use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use uuid::Uuid as Id;
pub const SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub project_id: Id,
    pub name: String,
    pub sample_rate_hz: u32,
    pub master: Master,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub transport: Transport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Master {
    pub gain_db: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub id: Id,
    pub name: String,
    pub source: Source,
    pub source_metadata: SourceMetadata,
    pub decoded_frame_count: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub kind: String,
    pub path: String,
    pub path_kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub sample_format: String,
    pub bits_per_sample: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: Id,
    pub name: String,
    pub gain_db: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub clips: Vec<Clip>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub id: Id,
    pub asset_id: Id,
    pub name: String,
    pub start_frame: u64,
    pub source_offset_frame: u64,
    pub length_frames: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Transport {
    pub playhead_frame: u64,
    pub r#loop: Loop,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Loop {
    pub enabled: bool,
    pub start_frame: u64,
    pub end_frame: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            project_id: Id::new_v4(),
            name: "Untitled".into(),
            sample_rate_hz: SAMPLE_RATE,
            master: Master { gain_db: 0.0 },
            assets: vec![],
            tracks: vec![],
            transport: Transport::default(),
        }
    }
}
impl Clip {
    pub fn end(&self) -> u64 {
        self.start_frame.saturating_add(self.length_frames)
    }
}
pub fn linear_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
pub fn frames(seconds: f64) -> u64 {
    (seconds.max(0.0) * f64::from(SAMPLE_RATE)).round() as u64
}
pub fn seconds(frames: u64) -> f64 {
    frames as f64 / f64::from(SAMPLE_RATE)
}

impl Project {
    pub fn end(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(Clip::end)
            .max()
            .unwrap_or(0)
    }
    pub fn audible(&self, track: &Track) -> bool {
        if self.tracks.iter().any(|t| t.soloed) {
            track.soloed
        } else {
            !track.muted
        }
    }
    pub fn validate(&self) -> Result<()> {
        let invalid = |text: &str| Error(text.into());
        if self.sample_rate_hz != SAMPLE_RATE {
            return Err(invalid("Project sample rate must be 48 kHz"));
        }
        if !self.master.gain_db.is_finite() || !linear_gain(self.master.gain_db).is_finite() {
            return Err(invalid("Invalid master gain"));
        }
        let mut ids = std::collections::HashSet::<Uuid>::new();
        ids.insert(self.project_id);
        for a in &self.assets {
            if !ids.insert(a.id) {
                return Err(invalid("Duplicate asset ID"));
            }
            if a.source.kind != "external"
                || !matches!(a.source.path_kind.as_str(), "relative" | "absolute")
                || a.source.path.is_empty()
            {
                return Err(invalid("Invalid external source reference"));
            }
            if !(1..=2).contains(&a.source_metadata.channels)
                || a.source_metadata.sample_rate_hz == 0
                || !matches!(
                    (
                        a.source_metadata.sample_format.as_str(),
                        a.source_metadata.bits_per_sample
                    ),
                    ("pcm_int", 16 | 24) | ("ieee_float", 32)
                )
            {
                return Err(invalid("Unsupported source metadata"));
            }
        }
        for t in &self.tracks {
            if !ids.insert(t.id) {
                return Err(invalid("Duplicate track ID"));
            }
            if !t.pan.is_finite()
                || !(-1.0..=1.0).contains(&t.pan)
                || !t.gain_db.is_finite()
                || !linear_gain(t.gain_db).is_finite()
            {
                return Err(invalid("Invalid track gain or pan"));
            }
            let mut previous_end = 0;
            let mut clips = t.clips.iter().collect::<Vec<_>>();
            clips.sort_by_key(|c| c.start_frame);
            for c in clips {
                if !ids.insert(c.id) {
                    return Err(invalid("Duplicate clip ID"));
                }
                let a = self
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)
                    .ok_or_else(|| invalid("Clip references a missing asset record"))?;
                if c.length_frames == 0
                    || c.start_frame.checked_add(c.length_frames).is_none()
                    || c.source_offset_frame
                        .checked_add(c.length_frames)
                        .is_none_or(|end| end > a.decoded_frame_count)
                {
                    return Err(invalid("Clip range exceeds saved asset bounds"));
                }
                if c.start_frame < previous_end {
                    return Err(invalid("Clips cannot overlap on the same track"));
                }
                previous_end = c.end();
            }
        }
        let l = &self.transport.r#loop;
        if l.enabled && l.end_frame <= l.start_frame {
            return Err(invalid("Loop end must be after its start"));
        }
        Ok(())
    }
    pub fn add_track(&mut self) -> Result<Id> {
        let id = Id::new_v4();
        self.tracks.push(Track {
            id,
            name: format!("Track {}", self.tracks.len() + 1),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            clips: vec![],
        });
        Ok(id)
    }
    pub fn edit(&mut self, command: Edit) -> Result<()> {
        let mut next = self.clone();
        match command {
            Edit::DeleteTrack(id) => next.tracks.retain(|t| t.id != id),
            Edit::DeleteClip(id) => {
                for t in &mut next.tracks {
                    t.clips.retain(|c| c.id != id);
                }
            }
            Edit::Place {
                clip_id,
                track_id,
                start,
                offset,
                length,
            } => {
                let c = next
                    .tracks
                    .iter_mut()
                    .find_map(|t| {
                        t.clips
                            .iter()
                            .position(|c| c.id == clip_id)
                            .map(|i| t.clips.remove(i))
                    })
                    .ok_or_else(|| Error("Clip not found".into()))?;
                let target = next
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?;
                target.clips.push(Clip {
                    start_frame: start,
                    source_offset_frame: offset,
                    length_frames: length,
                    ..c
                });
            }
            Edit::Split { clip_id, at } => {
                let t = next
                    .tracks
                    .iter_mut()
                    .find(|t| t.clips.iter().any(|c| c.id == clip_id))
                    .ok_or_else(|| Error("Clip not found".into()))?;
                let c = t.clips.iter_mut().find(|c| c.id == clip_id).unwrap();
                if at <= c.start_frame || at >= c.end() {
                    return Err(Error("Split must be inside the clip".into()));
                }
                let left_len = at - c.start_frame;
                let right = Clip {
                    id: Id::new_v4(),
                    start_frame: at,
                    source_offset_frame: c.source_offset_frame + left_len,
                    length_frames: c.length_frames - left_len,
                    ..c.clone()
                };
                c.length_frames = left_len;
                t.clips.push(right);
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Edit {
    DeleteTrack(Id),
    DeleteClip(Id),
    Place {
        clip_id: Id,
        track_id: Id,
        start: u64,
        offset: u64,
        length: u64,
    },
    Split {
        clip_id: Id,
        at: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn many_tracks_validate_and_solo_overrides_mute() {
        let mut p = Project::default();
        for _ in 0..32 {
            p.add_track().unwrap();
        }
        assert_eq!(p.tracks.len(), 32);
        p.validate().unwrap();
        p.tracks[0].muted = true;
        p.tracks[0].soloed = true;
        assert!(p.audible(&p.tracks[0]));
        assert!(p.tracks[1..].iter().all(|t| !p.audible(t)));
    }
}
