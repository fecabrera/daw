mod sampler;
pub use daw_unit_core::{
    DIRECTION_MIXER_TYPE_ID, DirectionMixerInput, DirectionMixerSettings, SamplerInput,
    SamplerMode, SamplerSettings, SamplerTrigger,
};
mod midi;
pub use daw_unit_core::{
    AUTO_FILTER_TYPE_ID, AutoFilterSettings, COMPRESSOR_TYPE_ID, CompressorDetection,
    CompressorSettings, CurveShape, DRIVE_TYPE_ID, DriveColorShape, DriveSettings, EQ_TYPE_ID,
    EffectInstance, EffectSettings, EnvelopeSettings, EnvelopeStageSettings, EnvelopeTiming,
    EqBandSettings, EqSettings, FilterShape, INSTRUMENT_STATE_VERSION, InstrumentLayerSettings,
    InstrumentSettings, LIMITER_TYPE_ID, LfoRateMode, LfoWave, LimiterMode, LimiterRouting,
    LimiterSettings, MAX_ENVELOPE_TIME_MS, NoteRate, OTT_TYPE_ID, OscillatorInstrumentSettings,
    OscillatorSettings, OttBandSettings, OttSettings, SILENT_GAIN_DB, Waveform,
};
pub use daw_unit_core::{
    AmplitudeSettings, EnvelopeInstance, EnvelopeMode, InstrumentBusSettings, LfoInstance,
    LfoPolarity, LfoRestart, LfoSettings, MacroInstance, MacroModuleSettings, MacroSettings,
    Modulation, ModulationParameter, ModulationSourceId, ModulationValue,
};
pub use midi::{MIDI_TICKS_PER_BEAT, MidiNote, MidiPattern, MidiPatternBounds, TrackKind};
mod audio_settings;
mod stretch_engine;
pub use stretch_engine::{StretchEngine, StretchEngineId};
mod boundary;
mod component_group;
mod component_transfer;
mod effects;
pub use component_group::ComponentTransfer;
pub use component_transfer::InstrumentComponent;
mod looping;
mod placement;
mod resize;
pub use placement::ClipPlacement;
mod trim;
pub use trim::ClipEdge;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use uuid::Uuid as Id;
pub const SAMPLE_RATE: u32 = 48_000;
pub const SAMPLE_RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];
pub const BUFFER_SIZES: [u32; 8] = [32, 64, 128, 256, 512, 1024, 2048, 4096];

fn default_buffer_size() -> u32 {
    512
}
pub const DEFAULT_TEMPO_BPM: f32 = 120.0;

fn default_tempo_bpm() -> f32 {
    DEFAULT_TEMPO_BPM
}

/// An opaque display color, independent of the UI toolkit and audio engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// The original clip color, retained when explicitly stored in older projects.
pub const LEGACY_CLIP_COLOR: RgbColor = RgbColor {
    r: 43,
    g: 81,
    b: 99,
};

/// Material Design 400 shades, ordered from Deep Orange through Red.
/// https://mui.com/material-ui/customization/color/#color-palette
pub const DEFAULT_TRACK_COLORS: [RgbColor; 16] = [
    rgb(0xff7043), // Deep Orange
    rgb(0xffa726), // Orange
    rgb(0xffca28), // Amber
    rgb(0xffee58), // Yellow
    rgb(0xd4e157), // Lime
    rgb(0x9ccc65), // Light Green
    rgb(0x66bb6a), // Green
    rgb(0x26a69a), // Teal
    rgb(0x26c6da), // Cyan
    rgb(0x29b6f6), // Light Blue
    rgb(0x42a5f5), // Blue
    rgb(0x5c6bc0), // Indigo
    rgb(0x7e57c2), // Deep Purple
    rgb(0xab47bc), // Purple
    rgb(0xec407a), // Pink
    rgb(0xef5350), // Red
];

pub const DEFAULT_TRACK_COLOR: RgbColor = DEFAULT_TRACK_COLORS[0];

const fn rgb(hex: u32) -> RgbColor {
    RgbColor {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
    }
}

/// Assign defaults by zero-based track position, repeating after all 16 hues.
pub const fn default_track_color(index: usize) -> RgbColor {
    DEFAULT_TRACK_COLORS[index % DEFAULT_TRACK_COLORS.len()]
}

fn first_track_color() -> RgbColor {
    DEFAULT_TRACK_COLOR
}

