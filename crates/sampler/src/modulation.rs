//! Sampler-owned gain conversion over the shared resolver and additive smoother.
use super::Sampler;
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, SamplerSettings,
};

impl Sampler {
    /// Last processed linear gain, including power/base ramps; excludes boundary fades and note shaping.
    pub fn effective_gain(&self) -> f32 {
        self.effective_gain
    }
    pub(super) fn process_modulated(&mut self, offset_db: f32) -> [f32; 2] {
        if self.remaining > 0 {
            self.gain += (self.target - self.gain) / self.remaining as f32;
            self.remaining -= 1;
        }
        let [offset] = self.external.process([offset_db]);
        // Preserve neutral arithmetic and exact base silence. Limit summed offsets before conversion.
        self.effective_gain = if offset == 0.0 {
            self.gain
        } else {
            (self.gain * 10_f32.powf(offset.clamp(-96.0, 96.0) as f32 / 20.0))
                .min(10_f32.powf(12.0 / 20.0))
        };
        self.render_sample()
    }
}
impl ModulationDestination for Sampler {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        SamplerSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = SamplerSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        _: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(modulation.offset(ModulationParameter::SamplerGain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
        ModulationValue, ProcessingContext, process_modulation,
    };
    use std::sync::Arc;
    #[test]
    fn gain_routes_share_scope_summing_smoothing_bounds_reset_and_neutral_audio() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = SamplerSettings::default();
            let samples = Arc::new(vec![[0.4, -0.2]; rate as usize]);
            let mut unit = Sampler::new(context, &settings, samples.clone());
            let mut reference = Sampler::new(context, &settings, samples.clone());
            assert!(unit.descriptor().consumes_modulation());
            assert_eq!(
                Sampler::MODULATION_PARAMETERS,
                SamplerSettings::MODULATION_PARAMETERS
            );
            assert_eq!(Sampler::MODULATION_SCOPE, ModulationScope::PerNote);
            let source = ModulationSourceId {
                layer: settings.id,
                unit: settings.id,
            };
            let value = ModulationValue {
                source,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Contour,
                release_age: None,
            };
            let route = |amount| {
                let mut r = Modulation::new(ModulationParameter::SamplerGain);
                r.source = Some(source);
                r.amount = amount;
                ModulationRoute::new(r, rate)
            };
            let mut cancelled = [route(24.0), route(-24.0)];
            unit.note_on();
            reference.note_on();
            for _ in 0..rate / 100 {
                assert_eq!(
                    process_modulation(
                        &mut unit,
                        &mut cancelled,
                        [0.0; 2],
                        &[value],
                        &[],
                        Default::default()
                    ),
                    reference.next_sample()
                );
            }
            // Both local contours and shared continuous controls use the same typed input.
            for local in [true, false] {
                unit.reset();
                unit.note_on();
                unit.advance(u64::from(rate / 100));
                let mut routes = [route(-12.0)];
                let continuous = ModulationValue {
                    behavior: ModulationBehavior::Continuous,
                    ..value
                };
                let (local_values, shared_values) = if local {
                    (&[value][..], &[][..])
                } else {
                    (&[][..], &[continuous][..])
                };
                let mut previous = 1.0;
                for _ in 0..rate / 100 {
                    let audio = process_modulation(
                        &mut unit,
                        &mut routes,
                        [0.0; 2],
                        local_values,
                        shared_values,
                        Default::default(),
                    );
                    assert!(
                        unit.effective_gain() <= previous
                            && unit.effective_gain() >= 10_f32.powf(-12.0 / 20.0)
                    );
                    assert_eq!(audio[0], audio[1] * -2.0);
                    previous = unit.effective_gain();
                }
                assert!((20.0 * unit.effective_gain().log10() + 12.0).abs() < 0.001);
            }
            let mut positive: Vec<_> = (0..8).map(|_| route(24.0)).collect();
            for _ in 0..rate / 100 {
                process_modulation(
                    &mut unit,
                    &mut positive,
                    [0.0; 2],
                    &[],
                    &[value],
                    Default::default(),
                );
            }
            assert!((20.0 * unit.effective_gain().log10() - 12.0).abs() < 0.001);
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                for _ in 0..rate / 100 {
                    assert!(unit.process_modulated(bad).iter().all(|v| v.is_finite()));
                }
            }
            for settings in [
                SamplerSettings {
                    enabled: false,
                    ..Default::default()
                },
                SamplerSettings {
                    gain_db: daw_unit_core::SILENT_GAIN_DB,
                    ..Default::default()
                },
            ] {
                unit.retarget(&Sampler::new(context, &settings, samples.clone()));
                unit.reset();
                unit.note_on();
                unit.advance(u64::from(rate / 100));
                for _ in 0..rate / 100 {
                    assert_eq!(
                        process_modulation(
                            &mut unit,
                            &mut positive,
                            [0.0; 2],
                            &[],
                            &[value],
                            Default::default()
                        ),
                        [0.0; 2]
                    );
                }
            }
            assert_eq!(samples[100], [0.4, -0.2]);
        }
    }
}
