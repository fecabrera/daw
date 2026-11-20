//! Worker-only isolated capture. Reference controls retain the original graph's
//! stage clocks and monitoring gates; only the target's selected excitation changes.
use super::*;
use daw_core::BounceTarget;
use std::collections::{BTreeMap, BTreeSet, HashSet};

struct ProviderCapture {
    id: Id,
    sources: Vec<ProviderSource>,
}
enum ProviderSource {
    Track {
        index: usize,
        raw: latency::Delay,
        post: latency::Delay,
    },
    Rendered {
        raw: Box<super::captured_taps::Tap>,
        post: Box<super::captured_taps::Tap>,
        post_gain: [f32; 2],
    },
}

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
    block_offset: usize,
    block_count: usize,
    pre_capture: Option<super::captured_taps::Tap>,
    lineage: Vec<ProviderCapture>,
    lineage_frames: Vec<([f32; 2], [f32; 2])>,
    return_sends: Option<super::send_routing::Sends>,
    returns: Vec<(Id, usize)>,
    return_frames: Vec<[f32; 2]>,
}
impl BounceCapture {
    pub fn new(plan: RenderPlan, target: &BounceTarget) -> Result<Self, String> {
        let mut isolated = plan.clone();
        let subtree = isolated
            .project
            .track_subtree(target.track_id)
            .map_err(|e| e.to_string())?;
        let clips: HashSet<_> = target.clip_ids.iter().copied().collect();
        for track in &mut isolated.project.tracks[subtree.clone()] {
            track.clips.retain(|c| clips.contains(&c.id));
        }
        // Keep the mix inside the group. Its own and downstream monitoring
        // gates remain live on the replacement path.
        for (index, track) in isolated.project.tracks.iter_mut().enumerate() {
            if index == subtree.start || !subtree.contains(&index) {
                track.muted = false;
                track.soloed = false;
            }
        }
        let mut reference = Renderer::try_new(plan).map_err(|e| e.to_string())?;
        let mut isolated = Renderer::try_new(isolated).map_err(|e| e.to_string())?;
        if reference.hosted.is_some() || isolated.hosted.is_some() {
            // Both graphs must use the same block evaluator, including references
            // whose retained content contains only built-in units.
            for renderer in [&mut reference, &mut isolated] {
                if renderer.hosted.is_none() {
                    renderer.hosted = Some(
                        super::hosted_graph::HostedGraph::prepare(
                            renderer,
                            &mut Default::default(),
                        )
                        .map_err(|e| e.to_string())?,
                    );
                }
                let mut graph = renderer.hosted.take().unwrap();
                graph.enable_capture(renderer, false);
                renderer.hosted = Some(graph);
            }
            let mut graph = isolated.hosted.take().unwrap();
            graph.enable_capture(&isolated, true);
            isolated.hosted = Some(graph);
        }
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
        let (return_sends, returns) = captured_returns(&isolated, index);
        let return_frames = vec![[0.0; 2]; returns.len()];
        let source = &isolated.plan.project.tracks[index];
        let pre_capture = super::captured_taps::Tap::new(
            &isolated.plan,
            source,
            |r| Some(r.pre_send_asset),
            0,
            latency,
        );
        let lineage = captured_providers(&isolated, index);
        let lineage_frames = vec![([0.0; 2], [0.0; 2]); lineage.len()];
        Ok(Self {
            reference,
            isolated,
            index,
            frame: 0,
            latency,
            block_offset: 0,
            block_count: 0,
            pre_capture,
            lineage,
            lineage_frames,
            return_sends,
            returns,
            return_frames,
            delay: latency::Delay::new(Id::nil(), delay),
            pre_delay: latency::Delay::new(Id::nil(), delay),
            raw_delay: latency::Delay::new(Id::nil(), delay),
        })
    }
    pub fn latency_frames(&self) -> u64 {
        self.latency
    }
    pub fn lineage_providers(&self) -> impl Iterator<Item = Id> + '_ {
        self.lineage.iter().map(|provider| provider.id)
    }
    pub fn lineage_frames(&self) -> &[([f32; 2], [f32; 2])] {
        &self.lineage_frames
    }
    pub fn return_destinations(&self) -> impl Iterator<Item = Id> + '_ {
        self.returns.iter().map(|&(id, _)| id)
    }
    pub fn return_frames(&self) -> &[[f32; 2]] {
        &self.return_frames
    }
    /// Feed every prefix frame. `None` is pipeline priming, not missing content.
    pub fn next_frame(&mut self) -> Result<Option<BounceFrame>, String> {
        let frame = self.frame;
        if self.reference.hosted.is_some() {
            let mut reference = self.reference.hosted.take().unwrap();
            let mut isolated = self.isolated.hosted.take().unwrap();
            if self.block_offset == self.block_count {
                let maximum = reference.quantum.min(isolated.quantum);
                let count = self
                    .reference
                    .hosted_quantum(&reference, frame, maximum, false)
                    .min(
                        self.isolated
                            .hosted_quantum(&isolated, frame, maximum, false),
                    );
                reference.render(&mut self.reference, frame, count, false, true, false);
                isolated.copy_capture(&reference);
                isolated.render(&mut self.isolated, frame, count, false, true, false);
                self.block_count = count;
                self.block_offset = 0;
            }
            reference.expose_capture(&mut self.reference, self.block_offset);
            isolated.expose_capture(&mut self.isolated, self.block_offset);
            self.block_offset += 1;
            self.reference.hosted = Some(reference);
            self.isolated.hosted = Some(isolated);
            if self
                .reference
                .hosted_fault()
                .or_else(|| self.isolated.hosted_fault())
                .is_some()
            {
                return Err("Plugin processing failed during bounce".into());
            }
        } else {
            self.reference.offline_frame(frame, false, false);
            if let Some(values) = &mut self.isolated.capture_controls {
                for (to, from) in values.iter_mut().zip(&self.reference.sidechains.values) {
                    to.copy_from_slice(from);
                }
            }
            self.isolated.offline_frame(frame, false, false);
        }
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
            if let Some(error) = tap.reader.source_error() {
                return Err(error);
            }
            if tap.active_at(frame, None) {
                let current_pan = track_processing::track_pan_coefficients(track)[1];
                pre_send = std::array::from_fn(|ch| value[ch] * current_pan[ch]);
            }
        }
        for (provider, output) in self.lineage.iter_mut().zip(&mut self.lineage_frames) {
            *output = ([0.0; 2], [0.0; 2]);
            for source in &mut provider.sources {
                let (raw, post, gains) = match source {
                    ProviderSource::Track { index, raw, post } => {
                        let sample = self.isolated.buses.raw[*index];
                        let gain = self.isolated.gains[*index].target;
                        let base = if sample.1 { 0 } else { 2 };
                        let post_sample = [sample.0[0] * gain[base], sample.0[1] * gain[base + 1]];
                        (
                            raw.tick(sample).0,
                            post.tick((post_sample, false)).0,
                            [1.0; 2],
                        )
                    }
                    ProviderSource::Rendered {
                        raw,
                        post,
                        post_gain,
                    } => {
                        let raw_sample = raw.sample(frame, None, false).0;
                        let post_sample = post.sample(frame, None, false).0;
                        if let Some(error) = raw
                            .reader
                            .source_error()
                            .or_else(|| post.reader.source_error())
                        {
                            return Err(error);
                        }
                        (raw_sample, post_sample, *post_gain)
                    }
                };
                for ch in 0..2 {
                    output.0[ch] += raw[ch];
                    output.1[ch] += post[ch] * gains[ch];
                }
            }
        }
        if let Some(sends) = &mut self.return_sends {
            for (&(_, destination), output) in self.returns.iter().zip(&mut self.return_frames) {
                *output = sends.sum(
                    destination,
                    &self.isolated.buses.raw,
                    false,
                    frame,
                    None,
                    false,
                );
            }
            if let Some(error) = sends.taps().find_map(|tap| tap.reader.source_error()) {
                return Err(error);
            }
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
            + daw_unit_core::memory::vec_bytes(&self.returns)
            + daw_unit_core::memory::vec_bytes(&self.return_frames)
            + self
                .return_sends
                .as_ref()
                .map_or(0, |sends| sends.heap_bytes())
            + self.reference.owned_memory_bytes()
            + self.isolated.owned_memory_bytes()
            + self.delay.heap_bytes()
            + self.pre_delay.heap_bytes()
            + self.raw_delay.heap_bytes()
            + self.pre_capture.as_ref().map_or(0, |t| t.heap_bytes())
            + self
                .lineage
                .iter()
                .map(|provider| {
                    daw_unit_core::memory::vec_bytes(&provider.sources)
                        + provider
                            .sources
                            .iter()
                            .map(|source| match source {
                                ProviderSource::Track { raw, post, .. } => {
                                    raw.heap_bytes() + post.heap_bytes()
                                }
                                ProviderSource::Rendered { raw, post, .. } => {
                                    2 * std::mem::size_of::<super::captured_taps::Tap>() as u64
                                        + raw.heap_bytes()
                                        + post.heap_bytes()
                                }
                            })
                            .sum::<u64>()
                })
                .sum::<u64>()
    }
}

