use daw_core::{EnvelopeInstance, EnvelopeMode, ModulationSourceId, ModulationValue};
use daw_envelope::Envelope;
pub(crate) use daw_unit_core::ModulationRoute as Route;
use daw_unit_core::{ModulationOutput, ProcessingContext};
pub(crate) struct Control {
    pub source: ModulationSourceId,
    pub mode: EnvelopeMode,
    pub envelope: Envelope,
    output: ModulationOutput,
}
impl Control {
    pub fn new(
        source: ModulationSourceId,
        settings: &EnvelopeInstance,
        context: ProcessingContext,
    ) -> Self {
        Self {
            source,
            mode: settings.mode,
            envelope: Envelope::new(settings.settings, context),
            output: ModulationOutput::new(source, settings.present && settings.enabled, context),
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        self.envelope.set_settings(next.envelope.settings());
        self.output.retarget(&next.output);
    }
    pub fn reset(&mut self) {
        self.envelope.reset();
        self.output.reset();
    }
    pub fn tick(&mut self) -> ModulationValue {
        self.output.sample(&mut self.envelope)
    }
}

pub(crate) struct SharedControls {
    pub controls: Vec<Control>,
    pub values: Vec<ModulationValue>,
    held: usize,
    frame: u64,
    release_frames: u64,
}
impl SharedControls {
    pub fn new(controls: Vec<Control>) -> Self {
        let values = controls
            .iter()
            .map(|e| ModulationValue {
                source: e.source,
                value: 0.0,
                weight: 0.0,
                release_age: None,
            })
            .collect();
        Self {
            controls,
            values,
            held: 0,
            frame: 0,
            release_frames: 0,
        }
    }
    pub fn reset(&mut self) {
        for e in &mut self.controls {
            e.reset();
        }
        for v in &mut self.values {
            v.value = 0.0;
            v.weight = 0.0;
        }
        self.held = 0;
        self.frame = 0;
        self.release_frames = 0;
    }
    pub fn end(&mut self) {
        self.held = 0;
        self.release_frames = 0;
        for e in &mut self.controls {
            e.envelope.note_off();
        }
    }
    pub fn advance_to(&mut self, frame: u64) {
        if self.held == 0 {
            self.release_frames = self
                .release_frames
                .saturating_add(frame.saturating_sub(self.frame));
        }
        for e in &mut self.controls {
            e.envelope.advance(frame.saturating_sub(self.frame));
        }
        self.frame = frame;
    }
    pub fn event(&mut self, on: bool) {
        if on {
            for e in &mut self.controls {
                if self.held == 0 || e.mode == EnvelopeMode::Retrigger {
                    e.envelope.note_on();
                }
            }
            self.held += 1;
        } else {
            self.held = self.held.saturating_sub(1);
            if self.held == 0 {
                self.release_frames = 0;
                for e in &mut self.controls {
                    e.envelope.note_off();
                }
            }
        }
    }
    pub fn tick(&mut self) {
        if self.held == 0 {
            self.release_frames = self.release_frames.saturating_add(1);
        }
        for (e, v) in self.controls.iter_mut().zip(&mut self.values) {
            *v = e.tick();
            if self.held == 0 {
                v.release_age = Some(self.release_frames);
            }
        }
    }
    pub fn retarget(&mut self, next: &mut Self) {
        for n in &mut next.controls {
            if let Some(e) = self.controls.iter_mut().find(|e| e.source == n.source) {
                e.retarget(n);
                std::mem::swap(e, n);
            } else if self.held > 0 {
                n.envelope.note_on();
            }
        }
        for v in &mut next.values {
            if let Some(old) = self.values.iter().find(|old| old.source == v.source) {
                *v = *old;
            }
        }
        std::mem::swap(&mut self.controls, &mut next.controls);
        std::mem::swap(&mut self.values, &mut next.values);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::Modulation;
    use daw_unit_core::{CurveShape, EnvelopeSettings, EnvelopeStageSettings};
    fn shared(mode: EnvelopeMode) -> SharedControls {
        let e = EnvelopeInstance {
            mode,
            settings: EnvelopeSettings {
                attack: EnvelopeStageSettings {
                    time_ms: 100.0,
                    shape: CurveShape::Linear,
                    amount: 0.5,
                },
                decay: EnvelopeStageSettings {
                    time_ms: 0.0,
                    shape: CurveShape::Linear,
                    amount: 0.5,
                },
                sustain: 1.0,
                release: EnvelopeStageSettings {
                    time_ms: 100.0,
                    shape: CurveShape::Linear,
                    amount: 0.5,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        SharedControls::new(vec![Control::new(
            ModulationSourceId {
                layer: daw_core::Id::from_u128(1),
                unit: e.id,
            },
            &e,
            ProcessingContext {
                sample_rate_hz: 1000,
                max_block_frames: 32,
            },
        )])
    }
    #[test]
    fn shared_retrigger_and_legato_count_held_notes_and_release_after_the_last_off() {
        let mut retrigger = shared(EnvelopeMode::Retrigger);
        let mut legato = shared(EnvelopeMode::Legato);
        for s in [&mut retrigger, &mut legato] {
            s.event(true);
            for _ in 0..25 {
                s.tick();
            }
            assert!((s.values[0].value - 0.25).abs() < 1e-6);
            s.event(true);
            s.tick();
        }
        assert!((retrigger.values[0].value - 0.01).abs() < 1e-6);
        assert!((legato.values[0].value - 0.26).abs() < 1e-6);
        for s in [&mut retrigger, &mut legato] {
            s.event(false);
            for _ in 0..100 {
                s.tick();
            }
            assert_eq!(s.values[0].value, 1.0);
            s.event(false);
            for _ in 0..100 {
                s.tick();
            }
            assert_eq!(s.values[0].value, 0.0);
            assert!(s.controls[0].envelope.is_idle());
        }
    }
    #[test]
    fn shared_seek_reconstruction_and_fanout_do_not_advance_a_source_twice() {
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            let mut played = shared(mode);
            let mut sought = shared(mode);
            played.event(true);
            for _ in 0..25 {
                played.tick();
            }
            sought.event(true);
            sought.advance_to(25);
            sought.tick();
            played.tick();
            assert_eq!(played.values[0].value, sought.values[0].value);
            let mut route = Modulation::new(daw_core::ModulationParameter::Frequency);
            route.source = Some(sought.values[0].source);
            route.amount = 4.0;
            let mut a = Route::new(route, 1000);
            let mut b = Route::new(route, 1000);
            assert_eq!(a.sample(&[], &sought.values), b.sample(&[], &sought.values));
            assert_eq!(sought.values[0].value, sought.controls[0].envelope.level());
        }
    }
}
