//! Native Harmonic Lift. Independent DSP from the documented transfer equations;
//! no extracted plugin implementation or coefficient arrays are used.
mod oversampling;
use daw_filter::svf::{HighShelf, StateVariableFilter as Svf};
use daw_unit_core::{
    BandMeter, EffectUnit, HarmonicLiftSettings as Settings, HarmonicQuality, LevelMonitor,
    ModulationContext, ModulationDestination, ModulationInputs, ModulationParameter as P,
    ModulationRamp as Ramp, ModulationScope, ModulationSmoother, ModulationTargetId,
    ModulationValue, PeakMeter, PortKind, ProcessingContext, RoutedDestination, UnitDescriptor,
    UnitMeter,
};
use std::sync::Arc;
const LATENCY: usize = 49;
fn finite(v: f64) -> f32 {
    if v.is_finite() {
        v.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
    } else {
        0.
    }
}
fn gain(db: f32) -> f64 {
    if db <= daw_unit_core::SILENT_GAIN_DB {
        0.
    } else {
        10_f64.powf(f64::from(db) / 20.)
    }
}
#[derive(Clone)]
struct BandControl {
    id: ModulationTargetId,
    index: usize,
    base: [Ramp; 3],
    external: ModulationSmoother<2>,
    effective: [f32; 3],
}
impl BandControl {
    fn new(c: ProcessingContext, s: &daw_unit_core::HarmonicBand, index: usize) -> Self {
        Self {
            id: s.id,
            index,
            base: [s.frequency_hz.ln(), s.gain_db, f32::from(s.enabled)].map(Ramp::new),
            external: ModulationSmoother::new(c),
            effective: [s.frequency_hz, s.gain_db, f32::from(s.enabled)],
        }
    }
    fn retarget(&mut self, s: &daw_unit_core::HarmonicBand, frames: u32) {
        for (r, v) in
            self.base
                .iter_mut()
                .zip([s.frequency_hz.ln(), s.gain_db, f32::from(s.enabled)])
        {
            r.set(v, frames);
        }
    }
}
impl ModulationDestination for BandControl {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[P::EqBandFrequency, P::EqBandGain];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        Some(self.id)
    }
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets([P::EqBandFrequency, P::EqBandGain]))
            .map(|v| v as f32);
        let range = daw_unit_core::HARMONIC_FREQUENCY_RANGES[self.index];
        self.effective = [
            (b[0].exp() * 2_f32.powf(o[0])).clamp(range[0], range[1]),
            (b[1] + o[1]).clamp(0., 24.),
            b[2],
        ];
        input
    }
}
#[derive(Clone)]
struct Controls {
    base: [Ramp; 6],
    external: ModulationSmoother<6>,
    effective: [f32; 6],
}
fn values(s: &Settings) -> [f32; 6] {
    [
        s.input_db, s.drive, s.push, s.limit_db, s.level_db, s.dry_wet,
    ]
}
impl Controls {
    fn retarget(&mut self, s: &Settings, frames: u32) {
        for (r, v) in self.base.iter_mut().zip(values(s)) {
            r.set(v, frames);
        }
    }
}
impl ModulationDestination for Controls {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &P::HARMONIC_LIFT;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets(P::HARMONIC_LIFT))
            .map(|v| v as f32);
        self.effective = [
            (b[0] + o[0]).clamp(-60., 12.),
            (b[1] + o[1]).clamp(0., 1.),
            (b[2] + o[2]).clamp(0., 1.),
            (b[3] + o[3]).clamp(-24., 0.),
            (b[4] + o[4]).clamp(daw_unit_core::SILENT_GAIN_DB, 12.),
            (b[5] + o[5]).clamp(0., 1.),
        ];
        input
    }
}
#[derive(Clone)]
struct Core {
    pre: [[Svf; 2]; 6],
    post: [[Svf; 2]; 6],
    emphasis: [HighShelf; 2],
    deemphasis: [HighShelf; 2],
    envelope: f64,
    coefficient: f64,
    rate: f64,
    reduction: f32,
    maximizer: Ramp,
    fade: u32,
}
impl Core {
    fn new(rate: u32, active: bool) -> Self {
        let fade = (rate / 200).max(1);
        let rate = f64::from(rate);
        Self {
            pre: std::array::from_fn(|_| std::array::from_fn(|_| Svf::default())),
            post: std::array::from_fn(|_| std::array::from_fn(|_| Svf::default())),
            emphasis: std::array::from_fn(|_| HighShelf::new(rate, 6300., 0.1, 3.)),
            deemphasis: std::array::from_fn(|_| HighShelf::new(rate, 6300., 0.1, -3.)),
            envelope: 0.,
            coefficient: (-std::f64::consts::TAU * 10. / rate).exp(),
            rate,
            reduction: 0.,
            maximizer: Ramp::new(f32::from(active)),
            fade,
        }
    }
    fn sample(
        &mut self,
        input: [f64; 2],
        bands: [[f32; 3]; 6],
        drive: f32,
        push: f32,
        limit: f32,
    ) -> [f64; 2] {
        let mut wet = input;
        for (i, b) in bands.iter().enumerate() {
            for ch in 0..2 {
                let v = if i == 5 {
                    self.pre[i][ch].high(input[ch], f64::from(b[0]), 0.58, self.rate)
                } else {
                    self.pre[i][ch].band(input[ch], f64::from(b[0]), 0.58, self.rate)
                };
                let shaped = if v > 0. {
                    v.tanh()
                } else {
                    v / (1. + v.abs().powf(0.4466)).powf(2.23914)
                };
                let branch =
                    self.post[i][ch].high(shaped, 10., std::f64::consts::FRAC_1_SQRT_2, self.rate);
                wet[ch] += f64::from(b[2]) * gain(b[1]) * branch;
            }
        }
        for (ch, v) in wet.iter_mut().enumerate() {
            let emphasized = self.emphasis[ch].sample(*v);
            let k = 3. * f64::from(drive);
            let shaped = (1. + k) * emphasized / (1. + k * emphasized.abs());
            let processed = self.deemphasis[ch].sample(shaped);
            if drive > 0. {
                *v = processed;
            }
        }
        let peak = wet[0].abs().max(wet[1].abs());
        self.envelope = (1. - self.coefficient) * peak + self.coefficient * self.envelope;
        let threshold = 10_f64.powf(-1.5 * f64::from(push));
        let reduction = (threshold / self.envelope.max(0.).sqrt().max(peak).max(1e-30)).min(1.);
        // Zero Push bypasses maximization. Ramp that discrete boundary as well
        // as Push itself, so hot input cannot step when reduction first engages.
        self.maximizer.set(f32::from(push > 0.0), self.fade);
        let active = f64::from(self.maximizer.next_value());
        self.reduction = (-20.0 * ((1.0 - active) + active * reduction).max(1e-30).log10()) as f32;
        let multiplier = gain(limit) * ((1.0 - active) + active * reduction / threshold);
        wet.map(|v| v * multiplier)
    }
    fn reset(&mut self, active: bool) {
        self.maximizer = Ramp::new(f32::from(active));
        for filter in self.pre.iter_mut().chain(&mut self.post).flatten() {
            filter.reset();
        }
        for f in self.emphasis.iter_mut().chain(&mut self.deemphasis) {
            f.reset();
        }
        self.envelope = 0.;
        self.reduction = 0.;
    }
}
#[derive(Clone)]
pub struct HarmonicLift {
    bands: [RoutedDestination<BandControl>; 6],
    controls: RoutedDestination<Controls>,
    core: Core,
    hq_core: Core,
    quality_blend: Ramp,
    oversampling: oversampling::Oversampling,
    quality: HarmonicQuality,
    dry: [[f32; 2]; LATENCY],
    normal: [[f64; 2]; LATENCY],
    cursor: usize,
    bypass: Ramp,
    fade: u32,
    output: LevelMonitor<3>,
    meter: UnitMeter<PeakMeter>,
    reduction: f32,
    attack: f32,
    release: f32,
}
impl HarmonicLift {
    pub const LATENCY_FRAMES: u32 = LATENCY as u32;
    pub fn new(c: ProcessingContext, s: &Settings) -> Self {
        Self {
            bands: std::array::from_fn(|i| {
                RoutedDestination::new(
                    BandControl::new(c, &s.bands[i], i),
                    &s.modulations,
                    c.sample_rate_hz,
                )
            }),
            controls: RoutedDestination::new(
                Controls {
                    base: values(s).map(Ramp::new),
                    external: ModulationSmoother::new(c),
                    effective: values(s),
                },
                &s.modulations,
                c.sample_rate_hz,
            ),
            core: Core::new(c.sample_rate_hz, s.push > 0.0),
            hq_core: Core::new(c.sample_rate_hz * 2, s.push > 0.0),
            quality_blend: Ramp::new(f32::from(s.quality == HarmonicQuality::Hq)),
            oversampling: oversampling::Oversampling::new(),
            quality: s.quality,
            dry: [[0.; 2]; LATENCY],
            normal: [[0.; 2]; LATENCY],
            cursor: 0,
            bypass: Ramp::new(f32::from(s.enabled)),
            fade: (c.sample_rate_hz / 200).max(1),
            output: LevelMonitor::new(c.sample_rate_hz, s.enabled),
            meter: UnitMeter::new(s.enabled),
            reduction: 0.,
            attack: (-1. / (0.01 * c.sample_rate_hz as f32)).exp(),
            release: (-1. / (0.3 * c.sample_rate_hz as f32)).exp(),
        }
    }
    pub fn retarget(&mut self, s: &Settings) -> bool {
        if self
            .bands
            .iter()
            .zip(&s.bands)
            .any(|(b, s)| b.destination.id != s.id)
        {
            return false;
        }
        self.quality = s.quality;
        self.quality_blend
            .set(f32::from(s.quality == HarmonicQuality::Hq), self.fade);
        for (b, s) in self.bands.iter_mut().zip(&s.bands) {
            b.destination.retarget(s, self.fade);
        }
        self.controls.destination.retarget(s, self.fade);
        self.bypass.set(f32::from(s.enabled), self.fade);
        self.output.set_enabled(s.enabled);
        self.meter.set_enabled(s.enabled);
        true
    }
    pub fn inherit(&mut self, old: &mut Self) {
        for (next, previous) in self.bands.iter_mut().zip(&mut old.bands) {
            if next.destination.id != previous.destination.id {
                continue;
            }
            next.inherit_routes(previous);
            let targets = next.destination.base.map(|v| v.target());
            std::mem::swap(&mut next.destination, &mut previous.destination);
            for (r, v) in next.destination.base.iter_mut().zip(targets) {
                r.set(v, self.fade);
            }
        }
        self.controls.inherit_routes(&mut old.controls);
        let targets = self.controls.destination.base.map(|r| r.target());
        std::mem::swap(
            &mut self.controls.destination,
            &mut old.controls.destination,
        );
        for (r, v) in self.controls.destination.base.iter_mut().zip(targets) {
            r.set(v, self.fade);
        }
        std::mem::swap(&mut self.core, &mut old.core);
        std::mem::swap(&mut self.hq_core, &mut old.hq_core);
        let quality = self.quality_blend.target();
        self.quality_blend = old.quality_blend;
        self.quality_blend.set(quality, self.fade);
        std::mem::swap(&mut self.oversampling, &mut old.oversampling);
        self.dry = old.dry;
        self.normal = old.normal;
        self.cursor = old.cursor;
        self.reduction = old.reduction;
        let target = self.bypass.target();
        self.bypass = old.bypass;
        self.bypass.set(target, self.fade);
    }
    pub fn process_at(
        &mut self,
        input: [f32; 2],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> [f32; 2] {
        let input = input.map(|v| if v.is_finite() { v } else { 0. });
        let dry = self.dry[self.cursor];
        self.dry[self.cursor] = input;
        for b in &mut self.bands {
            b.process([0.; 2], shared, context);
        }
        self.controls.process([0.; 2], shared, context);
        let bands = std::array::from_fn(|i| self.bands[i].destination.effective);
        let [input_db, drive, push, limit, level, mix] = self.controls.destination.effective;
        let driven = input.map(|v| (f64::from(v) * gain(input_db)).clamp(-1e12, 1e12));
        // Both preallocated paths stay warm for sample-timed standalone automation.
        // Fixed alignment lets a quality edit crossfade without reinitializing history.
        let normal = self.normal[self.cursor];
        self.normal[self.cursor] = self.core.sample(driven, bands, drive, push, limit);
        let frames = self
            .oversampling
            .up(driven)
            .map(|frame| self.hq_core.sample(frame, bands, drive, push, limit));
        let hq = self.oversampling.down(frames);
        let quality = f64::from(self.quality_blend.next_value());
        let wet = std::array::from_fn::<_, 2, _>(|ch| normal[ch] + (hq[ch] - normal[ch]) * quality);
        self.cursor = (self.cursor + 1) % LATENCY;
        let enabled = f64::from(self.bypass.next_value());
        let out = std::array::from_fn(|ch| {
            let d = f64::from(dry[ch]);
            let processed = (d * (1. - f64::from(mix)) + wet[ch] * f64::from(mix)) * gain(level);
            if enabled == 0. {
                dry[ch]
            } else {
                finite(d + (processed - d) * enabled)
            }
        });
        let target = if self.bypass.target() == 0. {
            0.
        } else {
            self.core.reduction
                + (self.hq_core.reduction - self.core.reduction) * self.quality_blend.target()
        };
        let c = if target > self.reduction {
            self.attack
        } else {
            self.release
        };
        self.reduction = target + (self.reduction - target) * c;
        self.meter.publish(self.reduction);
        self.output.sample_output(out);
        out
    }
    pub fn effective_parameters(&self) -> [f32; 6] {
        self.controls.destination.effective
    }
    pub fn band_parameters(&self, id: ModulationTargetId) -> Option<[f32; 2]> {
        self.bands
            .iter()
            .find(|b| b.destination.id == id)
            .map(|b| [b.destination.effective[0], b.destination.effective[1]])
    }
    pub fn set_output_peak_meter(&mut self, m: Arc<BandMeter<3>>) {
        self.output.attach_peak(m);
    }
    pub fn set_gain_reduction_peak_meter(&mut self, m: Arc<PeakMeter>) {
        self.meter.attach(m, self.reduction);
    }
}
impl EffectUnit for HarmonicLift {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::HARMONIC_LIFT_TYPE_ID,
            name: "Harmonic Lift",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn latency_frames(&self) -> u32 {
        Self::LATENCY_FRAMES
    }
    fn heap_bytes(&self) -> u64 {
        self.bands
            .iter()
            .map(RoutedDestination::heap_bytes)
            .sum::<u64>()
            + self.controls.heap_bytes()
    }
    fn reset(&mut self) {
        for b in &mut self.bands {
            for r in &mut b.destination.base {
                r.reset();
            }
            b.destination.external.reset();
            b.reset_routes();
        }
        for r in &mut self.controls.destination.base {
            r.reset();
        }
        self.controls.destination.external.reset();
        self.controls.reset_routes();
        let active = self.controls.destination.base[2].target() > 0.0;
        self.core.reset(active);
        self.hq_core.reset(active);
        self.quality_blend.reset();
        self.oversampling.reset();
        self.dry.fill([0.; 2]);
        self.normal.fill([0.; 2]);
        self.cursor = 0;
        self.bypass.reset();
        self.reduction = 0.;
        self.output.reset();
        self.meter.publish(0.);
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_at(input, &[], Default::default())
    }
}