fn captured_providers(renderer: &Renderer, root: usize) -> Vec<ProviderCapture> {
    let plan = &renderer.plan;
    let project = &plan.project;
    let root_id = project.tracks[root].id;
    let subtree = project.track_subtree(root_id).expect("valid bounce root");
    let used: HashSet<_> = project
        .audio_sidechain_routes()
        .into_iter()
        .filter_map(|(_, route)| route.sidechain.map(|sidechain| sidechain.track))
        .chain(
            project
                .hosted_units()
                .into_iter()
                .flat_map(|(_, unit)| unit.audio_inputs.iter().map(|route| route.source_track)),
        )
        .collect();
    let latency = renderer.alignment.total;
    let mut sources: BTreeMap<Id, Vec<ProviderSource>> = BTreeMap::new();
    for index in subtree {
        let track = &project.tracks[index];
        if index != root && used.contains(&track.id) {
            let delay = latency - renderer.alignment.node_frames[index];
            sources
                .entry(track.id)
                .or_default()
                .push(ProviderSource::Track {
                    index,
                    raw: latency::Delay::new(track.id, delay),
                    post: latency::Delay::new(track.id, delay),
                });
        }
        for id in project
            .rendered_providers(track)
            .into_iter()
            .filter(|id| *id != root_id)
        {
            let make_tap = |post| {
                super::captured_taps::Tap::new(
                    plan,
                    track,
                    |r| {
                        r.feeds
                            .iter()
                            .find(|f| f.provider == id)
                            .map(|f| if post { f.post_asset } else { f.raw_asset })
                    },
                    0,
                    latency,
                )
            };
            if let Some((raw, post)) = make_tap(false).zip(make_tap(true)) {
                let gain = renderer.gains[index].target;
                sources
                    .entry(id)
                    .or_default()
                    .push(ProviderSource::Rendered {
                        raw: Box::new(raw),
                        post: Box::new(post),
                        post_gain: [gain[2], gain[3]],
                    });
            }
        }
    }
    sources
        .into_iter()
        .map(|(id, sources)| ProviderCapture { id, sources })
        .collect()
}

