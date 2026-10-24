//! Shared per-note output controls: common source tuning and post-sum Level.
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationInputs, ModulationParameter,
    ModulationRamp, ModulationScope, ModulationSmoother, ProcessingContext, SILENT_GAIN_DB,
};

#[derive(Clone)]
pub struct OscillatorLevel {
    base: ModulationRamp,
    offsets: ModulationSmoother<2>,
    pitch: ModulationRamp,
    effective_pitch: f32,
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
            pitch: ModulationRamp::new(0.0),
            effective_pitch: 0.0,
            frames: (context.sample_rate_hz / 200).max(1),
            effective_gain: gain,
        }
    }
    /// Prepare a common base transpose; zero preserves the original output exactly.
    pub fn with_pitch(mut self, semitones: f32) -> Self {
        let pitch = if semitones.is_finite() {
            semitones.clamp(-48.0, 48.0)
        } else {
            0.0
        };
        self.pitch = ModulationRamp::new(pitch);
        self.effective_pitch = pitch;
        self
    }
    /// Last processed common transpose; excludes source transpose and source modulation.
    pub fn effective_pitch(&self) -> f32 {
        self.effective_pitch
    }
    /// One prepared frequency multiplier, reused by every source in this note.
    pub fn pitch_ratio(&self) -> f32 {
        if self.effective_pitch == 0.0 {
            1.0
        } else {
            2_f32.powf(self.effective_pitch / 12.0)
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        self.base.set(next.base.target(), self.frames);
        self.pitch.set(next.pitch.target(), self.frames);
    }
    /// Begin a fresh note without changing the base gain transition.
    pub fn note_on(&mut self) {
        self.offsets.reset();
    }
    pub fn reset(&mut self) {
        self.base.reset();
        self.pitch.reset();
        self.effective_pitch = self.pitch.target();
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
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &[ModulationParameter::Level, ModulationParameter::Pitch];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn process_with_modulation(
        &mut self,
        input: f32,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> f32 {
        let [offset, pitch_offset] = self.offsets.process([
            modulation.offset(ModulationParameter::Level),
            modulation.offset(ModulationParameter::Pitch),
        ]);
        self.effective_pitch = self.pitch.next_value() + pitch_offset.clamp(-96.0, 96.0) as f32;
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
    fn common_pitch_resolves_shared_and_local_routes_and_smooths_retargets() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let id = ModulationSourceId {
                layer: daw_unit_core::OscillatorSettings::default().id,
                unit: daw_unit_core::OscillatorSettings::default().id,
            };
            let value = ModulationValue {
                source: id,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Continuous,
                release_age: None,
            };
            let mut route = Modulation::new(ModulationParameter::Pitch);
            route.source = Some(id);
            route.amount = 12.0;
            for shared in [false, true] {
                let mut unit = OscillatorLevel::new(0.0, context).with_pitch(-12.0);
                assert_eq!(unit.pitch_ratio(), 0.5);
                let mut routes = [ModulationRoute::new(route, rate)];
                for _ in 0..rate / 20 {
                    let (local, bus) = if shared {
                        (&[][..], &[value][..])
                    } else {
                        (&[value][..], &[][..])
                    };
                    assert_eq!(
                        process_modulation(
                            &mut unit,
                            &mut routes,
                            0.25,
                            local,
                            bus,
                            Default::default()
                        ),
                        0.25
                    );
                }
                assert!(unit.effective_pitch().abs() < 0.001);
                assert!((unit.pitch_ratio() - 1.0).abs() < 0.001);
                unit.retarget(&OscillatorLevel::new(0.0, context).with_pitch(12.0));
                for _ in 0..rate / 20 {
                    process_modulation(
                        &mut unit,
                        &mut routes,
                        0.0,
                        &[value],
                        &[],
                        Default::default(),
                    );
                }
                assert!((unit.effective_pitch() - 24.0).abs() < 0.001);
                unit.reset();
                assert_eq!(unit.effective_pitch(), 12.0);
                assert_eq!(unit.pitch_ratio(), 2.0);
                let mut wrong = route;
                wrong.target = Some(id.unit);
                let mut wrong_routes = [ModulationRoute::new(wrong, rate)];
                process_modulation(
                    &mut unit,
                    &mut wrong_routes,
                    1.0,
                    &[value],
                    &[],
                    Default::default(),
                );
                assert_eq!(unit.effective_pitch(), 12.0);
            }
            for pitch in [f32::NAN, f32::INFINITY, -1000.0, 1000.0] {
                let unit = OscillatorLevel::new(0.0, context).with_pitch(pitch);
                assert!(unit.effective_pitch().is_finite());
                assert!((-48.0..=48.0).contains(&unit.effective_pitch()));
            }
        }
    }
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
