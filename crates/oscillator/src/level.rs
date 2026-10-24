//! Per-note output stage after summing oscillator sources.
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationInputs, ModulationParameter,
    ModulationRamp, ModulationScope, ModulationSmoother, ProcessingContext, SILENT_GAIN_DB,
};

#[derive(Clone)]
pub struct OscillatorLevel {
    base: ModulationRamp,
    offsets: ModulationSmoother<1>,
    frames: u32,
    effective_gain: f32,
}
impl OscillatorLevel {
    pub fn new(level_db: f32, context: ProcessingContext) -> Self {
        let level_db = if level_db.is_finite() {
            level_db.clamp(SILENT_GAIN_DB, 12.0)
        } else {
            0.0
        };
        let gain = if level_db <= SILENT_GAIN_DB {
            0.0
        } else {
            10_f32.powf(level_db / 20.0)
        };
        Self {
            base: ModulationRamp::new(gain),
            offsets: ModulationSmoother::new(context),
            frames: (context.sample_rate_hz / 200).max(1),
            effective_gain: gain,
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        self.base.set(next.base.target(), self.frames);
    }
    /// Begin a fresh note without changing the base gain transition.
    pub fn note_on(&mut self) {
        self.offsets.reset();
    }
    pub fn reset(&mut self) {
        self.base.reset();
        self.offsets.reset();
        self.effective_gain = self.base.target();
    }
    /// Processed gain before velocity, Amplitude, effects, and layer mixing.
    pub fn effective_gain(&self) -> f32 {
        self.effective_gain
    }
}
impl ModulationDestination for OscillatorLevel {
    type Audio = f32;
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = &[ModulationParameter::Level];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn process_with_modulation(
        &mut self,
        input: f32,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> f32 {
        let [offset] = self
            .offsets
            .process([modulation.offset(ModulationParameter::Level)]);
        let base = self.base.next_value();
        self.effective_gain = if offset == 0.0 {
            base
        } else {
            (base * 10_f32.powf(offset.clamp(-96.0, 96.0) as f32 / 20.0))
                .min(10_f32.powf(12.0 / 20.0))
        };
        input * self.effective_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
        ModulationValue, process_modulation,
    };
    #[test]
    fn invalid_base_values_remain_finite_and_bounded() {
        let context = ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        };
        for db in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1000.0, -2000.0] {
            let mut unit = OscillatorLevel::new(db, context);
            let sample = process_modulation(&mut unit, &mut [], 1.0, &[], &[], Default::default());
            assert!(sample.is_finite() && (0.0..=10_f32.powf(12.0 / 20.0)).contains(&sample));
        }
    }
    #[test]
    fn level_resolves_only_unit_routes_and_preserves_silence_bounds_cancellation_and_reset() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = daw_unit_core::OscillatorSettings::default();
            let id = ModulationSourceId {
                layer: settings.id,
                unit: settings.id,
            };
            let value = ModulationValue {
                source: id,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Continuous,
                release_age: None,
            };
            let mut route = Modulation::new(ModulationParameter::Level);
            route.source = Some(id);
            route.amount = 24.0;
            for db in [-9.0, SILENT_GAIN_DB] {
                for shared in [false, true] {
                    let mut unit = OscillatorLevel::new(db, context);
                    let mut routes = [ModulationRoute::new(route, rate)];
                    for _ in 0..rate / 20 {
                        let (local, bus) = if shared {
                            (&[][..], &[value][..])
                        } else {
                            (&[value][..], &[][..])
                        };
                        let output = process_modulation(
                            &mut unit,
                            &mut routes,
                            1.0,
                            local,
                            bus,
                            Default::default(),
                        );
                        assert!(output.is_finite() && output <= 10_f32.powf(12.0 / 20.0));
                        if db == SILENT_GAIN_DB {
                            assert_eq!(output, 0.0);
                        }
                    }
                    if db != SILENT_GAIN_DB {
                        assert_eq!(unit.effective_gain(), 10_f32.powf(12.0 / 20.0));
                    }
                    unit.reset();
                    let mut opposite = route;
                    opposite.amount = -24.0;
                    let mut cancel = [
                        ModulationRoute::new(route, rate),
                        ModulationRoute::new(opposite, rate),
                    ];
                    let base = if db == SILENT_GAIN_DB {
                        0.0
                    } else {
                        10_f32.powf(db / 20.0)
                    };
                    assert_eq!(
                        process_modulation(
                            &mut unit,
                            &mut cancel,
                            0.25,
                            &[value],
                            &[],
                            Default::default()
                        ),
                        0.25 * base
                    );
                    let mut wrong = route;
                    wrong.target = Some(settings.id);
                    let mut routes = [ModulationRoute::new(wrong, rate)];
                    assert_eq!(
                        process_modulation(
                            &mut unit,
                            &mut routes,
                            1.0,
                            &[value],
                            &[],
                            Default::default()
                        ),
                        base
                    );
                }
            }
        }
    }
}
