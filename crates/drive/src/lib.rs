//! Stereo soft-clipping drive with pre-clipping color and post-clipping tone.
//! FunDSP filter types stay private; callback operations do not allocate or block.
use daw_unit_core::{
    DRIVE_TYPE_ID, DriveSettings, EffectUnit, PortKind, ProcessingContext, UnitDescriptor,
};
use fundsp::{audionode::AudioNode, svf::*};

struct Ramp {
    value: f32,
    target: f32,
    remaining: u32,
}
impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f32, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn advance(&mut self) {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / self.remaining as f32;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.value = self.target;
            }
        }
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}

type Bell = FixedSvf<f64, BellMode<f64>>;
type Bass = FixedSvf<f64, LowshelfMode<f64>>;
type Tone = FixedSvf<f64, HighshelfMode<f64>>;

pub struct Drive {
    // Each color branch and channel has independent history, including inactive branches.
    bells: [[Bell; 2]; 3],
    bass: [Bass; 2],
    tone_filter: [Tone; 2],
    gain: Ramp,
    tone: Ramp,
    volume: Ramp,
    strength: Ramp,
    frequency_log: Ramp,
    colors: [Ramp; 4],
    mix: Ramp,
    fade_frames: u32,
    configured: [f32; 4],
    clipping: f32,
    normalization: f32,
    external: daw_unit_core::ModulationSmoother<5>,
    effective: [f32; 5],
}
fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}
impl Drive {
    /// Prepare on a control thread with validated settings and a supported sample rate.
    pub fn new(context: ProcessingContext, settings: &DriveSettings) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(context.sample_rate_hz),
            cutoff: f64::from(settings.color_frequency_hz),
            q: 0.65,
            gain: 1.0,
        };
        let mut drive = Self {
            bells: std::array::from_fn(|_| {
                std::array::from_fn(|_| Bell::new(BellMode::new(), &params))
            }),
            bass: std::array::from_fn(|_| Bass::new(LowshelfMode::new(), &params)),
            tone_filter: std::array::from_fn(|_| Tone::new(HighshelfMode::new(), &params)),
            gain: Ramp::new(settings.gain),
            tone: Ramp::new(settings.tone),
            volume: Ramp::new(amplitude(settings.volume_db)),
            strength: Ramp::new(settings.color_strength),
            frequency_log: Ramp::new(settings.color_frequency_hz.ln()),
            colors: std::array::from_fn(|index| {
                Ramp::new(f32::from(index == settings.color_shape.index()))
            }),
            mix: Ramp::new(f32::from(settings.enabled)),
            fade_frames: (context.sample_rate_hz / 200).max(1),
            configured: [f32::NAN; 4],
            clipping: 0.0,
            normalization: 1.0,
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: [
                amplitude(settings.volume_db),
                settings.gain,
                settings.tone,
                settings.color_strength,
                settings.color_frequency_hz,
            ],
        };
        drive.configure(drive.base_parameters());
        drive
    }
    /// Preserve filter history and any unrelated in-flight ramps on live edits.
    pub fn retarget(&mut self, settings: &DriveSettings) {
        let frames = self.fade_frames;
        self.gain.retarget(settings.gain, frames);
        self.tone.retarget(settings.tone, frames);
        self.volume.retarget(amplitude(settings.volume_db), frames);
        self.strength.retarget(settings.color_strength, frames);
        self.frequency_log
            .retarget(settings.color_frequency_hz.ln(), frames);
        for (index, color) in self.colors.iter_mut().enumerate() {
            color.retarget(f32::from(index == settings.color_shape.index()), frames);
        }
        self.mix.retarget(f32::from(settings.enabled), frames);
    }
    fn base_parameters(&self) -> [f32; 4] {
        [
            self.gain.value,
            self.tone.value,
            self.strength.value,
            self.frequency_log.value,
        ]
    }
    fn configure(&mut self, values: [f32; 4]) {
        if values == self.configured {
            return;
        }
        self.effective[1..].copy_from_slice(&[values[0], values[1], values[2], values[3].exp()]);
        if values[0] != self.configured[0] {
            // Zero is clean. Full gain supplies 36 dB of drive into a symmetric soft clipper.
            self.clipping = 10_f32.powf(1.8 * values[0]) - 1.0;
            self.normalization = if self.clipping < 1e-4 {
                1.0
            } else {
                self.clipping.tanh().recip()
            };
        }
        if values[1] != self.configured[1] {
            let gain = f64::from(amplitude((values[1] - 0.5) * 24.0));
            for filter in &mut self.tone_filter {
                filter.set_cutoff_q_gain(2500.0, std::f64::consts::FRAC_1_SQRT_2, gain);
            }
        }
        if values[2..] != self.configured[2..] {
            let frequency = f64::from(values[3].exp());
            for (index, filters) in self.bells.iter_mut().enumerate() {
                let db = values[2] * if index == 0 { -12.0 } else { 12.0 };
                for filter in filters {
                    filter.set_cutoff_q_gain(frequency, 0.65, f64::from(amplitude(db)));
                }
            }
            for filter in &mut self.bass {
                filter.set_cutoff_q_gain(
                    frequency * 0.5,
                    std::f64::consts::FRAC_1_SQRT_2,
                    f64::from(amplitude(-18.0 * values[2])),
                );
            }
        }
        self.configured = values;
    }
    fn ramps(&mut self) -> impl Iterator<Item = &mut Ramp> {
        [
            &mut self.gain,
            &mut self.tone,
            &mut self.volume,
            &mut self.strength,
            &mut self.frequency_log,
            &mut self.mix,
        ]
        .into_iter()
        .chain(self.colors.iter_mut())
    }
}
impl EffectUnit for Drive {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: DRIVE_TYPE_ID,
            name: "Drive",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for ramp in self.ramps() {
            ramp.reset();
        }
        for filter in self.bells.iter_mut().flatten() {
            filter.reset();
        }
        for filter in &mut self.bass {
            filter.reset();
        }
        for filter in &mut self.tone_filter {
            filter.reset();
        }
        self.external.reset();
        self.effective[0] = self.volume.value;
        self.configure(self.base_parameters());
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 5])
    }
}
impl Drive {
    /// Last processed Volume dB, Gain, Tone, Color strength, and Color frequency Hz.
    pub fn effective_parameters(&self) -> [f32; 5] {
        let mut values = self.effective;
        values[0] = if values[0] > 0.0 {
            20.0 * values[0].log10()
        } else {
            daw_unit_core::SILENT_GAIN_DB
        };
        values
    }
    /// Offsets in the same order: dB, normalized fractions, and octaves for Frequency.
    /// Sources and route amounts belong to the host; this portable stage smooths offsets by 1 ms.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 5]) -> [f32; 2] {
        let offsets = self.external.process(offsets);
        for ramp in self.ramps() {
            ramp.advance();
        }
        let offsets = offsets.map(|v| v as f32);
        let [gain, tone, strength, frequency_log] = self.base_parameters();
        self.configure([
            (gain + offsets[1]).clamp(0.0, 1.0),
            (tone + offsets[2]).clamp(0.0, 1.0),
            (strength + offsets[3]).clamp(0.0, 1.0),
            (frequency_log + offsets[4] * std::f32::consts::LN_2)
                .clamp(250.0_f32.ln(), 2000.0_f32.ln()),
        ]);
        let volume = if offsets[0] == 0.0 {
            self.volume.value
        } else {
            (self.volume.value * amplitude(offsets[0].clamp(-96.0, 96.0))).min(amplitude(12.0))
        };
        self.effective[0] = volume;
        std::array::from_fn(|channel| {
            let dry = input[channel];
            let bass = self.bass[channel].filter_mono(dry);
            let branches = [
                dry,
                self.bells[0][channel].filter_mono(dry),
                self.bells[1][channel].filter_mono(dry),
                self.bells[2][channel].filter_mono(bass),
            ];
            let color = branches
                .into_iter()
                .zip(&self.colors)
                .map(|(sample, weight)| sample * weight.value)
                .sum::<f32>();
            let clipped = if self.clipping < 1e-4 {
                color
            } else {
                (color * self.clipping).tanh() * self.normalization
            };
            let wet = self.tone_filter[channel].filter_mono(clipped) * volume;
            dry + (wet - dry) * self.mix.value
        })
    }
}

impl daw_unit_core::ModulationDestination for Drive {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [daw_unit_core::ModulationParameter] = <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: daw_unit_core::ModulationScope = <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &daw_unit_core::ModulationInputs,
        _context: daw_unit_core::ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(daw_unit_core::ModulationParameter::DRIVE),
        )
    }
}

#[cfg(test)]
mod tests;
