//! Portable stereo filter shared by Filter and Auto-filter. No internal clock or LFO.
use daw_unit_core::{
    EffectUnit, FILTER_TYPE_ID, FilterSettings, PortKind, ProcessingContext, UnitDescriptor,
};
use fundsp::svf::SvfCoefs;
#[derive(Clone, Copy)]
struct Ramp {
    value: f64,
    target: f64,
    remaining: u32,
}
impl Ramp {
    fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn next(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.value
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}
#[derive(Default, Clone)]
struct StateVariableFilter {
    ic1: f64,
    ic2: f64,
    coefs: SvfCoefs<f64>,
    configured: Option<(f64, f64)>,
}
impl StateVariableFilter {
    fn sample(&mut self, input: f64, frequency: f64, q: f64, rate: f64, weights: [f64; 4]) -> f64 {
        if self.configured != Some((frequency, q)) {
            self.coefs = SvfCoefs::lowpass(rate, frequency, q);
            self.configured = Some((frequency, q));
        }
        // Trapezoidal state-variable integration (Simper); see FunDSP's svf.rs.
        let c = &self.coefs;
        let v3 = input - self.ic2;
        let band = c.a1 * self.ic1 + c.a2 * v3;
        let low = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * band - self.ic1;
        self.ic2 = 2.0 * low - self.ic2;
        let normalized_band = band / q;
        let high = input - normalized_band - low;
        let notch = input - normalized_band;
        [low, high, normalized_band, notch]
            .into_iter()
            .zip(weights)
            .map(|(value, weight)| value * weight)
            .sum()
    }
}
fn weights<const N: usize>(index: usize) -> [Ramp; N] {
    std::array::from_fn(|i| Ramp::new(f64::from(i == index)))
}

#[derive(Clone)]
pub struct Filter {
    monitor: daw_unit_core::LevelMonitor<3>,
    sample_rate: f64,
    fade: u32,
    filters: [StateVariableFilter; 2],
    frequency_log: Ramp,
    q: Ramp,
    level: Ramp,
    wet: Ramp,
    mix: Ramp,
    shapes: [Ramp; 4],
    external: daw_unit_core::ModulationSmoother<4>,
    effective: [f32; 5],
}
impl Filter {
    pub fn new(context: ProcessingContext, settings: &FilterSettings) -> Self {
        Self {
            sample_rate: f64::from(context.sample_rate_hz),
            monitor: daw_unit_core::LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            fade: (context.sample_rate_hz / 200).max(1),
            filters: std::array::from_fn(|_| StateVariableFilter::default()),
            frequency_log: Ramp::new(f64::from(settings.frequency_hz).log2()),
            q: Ramp::new(settings.q.into()),
            level: Ramp::new(10_f64.powf(f64::from(settings.level_db) / 20.0)),
            wet: Ramp::new(settings.dry_wet.into()),
            mix: Ramp::new(f64::from(settings.enabled)),
            shapes: weights(settings.shape.index()),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: [
                settings.frequency_hz,
                settings.frequency_hz,
                settings.q,
                10.0_f32.powf(settings.level_db / 20.0),
                settings.dry_wet,
            ],
        }
    }
    /// Attach a prepared current-level handle on the control thread.
    pub fn set_meter(&mut self, meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }
    /// Attach current output and held-peak storage on the control thread.
    pub fn set_output_peak_meter(&mut self, meter: std::sync::Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }
    /// Last processed cutoff L/R (Hz), Q, output dB and wet fraction.
    pub fn effective_parameters(&self) -> [f32; 5] {
        let mut values = self.effective;
        values[3] = 20.0 * values[3].log10();
        values
    }
    /// Preserve integrators and ramps during compatible live publication.
    pub fn retarget(&mut self, settings: &FilterSettings) {
        self.monitor.set_enabled(settings.enabled);
        self.frequency_log
            .retarget(f64::from(settings.frequency_hz).log2(), self.fade);
        self.q.retarget(settings.q.into(), self.fade);
        self.level
            .retarget(10_f64.powf(f64::from(settings.level_db) / 20.0), self.fade);
        self.wet.retarget(settings.dry_wet.into(), self.fade);
        self.mix.retarget(f64::from(settings.enabled), self.fade);
        for (i, ramp) in self.shapes.iter_mut().enumerate() {
            ramp.retarget(f64::from(i == settings.shape.index()), self.fade);
        }
    }
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 4]) -> [f32; 2] {
        self.process_with_cutoff_offsets(input, offsets, [0.0; 2])
    }
    /// Composition entry point for Auto-filter's already smoothed bipolar sweep.
    /// Cutoff offsets are additive octaves per channel; external routes stay shared.
    pub fn process_with_cutoff_offsets(
        &mut self,
        input: [f32; 2],
        offsets: [f32; 4],
        cutoff_offsets: [f64; 2],
    ) -> [f32; 2] {
        let offsets = self.external.process(offsets);
        let frequency = self.frequency_log.next();
        let q = (self.q.next() + offsets[1]).clamp(0.1, 10.0);
        let shapes = std::array::from_fn(|i| self.shapes[i].next());
        let level = self.level.next();
        let level = if offsets[2] == 0.0 {
            level
        } else {
            (level * 10.0_f64.powf(offsets[2].clamp(-96.0, 96.0) / 20.0))
                .min(10.0_f64.powf(12.0 / 20.0))
        };
        let wet = (self.wet.next() + offsets[3]).clamp(0.0, 1.0);
        let mix = self.mix.next();
        self.effective[2..].copy_from_slice(&[q as f32, level as f32, wet as f32]);
        let mut output = [0.0; 2];
        for ch in 0..2 {
            let cutoff = 2_f64
                .powf(frequency + cutoff_offsets[ch] + offsets[0])
                .clamp(20.0, (self.sample_rate * 0.45).min(20_000.0));
            self.effective[ch] = cutoff as f32;
            let dry = f64::from(input[ch]);
            let filtered = self.filters[ch].sample(dry, cutoff, q, self.sample_rate, shapes);
            let processed = (dry + (filtered - dry) * wet) * level;
            output[ch] = (dry + (processed - dry) * mix) as f32;
        }
        self.monitor.sample_output(output);
        output
    }
}
impl EffectUnit for Filter {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: FILTER_TYPE_ID,
            name: "Filter",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.monitor.reset();
        for ramp in [
            &mut self.frequency_log,
            &mut self.q,
            &mut self.level,
            &mut self.wet,
            &mut self.mix,
        ] {
            ramp.reset();
        }
        for ramp in &mut self.shapes {
            ramp.reset();
        }
        self.filters = std::array::from_fn(|_| StateVariableFilter::default());
        self.external.reset();
        self.effective = [
            2.0_f64.powf(self.frequency_log.value) as f32,
            2.0_f64.powf(self.frequency_log.value) as f32,
            self.q.value as f32,
            self.level.value as f32,
            self.wet.value as f32,
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 4])
    }
}
impl daw_unit_core::ModulationDestination for Filter {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [daw_unit_core::ModulationParameter] =
        <FilterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: daw_unit_core::ModulationScope =
        <FilterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &daw_unit_core::ModulationInputs,
        _: daw_unit_core::ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(daw_unit_core::ModulationParameter::FILTER),
        )
    }
}
