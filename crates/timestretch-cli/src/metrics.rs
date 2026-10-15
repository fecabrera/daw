//! Measurements are independent of the engine's FFT, phase state, and window selection.
use crate::audio::{Audio, Result};
use serde_json::{Value, json};
use std::{fs, path::Path};
pub fn diagnostics(audio: &Audio) -> Value {
    let invalid = audio.samples.iter().filter(|s| !s.is_finite()).count();
    let peak = audio
        .samples
        .iter()
        .map(|s| f64::from(*s).abs())
        .fold(0.0, f64::max);
    let rms = (audio
        .samples
        .iter()
        .map(|s| f64::from(*s).powi(2))
        .sum::<f64>()
        / audio.samples.len().max(1) as f64)
        .sqrt();
    json!({"peak":peak,"rms":rms,"nonfinite_samples":invalid,"quality_acceptance":"not assessed"})
}
fn tone(audio: &Audio) -> Result<(f64, f64)> {
    let start = audio.frames() / 4;
    let end = audio.frames() * 3 / 4;
    let mut crossings = Vec::new();
    let mut energy = 0.;
    for i in start..end.saturating_sub(1) {
        let a = f64::from(audio.samples[i * audio.channels]);
        let b = f64::from(audio.samples[(i + 1) * audio.channels]);
        energy += a * a;
        if a <= 0. && b > 0. {
            crossings.push(i as f64 - a / (b - a));
        }
    }
    if crossings.len() < 21 {
        return Err("tone metric needs at least 20 cycles in middle half".into());
    }
    Ok((
        (crossings.len() - 1) as f64 * f64::from(audio.rate)
            / (crossings[crossings.len() - 1] - crossings[0]),
        (energy / (end - start - 1) as f64).sqrt(),
    ))
}
fn channel_residual(audio: &Audio, factor: f64) -> Result<(f64, f64)> {
    if audio.channels != 2 {
        return Err("channel relationship requires stereo".into());
    }
    let mut residual = 0.;
    let mut reference = 0.;
    for frame in audio.samples.chunks_exact(2) {
        residual += (f64::from(frame[1]) - factor * f64::from(frame[0])).powi(2);
        reference += f64::from(frame[0]).powi(2);
    }
    Ok((residual, reference))
}
fn event(audio: &Audio, start: usize, end: usize) -> Result<(f64, f64)> {
    let window = (audio.rate as usize / 1000).max(1);
    if start >= end || end > audio.frames() || end - start < window {
        return Err("event region is outside audio or too short to measure".into());
    }
    let energy: Vec<_> = audio.samples[start * audio.channels..end * audio.channels]
        .chunks_exact(audio.channels)
        .map(|f| f.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>())
        .collect();
    let envelope: Vec<_> = energy
        .windows(window)
        .map(|s| s.iter().sum::<f64>() / window as f64)
        .collect();
    let peak = envelope.iter().copied().fold(0., f64::max);
    if peak <= 1e-20 {
        return Err("expected event is missing".into());
    }
    let onset = start
        + envelope
            .iter()
            .position(|e| *e >= peak * 0.1)
            .ok_or("missing event onset")?;
    let total: f64 = energy.iter().sum();
    let (mut sum, mut low, mut high) = (0., None, None);
    for (i, e) in energy.iter().enumerate() {
        sum += e;
        if low.is_none() && sum >= total * 0.1 {
            low = Some(i);
        }
        if high.is_none() && sum >= total * 0.9 {
            high = Some(i);
        }
    }
    Ok((
        onset as f64,
        (high.ok_or("missing event energy")? - low.ok_or("missing event energy")?) as f64,
    ))
}
pub fn fixture(
    path: Option<&Path>,
    input: &Audio,
    output: &Audio,
    ratio: timestretch::Ratio,
) -> Result<Value> {
    let Some(path) = path else {
        return Ok(json!({"status":"unavailable","reason":"no known-signal manifest"}));
    };
    let spec: Value = serde_json::from_slice(&fs::read(path)?)?;
    analyze(&spec, input, output, ratio)
}
fn analyze(
    spec: &Value,
    input: &Audio,
    output: &Audio,
    ratio: timestretch::Ratio,
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
    let ordinary = (0.5..=2.).contains(&ratio.as_f64());
    let mut result = json!({"profile":spec["metric_profile"],"quality_acceptance":"specified metrics only; other gates unassessed"});
    let mut measurements = 0;
    if let Some(hz) = spec["frequency_hz"]
        .as_f64()
        .filter(|f| f.is_finite() && *f > 0.)
    {
        let (_, input_rms) = tone(input)?;
        let (frequency, output_rms) = tone(output)?;
        let cents = 1200. * (frequency / hz).log2();
        let level_db = 20. * (output_rms / input_rms).log10();
        result["tone"] = json!({"frequency_hz":frequency,"pitch_error_cents":cents,"level_error_db":level_db,"pitch_pass":cents.abs()<=if ordinary{5.}else{15.},"level_pass":level_db.abs()<=if ordinary{0.5}else{1.5}});
        measurements += 1;
        if let Some(expected_phase) = spec["stereo_phase_radians"].as_f64() {
            if output.channels != 2 {
                return Err("phase metric requires stereo".into());
            }
            let mut phases = [0.; 2];
            for (c, phase) in phases.iter_mut().enumerate() {
                let (mut sine, mut cosine) = (0., 0.);
                for i in output.frames() / 4..output.frames() * 3 / 4 {
                    let a = i as f64 * hz * std::f64::consts::TAU / f64::from(output.rate);
                    let value = f64::from(output.samples[2 * i + c]);
                    sine += value * a.sin();
                    cosine += value * a.cos();
                }
                *phase = cosine.atan2(sine);
            }
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
        let (residual, power) = channel_residual(output, factor)?;
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
            let (source_time, source_width) = event(input, start, end)?;
            let mapped = |frame: usize| {
                ((frame as f64 * ratio.as_f64()).round() as usize).min(output.frames())
            };
            let (output_time, output_width) = event(output, mapped(start), mapped(end))?;
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
            analyze(&spec, &input, &input, ratio).unwrap()["tone"]["pitch_pass"],
            true
        );
        assert_eq!(
            analyze(&spec, &input, &make(880., 0.4), ratio).unwrap()["tone"]["pitch_pass"],
            false
        );
        assert_eq!(
            analyze(&spec, &input, &make(440., 0.8), ratio).unwrap()["tone"]["level_pass"],
            false
        );
        let mut reversed = make(440., 0.4);
        for f in reversed.samples.chunks_exact_mut(2) {
            f.swap(0, 1);
        }
        assert_eq!(
            analyze(&spec, &input, &reversed, ratio).unwrap()["stereo_relationship"]["pass"],
            false
        );
        for f in reversed.samples.chunks_exact_mut(2) {
            f[1] = f[0];
        }
        assert_eq!(
            analyze(&spec, &input, &reversed, ratio).unwrap()["stereo_relationship"]["pass"],
            false
        );
        reversed.samples.pop();
        assert!(analyze(&spec, &input, &reversed, ratio).is_err());
        reversed = make(440., 0.);
        assert!(analyze(&spec, &input, &reversed, ratio).is_err());
        reversed = make(440., 0.4);
        reversed.samples[0] = f32::NAN;
        assert!(analyze(&spec, &input, &reversed, ratio).is_err());
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
            analyze(&spec, &event_input, &delayed, ratio).unwrap()["events"][0]["timing_pass"],
            false
        );
    }
}
