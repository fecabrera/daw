//! One destination definition supplies normal controls and signed modulation amount controls.
use crate::knobs::{self, AmountMapping, KnobSpec};
use daw_core::ModulationParameter as Parameter;

fn percentage(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}
fn percent(name: &'static str, reset: f32) -> KnobSpec {
    KnobSpec::new(name, 0.0..=1.0, reset, 0.01)
        .value_label(percentage)
        .format(percentage)
        .endpoint_labels("0", "100")
}
fn spread_intensity(name: &'static str) -> KnobSpec {
    KnobSpec::new(name, -1.0..=1.0, 0.0, 0.01)
        .centered(0.0)
        .bipolar()
        .value_label(percentage)
        .format(percentage)
        .endpoint_labels("−100", "+100")
}
fn frequency(name: &'static str, reset: f32) -> KnobSpec {
    KnobSpec::new(name, 20.0..=20_000.0, reset, 1.0)
        .scale(
            |v| (v / 20.0).log10() / 3.0,
            |p| 20.0 * 1000_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels("20", "20k")
}
fn eq_gain() -> KnobSpec {
    KnobSpec::new("EQ gain", -24.0..=24.0, 0.0, 0.1)
        .centered(0.0)
        .bipolar()
        .reference_step(6.0)
        .value_label(|v| format!("{v:.1} dB"))
        .format(|v| format!("{v:.1} dB"))
        .endpoint_labels("−24", "+24")
}
fn time(value: f32) -> String {
    if value < 10.0 {
        format!("{value:.1} ms")
    } else {
        format!("{value:.0} ms")
    }
}
fn compressor_time(attack: bool) -> KnobSpec {
    let spec = if attack {
        KnobSpec::new("Attack", 0.1..=200.0, 10.0, 0.1).scale(
            |v| (v / 0.1).ln() / 2000_f32.ln(),
            |p| 0.1 * 2000_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
    } else {
        KnobSpec::new("Release", 10.0..=2000.0, 100.0, 0.1).scale(
            |v| (v / 10.0).ln() / 200_f32.ln(),
            |p| 10.0 * 200_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
    };
    spec.value_label(time).format(time).endpoint_labels(
        if attack { "0.1" } else { "10" },
        if attack { "200" } else { "2k" },
    )
}
fn ott_frequency(low: bool) -> KnobSpec {
    let (name, min, max, reset) = if low {
        ("Low/Mid crossover", 20.0, 2000.0, 250.0)
    } else {
        ("Mid/High crossover", 200.0, 20_000.0, 2500.0)
    };
    KnobSpec::new(name, min..=max, reset, 1.0)
        .scale(
            if low {
                |v| (v / 20.0).log10() / 2.0
            } else {
                |v| (v / 200.0).log10() / 2.0
            },
            if low {
                |p| 20.0 * 100_f32.powf(p)
            } else {
                |p| 200.0 * 100_f32.powf(p)
            },
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels(
            if low { "20" } else { "200" },
            if low { "2k" } else { "20k" },
        )
}
pub(super) fn destination(parameter: Parameter) -> KnobSpec {
    match parameter {
        Parameter::BitcrusherDrive => KnobSpec::new("Drive", 0.0..=24.0, 0.0, 0.1)
            .reference_step(6.0)
            .value_label(|v| format!("{v:.1} dB"))
            .format(|v| format!("{v:.1} dB"))
            .endpoint_labels("0", "+24"),
        Parameter::BitcrusherResolution => KnobSpec::new("Resolution", 1.0..=24.0, 24.0, 1.0)
            .stepped(1.0)
            .reference_step(4.0)
            .value_label(|v| format!("{v:.0} bit"))
            .format(|v| format!("{v:.0} bit"))
            .endpoint_labels("1", "24"),
        Parameter::BitcrusherDownsampling => KnobSpec::new("Downsampling", 1.0..=40.0, 1.0, 1.0)
            .stepped(1.0)
            .reference_step(5.0)
            .value_label(|v| format!("{v:.0}×"))
            .format(|v| format!("{v:.0}×"))
            .endpoint_labels("1", "40"),
        Parameter::StereoSpreadLowerIntensity => spread_intensity("Lower intensity"),
        Parameter::StereoSpreadUpperIntensity => spread_intensity("Upper intensity"),
        Parameter::StereoSpreadLowerFrequency => frequency("Lower frequency", 20.0),
        Parameter::StereoSpreadUpperFrequency => frequency("Upper frequency", 20_000.0),
        Parameter::DirectionMixerDirection => crate::lfo_controls::phase_spec("Direction"),
        Parameter::DirectionMixerSpread => KnobSpec::new("Spread", 0.0..=2.0, 1.0, 0.01)
            .centered(1.0)
            .value_label(|v| format!("{v:.2}"))
            .format(|v| format!("{v:.2}"))
            .endpoint_labels("0", "2"),
        Parameter::OttLowFrequency => ott_frequency(true),
        Parameter::OttHighFrequency => ott_frequency(false),
        Parameter::OttLowDepth | Parameter::OttMidDepth | Parameter::OttHighDepth => {
            percent("Compression depth", 1.0)
        }
        Parameter::OttLowInput | Parameter::OttMidInput | Parameter::OttHighInput => {
            KnobSpec::new("Band input", -24.0..=24.0, 0.0, 0.1)
                .centered(0.0)
                .bipolar()
                .value_label(|v| format!("{v:.1} dB"))
                .format(|v| format!("{v:.1} dB"))
                .endpoint_labels("−24", "+24")
        }
        Parameter::OttLowOutput | Parameter::OttMidOutput | Parameter::OttHighOutput => {
            KnobSpec::new("Band output", -24.0..=24.0, 0.0, 0.1)
                .centered(0.0)
                .bipolar()
                .value_label(|v| format!("{v:.1} dB"))
                .format(|v| format!("{v:.1} dB"))
                .endpoint_labels("−24", "+24")
        }
        Parameter::OttLowAttack | Parameter::OttMidAttack | Parameter::OttHighAttack => {
            compressor_time(true)
        }
        Parameter::OttLowRelease | Parameter::OttMidRelease | Parameter::OttHighRelease => {
            compressor_time(false)
        }

        Parameter::LimiterInput => KnobSpec::new("Limiter input", -24.0..=24.0, 0.0, 0.1)
            .reference_step(6.0)
            .value_label(|v| format!("{v:.1} dB"))
            .format(|v| format!("{v:.1} dB"))
            .endpoint_labels("−24", "+24"),
        Parameter::LimiterRelease => KnobSpec::new("Release", 1.0..=2000.0, 100.0, 0.1)
            .scale(
                |v| v.ln() / 2000_f32.ln(),
                |p| 2000_f32.powf(p),
                |v, d| v * 2_f32.powf(d / 24.0),
            )
            .value_label(time)
            .format(time)
            .endpoint_labels("1", "2k"),
        Parameter::LimiterLookahead => KnobSpec::new("Lookahead", 0.0..=10.0, 1.0, 0.1)
            .value_label(time)
            .format(time)
            .endpoint_labels("0", "10"),
        Parameter::LimiterCeiling => KnobSpec::new("Ceiling", -24.0..=0.0, -1.0, 0.1)
            .value_label(|v| format!("{v:.1} dB"))
            .format(|v| format!("{v:.1} dB"))
            .endpoint_labels("−24", "0"),
        Parameter::CompressorThreshold => KnobSpec::new("Threshold", -60.0..=0.0, -18.0, 0.1)
            .value_label(|v| format!("{v:.1} dB"))
            .format(|v| format!("{v:.1} dBFS"))
            .endpoint_labels("−60", "0"),
        Parameter::CompressorRatio => KnobSpec::new("Ratio", 1.0..=20.0, 4.0, 0.1)
            .value_label(|v| format!("{v:.1}:1"))
            .format(|v| format!("{v:.1}:1"))
            .endpoint_labels("1:1", "20:1"),
        Parameter::CompressorAttack => compressor_time(true),
        Parameter::CompressorRelease => compressor_time(false),
        Parameter::CompressorKnee => KnobSpec::new("Knee", 0.0..=24.0, 6.0, 0.1)
            .value_label(|v| format!("{v:.1} dB"))
            .format(|v| format!("{v:.1} dB"))
            .endpoint_labels("0", "24"),
        Parameter::MacroValue => percent("Macro value", 0.0),
        Parameter::LfoDepth => percent("LFO depth", 1.0),
        Parameter::LfoRate => KnobSpec::new("LFO rate", 0.01..=20.0, 2.0, 0.01)
            .scale(
                |v| (v / 0.01).ln() / 2000_f32.ln(),
                |p| 0.01 * 2000_f32.powf(p),
                |v, d| v * 2_f32.powf(d / 24.0),
            )
            .value_label(|v| format!("{v:.2} Hz"))
            .format(|v| format!("{v:.2} Hz"))
            .endpoint_labels("0.01", "20"),
        Parameter::Pitch => knobs::pitch_spec(),
        Parameter::Amplitude => knobs::marked_gain_spec(),
        Parameter::SamplerGain => knobs::marked_gain_spec().named("Sampler gain"),
        Parameter::Level => knobs::marked_gain_spec().named("Filter output level"),
        Parameter::Volume => knobs::marked_gain_spec().named("Drive volume"),
        Parameter::Frequency => frequency("Filter frequency", 1000.0),
        Parameter::EqLowFrequency => frequency(parameter.label(), 250.0),
        Parameter::EqHighFrequency => frequency(parameter.label(), 2500.0),
        Parameter::EqLowGain | Parameter::EqMidGain | Parameter::EqHighGain => {
            eq_gain().named(parameter.label())
        }
        Parameter::EqLowQ | Parameter::EqHighQ => {
            destination(Parameter::Resonance).named(parameter.label())
        }
        Parameter::ColorFrequency => KnobSpec::new("Color frequency", 250.0..=2000.0, 750.0, 1.0)
            .scale(
                |v| (v / 250.0).log2() / 3.0,
                |p| 250.0 * 8_f32.powf(p),
                |v, d| v * 2_f32.powf(d / 24.0),
            )
            .value_label(knobs::format_frequency)
            .format(knobs::format_frequency)
            .endpoint_labels("250", "2k"),
        Parameter::Resonance => KnobSpec::new(
            "Resonance",
            0.1..=10.0,
            std::f32::consts::FRAC_1_SQRT_2,
            0.01,
        )
        .scale(
            |v| (v / 0.1).log10() / 2.0,
            |p| 0.1 * 100_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
        .value_label(|v| format!("Q {v:.2}"))
        .format(|v| format!("Q {v:.2}"))
        .endpoint_labels("0.1", "10"),
        Parameter::DryWet => percent("Dry/wet", 1.0).endpoint_labels("Dry", "Wet"),
        Parameter::DriveGain => percent("Drive gain", 0.2),
        Parameter::Tone => percent("Drive tone", 0.5)
            .centered(0.5)
            .bipolar()
            .endpoint_labels("−", "+"),
        Parameter::ColorStrength => percent("Color strength", 0.5),
    }
}
fn signed_percent(value: f32) -> String {
    format!("{:+.0}%", value * 100.0)
}
fn signed_depth(value: f32) -> String {
    format!("{:+.1}%", value * 100.0)
}
fn signed_db(value: f32) -> String {
    format!("{value:+.1} dB")
}
fn signed_octaves(value: f32) -> String {
    format!("{value:+.2} oct")
}
fn signed_q(value: f32) -> String {
    format!("Q {value:+.2}")
}
fn signed_pitch(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:+.0}")
    } else {
        format!("{value:+.2}")
    }
}
pub(super) fn amount(parameter: Parameter) -> KnobSpec {
    let (mapping, format, endpoints): (AmountMapping, fn(f32) -> String, [&str; 2]) =
        match parameter {
            Parameter::BitcrusherDrive => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                signed_db,
                ["−24", "+24"],
            ),
            Parameter::BitcrusherResolution => (
                AmountMapping::Offset {
                    origin: 1.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.0} bits"),
                ["−23", "+23"],
            ),
            Parameter::BitcrusherDownsampling => (
                AmountMapping::Offset {
                    origin: 1.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.0}×"),
                ["−39", "+39"],
            ),
            Parameter::DirectionMixerDirection => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.0}°"),
                ["−180", "+180"],
            ),
            Parameter::DirectionMixerSpread => (
                AmountMapping::Offset {
                    origin: 1.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.2}"),
                ["−2", "+2"],
            ),
            Parameter::OttLowInput
            | Parameter::OttMidInput
            | Parameter::OttHighInput
            | Parameter::OttLowOutput
            | Parameter::OttMidOutput
            | Parameter::OttHighOutput
            | Parameter::LimiterInput
            | Parameter::EqLowGain
            | Parameter::EqMidGain
            | Parameter::EqHighGain => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                signed_db,
                ["−24", "+24"],
            ),
            Parameter::LimiterCeiling => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: -1.0,
                },
                signed_db,
                ["−24", "+24"],
            ),
            Parameter::LimiterLookahead => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.1} ms"),
                ["−10", "+10"],
            ),
            Parameter::CompressorThreshold => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: -1.0,
                },
                signed_db,
                ["−60", "+60"],
            ),
            Parameter::CompressorKnee => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                signed_db,
                ["−24", "+24"],
            ),
            Parameter::CompressorRatio => (
                AmountMapping::Offset {
                    origin: 1.0,
                    direction: 1.0,
                },
                |v| format!("{v:+.1}:1"),
                ["−19", "+19"],
            ),
            Parameter::Pitch => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                signed_pitch,
                ["−48", "+48"],
            ),
            Parameter::Amplitude => (AmountMapping::Depth, signed_depth, ["−100", "+100"]),
            Parameter::SamplerGain | Parameter::Level | Parameter::Volume => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: -1.0,
                },
                signed_db,
                ["−24", "+24"],
            ),
            Parameter::StereoSpreadLowerFrequency
            | Parameter::StereoSpreadUpperFrequency
            | Parameter::OttLowFrequency
            | Parameter::OttHighFrequency
            | Parameter::OttLowAttack
            | Parameter::OttMidAttack
            | Parameter::OttHighAttack
            | Parameter::OttLowRelease
            | Parameter::OttMidRelease
            | Parameter::OttHighRelease
            | Parameter::LimiterRelease
            | Parameter::CompressorAttack
            | Parameter::CompressorRelease
            | Parameter::Frequency
            | Parameter::LfoRate
            | Parameter::EqLowFrequency
            | Parameter::EqHighFrequency => (AmountMapping::Octaves, signed_octaves, ["−6", "+6"]),
            Parameter::ColorFrequency => (AmountMapping::Octaves, signed_octaves, ["−3", "+3"]),
            Parameter::Resonance | Parameter::EqLowQ | Parameter::EqHighQ => (
                AmountMapping::Offset {
                    origin: 0.1,
                    direction: 1.0,
                },
                signed_q,
                ["−10", "+10"],
            ),
            Parameter::StereoSpreadLowerIntensity
            | Parameter::StereoSpreadUpperIntensity
            | Parameter::OttLowDepth
            | Parameter::OttMidDepth
            | Parameter::OttHighDepth
            | Parameter::MacroValue
            | Parameter::LfoDepth
            | Parameter::DryWet
            | Parameter::DriveGain
            | Parameter::Tone
            | Parameter::ColorStrength => (
                AmountMapping::Offset {
                    origin: 0.0,
                    direction: 1.0,
                },
                signed_percent,
                ["−100", "+100"],
            ),
        };
    let spec = destination(parameter)
        .modulation_amount(parameter.limit(), mapping)
        .value_label(format)
        .format(format)
        .endpoint_labels(endpoints[0], endpoints[1]);
    if parameter == Parameter::Pitch {
        spec.format(|v| format!("{} semitones", signed_pitch(v)))
    } else {
        spec
    }
}
