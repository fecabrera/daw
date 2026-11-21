//! Prepared external processing node. All buffers and event budgets are fixed
//! before publication. The common modulation resolver owns route evaluation.
use daw_core::HostedSettings;
use daw_plugin_host::{NativeEvent, NativeProcessStatus, NativeTransport, PreparedInstance};
use daw_unit_core::{
    HostedScalar, ModulationContext, ModulationRoute, ModulationSourceIndex, ModulationValue,
    ProcessingContext, process_modulation,
};

pub(super) type NativeBank = std::collections::HashMap<daw_core::Id, PreparedInstance>;
struct Parameter {
    scalar: HostedScalar,
    base: f64,
    previous: f64,
    range: [f32; 2],
    routes: Vec<ModulationRoute>,
}
pub(super) struct HostedNode {
    pub id: daw_core::Id,
    pub settings: HostedSettings,
    pub effect_id: Option<daw_core::Id>,
    pub native: PreparedInstance,
    parameters: Vec<Parameter>,
    events: Vec<NativeEvent>,
    /// Planar bus facade is filled from this node's delayed main and PCM inputs.
    pub inputs: Vec<Vec<[f32; 2]>>,
    output: Vec<[f32; 2]>,
    dry: super::latency::Delay,
    wet: f32,
    epoch: u64,
    held_output: [f32; 2],
    fault: bool,
    state_tag: Option<daw_plugin_host::NativeStateTag>,
    meter: daw_unit_core::LevelMonitor<3>,
}
impl HostedNode {
    /// Native latency and tail metadata are measured at the processing rate
    /// before any graph delay is allocated. Saved metadata is diagnostic only.
    pub fn prepare_instances(plan: &mut super::RenderPlan) -> daw_plugin_host::Result<NativeBank> {
        if plan.project.has_unsupported_hosted()
            && !plan.external.host.as_ref().is_some_and(|c| c.is_degraded())
        {
            return Err(daw_plugin_host::Error::Invalid(
                "Unsupported saved plugin version; strict rendering cannot substitute it".into(),
            ));
        }
        let units: Vec<_> = plan
            .project
            .hosted_units()
            .into_iter()
            .map(|(_, s)| s.clone())
            .collect();
        let mut bank = NativeBank::new();
        if units.is_empty() {
            return Ok(bank);
        }
        let factory = plan.external.host.as_ref().ok_or_else(|| {
            daw_plugin_host::Error::Invalid(
                "Native plugin services are not attached to this render plan".into(),
            )
        })?;
        let sample_rate_hz = plan.project.sample_rate_hz;
        for settings in units {
            if factory.is_missing(&settings.plugin.identity) {
                // Delayed dry effects and silent sources retain saved alignment.
                let rate = u64::from(sample_rate_hz);
                let latency = (u64::from(settings.latency_frames) * rate
                    / u64::from(settings.metadata_rate)) as u32;
                let tail =
                    settings.tail_frames.saturating_mul(rate) / u64::from(settings.metadata_rate);
                plan.project
                    .set_prepared_hosted_metadata(
                        settings.id,
                        sample_rate_hz,
                        latency,
                        tail,
                        settings.output_channels,
                    )
                    .map_err(|e| daw_plugin_host::Error::Invalid(e.to_string()))?;
                continue;
            }
            let state = settings
                .state
                .as_ref()
                .map(|s| {
                    plan.external
                        .states
                        .restore(settings.plugin.identity.format(), s)
                })
                .transpose()?;
            let native = factory.prepare_mode(
                &settings,
                sample_rate_hz,
                plan.project.buffer_size_frames,
                state,
                plan.external.offline,
            )?;
            let id = settings.id;
            plan.project
                .set_prepared_hosted_metadata(
                    id,
                    sample_rate_hz,
                    native.metadata.latency_frames,
                    native.metadata.tail_frames,
                    native.metadata.outputs[0].channels,
                )
                .map_err(|e| daw_plugin_host::Error::Invalid(e.to_string()))?;
            bank.insert(id, native);
        }
        Ok(bank)
    }
    pub fn prepare(
        settings: &HostedSettings,
        context: ProcessingContext,
        bank: &mut NativeBank,
    ) -> daw_plugin_host::Result<Self> {
        let native = bank.remove(&settings.id).ok_or_else(|| {
            daw_plugin_host::Error::Invalid(
                "Prepared plugin instance is missing or already owned".into(),
            )
        })?;
        // Worst case: one changed parameter per sample, plus the admitted notes.
        // Reduce block quantum instead of dropping events or allocating on audio.
        let parameters = settings
            .parameters
            .iter()
            .zip(&native.values)
            .filter(|(p, _)| !p.definition.read_only)
            .map(|(binding, &base)| Parameter {
                scalar: HostedScalar::new(context, binding),
                base,
                previous: f64::NAN,
                range: [f32::INFINITY, f32::NEG_INFINITY],
                routes: daw_unit_core::prepare_target_routes(
                    &settings.modulations,
                    binding.target,
                    context.sample_rate_hz,
                ),
            })
            .collect();
        Ok(Self {
            id: settings.id,
            effect_id: None,
            settings: settings.clone(),
            inputs: native
                .metadata
                .inputs
                .iter()
                .map(|_| vec![[0.0; 2]; context.max_block_frames as usize])
                .collect(),
            output: vec![[0.0; 2]; context.max_block_frames as usize],
            dry: super::latency::Delay::new(settings.id, u64::from(settings.latency_frames)),
            wet: if settings.enabled { 1.0 } else { 0.0 },
            epoch: u64::MAX,
            held_output: [0.0; 2],
            fault: false,
            state_tag: settings
                .state
                .as_ref()
                .and_then(daw_plugin_host::NativeStateTag::from_reference),
            meter: daw_unit_core::LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            native,
            parameters,
            events: Vec::with_capacity(daw_plugin_host::MAX_HOSTED_EVENTS),
        })
    }
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        // SDK-owned buffers are broker-retained with this lease. Include a
        // conservative host ABI allowance; foreign private allocations are opaque.
        self.settings.heap_bytes()
            + vec_bytes(&self.parameters)
            + self
                .parameters
                .iter()
                .map(|p| {
                    vec_bytes(&p.routes)
                        + (p.scalar.definition.name.capacity()
                            + p.scalar.definition.module.capacity())
                            as u64
                })
                .sum::<u64>()
            + vec_bytes(&self.events)
            + vec_bytes(&self.inputs)
            + self.inputs.iter().map(vec_bytes).sum::<u64>()
            + vec_bytes(&self.output)
            + self.dry.heap_bytes()
            + daw_plugin_host::MAX_HOSTED_EVENTS as u64 * 256
    }
    pub fn compatible(&self, previous: &Self) -> bool {
        !previous.fault
            && previous.native.processor.configuration_valid()
            && self.id == previous.id
            && self.settings.plugin.identity == previous.settings.plugin.identity
            && self.settings.metadata_rate == previous.settings.metadata_rate
            && self.settings.latency_frames == previous.settings.latency_frames
            && self.settings.output_channels == previous.settings.output_channels
            && self.state_tag == previous.native.processor.state_tag()
            && self.settings.audio_inputs == previous.settings.audio_inputs
            && self.settings.parameters.len() == previous.settings.parameters.len()
            && self
                .settings
                .parameters
                .iter()
                .zip(&previous.settings.parameters)
                .all(|(a, b)| a.target == b.target && a.definition == b.definition)
    }
    pub fn inherit(&mut self, previous: &mut Self) {
        std::mem::swap(&mut self.native, &mut previous.native);
        std::mem::swap(&mut self.dry, &mut previous.dry);
        self.wet = previous.wet;
        self.held_output = previous.held_output;
        self.epoch = u64::MAX;
        previous.meter.retarget(&mut self.meter);
        std::mem::swap(&mut self.meter, &mut previous.meter);
        for parameter in &mut self.parameters {
            if let Some(old) = previous
                .parameters
                .iter_mut()
                .find(|p| p.scalar.target == parameter.scalar.target)
            {
                parameter.scalar.inherit(&mut old.scalar);
                daw_unit_core::retarget_modulation_routes(&mut old.routes, &mut parameter.routes);
                std::mem::swap(&mut old.routes, &mut parameter.routes);
            }
        }
    }
    pub fn snapshot(&self, target: daw_core::Id) -> Option<super::ModulationSnapshot> {
        if !self.settings.enabled || self.fault {
            return None;
        }
        let parameter = self.parameters.iter().find(|p| p.scalar.target == target)?;
        if !parameter.range.iter().all(|v| v.is_finite()) {
            return None;
        }
        let mut snapshot = [f32::NAN; super::MODULATION_SNAPSHOT_SIZE];
        snapshot[..2].copy_from_slice(&parameter.range);
        Some(snapshot)
    }
    pub fn fault(&self) -> bool {
        self.fault
    }
    pub fn fail(&mut self) {
        self.fault = true;
    }
    pub fn attach_meter(&mut self, meter: std::sync::Arc<daw_unit_core::BandMeter<3>>) {
        self.meter.attach_peak_prepared(meter);
    }
    pub fn event_quantum(&self) -> usize {
        // Reserve half the queue for note changes and manual controls.
        (daw_plugin_host::MAX_HOSTED_EVENTS / 2 / self.parameters.len().max(1)).max(1)
    }
    pub fn begin(&mut self) {
        self.events.clear();
        for p in &mut self.parameters {
            p.range = [f32::INFINITY, f32::NEG_INFINITY];
        }
        for bus in &mut self.inputs {
            bus.fill([0.0; 2]);
        }
    }
    pub fn note(&mut self, event: NativeEvent) {
        if self.events.len() < self.events.capacity() {
            self.events.push(event);
        } else {
            self.fault = true;
        }
    }
    /// `values` is a prepared per-sample provider snapshot. The host publishes
    /// providers once; consumers resolve routes through the shared contract.
    pub fn controls<'a>(
        &mut self,
        offset: usize,
        values: &[ModulationValue],
        index: Option<&'a ModulationSourceIndex>,
        mut context: ModulationContext<'a>,
    ) {
        let epoch = self.native.processor.parameter_epoch();
        let resend = self.epoch != epoch;
        if resend {
            for p in &mut self.parameters {
                p.previous = f64::NAN;
            }
            self.epoch = epoch;
        }
        context.shared_sources = index;
        for parameter in &mut self.parameters {
            let base = self
                .native
                .processor
                .base_value(parameter.scalar.definition.id)
                .unwrap_or(parameter.base);
            let value = process_modulation(
                &mut parameter.scalar,
                &mut parameter.routes,
                base,
                &[],
                values,
                context,
            );
            let (low, high) = parameter.scalar.definition.domain.bounds();
            let normalized = if high > low {
                ((value - low) / (high - low)) as f32
            } else {
                0.0
            };
            parameter.range[0] = parameter.range[0].min(normalized);
            parameter.range[1] = parameter.range[1].max(normalized);
            let base_changed = base != parameter.base;
            parameter.base = base;
            if value == parameter.previous && !base_changed {
                continue;
            }
            parameter.previous = value;
            let event = if parameter.scalar.definition.modulatable {
                NativeEvent::Modulation {
                    offset: offset as u32,
                    id: parameter.scalar.definition.id,
                    amount: value - base,
                }
            } else {
                NativeEvent::Parameter {
                    offset: offset as u32,
                    id: parameter.scalar.definition.id,
                    value,
                }
            };
            if self.events.len() < self.events.capacity() {
                self.events.push(event);
            } else {
                self.fault = true;
            }
        }
    }
    pub fn process(&mut self, main: &mut [[f32; 2]], transport: NativeTransport) {
        // The scheduler appends in sample order. Retain equal-offset insertion
        // order so a note-off precedes its replacement note-on.
        let mut inputs: [&[[f32; 2]]; 32] = [&[]; 32];
        for (target, source) in inputs.iter_mut().zip(&self.inputs) {
            *target = &source[..main.len()];
        }
        let status = if self.fault {
            NativeProcessStatus::Fault
        } else {
            self.native.processor.process(
                &inputs[..self.inputs.len()],
                &mut self.output[..main.len()],
                &self.events,
                transport,
            )
        };
        if status == NativeProcessStatus::Fault {
            self.fault = true;
        }
        let rate = self.settings.metadata_rate;
        let target =
            if self.settings.enabled && !self.fault && status == NativeProcessStatus::Processed {
                1.0
            } else {
                0.0
            };
        for (frame, input) in main.iter_mut().enumerate() {
            let dry = self.dry.tick((*input, false)).0;
            let dry = if self.settings.output_channels == 1 {
                [(dry[0] + dry[1]) * 0.5; 2]
            } else {
                dry
            };
            self.wet += (target - self.wet).clamp(-200.0 / rate as f32, 200.0 / rate as f32);
            if status == NativeProcessStatus::Processed {
                self.held_output = self.output[frame];
            }
            *input = std::array::from_fn(|ch| {
                dry[ch] * (1.0 - self.wet) + self.held_output[ch] * self.wet
            });
            if self.settings.enabled {
                self.meter.sample_output(*input);
            }
        }
    }
    pub fn flush(&mut self) {
        self.native.processor.flush_controls();
    }
}