fn deserialize_tracks<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Track>, D::Error> {
    // Capture whether color was present before flattening into the required Track fields.
    #[derive(Deserialize)]
    struct StoredTrack {
        #[serde(default, deserialize_with = "deserialize_present_color")]
        color: Option<RgbColor>,
        #[serde(flatten)]
        track: Track,
    }
    fn deserialize_present_color<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Option<RgbColor>, D::Error> {
        RgbColor::deserialize(deserializer).map(Some)
    }
    Vec::<StoredTrack>::deserialize(deserializer).map(|tracks| {
        tracks
            .into_iter()
            .enumerate()
            .map(|(index, stored)| Track {
                color: stored.color.unwrap_or_else(|| default_track_color(index)),
                ..stored.track
            })
            .collect()
    })
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub stretch_engine: StretchEngine,
    pub project_id: Id,
    pub name: String,
    pub sample_rate_hz: u32,
    #[serde(default = "default_buffer_size")]
    pub buffer_size_frames: u32,
    #[serde(default = "default_tempo_bpm")]
    pub tempo_bpm: f32,
    pub master: Master,
    pub assets: Vec<Asset>,
    #[serde(deserialize_with = "deserialize_tracks")]
    pub tracks: Vec<Track>,
    pub transport: Transport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Master {
    pub gain_db: f32,
    #[serde(default)]
    pub effects: Vec<EffectInstance>,
    #[serde(default)]
    pub effects_compact: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_pattern: Option<MidiPattern>,
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
    #[serde(default)]
    pub effects: Vec<EffectInstance>,
    #[serde(default)]
    pub effects_compact: bool,
    #[serde(default)]
    pub kind: TrackKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument: Option<InstrumentSettings>,
    pub id: Id,
    pub name: String,
    /// Default color for this track's clips; older projects assign it by track position.
    #[serde(default = "first_track_color")]
    pub color: RgbColor,
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
    /// None inherits the containing track's color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<RgbColor>,
    pub start_frame: u64,
    pub source_offset_frame: u64,
    pub length_frames: u64,
    /// Repeat this source range without duplicating the underlying audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<ClipLoop>,
    /// Ratio applied to the original asset. Offsets and loop ranges use stretched frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretch: Option<ClipStretch>,
}

pub use daw_unit_core::{ClipLoop, ClipStretch};

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
            stretch_engine: StretchEngine::default(),
            project_id: Id::new_v4(),
            name: "Untitled".into(),
            sample_rate_hz: SAMPLE_RATE,
            buffer_size_frames: default_buffer_size(),
            tempo_bpm: DEFAULT_TEMPO_BPM,
            master: Master {
                gain_db: 0.0,
                effects: vec![],
                effects_compact: false,
            },
            assets: vec![],
            tracks: vec![],
            transport: Transport::default(),
        }
    }
}
impl Clip {
    pub fn source_length(&self, original_frames: u64) -> Option<u64> {
        self.stretch.map_or(Some(original_frames), |stretch| {
            stretch.scale(original_frames)
        })
    }

    /// Rescale the current source position and repeat base without changing the source asset.
    pub fn stretched_to(&self, start: u64, length: u64) -> Option<Self> {
        let change = ClipStretch::new(self.length_frames, length)?;
        let old = self.stretch.unwrap_or(ClipStretch {
            source_frames: 1,
            output_frames: 1,
        });
        // Reduce cross factors before multiplication to keep repeated edits within u64.
        let a = ClipStretch::new(old.source_frames, change.output_frames)?;
        let b = ClipStretch::new(change.source_frames, old.output_frames)?;
        let stretch = ClipStretch::new(
            a.source_frames.checked_mul(b.source_frames)?,
            a.output_frames.checked_mul(b.output_frames)?,
        )?;
        if !stretch.valid() {
            return None;
        }
        let mut clip = self.clone();
        clip.start_frame = start;
        clip.length_frames = length;
        clip.source_offset_frame = change.scale(self.source_offset_frame)?;
        clip.repeat = match self.repeat {
            Some(repeat) => {
                let length_frames = change.scale(repeat.length_frames)?.max(1);
                Some(ClipLoop {
                    length_frames,
                    phase_frame: change.scale(repeat.phase_frame)?.min(length_frames - 1),
                })
            }
            None => None,
        };
        clip.stretch = (stretch.source_frames != stretch.output_frames).then_some(stretch);
        Some(clip)
    }
    pub fn end(&self) -> u64 {
        self.start_frame.saturating_add(self.length_frames)
    }

