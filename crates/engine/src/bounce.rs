//! Worker-only isolated capture. Reference controls retain the original graph's
//! stage clocks and monitoring gates; only the target's selected excitation changes.
use super::*;
use daw_core::BounceTarget;

#[derive(Clone, Copy, Debug, Default)]
pub struct BounceFrame {
    pub main: [f32; 2],
    pub raw: [f32; 2],
    pub pre_send: [f32; 2],
}
pub struct BounceCapture {
    reference: Renderer,
    isolated: Renderer,
    index: usize,
    frame: u64,
    delay: latency::Delay,
    pre_delay: latency::Delay,
    raw_delay: latency::Delay,
    latency: u64,
    pre_capture: Option<super::captured_taps::Tap>,
    lineage: Vec<(Id, super::captured_taps::Tap, super::captured_taps::Tap)>,
    lineage_frames: Vec<([f32; 2], [f32; 2])>,
}
impl BounceCapture {
    pub fn new(plan: RenderPlan, target: &BounceTarget) -> Result<Self, String> {
        let mut isolated = plan.clone();
        let track = isolated
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == target.track_id)
            .ok_or("Bounce target is missing")?;
        track.clips.retain(|c| target.clip_ids.contains(&c.id));
        // Monitoring is not authored content. The reference retains its exact
        // gates for external sidechains, while isolated source/layer gates stay authored.
        for t in &mut isolated.project.tracks {
            t.muted = false;
            t.soloed = false;
        }
        let reference = Renderer::new(plan);
        let mut isolated = Renderer::new(isolated);
        let index = isolated
            .plan
            .project
            .tracks
            .iter()
            .position(|t| t.id == target.track_id)
            .ok_or("Lowered bounce root is missing")?;
        isolated.capture_controls = Some(reference.sidechains.values.clone());
        let latency = isolated.alignment.total;
        let delay = latency - isolated.alignment.node_frames[index];
        let source = &isolated.plan.project.tracks[index];
        let pre_capture = super::captured_taps::Tap::new(
            &isolated.plan,
            source,
            |r| Some(r.pre_send_asset),
            0,
            latency,
        );
        let lineage = isolated
            .plan
            .project
            .rendered_providers(source)
            .into_iter()
            .filter(|id| *id != target.track_id)
            .filter_map(|id| {
                let raw = super::captured_taps::Tap::new(
                    &isolated.plan,
                    source,
                    |r| {
                        r.feeds
                            .iter()
                            .find(|f| f.provider == id)
                            .map(|f| f.raw_asset)
                    },
                    0,
                    latency,
                )?;
                let post = super::captured_taps::Tap::new(
                    &isolated.plan,
                    source,
                    |r| {
                        r.feeds
                            .iter()
                            .find(|f| f.provider == id)
                            .map(|f| f.post_asset)
                    },
                    0,
                    latency,
                )?;
                Some((id, raw, post))
            })
            .collect::<Vec<_>>();
        let lineage_frames = vec![([0.0; 2], [0.0; 2]); lineage.len()];
        Ok(Self {
            reference,
            isolated,
            index,
            frame: 0,
            latency,
            pre_capture,
            lineage,
            lineage_frames,
            delay: latency::Delay::new(Id::nil(), delay),
            pre_delay: latency::Delay::new(Id::nil(), delay),
            raw_delay: latency::Delay::new(Id::nil(), delay),
        })
    }
    pub fn latency_frames(&self) -> u64 {
        self.latency
    }
    pub fn lineage_providers(&self) -> impl Iterator<Item = Id> + '_ {
        self.lineage.iter().map(|v| v.0)
    }
    pub fn lineage_frames(&self) -> &[([f32; 2], [f32; 2])] {
        &self.lineage_frames
    }
    /// Feed every prefix frame. `None` is pipeline priming, not missing content.
    pub fn next_frame(&mut self) -> Result<Option<BounceFrame>, String> {
        let frame = self.frame;
        self.reference.offline_frame(frame, false, false);
        if let Some(values) = &mut self.isolated.capture_controls {
            for (to, from) in values.iter_mut().zip(&self.reference.sidechains.values) {
                to.copy_from_slice(from);
            }
        }
        self.isolated.offline_frame(frame, false, false);
        let (raw, mono) = self.isolated.buses.raw[self.index];
        let track = &self.isolated.plan.project.tracks[self.index];
        let pan = track_processing::track_pan_coefficients(track)[usize::from(!mono)];
        let content = frame.saturating_sub(self.isolated.alignment.node_frames[self.index]);
        let envelope = self.isolated.buses.clip_gains[self.index].at(content, None);
        let raw = raw.map(|v| v * envelope);
        let pre = std::array::from_fn(|ch| raw[ch] * pan[ch]);
        let gain = if track.gain_db <= daw_core::SILENT_GAIN_DB {
            0.0
        } else {
            linear_gain(track.gain_db)
        };
        let main = self.delay.tick((pre.map(|v| v * gain), false)).0;
        let raw = self.raw_delay.tick((raw, false)).0;
        let mut pre_send = self.pre_delay.tick((pre, false)).0;
        if let Some(tap) = &mut self.pre_capture {
            let value = tap.sample(frame, None, false).0;
            if tap.active_at(frame, None) {
                let current_pan = track_processing::track_pan_coefficients(track)[1];
                pre_send = std::array::from_fn(|ch| value[ch] * current_pan[ch]);
            }
        }
        for ((_, raw, post), out) in self.lineage.iter_mut().zip(&mut self.lineage_frames) {
            let raw = raw.sample(frame, None, false).0;
            let post = post.sample(frame, None, false).0;
            let current_pan = track_processing::track_pan_coefficients(track)[1];
            *out = (
                raw,
                std::array::from_fn(|ch| post[ch] * current_pan[ch] * gain),
            );
        }
        if let Some(e) = self
            .reference
            .source_error()
            .or_else(|| self.isolated.source_error())
        {
            return Err(e);
        }
        self.frame = self.frame.checked_add(1).ok_or("Bounce clock overflow")?;
        Ok((frame >= self.latency).then_some(BounceFrame {
            main,
            raw,
            pre_send,
        }))
    }
    pub fn memory_bytes(&self) -> u64 {
        std::mem::size_of::<Self>() as u64
            + daw_unit_core::memory::vec_bytes(&self.lineage)
            + daw_unit_core::memory::vec_bytes(&self.lineage_frames)
            + self.reference.owned_memory_bytes()
            + self.isolated.owned_memory_bytes()
            + self.delay.heap_bytes()
            + self.pre_delay.heap_bytes()
            + self.raw_delay.heap_bytes()
            + self.pre_capture.as_ref().map_or(0, |t| t.heap_bytes())
            + self
                .lineage
                .iter()
                .map(|(_, a, b)| a.heap_bytes() + b.heap_bytes())
                .sum::<u64>()
    }
}

