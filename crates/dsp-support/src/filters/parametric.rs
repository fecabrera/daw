//! Parametric EQ filter modes and corresponding settled coefficients.
use fundsp::{audionode::AudioNode, svf::*};

#[derive(Clone, Copy)]
pub enum Mode {
    Bell,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
    HighPass4,
    LowPass4,
    HighPass8,
    LowPass8,
}

pub struct FilterParameters {
    pub sample_rate: f64,
    pub cutoff: f64,
    pub q: f64,
    pub gain: f64,
}

// Butterworth section Qs relative to the shared default Q (1/√2):
// √2 / (2 cos((2k + 1)π / (2 × order))). Modulated Q scales every section.
pub const FOUR_POLE_Q_SCALE: [f64; 2] = [0.7653668647301796, 1.8477590650225735];
pub const EIGHT_POLE_Q_SCALE: [f64; 4] = [
    0.7209598220069479,
    0.8504300947672565,
    1.272758580572834,
    3.6245097854115516,
];
// SVF histories use f64, but AudioNode returns f32. Bound representational
// overflow before feeding another section; ordinary finite headroom is unchanged.
pub fn bounded(value: f32) -> f32 {
    value.clamp(-f32::MAX, f32::MAX)
}
#[derive(Clone)]
struct Cascade<M: SvfMode<f64>, const N: usize> {
    stages: [FixedSvf<f64, M>; N],
    q_scales: [f64; N],
}
impl<M: SvfMode<f64>, const N: usize> Cascade<M, N> {
    fn new(params: &SvfParams<f64>, q_scales: [f64; N]) -> Self {
        Self {
            stages: std::array::from_fn(|i| {
                FixedSvf::new(
                    M::default(),
                    &SvfParams {
                        sample_rate: params.sample_rate,
                        cutoff: params.cutoff,
                        q: params.q * q_scales[i],
                        gain: params.gain,
                    },
                )
            }),
            q_scales,
        }
    }
    fn set_cutoff_q_gain(&mut self, cutoff: f64, q: f64, gain: f64) {
        for (stage, scale) in self.stages.iter_mut().zip(self.q_scales) {
            stage.set_cutoff_q_gain(cutoff, q * scale, gain);
        }
    }
    fn filter_mono(&mut self, mut input: f32) -> f32 {
        for stage in &mut self.stages {
            input = bounded(stage.filter_mono(input));
        }
        input
    }
    fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }
    fn response(&mut self, channel: usize, frequency: f64) -> Option<fundsp::math::Complex64> {
        let mut response = fundsp::math::Complex64::new(1.0, 0.0);
        for stage in &mut self.stages {
            response *= stage.response(channel, frequency)?;
        }
        Some(response)
    }
}

#[derive(Clone)]
enum Node {
    Bell(FixedSvf<f64, BellMode<f64>>),
    LowShelf(FixedSvf<f64, LowshelfMode<f64>>),
    HighShelf(FixedSvf<f64, HighshelfMode<f64>>),
    LowPass(FixedSvf<f64, LowpassMode<f64>>),
    HighPass(FixedSvf<f64, HighpassMode<f64>>),
    Notch(FixedSvf<f64, NotchMode<f64>>),
    HighPass4(Cascade<HighpassMode<f64>, 2>),
    LowPass4(Cascade<LowpassMode<f64>, 2>),
    HighPass8(Cascade<HighpassMode<f64>, 4>),
    LowPass8(Cascade<LowpassMode<f64>, 4>),
}
macro_rules! filter {
    ($s:expr, $v:ident, $body:expr) => {
        match $s {
            Node::Bell($v) => $body,
            Node::LowShelf($v) => $body,
            Node::HighShelf($v) => $body,
            Node::LowPass($v) => $body,
            Node::HighPass($v) => $body,
            Node::Notch($v) => $body,
            Node::HighPass4($v) => $body,
            Node::LowPass4($v) => $body,
            Node::HighPass8($v) => $body,
            Node::LowPass8($v) => $body,
        }
    };
}
impl Node {
    fn new(mode: Mode, params: &SvfParams<f64>) -> Self {
        match mode {
            Mode::Bell => Self::Bell(FixedSvf::new(BellMode::new(), params)),
            Mode::LowShelf => Self::LowShelf(FixedSvf::new(LowshelfMode::new(), params)),
            Mode::HighShelf => Self::HighShelf(FixedSvf::new(HighshelfMode::new(), params)),
            Mode::LowPass => Self::LowPass(FixedSvf::new(LowpassMode::new(), params)),
            Mode::HighPass => Self::HighPass(FixedSvf::new(HighpassMode::new(), params)),
            Mode::Notch => Self::Notch(FixedSvf::new(NotchMode::new(), params)),
            Mode::HighPass4 => Self::HighPass4(Cascade::new(params, FOUR_POLE_Q_SCALE)),
            Mode::LowPass4 => Self::LowPass4(Cascade::new(params, FOUR_POLE_Q_SCALE)),
            Mode::HighPass8 => Self::HighPass8(Cascade::new(params, EIGHT_POLE_Q_SCALE)),
            Mode::LowPass8 => Self::LowPass8(Cascade::new(params, EIGHT_POLE_Q_SCALE)),
        }
    }
    pub fn configure(&mut self, values: [f64; 3]) {
        filter!(
            self,
            node,
            node.set_cutoff_q_gain(values[1], values[2], values[0])
        );
    }
    fn sample(&mut self, input: f32) -> f32 {
        bounded(filter!(self, node, node.filter_mono(input)))
    }
    fn reset(&mut self) {
        filter!(self, node, node.reset());
    }
}

