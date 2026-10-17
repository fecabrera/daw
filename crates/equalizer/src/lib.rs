//! Reusable stereo three-band EQ. FunDSP coefficients and filter state stay private.
use daw_unit_core::{
    EQ_TYPE_ID, EffectUnit, EqBandSettings, EqSettings, PortKind, ProcessingContext, UnitDescriptor,
};
use fundsp::{audionode::AudioNode, svf::*};

mod modulation;
#[cfg(test)]
mod modulation_tests;
mod monitor;

#[derive(Clone)]
enum Filter {
    LowShelf(FixedSvf<f64, LowshelfMode<f64>>),
    HighShelf(FixedSvf<f64, HighshelfMode<f64>>),
    HighPass(FixedSvf<f64, HighpassMode<f64>>),
    LowPass(FixedSvf<f64, LowpassMode<f64>>),
}
macro_rules! with_filter {
    ($filter:expr, $node:ident, $body:expr) => {
        match $filter {
            Filter::LowShelf($node) => $body,
            Filter::HighShelf($node) => $body,
            Filter::HighPass($node) => $body,
            Filter::LowPass($node) => $body,
        }
    };
}
impl Filter {
    fn new(index: usize, cut: bool, params: &SvfParams<f64>) -> Self {
        match (index, cut) {
            (0, false) => Self::LowShelf(FixedSvf::new(LowshelfMode::new(), params)),
            (_, false) => Self::HighShelf(FixedSvf::new(HighshelfMode::new(), params)),
            (0, true) => Self::HighPass(FixedSvf::new(HighpassMode::new(), params)),
            (_, true) => Self::LowPass(FixedSvf::new(LowpassMode::new(), params)),
        }
    }
    fn configure(&mut self, values: [f32; 4]) {
        with_filter!(
            self,
            node,
            node.set_cutoff_q_gain(
                f64::from(values[0]),
                f64::from(values[1]),
                10_f64.powf(f64::from(values[2]) / 20.0)
            )
        );
    }
    fn sample(&mut self, input: f32) -> f32 {
        with_filter!(self, node, node.filter_mono(input))
    }
    fn reset(&mut self) {
        with_filter!(self, node, node.reset());
    }
}
struct Band {
    normal: Option<[Filter; 2]>,
    cut: Option<[Filter; 2]>,
    current: [f32; 4],
    target: [f32; 4],
    remaining: u32,
    configured: [f32; 4],
}
fn values(settings: EqBandSettings) -> [f32; 4] {
    [
        settings.frequency_hz,
        settings.q,
        settings.gain_db,
        if settings.enabled { 1.0 } else { 0.0 },
    ]
}
impl Band {
    fn new(index: usize, sample_rate_hz: u32, settings: EqBandSettings) -> Self {
        let current = values(settings);
        let params = SvfParams {
            sample_rate: f64::from(sample_rate_hz),
            cutoff: f64::from(current[0]),
            q: f64::from(current[1]),
            gain: 10_f64.powf(f64::from(current[2]) / 20.0),
        };
        Self {
            normal: (index != 1)
                .then(|| std::array::from_fn(|_| Filter::new(index, false, &params))),
            cut: (index != 1).then(|| std::array::from_fn(|_| Filter::new(index, true, &params))),
            current,
            target: current,
            remaining: 0,
            configured: current,
        }
    }
    fn retarget(&mut self, settings: EqBandSettings, frames: u32) {
        let mut target = values(settings);
        if self.normal.is_none() {
            // Mid owns gain/power only. Legacy frequency/Q changes do not restart its ramp.
            target[..2].copy_from_slice(&self.target[..2]);
        }
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn advance(&mut self) {
        if self.remaining > 0 {
            for (current, target) in self.current.iter_mut().zip(self.target) {
                *current += (target - *current) / self.remaining as f32;
            }
            self.remaining -= 1;
        }
    }
    fn configure(&mut self, values: [f32; 4]) {
        if self.configured[..3] != values[..3] {
            for filter in self
                .normal
                .iter_mut()
                .flatten()
                .chain(self.cut.iter_mut().flatten())
            {
                filter.configure(values);
            }
            self.configured = values;
        }
    }
    fn sample(&mut self, input: [f32; 2], external: Option<([f32; 2], [f32; 2])>) -> [f32; 2] {
        std::array::from_fn(|ch| {
            let normal = self.normal.as_mut().map_or_else(
                || external.map_or(input, |outputs| outputs.0)[ch],
                |normal| normal[ch].sample(input[ch]),
            );
            let cut = self.cut.as_mut().map_or_else(
                || external.map_or(input, |outputs| outputs.1)[ch],
                |cut| cut[ch].sample(input[ch]),
            );
            cut + (normal - cut) * self.current[3]
        })
    }
    fn reset(&mut self) {
        self.current = self.target;
        self.remaining = 0;
        for filter in self
            .normal
            .iter_mut()
            .flatten()
            .chain(self.cut.iter_mut().flatten())
        {
            filter.reset();
            filter.configure(self.current);
        }
        self.configured = self.current;
    }
}

/// Parallel low/high-pass branches reject the interval between the shelf frequencies.
/// Q maps continuously from one to eight biquad stages; all stages keep advancing.
struct MidCut {
    low: [[Filter; 2]; 8],
    high: [[Filter; 2]; 8],
    parameters: [f32; 4],
    order: [f32; 2],
    width: f32,
}
impl MidCut {
    fn new(sample_rate_hz: u32, settings: &EqSettings) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(sample_rate_hz),
            cutoff: 1000.0,
            q: f64::from(std::f32::consts::FRAC_1_SQRT_2),
            gain: 1.0,
        };
        let mut cut = Self {
            low: std::array::from_fn(|_| {
                std::array::from_fn(|_| Filter::LowPass(FixedSvf::new(LowpassMode::new(), &params)))
            }),
            high: std::array::from_fn(|_| {
                std::array::from_fn(|_| {
                    Filter::HighPass(FixedSvf::new(HighpassMode::new(), &params))
                })
            }),
            parameters: [0.0; 4],
            order: [1.0; 2],
            width: 1.0,
        };
        cut.configure(
            settings.bands[0].frequency_hz,
            settings.bands[2].frequency_hz,
            settings.bands[0].q,
            settings.bands[2].q,
        );
        cut
    }
    fn configure(&mut self, low: f32, high: f32, low_q: f32, high_q: f32) {
        let parameters = [low, high, low_q, high_q];
        if parameters == self.parameters {
            return;
        }
        self.parameters = parameters;
        let (low, high, qs) = if low <= high {
            (low, high, [low_q, high_q])
        } else {
            (high, low, [high_q, low_q])
        };
        self.order = qs.map(|q| 1.0 + 7.0 * ((q / 0.1).log10() / 2.0).clamp(0.0, 1.0));
        // Equal boundaries mean no middle interval. Blend to dry as that interval closes.
        self.width = ((high - low) / ((low + high) * 0.5) * 10.0).clamp(0.0, 1.0);
        for (branch, (filters, cutoff)) in [(&mut self.low, low), (&mut self.high, high)]
            .into_iter()
            .enumerate()
        {
            let order = self.order[branch];
            for (stage, filters) in filters.iter_mut().enumerate() {
                let lower = order.floor() as usize;
                let upper = (lower + 1).min(8);
                let stage_q = |stages: usize| {
                    let stages = if stage < stages { stages } else { 8 };
                    1.0 / (2.0
                        * ((2 * stage + 1) as f32 * std::f32::consts::PI / (4.0 * stages as f32))
                            .cos())
                };
                let q = if stage < lower {
                    stage_q(lower)
                } else {
                    stage_q(upper)
                };
                let q = q + (stage_q(upper) - q) * order.fract();
                for filter in filters {
                    filter.configure([cutoff, q, 0.0, 0.0]);
                }
            }
        }
    }
    fn sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|ch| {
            let mut output = 0.0;
            for (branch, filters) in [&mut self.low, &mut self.high].into_iter().enumerate() {
                let order = self.order[branch];
                let lower = order.floor() as usize - 1;
                let upper = order.ceil() as usize - 1;
                let mut value = input[ch];
                let mut first = 0.0;
                let mut last = 0.0;
                for (stage, filters) in filters.iter_mut().enumerate() {
                    value = filters[ch].sample(value);
                    if stage == lower {
                        first = value;
                    }
                    if stage == upper {
                        last = value;
                    }
                }
                output += first + (last - first) * order.fract();
            }
            input[ch] + (output - input[ch]) * self.width
        })
    }
    fn reset(&mut self) {
        for filter in self.low.iter_mut().chain(&mut self.high).flatten() {
            filter.reset();
        }
    }
}

