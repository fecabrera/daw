//! Centered weighted overlap-add phase vocoder. See PROVENANCE.md for equations.
use crate::{Error, Request, buffer, checkpoint};
use rustfft::{FftPlanner, num_complex::Complex};
use std::{f64::consts::TAU, ops::ControlFlow};

fn wrap(phase: f64) -> f64 {
    (phase + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI
}

pub(crate) fn process(
    request: Request,
    input: &[f32],
    frames_out: usize,
    progress: &mut impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Vec<f32>, Error> {
    let channels = request.channels;
    let frames_in = input.len() / channels;
    let ratio = request.ratio.as_f64();
    let tonal_width =
        ((request.sample_rate as usize * 40 / 1000).next_power_of_two()).clamp(1024, 8192);
    let width = analysis_width(request, input, tonal_width, progress)?;
    let bins = width / 2 + 1;
    let half = (width / 2) as i64;
    // Limit both analysis and synthesis advances to 1/8 window, even at extremes.
    let hop = ((width / 8) as f64 * ratio.min(1.0)).round().max(1.0) as usize;
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(width);
    let inverse = planner.plan_fft_inverse(width);
    let mut scratch = buffer(
        forward
            .get_inplace_scratch_len()
            .max(inverse.get_inplace_scratch_len()),
        Complex::default(),
    )?;
    let window: Vec<_> = (0..width)
        .map(|i| 0.5 - 0.5 * (TAU * i as f64 / width as f64).cos())
        .collect();
    let mut spectra = buffer(channels * width, Complex::default())?;
    let mut previous = buffer(channels * bins, Complex::default())?;
    let mut rotations = buffer(bins, 0.0)?;
    let mut magnitudes = buffer(bins, 0.0)?;
    let mut peaks = Vec::with_capacity(bins);
    let mut accum = buffer(frames_out * channels, 0.0f64)?;
    // Every synthesis center is a multiple of hop. With padded centers at both
    // ends, the window-square sum is periodic even at the returned boundaries.
    let mut weights = buffer(hop, 0.0f64)?;
    for (i, value) in window.iter().enumerate() {
        weights[(i + hop - width / 2 % hop) % hop] += value * value;
    }
    let mut last_source = 0i64;
    let mut first = true;
    // Synthesis centers extend past both boundaries; crop by output coordinate, not delay.
    let start = -(width as i64 / hop as i64) * hop as i64;
    let end = frames_out as i64 + half;
    let mut center = start;
    while center < end {
        checkpoint(
            progress,
            0.15 + 0.75 * (center - start) as f32 / (end - start) as f32,
        )?;
        let source = (center as f64 / ratio).round() as i64;
        for channel in 0..channels {
            let spectrum = &mut spectra[channel * width..(channel + 1) * width];
            for (i, value) in spectrum.iter_mut().enumerate() {
                let position = source + i as i64 - half;
                let sample = if (0..frames_in as i64).contains(&position) {
                    input[position as usize * channels + channel]
                } else {
                    0.0
                };
                *value = Complex::new(f64::from(sample) * window[i], 0.0);
            }
            forward.process_with_scratch(spectrum, &mut scratch);
        }
        for bin in 0..bins {
            // Sum cross-products, never channel samples: anti-phase cannot cancel analysis.
            let mut correlation = Complex::<f64>::default();
            let mut energy = 0.0;
            for channel in 0..channels {
                let value = spectra[channel * width + bin];
                correlation += value * previous[channel * bins + bin].conj();
                energy += value.norm_sqr();
                previous[channel * bins + bin] = value;
            }
            magnitudes[bin] = energy;
            if !first {
                let advance = (source - last_source) as f64;
                let expected = TAU * bin as f64 / width as f64;
                let frequency = expected + wrap(correlation.arg() - expected * advance) / advance;
                rotations[bin] = wrap(rotations[bin] + frequency * (hop as f64 - advance));
            }
        }
        peaks.clear();
        for bin in 1..bins - 1 {
            if magnitudes[bin] > magnitudes[bin - 1] && magnitudes[bin] >= magnitudes[bin + 1] {
                peaks.push(bin);
            }
        }
        // Identity phase locking: neighboring bins share a peak's rotation.
        let mut peak_index = 0;
        for bin in 1..bins - 1 {
            while peak_index + 1 < peaks.len()
                && bin > (peaks[peak_index] + peaks[peak_index + 1]) / 2
            {
                peak_index += 1;
            }
            let phase = rotations[peaks.get(peak_index).copied().unwrap_or(bin)];
            let rotation = Complex::from_polar(1.0, phase);
            for channel in 0..channels {
                let spectrum = &mut spectra[channel * width..(channel + 1) * width];
                spectrum[bin] *= rotation;
                spectrum[width - bin] = spectrum[bin].conj();
            }
        }
        for channel in 0..channels {
            let spectrum = &mut spectra[channel * width..(channel + 1) * width];
            inverse.process_with_scratch(spectrum, &mut scratch);
            for i in 0..width {
                let position = center + i as i64 - half;
                if (0..frames_out as i64).contains(&position) {
                    let frame = position as usize;
                    accum[frame * channels + channel] += spectrum[i].re * window[i] / width as f64;
                }
            }
        }
        first = false;
        last_source = source;
        center += hop as i64;
    }
    let mut output = buffer(frames_out * channels, 0.0)?;
    for frame in 0..frames_out {
        if frame % 4096 == 0 {
            checkpoint(progress, 0.9 + 0.1 * frame as f32 / frames_out as f32)?;
        }
        if weights[frame % hop] <= 0.0 {
            return Err(Error::NumericalFailure);
        }
        for channel in 0..channels {
            let value = (accum[frame * channels + channel] / weights[frame % hop]) as f32;
            if !value.is_finite() {
                return Err(Error::NumericalFailure);
            }
            output[frame * channels + channel] = value;
        }
    }
    checkpoint(progress, 1.0)?;
    Ok(output)
}

/// Broadband material needs short windows to keep attacks from spreading when expanded.
/// Flatness is geometric / arithmetic spectral power, averaged by frame energy.
/// Use summed channel powers so selection is symmetric and does not cancel anti-phase.
fn analysis_width(
    request: Request,
    input: &[f32],
    width: usize,
    progress: &mut impl FnMut(f32) -> ControlFlow<()>,
) -> Result<usize, Error> {
    let channels = request.channels;
    let frames = input.len() / channels;
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(width);
    let mut scratch = buffer(fft.get_inplace_scratch_len(), Complex::default())?;
    let mut spectrum = buffer(width, Complex::default())?;
    let mut power = buffer(width / 2, 0.0)?;
    let mut weighted_flatness = 0.0;
    let mut total_energy = 0.0;
    for start in (0..frames).step_by(width / 2) {
        checkpoint(progress, 0.05 + 0.1 * start as f32 / frames as f32)?;
        power.fill(0.0);
        for channel in 0..channels {
            for (i, bin) in spectrum.iter_mut().enumerate() {
                let position = start as i64 + i as i64 - (width / 2) as i64;
                let value = if (0..frames as i64).contains(&position) {
                    input[position as usize * channels + channel]
                } else {
                    0.0
                };
                let window = 0.5 - 0.5 * (TAU * i as f64 / width as f64).cos();
                *bin = Complex::new(f64::from(value) * window, 0.0);
            }
            fft.process_with_scratch(&mut spectrum, &mut scratch);
            for (p, bin) in power.iter_mut().zip(&spectrum[1..=width / 2]) {
                *p += bin.norm_sqr();
            }
        }
        let energy: f64 = power.iter().sum();
        if energy > 0.0 {
            let mean = energy / power.len() as f64;
            let flatness = (power
                .iter()
                .map(|p| (p / mean).max(1e-30).ln())
                .sum::<f64>()
                / power.len() as f64)
                .exp();
            weighted_flatness += energy * flatness;
            total_energy += energy;
        }
    }
    Ok(
        if total_energy > 0.0 && weighted_flatness / total_energy > 0.15 {
            (request.sample_rate as usize / 500)
                .next_power_of_two()
                .clamp(64, 512)
        } else {
            width
        },
    )
}
