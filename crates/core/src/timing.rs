//! Frame-owned conductor events and one compiled musical clock.
use crate::{Asset, Clip, Error, Id, Project, Result, TimeSignature, TrackKind};
use serde::{Deserialize, Serialize};

pub const TIMING_EVENT_LIMIT: usize = 4096;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrangementMarker {
    pub id: Id,
    pub start_frame: u64,
    pub name: String,
    /// Missing in early marker manifests; resolve those as one bar on the clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_frames: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoEvent {
    pub id: Id,
    pub start_frame: u64,
    pub bpm: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignatureEvent {
    pub id: Id,
    pub start_frame: u64,
    pub signature: TimeSignature,
}
/// Empty lists are the lossless legacy constant-map representation. The scalar
/// fields remain the reference clock for immutable pattern/source frame ranges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimingMap {
    #[serde(default = "timing_version")]
    pub version: u32,
    #[serde(default)]
    pub tempos: Vec<TempoEvent>,
    #[serde(default)]
    pub signatures: Vec<SignatureEvent>,
}
fn timing_version() -> u32 {
    1
}
impl Default for TimingMap {
    fn default() -> Self {
        Self {
            version: timing_version(),
            tempos: Vec::new(),
            signatures: Vec::new(),
        }
    }
}
impl TimingMap {
    pub fn is_empty(&self) -> bool {
        self.tempos.is_empty() && self.signatures.is_empty()
    }
}
#[derive(Clone, Debug)]
struct TempoSegment {
    frame: u64,
    quarter: f64,
    bpm: f32,
}
#[derive(Clone, Debug)]
struct MeterSegment {
    frame: u64,
    quarter: f64,
    bar: u64,
    signature: TimeSignature,
}
/// Prepared on a control/worker thread. Queries neither allocate nor mutate.
#[derive(Clone, Debug)]
pub struct MusicalClock {
    rate: u32,
    tempos: Vec<TempoSegment>,
    meters: Vec<MeterSegment>,
}
/// Retime source coordinates on the reference tempo, independently of timeline reprojection.
pub(super) fn retime_pattern_source(
    clip: &mut Clip,
    ratio: f64,
    checked_frame: impl Fn(f64) -> Result<u64>,
) -> Result<()> {
    let scale = |frame: u64| checked_frame((frame as f64 * ratio).round());
    clip.source_offset_frame = scale(clip.source_offset_frame)?;
    if let Some(repeat) = &mut clip.repeat {
        repeat.length_frames = scale(repeat.length_frames)?;
        repeat.phase_frame = scale(repeat.phase_frame)?;
    }
    Ok(())
}

pub(super) fn retime_pattern_assets(
    assets: &mut [Asset],
    rate: u32,
    bpm: f32,
    checked_frame: impl Fn(f64) -> Result<u64>,
) -> Result<()> {
    for asset in assets {
        if let Some(ticks) = asset.pattern_length_ticks() {
            asset.decoded_frame_count = checked_frame(
                (f64::from(ticks) * f64::from(rate) * 60.0
                    / (f64::from(bpm) * f64::from(crate::MIDI_TICKS_PER_BEAT)))
                .round(),
            )?;
        }
    }
    Ok(())
}