/// Opposing shelves keep the outer ranges at unity and set the middle plateau's gain.
struct MidShelf {
    low: [Filter; 2],
    high: [Filter; 2],
    parameters: [f32; 5],
    gain: f32,
}
impl MidShelf {
    fn new(rate: u32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(rate),
            cutoff: 1000.0,
            q: f64::from(std::f32::consts::FRAC_1_SQRT_2),
            gain: 1.0,
        };
        Self {
            low: std::array::from_fn(|_| Filter::new(0, false, &params)),
            high: std::array::from_fn(|_| Filter::new(2, false, &params)),
            parameters: [f32::NAN; 5],
            gain: 1.0,
        }
    }
    fn configure(&mut self, low: [f32; 4], high: [f32; 4], gain: f32) {
        let parameters = [low[0], low[1], high[0], high[1], gain];
        if self.parameters == parameters {
            return;
        }
        self.parameters = parameters;
        let (low, high) = if low[0] <= high[0] {
            (low, high)
        } else {
            (high, low)
        };
        // No interval at equal boundaries; approach unity as the interval closes.
        let width = ((high[0] - low[0]) / ((high[0] + low[0]) * 0.5) * 10.0).clamp(0.0, 1.0);
        let gain = gain * width;
        self.gain = 10_f32.powf(gain / 20.0);
        for filter in &mut self.low {
            filter.configure([low[0], low[1], -gain, 1.0]);
        }
        for filter in &mut self.high {
            filter.configure([high[0], high[1], -gain, 1.0]);
        }
    }
    fn sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|ch| self.high[ch].sample(self.low[ch].sample(input[ch])) * self.gain)
    }
    fn reset(&mut self) {
        for filter in self.low.iter_mut().chain(&mut self.high) {
            filter.reset();
        }
    }
}

