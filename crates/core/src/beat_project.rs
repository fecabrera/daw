//! Beat ownership at the project boundary. Shared instrument operations resolve stable owner IDs.
use crate::*;
use std::collections::HashSet;

impl Asset {
    pub fn is_audio(&self) -> bool {
        self.midi_pattern.is_none() && self.beat_pattern.is_none()
    }
    pub fn pattern_length_ticks(&self) -> Option<u32> {
        self.midi_pattern
            .as_ref()
            .map(|p| p.length_ticks)
            .or_else(|| self.beat_pattern.as_ref().map(|s| s.pattern.length_ticks()))
    }
}
impl Project {
    /// Project parent and stable instrument owner for ordinary and nested sources.
    pub fn instrument_owners(&self) -> impl Iterator<Item = (Id, Id, &InstrumentSettings)> {
        self.tracks.iter().flat_map(|track| {
            track
                .instrument
                .iter()
                .map(|instrument| (track.id, track.id, instrument))
                .chain(track.beat.iter().flat_map(|beat| {
                    beat.subtracks
                        .iter()
                        .map(|row| (track.id, row.id, &row.instrument))
                }))
        })
    }
    pub fn instruments(&self) -> impl Iterator<Item = &InstrumentSettings> {
        self.instrument_owners()
            .map(|(_, _, instrument)| instrument)
    }
    pub fn instruments_mut(&mut self) -> impl Iterator<Item = &mut InstrumentSettings> {
        self.tracks.iter_mut().flat_map(|t| {
            t.instrument.iter_mut().chain(
                t.beat
                    .iter_mut()
                    .flat_map(|b| b.subtracks.iter_mut().map(|s| &mut s.instrument)),
            )
        })
    }
    pub fn instrument_settings(&self, owner: Id) -> Option<&InstrumentSettings> {
        self.tracks.iter().find_map(|t| {
            if t.id == owner {
                t.instrument.as_ref()
            } else {
                t.beat
                    .as_ref()?
                    .subtracks
                    .iter()
                    .find(|s| s.id == owner)
                    .map(|s| &s.instrument)
            }
        })
    }
    pub fn instrument_settings_mut(&mut self, owner: Id) -> Option<&mut InstrumentSettings> {
        self.instrument_parts_mut(owner).map(|(i, _)| i)
    }
    pub(crate) fn instrument_parts_mut(
        &mut self,
        owner: Id,
    ) -> Option<(&mut InstrumentSettings, &mut [EffectInstance])> {
        for t in &mut self.tracks {
            if t.id == owner {
                return t.instrument.as_mut().map(|i| (i, t.effects.as_mut_slice()));
            }
            if let Some(row) = t
                .beat
                .as_mut()
                .and_then(|b| b.subtracks.iter_mut().find(|s| s.id == owner))
            {
                return Some((&mut row.instrument, &mut []));
            }
        }
        None
    }
    /// Read-only presentation snapshot; never substitutes the live Project or its track selection.
    pub fn instrument_track(&self, owner: Id) -> Option<Track> {
        for track in &self.tracks {
            if track.id == owner {
                return Some(track.clone());
            }
            if let Some(beat) = &track.beat {
                let (name, instrument, gain_db, pan, muted, soloed) = if beat.output.id == owner {
                    (
                        "Output".into(),
                        beat.output_settings(),
                        track.gain_db,
                        track.pan,
                        track.muted,
                        track.soloed,
                    )
                } else if let Some(row) = beat.subtracks.iter().find(|s| s.id == owner) {
                    (
                        row.name.clone(),
                        row.instrument.clone(),
                        0.0,
                        0.0,
                        row.muted,
                        row.soloed,
                    )
                } else {
                    continue;
                };
                // Copy only this owner's presentation data, not sibling instruments or clips.
                return Some(Track {
                    sends: vec![],
                    id: owner,
                    name,
                    kind: TrackKind::Midi,
                    instrument: Some(instrument),
                    beat: None,
                    group_id: None,
                    group_collapsed: false,
                    color: track.color,
                    midi_options: track.midi_options,
                    effects: Vec::new(),
                    effects_compact: track.effects_compact,
                    clips: Vec::new(),
                    gain_db,
                    pan,
                    muted,
                    soloed,
                });
            }
        }
        None
    }
    pub(crate) fn set_instrument_settings(
        &mut self,
        owner: Id,
        settings: Option<InstrumentSettings>,
    ) -> Result<()> {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.id == owner) {
            t.instrument = settings;
            return Ok(());
        }
        if let Some(beat) = self
            .tracks
            .iter_mut()
            .filter_map(|t| t.beat.as_mut())
            .find(|b| b.output.id == owner)
        {
            let settings = settings.ok_or_else(|| Error("Beat Output cannot be removed".into()))?;
            if settings.version != INSTRUMENT_STATE_VERSION
                || settings.main.id != owner
                || !settings.layers.is_empty()
                || !settings.lfo_lane.lfos.is_empty()
                || !settings.envelope_lane.envelopes.is_empty()
            {
                return Err(Error(
                    "Only Output components are allowed on beat Output".into(),
                ));
            }
            beat.output = settings.main;
            return Ok(());
        }
        let settings =
            settings.ok_or_else(|| Error("Beat subtracks require an instrument".into()))?;
        let i = self
            .instrument_settings_mut(owner)
            .ok_or_else(|| Error("Instrument not found".into()))?;
        *i = settings;
        Ok(())
    }
    pub fn beat(&self, track: Id) -> Option<&BeatInstrument> {
        self.tracks.iter().find(|t| t.id == track)?.beat.as_deref()
    }
    pub(crate) fn beat_mut(&mut self, track: Id) -> Result<&mut BeatInstrument> {
        self.tracks
            .iter_mut()
            .find(|t| t.id == track)
            .and_then(|t| t.beat.as_deref_mut())
            .ok_or_else(|| Error("Beat track not found".into()))
    }
    pub fn add_beat_track(&mut self) -> Result<Id> {
        let mut next = self.clone();
        let id = next.add_track()?;
        let count = next
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Beat)
            .count()
            + 1;
        let t = next.tracks.last_mut().unwrap();
        t.name = format!("Beat {count}");
        t.kind = TrackKind::Beat;
        t.beat = Some(Box::default());
        next.validate()?;
        *self = next;
        Ok(id)
    }
    pub fn add_beat_pattern(&mut self, track_id: Id, start_frame: u64) -> Result<Id> {
        let owner = self
            .beat(track_id)
            .ok_or_else(|| Error("Beat track not found".into()))?
            .id;
        let mut next = self.clone();
        let asset = next.beat_asset(owner, "Beat pattern".into(), BeatPattern::default())?;
        let clip = Clip {
            id: Id::new_v4(),
            asset_id: asset.id,
            name: asset.name.clone(),
            start_frame,
            length_frames: asset.decoded_frame_count,
            source_offset_frame: 0,
            muted: false,
            color: None,
            repeat: None,
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
        };
        let id = clip.id;
        next.assets.push(asset);
        next.edit(Edit::OverwriteClips(vec![ClipPlacement { track_id, clip }]))?;
        *self = next;
        Ok(id)
    }
    pub(crate) fn beat_asset(
        &self,
        owner: Id,
        name: String,
        pattern: BeatPattern,
    ) -> Result<Asset> {
        let source = BeatPatternSource::new(owner, pattern)?;
        let frames = self.midi_frame(source.pattern.length_ticks());
        if frames == 0 || frames == u64::MAX {
            return Err(Error("Beat pattern exceeds timing bounds".into()));
        }
        Ok(Asset {
            id: source.id,
            name,
            midi_pattern: None,
            beat_pattern: Some(source),
            decoded_frame_count: frames,
            source: Source {
                kind: "beat_pattern".into(),
                path: String::new(),
                path_kind: "embedded".into(),
                storage: Some(SourceStorage::Embedded),
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: self.sample_rate_hz,
                channels: 1,
                sample_format: "beat".into(),
                bits_per_sample: 0,
            },
        })
    }
    pub(crate) fn replace_beat_pattern(&mut self, clip_id: Id, pattern: BeatPattern) -> Result<()> {
        let clip = self
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == clip_id)
            .ok_or_else(|| Error("Clip not found".into()))?
            .clone();
        let old = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or_else(|| Error("Pattern not found".into()))?;
        let source = old
            .beat_pattern
            .as_ref()
            .ok_or_else(|| Error("Clip is not a beat pattern".into()))?;
        if source.pattern == pattern {
            return Ok(());
        }
        let asset = self.beat_asset(source.instrument_id, old.name.clone(), pattern)?;
        let old_frames = old.decoded_frame_count;
        let new_frames = asset.decoded_frame_count;
        let target = self.clip_mut(clip_id)?;
        target.asset_id = asset.id;
        if old_frames != new_frames {
            let available = target
                .source_length(new_frames)
                .ok_or_else(|| Error("Beat source exceeds timing bounds".into()))?;
            if target.source_offset_frame >= available {
                return Err(Error(
                    "Grid change would exclude the clip's source range".into(),
                ));
            }
            if let Some(repeat) = &mut target.repeat {
                repeat.length_frames = repeat
                    .length_frames
                    .min(available - target.source_offset_frame);
                repeat.phase_frame %= repeat.length_frames;
            } else if target.source_offset_frame == 0
                && target.length_frames == old_frames
                && target.stretch.is_none()
            {
                target.length_frames = new_frames;
            } else {
                target.length_frames = target
                    .length_frames
                    .min(available - target.source_offset_frame);
            }
        }
        self.assets.push(asset);
        Ok(())
    }
    pub(crate) fn validate_beat_destination(&self, track: Id, asset: Id) -> Result<()> {
        if let Some(source) = self
            .assets
            .iter()
            .find(|a| a.id == asset)
            .and_then(|a| a.beat_pattern.as_ref())
        {
            let owner = self
                .beat(track)
                .ok_or_else(|| Error("Beat clips require a beat track".into()))?;
            source.check_destination(owner.id)?;
        }
        Ok(())
    }
    pub(crate) fn validate_beat_track(&self, track: &Track, ids: &mut HashSet<Id>) -> Result<()> {
        if (track.kind == TrackKind::Beat) != track.beat.is_some() {
            return Err(Error(
                "Beat tracks require exactly one beat container".into(),
            ));
        }
        let Some(beat) = &track.beat else {
            return Ok(());
        };
        if track.instrument.is_some() {
            return Err(Error(
                "Beat tracks cannot own a separate MIDI instrument".into(),
            ));
        }
        beat.validate()?;
        for id in std::iter::once(beat.id)
            .chain(std::iter::once(beat.output.id))
            .chain(beat.subtracks.iter().map(|s| s.id))
            .chain(beat.subtracks.iter().flat_map(|s| {
                s.instrument
                    .layers
                    .iter()
                    .map(|l| l.id)
                    .chain(s.instrument.buses().map(|b| b.id))
            }))
        {
            if id.is_nil() || !ids.insert(id) {
                return Err(Error("Duplicate beat owner identity".into()));
            }
        }
        Ok(())
    }
}
impl Project {
    pub fn beat_subtrack(&self, owner: Id) -> Option<(Id, &BeatSubtrack)> {
        self.tracks.iter().find_map(|t| {
            t.beat
                .as_ref()?
                .subtracks
                .iter()
                .find(|s| s.id == owner)
                .map(|s| (t.id, s))
        })
    }
}
impl BeatInstrument {
    /// Output-only control graph. No source or kit-wide LFO/Envelope units are installed.
    pub fn output_settings(&self) -> InstrumentSettings {
        let mut empty = self.output.clone();
        empty.component_order.clear();
        empty.effects.clear();
        empty.envelopes.clear();
        empty.lfos.clear();
        empty.macro_module = MacroModuleSettings::default();
        empty.gain_db = 0.0;
        empty.pan = 0.0;
        let mut lfo = empty.clone();
        lfo.id = Id::from_u128(self.output.id.as_u128().wrapping_add(1));
        let mut envelope = empty;
        envelope.id = Id::from_u128(self.output.id.as_u128().wrapping_add(2));
        InstrumentSettings {
            version: INSTRUMENT_STATE_VERSION,
            layers: Vec::new(),
            main: self.output.clone(),
            lfo_lane: Box::new(lfo),
            envelope_lane: Box::new(envelope),
        }
    }
}
impl Project {
    pub fn instrument_snapshot(&self, owner: Id) -> Option<InstrumentSettings> {
        self.instrument_settings(owner).cloned().or_else(|| {
            self.tracks
                .iter()
                .filter_map(|t| t.beat.as_ref())
                .find(|b| b.output.id == owner)
                .map(|b| b.output_settings())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Project, Id, Id, Id) {
        let mut p = Project::default();
        let track = p.add_beat_track().unwrap();
        let mut beat = p.beat(track).unwrap().clone();
        let row = beat.add_subtrack().unwrap();
        p.edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(beat.subtracks.pop().unwrap()),
                index: 0,
            },
        })
        .unwrap();
        let clip = p.add_beat_pattern(track, 0).unwrap();
        (p, track, row, clip)
    }
    #[test]
    fn embedded_beat_roundtrip_scope_and_immutable_edits_are_transactional() {
        let (mut p, track, row, clip) = fixture();
        let asset = p.tracks[0].clips[0].asset_id;
        let source = p
            .assets
            .iter()
            .find(|a| a.id == asset)
            .unwrap()
            .beat_pattern
            .as_ref()
            .unwrap();
        let pattern = source.pattern.with_step(row, 0, Some(100)).unwrap();
        let mut second = p.tracks[0].clips[0].clone();
        second.id = Id::new_v4();
        second.start_frame = second.end();
        p.edit(Edit::InsertClip {
            track_id: track,
            clip: second.clone(),
        })
        .unwrap();
        p.edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
        assert_ne!(p.tracks[0].clips[0].asset_id, asset);
        assert_eq!(p.tracks[0].clips[1].asset_id, asset);
        let decoded: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, p);
        let foreign = p.add_beat_track().unwrap();
        let before = p.clone();
        assert!(
            p.edit(Edit::OverwriteClips(vec![ClipPlacement {
                track_id: foreign,
                clip: second
            }]))
            .is_err()
        );
        assert_eq!(p, before);
        assert!(p.assets.iter().all(|a| !a.is_audio()));
    }
    #[test]
    fn beat_sampler_ownership_and_audio_settings_include_nested_instruments() {
        let (mut p, track, row, _) = fixture();
        let settings = InstrumentSettings::from(OscillatorSettings::default());
        p.edit(Edit::SetInstrument {
            track_id: row,
            instrument: Some(settings),
        })
        .unwrap();
        let before = p.tracks[0].clips[0].length_frames;
        let scaled = p.with_audio_settings(96_000, 256).unwrap();
        assert_eq!(scaled.tracks[0].clips[0].length_frames, 2 * before);
        scaled.validate().unwrap();
        p.edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
        assert_eq!(p.tracks[0].clips[0].length_frames, 2 * before);
        let copy = Id::new_v4();
        p.edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: copy,
        })
        .unwrap();
        let a = p.beat(track).unwrap();
        let b = p.beat(copy).unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(a.subtracks[0].id, b.subtracks[0].id);
        assert_ne!(
            a.subtracks[0].instrument.layers[0].id,
            b.subtracks[0].instrument.layers[0].id
        );
    }
    #[test]
    fn output_and_nested_effects_use_shared_chain_and_component_transactions() {
        let (mut p, track, row, _) = fixture();
        let main = p.instrument_settings(row).unwrap().main.id;
        for (owner, bus) in [
            (row, main),
            (
                p.beat(track).unwrap().output.id,
                p.beat(track).unwrap().output.id,
            ),
        ] {
            let mut settings = p.instrument_snapshot(owner).unwrap();
            settings.main.macro_module.add();
            p.edit(Edit::SetInstrument {
                track_id: owner,
                instrument: Some(settings),
            })
            .unwrap();
            assert!(
                p.effect_chain(EffectChainTarget::Layer {
                    track: owner,
                    layer: bus
                })
                .is_some()
            );
            assert!(p.can_transfer_component(owner, bus, bus, InstrumentComponent::Macros, false));
        }
        p.validate().unwrap();
    }

    #[test]
    fn nested_presentation_snapshots_keep_local_settings_and_parent_metadata() {
        let (mut p, track, row, _) = fixture();
        p.tracks[0].gain_db = -9.0;
        p.tracks[0].pan = 0.25;
        p.tracks[0].effects.push(EffectInstance::limiter());
        let beat = p.tracks[0].beat.as_mut().unwrap();
        beat.add_subtrack().unwrap();
        beat.subtracks[0].muted = true;
        beat.subtracks[0].instrument.main.gain_db = -3.0;
        let output = beat.output.id;
        let before = p.clone();
        let mut snapshot = p.instrument_track(row).unwrap();
        assert_eq!(snapshot.id, row);
        assert_eq!(snapshot.instrument.as_ref(), p.instrument_settings(row));
        assert_eq!(snapshot.color, p.tracks[0].color);
        assert_eq!(snapshot.midi_options, p.tracks[0].midi_options);
        assert!(snapshot.muted);
        assert_eq!((snapshot.gain_db, snapshot.pan), (0.0, 0.0));
        assert!(
            snapshot.beat.is_none() && snapshot.clips.is_empty() && snapshot.effects.is_empty()
        );
        snapshot.instrument.as_mut().unwrap().main.gain_db = 6.0;
        assert_eq!(p, before, "presentation edits must not mutate saved owners");
        let final_output = p.instrument_track(output).unwrap();
        assert_eq!(final_output.id, output);
        assert_eq!((final_output.gain_db, final_output.pan), (-9.0, 0.25));
        assert_eq!(
            final_output.instrument,
            Some(p.beat(track).unwrap().output_settings())
        );
        assert!(
            final_output.beat.is_none()
                && final_output.clips.is_empty()
                && final_output.effects.is_empty()
        );
        assert_eq!(p.instrument_track(track), Some(p.tracks[0].clone()));
    }

    #[test]
    #[ignore = "manual optimized nested-instrument snapshot scaling probe"]
    fn beat_instrument_snapshot_scaling() {
        for count in [16, 64, 128, 256] {
            let mut project = Project::default();
            let track = project.add_beat_track().unwrap();
            project.beat_mut(track).unwrap().subtracks = (0..count)
                .map(|index| BeatSubtrack::new(format!("Row {index}")))
                .collect();
            let owners: Vec<_> = project
                .beat(track)
                .unwrap()
                .subtracks
                .iter()
                .map(|r| r.id)
                .collect();
            let started = std::time::Instant::now();
            for _ in 0..10 {
                for &owner in &owners {
                    std::hint::black_box(project.instrument_track(std::hint::black_box(owner)));
                }
            }
            eprintln!(
                "rows={count} average_snapshot_sweep_ms={:.3}",
                started.elapsed().as_secs_f64() * 100.0
            );
        }
    }
}
