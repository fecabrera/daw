//! Serial parametric EQ bands. All allocation and topology changes are prepared by the host.
use daw_unit_core::{
    EffectUnit, EqBandMode, ModulationContext, ModulationDestination, ModulationInputs,
    ModulationParameter as P, ModulationScope, ModulationSmoother, ModulationTargetId,
    ModulationValue, MultibandEqBand, MultibandEqSettings, PortKind, ProcessingContext,
    RoutedDestination, UnitDescriptor, process_modulation_chain,
};
use fundsp::{audionode::AudioNode, svf::*};

#[derive(Clone)]
enum Filter {
    Bell(FixedSvf<f64, BellMode<f64>>),
    LowShelf(FixedSvf<f64, LowshelfMode<f64>>),
    HighShelf(FixedSvf<f64, HighshelfMode<f64>>),
    LowPass(FixedSvf<f64, LowpassMode<f64>>),
    HighPass(FixedSvf<f64, HighpassMode<f64>>),
    Notch(FixedSvf<f64, NotchMode<f64>>),
}
macro_rules! filter {
    ($s:expr, $v:ident, $body:expr) => {
        match $s {
            Filter::Bell($v) => $body,
            Filter::LowShelf($v) => $body,
            Filter::HighShelf($v) => $body,
            Filter::LowPass($v) => $body,
            Filter::HighPass($v) => $body,
            Filter::Notch($v) => $body,
        }
    };
}
impl Filter {
    fn new(mode: EqBandMode, params: &SvfParams<f64>) -> Self {
        match mode {
            EqBandMode::Bell => Self::Bell(FixedSvf::new(BellMode::new(), params)),
            EqBandMode::LowShelf => Self::LowShelf(FixedSvf::new(LowshelfMode::new(), params)),
            EqBandMode::HighShelf => Self::HighShelf(FixedSvf::new(HighshelfMode::new(), params)),
            EqBandMode::LowPass => Self::LowPass(FixedSvf::new(LowpassMode::new(), params)),
            EqBandMode::HighPass => Self::HighPass(FixedSvf::new(HighpassMode::new(), params)),
            EqBandMode::Notch => Self::Notch(FixedSvf::new(NotchMode::new(), params)),
        }
    }
    fn configure(&mut self, values: [f32; 3]) {
        filter!(
            self,
            node,
            node.set_cutoff_q_gain(
                f64::from(values[1]),
                f64::from(values[2]),
                10_f64.powf(f64::from(values[0]) / 20.0)
            )
        );
    }
    fn sample(&mut self, input: f32) -> f32 {
        filter!(self, node, node.filter_mono(input))
    }
    fn reset(&mut self) {
        filter!(self, node, node.reset());
    }
}
#[derive(Clone)]
struct Ramp<const N: usize> {
    current: [f32; N],
    target: [f32; N],
    remaining: u32,
}
impl<const N: usize> Ramp<N> {
    fn new(values: [f32; N]) -> Self {
        Self {
            current: values,
            target: values,
            remaining: 0,
        }
    }
    fn set(&mut self, values: [f32; N], frames: u32) {
        if self.target != values {
            self.target = values;
            self.remaining = frames;
        }
    }
    fn advance(&mut self) -> [f32; N] {
        if self.remaining > 0 {
            for (v, t) in self.current.iter_mut().zip(self.target) {
                *v += (t - *v) / self.remaining as f32;
            }
            self.remaining -= 1;
        }
        self.current
    }
    fn reset(&mut self) {
        self.current = self.target;
        self.remaining = 0;
    }
}
fn weights(s: &MultibandEqBand) -> [f32; 6] {
    EqBandMode::ALL.map(|m| if s.enabled && m == s.mode { 1.0 } else { 0.0 })
}
fn values(s: &MultibandEqBand) -> [f32; 3] {
    [s.gain_db, s.frequency_hz, s.q]
}
#[derive(Clone)]
struct Band {
    id: ModulationTargetId,
    filters: [[Filter; 2]; 6],
    parameters: Ramp<3>,
    weights: Ramp<6>,
    external: ModulationSmoother<3>,
    effective: [f32; 3],
    configured: [f32; 3],
    max_frequency: f32,
    monitor: daw_unit_core::LevelMonitor<3>,
}
impl Band {
    fn new(c: ProcessingContext, s: &MultibandEqBand, enabled: bool) -> Self {
        let effective = values(s);
        let params = SvfParams {
            sample_rate: f64::from(c.sample_rate_hz),
            cutoff: f64::from(s.frequency_hz),
            q: f64::from(s.q),
            gain: 10_f64.powf(f64::from(s.gain_db) / 20.0),
        };
        Self {
            id: s.id,
            filters: EqBandMode::ALL
                .map(|mode| std::array::from_fn(|_| Filter::new(mode, &params))),
            parameters: Ramp::new(effective),
            weights: Ramp::new(weights(s)),
            external: ModulationSmoother::new(c),
            effective,
            configured: effective,
            monitor: daw_unit_core::LevelMonitor::new(c.sample_rate_hz, enabled),
            max_frequency: (c.sample_rate_hz as f32 * 0.49).min(20_000.0),
        }
    }
    fn reset(&mut self) {
        self.monitor.reset();
        self.parameters.reset();
        self.weights.reset();
        self.external.reset();
        for f in self.filters.iter_mut().flatten() {
            f.reset();
        }
        self.configured = [f32::NAN; 3];
    }
    fn inherit(&mut self, previous: &mut Self, frames: u32) {
        let enabled = self.monitor.is_enabled();
        let parameters = self.parameters.target;
        let weights = self.weights.target;
        std::mem::swap(self, previous);
        self.monitor.set_enabled(enabled);
        self.parameters.set(parameters, frames);
        self.weights.set(weights, frames);
    }
}
impl ModulationDestination for Band {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &P::EQ_BAND;
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
        let base = self.parameters.advance();
        let offsets = self
            .external
            .process(m.offsets(P::EQ_BAND))
            .map(|v| v as f32);
        self.effective = [
            (base[0] + offsets[0]).clamp(-24.0, 24.0),
            (base[1] * 2_f32.powf(offsets[1])).clamp(20.0, self.max_frequency),
            (base[2] + offsets[2]).clamp(0.1, 10.0),
        ];
        if self.configured != self.effective {
            for f in self.filters.iter_mut().flatten() {
                f.configure(self.effective);
            }
            self.configured = self.effective;
        }
        let weights = self.weights.advance();
        let mut output = input;
        // Keep all modes warm. Crossfade responses, rather than interpolating unlike coefficients.
        for (filters, weight) in self.filters.iter_mut().zip(weights) {
            for ch in 0..2 {
                let wet = filters[ch].sample(input[ch]);
                output[ch] += (wet - input[ch]) * weight;
            }
        }
        self.monitor
            .sample([output[0].abs().max(output[1].abs()), 0.0, 0.0]);
        output
    }
}
#[derive(Clone)]
struct Output {
    gain: Ramp<1>,
    external: ModulationSmoother<1>,
    effective: f32,
}
fn linear(db: f32) -> f32 {
    if db <= daw_unit_core::SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(db / 20.0)
    }
}
impl ModulationDestination for Output {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[P::Level];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        let base = self.gain.advance()[0];
        let offset = self.external.process([m.offset(P::Level)])[0] as f32;
        self.effective = (base + offset).clamp(daw_unit_core::SILENT_GAIN_DB, 12.0);
        input.map(|v| v * linear(self.effective))
    }
}
#[derive(Clone)]
pub struct MultibandEq {
    bands: Vec<RoutedDestination<Band>>,
    output: RoutedDestination<Output>,
    bypass: Ramp<1>,
    fade_frames: u32,
}
impl MultibandEq {
    pub fn new(c: ProcessingContext, s: &MultibandEqSettings) -> Self {
        Self {
            bands: s
                .bands
                .iter()
                .map(|b| {
                    RoutedDestination::new(
                        Band::new(c, b, s.enabled),
                        &s.modulations,
                        c.sample_rate_hz,
                    )
                })
                .collect(),
            output: RoutedDestination::new(
                Output {
                    gain: Ramp::new([s.output_gain_db]),
                    external: ModulationSmoother::new(c),
                    effective: s.output_gain_db,
                },
                &s.modulations,
                c.sample_rate_hz,
            ),
            bypass: Ramp::new([if s.enabled { 1.0 } else { 0.0 }]),
            fade_frames: (c.sample_rate_hz / 200).max(1),
        }
    }
    /// Attach a prepared level handle to one stable band. Remaining cells are reserved.
    pub fn set_band_level_meter(
        &mut self,
        id: ModulationTargetId,
        meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>,
    ) {
        if let Some(band) = self.bands.iter_mut().find(|b| b.destination.id == id) {
            band.destination.monitor.attach(meter);
        }
    }
    /// Transfer histories into a prepared topology. Removed owners stay in the retired processor.
    pub fn inherit(&mut self, previous: &mut Self) {
        for band in &mut self.bands {
            if let Some(old) = previous
                .bands
                .iter_mut()
                .find(|old| old.destination.id == band.destination.id)
            {
                band.inherit_routes(old);
                band.destination
                    .inherit(&mut old.destination, self.fade_frames);
            }
        }
        self.output.inherit_routes(&mut previous.output);
        let gain = self.output.destination.gain.target;
        std::mem::swap(
            &mut self.output.destination,
            &mut previous.output.destination,
        );
        self.output.destination.gain.set(gain, self.fade_frames);
        let bypass = self.bypass.target;
        self.bypass = previous.bypass.clone();
        self.bypass.set(bypass, self.fade_frames);
    }
    pub fn process_at(
        &mut self,
        input: [f32; 2],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> [f32; 2] {
        let wet = process_modulation_chain(&mut self.bands, input, shared, context);
        let wet = self.output.process(wet, shared, context);
        let mix = self.bypass.advance()[0];
        std::array::from_fn(|ch| input[ch] + (wet[ch] - input[ch]) * mix)
    }
    pub fn band_parameters(&self, id: ModulationTargetId) -> Option<[f32; 3]> {
        self.bands
            .iter()
            .find(|b| b.destination.id == id)
            .map(|b| b.destination.effective)
    }
    pub fn output_gain_db(&self) -> f32 {
        self.output.destination.effective
    }
}
impl EffectUnit for MultibandEq {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::MULTIBAND_EQ_TYPE_ID,
            name: "Multiband EQ",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_at(input, &[], ModulationContext::default())
    }
    fn reset(&mut self) {
        for b in &mut self.bands {
            b.destination.reset();
            b.reset_routes();
        }
        self.output.reset_routes();
        self.output.destination.gain.reset();
        self.output.destination.external.reset();
        self.bypass.reset();
    }
    fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.bands)
            + self
                .bands
                .iter()
                .map(RoutedDestination::heap_bytes)
                .sum::<u64>()
            + self.output.heap_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{Modulation, ModulationBehavior, ModulationRange, ModulationSourceId};
    fn context(rate: u32) -> ProcessingContext {
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        }
    }
    fn response(mode: EqBandMode, rate: u32, hz: f32, gain: f32) -> f32 {
        let mut s = MultibandEqSettings::default();
        s.bands[0].mode = mode;
        s.bands[0].gain_db = gain;
        let mut eq = MultibandEq::new(context(rate), &s);
        let mut power = [0.0_f64; 2];
        for frame in 0..rate / 5 {
            let input = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.01;
            let output = eq.process_sample([input, 0.0]);
            assert_eq!(output[1], 0.0);
            assert!(output[0].is_finite());
            if frame > rate / 10 {
                power[0] += f64::from(input).powi(2);
                power[1] += f64::from(output[0]).powi(2);
            }
        }
        10.0 * (power[1] / power[0]).log10() as f32
    }
    #[test]
    fn modes_have_expected_response_at_all_project_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            assert!((response(EqBandMode::Bell, rate, 1000.0, 12.0) - 12.0).abs() < 0.1);
            assert!(response(EqBandMode::Bell, rate, 50.0, 12.0).abs() < 0.1);
            assert!((response(EqBandMode::LowShelf, rate, 50.0, -12.0) + 12.0).abs() < 0.1);
            assert!((response(EqBandMode::HighShelf, rate, 12000.0, 12.0) - 12.0).abs() < 0.15);
            assert!(response(EqBandMode::LowPass, rate, 12000.0, 24.0) < -40.0);
            assert!(response(EqBandMode::HighPass, rate, 50.0, 24.0) < -40.0);
            assert!(response(EqBandMode::Notch, rate, 1000.0, 24.0) < -60.0);
        }
    }
    #[test]
    fn empty_disabled_bypass_and_output_gain_are_independent() {
        let mut s = MultibandEqSettings::default();
        s.bands[0].enabled = false;
        s.bands[0].mode = EqBandMode::LowPass;
        let mut eq = MultibandEq::new(context(48000), &s);
        for _ in 0..1024 {
            assert_eq!(eq.process_sample([0.1, -0.2]), [0.1, -0.2]);
        }
        s.bands.clear();
        s.output_gain_db = -6.0;
        let mut next = MultibandEq::new(context(48000), &s);
        next.inherit(&mut eq);
        for _ in 0..240 {
            next.process_sample([0.1, -0.2]);
        }
        assert!((next.process_sample([0.1, -0.2])[0] - 0.1 * linear(-6.0)).abs() < 1e-6);
        s.enabled = false;
        let mut bypass = MultibandEq::new(context(48000), &s);
        bypass.inherit(&mut next);
        for _ in 0..240 {
            bypass.process_sample([0.1, -0.2]);
        }
        assert_eq!(bypass.process_sample([0.1, -0.2]), [0.1, -0.2]);
    }
    #[test]
    fn target_routes_use_identity_and_clamp_all_continuous_controls() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut s = MultibandEqSettings::default();
            s.bands.push(Default::default());
            let source = ModulationSourceId {
                layer: s.bands[0].id,
                unit: s.bands[1].id,
            };
            for (target, parameters) in [
                (Some(s.bands[0].id), P::EQ_BAND.as_slice()),
                (None, [P::Level].as_slice()),
            ] {
                for &parameter in parameters {
                    for _ in 0..2 {
                        s.modulations.push(Modulation {
                            target,
                            source: Some(source),
                            amount: parameter.limit(),
                            ..Modulation::new(parameter)
                        });
                    }
                }
            }
            let original = s.clone();
            let mut eq = MultibandEq::new(context(rate), &s);
            let values = [ModulationValue {
                source,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Bipolar,
                behavior: ModulationBehavior::Continuous,
                release_age: None,
            }];
            for _ in 0..rate / 50 {
                assert!(
                    eq.process_at([0.001, -0.001], &values, ModulationContext::default())
                        .iter()
                        .all(|v| v.is_finite())
                );
            }
            assert_eq!(
                eq.band_parameters(s.bands[0].id).unwrap(),
                [24.0, 20_000.0, 10.0]
            );
            assert_eq!(
                eq.band_parameters(s.bands[1].id).unwrap(),
                values_from(&s.bands[1])
            );
            assert_eq!(eq.output_gain_db(), 12.0);
            assert_eq!(s, original);
            s.bands.swap(0, 1);
            s.bands.push(Default::default());
            let mut next = MultibandEq::new(context(rate), &s);
            next.inherit(&mut eq);
            for _ in 0..100 {
                next.process_at([0.001, -0.001], &values, ModulationContext::default());
            }
            assert_eq!(
                next.band_parameters(s.bands[1].id).unwrap(),
                [24.0, 20_000.0, 10.0]
            );
        }
    }
    fn values_from(s: &MultibandEqBand) -> [f32; 3] {
        super::values(s)
    }
}