pub struct Equalizer {
    bands: [Band; 3],
    mid_cut: MidCut,
    mid_shelf: MidShelf,
    mix: f32,
    target: f32,
    remaining: u32,
    fade_frames: u32,
    monitor: monitor::Monitor,
    external: daw_unit_core::ModulationSmoother<7>,
    effective: [[f32; 4]; 3],
}
impl Equalizer {
    /// Prepare on a control thread with valid settings and a supported processing rate.
    pub fn new(context: ProcessingContext, settings: &EqSettings) -> Self {
        let mix = if settings.enabled { 1.0 } else { 0.0 };
        Self {
            bands: std::array::from_fn(|index| {
                Band::new(index, context.sample_rate_hz, settings.bands[index])
            }),
            mid_cut: MidCut::new(context.sample_rate_hz, settings),
            mid_shelf: MidShelf::new(context.sample_rate_hz),
            mix,
            target: mix,
            remaining: 0,
            fade_frames: (context.sample_rate_hz / 200).max(1),
            monitor: monitor::Monitor::new(context.sample_rate_hz),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: settings.bands.map(values),
        }
    }
    /// Retarget without allocating or resetting filter history. Display-only changes are no-ops.
    pub fn retarget(&mut self, settings: &EqSettings) {
        for (band, settings) in self.bands.iter_mut().zip(settings.bands) {
            band.retarget(settings, self.fade_frames);
        }
        let target = if settings.enabled { 1.0 } else { 0.0 };
        if target != self.target {
            self.target = target;
            self.remaining = self.fade_frames;
        }
    }
    /// Attach prepared stereo-peak monitors in Low/Mid/High order on the control thread.
    /// Values are linear amplitudes of the filtered unit output, with a 300 ms release.
    pub fn set_band_level_meter(
        &mut self,
        meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>,
    ) {
        self.monitor.attach(meter);
    }
}
impl EffectUnit for Equalizer {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: EQ_TYPE_ID,
            name: "3-band EQ",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
        self.mid_cut.reset();
        self.mid_shelf.reset();
        self.mix = self.target;
        self.remaining = 0;
        self.monitor.reset();
        self.external.reset();
        self.effective = std::array::from_fn(|i| self.bands[i].current);
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 7])
    }
}
impl Equalizer {
    fn process_effective_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        for (band, effective) in self.bands.iter_mut().zip(self.effective) {
            band.configure(effective);
        }
        self.mid_cut.configure(
            self.effective[0][0],
            self.effective[2][0],
            self.effective[0][1],
            self.effective[2][1],
        );
        self.mid_shelf
            .configure(self.effective[0], self.effective[2], self.effective[1][2]);
        let mut wet = self.bands[0].sample(input, None);
        let cut = self.mid_cut.sample(wet);
        let normal = self.mid_shelf.sample(wet);
        wet = self.bands[1].sample(wet, Some((normal, cut)));
        wet = self.bands[2].sample(wet, None);
        // Finite cut slopes overlap. No enabled band means silence, including that leakage.
        // Use the existing band ramps so switching the last band does not add a click.
        let audible = 1.0
            - self
                .bands
                .iter()
                .map(|band| 1.0 - band.current[3])
                .product::<f32>();
        for sample in &mut wet {
            *sample *= audible;
        }
        if self.remaining > 0 {
            self.mix += (self.target - self.mix) / self.remaining as f32;
            self.remaining -= 1;
        }
        let output = std::array::from_fn(|ch| input[ch] + (wet[ch] - input[ch]) * self.mix);
        self.monitor
            .sample(output, self.effective[0][0], self.effective[2][0]);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read_meter(meter: &[std::sync::atomic::AtomicU32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| f32::from_bits(meter[i].load(std::sync::atomic::Ordering::Relaxed)))
    }

