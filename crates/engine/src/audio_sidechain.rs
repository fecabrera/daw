//! Worker-prepared sidechain taps, delay alignment, provider publication and telemetry.
use super::{RenderPlan, latency};
use daw_core::{Id, Modulation, Project};
use daw_unit_core::{
    ModulationOutput, ModulationSourceContext, ModulationSourceSettings, ModulationValue,
    ProcessingContext, SidechainFollower, SidechainProviderSettings, SidechainTap,
};

#[derive(Clone)]
pub(super) struct Spec {
    pub destination: usize,
    pub route: Modulation,
    pub stage: u64,
}
/// Route stage is measured from the instrument's input. This includes preceding effects,
/// layer summation alignment, Output effects and the outer track effect chain.
pub(super) fn specs(project: &Project) -> Vec<Spec> {
    fn chain(
        out: &mut Vec<Spec>,
        destination: usize,
        effects: &[daw_core::EffectInstance],
        mut stage: u64,
        rate: u32,
    ) {
        for e in effects {
            out.extend(
                e.unit
                    .modulations()
                    .iter()
                    .filter(|r| r.sidechain.is_some())
                    .map(|r| Spec {
                        destination,
                        route: *r,
                        stage,
                    }),
            );
            stage += latency::chain_frames(std::slice::from_ref(e), rate);
        }
    }
    let mut out = Vec::new();
    let rate = project.sample_rate_hz;
    for (destination, t) in project.tracks.iter().enumerate() {
        if let Some(i) = &t.instrument {
            for l in &i.layers {
                out.extend(
                    l.modulation_routes()
                        .filter(|r| {
                            r.sidechain.is_some()
                                && !l.effects.iter().any(|e| {
                                    e.unit
                                        .modulations()
                                        .iter()
                                        .any(|v| v.sidechain.is_some() && v.id == r.id)
                                })
                        })
                        .map(|r| Spec {
                            destination,
                            route: *r,
                            stage: 0,
                        }),
                );
                chain(&mut out, destination, &l.effects, 0, rate);
            }
            for b in i.buses() {
                out.extend(
                    b.modulation_routes()
                        .filter(|r| {
                            r.sidechain.is_some()
                                && !b.effects.iter().any(|e| {
                                    e.unit
                                        .modulations()
                                        .iter()
                                        .any(|v| v.sidechain.is_some() && v.id == r.id)
                                })
                        })
                        .map(|r| Spec {
                            destination,
                            route: *r,
                            stage: 0,
                        }),
                );
            }
            let layer_max = i
                .layers
                .iter()
                .map(|l| latency::chain_frames(&l.effects, rate))
                .max()
                .unwrap_or(0);
            chain(&mut out, destination, &i.main.effects, layer_max, rate);
        }
        let instrument = t
            .instrument
            .as_ref()
            .map_or(0, |i| super::layer_effects::latency(i, rate));
        chain(&mut out, destination, &t.effects, instrument, rate);
    }
    chain(
        &mut out,
        project.tracks.len(),
        &project.master.effects,
        0,
        rate,
    );
    out
}
#[derive(Clone)]
struct Contribution {
    source: usize,
    tap: super::captured_taps::Tap,
}
#[derive(Clone)]
struct Edge {
    captured: Vec<Contribution>,
    id: Id,
    source: Option<usize>,
    settings: daw_unit_core::AudioSidechain,
    follower: SidechainFollower,
    output: ModulationOutput,
    delay: latency::Delay,
}
#[derive(Clone)]
pub(super) struct Sidechains {
    pub indices: Vec<daw_unit_core::ModulationSourceIndex>,
    active: bool,
    slots: std::collections::HashMap<Id, (usize, usize)>,
    clocks: Vec<Option<u64>>,
    edges: Vec<Vec<Edge>>,
    pub values: Vec<Vec<ModulationValue>>,
    pub order: Vec<usize>,
}
impl Sidechains {
    pub fn new(plan: &RenderPlan, alignment: &latency::Alignment) -> Self {
        let p = &plan.project;
        let count = p.tracks.len();
        let context = ProcessingContext {
            sample_rate_hz: p.sample_rate_hz,
            max_block_frames: p.buffer_size_frames,
        };
        let mut this = Self {
            indices: Vec::new(),
            active: false,
            slots: std::collections::HashMap::new(),
            clocks: vec![None; count],
            edges: vec![Vec::new(); count + 1],
            values: vec![Vec::new(); count + 1],
            order: p.sidechain_order().expect("validated audio/control graph"),
        };
        for spec in specs(p) {
            let settings = spec.route.sidechain.unwrap();
            let source = p.tracks.iter().position(|t| t.id == settings.track);
            let provider = SidechainProviderSettings {
                id: spec.route.id,
                present: source.is_some(),
                enabled: true,
            };
            let metadata = provider.modulation_source(settings.track, "Audio sidechain", 0);
            let time = if spec.destination == count {
                alignment.track_max
            } else {
                alignment.input_frames[spec.destination]
            };
            let delay = source.map_or(0, |s| time + spec.stage - alignment.node_frames[s]);
            this.active = true;
            this.slots.insert(
                spec.route.id,
                (spec.destination, this.edges[spec.destination].len()),
            );
            let captured = if source.is_none() {
                Vec::new()
            } else {
                p.tracks
                    .iter()
                    .enumerate()
                    .filter_map(|(index, track)| {
                        let tap = super::captured_taps::Tap::new(
                            plan,
                            track,
                            |r| {
                                r.feeds
                                    .iter()
                                    .find(|f| f.provider == settings.track)
                                    .map(|f| {
                                        if settings.tap == SidechainTap::PreFader {
                                            f.raw_asset
                                        } else {
                                            f.post_asset
                                        }
                                    })
                            },
                            0,
                            time + spec.stage,
                        )?;
                        Some(Contribution { source: index, tap })
                    })
                    .collect()
            };
            this.edges[spec.destination].push(Edge {
                captured,
                id: spec.route.id,
                source,
                settings,
                follower: SidechainFollower::new(settings, context),
                output: ModulationOutput::new(metadata.id, metadata.present, context),
                delay: latency::Delay::new(spec.route.id, delay),
            });
            this.values[spec.destination].push(ModulationValue {
                source: metadata.id,
                value: 0.0,
                weight: 0.0,
                range: metadata.output.range,
                behavior: metadata.output.behavior,
                release_age: None,
            });
        }
        this.indices = this
            .values
            .iter()
            .map(|v| daw_unit_core::ModulationSourceIndex::from_values(v))
            .collect();
        this
    }
    pub(super) fn taps_mut(&mut self) -> impl Iterator<Item = &mut super::captured_taps::Tap> {
        self.edges
            .iter_mut()
            .flatten()
            .flat_map(|e| e.captured.iter_mut().map(|c| &mut c.tap))
    }
    pub(super) fn taps(&self) -> impl Iterator<Item = &super::captured_taps::Tap> {
        self.edges
            .iter()
            .flatten()
            .flat_map(|e| e.captured.iter().map(|c| &c.tap))
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn wrapped(&mut self, index: usize, clock: u64, region: Option<(u64, u64)>) -> bool {
        let wrapped = region.is_some() && self.clocks[index].is_some_and(|old| clock < old);
        self.clocks[index] = Some(clock);
        wrapped
    }
    pub fn tick(
        &mut self,
        destination: usize,
        raw: &[latency::Sample],
        post: &[[f32; 4]],
        frame: u64,
        region: Option<(u64, u64)>,
        realtime: bool,
    ) {
        for (edge, value) in self.edges[destination]
            .iter_mut()
            .zip(&mut self.values[destination])
        {
            let (mut sample, mono) = edge.source.map_or(([0.0; 2], false), |s| raw[s]);
            if edge.settings.tap == SidechainTap::PostFader
                && let Some(s) = edge.source
            {
                let base = if mono { 0 } else { 2 };
                sample = std::array::from_fn(|ch| sample[ch] * post[s][base + ch]);
            }
            let (sample, _) = edge.delay.tick((sample, mono));
            let mut sample = sample;
            for contribution in &mut edge.captured {
                let (value, _) = contribution.tap.sample(frame, region, realtime);
                for ch in 0..2 {
                    let gain = if edge.settings.tap == SidechainTap::PostFader {
                        post[contribution.source][2 + ch]
                    } else {
                        1.0
                    };
                    sample[ch] += value[ch] * gain;
                }
            }
            edge.follower.feed(sample);
            *value = edge.output.sample(
                &mut edge.follower,
                ModulationSourceContext {
                    frame,
                    ..Default::default()
                },
            );
        }
    }
    pub fn readings(&self, id: Id) -> Option<[f32; 2]> {
        let &(destination, slot) = self.slots.get(&id)?;
        let edge = &self.edges[destination][slot];
        edge.source.map(|_| edge.follower.readings())
    }
    pub fn reset(&mut self) {
        self.clocks.fill(None);
        for e in self.edges.iter_mut().flatten() {
            e.delay.reset();
            for c in &mut e.captured {
                c.tap.delay.reset();
            }
            e.follower.reset();
            e.output.reset();
        }
    }
    pub fn compatible(&self, old: &Self) -> bool {
        if !self.active() && !old.active() {
            return true;
        }
        self.edges.iter().zip(&old.edges).all(|(a, b)| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    a.id == b.id
                        && a.source == b.source
                        && a.settings.track == b.settings.track
                        && a.settings.tap == b.settings.tap
                        && a.delay.frames() == b.delay.frames()
                        && a.captured.len() == b.captured.len()
                        && a.captured.iter().zip(&b.captured).all(|(a, b)| {
                            a.source == b.source && a.tap.reader.same_source(&b.tap.reader)
                        })
                })
        }) && self.edges.len() == old.edges.len()
    }
    pub fn inherit(&mut self, old: &mut Self) {
        if self.active {
            self.clocks.copy_from_slice(&old.clocks);
        }
        for e in self.edges.iter_mut().flatten() {
            if let Some(&(destination, slot)) = old.slots.get(&e.id) {
                let previous = &mut old.edges[destination][slot];
                if previous.settings.track != e.settings.track
                    || previous.settings.tap != e.settings.tap
                {
                    continue;
                }
                e.follower.inherit_history(&previous.follower);
                std::mem::swap(&mut e.delay, &mut previous.delay);
                previous.output.retarget(&e.output);
                std::mem::swap(&mut e.output, &mut previous.output);
            }
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        vec_bytes(&self.edges)
            + vec_bytes(&self.indices)
            + vec_bytes(&self.clocks)
            + self.indices.iter().map(|i| i.heap_bytes()).sum::<u64>()
            + daw_unit_core::memory::map_bytes(&self.slots)
            + vec_bytes(&self.values)
            + vec_bytes(&self.order)
            + self.values.iter().map(vec_bytes).sum::<u64>()
            + self
                .edges
                .iter()
                .map(|v| {
                    vec_bytes(v)
                        + v.iter()
                            .map(|e| {
                                e.delay.heap_bytes()
                                    + daw_unit_core::memory::vec_bytes(&e.captured)
                                    + e.captured.iter().map(|c| c.tap.heap_bytes()).sum::<u64>()
                            })
                            .sum::<u64>()
                })
                .sum::<u64>()
    }
}
