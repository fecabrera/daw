//! Parallel harmonic residuals over phase-matched fourth-order crossover bands.
use daw_filter::crossover::LinkwitzRileySplit as Split;
use daw_saturator::oversampling::{LATENCY, Oversampling};
use daw_unit_core::{
    EffectUnit, ExciterBand, ExciterMode, ExciterSettings, LevelMonitor, ModulationContext,
    ModulationDestination, ModulationInputs, ModulationParameter as P, ModulationRamp as Ramp,
    ModulationScope, ModulationSmoother, ModulationTargetId, ModulationValue, PortKind,
    ProcessingContext, RoutedDestination, UnitDescriptor,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn gain(db: f32) -> f32 {
    if db <= daw_unit_core::SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(db / 20.0)
    }
}
fn db(v: f32) -> f32 {
    if v == 0.0 {
        daw_unit_core::SILENT_GAIN_DB
    } else {
        20.0 * v.log10()
    }
}
fn finite(v: f64) -> f32 {
    if v.is_finite() {
        v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    } else {
        0.0
    }
}
fn clean(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}
fn weights(s: &ExciterBand) -> [f32; 4] {
    ExciterMode::ALL.map(|mode| f32::from(mode == s.mode))
}
struct Band {
    id: ModulationTargetId,
    base: [Ramp; 3],
    mode: [Ramp; 4],
    power: Ramp,
    external: ModulationSmoother<3>,
    effective: [f32; 3],
    oversampling: Oversampling,
    dry: [[f32; 2]; LATENCY],
    cursor: usize,
    tape: [f64; 2],
    dc_x: [f64; 2],
    dc_y: [f64; 2],
    tube_bias: f64,
    tube_slope: f64,
    tape_coefficient: f64,
    dc_coefficient: f64,
    solo: Option<Arc<AtomicBool>>,
    audible: Ramp,
}
impl Clone for Band {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            base: self.base,
            mode: self.mode,
            power: self.power,
            external: self.external.clone(),
            effective: self.effective,
            oversampling: self.oversampling.clone(),
            dry: self.dry,
            cursor: self.cursor,
            tape: self.tape,
            dc_x: self.dc_x,
            dc_y: self.dc_y,
            tube_bias: self.tube_bias,
            tube_slope: self.tube_slope,
            tape_coefficient: self.tape_coefficient,
            dc_coefficient: self.dc_coefficient,
            solo: None,
            audible: Ramp::new(1.0),
        }
    }
}
impl Band {
    fn new(c: ProcessingContext, s: &ExciterBand) -> Self {
        let tube_bias = 0.35_f64.tanh();
        Self {
            id: s.id,
            base: [s.drive_db, gain(s.level_db), s.mix].map(Ramp::new),
            mode: weights(s).map(Ramp::new),
            power: Ramp::new(f32::from(s.enabled)),
            external: ModulationSmoother::new(c),
            effective: [s.drive_db, s.level_db, s.mix],
            oversampling: Oversampling::new(),
            dry: [[0.0; 2]; LATENCY],
            cursor: 0,
            tape: [0.0; 2],
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            tube_bias,
            tube_slope: 1.0 - tube_bias.powi(2),
            tape_coefficient: 1.0
                - (-std::f64::consts::TAU * 8000.0 / (f64::from(c.sample_rate_hz) * 4.0)).exp(),
            dc_coefficient: (-std::f64::consts::TAU * 10.0 / (f64::from(c.sample_rate_hz) * 4.0))
                .exp(),
            solo: None,
            audible: Ramp::new(1.0),
        }
    }
    fn retarget(&mut self, s: &ExciterBand, frames: u32) {
        for (r, v) in self
            .base
            .iter_mut()
            .zip([s.drive_db, gain(s.level_db), s.mix])
        {
            r.set(v, frames);
        }
        for (r, v) in self.mode.iter_mut().zip(weights(s)) {
            r.set(v, frames);
        }
        self.power.set(f32::from(s.enabled), frames);
    }
    fn reset(&mut self) {
        for r in self.base.iter_mut().chain(&mut self.mode) {
            r.reset();
        }
        self.power.reset();
        self.audible.reset();
        self.external.reset();
        self.oversampling.reset();
        self.dry.fill([0.0; 2]);
        self.cursor = 0;
        self.tape = [0.0; 2];
        self.dc_x = [0.0; 2];
        self.dc_y = [0.0; 2];
    }
    fn audio(&mut self, input: [f32; 2]) -> ([f32; 2], [f64; 2]) {
        let dry = self.dry[self.cursor];
        self.dry[self.cursor] = input;
        self.cursor = (self.cursor + 1) % LATENCY;
        let mut w = self.mode.each_mut().map(Ramp::next_value).map(f64::from);
        let total = w.iter().sum::<f64>();
        for v in &mut w {
            *v /= total.max(1e-12);
        }
        let drive = f64::from(gain(self.effective[0]));
        let residual = self.oversampling.process(input, |up| {
            std::array::from_fn(|ch| {
                let x = up[ch] * drive;
                let mut value = 0.0;
                if w[0] > 0.0 {
                    value += w[0]
                        * (daw_saturator::shape(daw_unit_core::SaturatorShape::AnalogClip, x)
                            / drive
                            - up[ch]);
                }
                if w[1] > 0.0 {
                    // Shifted tanh is centered at zero with unity small-signal gain.
                    value += w[1]
                        * (((x + 0.35).tanh() - self.tube_bias) / self.tube_slope / drive - up[ch]);
                }
                if w[2] > 0.0 {
                    let tape = x.tanh() / drive - up[ch];
                    self.tape[ch] += self.tape_coefficient * (tape - self.tape[ch]);
                    value += w[2] * self.tape[ch];
                } else {
                    self.tape[ch] *= 1.0 - self.tape_coefficient;
                }
                if w[3] > 0.0 {
                    value += w[3]
                        * (daw_saturator::shape(daw_unit_core::SaturatorShape::HardCurve, x)
                            / drive
                            - up[ch]);
                }
                // Remove DC introduced by asymmetric shaping on the residual path.
                let blocked = value - self.dc_x[ch] + self.dc_coefficient * self.dc_y[ch];
                self.dc_x[ch] = value;
                self.dc_y[ch] = blocked;
                blocked
            })
        });
        let enabled = f64::from(self.power.next_value());
        let level = f64::from(gain(self.effective[1]));
        let mix = f64::from(self.effective[2]);
        (
            dry,
            std::array::from_fn(|ch| {
                enabled * ((level - 1.0) * f64::from(dry[ch]) + level * mix * residual[ch])
            }),
        )
    }
}
impl ModulationDestination for Band {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &P::SATURATOR;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        Some(self.id)
    }
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets(P::SATURATOR))
            .map(|v| v as f32);
        self.effective = [
            (b[0] + o[0]).clamp(-36.0, 36.0),
            if b[1] == 0.0 {
                daw_unit_core::SILENT_GAIN_DB
            } else {
                (db(b[1]) + o[1]).clamp(daw_unit_core::SILENT_GAIN_DB, 12.0)
            },
            (b[2] + o[2]).clamp(0.0, 1.0),
        ];
        input
    }
}
#[derive(Clone)]
struct Output {
    base: [Ramp; 2],
    external: ModulationSmoother<2>,
    effective: [f32; 2],
}
impl ModulationDestination for Output {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[P::Level, P::DryWet];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets([P::Level, P::DryWet]))
            .map(|v| v as f32);
        self.effective = [
            if b[0] == 0.0 {
                daw_unit_core::SILENT_GAIN_DB
            } else {
                (db(b[0]) + o[0]).clamp(daw_unit_core::SILENT_GAIN_DB, 12.0)
            },
            (b[1] + o[1]).clamp(0.0, 1.0),
        ];
        input
    }
}
#[derive(Clone)]
pub struct Exciter {
    bands: Vec<RoutedDestination<Band>>,
    output: RoutedDestination<Output>,
    // [current log Hz, target log Hz] share one clock to preserve boundary order.
    frequencies: Vec<[f32; 2]>,
    crossover_left: u32,
    splits: Vec<Split>,
    compensation: Vec<Vec<Split>>,
    dry: [[f32; 2]; LATENCY],
    cursor: usize,
    bypass: Ramp,
    fade_frames: u32,
    monitor: LevelMonitor<3>,
}
impl Exciter {
    pub const LATENCY_FRAMES: u32 = LATENCY as u32;
    pub fn new(c: ProcessingContext, s: &ExciterSettings) -> Self {
        let frequencies: Vec<_> = s
            .bands
            .iter()
            .take(s.bands.len().saturating_sub(1))
            .map(|b| b.upper_hz)
            .collect();
        Self {
            bands: s
                .bands
                .iter()
                .map(|b| RoutedDestination::new(Band::new(c, b), &s.modulations, c.sample_rate_hz))
                .collect(),
            output: RoutedDestination::new(
                Output {
                    base: [gain(s.level_db), s.dry_wet].map(Ramp::new),
                    external: ModulationSmoother::new(c),
                    effective: [s.level_db, s.dry_wet],
                },
                &s.modulations,
                c.sample_rate_hz,
            ),
            splits: frequencies
                .iter()
                .map(|f| Split::new(c.sample_rate_hz, *f))
                .collect(),
            compensation: (0..s.bands.len())
                .map(|i| {
                    frequencies
                        .iter()
                        .skip(i + 1)
                        .map(|f| Split::new(c.sample_rate_hz, *f))
                        .collect()
                })
                .collect(),
            frequencies: frequencies.iter().map(|f| [f.ln(); 2]).collect(),
            crossover_left: 0,
            dry: [[0.0; 2]; LATENCY],
            cursor: 0,
            bypass: Ramp::new(f32::from(s.enabled)),
            fade_frames: (c.sample_rate_hz / 200).max(1),
            monitor: LevelMonitor::new(c.sample_rate_hz, s.enabled),
        }
    }
    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }
    /// Live monitoring only. Offline processors never receive these handles.
    pub fn set_band_solo(&mut self, id: ModulationTargetId, solo: Arc<AtomicBool>) {
        if let Some(b) = self.bands.iter_mut().find(|b| b.destination.id == id) {
            b.destination.solo = Some(solo);
        }
    }
    pub fn retarget(&mut self, s: &ExciterSettings) -> bool {
        if self.bands.len() != s.bands.len()
            || !self
                .bands
                .iter()
                .zip(&s.bands)
                .all(|(a, b)| a.destination.id == b.id)
        {
            return false;
        }
        for (b, s) in self.bands.iter_mut().zip(&s.bands) {
            b.destination.retarget(s, self.fade_frames);
        }
        if self
            .frequencies
            .iter()
            .zip(&s.bands)
            .any(|(f, b)| f[1] != b.upper_hz.ln())
        {
            for (f, b) in self.frequencies.iter_mut().zip(&s.bands) {
                f[1] = b.upper_hz.ln();
            }
            self.crossover_left = self.fade_frames;
        }
        for (r, v) in self
            .output
            .destination
            .base
            .iter_mut()
            .zip([gain(s.level_db), s.dry_wet])
        {
            r.set(v, self.fade_frames);
        }
        self.bypass.set(f32::from(s.enabled), self.fade_frames);
        self.monitor.set_enabled(s.enabled);
        true
    }
    pub fn inherit(&mut self, previous: &mut Self) {
        for next in &mut self.bands {
            if let Some(old) = previous
                .bands
                .iter_mut()
                .find(|b| b.destination.id == next.destination.id)
            {
                next.inherit_routes(old);
                let targets = next.destination.base.map(|r| r.target());
                let mode = next.destination.mode.map(|r| r.target());
                let power = next.destination.power.target();
                // Keep the prepared live handle; retire its old owner on the worker.
                std::mem::swap(&mut next.destination.solo, &mut old.destination.solo);
                std::mem::swap(&mut next.destination, &mut old.destination);
                for (r, v) in next.destination.base.iter_mut().zip(targets) {
                    r.set(v, self.fade_frames);
                }
                for (r, v) in next.destination.mode.iter_mut().zip(mode) {
                    r.set(v, self.fade_frames);
                }
                next.destination.power.set(power, self.fade_frames);
            }
        }
        self.output.inherit_routes(&mut previous.output);
        let targets = self.output.destination.base.map(|r| r.target());
        std::mem::swap(
            &mut self.output.destination,
            &mut previous.output.destination,
        );
        for (r, v) in self.output.destination.base.iter_mut().zip(targets) {
            r.set(v, self.fade_frames);
        }
        self.dry = previous.dry;
        self.cursor = previous.cursor;
        let target = self.bypass.target();
        self.bypass = previous.bypass;
        self.bypass.set(target, self.fade_frames);
        if self.splits.len() == previous.splits.len() {
            std::mem::swap(&mut self.splits, &mut previous.splits);
            std::mem::swap(&mut self.compensation, &mut previous.compensation);
            let unchanged = self
                .frequencies
                .iter()
                .zip(&previous.frequencies)
                .all(|(a, b)| a[1] == b[1]);
            for (next, old) in self.frequencies.iter_mut().zip(&previous.frequencies) {
                next[0] = old[0];
            }
            self.crossover_left = if unchanged {
                previous.crossover_left
            } else {
                self.fade_frames
            };
        }
        let enabled = self.monitor.is_enabled();
        std::mem::swap(&mut self.monitor, &mut previous.monitor);
        self.monitor.set_enabled(enabled);
    }
    pub fn process_at(
        &mut self,
        input: [f32; 2],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> [f32; 2] {
        let input = input.map(clean);
        let dry = self.dry[self.cursor];
        self.dry[self.cursor] = input;
        self.cursor = (self.cursor + 1) % LATENCY;
        let any_solo = self.bands.iter().any(|b| {
            b.destination
                .solo
                .as_ref()
                .is_some_and(|s| s.load(Ordering::Relaxed))
        });
        for (i, f) in self.frequencies.iter_mut().enumerate() {
            if self.crossover_left > 0 {
                f[0] += (f[1] - f[0]) / self.crossover_left as f32;
            }
            let frequency = f[0].exp();
            self.splits[i].configure(frequency);
            // A lower branch receives the same smoothed boundary as the later split.
            for (row_index, row) in self.compensation.iter_mut().enumerate().take(i) {
                row[i - row_index - 1].configure(frequency);
            }
        }
        self.crossover_left = self.crossover_left.saturating_sub(1);
        // Keep crossover state finite even for extreme finite host samples.
        let mut rest = input.map(|v| v.clamp(-1e12, 1e12));
        let mut delta = [0.0_f64; 2];
        let mut solo_dry = [0.0_f64; 2];
        for (i, band) in self.bands.iter_mut().enumerate() {
            let mut sample = if let Some(split) = self.splits.get_mut(i) {
                let (low, high) = split.sample(rest);
                rest = high;
                low
            } else {
                rest
            };
            for split in &mut self.compensation[i] {
                let (low, high) = split.sample(sample);
                sample = std::array::from_fn(|ch| low[ch] + high[ch]);
            }
            band.process([0.0; 2], shared, context);
            let b = &mut band.destination;
            let solo = b.solo.as_ref().is_some_and(|s| s.load(Ordering::Relaxed));
            b.audible
                .set(f32::from(!any_solo || solo), self.fade_frames);
            let mask = f64::from(b.audible.next_value());
            let (original, residual) = b.audio(sample.map(clean));
            for ch in 0..2 {
                delta[ch] += mask * residual[ch];
                solo_dry[ch] += mask * f64::from(original[ch]);
            }
        }
        self.output.process([0.0; 2], shared, context);
        let [level, mix] = self.output.destination.effective;
        let enabled = f64::from(self.bypass.next_value());
        let out = std::array::from_fn(|ch| {
            let base = solo_dry[ch];
            let wet = (base + f64::from(mix) * delta[ch]) * f64::from(gain(level));
            if enabled == 0.0 {
                dry[ch]
            } else if enabled == 1.0 {
                finite(wet)
            } else {
                finite(f64::from(dry[ch]) * (1.0 - enabled) + wet * enabled)
            }
        });
        self.monitor.sample_output(out);
        out
    }
    pub fn band_parameters(&self, id: ModulationTargetId) -> Option<[f32; 3]> {
        self.bands
            .iter()
            .find(|b| b.destination.id == id)
            .map(|b| b.destination.effective)
    }
    pub fn effective_parameters(&self) -> [f32; 2] {
        self.output.destination.effective
    }
}
impl EffectUnit for Exciter {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::EXCITER_TYPE_ID,
            name: "Multiband Exciter",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_at(input, &[], Default::default())
    }
    fn latency_frames(&self) -> u32 {
        Self::LATENCY_FRAMES
    }
    fn reset(&mut self) {
        self.monitor.reset();
        self.dry.fill([0.0; 2]);
        self.cursor = 0;
        self.bypass.reset();
        for b in &mut self.bands {
            b.reset_routes();
            b.destination.reset();
        }
        self.output.reset_routes();
        for r in &mut self.output.destination.base {
            r.reset();
        }
        self.output.destination.external.reset();
        for f in &mut self.frequencies {
            f[0] = f[1];
        }
        self.crossover_left = 0;
        for s in &mut self.splits {
            s.reset();
        }
        for s in self.compensation.iter_mut().flatten() {
            s.reset();
        }
    }
    fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.bands)
            + self
                .bands
                .iter()
                .map(RoutedDestination::heap_bytes)
                .sum::<u64>()
            + self.output.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.frequencies)
            + daw_unit_core::memory::vec_bytes(&self.splits)
            + daw_unit_core::memory::vec_bytes(&self.compensation)
            + self
                .compensation
                .iter()
                .map(daw_unit_core::memory::vec_bytes)
                .sum::<u64>()
    }
}

#[cfg(test)]
mod tests;