#[cfg(test)]
mod route_update_tests {
    use super::*;
    use daw_unit_core::{Modulation, ModulationBehavior, ModulationRange, ModulationSourceId};
    #[test]
    fn prepared_route_add_edit_and_removal_publish_new_connections_and_keep_ramps() {
        let c = ProcessingContext {
            sample_rate_hz: 48_000,
            max_block_frames: 512,
        };
        let mut settings = MultibandEqSettings::default();
        let source = ModulationSourceId {
            layer: settings.bands[0].id,
            unit: settings.bands[0].id,
        };
        let value = [ModulationValue {
            source,
            value: 1.0,
            weight: 1.0,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
            release_age: None,
        }];
        let band = settings.bands[0].id;
        let mut current = MultibandEq::new(c, &settings);
        let mut route = Modulation {
            source: Some(source),
            target: Some(band),
            amount: 6.0,
            ..Modulation::new(P::EqBandGain)
        };
        for stage in 0..4 {
            match stage {
                0 => settings.modulations = vec![route],
                1 => {
                    route.amount = -12.0;
                    settings.modulations = vec![route];
                }
                2 => settings.modulations.clear(),
                _ => {
                    route.target = None;
                    route.parameter = P::Level;
                    route.amount = -6.0;
                    settings.modulations = vec![route];
                }
            }
            let mut next = MultibandEq::new(c, &settings);
            next.inherit(&mut current);
            for _ in 0..2048 {
                next.process_at([0.01, -0.01], &value, ModulationContext::default());
            }
            let expected = match stage {
                0 => 6.0,
                1 => -12.0,
                _ => 0.0,
            };
            assert!(
                (next.band_parameters(band).unwrap()[0] - expected).abs() < 1e-5,
                "stage {stage}"
            );
            assert!((next.output_gain_db() - if stage == 3 { -6.0 } else { 0.0 }).abs() < 1e-5);
            current = next;
        }
    }
}

