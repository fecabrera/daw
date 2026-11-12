//! Audio Merge captures immutable clip recipes; it never modifies recordings.
use crate::{
    Asset, COMPOUND_AUDIO_VERSION, ClipChannels, CompoundAudio, Error, Id, Project, Result, Source,
    SourceMetadata, SourceStorage,
};
impl Project {
    pub fn can_merge_audio_clips(&self, ids: &[Id]) -> bool {
        self.merge_targets(ids)
            .is_ok_and(|(_, clips)| clips.iter().all(|c| self.audio_asset(c.asset_id).is_some()))
    }
    pub fn can_merge_clips(&self, ids: &[Id]) -> bool {
        self.can_merge_audio_clips(ids) || self.can_merge_pattern_clips(ids)
    }
    pub fn merged_clips(&self, ids: &[Id]) -> Result<(Self, Id)> {
        if self.can_merge_audio_clips(ids) {
            self.merge_audio_clips(ids)
        } else {
            self.merge_pattern_clips(ids)
        }
    }
    pub fn merge_audio_clips(&self, ids: &[Id]) -> Result<(Self, Id)> {
        let (track, clips) = self.merge_targets(ids)?;
        if clips.iter().any(|c| self.audio_asset(c.asset_id).is_none()) {
            return Err(Error(
                "Audio Merge requires audio clips on one track".into(),
            ));
        }
        let first = clips[0];
        if clips.len() == 1
            && self.audio_asset(first.asset_id).is_some_and(|a| {
                a.compound_audio.is_some()
                    && first.source_offset_frame == 0
                    && first.length_frames == a.decoded_frame_count
            })
            && first.gain_db == 0.0
            && first.transpose == 0
            && first.repeat.is_none()
            && first.stretch.is_none()
            && first.channels == ClipChannels::default()
            && !first.muted
            && first.fades.fade_in.inherit_source
            && first.fades.fade_out.inherit_source
        {
            return Ok((self.clone(), first.id));
        }
        let last = clips.last().unwrap();
        let mut members = Vec::new();
        members
            .try_reserve_exact(clips.len())
            .map_err(|_| Error("Cannot reserve compound audio metadata".into()))?;
        for clip in &clips {
            let mut member = (*clip).clone();
            member.id = Id::new_v4();
            member.start_frame -= first.start_frame;
            members.push(member);
        }
        let source = CompoundAudio {
            version: COMPOUND_AUDIO_VERSION,
            members,
        };
        let length = last.end() - first.start_frame;
        let asset = Asset {
            id: Id::new_v4(),
            name: first.name.clone(),
            rendered_audio: None,
            compound_audio: Some(source),
            midi_pattern: None,
            beat_pattern: None,
            source: Source {
                kind: "compound_audio".into(),
                storage: Some(SourceStorage::Embedded),
                path_kind: "embedded".into(),
                path: String::new(),
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: self.sample_rate_hz,
                channels: 2,
                sample_format: "ieee_float".into(),
                bits_per_sample: 32,
            },
            decoded_frame_count: length,
        };
        let mut fades = first.fades;
        fades.fade_out = last.fades.fade_out;
        fades.fade_in.inherit_source = true;
        fades.fade_out.inherit_source = true;
        let (mut next, id) = self.finish_pattern_merge(track, &clips, asset, false)?;
        let merged = next.clip_mut(id)?;
        merged.fades = fades;
        merged.gain_db = 0.0;
        merged.channels = ClipChannels::default();
        merged.transpose = 0;
        merged.muted = false;
        next.validate()?;
        Ok((next, id))
    }
}
