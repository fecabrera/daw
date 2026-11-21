//! Mixed-format block chains retain the existing built-in sample kernels.
use super::{RenderPlan, effects::EffectChain, hosted_node::HostedNode, latency};
use daw_core::{EffectInstance, EffectSettings, HostedInputId, HostedSettings};
use daw_plugin_host::{NativeTransport, Result};
use daw_unit_core::{
    ModulationContext, ModulationSourceIndex, ModulationValue, ProcessingContext, SidechainTap,
};

pub(super) struct Controls {
    width: usize,
    slots: Vec<usize>,
    values: Vec<ModulationValue>,
    pub index: ModulationSourceIndex,
}
impl Controls {
    pub fn new(
        values: &[ModulationValue],
        needed: &std::collections::HashSet<daw_unit_core::ModulationSourceId>,
        maximum: usize,
    ) -> Self {
        let slots: Vec<_> = values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| needed.contains(&v.source).then_some(i))
            .collect();
        let selected: Vec<_> = slots.iter().map(|&i| values[i]).collect();
        Self {
            width: selected.len(),
            slots,
            values: (0..maximum)
                .flat_map(|_| selected.iter().copied())
                .collect(),
            index: ModulationSourceIndex::from_values(&selected),
        }
    }
    pub fn publish(&mut self, frame: usize, values: &[ModulationValue]) {
        for (out, &slot) in self.values[frame * self.width..(frame + 1) * self.width]
            .iter_mut()
            .zip(&self.slots)
        {
            *out = values[slot];
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.values)
            + daw_unit_core::memory::vec_bytes(&self.slots)
            + self.index.heap_bytes()
    }
    pub fn at(&self, frame: usize) -> &[ModulationValue] {
        &self.values[frame * self.width..(frame + 1) * self.width]
    }
}
#[derive(Clone)]
pub(super) struct Taps {
    pub raw: Vec<Vec<latency::Sample>>,
    pub post: Vec<Vec<[f32; 4]>>,
    pub output: Vec<Vec<[f32; 2]>>,
}
impl Taps {
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        vec_bytes(&self.raw)
            + self.raw.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.post)
            + self.post.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.output)
            + self.output.iter().map(vec_bytes).sum::<u64>()
    }
    pub fn new(tracks: usize, maximum: usize) -> Self {
        Self {
            raw: vec![vec![([0.0; 2], false); maximum]; tracks],
            post: vec![vec![[0.0; 4]; maximum]; tracks],
            output: vec![vec![[0.0; 2]; maximum]; tracks],
        }
    }
}
struct Route {
    source: usize,
    bus: usize,
    settings: daw_core::HostedAudioInput,
    delay: latency::Delay,
    captured: Vec<super::audio_sidechain::Contribution>,
}
pub(super) struct Node {
    pub native: HostedNode,
    pub clock_delay: u64,
    routes: Vec<Route>,
    main: Option<usize>,
    control_delay: Option<super::hosted_control_delay::ControlDelay>,
}
impl Node {
    pub fn prepare(
        settings: &HostedSettings,
        plan: &RenderPlan,
        alignment: &latency::Alignment,
        owner: Option<usize>,
        stage: u64,
        bank: &mut super::hosted_node::NativeBank,
    ) -> Result<Self> {
        let context = ProcessingContext {
            sample_rate_hz: plan.project.sample_rate_hz,
            max_block_frames: plan.project.buffer_size_frames,
        };
        let native = HostedNode::prepare(settings, context, bank)?;
        let main = (settings.plugin.role == daw_core::HostedRole::Effect)
            .then(|| native.native.metadata.inputs.iter().position(|p| p.main))
            .flatten();
        let signature = native.native.metadata.audio_layout_signature();
        let time = owner.map_or(alignment.track_max, |i| alignment.input_frames[i]) + stage;
        let mut routes = Vec::new();
        for input in &settings.audio_inputs {
            let Some(source) = plan
                .project
                .tracks
                .iter()
                .position(|t| t.id == input.source_track)
            else {
                continue;
            };
            let bus = match &input.port {
                HostedInputId::Clap { id } => native
                    .native
                    .metadata
                    .inputs
                    .iter()
                    .position(|p| p.id == *id),
                HostedInputId::Vst3 { index, layout } if layout == &signature => native
                    .native
                    .metadata
                    .inputs
                    .get(*index as usize)
                    .map(|_| *index as usize),
                _ => None,
            };
            let Some(bus) = bus else {
                continue;
            };
            if Some(bus) == main {
                return Err(daw_plugin_host::Error::Invalid(
                    "An effect's main audio input is owned by its unit chain".into(),
                ));
            }
            let frames = time
                .checked_sub(alignment.node_frames[source])
                .ok_or_else(|| {
                    daw_plugin_host::Error::Invalid(
                        "Plugin PCM input is not latency-aligned".into(),
                    )
                })?;
            routes.push(Route {
                source,
                bus,
                settings: input.clone(),
                delay: latency::Delay::new(input.id, frames),
                captured: super::audio_sidechain::Contribution::prepare(
                    plan,
                    input.source_track,
                    input.tap,
                    time,
                ),
            });
        }
        Ok(Self {
            native,
            clock_delay: time,
            main,
            routes,
            control_delay: None,
        })
    }
    pub fn configure_controls(&mut self, controls: &Controls, frames: u64) {
        self.control_delay = Some(super::hosted_control_delay::ControlDelay::new(
            &self.native.settings,
            controls,
            frames,
        ));
    }
    pub fn controls(&mut self, offset: usize, controls: &Controls, context: ModulationContext) {
        let delay = self
            .control_delay
            .as_mut()
            .expect("prepared native control alignment");
        let (values, index) = delay.sample(controls.at(offset));
        self.native.controls(
            offset,
            values,
            Some(index),
            ModulationContext {
                shared_sources: Some(index),
                ..context
            },
        );
    }
    pub fn inherit_control_delay(&mut self, previous: &mut Self) {
        for route in &mut self.routes {
            if let Some(old) = previous.routes.iter_mut().find(|old| {
                old.settings == route.settings && old.delay.frames() == route.delay.frames()
            }) {
                std::mem::swap(&mut route.delay, &mut old.delay);
                std::mem::swap(&mut route.captured, &mut old.captured);
            }
        }

        if let (Some(next), Some(old)) = (&mut self.control_delay, &mut previous.control_delay) {
            next.inherit(old);
        }
    }
    pub fn begin(&mut self) {
        self.native.begin();
    }
    pub fn taps(&self) -> impl Iterator<Item = &super::captured_taps::Tap> {
        self.routes
            .iter()
            .flat_map(|r| r.captured.iter().map(|c| &c.tap))
    }
    pub fn taps_mut(&mut self) -> impl Iterator<Item = &mut super::captured_taps::Tap> {
        self.routes
            .iter_mut()
            .flat_map(|r| r.captured.iter_mut().map(|c| &mut c.tap))
    }
    pub fn inputs(
        &mut self,
        audio: &[[f32; 2]],
        taps: &Taps,
        start: u64,
        region: Option<(u64, u64)>,
        realtime: bool,
    ) {
        if let Some(bus) = self.main {
            self.native.inputs[bus][..audio.len()].copy_from_slice(audio);
        }
        for route in &mut self.routes {
            let gain = if route.settings.enabled {
                daw_core::linear_gain(route.settings.gain_db)
            } else {
                0.0
            };
            for frame in 0..audio.len() {
                let (raw, mono) = taps.raw[route.source][frame];
                let value = if route.settings.tap == SidechainTap::PreFader {
                    raw
                } else {
                    let base = if mono { 0 } else { 2 };
                    std::array::from_fn(|ch| raw[ch] * taps.post[route.source][frame][base + ch])
                };
                let mut value = route.delay.tick((value, false)).0;
                for captured in &mut route.captured {
                    let contribution = captured
                        .tap
                        .sample(start + frame as u64, region, realtime)
                        .0;
                    for ch in 0..2 {
                        value[ch] += contribution[ch]
                            * if route.settings.tap == SidechainTap::PostFader {
                                taps.post[captured.source][frame][2 + ch]
                            } else {
                                1.0
                            };
                    }
                }
                self.native.inputs[route.bus][frame] =
                    route.settings.channels.apply(value).map(|v| v * gain);
            }
        }
    }
}
enum Stage {
    Dry {
        delay: latency::Delay,
        channels: u32,
    },
    Builtin {
        range: std::ops::Range<usize>,
        delay: u64,
    },
    Native {
        node: Box<Node>,
        delay: u64,
    },
}
pub(super) struct Chain {
    stages: Vec<Stage>,
    output_mono: Option<bool>,
}
impl Chain {
    pub fn inherit_dry_delays(&mut self, previous: &mut Self) {
        for stage in &mut self.stages {
            if let Stage::Dry { delay, .. } = stage
                && let Some(old) = previous.stages.iter_mut().find_map(|stage| match stage {
                    Stage::Dry { delay: old, .. } if old.id == delay.id => Some(old),
                    _ => None,
                })
            {
                std::mem::swap(delay, old);
            }
        }
    }

