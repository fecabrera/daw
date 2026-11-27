//! Stereo shelf, cut and middle-band filters with matching response and analysis.
use fundsp::{audionode::AudioNode, svf::*};

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
#[derive(Clone)]
pub struct Band {
    normal: Option<[Filter; 2]>,
    cut: Option<[Filter; 2]>,
    current: [f32; 4],
    target: [f32; 4],
    remaining: u32,
    configured: [f32; 4],
}
impl Band {
    pub fn current(&self) -> [f32; 4] {
        self.current
    }

    pub fn new(index: usize, sample_rate_hz: u32, current: [f32; 4]) -> Self {
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
    pub fn retarget(&mut self, mut target: [f32; 4], frames: u32) {
        if self.normal.is_none() {
            // Mid owns gain/power only. Legacy frequency/Q changes do not restart its ramp.
            target[..2].copy_from_slice(&self.target[..2]);
        }
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    pub fn advance(&mut self) {
        if self.remaining > 0 {
            for (current, target) in self.current.iter_mut().zip(self.target) {
                *current += (target - *current) / self.remaining as f32;
            }
            self.remaining -= 1;
        }
    }
    pub fn configure(&mut self, values: [f32; 4]) {
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
    pub fn sample(&mut self, input: [f32; 2], external: Option<([f32; 2], [f32; 2])>) -> [f32; 2] {
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
    pub fn reset(&mut self) {
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
#[derive(Clone)]
pub struct MidCut {
    low: [[Filter; 2]; 8],
    high: [[Filter; 2]; 8],
    parameters: [f32; 4],
    order: [f32; 2],
    width: f32,
}
impl MidCut {
    pub fn new(sample_rate_hz: u32, low: [f32; 4], high: [f32; 4]) -> Self {
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
        cut.configure(low[0], high[0], low[1], high[1]);
        cut
    }
    pub fn configure(&mut self, low: f32, high: f32, low_q: f32, high_q: f32) {
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
    pub fn sample(&mut self, input: [f32; 2]) -> [f32; 2] {
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
    pub fn reset(&mut self) {
        for filter in self.low.iter_mut().chain(&mut self.high).flatten() {
            filter.reset();
        }
    }
}

/// Opposing shelves keep the outer ranges at unity and set the middle plateau's gain.
#[derive(Clone)]
pub struct MidShelf {
    low: [Filter; 2],
    high: [Filter; 2],
    parameters: [f32; 5],
    gain: f32,
}
impl MidShelf {
    pub fn new(rate: u32) -> Self {
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
    pub fn configure(&mut self, low: [f32; 4], high: [f32; 4], gain: f32) {
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
    pub fn sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|ch| self.high[ch].sample(self.low[ch].sample(input[ch])) * self.gain)
    }
    pub fn reset(&mut self) {
        for filter in self.low.iter_mut().chain(&mut self.high) {
            filter.reset();
        }
    }
}

type Complex = fundsp::math::Complex64;
fn filter(filter: &mut Filter, hz: f64) -> Complex {
    with_filter!(
        filter,
        node,
        node.response(0, hz).unwrap_or(Complex::new(1.0, 0.0))
    )
}
/// Low/Mid/High response curves, including band cuts. Bypass is handled by the graph.
pub fn response(settings: [[f32; 4]; 3], rate: u32) -> [Vec<f32>; 3] {
    let mut bands: [Band; 3] = std::array::from_fn(|i| Band::new(i, rate, settings[i]));
    let mut mid_shelf = MidShelf::new(rate);
    mid_shelf.configure(settings[0], settings[2], settings[1][2]);
    let mut mid_cut = MidCut::new(rate, settings[0], settings[2]);
    std::array::from_fn(|band| {
        (0..160)
            .map(|i| {
                let hz = 20.0 * 1000_f64.powf(i as f64 / 159.0);
                let v = if !settings.iter().any(|b| b[3] != 0.0) {
                    Complex::new(0.0, 0.0)
                } else if band != 1 {
                    let b = &mut bands[band];
                    let filters = if settings[band][3] != 0.0 {
                        b.normal.as_mut()
                    } else {
                        b.cut.as_mut()
                    }
                    .unwrap();
                    filter(&mut filters[0], hz)
                } else if settings[1][3] != 0.0 {
                    filter(&mut mid_shelf.low[0], hz)
                        * filter(&mut mid_shelf.high[0], hz)
                        * f64::from(mid_shelf.gain)
                } else {
                    let mut total = Complex::new(0.0, 0.0);
                    for (branch, filters) in [&mut mid_cut.low, &mut mid_cut.high]
                        .into_iter()
                        .enumerate()
                    {
                        let order = mid_cut.order[branch];
                        let mut value = Complex::new(1.0, 0.0);
                        let mut first = value;
                        let mut last = value;
                        for (stage, filters) in filters.iter_mut().enumerate() {
                            value *= filter(&mut filters[0], hz);
                            if stage == order.floor() as usize - 1 {
                                first = value;
                            }
                            if stage == order.ceil() as usize - 1 {
                                last = value;
                            }
                        }
                        total += first + (last - first) * f64::from(order.fract());
                    }
                    Complex::new(1.0, 0.0)
                        + (total - Complex::new(1.0, 0.0)) * f64::from(mid_cut.width)
                };
                (20.0 * v.norm().max(1e-12).log10()).clamp(-144.0, 144.0) as f32
            })
            .collect()
    })
}

#[derive(Clone)]
pub struct BandAnalyzer {
    filters: [[Filter; 2]; 4],
    boundaries: [f32; 2],
}

impl BandAnalyzer {
    pub fn new(sample_rate_hz: u32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(sample_rate_hz),
            cutoff: 1000.0,
            q: f64::from(std::f32::consts::FRAC_1_SQRT_2),
            gain: 1.0,
        };
        Self {
            filters: std::array::from_fn(|index| {
                std::array::from_fn(|_| {
                    Filter::new(if index % 2 == 0 { 2 } else { 0 }, true, &params)
                })
            }),
            boundaries: [f32::NAN; 2],
        }
    }
    pub fn sample(&mut self, output: [f32; 2], low: f32, high: f32) -> [f32; 3] {
        let boundaries = [low.min(high), low.max(high)];
        if self.boundaries != boundaries {
            self.boundaries = boundaries;
            for (index, filters) in self.filters.iter_mut().enumerate() {
                let cutoff = boundaries[usize::from(index >= 2)];
                for filter in filters {
                    filter.configure([cutoff, std::f32::consts::FRAC_1_SQRT_2, 0.0, 1.0]);
                }
            }
        }
        let mut peaks = [0.0_f32; 3];
        for (ch, sample) in output.into_iter().enumerate() {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            let low = self.filters[0][ch].sample(sample);
            let middle = self.filters[1][ch].sample(sample);
            let middle = self.filters[2][ch].sample(middle);
            let high = self.filters[3][ch].sample(sample);
            for (peak, value) in peaks.iter_mut().zip([low, middle, high]) {
                if value.is_finite() {
                    *peak = peak.max(value.abs());
                }
            }
        }
        if boundaries[0] == boundaries[1] {
            peaks[1] = 0.0;
        }
        peaks
    }
    pub fn reset(&mut self) {
        for filter in self.filters.iter_mut().flatten() {
            filter.reset();
        }
    }
}
