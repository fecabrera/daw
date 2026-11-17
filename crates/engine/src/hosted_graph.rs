//! Shared live/offline block graph. Source scheduling and provider publication
//! remain in the existing runtime. Foreign DSP has one exclusive block owner.
use super::{
    Renderer,
    hosted_block::{Chain, Controls, Node, Taps},
    latency,
};
use daw_core::{Id, TrackKind};
use daw_plugin_host::{NativeEvent, NativeTransport, Result};

#[derive(Clone, Copy)]
struct Note {
    identity: Id,
    generation: u64,
    key: u8,
    id: i32,
}
struct Notes {
    previous: Vec<Option<Note>>,
    next: i32,
}
impl Notes {
    fn new(capacity: usize) -> Self {
        Self {
            previous: vec![None; capacity],
            next: 0,
        }
    }
    fn capture(&mut self, runtime: &super::midi::MidiTrackRuntime, node: &mut Node, offset: usize) {
        for (slot, (current, key, velocity, generation)) in runtime.block_notes().enumerate() {
            let Some(previous) = self.previous.get_mut(slot) else {
                node.native.fail();
                return;
            };
            if previous.is_some_and(|p| {
                Some(p.identity) == current && p.key == key && p.generation == generation
            }) {
                continue;
            }
            if let Some(old) = previous.take() {
                node.native.note(NativeEvent::Note {
                    offset: offset as u32,
                    on: false,
                    id: old.id,
                    channel: 0,
                    key: i16::from(old.key),
                    velocity: 0.0,
                });
            }
            if let Some(identity) = current {
                let id = self.next;
                self.next = self.next.wrapping_add(1) & i32::MAX;
                *previous = Some(Note {
                    identity,
                    key,
                    id,
                    generation,
                });
                node.native.note(NativeEvent::Note {
                    offset: offset as u32,
                    on: true,
                    id,
                    channel: 0,
                    key: i16::from(key),
                    velocity: f64::from(velocity),
                });
            }
        }
    }
}
struct Source {
    node: Node,
    notes: Notes,
    fade: latency::Delay,
}
struct Layer {
    source: Option<Source>,
    chain: Chain,
    samples: Vec<[f32; 2]>,
}
struct Instrument {
    layers: Vec<Layer>,
    main: Chain,
    gains: Vec<[f32; 2]>,
    fades: Vec<f32>,
}
struct Track {
    chain: Chain,
    instrument: Option<Instrument>,
    samples: Vec<[f32; 2]>,
    mono: Vec<bool>,
    controls: Controls,
}
pub(super) struct HostedGraph {
    tracks: Vec<Track>,
    master: Chain,
    master_controls: Controls,
    taps: Taps,
    capture_taps: Option<Taps>,
    order: Vec<usize>,
    roots: Vec<usize>,
    send_values: Vec<Vec<[f32; 8]>>,
    pub output: Vec<[f32; 2]>,
    pub consumed: usize,
    pub count: usize,
    pub quantum: usize,
    pub primed: bool,
    pub evaluated: bool,
    active: bool,
    pub clock_delays: Vec<u64>,
    capture: Option<Vec<Vec<Vec<daw_unit_core::ModulationValue>>>>,
    capture_override: bool,
}
impl HostedGraph {
    pub fn prepare(
        renderer: &mut Renderer,
        bank: &mut super::hosted_node::NativeBank,
    ) -> Result<Self> {
        let plan = &renderer.plan;
        let maximum = plan.project.buffer_size_frames as usize;
        let alignment = &renderer.alignment;
        let mut tracks = Vec::new();
        let mut quantum = maximum;
        for (index, track) in plan.project.tracks.iter().enumerate() {
            let instrument_delay = track.instrument.as_ref().map_or(0, |i| {
                super::layer_effects::latency(i, plan.project.sample_rate_hz)
            });
            let chain = Chain::prepare(
                &track.effects,
                plan,
                alignment,
                Some(index),
                instrument_delay,
                bank,
            )?;
            quantum = quantum.min(chain.quantum(maximum));
            let needed = track
                .effects
                .iter()
                .flat_map(|e| e.unit.modulations())
                .chain(track.instrument.iter().flat_map(|i| {
                    i.layers
                        .iter()
                        .flat_map(|l| {
                            l.effects.iter().flat_map(|e| e.unit.modulations()).chain(
                                l.hosted
                                    .iter()
                                    .filter_map(|s| s.settings())
                                    .flat_map(|s| &s.modulations),
                            )
                        })
                        .chain(i.main.effects.iter().flat_map(|e| e.unit.modulations()))
                }))
                .filter_map(|r| r.effective_source())
                .collect();
            let controls = if let Some(runtime) = &renderer.midi[index] {
                Controls::new(runtime.modulation_values(), &needed, maximum)
            } else {
                Controls::new(&renderer.sidechains.values[index], &needed, maximum)
            };
            let instrument = if let (Some(settings), Some(runtime)) =
                (&track.instrument, &mut renderer.midi[index])
            {
                runtime.collect_block_sources(true);
                let mut layers = Vec::new();
                for bus in runtime
                    .layer_effects
                    .iter()
                    .take(runtime.layer_effects.len() - 1)
                {
                    let layer = settings
                        .layers
                        .iter()
                        .find(|l| l.id == bus.id)
                        .expect("prepared layer identity");
                    let source = layer
                        .hosted
                        .as_ref()
                        .and_then(|s| s.settings())
                        .filter(|s| bank.contains_key(&s.id))
                        .map(|s| {
                            Node::prepare(s, plan, alignment, Some(index), 0, bank).map(|node| {
                                Source {
                                    fade: latency::Delay::new(s.id, u64::from(s.latency_frames)),
                                    node,
                                    notes: Notes::new(runtime.block_note_capacity()),
                                }
                            })
                        })
                        .transpose()?;
                    let source_delay = layer
                        .hosted
                        .as_ref()
                        .and_then(|s| s.settings())
                        .map_or(0, |s| u64::from(s.latency_frames));
                    if let Some(source) = &source {
                        quantum = quantum.min(source.node.native.event_quantum());
                    }
                    let chain = Chain::prepare(
                        &layer.effects,
                        plan,
                        alignment,
                        Some(index),
                        source_delay,
                        bank,
                    )?;
                    quantum = quantum.min(chain.quantum(maximum));
                    layers.push(Layer {
                        source,
                        chain,
                        samples: vec![[0.0; 2]; maximum],
                    });
                }
                let main = Chain::prepare(
                    &settings.main.effects,
                    plan,
                    alignment,
                    Some(index),
                    super::layer_effects::parallel_latency(settings, plan.project.sample_rate_hz),
                    bank,
                )?;
                quantum = quantum.min(main.quantum(maximum));
                Some(Instrument {
                    layers,
                    main,
                    gains: vec![[1.0; 2]; maximum],
                    fades: vec![1.0; maximum],
                })
            } else {
                None
            };
            tracks.push(Track {
                chain,
                instrument,
                samples: vec![[0.0; 2]; maximum],
                mono: vec![false; maximum],
                controls,
            });
        }
        let master = Chain::prepare(&plan.project.master.effects, plan, alignment, None, 0, bank)?;
        quantum = quantum.min(master.quantum(maximum));
        let last = plan.project.tracks.len();
        let master_needed = plan
            .project
            .master
            .effects
            .iter()
            .flat_map(|e| e.unit.modulations())
            .filter_map(|r| r.effective_source())
            .collect();
        let mut graph = Self {
            tracks,
            master,
            master_controls: Controls::new(
                &renderer.sidechains.values[last],
                &master_needed,
                maximum,
            ),
            taps: Taps::new(last, maximum),
            capture_taps: None,
            order: plan
                .project
                .sidechain_order()
                .map_err(|e| daw_plugin_host::Error::Invalid(e.to_string()))?,
            roots: plan
                .project
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(i, t)| t.group_id.is_none().then_some(i))
                .collect(),
            send_values: vec![vec![[0.0; 8]; renderer.buses.sends.source_count()]; maximum],
            output: vec![[0.0; 2]; maximum],
            consumed: 0,
            count: 0,
            quantum,
            primed: false,
            evaluated: false,
            active: false,
            clock_delays: Vec::new(),
            capture: None,
            capture_override: false,
        };
        for track in &mut graph.tracks {
            track.chain.configure_controls(&track.controls);
            if let Some(instrument) = &mut track.instrument {
                instrument.main.configure_controls(&track.controls);
                for layer in &mut instrument.layers {
                    layer.chain.configure_controls(&track.controls);
                    if let Some(source) = &mut layer.source {
                        source.node.configure_controls(&track.controls, 0);
                    }
                }
            }
        }
        graph.master.configure_controls(&graph.master_controls);
        graph.clock_delays = graph
            .nodes_mut()
            .map(|n| n.clock_delay)
            .chain(std::iter::once(0))
            .collect();
        graph.clock_delays.sort_unstable();
        graph.clock_delays.dedup();
        Ok(graph)
    }
    // Bounce owns this bounded worker-only bank. Keep every frame of the reference
    // follower publications so isolation cannot alter external control signals.
    pub fn enable_capture(&mut self, renderer: &Renderer, override_values: bool) {
        self.capture = Some(
            renderer
                .sidechains
                .values
                .iter()
                .map(|v| vec![v.clone(); self.output.len()])
                .collect(),
        );
        self.capture_override = override_values;
        self.capture_taps =
            override_values.then(|| Taps::new(self.tracks.len(), self.output.len()));
    }
    pub fn copy_capture(&mut self, reference: &Self) {
        if let Some(taps) = &mut self.capture_taps {
            taps.clone_from(&reference.taps);
        }
        self.capture
            .as_mut()
            .expect("capture bank")
            .clone_from(reference.capture.as_ref().expect("reference bank"));
    }
    pub fn expose_capture(&self, renderer: &mut Renderer, offset: usize) {
        for index in 0..self.taps.raw.len() {
            renderer.buses.raw[index] = self.taps.raw[index][offset];
            renderer.sidechain_post[index] = self.taps.post[index][offset];
        }
        if let Some(bank) = &self.capture {
            for (values, frames) in renderer.sidechains.values.iter_mut().zip(bank) {
                values.copy_from_slice(&frames[offset]);
            }
        }
    }
    fn capture_controls(
        bank: &mut Option<Vec<Vec<Vec<daw_unit_core::ModulationValue>>>>,
        override_values: bool,
        values: &mut [daw_unit_core::ModulationValue],
        index: usize,
        offset: usize,
    ) {
        if let Some(bank) = bank {
            if override_values {
                values.copy_from_slice(&bank[index][offset]);
            } else {
                bank[index][offset].copy_from_slice(values);
            }
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        let tracks = vec_bytes(&self.tracks)
            + self
                .tracks
                .iter()
                .map(|t| {
                    t.chain.heap_bytes()
                        + t.controls.heap_bytes()
                        + vec_bytes(&t.samples)
                        + vec_bytes(&t.mono)
                        + t.instrument.as_ref().map_or(0, |i| {
                            vec_bytes(&i.layers)
                                + i.main.heap_bytes()
                                + vec_bytes(&i.gains)
                                + vec_bytes(&i.fades)
                                + i.layers
                                    .iter()
                                    .map(|l| {
                                        l.chain.heap_bytes()
                                            + vec_bytes(&l.samples)
                                            + l.source.as_ref().map_or(0, |s| {
                                                s.node.native.heap_bytes()
                                                    + vec_bytes(&s.notes.previous)
                                            })
                                    })
                                    .sum::<u64>()
                        })
                })
                .sum::<u64>();
        tracks
            + self.capture_taps.as_ref().map_or(0, |t| t.heap_bytes())
            + self.master.heap_bytes()
            + self.master_controls.heap_bytes()
            + vec_bytes(&self.output)
            + vec_bytes(&self.order)
            + vec_bytes(&self.roots)
            + vec_bytes(&self.taps.raw)
            + self.taps.raw.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.taps.post)
            + self.taps.post.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.taps.output)
            + self.taps.output.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.send_values)
            + self.send_values.iter().map(vec_bytes).sum::<u64>()
    }
    fn native(&self) -> impl Iterator<Item = &super::hosted_node::HostedNode> {
        self.tracks
            .iter()
            .flat_map(|t| {
                t.chain.native().chain(t.instrument.iter().flat_map(|i| {
                    i.main.native().chain(i.layers.iter().flat_map(|l| {
                        l.chain
                            .native()
                            .chain(l.source.iter().map(|s| &s.node.native))
                    }))
                }))
            })
            .chain(self.master.native())
    }
    fn native_mut(&mut self) -> impl Iterator<Item = &mut super::hosted_node::HostedNode> {
        self.tracks
            .iter_mut()
            .flat_map(|t| {
                t.chain
                    .native_mut()
                    .chain(t.instrument.iter_mut().flat_map(|i| {
                        i.main.native_mut().chain(i.layers.iter_mut().flat_map(|l| {
                            l.chain
                                .native_mut()
                                .chain(l.source.iter_mut().map(|s| &mut s.node.native))
                        }))
                    }))
            })
            .chain(self.master.native_mut())
    }
    fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.tracks
            .iter_mut()
            .flat_map(|t| {
                t.chain
                    .nodes_mut()
                    .chain(t.instrument.iter_mut().flat_map(|i| {
                        i.main.nodes_mut().chain(i.layers.iter_mut().flat_map(|l| {
                            l.chain
                                .nodes_mut()
                                .chain(l.source.iter_mut().map(|s| &mut s.node))
                        }))
                    }))
            })
            .chain(self.master.nodes_mut())
    }
    pub fn taps_mut(&mut self) -> impl Iterator<Item = &mut super::captured_taps::Tap> {
        self.nodes_mut().flat_map(|n| n.taps_mut())
    }
    pub fn taps(&self) -> impl Iterator<Item = &super::captured_taps::Tap> {
        self.tracks
            .iter()
            .flat_map(|t| {
                t.chain.taps().chain(t.instrument.iter().flat_map(|i| {
                    i.main.taps().chain(i.layers.iter().flat_map(|l| {
                        l.chain
                            .taps()
                            .chain(l.source.iter().flat_map(|s| s.node.taps()))
                    }))
                }))
            })
            .chain(self.master.taps())
    }
    pub fn compatible(&self, previous: &Self) -> bool {
        self.native().count() == previous.native().count()
            && self
                .native()
                .all(|node| previous.native().any(|old| node.compatible(old)))
    }
    pub fn inherit(&mut self, previous: &mut Self) {
        for node in self.nodes_mut() {
            if let Some(old) = previous
                .nodes_mut()
                .find(|old| node.native.compatible(&old.native))
            {
                node.native.inherit(&mut old.native);
                node.inherit_control_delay(old);
            }
        }
        for source in self
            .tracks
            .iter_mut()
            .filter_map(|t| t.instrument.as_mut())
            .flat_map(|i| &mut i.layers)
            .filter_map(|l| l.source.as_mut())
        {
            if let Some(old) = previous
                .tracks
                .iter_mut()
                .filter_map(|t| t.instrument.as_mut())
                .flat_map(|i| &mut i.layers)
                .filter_map(|l| l.source.as_mut())
                .find(|old| old.node.native.id == source.node.native.id)
            {
                std::mem::swap(&mut source.notes, &mut old.notes);
                std::mem::swap(&mut source.fade, &mut old.fade);
            }
        }
        std::mem::swap(&mut self.output, &mut previous.output);
        self.consumed = previous.consumed;
        self.count = previous.count;
        self.primed = previous.primed;
    }
    pub fn attach_effect_meter(
        &mut self,
        effect: Id,
        meter: std::sync::Arc<daw_unit_core::BandMeter<3>>,
    ) {
        if let Some(node) = self.native_mut().find(|n| n.effect_id == Some(effect)) {
            node.attach_meter(meter);
        }
    }
    pub fn snapshot(&self, unit: Id, target: Id) -> Option<super::ModulationSnapshot> {
        self.native().find(|n| n.id == unit)?.snapshot(target)
    }
    pub fn fault(&self) -> Option<Id> {
        self.native().find(|n| n.fault()).map(|n| n.id)
    }
    pub fn activate(&mut self) {
        if !self.active {
            for node in self.native_mut() {
                node.native.active.publish(node.native.token);
            }
            self.active = true;
        }
    }
    pub fn invalidate(&mut self) {
        self.count = 0;
        self.consumed = 0;
        self.primed = false;
    }
    pub fn flush(&mut self) {
        for track in &mut self.tracks {
            track.chain.flush();
            if let Some(instrument) = &mut track.instrument {
                instrument.main.flush();
                for layer in &mut instrument.layers {
                    layer.chain.flush();
                    if let Some(source) = &mut layer.source {
                        source.node.native.flush();
                    }
                }
            }
        }
        self.master.flush();
    }
    /// One contiguous graph quantum. The caller splits loop and transport changes.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        start: u64,
        count: usize,
        live: bool,
        advancing: bool,
        realtime: bool,
    ) {
        self.evaluated = true;
        let region = renderer.loop_region(live && advancing);
        let clock = &renderer.clock;
        let signature = clock.signature_at(start);
        let transport = NativeTransport {
            frame: start,
            quarter: clock.quarter_at(start),
            tempo: f64::from(clock.tempo_at(start)),
            playing: advancing,
            numerator: signature.numerator,
            denominator: signature.denominator,
            loop_quarters: region.map(|(a, b)| (clock.quarter_at(a), clock.quarter_at(b))),
        };
        for values in self.send_values.iter_mut().take(count) {
            renderer.buses.sends.advance(live);
            renderer.buses.sends.save_current(values);
        }
        for &index in &self.order {
            let track = &renderer.plan.project.tracks[index];
            let block = &mut self.tracks[index];
            block.samples[..count].fill([0.0; 2]);
            if let Some(instrument) = &mut block.instrument {
                instrument.fades[..count].fill(0.0);
                instrument.gains[..count].fill([0.0; 2]);
                for layer in &mut instrument.layers {
                    layer.samples[..count].fill([0.0; 2]);
                    if let Some(source) = &mut layer.source {
                        source.node.begin();
                    }
                }
            }
            for offset in 0..count {
                let frame = start.saturating_add(offset as u64);
                for source in 0..self.taps.raw.len() {
                    renderer.buses.raw[source] = self.taps.raw[source][offset];
                    renderer.sidechain_post[source] = self.taps.post[source][offset];
                }
                renderer.sidechains.tick(
                    index,
                    &renderer.buses.raw,
                    &renderer.sidechain_post,
                    frame,
                    region,
                    realtime,
                );
                Self::capture_controls(
                    &mut self.capture,
                    self.capture_override,
                    &mut renderer.sidechains.values[index],
                    index,
                    offset,
                );
                let source_frame =
                    latency::delayed_frame(frame, renderer.alignment.input_frames[index], region);
                let valid = frame >= renderer.alignment.input_frames[index]
                    || region.is_some_and(|(first, _)| frame >= first);
                if let Some(runtime) = &mut renderer.midi[index] {
                    if renderer.sidechains.wrapped(index, source_frame, region) {
                        runtime.wrap_loop();
                    }
                    runtime.publish_sidechains(&renderer.sidechains.values[index]);
                    if valid {
                        if advancing {
                            runtime.sample_in_region(source_frame, region);
                        } else {
                            runtime.sample_live(source_frame);
                        }
                    }
                    block.controls.publish(offset, runtime.modulation_values());
                    if let Some(instrument) = &mut block.instrument {
                        if valid {
                            instrument.gains[offset] = runtime.block_output_gain();
                            instrument.fades[offset] = runtime.block_source_fade();
                        }
                        for (layer, sample) in
                            instrument.layers.iter_mut().zip(runtime.block_sources())
                        {
                            if valid {
                                layer.samples[offset] = *sample;
                            }
                            if let Some(source) = &mut layer.source {
                                if valid {
                                    source.notes.capture(runtime, &mut source.node, offset);
                                }
                                source.node.controls(
                                    offset,
                                    &block.controls,
                                    daw_unit_core::ModulationContext {
                                        frame: source_frame,
                                        tempo_bpm: clock.tempo_at(source_frame),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }
                } else {
                    block
                        .controls
                        .publish(offset, &renderer.sidechains.values[index]);
                    if valid && advancing && !super::send_routing::is_bus(track.kind) {
                        let (sample, mono) = renderer.audio_tracks[index].sample(
                            &track.clips,
                            source_frame,
                            realtime,
                        );
                        block.samples[offset] = sample;
                        if let Some(mono) = mono {
                            block.mono[offset] = mono;
                        }
                    }
                }
                if super::send_routing::is_bus(track.kind) {
                    renderer.buses.sends.load_current(&self.send_values[offset]);
                    let mut sum = if track.kind == TrackKind::Return {
                        renderer.buses.sends.sum(
                            index,
                            &renderer.buses.raw,
                            live,
                            frame,
                            region,
                            realtime,
                        )
                    } else {
                        [0.0; 2]
                    };
                    for &child in &renderer.buses.children[index] {
                        for (ch, value) in sum.iter_mut().enumerate() {
                            *value += self.taps.output[child][offset][ch];
                        }
                    }
                    block.samples[offset] = sum;
                }
            }
            let content_start =
                latency::delayed_frame(start, renderer.alignment.input_frames[index], region);
            if !super::send_routing::is_bus(track.kind)
                && let (Some(instrument), Some(runtime)) =
                    (&mut block.instrument, &mut renderer.midi[index])
            {
                let regular = instrument.layers.len();
                let parallel = track.instrument.as_ref().map_or(0, |i| {
                    super::layer_effects::parallel_latency(i, renderer.plan.project.sample_rate_hz)
                });
                for (layer, bus) in instrument.layers.iter_mut().zip(&mut runtime.layer_effects) {
                    if let Some(source) = &mut layer.source {
                        layer.samples[..count].fill([0.0; 2]);
                        source.node.inputs(
                            &layer.samples[..count],
                            self.capture_taps.as_ref().unwrap_or(&self.taps),
                            start,
                            region,
                            realtime,
                        );
                        source.node.native.process(
                            &mut layer.samples[..count],
                            NativeTransport {
                                frame: content_start,
                                quarter: clock.quarter_at(content_start),
                                tempo: f64::from(clock.tempo_at(content_start)),
                                numerator: clock.signature_at(content_start).numerator,
                                denominator: clock.signature_at(content_start).denominator,
                                ..transport
                            },
                        );
                        for (sample, fade) in
                            layer.samples.iter_mut().zip(&instrument.fades).take(count)
                        {
                            let fade = source.fade.tick(([*fade; 2], false)).0[0];
                            *sample = sample.map(|v| v * fade);
                            if let Some(meter) = &mut bus.source_output {
                                meter.sample = *sample;
                                if meter.level.is_enabled() {
                                    meter.level.sample_output(*sample);
                                }
                            }
                        }
                    }
                    layer.chain.process(
                        &mut bus.chain,
                        &mut layer.samples[..count],
                        &block.controls,
                        clock,
                        content_start,
                        region,
                        transport,
                        self.capture_taps.as_ref().unwrap_or(&self.taps),
                        realtime,
                    );
                    for offset in 0..count {
                        let sample = bus.delay.tick((layer.samples[offset], false)).0;
                        let sample = bus.mix(sample);
                        bus.meter_sample = sample;
                        for (peak, value) in bus.peak.iter_mut().zip(sample) {
                            *peak = peak.max(value.abs());
                        }
                        for (sum, value) in block.samples[offset].iter_mut().zip(sample) {
                            *sum += value;
                        }
                    }
                }
                instrument.main.process(
                    &mut runtime.layer_effects[regular].chain,
                    &mut block.samples[..count],
                    &block.controls,
                    clock,
                    content_start,
                    region,
                    transport,
                    self.capture_taps.as_ref().unwrap_or(&self.taps),
                    realtime,
                );
                let _ = parallel; // Main's prepared chain already carries its parallel-stage clock.
                for (sample, gain) in block.samples.iter_mut().zip(&instrument.gains).take(count) {
                    for ch in 0..2 {
                        sample[ch] *= gain[ch];
                    }
                }
            }
            block.chain.process(
                &mut renderer.effects[index],
                &mut block.samples[..count],
                &block.controls,
                clock,
                content_start,
                region,
                transport,
                self.capture_taps.as_ref().unwrap_or(&self.taps),
                realtime,
            );
            for offset in 0..count {
                let sample = block.samples[offset];
                let mono = block.chain.output_mono(block.mono[offset]);
                self.taps.raw[index][offset] = (sample, mono);

                let (sample, mono) = renderer.alignment.tracks[index].tick((sample, mono));
                let content = latency::delayed_frame(
                    start + offset as u64,
                    renderer.alignment.output_frames[index],
                    region,
                );
                let gain = renderer.buses.clip_gains[index].at(content, region);
                self.taps.output[index][offset] = renderer.gains[index].apply(
                    sample.map(|v| v * gain),
                    mono,
                    live,
                    renderer.ramp,
                );
                self.taps.post[index][offset] = if live {
                    renderer.gains[index].value
                } else {
                    renderer.gains[index].target
                };
            }
        }
        for offset in 0..count {
            let mut sum = [0.0; 2];
            for &root in &self.roots {
                for (ch, value) in sum.iter_mut().enumerate() {
                    *value += self.taps.output[root][offset][ch];
                }
            }
            self.output[offset] = sum;
            for source in 0..self.taps.raw.len() {
                renderer.buses.raw[source] = self.taps.raw[source][offset];
                renderer.sidechain_post[source] = self.taps.post[source][offset];
            }
            let index = self.tracks.len();
            renderer.sidechains.tick(
                index,
                &renderer.buses.raw,
                &renderer.sidechain_post,
                start + offset as u64,
                region,
                realtime,
            );
            Self::capture_controls(
                &mut self.capture,
                self.capture_override,
                &mut renderer.sidechains.values[index],
                index,
                offset,
            );
            self.master_controls
                .publish(offset, &renderer.sidechains.values[index]);
        }
        let content = latency::delayed_frame(start, renderer.alignment.track_max, region);
        self.master.process(
            &mut renderer.master_effects,
            &mut self.output[..count],
            &self.master_controls,
            clock,
            content,
            region,
            transport,
            self.capture_taps.as_ref().unwrap_or(&self.taps),
            realtime,
        );
        self.count = count;
        self.consumed = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare(plan: super::super::RenderPlan) {
        let mut scalar = Renderer::new(plan.clone());
        let mut block = Renderer::new(plan);
        block.hosted = Some(HostedGraph::prepare(&mut block, &mut Default::default()).unwrap());
        let mut expected = vec![[0.0; 2]; 8192];
        let mut actual = vec![[0.0; 2]; 8192];
        scalar.seek(0);
        block.seek(0);
        scalar.render_offline(&mut expected);
        let mut start = 0;
        for size in [1, 17, 32, 513, 4096, 3533] {
            block.render_offline(&mut actual[start..start + size]);
            start += size;
        }
        for (index, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            for ch in 0..2 {
                assert!(
                    (expected[ch] - actual[ch]).abs() < 1e-6,
                    "frame {index} channel {ch}: {expected:?} {actual:?}"
                );
            }
        }
        assert!(block.snapshot().is_none());
    }
    #[test]
    fn built_in_audio_block_chain_matches_scalar_with_irregular_consumers() {
        for effects in [
            vec![],
            vec![daw_core::EffectInstance::drive()],
            vec![
                daw_core::EffectInstance::saturator(),
                daw_core::EffectInstance::limiter(),
            ],
        ] {
            let mut plan = super::super::tests::plan();
            plan.project.tracks[0].effects = effects;
            compare(plan);
        }
    }
    #[test]
    fn instrument_layer_and_main_blocks_match_the_existing_source_scheduler() {
        let mut plan = super::super::tests::plan();
        plan.project.tracks.clear();
        let id = plan.project.add_midi_track().unwrap();
        plan.project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
        let clip = plan.project.add_midi_pattern(id, 0).unwrap();
        plan.project
            .edit(daw_core::Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: daw_core::MidiPattern {
                    length_ticks: 3840,
                    notes: vec![daw_core::MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 1920,
                        pitch: 60,
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        let track = plan.project.tracks.iter_mut().find(|t| t.id == id).unwrap();
        let instrument = track.instrument.as_mut().unwrap();
        instrument.layers[0]
            .effects
            .push(daw_core::EffectInstance::drive());
        instrument
            .main
            .effects
            .push(daw_core::EffectInstance::saturator());
        compare(plan);
    }
}