    pub fn prepare(
        effects: &[EffectInstance],
        plan: &RenderPlan,
        alignment: &latency::Alignment,
        owner: Option<usize>,
        start: u64,
        bank: &mut super::hosted_node::NativeBank,
    ) -> Result<Self> {
        let mut stages = Vec::new();
        let mut first = 0;
        let mut delay = start;
        let mut segment_delay = start;
        for (index, effect) in effects.iter().enumerate() {
            if let EffectSettings::Hosted(settings) = &effect.unit {
                if first < index {
                    stages.push(Stage::Builtin {
                        range: first..index,
                        delay: segment_delay,
                    });
                }
                if bank.contains_key(&settings.id) {
                    let mut node = Node::prepare(settings, plan, alignment, owner, delay, bank)?;
                    node.native.effect_id = Some(effect.id);
                    stages.push(Stage::Native {
                        node: Box::new(node),
                        delay,
                    });
                } else {
                    stages.push(Stage::Dry {
                        delay: latency::Delay::new(settings.id, u64::from(settings.latency_frames)),
                        channels: settings.output_channels,
                    });
                }
                first = index + 1;
                segment_delay = delay + u64::from(settings.latency_frames);
            }
            delay +=
                latency::chain_frames(std::slice::from_ref(effect), plan.project.sample_rate_hz);
        }
        if first < effects.len() {
            stages.push(Stage::Builtin {
                range: first..effects.len(),
                delay: segment_delay,
            });
        }
        let mono = super::track_processing::channel_shape(true, effects);
        let stereo = super::track_processing::channel_shape(false, effects);
        // Built-in audio chains retain their established source-channel policy.
        // A native output port introduces a declared shape for following stages.
        let output_mono = effects
            .iter()
            .any(|e| matches!(e.unit, EffectSettings::Hosted(_)))
            .then(|| (mono == stereo).then_some(mono))
            .flatten();
        Ok(Self {
            stages,
            output_mono,
        })
    }
    pub fn output_mono(&self, input: bool) -> bool {
        self.output_mono.unwrap_or(input)
    }
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.stages)
            + self
                .stages
                .iter()
                .map(|s| match s {
                    Stage::Native { node, .. } => {
                        node.native.heap_bytes()
                            + node.control_delay.as_ref().map_or(0, |d| d.heap_bytes())
                            + daw_unit_core::memory::vec_bytes(&node.routes)
                            + node
                                .routes
                                .iter()
                                .map(|r| {
                                    r.delay.heap_bytes()
                                        + daw_unit_core::memory::vec_bytes(&r.captured)
                                        + r.captured.iter().map(|c| c.tap.heap_bytes()).sum::<u64>()
                                })
                                .sum::<u64>()
                    }
                    Stage::Dry { delay, .. } => delay.heap_bytes(),
                    _ => 0,
                })
                .sum::<u64>()
    }
    pub fn quantum(&self, maximum: usize) -> usize {
        self.stages.iter().fold(maximum, |size, s| match s {
            Stage::Native { node, .. } => size.min(node.native.event_quantum()),
            _ => size,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        chain: &mut EffectChain,
        audio: &mut [[f32; 2]],
        controls: &Controls,
        clock: &daw_core::MusicalClock,
        start: u64,
        region: Option<(u64, u64)>,
        transport: NativeTransport,
        taps: &Taps,
        realtime: bool,
    ) {
        for stage in &mut self.stages {
            match stage {
                Stage::Dry { delay, channels } => {
                    for sample in audio.iter_mut() {
                        let value = delay.tick((*sample, false)).0;
                        *sample = if *channels == 1 {
                            [(value[0] + value[1]) * 0.5; 2]
                        } else {
                            value
                        };
                    }
                }
                Stage::Builtin { range, delay } => {
                    for (index, sample) in audio.iter_mut().enumerate() {
                        let frame = latency::delayed_frame(start + index as u64, *delay, region);
                        *sample = chain
                            .sample_range(
                                *sample,
                                Some(false),
                                frame,
                                clock.tempo_at(frame),
                                region,
                                (controls.at(index), Some(&controls.index)),
                                range.clone(),
                            )
                            .0;
                    }
                }
                Stage::Native { node, delay } => {
                    node.begin();
                    node.inputs(audio, taps, transport.frame, region, realtime);
                    for index in 0..audio.len() {
                        let frame = latency::delayed_frame(start + index as u64, *delay, region);
                        node.controls(
                            index,
                            controls,
                            ModulationContext {
                                frame,
                                tempo_bpm: clock.tempo_at(frame),
                                musical: Some(daw_unit_core::MusicalPosition {
                                    quarter: clock.quarter_at(frame),
                                    note_origin: None,
                                    legato_origin: None,
                                }),
                                ..Default::default()
                            },
                        );
                    }
                    let frame = latency::delayed_frame(start, *delay, region);
                    node.native.process(
                        audio,
                        NativeTransport {
                            frame,
                            quarter: clock.quarter_at(frame),
                            tempo: f64::from(clock.tempo_at(frame)),
                            numerator: clock.signature_at(frame).numerator,
                            denominator: clock.signature_at(frame).denominator,
                            ..transport
                        },
                    );
                }
            }
        }
    }
    pub fn flush(&mut self) {
        for stage in &mut self.stages {
            if let Stage::Native { node, .. } = stage {
                node.native.flush();
            }
        }
    }
    pub fn native(&self) -> impl Iterator<Item = &HostedNode> {
        self.stages.iter().filter_map(|s| match s {
            Stage::Native { node, .. } => Some(&node.native),
            _ => None,
        })
    }
    pub fn configure_controls(&mut self, controls: &Controls) {
        for stage in &mut self.stages {
            if let Stage::Native { node, delay } = stage {
                node.configure_controls(controls, *delay);
            }
        }
    }
    pub fn taps(&self) -> impl Iterator<Item = &super::captured_taps::Tap> {
        self.stages
            .iter()
            .filter_map(|s| {
                if let Stage::Native { node, .. } = s {
                    Some(node.as_ref())
                } else {
                    None
                }
            })
            .flat_map(|n| n.taps())
    }
    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.stages.iter_mut().filter_map(|s| match s {
            Stage::Native { node, .. } => Some(node.as_mut()),
            _ => None,
        })
    }
    pub fn native_mut(&mut self) -> impl Iterator<Item = &mut HostedNode> {
        self.stages.iter_mut().filter_map(|s| match s {
            Stage::Native { node, .. } => Some(&mut node.native),
            _ => None,
        })
    }
}