#[cfg(test)]
mod meter_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };
    fn read(handle: &[AtomicU32; 3]) -> f32 {
        f32::from_bits(handle[0].load(Ordering::Relaxed))
    }
    #[test]
    fn multiband_meters_keep_audio_unchanged_follow_stereo_and_clear_on_unit_bypass() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let c = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let mut settings = MultibandEqSettings::default();
            settings.bands[0].frequency_hz = 440.0;
            settings.bands[0].gain_db = 6.0;
            settings.bands.push(Default::default());
            let handles: [Arc<[AtomicU32; 3]>; 2] =
                std::array::from_fn(|_| Arc::new(std::array::from_fn(|_| AtomicU32::new(0))));
            let mut monitored = MultibandEq::new(c, &settings);
            let mut reference = monitored.clone();
            for (b, h) in settings.bands.iter().zip(&handles) {
                monitored.set_band_level_meter(b.id, h.clone());
            }
            for frame in 0..rate / 20 {
                let input =
                    (frame as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.4;
                assert_eq!(
                    monitored.process_sample([input * 0.1, input]),
                    reference.process_sample([input * 0.1, input])
                );
            }
            assert!(read(&handles[0]) > 0.6);
            assert!((read(&handles[0]) - read(&handles[1])).abs() < 1e-5);
            let peak = read(&handles[0]);
            for _ in 0..rate / 10 {
                monitored.process_sample([0.0; 2]);
            }
            assert!(read(&handles[0]) < peak);
            settings.enabled = false;
            settings.bands.swap(0, 1);
            let mut next = MultibandEq::new(c, &settings);
            next.inherit(&mut monitored);
            assert_eq!(read(&handles[0]), 0.0);
            assert_eq!(read(&handles[1]), 0.0);
            for _ in 0..1000 {
                next.process_sample([0.8; 2]);
            }
            assert_eq!(read(&handles[0]), 0.0);
            settings.enabled = true;
            let mut enabled = MultibandEq::new(c, &settings);
            enabled.inherit(&mut next);
            for _ in 0..1000 {
                enabled.process_sample([0.4, -0.4]);
            }
            assert!(read(&handles[0]) > 0.0);
            assert!(read(&handles[1]) > 0.0);
            enabled.reset();
            assert_eq!(read(&handles[0]), 0.0);
            assert_eq!(read(&handles[1]), 0.0);
        }
    }
}