/// Shared export/bounce tail ceiling, in project-rate frames.
pub fn offline_tail_cap(sample_rate_hz: u32) -> u64 {
    u64::from(sample_rate_hz) * 10
}

/// Bound actual source continuations, not the maximum voice-slot reservation.
/// Filters without a finite audible-tail contract use the cap. Never stop on
/// measured energy: silent attacks and sparse releases must retain their clocks.
pub fn bounce_tail_frames(plan: &RenderPlan, track: Id) -> (u64, bool) {
    let project = &plan.project;
    let cap = offline_tail_cap(project.sample_rate_hz);
    let Some(t) = project.tracks.iter().find(|t| t.id == track) else {
        return (0, false);
    };
    let mut tail = 0u64;
    let unknown = std::cell::Cell::new(false);
    let mut effect = |e: &daw_core::EffectInstance| {
        let frames = e.unit.tail_frames(project.sample_rate_hz);
        if frames == 0
            && !matches!(
                e.unit,
                daw_core::EffectSettings::DirectionMixer(_)
                    | daw_core::EffectSettings::Loudness(_)
                    | daw_core::EffectSettings::Missing { .. }
            )
        {
            unknown.set(true);
        }
        if frames > 0
            && e.unit
                .modulations()
                .iter()
                .any(|route| route.enabled && route.amount != 0.0)
        {
            unknown.set(true);
        }
        tail = tail.saturating_add(frames);
    };
    for e in &t.effects {
        effect(e);
    }
    let instruments = t.instrument.iter().chain(
        t.beat
            .iter()
            .flat_map(|b| b.subtracks.iter().map(|s| &s.instrument)),
    );
    let mut continuation = 0;
    for i in instruments {
        // Every enabled note envelope can retain a voice. Modulated release
        // times are bounded by the portable envelope's final time limit.
        for envelope in i
            .layers
            .iter()
            .flat_map(|l| &l.envelopes)
            .chain(i.buses().flat_map(|b| &b.envelopes))
            .filter(|e| e.present && e.enabled)
        {
            let release = release_bound(envelope, project.sample_rate_hz);
            continuation = continuation.max(release);
        }
        for layer in &i.layers {
            continuation = continuation.max(u64::from(project.sample_rate_hz / 200));
            if let Some(sampler) = &layer.sampler {
                if sampler.granulizer.is_some() {
                    unknown.set(true);
                }
                if let Some(prepared) = plan.sampler_audio.get(&sampler.id) {
                    continuation = continuation.max(
                        prepared
                            .variants()
                            .map(|(_, a)| a.frame_count())
                            .max()
                            .unwrap_or(0),
                    );
                } else {
                    unknown.set(true);
                }
            }
        }
        for e in i
            .layers
            .iter()
            .flat_map(|l| &l.effects)
            .chain(i.buses().flat_map(|b| &b.effects))
        {
            effect(e);
        }
    }
    if let Some(beat) = &t.beat {
        for e in &beat.output.effects {
            effect(e);
        }
        for envelope in beat
            .output
            .envelopes
            .iter()
            .filter(|e| e.present && e.enabled)
        {
            continuation =
                continuation.max(envelope.settings.release.frames(project.sample_rate_hz));
        }
    }
    let requested = tail.saturating_add(continuation);
    (
        if unknown.get() {
            cap
        } else {
            requested.min(cap)
        },
        unknown.get() || requested > cap,
    )
}

fn release_bound(envelope: &daw_unit_core::EnvelopeInstance, rate: u32) -> u64 {
    if envelope.modulations.iter().any(|route| {
        route.enabled && route.parameter == daw_core::ModulationParameter::EnvelopeRelease
    }) {
        (f64::from(daw_core::MAX_ENVELOPE_TIME_MS) * f64::from(rate) / 1000.0).ceil() as u64
    } else {
        envelope.settings.release.frames(rate)
    }
}
