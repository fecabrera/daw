//! Drive filters, normalized clipping, and the distinct output mixes for both units.
use crate::filters::drive::DriveFilters;
use crate::gain::SILENT_GAIN_DB;
use crate::waveshaping::NormalizedClipper;

#[derive(Clone)]
pub struct DriveKernel {
    filters: DriveFilters,
    clipper: NormalizedClipper,
    configured: [f32; 4],
}

impl DriveKernel {
    pub fn new(rate: u32, frequency_hz: f32) -> Self {
        Self {
            filters: DriveFilters::new(rate, frequency_hz),
            clipper: NormalizedClipper::new(0.0),
            configured: [f32::NAN; 4],
        }
    }

    /// Configure Gain, Tone, Color strength, and log frequency.
    pub fn configure(&mut self, values: [f32; 4]) -> bool {
        if values == self.configured {
            return false;
        }
        if values[0] != self.configured[0] {
            // Zero is clean. Full gain supplies approximately 36 dB of drive.
            self.clipper = NormalizedClipper::new(10_f32.powf(1.8 * values[0]) - 1.0);
        }
        self.filters.configure(values, self.configured);
        self.configured = values;
        true
    }

    pub fn sample(&mut self, input: [f32; 2], colors: [f32; 4], hard: f32, tube: f32) -> [f32; 2] {
        std::array::from_fn(|channel| {
            let tone = self.filters.sample(channel, input[channel], colors);
            self.clipper.process(tone, hard, tube)
        })
    }

    pub fn reset(&mut self) {
        self.filters.reset();
    }
}

pub fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

pub fn modulated_gain(base: f32, offset: f32) -> f32 {
    if offset == 0.0 {
        base
    } else {
        (base * amplitude(offset.clamp(-96.0, 96.0))).min(amplitude(12.0))
    }
}

pub fn gain_db(value: f32) -> f32 {
    if value > 0.0 {
        20.0 * value.log10()
    } else {
        SILENT_GAIN_DB
    }
}

pub fn output(dry: [f32; 2], clipped: [f32; 2], level: f32, enabled: f32) -> [f32; 2] {
    std::array::from_fn(|i| {
        let wet = clipped[i] * level;
        dry[i] + (wet - dry[i]) * enabled
    })
}

fn blend_sample(dry: f32, wet: f32, amount: f32) -> f32 {
    if amount == 0.0 {
        dry
    } else if amount == 1.0 {
        wet
    } else {
        dry + (wet - dry) * amount
    }
}

pub fn output_v2(
    input: [f32; 2],
    dry: [f32; 2],
    wet: [f32; 2],
    level: f32,
    blend: f32,
    enabled: f32,
) -> [f32; 2] {
    std::array::from_fn(|i| {
        // Full wet retains the original Drive's output-level arithmetic exactly.
        let mixed = if blend == 1.0 {
            dry[i] + (wet[i] * level - dry[i])
        } else {
            blend_sample(dry[i], wet[i], blend) * level
        };
        blend_sample(input[i], mixed, enabled)
    })
}
