//! Measurements are independent of the engine's FFT, phase state, and window selection.
use crate::audio::{Audio, Result};
use audio_analysis::signal::{self, MeasurementError, Pcm};
use serde_json::{Value, json};
use std::{fs, path::Path};
use timestretch::Pitch;

pub fn diagnostics(audio: &Audio) -> Value {
    let result = signal::diagnostics(&audio.samples);
    json!({"peak":result.peak,"rms":result.rms,"nonfinite_samples":result.nonfinite_samples,"quality_acceptance":"not assessed"})
}

fn tone(audio: Pcm<'_>) -> Result<(f64, f64)> {
    let tone = audio
        .tone(audio.frames() / 4..audio.frames() * 3 / 4, 0, 20)
        .map_err(|error| match error {
            MeasurementError::InsufficientCycles => {
                "tone metric needs at least 20 cycles in middle half".into()
            }
            error => Box::<dyn std::error::Error>::from(error),
        })?;
    Ok((tone.frequency_hz, tone.rms))
}

fn event(audio: Pcm<'_>, start: usize, end: usize, rate: u32) -> Result<(f64, f64)> {
    let window = (rate as usize / 1000).max(1);
    let event = audio.event(start..end, window, 0.1, 0.1..0.9, 1e-20)?;
    Ok((event.onset_frame as f64, event.width_frames as f64))
}