    /// Map a timeline position inside a validated clip to its source frame.
    pub fn source_frame(&self, local: u64) -> u64 {
        self.source_offset_frame + self.repeat.map_or(local, |repeat| repeat.position(local))
    }

    /// Restore the original base at one copy, or unwrap a contiguous partial range.
    pub fn restore_source_range(&mut self) {
        let Some(repeat) = self.repeat else {
            return;
        };
        if self.length_frames == repeat.length_frames {
            // Returning to one copy exits looping, including after opposite-edge resizes.
            self.repeat = None;
        } else if self.length_frames <= repeat.length_frames - repeat.phase_frame {
            self.source_offset_frame += repeat.phase_frame;
            self.repeat = None;
        }
    }
}
/// Shared track/layer solo precedence. Solo overrides mute when any peer is soloed.
pub fn mix_audible(muted: bool, soloed: bool, any_solo: bool) -> bool {
    if any_solo { soloed } else { !muted }
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
    /// Track and Master instances share validation, monitoring, and missing-unit handling.
    pub fn all_effects(&self) -> impl Iterator<Item = &EffectInstance> {
        self.tracks
            .iter()
            .flat_map(|track| &track.effects)
            .chain(&self.master.effects)
    }
    pub fn frames(&self, seconds: f64) -> u64 {
        (seconds.max(0.0) * f64::from(self.sample_rate_hz)).round() as u64
    }

    pub fn seconds(&self, frames: u64) -> f64 {
        frames as f64 / f64::from(self.sample_rate_hz)
    }

    pub fn end(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(Clip::end)
            .max()
            .unwrap_or(0)
    }
    pub fn audible(&self, track: &Track) -> bool {
        mix_audible(
            track.muted,
            track.soloed,
            self.tracks.iter().any(|t| t.soloed),
        )
    }
    pub fn validate(&self) -> Result<()> {
        self.stretch_engine.validate()?;
        let invalid = |text: &str| Error(text.into());
        if !SAMPLE_RATES.contains(&self.sample_rate_hz) {
            return Err(invalid("Unsupported project sample rate"));
        }
        if !BUFFER_SIZES.contains(&self.buffer_size_frames) {
            return Err(invalid("Unsupported project buffer size"));
        }
        if !self.tempo_bpm.is_finite() || self.tempo_bpm <= 0.0 {
            return Err(invalid("Tempo must be a positive finite BPM value"));
        }
        if !self.master.gain_db.is_finite() || !linear_gain(self.master.gain_db).is_finite() {
            return Err(invalid("Invalid master gain"));
        }
        let mut ids = std::collections::HashSet::<Uuid>::new();
        ids.insert(self.project_id);
        if self
            .master
            .effects
            .iter()
            .any(|e| e.unit.modulations().iter().any(|r| r.source.is_some()))
        {
            return Err(invalid(
                "Master effects cannot reference instrument envelopes",
            ));
        }
        for effect in self.all_effects() {
            if effect.id.is_nil() || !ids.insert(effect.id) || !effect.unit.valid() {
                return Err(invalid("Invalid effect settings or duplicate effect ID"));
            }
        }
        for a in &self.assets {
            if !ids.insert(a.id) {
                return Err(invalid("Duplicate asset ID"));
            }
            if let Some(pattern) = &a.midi_pattern {
                pattern.validate()?;
                if a.source.kind != "midi_pattern"
                    || a.source.path_kind != "embedded"
                    || !a.source.path.is_empty()
                    || a.decoded_frame_count == 0
                    || a.decoded_frame_count == u64::MAX
                    || a.decoded_frame_count != self.midi_frame(pattern.length_ticks)
                    || pattern.notes.iter().any(|note| {
                        self.midi_frame(note.start_tick + note.length_ticks)
                            <= self.midi_frame(note.start_tick)
                    })
                {
                    return Err(invalid("Invalid MIDI pattern source"));
                }
                continue;
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
            if t.instrument
                .as_ref()
                .is_some_and(|instrument| t.kind != TrackKind::Midi || !instrument.valid())
            {
                return Err(invalid(
                    "Invalid instrument or source instrument on an audio track",
                ));
            }
            self.validate_samplers(t)?;
            for route in t.instrument.iter().flat_map(|i| &i.layers).flat_map(|l| {
                l.amplitude
                    .modulations
                    .iter()
                    .chain(&l.oscillator_modulations)
                    .chain(l.sampler.iter().flat_map(|s| &s.modulations))
            }) {
                if route.source.is_some_and(|source| {
                    !t.instrument
                        .as_ref()
                        .is_some_and(|i| i.modulation_source(source).is_some())
                }) {
                    return Err(invalid(
                        "Instrument modulation source does not exist in this instrument",
                    ));
                }
            }
            for route in t.effects.iter().flat_map(|e| e.unit.modulations()) {
                if route.source.is_some_and(|source| {
                    !t.instrument.as_ref().is_some_and(|i| {
                        i.modulation_source(source).is_some_and(|s| {
                            daw_unit_core::ModulationScope::Shared.accepts(s.scope)
                        })
                    })
                }) {
                    return Err(invalid(
                        "Effect modulation requires a shared source in the same MIDI track",
                    ));
                }
            }
            let mut previous_end = 0;
            let mut clips = t.clips.iter().collect::<Vec<_>>();
            clips.sort_by_key(|c| c.start_frame);
            for c in clips {
                if !ids.insert(c.id) {
                    return Err(invalid("Duplicate clip ID"));
                }
                self.validate_clip(c)?;
                let asset = self.assets.iter().find(|a| a.id == c.asset_id).unwrap();
                if (t.kind == TrackKind::Midi) != asset.midi_pattern.is_some() {
                    return Err(invalid("Clip source type does not match track type"));
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
    fn validate_clip(&self, c: &Clip) -> Result<()> {
        let invalid = |s: &str| Error(s.into());
        let a = self
            .assets
            .iter()
            .find(|a| a.id == c.asset_id)
            .ok_or_else(|| invalid("Clip references a missing asset record"))?;
        if c.repeat.is_some_and(|repeat| {
            repeat.length_frames == 0 || repeat.phase_frame >= repeat.length_frames
        }) {
            return Err(invalid("Invalid clip loop length or phase"));
        }
        if c.stretch.is_some_and(|stretch| !stretch.valid()) {
            return Err(invalid(
                "Clip stretch must be between 1/8 and 8 times the source duration",
            ));
        }
        let available = c
            .source_length(a.decoded_frame_count)
            .ok_or_else(|| invalid("Stretched source length exceeds supported bounds"))?;
        let source_length = c
            .repeat
            .map_or(c.length_frames, |repeat| repeat.length_frames);
        if c.length_frames == 0
            || c.start_frame.checked_add(c.length_frames).is_none()
            || c.source_offset_frame
                .checked_add(source_length)
                .is_none_or(|end| end > available)
        {
            return Err(invalid("Clip range exceeds saved asset bounds"));
        }
        Ok(())
    }
    pub fn add_track(&mut self) -> Result<Id> {
        let id = Id::new_v4();
        self.tracks.push(Track {
            effects: vec![],
            effects_compact: false,
            kind: TrackKind::Audio,
            instrument: None,
            id,
            name: format!("Audio {}", self.tracks.len() + 1),
            color: default_track_color(self.tracks.len()),
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
        next.apply_edit(command)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn apply_edit(&mut self, command: Edit) -> Result<()> {
        match command {
            Edit::SetMasterEffects { effects, compact } => {
                self.master.effects = effects;
                self.master.effects_compact = compact;
            }
            Edit::SetEffects {
                track_id,
                effects,
                compact,
            } => {
                let track = self
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?;
                track.effects = effects;
                track.effects_compact = compact;
            }
            Edit::ResizeMidiPattern {
                clip_id,
                start_tick,
                end_tick,
            } => {
                self.apply_midi_pattern_resize(clip_id, start_tick, end_tick)?;
            }
            Edit::ReplaceMidiPattern { clip_id, pattern } => {
                self.replace_midi_pattern(clip_id, pattern)?
            }
            Edit::TransferInstrumentComponent {
                track_id,
                source,
                destination,
                destination_index,
                component,
                duplicate,
            } => {
                self.apply_component_transfer(
                    track_id,
                    source,
                    destination,
                    destination_index,
                    component,
                    duplicate,
                )?;
            }
            Edit::SetInstrument {
                track_id,
                instrument,
            } => {
                self.tracks
                    .iter_mut()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?
                    .instrument = instrument;
            }
            Edit::Batch(commands) => {
                for command in commands {
                    self.apply_edit(command)?;
                }
            }
            Edit::OverwriteClips(clips) => self.apply_placements(&clips)?,
            Edit::ResizeClips { clips, edge } => self.apply_resizes(&clips, edge)?,
            Edit::TrimBoundary {
                clip_id,
                selected,
                edge,
                delta,
            } => {
                self.apply_trim_boundary(clip_id, &selected, edge, delta)?;
            }
            Edit::StretchClips { clips, edge } => self.apply_stretches(&clips, edge)?,
            Edit::SetTempo { bpm } => {
                if !bpm.is_finite() || bpm <= 0.0 {
                    return Err(Error("Tempo must be a positive finite BPM value".into()));
                }
                if bpm != self.tempo_bpm {
                    // Beat position = frames * BPM / (sample rate * 60).
                    // Audio lengths stay fixed. MIDI ranges scale with their beat-timed notes.
                    let ratio = f64::from(self.tempo_bpm) / f64::from(bpm);
                    let checked_frame = |frame: f64| {
                        // u64::MAX rounds up to 2^64 as f64. Reject before casting.
                        if !frame.is_finite() || frame < 0.0 || frame >= u64::MAX as f64 {
                            return Err(Error("Tempo change exceeds timeline bounds".into()));
                        }
                        Ok(frame as u64)
                    };
                    for track in &mut self.tracks {
                        for clip in &mut track.clips {
                            if track.kind == TrackKind::Midi {
                                clip.length_frames =
                                    checked_frame((clip.length_frames as f64 * ratio).round())?;
                                clip.source_offset_frame = checked_frame(
                                    (clip.source_offset_frame as f64 * ratio).round(),
                                )?;
                                if let Some(repeat) = &mut clip.repeat {
                                    repeat.length_frames = checked_frame(
                                        (repeat.length_frames as f64 * ratio).round(),
                                    )?;
                                    repeat.phase_frame =
                                        checked_frame((repeat.phase_frame as f64 * ratio).round())?;
                                }
                            }
                            clip.start_frame =
                                checked_frame((clip.start_frame as f64 * ratio).round())?;
                        }
                    }
                    for asset in &mut self.assets {
                        if let Some(pattern) = &asset.midi_pattern {
                            asset.decoded_frame_count = checked_frame(
                                (f64::from(pattern.length_ticks)
                                    * f64::from(self.sample_rate_hz)
                                    * 60.0
                                    / (f64::from(bpm) * f64::from(MIDI_TICKS_PER_BEAT)))
                                .round(),
                            )?;
                        }
                    }
                    self.bound_retimed_midi_sources()?;
                    let region = &mut self.transport.r#loop;
                    // Match ruler snapping: fractional beat boundaries must not land
                    // in the preceding beat in the selection monitor.
                    region.start_frame = checked_frame((region.start_frame as f64 * ratio).ceil())?;
                    region.end_frame = checked_frame((region.end_frame as f64 * ratio).ceil())?;
                    self.tempo_bpm = bpm;
                }
            }
            Edit::SetTrackMuted { track_id, muted } => {
                self.tracks
                    .iter_mut()
                    .find(|track| track.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?
                    .muted = muted;
            }
            Edit::SetTrackSoloed { track_id, soloed } => {
                self.tracks
                    .iter_mut()
                    .find(|track| track.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?
                    .soloed = soloed;
            }
            Edit::SetTrackColor { track_id, color } => {
                self.tracks
                    .iter_mut()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?
                    .color = color;
            }
            Edit::SetClipColor { clip_id, color } => {
                self.tracks
                    .iter_mut()
                    .flat_map(|t| &mut t.clips)
                    .find(|c| c.id == clip_id)
                    .ok_or_else(|| Error("Clip not found".into()))?
                    .color = color;
            }
            Edit::DeleteTrack(id) => self.tracks.retain(|t| t.id != id),
            Edit::DuplicateTrack {
                track_id,
                new_track_id,
            } => {
                let index = self
                    .tracks
                    .iter()
                    .position(|track| track.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?;
                let mut copy = self.tracks[index].clone();
                copy.id = new_track_id;
                copy.clips.clear();
                for s in copy
                    .instrument
                    .iter_mut()
                    .flat_map(|i| &mut i.layers)
                    .filter_map(|l| l.sampler.as_mut())
                {
                    s.id = Id::new_v4();
                }
                for effect in &mut copy.effects {
                    effect.id = Id::new_v4();
                }
                self.tracks.insert(index + 1, copy);
            }
            Edit::ReorderTrack { track_id, index } => {
                if index >= self.tracks.len() {
                    return Err(Error("Track position is out of bounds".into()));
                }
                let source = self
                    .tracks
                    .iter()
                    .position(|track| track.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?;
                let track = self.tracks.remove(source);
                self.tracks.insert(index, track);
            }
            Edit::DeleteClip(id) => {
                for t in &mut self.tracks {
                    t.clips.retain(|c| c.id != id);
                }
            }
            Edit::InsertClip { track_id, clip } => {
                self.tracks
                    .iter_mut()
                    .find(|track| track.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?
                    .clips
                    .push(clip);
            }
            Edit::Place {
                clip_id,
                track_id,
                start,
                offset,
                length,
                repeat,
            } => {
                let c = self
                    .tracks
                    .iter_mut()
                    .find_map(|t| {
                        t.clips
                            .iter()
                            .position(|c| c.id == clip_id)
                            .map(|i| t.clips.remove(i))
                    })
                    .ok_or_else(|| Error("Clip not found".into()))?;
                let target = self
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| Error("Track not found".into()))?;
                target.clips.push(Clip {
                    start_frame: start,
                    source_offset_frame: offset,
                    length_frames: length,
                    repeat,
                    ..c
                });
            }
            Edit::Split { clip_id, at } => {
                let t = self
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
                    source_offset_frame: if c.repeat.is_some() {
                        c.source_offset_frame
                    } else {
                        c.source_offset_frame + left_len
                    },
                    length_frames: c.length_frames - left_len,
                    repeat: c.repeat.map(|repeat| repeat.shifted(i128::from(left_len))),
                    ..c.clone()
                };
                c.length_frames = left_len;
                t.clips.push(right);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Edit {
    /// Move or copy one installed component within an instrument, remapping references atomically.
    TransferInstrumentComponent {
        track_id: Id,
        source: Id,
        destination: Id,
        /// Final source-strip index for oscillator transfers; otherwise a visible destination unit index.
        destination_index: Option<usize>,
        component: InstrumentComponent,
        duplicate: bool,
    },
    SetMasterEffects {
        effects: Vec<EffectInstance>,
        compact: bool,
    },
    SetEffects {
        track_id: Id,
        effects: Vec<EffectInstance>,
        compact: bool,
    },
    ResizeMidiPattern {
        clip_id: Id,
        start_tick: i64,
        end_tick: i64,
    },
    ReplaceMidiPattern {
        clip_id: Id,
        pattern: MidiPattern,
    },
    SetInstrument {
        track_id: Id,
        instrument: Option<InstrumentSettings>,
    },
    /// Apply all commands and validate the final state as one transaction.
    Batch(Vec<Edit>),
    /// Place clips with priority over existing ranges, trimming/splitting neighbors.
    OverwriteClips(Vec<ClipPlacement>),
    /// Resize existing clips and trim or remove intersecting neighbors atomically.
    ResizeClips {
        clips: Vec<Clip>,
        edge: ClipEdge,
    },
    /// Move a touching waveform boundary and trim the remaining selected clips.
    TrimBoundary {
        clip_id: Id,
        selected: Vec<Id>,
        edge: ClipEdge,
        delta: i128,
    },
    /// Apply stretch metadata and trim/remove intersecting neighbors atomically.
    StretchClips {
        clips: Vec<Clip>,
        edge: ClipEdge,
    },
    /// Retain clip start beat positions, rounding to the nearest sample frame.
    /// Scale loop-selection endpoints too, rounding up as with ruler snapping.
    /// Audio lengths stay unchanged; MIDI ranges retain their beat timing.
    /// Final overlap/bounds validation is transactional.
    SetTempo {
        bpm: f32,
    },
    SetTrackMuted {
        track_id: Id,
        muted: bool,
    },
    SetTrackSoloed {
        track_id: Id,
        soloed: bool,
    },
    SetTrackColor {
        track_id: Id,
        color: RgbColor,
    },
    SetClipColor {
        clip_id: Id,
        color: Option<RgbColor>,
    },
    DeleteTrack(Id),
    /// Insert an empty copy after the original, retaining all track settings.
    DuplicateTrack {
        track_id: Id,
        new_track_id: Id,
    },
    /// Move a whole track to a zero-based position in the resulting track order.
    ReorderTrack {
        track_id: Id,
        index: usize,
    },
    DeleteClip(Id),
    InsertClip {
        track_id: Id,
        clip: Clip,
    },
    Place {
        clip_id: Id,
        track_id: Id,
        start: u64,
        offset: u64,
        length: u64,
        repeat: Option<ClipLoop>,
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
    fn track_duplication_preserves_settings_and_order_and_rolls_back_invalid_groups() {
        let mut project = Project::default();
        let first = project.add_track().unwrap();
        let second = project.add_track().unwrap();
        let third = project.add_track().unwrap();
        project.tracks[0].name = "Bass".into();
        project.tracks[0].color = RgbColor { r: 3, g: 4, b: 5 };
        project.tracks[0].gain_db = -7.5;
        project.tracks[0].pan = -0.25;
        project.tracks[0].muted = true;
        project.tracks[2].soloed = true;
        let originals = project.tracks.clone();
        let copies = [Id::new_v4(), Id::new_v4()];
        project
            .edit(Edit::Batch(vec![
                Edit::DuplicateTrack {
                    track_id: third,
                    new_track_id: copies[1],
                },
                Edit::DuplicateTrack {
                    track_id: first,
                    new_track_id: copies[0],
                },
            ]))
            .unwrap();
        assert_eq!(
            project
                .tracks
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            [first, copies[0], second, third, copies[1]]
        );
        for (source, copy) in [(0, 1), (2, 4)] {
            let mut expected = originals[source].clone();
            expected.id = project.tracks[copy].id;
            expected.clips.clear();
            assert_eq!(
                format!("{:?}", project.tracks[copy]),
                format!("{expected:?}")
            );
        }
        let before = format!("{project:?}");
        for (track_id, new_track_id) in [
            (Id::new_v4(), Id::new_v4()),
            (third, first),
            (third, project.project_id),
        ] {
            assert!(
                project
                    .edit(Edit::Batch(vec![
                        Edit::DuplicateTrack {
                            track_id: first,
                            new_track_id: Id::new_v4()
                        },
                        Edit::DuplicateTrack {
                            track_id,
                            new_track_id
                        },
                    ]))
                    .is_err()
            );
            assert_eq!(format!("{project:?}"), before);
        }
    }
    #[test]
    fn mute_and_solo_group_commands_are_atomic() {
        let mut project = Project::default();
        let first = project.add_track().unwrap();
        let second = project.add_track().unwrap();
        project
            .edit(Edit::Batch(vec![
                Edit::SetTrackMuted {
                    track_id: first,
                    muted: true,
                },
                Edit::SetTrackSoloed {
                    track_id: second,
                    soloed: true,
                },
            ]))
            .unwrap();
        assert!(project.tracks[0].muted);
        assert!(project.tracks[1].soloed);
        assert!(!project.tracks[0].soloed && !project.tracks[1].muted);
        let before = format!("{project:?}");
        for invalid_solo in [false, true] {
            let invalid = if invalid_solo {
                Edit::SetTrackSoloed {
                    track_id: Id::new_v4(),
                    soloed: true,
                }
            } else {
                Edit::SetTrackMuted {
                    track_id: Id::new_v4(),
                    muted: true,
                }
            };
            assert!(
                project
                    .edit(Edit::Batch(vec![
                        Edit::SetTrackMuted {
                            track_id: first,
                            muted: false
                        },
                        invalid
                    ]))
                    .is_err()
            );
            assert_eq!(format!("{project:?}"), before);
        }
    }
    #[test]
    fn stretch_scales_offsets_and_repeats_and_can_return_to_original_duration() {
        let clip = Clip {
            id: Id::nil(),
            asset_id: Id::nil(),
            name: "Trimmed".into(),
            color: None,
            start_frame: 100,
            source_offset_frame: 200,
            length_frames: 100,
            repeat: Some(ClipLoop {
                length_frames: 40,
                phase_frame: 10,
            }),
            stretch: None,
        };
        let longer = clip.stretched_to(0, 200).unwrap();
        assert_eq!(longer.source_offset_frame, 400);
        assert_eq!(
            longer.repeat,
            Some(ClipLoop {
                length_frames: 80,
                phase_frame: 20
            })
        );
        assert_eq!(longer.source_length(1000), Some(2000));
        assert_eq!(longer.stretch, ClipStretch::new(1, 2));
        let restored = longer.stretched_to(100, 100).unwrap();
        assert_eq!(restored.stretch, None);
        assert_eq!(restored.source_offset_frame, clip.source_offset_frame);
        assert_eq!(restored.repeat, clip.repeat);
        assert!(clip.stretched_to(0, 12).is_none());
        assert!(clip.stretched_to(0, 801).is_none());
        assert!(clip.stretched_to(0, 0).is_none());
        assert!(
            !ClipStretch {
                source_frames: 0,
                output_frames: 1
            }
            .valid()
        );
        assert_eq!(
            ClipStretch {
                source_frames: 1,
                output_frames: u64::MAX
            }
            .scale(2),
            None
        );
    }
    #[test]
    fn restoring_source_trimming_restores_the_base_and_preserves_partial_audio_ranges() {
        for (length, phase, restored) in [
            (10, 0, true),
            (10, 3, true),
            (7, 3, true),
            (8, 3, false),
            (13, 0, false),
        ] {
            let mut clip = Clip {
                stretch: None,
                id: Id::nil(),
                asset_id: Id::nil(),
                name: "Trimmed".into(),
                color: None,
                start_frame: 20,
                source_offset_frame: 100,
                length_frames: length,
                repeat: Some(ClipLoop {
                    length_frames: 10,
                    phase_frame: phase,
                }),
            };
            let before = clip.clone();
            clip.restore_source_range();
            assert_eq!(clip.repeat.is_none(), restored);
            assert_eq!(clip.start_frame, before.start_frame);
            assert_eq!(clip.length_frames, before.length_frames);
            assert_eq!(
                clip.source_offset_frame,
                100 + if restored && length < 10 { phase } else { 0 }
            );
            for local in 0..length {
                assert_eq!(
                    clip.source_frame(local),
                    if length == 10 {
                        100 + local
                    } else {
                        before.source_frame(local)
                    }
                );
            }
            let offset = clip.source_offset_frame;
            clip.restore_source_range();
            assert_eq!(clip.source_offset_frame, offset);
        }
    }

    #[test]
    fn clip_loop_phase_wraps_in_both_directions_without_overflow() {
        let repeat = ClipLoop {
            length_frames: 4,
            phase_frame: 3,
        };
        assert_eq!(
            (0..8).map(|n| repeat.position(n)).collect::<Vec<_>>(),
            [3, 0, 1, 2, 3, 0, 1, 2]
        );
        assert_eq!(repeat.shifted(-6).phase_frame, 1);
        assert_eq!(repeat.shifted(10).phase_frame, 1);
        let large = ClipLoop {
            length_frames: u64::MAX,
            phase_frame: u64::MAX - 1,
        };
        assert_eq!(large.position(1), 0);
        assert_eq!(large.position(u64::MAX), u64::MAX - 1);
        assert_eq!(large.shifted(i128::MAX).phase_frame, (u64::MAX - 1) / 2 - 1);
        assert!(large.shifted(i128::MIN).phase_frame < large.length_frames);
    }

    #[test]
    fn tempo_must_be_positive_and_finite() {
        let mut project = Project::default();
        for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            project.tempo_bpm = value;
            assert!(project.validate().is_err());
        }
        project.tempo_bpm = 123.45;
        project.validate().unwrap();
    }
    #[test]
    fn new_tracks_cycle_material_400_colors_without_recoloring_existing_tracks() {
        let mut project = Project::default();
        for index in 0..34 {
            project.add_track().unwrap();
            assert_eq!(
                project.tracks[index].color,
                DEFAULT_TRACK_COLORS[index % 16]
            );
        }
        assert_eq!(
            project.tracks[0].color,
            RgbColor {
                r: 255,
                g: 112,
                b: 67
            }
        );
        assert_eq!(
            project.tracks[1].color,
            RgbColor {
                r: 255,
                g: 167,
                b: 38
            }
        );
        assert_eq!(
            project.tracks[15].color,
            RgbColor {
                r: 239,
                g: 83,
                b: 80
            }
        );
        project.tracks[0].color = RgbColor { r: 3, g: 4, b: 5 };
        let removed = project.tracks[4].id;
        let before = project
            .tracks
            .iter()
            .filter(|t| t.id != removed)
            .map(|t| (t.id, t.color))
            .collect::<Vec<_>>();
        project.edit(Edit::DeleteTrack(removed)).unwrap();
        project.add_track().unwrap();
        assert_eq!(
            project.tracks.last().unwrap().color,
            default_track_color(33)
        );
        assert_eq!(
            project.tracks[..33]
                .iter()
                .map(|t| (t.id, t.color))
                .collect::<Vec<_>>(),
            before
        );
        project.validate().unwrap();
    }

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
