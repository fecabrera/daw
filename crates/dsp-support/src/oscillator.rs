//! Oscillator nodes and unison tuning over caller-provided frequencies.
use crate::ramps::ModulationRamp;
use fundsp::prelude32::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[derive(Clone)]
pub struct OscillatorBank {
    nodes: Vec<Box<dyn AudioUnit>>,
    waveform: Waveform,
    controls: [ModulationRamp; 3],
    tuning: [f32; 8],
    pans: [[f32; 2]; 8],
    last_controls: [f32; 3],
    sample_rate_hz: u32,
}

impl OscillatorBank {
    pub fn heap_bytes(&self) -> u64 {
        self.nodes
            .iter()
            .map(|node| node.footprint() as u64)
            .sum::<u64>()
            + crate::memory::vec_bytes(&self.nodes)
    }

    fn prepare_node(waveform: Waveform, sample_rate: u32) -> Box<dyn AudioUnit> {
        let mut node: Box<dyn AudioUnit> = match waveform {
            Waveform::Sine => Box::new(sine()),
            Waveform::Saw => Box::new(saw()),
            Waveform::Square => Box::new(square()),
            Waveform::Triangle => Box::new(triangle()),
        };
        node.set_sample_rate(f64::from(sample_rate));
        node.allocate();
        node.reset();
        node
    }

    pub fn new(waveform: Waveform, sample_rate_hz: u32) -> Self {
        Self {
            nodes: vec![Self::prepare_node(waveform, sample_rate_hz)],
            waveform,
            controls: [0.0, 10.0, 100.0].map(ModulationRamp::new),
            tuning: [1.0; 8],
            pans: [[1.0; 2]; 8],
            last_controls: [0.0, 10.0, 100.0],
            sample_rate_hz,
        }
    }

    /// Prepare the requested unison bank before handing it to the audio thread.
    pub fn prepare_voices(&mut self, count: usize, cents: f32, detune: f32, width: f32) {
        let count = count.clamp(1, self.tuning.len());
        self.nodes.truncate(count);
        for index in self.nodes.len()..count {
            let mut node = Self::prepare_node(self.waveform, self.sample_rate_hz);
            node.set_hash((index as u64).wrapping_mul(0x9e3779b97f4a7c15));
            self.nodes.push(node);
        }
        let finite = |value: f32, fallback: f32, min: f32, max: f32| {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                fallback
            }
        };
        self.controls = [
            finite(cents, 0.0, -100.0, 100.0),
            finite(detune, 10.0, 0.0, 100.0),
            finite(width, 100.0, 0.0, 100.0),
        ]
        .map(ModulationRamp::new);
        self.last_controls = [f32::NAN; 3];
        self.update_tuning([0.0; 2]);
    }

    pub fn compatible(&self, next: &Self) -> bool {
        self.waveform == next.waveform
            && self.sample_rate_hz == next.sample_rate_hz
            && self.nodes.len() == next.nodes.len()
    }

    pub fn retarget(&mut self, next: &Self) -> bool {
        if !self.compatible(next) {
            return false;
        }
        let frames = std::cmp::max(self.sample_rate_hz / 200, 1);
        for (current, target) in self.controls.iter_mut().zip(&next.controls) {
            current.set(target.target(), frames);
        }
        true
    }

    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            node.reset();
        }
        for control in &mut self.controls {
            control.reset();
        }
        self.last_controls = [f32::NAN; 3];
        self.update_tuning([0.0; 2]);
    }

    pub fn cents(&self) -> f32 {
        self.last_controls[0]
    }

    pub fn width(&self) -> f32 {
        self.last_controls[2]
    }

    fn update_tuning(&mut self, offsets: [f32; 2]) {
        let mut controls = self.controls.each_mut().map(ModulationRamp::next_value);
        controls[0] = (controls[0] + offsets[0]).clamp(-100.0, 100.0);
        controls[2] = (controls[2] + offsets[1]).clamp(0.0, 100.0);
        if controls == self.last_controls {
            return;
        }
        self.last_controls = controls;
        let count = self.nodes.len();
        for index in 0..count {
            let position = if count == 1 {
                0.0
            } else {
                2.0 * index as f32 / (count - 1) as f32 - 1.0
            };
            let cents = controls[0] + position * controls[1];
            self.tuning[index] = if cents == 0.0 {
                1.0
            } else {
                2_f32.powf(cents / 1200.0)
            };
            let pan = position * controls[2] / 100.0;
            self.pans[index] = [1.0 - pan, 1.0 + pan];
        }
    }
    pub fn sample(&mut self, frequency: f32, offsets: [f32; 2]) -> [f32; 2] {
        self.update_tuning(offsets);
        let mut sum = [0.0; 2];
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let hz = if self.tuning[index] == 1.0 {
                frequency
            } else {
                frequency * self.tuning[index]
            };
            let enabled = hz.is_finite() && hz > 0.0 && hz < self.sample_rate_hz as f32 * 0.5;
            let mut output = [0.0];
            node.tick(&[if enabled { hz } else { 0.0 }], &mut output);
            let sample = if enabled { output[0] } else { 0.0 };
            for (channel, value) in sum.iter_mut().enumerate() {
                *value += sample * self.pans[index][channel];
            }
        }
        let scale = 1.0 / self.nodes.len() as f32;
        sum.map(|v| v * scale)
    }
}

pub fn note_frequency(pitch: u8, transpose: i8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(pitch) + f32::from(transpose) - 69.0) / 12.0)
}