pub fn fixture(
    path: Option<&Path>,
    input: &Audio,
    output: &Audio,
    ratio: timestretch::Ratio,
    pitch: Pitch,
) -> Result<Value> {
    let Some(path) = path else {
        return Ok(json!({"status":"unavailable","reason":"no known-signal manifest"}));
    };
    let spec: Value = serde_json::from_slice(&fs::read(path)?)?;
    analyze(&spec, input, output, ratio, pitch)
}
fn analyze(
    spec: &Value,
    input: &Audio,
    output: &Audio,
    ratio: timestretch::Ratio,
    pitch: Pitch,
) -> Result<Value> {
    if spec["schema_version"] != 1
        || !matches!(
            spec["metric_profile"].as_str(),
            Some("tone-v1" | "synthetic-v1")
        )
        || spec["prepared_sha256"] != input.hash()
    {
        return Err("fixture schema/profile or prepared hash mismatch".into());
    }
    let expected = (input.frames() as u128 * u128::from(ratio.numerator())
        + u128::from(ratio.denominator() / 2))
        / u128::from(ratio.denominator());
    if !output.samples.len().is_multiple_of(output.channels)
        || output.frames() as u128 != expected
        || output.channels != input.channels
        || output.rate != input.rate
        || output.samples.iter().any(|s| !s.is_finite())
    {
        return Err("invalid measured output layout, length, or samples".into());
    }
    let source_pcm = Pcm::new(&input.samples, input.channels, input.rate)?;
    let output_pcm = Pcm::new(&output.samples, output.channels, output.rate)?;
    let ordinary = (0.5..=2.).contains(&ratio.as_f64()) && (0.5..=2.).contains(&pitch.factor());
    let mut result = json!({"profile":spec["metric_profile"],"quality_acceptance":"specified metrics only; other gates unassessed"});
    let mut measurements = 0;
    if let Some(hz) = spec["frequency_hz"]
        .as_f64()
        .filter(|f| f.is_finite() && *f > 0.)
    {
        let expected_hz = hz * pitch.factor();
        if expected_hz >= f64::from(output.rate) / 2.0 {
            return Err("shifted test frequency is at or above Nyquist".into());
        }
        let (_, input_rms) = tone(source_pcm)?;
        let (frequency, output_rms) = tone(output_pcm)?;
        let cents = 1200. * (frequency / expected_hz).log2();
        let level_db = 20. * (output_rms / input_rms).log10();
        result["tone"] = json!({"frequency_hz":frequency,"expected_frequency_hz":expected_hz,"pitch_error_cents":cents,"level_error_db":level_db,"pitch_pass":cents.abs()<=if ordinary{5.}else{15.},"level_pass":level_db.abs()<=if ordinary{0.5}else{1.5}});
        measurements += 1;
        if let Some(expected_phase) = spec["stereo_phase_radians"].as_f64() {
            let phases = output_pcm
                .stereo_phase(output.frames() / 4..output.frames() * 3 / 4, expected_hz)?;
            let error = ((phases[1] - phases[0] - expected_phase + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI)
                .to_degrees();
            result["stereo_phase"] =
                json!({"error_degrees":error,"pass":error.abs()<=if ordinary{5.}else{15.}});
        }
    } else if spec["metric_profile"] == "tone-v1" {
        return Err("tone-v1 requires positive frequency_hz".into());
    }
    if let Some(factor) = spec["channel_factor"].as_f64() {
        let (residual, power) = output_pcm.channel_residual(factor)?;
        let normalized = residual / power.max(1e-24);
        let db = if residual == 0. {
            json!("-inf")
        } else {
            json!(10. * normalized.log10())
        };
        result["stereo_relationship"] = json!({"residual_db":db,"pass":normalized<=1e-8&&power>0.});
        measurements += 1;
    }
    if let Some(events) = spec["events"].as_array() {
        let mut reports = Vec::new();
        let mut previous_end = 0;
        for region in events {
            let start = region["start_frame"]
                .as_u64()
                .ok_or("event needs start_frame")? as usize;
            let end = region["end_frame"]
                .as_u64()
                .ok_or("event needs end_frame")? as usize;
            if start < previous_end {
                return Err("event regions must be ordered and non-overlapping".into());
            }
            previous_end = end;
            let (source_time, source_width) = event(source_pcm, start, end, input.rate)?;
            let mapped = |frame: usize| {
                ((frame as f64 * ratio.as_f64()).round() as usize).min(output.frames())
            };
            let (output_time, output_width) =
                event(output_pcm, mapped(start), mapped(end), output.rate)?;
            let displacement_ms =
                (output_time - source_time * ratio.as_f64()) * 1000. / f64::from(input.rate);
            let width_ratio =
                output_width / source_width.max(source_width * ratio.as_f64()).max(1.);
            reports.push(json!({"onset_displacement_ms":displacement_ms,"energy_width_ratio":width_ratio,"timing_pass":displacement_ms.abs()<=if ordinary{5.}else{15.},"width_pass":width_ratio<=if ordinary{2.}else{4.}}));
        }
        result["events"] = json!(reports);
        measurements += events.len();
    }
    if measurements == 0 {
        return Err("manifest contains no supported measurements".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analyzer_rejects_known_defects() {
        let make = |hz: f64, gain: f64| Audio {
            rate: 48000,
            channels: 2,
            samples: (0..48000)
                .flat_map(|i| {
                    let s =
                        (i as f64 * hz * std::f64::consts::TAU / 48000.).sin() as f32 * gain as f32;
                    [s, s * 0.5]
                })
                .collect(),
        };
        let input = make(440., 0.4);
        let ratio = timestretch::Ratio::new(1, 1).unwrap();
        let spec = json!({"schema_version":1,"metric_profile":"synthetic-v1","prepared_sha256":input.hash(),"frequency_hz":440.,"channel_factor":0.5});
        assert_eq!(
            analyze(&spec, &input, &input, ratio, Pitch::UNISON).unwrap()["tone"]["pitch_pass"],
            true
        );
        assert_eq!(
            analyze(&spec, &input, &make(880., 0.4), ratio, Pitch::UNISON).unwrap()["tone"]["pitch_pass"],
            false
        );
        let up = Pitch::from_semitones(12.0).unwrap();
        assert_eq!(
            analyze(&spec, &input, &make(880., 0.4), ratio, up).unwrap()["tone"]["pitch_pass"],
            true
        );
        assert_eq!(
            analyze(&spec, &input, &input, ratio, up).unwrap()["tone"]["pitch_pass"],
            false
        );
        assert_eq!(
            analyze(&spec, &input, &make(440., 0.8), ratio, Pitch::UNISON).unwrap()["tone"]["level_pass"],
            false
        );
        let mut reversed = make(440., 0.4);
        for f in reversed.samples.chunks_exact_mut(2) {
            f.swap(0, 1);
        }
        assert_eq!(
            analyze(&spec, &input, &reversed, ratio, Pitch::UNISON).unwrap()["stereo_relationship"]
                ["pass"],
            false
        );
        for f in reversed.samples.chunks_exact_mut(2) {
            f[1] = f[0];
        }
        assert_eq!(
            analyze(&spec, &input, &reversed, ratio, Pitch::UNISON).unwrap()["stereo_relationship"]
                ["pass"],
            false
        );
        reversed.samples.pop();
        assert!(analyze(&spec, &input, &reversed, ratio, Pitch::UNISON).is_err());
        reversed = make(440., 0.);
        assert!(analyze(&spec, &input, &reversed, ratio, Pitch::UNISON).is_err());
        reversed = make(440., 0.4);
        reversed.samples[0] = f32::NAN;
        assert!(analyze(&spec, &input, &reversed, ratio, Pitch::UNISON).is_err());
        let mut event_input = Audio {
            rate: 48000,
            channels: 1,
            samples: vec![0.; 48000],
        };
        for s in &mut event_input.samples[20000..20480] {
            *s = 0.5;
        }
        let spec = json!({"schema_version":1,"metric_profile":"synthetic-v1","prepared_sha256":event_input.hash(),"events":[{"start_frame":0,"end_frame":48000}]});
        let mut delayed = Audio {
            rate: 48000,
            channels: 1,
            samples: vec![0.; 48000],
        };
        for s in &mut delayed.samples[22000..22480] {
            *s = 0.5;
        }
        assert_eq!(
            analyze(&spec, &event_input, &delayed, ratio, Pitch::UNISON).unwrap()["events"][0]["timing_pass"],
            false
        );
    }
}