fn checked_frame(value: f64) -> Result<u64> {
    if value.is_finite() && value >= 0.0 && value < u64::MAX as f64 {
        Ok(value as u64)
    } else {
        Err(Error("Musical timing exceeds timeline bounds".into()))
    }
}
impl MusicalClock {
    pub fn constant(rate: u32, bpm: f32, signature: TimeSignature) -> Self {
        Self {
            rate,
            tempos: vec![TempoSegment {
                frame: 0,
                quarter: 0.0,
                bpm,
            }],
            meters: vec![MeterSegment {
                frame: 0,
                quarter: 0.0,
                bar: 1,
                signature,
            }],
        }
    }
    pub fn compile(project: &Project) -> Result<Self> {
        project.validate_timing()?;
        let mut clock = Self::constant(
            project.sample_rate_hz,
            project.tempo_bpm,
            project.time_signature,
        );
        for event in project.timing.tempos.iter().skip(1) {
            let quarter = clock.quarter_at(event.start_frame);
            if !quarter.is_finite() || quarter <= clock.tempos.last().unwrap().quarter {
                return Err(Error(
                    "Tempo map exceeds musical precision or timeline bounds".into(),
                ));
            }
            clock.tempos.push(TempoSegment {
                frame: event.start_frame,
                quarter,
                bpm: event.bpm,
            });
        }
        for event in project.timing.signatures.iter().skip(1) {
            let quarter = clock.quarter_at(event.start_frame);
            let previous = clock.meters.last().unwrap();
            let bars = ((quarter - previous.quarter) / Self::bar_quarters(previous.signature)
                - 1e-10)
                .ceil()
                .max(1.0);
            let bar = previous
                .bar
                .checked_add(checked_frame(bars)?)
                .ok_or_else(|| Error("Bar number exceeds supported bounds".into()))?;
            clock.meters.push(MeterSegment {
                frame: event.start_frame,
                quarter,
                bar,
                signature: event.signature,
            });
        }
        Ok(clock)
    }
    pub fn sample_rate_hz(&self) -> u32 {
        self.rate
    }
    pub fn memory_bytes(&self) -> u64 {
        (self.tempos.capacity() * std::mem::size_of::<TempoSegment>()
            + self.meters.capacity() * std::mem::size_of::<MeterSegment>()) as u64
    }
    pub fn is_constant(&self) -> bool {
        self.tempos.len() == 1 && self.meters.len() == 1
    }
    pub fn tempo_at(&self, frame: u64) -> f32 {
        self.tempos[self
            .tempos
            .partition_point(|s| s.frame <= frame)
            .saturating_sub(1)]
        .bpm
    }
    /// One bar from this frame, using the active signature and mapped tempo.
    pub fn section_end_frame(&self, marker: &ArrangementMarker) -> Result<u64> {
        if let Some(length) = marker.length_frames {
            return marker
                .start_frame
                .checked_add(length)
                .filter(|_| length > 0)
                .ok_or_else(|| Error("Invalid section range".into()));
        }
        self.frame_at_quarter(
            self.quarter_at(marker.start_frame)
                + Self::bar_quarters(self.signature_at(marker.start_frame)),
        )
        .and_then(|end| {
            if end > marker.start_frame {
                Ok(end)
            } else {
                Err(Error("Section is empty".into()))
            }
        })
    }
    pub fn signature_at(&self, frame: u64) -> TimeSignature {
        self.meter(frame).signature
    }
    fn meter(&self, frame: u64) -> &MeterSegment {
        &self.meters[self
            .meters
            .partition_point(|s| s.frame <= frame)
            .saturating_sub(1)]
    }
    pub fn quarter_at(&self, frame: u64) -> f64 {
        let s = &self.tempos[self
            .tempos
            .partition_point(|s| s.frame <= frame)
            .saturating_sub(1)];
        s.quarter + (frame - s.frame) as f64 * f64::from(s.bpm) / (60.0 * f64::from(self.rate))
    }
    pub fn frame_at_quarter(&self, quarter: f64) -> Result<u64> {
        checked_frame(self.frame_value(quarter)?.round())
    }
    pub fn ceil_frame_at_quarter(&self, quarter: f64) -> Result<u64> {
        checked_frame(self.frame_value(quarter)?.ceil())
    }
    fn frame_value(&self, quarter: f64) -> Result<f64> {
        if !quarter.is_finite() || quarter < 0.0 {
            return Err(Error("Invalid musical position".into()));
        }
        Ok(self.frame_value_at(quarter))
    }
    // Callback callers provide finite musical positions from this compiled clock.
    // Return a number here: error construction belongs only to control-side conversions.
    fn frame_value_at(&self, quarter: f64) -> f64 {
        let s = &self.tempos[self
            .tempos
            .partition_point(|s| s.quarter <= quarter)
            .saturating_sub(1)];
        s.frame as f64 + (quarter - s.quarter) * 60.0 * f64::from(self.rate) / f64::from(s.bpm)
    }
    /// Extrapolate negative trimmed-note origins with the origin tempo.
    pub fn signed_quarter_at(&self, frame: i128) -> f64 {
        if frame < 0 {
            frame as f64 * f64::from(self.tempos[0].bpm) / (60.0 * f64::from(self.rate))
        } else {
            self.quarter_at(frame.min(i128::from(u64::MAX)) as u64)
        }
    }
    pub fn frame_for_signed_reference(&self, start: u64, offset: i128, reference_bpm: f32) -> i128 {
        let q = self.quarter_at(start)
            + offset as f64 * f64::from(reference_bpm) / (60.0 * f64::from(self.rate));
        if q < 0.0 {
            (q * 60.0 * f64::from(self.rate) / f64::from(self.tempos[0].bpm)).round() as i128
        } else {
            (self.frame_value_at(q).round() as i128).clamp(0, i128::from(u64::MAX))
        }
    }
    pub fn interval_quarters(&self, start: u64, end: u64) -> f64 {
        self.quarter_at(end) - self.quarter_at(start)
    }
    pub fn bar_quarters(signature: TimeSignature) -> f64 {
        f64::from(signature.numerator) * 4.0 / f64::from(signature.denominator)
    }
    pub fn position(&self, frame: u64) -> (u64, u64) {
        let s = self.meter(frame);
        let beat = ((self.quarter_at(frame) - s.quarter).max(0.0)
            * f64::from(s.signature.denominator)
            / 4.0
            + 1e-10)
            .floor() as u64;
        (
            s.bar
                .saturating_add(beat / u64::from(s.signature.numerator)),
            beat % u64::from(s.signature.numerator) + 1,
        )
    }
    /// Current denominator-beat boundary and accent, including partial bars.
    pub fn beat_at(&self, frame: u64) -> (u64, bool) {
        let s = self.meter(frame);
        let beat_q = 4.0 / f64::from(s.signature.denominator);
        let half_sample = f64::from(self.tempo_at(frame)) / (120.0 * f64::from(self.rate));
        let beat =
            ((self.quarter_at(frame) - s.quarter + half_sample).max(0.0) / beat_q + 1e-10).floor();
        let boundary = self.frame_value_at(s.quarter + beat * beat_q).round() as u64;
        (
            boundary,
            (beat as u64).is_multiple_of(u64::from(s.signature.numerator)),
        )
    }
    pub fn meter_origin_quarter(&self, frame: u64) -> f64 {
        self.meter(frame).quarter
    }
    pub fn next_tempo_frame(&self, frame: u64) -> Option<u64> {
        self.tempos
            .iter()
            .find(|s| s.frame > frame)
            .map(|s| s.frame)
    }
    pub fn next_signature_frame(&self, frame: u64) -> Option<u64> {
        self.meters
            .iter()
            .find(|s| s.frame > frame)
            .map(|s| s.frame)
    }
}
impl Project {
    pub fn musical_clock(&self) -> Result<MusicalClock> {
        MusicalClock::compile(self)
    }
    pub fn origin_tempo_id(&self) -> Id {
        Id::from_u128((self.project_id.as_u128() ^ 0xa923c163b07d4af28ab9154e77e10501) | (1 << 127))
    }
    pub fn origin_signature_id(&self) -> Id {
        Id::from_u128((self.project_id.as_u128() ^ 0xa923c163b07d4af28ab9154e77e10502) | (1 << 127))
    }
    pub fn tempo_events(&self) -> Vec<TempoEvent> {
        if self.timing.tempos.is_empty() {
            vec![TempoEvent {
                id: self.origin_tempo_id(),
                start_frame: 0,
                bpm: self.tempo_bpm,
            }]
        } else {
            self.timing.tempos.clone()
        }
    }
    pub fn signature_events(&self) -> Vec<SignatureEvent> {
        if self.timing.signatures.is_empty() {
            vec![SignatureEvent {
                id: self.origin_signature_id(),
                start_frame: 0,
                signature: self.time_signature,
            }]
        } else {
            self.timing.signatures.clone()
        }
    }
    pub fn validate_timing(&self) -> Result<()> {
        if self.timing.version != 1 {
            return Err(Error("Unsupported timing map version".into()));
        }
        if self.sample_rate_hz == 0 || !self.tempo_bpm.is_finite() || self.tempo_bpm <= 0.0 {
            return Err(Error("Invalid timing reference".into()));
        }
        self.time_signature.validate()?;
        if self.timing.tempos.len() + self.timing.signatures.len() + self.markers.len()
            > TIMING_EVENT_LIMIT
        {
            return Err(Error(
                "Arrangement metadata limit reached (4096 points)".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        let mut previous = None;
        for e in &self.timing.tempos {
            if e.id.is_nil()
                || !ids.insert(e.id)
                || !e.bpm.is_finite()
                || e.bpm <= 0.0
                || previous.is_some_and(|p| e.start_frame <= p)
            {
                return Err(Error("Invalid tempo event identity, order or BPM".into()));
            }
            previous = Some(e.start_frame);
        }
        if self
            .timing
            .tempos
            .first()
            .is_some_and(|e| e.start_frame != 0 || e.bpm != self.tempo_bpm)
        {
            return Err(Error(
                "Tempo map must start with the reference tempo at frame zero".into(),
            ));
        }
        previous = None;
        for e in &self.timing.signatures {
            e.signature.validate()?;
            if e.id.is_nil() || !ids.insert(e.id) || previous.is_some_and(|p| e.start_frame <= p) {
                return Err(Error("Invalid signature event identity or order".into()));
            }
            previous = Some(e.start_frame);
        }
        if self
            .timing
            .signatures
            .first()
            .is_some_and(|e| e.start_frame != 0 || e.signature != self.time_signature)
        {
            return Err(Error("Signature map must start at frame zero".into()));
        }
        for m in &self.markers {
            if m.id.is_nil()
                || !ids.insert(m.id)
                || m.length_frames.is_some_and(|length| {
                    length == 0 || m.start_frame.checked_add(length).is_none()
                })
                || m.name.trim().is_empty()
                || m.name.chars().count() > 256
                || m.name.chars().any(char::is_control)
            {
                return Err(Error(
                    "Invalid arrangement marker identity, name or range".into(),
                ));
            }
        }
        Ok(())
    }
    /// New tempo positions are requested on the committed old clock. Unchanged
    /// events retain their old musical anchors; only the candidate is published.
    pub(crate) fn apply_tempo_events(&mut self, mut tempos: Vec<TempoEvent>) -> Result<()> {
        let old = self.musical_clock()?;
        let old_reference = self.tempo_bpm;
        tempos.sort_by_key(|e| e.start_frame);
        if tempos.first().is_none_or(|e| e.start_frame != 0) {
            return Err(Error("The origin tempo cannot be removed or moved".into()));
        }
        if tempos[0].id != self.tempo_events()[0].id {
            return Err(Error("The origin tempo identity must be preserved".into()));
        }
        let anchors: Vec<_> = tempos
            .iter()
            .map(|e| old.quarter_at(e.start_frame))
            .collect();
        let mut elapsed = 0.0;
        for i in 1..tempos.len() {
            elapsed += (anchors[i] - anchors[i - 1]) * 60.0 * f64::from(self.sample_rate_hz)
                / f64::from(tempos[i - 1].bpm);
            tempos[i].start_frame = checked_frame(elapsed.round())?;
        }
        self.tempo_bpm = tempos[0].bpm;
        self.timing.tempos = tempos;
        // Signature events share the same musical anchors as markers.
        let signatures = self.signature_events();
        self.timing.signatures.clear();
        let next = self.musical_clock()?;
        self.timing.signatures = signatures
            .into_iter()
            .map(|mut e| {
                e.start_frame = next.frame_at_quarter(old.quarter_at(e.start_frame))?;
                Ok(e)
            })
            .collect::<Result<_>>()?;
        for marker in &mut self.markers {
            let end = old.section_end_frame(marker)?;
            marker.start_frame = next.frame_at_quarter(old.quarter_at(marker.start_frame))?;
            marker.length_frames = Some(
                next.frame_at_quarter(old.quarter_at(end))?
                    .checked_sub(marker.start_frame)
                    .filter(|length| *length > 0)
                    .ok_or_else(|| Error("Tempo edit makes a section empty".into()))?,
            );
        }
        let reference_ratio = f64::from(old_reference) / f64::from(self.tempo_bpm);
        for track in &mut self.tracks {
            for clip in &mut track.clips {
                let start = next.frame_at_quarter(old.quarter_at(clip.start_frame))?;
                if matches!(track.kind, TrackKind::Midi | TrackKind::Beat) {
                    let end = next.frame_at_quarter(old.quarter_at(clip.end()))?;
                    clip.length_frames = end
                        .checked_sub(start)
                        .filter(|v| *v > 0)
                        .ok_or_else(|| Error("Tempo edit makes a pattern clip empty".into()))?;
                    retime_pattern_source(clip, reference_ratio, checked_frame)?;
                }
                clip.start_frame = start;
            }
        }
        retime_pattern_assets(
            &mut self.assets,
            self.sample_rate_hz,
            self.tempo_bpm,
            checked_frame,
        )?;
        self.bound_retimed_midi_sources()?;
        self.transport.r#loop.start_frame =
            next.ceil_frame_at_quarter(old.quarter_at(self.transport.r#loop.start_frame))?;
        self.transport.r#loop.end_frame =
            next.ceil_frame_at_quarter(old.quarter_at(self.transport.r#loop.end_frame))?;
        self.validate_timing()
    }
    pub fn midi_reference_position(&self, clip: &crate::Clip, frame: u64) -> u64 {
        if self.timing.tempos.len() <= 1 {
            return frame.saturating_sub(clip.start_frame);
        }
        self.musical_clock()
            .map_or(frame.saturating_sub(clip.start_frame), |clock| {
                clock.reference_position(clip.start_frame, frame, self.tempo_bpm)
            })
    }
}
impl MusicalClock {
    pub fn reference_position(&self, start: u64, frame: u64, reference_bpm: f32) -> u64 {
        (self.interval_quarters(start, frame).max(0.0) * 60.0 * f64::from(self.rate)
            / f64::from(reference_bpm))
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
    }
    /// Scheduling cursor: account for half an output sample, rather than rounding
    /// by half a reference sample (which starts notes early at slower tempos).
    pub fn sample_reference_position(&self, start: u64, frame: u64, reference_bpm: f32) -> u64 {
        let value = self.interval_quarters(start, frame).max(0.0) * 60.0 * f64::from(self.rate)
            / f64::from(reference_bpm);
        let half_sample = 0.5 * f64::from(self.tempo_at(frame)) / f64::from(reference_bpm);
        (value + half_sample + 1e-9)
            .floor()
            .clamp(0.0, u64::MAX as f64) as u64
    }
    pub fn frame_for_reference(&self, start: u64, offset: u64, reference_bpm: f32) -> Result<u64> {
        self.frame_at_quarter(
            self.quarter_at(start)
                + offset as f64 * f64::from(reference_bpm) / (60.0 * f64::from(self.rate)),
        )
    }
}

/// Shared source mapping for control-thread clip operations.
pub(crate) struct ClipClock {
    clock: MusicalClock,
    bpm: f32,
    patterns: std::collections::HashSet<Id>,
}
impl ClipClock {
    pub fn new(project: &Project) -> Result<Self> {
        Ok(Self {
            clock: project.musical_clock()?,
            bpm: project.tempo_bpm,
            patterns: project
                .assets
                .iter()
                .filter(|a| !a.is_audio())
                .map(|a| a.id)
                .collect(),
        })
    }
    pub fn length(&self, clip: &crate::Clip) -> u64 {
        if self.patterns.contains(&clip.asset_id) {
            self.clock
                .reference_position(clip.start_frame, clip.end(), self.bpm)
        } else {
            clip.length_frames
        }
    }
    pub fn retained(&self, clip: &crate::Clip, start: u64, end: u64) -> crate::Clip {
        if !self.patterns.contains(&clip.asset_id) || self.clock.is_constant() {
            return clip.retained_range(start, end);
        }
        let advance = self
            .clock
            .reference_position(clip.start_frame, start, self.bpm);
        let length = self.clock.reference_position(start, end, self.bpm);
        let mut source = clip.clone();
        source.start_frame = 0;
        source.length_frames = self.length(clip);
        let mut result = source.retained_range(advance, advance.saturating_add(length));
        result.start_frame = start;
        result.length_frames = end - start;
        result
    }
    pub fn uncovered(&self, clip: &crate::Clip, start: u64, end: u64) -> Vec<crate::Clip> {
        if self.clock.is_constant() || !self.patterns.contains(&clip.asset_id) {
            return clip.uncovered_ranges(start, end);
        }
        clip.uncovered_with(start, end, |start, end| self.retained(clip, start, end))
    }
}

impl Project {
    /// Header looping keeps its frame gesture and maps only the source cycle.
    pub fn looped_clip(
        &self,
        clip: &crate::Clip,
        edge: crate::ClipEdge,
        delta: i128,
    ) -> Option<crate::Clip> {
        if self.timing.tempos.len() <= 1
            || self
                .assets
                .iter()
                .any(|a| a.id == clip.asset_id && a.is_audio())
        {
            return Some(clip.looped_by(edge, delta));
        }
        let clock = self.musical_clock().ok()?;
        let full = clock.reference_position(clip.start_frame, clip.end(), self.tempo_bpm);
        if full == 0 {
            return None;
        }
        let repeat = clip.repeat.unwrap_or(crate::ClipLoop {
            length_frames: full,
            phase_frame: 0,
        });
        let minimum = full.min(repeat.length_frames);
        let mut result = clip.clone();
        match edge {
            crate::ClipEdge::Right => {
                let lower = clock
                    .frame_for_reference(clip.start_frame, minimum, self.tempo_bpm)
                    .ok()?;
                let end = (i128::from(clip.end()).saturating_add(delta))
                    .clamp(i128::from(lower), i128::from(u64::MAX))
                    as u64;
                result.length_frames = end.checked_sub(clip.start_frame)?;
                result.repeat = Some(repeat);
            }
            crate::ClipEdge::Left => {
                let upper = clock
                    .frame_for_signed_reference(clip.end(), -i128::from(minimum), self.tempo_bpm)
                    .max(0);
                result.start_frame =
                    (i128::from(clip.start_frame).saturating_add(delta)).clamp(0, upper) as u64;
                result.length_frames = clip.end().checked_sub(result.start_frame)?;
                let shift = (clock.interval_quarters(clip.start_frame, result.start_frame)
                    * 60.0
                    * f64::from(self.sample_rate_hz)
                    / f64::from(self.tempo_bpm))
                .round() as i128;
                result.repeat = Some(repeat.shifted(shift));
            }
        }
        let actual = result.length_frames;
        result.length_frames =
            clock.reference_position(result.start_frame, result.end(), self.tempo_bpm);
        result.restore_source_range();
        result.length_frames = actual;
        Some(result)
    }
    /// Stretch source coordinates by the musical distance of the requested frame range.
    pub fn stretched_clip(
        &self,
        clip: &crate::Clip,
        start: u64,
        length: u64,
    ) -> Option<crate::Clip> {
        if self.timing.tempos.len() <= 1
            || self
                .assets
                .iter()
                .any(|a| a.id == clip.asset_id && a.is_audio())
        {
            return clip.stretched_to(start, length);
        }
        let clock = self.musical_clock().ok()?;
        let mut source = clip.clone();
        source.start_frame = 0;
        source.length_frames =
            clock.reference_position(clip.start_frame, clip.end(), self.tempo_bpm);
        let target = clock.reference_position(start, start.checked_add(length)?, self.tempo_bpm);
        let mut result = source.stretched_to(start, target)?;
        result.length_frames = length;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edit, MidiNote, MidiNoteSplit, MidiPattern};
    fn mapped() -> Project {
        let mut p = Project::default();
        let mut events = p.tempo_events();
        events.push(TempoEvent {
            id: Id::new_v4(),
            start_frame: 48_000,
            bpm: 60.0,
        });
        p.edit(Edit::SetTempoEvents { events }).unwrap();
        p
    }
    #[test]
    fn piecewise_clock_round_trips_and_partial_bars_start_new_numbers() {
        let mut p = mapped();
        let mut events = p.signature_events();
        events.push(SignatureEvent {
            id: Id::new_v4(),
            start_frame: 72_000,
            signature: TimeSignature {
                numerator: 3,
                denominator: 8,
            },
        });
        p.edit(Edit::SetSignatureEvents { events }).unwrap();
        let c = p.musical_clock().unwrap();
        assert_eq!(c.quarter_at(48_000), 2.0);
        assert_eq!(c.quarter_at(96_000), 3.0);
        assert_eq!(c.position(72_000), (2, 1));
        assert_eq!(c.position(96_000), (2, 2));
        assert_eq!(c.beat_at(96_001), (96_000, false));
        for f in [0, 1, 23_999, 48_000, 72_000, 96_000, 10_000_000] {
            assert_eq!(c.frame_at_quarter(c.quarter_at(f)).unwrap(), f);
        }
    }
    #[test]
    fn map_edits_retain_musical_anchors_and_reference_source_ranges() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let clip = p.add_midi_pattern(track, 0).unwrap();
        p.edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    start_tick: 960,
                    length_ticks: 1920,
                    pitch: 60,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
        p.tracks[0].clips[0].length_frames = 96_000;
        p.markers.push(ArrangementMarker {
            id: Id::new_v4(),
            start_frame: 72_000,
            name: "Verse".into(),
            length_frames: Some(96_000),
        });
        let mut events = p.tempo_events();
        events.push(TempoEvent {
            id: Id::new_v4(),
            start_frame: 48_000,
            bpm: 60.0,
        });
        p.edit(Edit::SetTempoEvents { events }).unwrap();
        assert_eq!(p.tracks[0].clips[0].length_frames, 144_000);
        assert_eq!(
            p.assets
                .iter()
                .find(|a| a.id == p.tracks[0].clips[0].asset_id)
                .unwrap()
                .decoded_frame_count,
            96_000
        );
        assert_eq!(p.markers[0].start_frame, 96_000);
        assert_eq!(p.markers[0].length_frames, Some(192_000));
        let c = &p.tracks[0].clips[0];
        let slices = p.clip_range_slices(&[track], 72_000, 120_000).unwrap();
        assert_eq!(slices[0].clip.source_offset_frame, 60_000);
        assert_eq!(slices[0].clip.length_frames, 48_000);
        assert_eq!(p.midi_reference_position(c, 96_000), 72_000);
        let (split, _) = p
            .split_clips(&[clip], 72_000, MidiNoteSplit::Split)
            .unwrap();
        assert_eq!(split.tracks[0].clips[1].source_offset_frame, 60_000);
        split.validate().unwrap();
    }
    #[test]
    fn maps_round_trip_reject_invalid_points_and_preserve_origin_identity() {
        let p = mapped();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
        let mut candidate = p.clone();
        for bpm in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut events = p.tempo_events();
            events[1].bpm = bpm;
            assert!(candidate.edit(Edit::SetTempoEvents { events }).is_err());
            assert_eq!(candidate, p);
        }
        let mut events = p.tempo_events();
        events[0].id = Id::new_v4();
        assert!(candidate.edit(Edit::SetTempoEvents { events }).is_err());
        let mut events = p.tempo_events();
        events[1].start_frame = 0;
        assert!(candidate.edit(Edit::SetTempoEvents { events }).is_err());
        assert!(
            candidate
                .edit(Edit::SetMarkers {
                    markers: vec![ArrangementMarker {
                        id: Id::new_v4(),
                        start_frame: 0,
                        name: "\n".into(),
                        length_frames: Some(96_000),
                    }]
                })
                .is_err()
        );
        assert_eq!(candidate, p);
    }
}

#[cfg(test)]
mod section_tests {
    use super::*;
    use crate::Edit;
    #[test]
    fn section_ranges_validate_atomically_and_legacy_defaults_follow_the_clock() {
        let mut p = Project {
            time_signature: TimeSignature {
                numerator: 3,
                denominator: 8,
            },
            ..Default::default()
        };
        let mut marker = ArrangementMarker {
            id: Id::new_v4(),
            start_frame: 24_000,
            name: "Verse".into(),
            length_frames: None,
        };
        let clock = p.musical_clock().unwrap();
        assert_eq!(clock.section_end_frame(&marker).unwrap(), 60_000);
        let json = serde_json::to_string(&marker).unwrap();
        assert!(!json.contains("length_frames"));
        assert_eq!(
            serde_json::from_str::<ArrangementMarker>(&json).unwrap(),
            marker
        );
        marker.length_frames = Some(36_000);
        p.edit(Edit::SetMarkers {
            markers: vec![marker.clone()],
        })
        .unwrap();
        let before = p.clone();
        for length in [0, u64::MAX] {
            marker.length_frames = Some(length);
            assert!(
                p.edit(Edit::SetMarkers {
                    markers: vec![marker.clone()]
                })
                .is_err()
            );
            assert_eq!(p, before);
        }
        p.edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
        assert_eq!(p.markers[0].start_frame, 48_000);
        assert_eq!(p.markers[0].length_frames, Some(72_000));
    }
}