    #[test]
    fn band_levels_follow_output_frequency_gain_and_stereo_at_every_rate() {
        use std::sync::{Arc, atomic::AtomicU32};
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            for (band, hz) in [(0, 40.0), (1, 800.0), (2, 12000.0)] {
                let settings = EqSettings::default();
                let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
                let mut eq = Equalizer::new(context, &settings);
                let mut reference = Equalizer::new(context, &settings);
                eq.set_band_level_meter(meter.clone());
                for frame in 0..rate / 10 {
                    let sample = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin();
                    // The right channel is louder; monitoring must inspect both channels.
                    let input = [sample * 0.1, sample * 0.4];
                    assert_eq!(eq.process_sample(input), reference.process_sample(input));
                }
                let levels = read_meter(&meter);
                assert!(
                    (0.32..0.45).contains(&levels[band]),
                    "rate={rate} band={band}: {levels:?}"
                );
                for (other, level) in levels.into_iter().enumerate() {
                    if other != band {
                        assert!(level < levels[band] * 0.25, "{levels:?}");
                    }
                }
                let before = levels[band];
                let mut boosted = settings.clone();
                boosted.bands[band].gain_db = 12.0;
                eq.retarget(&boosted);
                for frame in 0..rate / 10 {
                    let sample =
                        (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.4;
                    eq.process_sample([sample, 0.0]);
                }
                assert!(read_meter(&meter)[band] > before * 3.0);
                // Bypass shows the actual dry output; all band cuts show silence after decay.
                boosted.enabled = false;
                eq.retarget(&boosted);
                eq.reset();
                for frame in 0..rate / 10 {
                    let sample =
                        (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.4;
                    eq.process_sample([sample; 2]);
                }
                assert!(read_meter(&meter)[band] < before * 1.2);
                boosted.enabled = true;
                for band in &mut boosted.bands {
                    band.enabled = false;
                }
                eq.retarget(&boosted);
                eq.reset();
                for _ in 0..rate / 100 {
                    eq.process_sample([0.4; 2]);
                }
                assert!(read_meter(&meter).iter().all(|v| *v < 0.001));
                eq.reset();
                assert_eq!(read_meter(&meter), [0.0; 3]);
            }
        }
    }

    #[test]
    fn band_monitor_follows_ordered_frequency_boundaries_and_releases_on_silence() {
        use std::sync::{Arc, atomic::AtomicU32};
        let mut monitor = monitor::Monitor::new(48000);
        let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        monitor.attach(meter.clone());
        let run = |monitor: &mut monitor::Monitor, low, high| {
            for frame in 0..24000 {
                let sample = (frame as f32 * 800.0 * std::f32::consts::TAU / 48000.0).sin() * 0.4;
                monitor.sample([sample; 2], low, high);
            }
        };
        run(&mut monitor, 250.0, 2500.0);
        assert!(read_meter(&meter)[1] > 0.35);
        monitor.reset();
        run(&mut monitor, 2500.0, 250.0);
        assert!(read_meter(&meter)[1] > 0.35);
        monitor.reset();
        run(&mut monitor, 2000.0, 10000.0);
        let moved = read_meter(&meter);
        assert!(moved[0] > 0.35 && moved[1] < moved[0] * 0.2, "{moved:?}");
        monitor.reset();
        run(&mut monitor, 1000.0, 1000.0);
        assert_eq!(read_meter(&meter)[1], 0.0);
        let before = read_meter(&meter)[0];
        for _ in 0..48000 {
            monitor.sample([0.0; 2], 1000.0, 1000.0);
        }
        assert!(read_meter(&meter)[0] < before * 0.05);
        monitor.sample([f32::NAN, f32::INFINITY], 1000.0, 1000.0);
        assert!(read_meter(&meter).iter().all(|v| v.is_finite()));
    }

    fn response(settings: &EqSettings, rate: u32, hz: f32) -> f32 {
        let mut eq = Equalizer::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            },
            settings,
        );
        let mut input_power = 0.0_f64;
        let mut output_power = 0.0_f64;
        for frame in 0..rate {
            let input = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.1;
            let output = eq.process_sample([input, 0.0]);
            assert!(output[0].is_finite());
            assert_eq!(output[1], 0.0, "stereo channels share no filter state");
            if frame > rate / 2 {
                input_power += f64::from(input).powi(2);
                output_power += f64::from(output[0]).powi(2);
            }
        }
        10.0 * (output_power / input_power).log10() as f32
    }
    #[test]
    fn shelves_have_the_requested_gain_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for (index, hz) in [(0, 20.0), (1, 1000.0), (2, 15000.0)] {
                for gain in [-12.0, 12.0] {
                    let mut settings = EqSettings::default();
                    settings.bands[index].gain_db = gain;
                    if index == 0 {
                        settings.bands[index].frequency_hz = 1000.0;
                    }
                    if index == 2 {
                        settings.bands[index].frequency_hz = 1000.0;
                    }
                    if index == 1 {
                        settings.bands[0].frequency_hz = 100.0;
                        settings.bands[2].frequency_hz = 10000.0;
                    }
                    let actual = response(&settings, rate, hz);
                    assert!(
                        (actual - gain).abs() < 0.2,
                        "band={index} rate={rate} gain={gain} actual={actual}"
                    );
                }
            }
        }
    }
    #[test]
    fn middle_shelf_uses_outer_boundaries_and_ignores_legacy_mid_controls() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].gain_db = 12.0;
            let middle = response(&settings, rate, 800.0);
            assert!(middle > 10.0, "middle plateau is boosted: {middle}");
            for hz in [20.0, 18000.0] {
                assert!(response(&settings, rate, hz).abs() < 0.3);
            }
            settings.bands[1].frequency_hz = 18000.0;
            settings.bands[1].q = 10.0;
            assert_eq!(response(&settings, rate, 800.0), middle);
            settings.bands[0].frequency_hz = 3000.0;
            settings.bands[2].frequency_hz = 12000.0;
            assert!(response(&settings, rate, 800.0).abs() < 1.0);
            assert!(response(&settings, rate, 6000.0) > 10.0);
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            assert!(response(&settings, rate, 800.0).abs() < 0.001);
        }
    }
    #[test]
    fn band_off_cuts_the_range_and_retains_gain_frequency_and_q() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for (index, cut_hz, pass_hz) in [(0, 20.0, 10000.0), (2, 15000.0, 100.0)] {
                let mut settings = EqSettings::default();
                settings.bands[index].frequency_hz = 1000.0;
                settings.bands[index].gain_db = 12.0;
                settings.bands[index].enabled = false;
                let original = settings.clone();
                assert!(
                    response(&settings, rate, cut_hz) < -35.0,
                    "band={index}, rate={rate}"
                );
                assert!(response(&settings, rate, pass_hz).abs() < 0.2);
                assert_eq!(settings, original);
                settings.bands[index].frequency_hz = if index == 2 { 15000.0 } else { 20.0 };
                assert!(
                    response(&settings, rate, cut_hz) > -4.0,
                    "cut follows frequency"
                );
            }
        }
    }
    #[test]
    fn middle_cut_follows_shelf_boundaries_and_outer_q_controls_steepness() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].enabled = false;
            for hz in [400.0, 800.0, 1600.0] {
                assert!(
                    response(&settings, rate, hz) < -20.0,
                    "cuts the full middle range at {hz} Hz"
                );
            }
            for hz in [40.0, 10000.0] {
                assert!(
                    response(&settings, rate, hz).abs() < 0.5,
                    "retains outer range at {hz} Hz"
                );
            }
            let original = response(&settings, rate, 800.0);
            settings.bands[1].frequency_hz = 15000.0;
            settings.bands[1].gain_db = 24.0;
            settings.bands[1].q = 10.0;
            assert_eq!(
                response(&settings, rate, 800.0),
                original,
                "legacy Mid frequency/Q and disabled gain do not affect the cut"
            );
            settings.bands[0].frequency_hz = 2000.0;
            settings.bands[2].frequency_hz = 5000.0;
            assert!(
                response(&settings, rate, 800.0).abs() < 0.5,
                "moving boundaries restores the old middle range"
            );
            settings.bands[0].frequency_hz = 5000.0;
            settings.bands[2].frequency_hz = 2000.0;
            assert!(
                response(&settings, rate, 3200.0) < -20.0,
                "crossed controls still cut between their values"
            );
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            assert!(
                response(&settings, rate, 800.0).abs() < 0.001,
                "equal boundaries leave no middle range"
            );
        }
        let mut gentle = EqSettings::default();
        gentle.bands[1].enabled = false;
        gentle.bands[0].q = 0.1;
        gentle.bands[2].q = 0.1;
        let mut steep = gentle.clone();
        steep.bands[0].q = 10.0;
        steep.bands[2].q = 10.0;
        assert!(response(&steep, 48000, 400.0) < response(&gentle, 48000, 400.0) - 25.0);
    }

    #[test]
    fn neutral_bypass_reset_and_parameter_transitions_are_finite() {
        let mut settings = EqSettings::default();
        let mut eq = Equalizer::new(
            ProcessingContext {
                sample_rate_hz: 48000,
                max_block_frames: 32,
            },
            &settings,
        );
        for frame in 0..1000 {
            let input = [(frame as f32 * 0.1).sin(), (frame as f32 * 0.07).cos()];
            let output = eq.process_sample(input);
            for ch in 0..2 {
                assert!((output[ch] - input[ch]).abs() < 1e-6);
            }
        }
        settings.bands[0].gain_db = 24.0;
        settings.bands[1].enabled = false;
        eq.retarget(&settings);
        let first = eq.process_sample([0.1, -0.1]);
        assert!((first[0] - 0.1).abs() < 0.05, "5 ms ramp starts near dry");
        for _ in 0..240 {
            assert!(eq.process_sample([0.1, -0.1]).iter().all(|v| v.is_finite()));
        }
        settings.enabled = false;
        eq.retarget(&settings);
        for _ in 0..240 {
            eq.process_sample([0.1, -0.1]);
        }
        assert_eq!(eq.process_sample([0.1, -0.1]), [0.1, -0.1]);
        settings.enabled = true;
        eq.retarget(&settings);
        eq.reset();
        assert_eq!(eq.process_sample([0.0; 2]), [0.0; 2]);
        for index in 0..3 {
            settings.bands[index].frequency_hz = 20000.0;
            settings.bands[index].q = 10.0;
        }
        eq.retarget(&settings);
        for _ in 0..10000 {
            assert!(
                eq.process_sample([0.01, -0.01])
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }

    #[test]
    fn all_bands_off_are_silent_and_last_band_switches_ramp() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            for band in &mut settings.bands {
                band.enabled = false;
                band.gain_db = 24.0;
                band.q = 10.0;
            }
            // Equal and crossed boundaries must also remain silent.
            for boundaries in [(250.0, 2500.0), (1000.0, 1000.0), (5000.0, 200.0)] {
                settings.bands[0].frequency_hz = boundaries.0;
                settings.bands[2].frequency_hz = boundaries.1;
                let mut eq = Equalizer::new(
                    ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 512,
                    },
                    &settings,
                );
                for frame in 0..rate / 10 {
                    let input = [(frame as f32 * 0.13).sin(), (frame as f32 * 0.07).cos()];
                    assert_eq!(eq.process_sample(input), [0.0; 2]);
                }
                settings.enabled = false;
                eq.retarget(&settings);
                for _ in 0..rate / 200 {
                    eq.process_sample([0.1, -0.1]);
                }
                assert_eq!(eq.process_sample([0.1, -0.1]), [0.1, -0.1]);
                settings.enabled = true;
            }

            // Equal boundaries and low frequencies isolate the last Low band's gain ramp.
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            for band in &mut settings.bands {
                band.gain_db = 0.0;
                band.q = std::f32::consts::FRAC_1_SQRT_2;
            }
            settings.bands[0].enabled = true;
            let mut eq = Equalizer::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            for _ in 0..rate / 10 {
                eq.process_sample([0.1, -0.1]);
            }
            let before = eq.process_sample([0.1, -0.1]);
            assert!(before[0] > 0.09);
            settings.bands[0].enabled = false;
            eq.retarget(&settings);
            let first = eq.process_sample([0.1, -0.1]);
            assert!((first[0] - before[0]).abs() < 0.002);
            for _ in 1..rate / 200 {
                eq.process_sample([0.1, -0.1]);
            }
            assert_eq!(eq.process_sample([0.1, -0.1]), [0.0; 2]);
            settings.bands[0].enabled = true;
            eq.retarget(&settings);
            assert!(eq.process_sample([0.1, -0.1])[0].abs() < 0.002);
            for _ in 0..rate / 10 {
                eq.process_sample([0.1, -0.1]);
            }
            assert!(eq.process_sample([0.1, -0.1])[0] > 0.09);
            settings.bands[0].enabled = false;
            eq.retarget(&settings);
            eq.reset();
            assert_eq!(eq.process_sample([0.1, -0.1]), [0.0; 2]);
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn mid_cut_steepness_sweeps_and_crossed_boundaries_stay_stable() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].enabled = false;
            settings.bands[0].q = 0.1;
            settings.bands[2].q = 0.1;
            let mut eq = Equalizer::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            for index in 0..40 {
                settings.bands[0].q = 0.1 * 100_f32.powf(index as f32 / 39.0);
                settings.bands[2].q = 10.0 / 100_f32.powf(index as f32 / 39.0);
                settings.bands[0].frequency_hz = 20.0 + index as f32 * 500.0;
                settings.bands[2].frequency_hz = 20000.0 - index as f32 * 500.0;
                eq.retarget(&settings);
                for frame in 0..rate / 100 {
                    let source = (frame as f32 * 0.1).sin() * 0.01;
                    assert!(
                        eq.process_sample([source, -source])
                            .iter()
                            .all(|value| value.is_finite() && value.abs() < 1.0)
                    );
                }
            }
        }
    }
}