#[derive(Clone)]
pub struct Filter(Node);

impl Filter {
    pub fn new(mode: Mode, p: &FilterParameters) -> Self {
        Self(Node::new(
            mode,
            &SvfParams {
                sample_rate: p.sample_rate,
                cutoff: p.cutoff,
                q: p.q,
                gain: p.gain,
            },
        ))
    }

    pub fn configure(&mut self, values: [f64; 3]) {
        self.0.configure(values);
    }

    pub fn sample(&mut self, input: f32) -> f32 {
        self.0.sample(input)
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }

    pub fn response_magnitude(&mut self, hz: f64) -> Option<f64> {
        filter!(&mut self.0, node, node.response(0, hz)).map(|value| value.norm())
    }
}

/// Return the biquad coefficients for one SVF section.
pub fn biquad(
    mode: Mode,
    frequency_hz: f32,
    q: f32,
    gain_db: f32,
    rate: u32,
) -> ([f64; 3], [f64; 3]) {
    let params = SvfParams {
        sample_rate: f64::from(rate),
        cutoff: f64::from(frequency_hz.clamp(20.0, (rate as f32 * 0.49).min(20_000.0))),
        q: f64::from(q),
        gain: 10_f64.powf(f64::from(gain_db) / 20.0),
    };
    let mut c = SvfCoefs::default();
    match mode {
        Mode::Bell => BellMode::new().update(&params, &mut c),
        Mode::LowShelf => LowshelfMode::new().update(&params, &mut c),
        Mode::HighShelf => HighshelfMode::new().update(&params, &mut c),
        Mode::LowPass => LowpassMode::new().update(&params, &mut c),
        Mode::HighPass => HighpassMode::new().update(&params, &mut c),
        Mode::Notch => NotchMode::new().update(&params, &mut c),
        Mode::HighPass4 | Mode::LowPass4 | Mode::HighPass8 | Mode::LowPass8 => {
            unreachable!("handled as cascaded responses")
        }
    }
    // H(z)=D+C(zI−A)⁻¹B from the SVF's two integrator states.
    let (a, b, c2, d) = (2.0 * c.a1 - 1.0, -2.0 * c.a2, 2.0 * c.a2, 1.0 - 2.0 * c.a3);
    let (b1, b2) = (2.0 * c.a2, 2.0 * c.a3);
    let direct = c.m0 + c.m1 * c.a2 + c.m2 * c.a3;
    let (c1, c2out) = (
        c.m1 * c.a1 + c.m2 * c.a2,
        -c.m1 * c.a2 + c.m2 * (1.0 - c.a3),
    );
    let det = a * d - b * c2;
    (
        [
            direct,
            -direct * (a + d) + c1 * b1 + c2out * b2,
            direct * det + c1 * (-d * b1 + b * b2) + c2out * (c2 * b1 - a * b2),
        ],
        [1.0, -a - d, det],
    )
}
