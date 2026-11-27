//! PCM playback calculations over caller-owned audio.

fn read(samples: &[[f32; 2]], position: u64, fraction: f64) -> [f32; 2] {
    let mut sample = samples.get(position as usize).copied().unwrap_or_default();
    if fraction != 0.0 {
        let next = samples
            .get(position as usize + 1)
            .copied()
            .unwrap_or(sample);
        for (value, next) in sample.iter_mut().zip(next) {
            if value.is_finite() && next.is_finite() {
                *value =
                    (f64::from(*value) + (f64::from(next) - f64::from(*value)) * fraction) as f32;
            }
        }
    }
    sample
}

fn edge(
    elapsed_frames: u64,
    length: usize,
    position: u64,
    pitch_ratio: f64,
    fade_frames: u32,
) -> f32 {
    // Five-millisecond boundaries keep region edits and one-shot ends free of hard cuts.
    (elapsed_frames as f32 / fade_frames as f32)
        .min((length as u64 - position - 1) as f32 / pitch_ratio as f32 / fade_frames as f32)
        .clamp(0.0, 1.0)
}

fn advance_pitch(ratio: &mut f64, target: f64, remaining: &mut u32) {
    if *remaining > 0 {
        *ratio += (target - *ratio) / f64::from(*remaining);
        *remaining -= 1;
    }
}

fn advance_position(position: u64, fraction: f64, pitch_ratio: f64) -> (u64, f64) {
    let position = position as f64 + fraction + pitch_ratio;
    (position.floor() as u64, position.fract())
}

fn apply_gain(sample: [f32; 2], gain: f32, edge: f32) -> [f32; 2] {
    sample.map(|v| {
        if v.is_finite() {
            (f64::from(v) * f64::from(gain) * f64::from(edge))
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        } else {
            0.0
        }
    })
}

fn crossfade(previous: [f32; 2], output: [f32; 2], remaining: u32, fade_frames: u32) -> [f32; 2] {
    let mix = 1.0 - remaining as f32 / fade_frames as f32;
    std::array::from_fn(|ch| previous[ch] + (output[ch] - previous[ch]) * mix)
}

/// Fractional cursor and crossfade state. The caller owns the selected PCM.
#[derive(Clone)]
pub struct Playback {
    position: u64,
    elapsed_frames: u64,
    fraction: f64,
    pitch_ratio: f64,
    pitch_target: f64,
    pitch_remaining: u32,
    previous_sample: [f32; 2],
    transition_from: [f32; 2],
    transition_remaining: u32,
    active: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            position: 0,
            elapsed_frames: 0,
            fraction: 0.0,
            pitch_ratio: 1.0,
            pitch_target: 1.0,
            pitch_remaining: 0,
            previous_sample: [0.0; 2],
            transition_from: [0.0; 2],
            transition_remaining: 0,
            active: false,
        }
    }
}

impl Playback {
    pub fn restart(&mut self, active: bool) {
        *self = Self {
            active,
            ..Self::default()
        };
    }

    pub fn constrain(&mut self, length: usize) {
        self.active &= self.position < length as u64;
    }

    pub fn begin_transition(&mut self, frames: u32) {
        self.transition_from = self.previous_sample;
        self.transition_remaining = frames;
    }

    pub fn retarget_pitch(&mut self, ratio: f64, frames: u32) {
        self.pitch_target = ratio;
        self.pitch_remaining = frames;
    }

    pub fn scale_pitch(&mut self, scale: f64) {
        self.pitch_ratio *= scale;
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn position(&self) -> u64 {
        self.position
    }

    pub fn pitch_ratio(&self) -> f64 {
        self.pitch_ratio
    }

    pub fn advance(&mut self, frames: u64, length: usize) {
        if self.active {
            self.elapsed_frames = self.elapsed_frames.saturating_add(frames);
            self.position = self.position.saturating_add(frames).min(length as u64);
        }
        self.constrain(length);
    }

    pub fn sample(&mut self, samples: &[[f32; 2]], gain: f32, fade_frames: u32) -> [f32; 2] {
        if !self.active {
            return [0.0; 2];
        }
        let sample = read(samples, self.position, self.fraction);
        let fade = edge(
            self.elapsed_frames,
            samples.len(),
            self.position,
            self.pitch_ratio,
            fade_frames,
        );
        self.elapsed_frames = self.elapsed_frames.saturating_add(1);
        advance_pitch(
            &mut self.pitch_ratio,
            self.pitch_target,
            &mut self.pitch_remaining,
        );
        (self.position, self.fraction) =
            advance_position(self.position, self.fraction, self.pitch_ratio);
        self.constrain(samples.len());
        let mut output = apply_gain(sample, gain, fade);
        if self.transition_remaining > 0 {
            output = crossfade(
                self.transition_from,
                output,
                self.transition_remaining,
                fade_frames,
            );
            self.transition_remaining -= 1;
        }
        self.previous_sample = output;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_pitch_boundary_fade_uses_output_frames() {
        let samples = [[-1.0; 2]; 4800];
        let mut playback = Playback {
            position: 4800 - 240,
            elapsed_frames: 1000,
            pitch_ratio: 2.0,
            active: true,
            ..Playback::default()
        };
        assert!(playback.sample(&samples, 1.0, 240)[0].abs() < 0.51);
    }
}