// Descendant sends bypass the selected group. Capture their Return ingress
// on the same content clock as the main output, before any Return processing.
fn captured_returns(
    renderer: &Renderer,
    root: usize,
) -> (Option<super::send_routing::Sends>, Vec<(Id, usize)>) {
    let project = &renderer.plan.project;
    let subtree = project
        .track_subtree(project.tracks[root].id)
        .expect("valid bounce root");
    let alignment = &renderer.alignment;
    if alignment.send_delays.is_empty() && alignment.captured_sends.is_empty() {
        return (None, Vec::new());
    }
    let direct: Vec<_> = alignment
        .send_delays
        .iter()
        .copied()
        .filter(|&(id, source, _, _)| {
            source != root
                && subtree.contains(&source)
                && project.tracks[source]
                    .sends
                    .iter()
                    .any(|s| s.id == id && s.enabled && s.gain_db > daw_core::SILENT_GAIN_DB)
        })
        .collect();
    let captured: Vec<_> = alignment
        .captured_sends
        .iter()
        .copied()
        .filter(|&(source, _)| subtree.contains(&source))
        .collect();
    let returns: BTreeSet<_> = direct
        .iter()
        .map(|&(_, _, index, _)| index)
        .chain(captured.iter().map(|&(_, index)| index))
        .collect();
    if returns.is_empty() {
        return (None, Vec::new());
    }
    let mut sends = super::send_routing::Sends::prepare(
        &renderer.plan,
        alignment,
        &direct,
        &captured,
        Some(alignment.total),
    );
    sends.advance(false);
    (
        Some(sends),
        returns
            .into_iter()
            .map(|index| (project.tracks[index].id, index))
            .collect(),
    )
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
    let Ok(subtree) = project.track_subtree(track) else {
        return (0, false);
    };
    let mut child_tails = std::collections::HashMap::new();
    for t in project.tracks[subtree].iter().rev() {
        let (own, unknown) = track_tail_frames(plan, t);
        let (children, child_unknown) = child_tails.remove(&t.id).unwrap_or((0u64, false));
        let frames = own.saturating_add(children);
        let unknown = unknown || child_unknown;
        if t.id == track {
            return (
                if unknown { cap } else { frames.min(cap) },
                unknown || frames > cap,
            );
        }
        if let Some(parent) = t.group_id {
            let tail = child_tails.entry(parent).or_insert((0, false));
            tail.0 = tail.0.max(frames);
            tail.1 |= unknown;
        }
    }
    (0, false)
}

fn track_tail_frames(plan: &RenderPlan, t: &daw_core::Track) -> (u64, bool) {
    let project = &plan.project;
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
    (tail.saturating_add(continuation), unknown.get())
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
