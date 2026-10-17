use super::continuous_modulation::ProviderControl;
use daw_core::{EnvelopeInstance, EnvelopeMode, ModulationSourceId, ModulationValue};
use daw_envelope::Envelope;
pub(crate) use daw_unit_core::ModulationRoute as Route;
use daw_unit_core::{ModulationOutput, ModulationSourceContext, ProcessingContext};
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
        self.output.sample(&mut self.envelope, Default::default())
    }
}

pub(crate) struct SharedControls {
    pub controls: Vec<Control>,
    pub values: Vec<ModulationValue>,
    providers: Vec<ProviderControl>,
    held: usize,
    note_durations: [Option<u64>; 2],
    note_ages: [u64; 2],
    frame: u64,
    release_frames: u64,
}
impl SharedControls {
    pub fn new(controls: Vec<Control>) -> Self {
        let values = controls
            .iter()
            .map(|e| ModulationValue {
                range: daw_unit_core::ModulationRange::Unipolar,
                behavior: daw_unit_core::ModulationBehavior::Contour,
                source: e.source,
                value: 0.0,
                weight: 0.0,
                release_age: None,
            })
            .collect();
        Self {
            controls,
            values,
            providers: Vec::new(),
            held: 0,
            note_durations: [None; 2],
            note_ages: [0; 2],
            frame: 0,
            release_frames: 0,
        }
    }
    pub fn with_providers(mut self, mut providers: Vec<ProviderControl>) -> Self {
        providers.sort_by_key(|l| (l.source.layer, l.source.unit));
        let settings: Vec<Vec<_>> = providers
            .iter()
            .map(|l| l.routes.iter().map(|r| r.settings).collect())
            .collect();
        let dependencies: Vec<_> = providers
            .iter()
            .zip(&settings)
            .map(|(l, routes)| (l.source, routes.as_slice()))
            .collect();
        let order = daw_unit_core::modulation_dependency_order(&dependencies)
            .expect("validated modulation graph");
        let mut slots: Vec<_> = providers.into_iter().map(Some).collect();
        let providers: Vec<_> = order
            .into_iter()
            .map(|i| slots[i].take().unwrap())
            .collect();
        self.values
            .extend(providers.iter().map(|l| ModulationValue {
                source: l.source,
                value: 0.0,
                weight: 0.0,
                range: daw_unit_core::ModulationRange::Unipolar,
                behavior: daw_unit_core::ModulationBehavior::Continuous,
                release_age: None,
            }));
        self.providers = providers;
        self
    }
    pub fn reset(&mut self) {
        self.reset_notes();
        for provider in &mut self.providers {
            provider.reset();
        }
    }
    pub fn reset_notes(&mut self) {
        for e in &mut self.controls {
            e.reset();
        }
        for v in &mut self.values {
            v.value = 0.0;
            v.weight = 0.0;
        }
        self.held = 0;
        self.note_durations = [None; 2];
        self.note_ages = [0; 2];
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
        if self.held > 0 {
            for age in &mut self.note_ages {
                *age = age.saturating_add(frame.saturating_sub(self.frame));
            }
        }
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
    pub fn event(&mut self, on: bool, duration_frames: Option<u64>) {
        if on {
            self.note_durations[0] = duration_frames;
            self.note_ages[0] = 0;
            if self.held == 0 {
                self.note_durations[1] = duration_frames;
                self.note_ages[1] = 0;
            }
            for e in &mut self.controls {
                if self.held == 0 || e.mode == EnvelopeMode::Retrigger {
                    e.envelope.note_on_with_duration(duration_frames);
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
    pub fn tick(&mut self, context: ModulationSourceContext) {
        if self.held > 0 {
            for age in &mut self.note_ages {
                *age = age.saturating_add(1);
            }
        }
        if self.held == 0 {
            self.release_frames = self.release_frames.saturating_add(1);
        }
        for (e, v) in self.controls.iter_mut().zip(&mut self.values) {
            *v = e.tick();
            if self.held == 0 {
                v.release_age = Some(self.release_frames);
            }
        }
        for (index, provider) in self.providers.iter_mut().enumerate() {
            self.values[self.controls.len() + index] = provider.tick(&self.values, context);
        }
    }
    pub fn provider_values(&self, id: ModulationSourceId) -> Option<[f32; 2]> {
        self.providers
            .iter()
            .find(|l| l.source == id)
            .map(|l| l.values())
    }
    pub fn retarget(&mut self, next: &mut Self) {
        for n in &mut next.providers {
            if let Some(old) = self.providers.iter_mut().find(|old| old.source == n.source) {
                old.retarget(n);
            }
        }
        std::mem::swap(&mut self.providers, &mut next.providers);

        for n in &mut next.controls {
            if let Some(e) = self.controls.iter_mut().find(|e| e.source == n.source) {
                e.retarget(n);
                std::mem::swap(e, n);
            } else if self.held > 0 {
                let index = usize::from(n.mode == EnvelopeMode::Legato);
                n.envelope.note_on_with_duration(self.note_durations[index]);
                n.envelope.advance(self.note_ages[index]);
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
            s.event(true, None);
            for _ in 0..25 {
                s.tick(Default::default());
            }
            assert!((s.values[0].value - 0.25).abs() < 1e-6);
            s.event(true, None);
            s.tick(Default::default());
        }
        assert!((retrigger.values[0].value - 0.01).abs() < 1e-6);
        assert!((legato.values[0].value - 0.26).abs() < 1e-6);
        for s in [&mut retrigger, &mut legato] {
            s.event(false, None);
            for _ in 0..100 {
                s.tick(Default::default());
            }
            assert_eq!(s.values[0].value, 1.0);
            s.event(false, None);
            for _ in 0..100 {
                s.tick(Default::default());
            }
            assert_eq!(s.values[0].value, 0.0);
            assert!(s.controls[0].envelope.is_idle());
        }
    }
    #[test]
    fn note_relative_shared_modes_keep_the_triggering_length_and_live_addition_age() {
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            let mut s = shared(mode);
            let mut settings = s.controls[0].envelope.settings();
            settings.timing = daw_core::EnvelopeTiming::Note;
            settings.attack_note = 1.0;
            s.controls[0].envelope.set_settings(settings);
            s.event(true, Some(100));
            for _ in 0..25 {
                s.tick(Default::default());
            }
            s.event(true, Some(200));
            for _ in 0..25 {
                s.tick(Default::default());
            }
            let expected = if mode == EnvelopeMode::Retrigger {
                0.125
            } else {
                0.5
            };
            assert!((s.values[0].value - expected).abs() < 1e-6);
            let mut next = shared(mode);
            next.controls[0].source.unit = daw_core::Id::from_u128(999);
            next.controls[0].envelope.set_settings(settings);
            s.retarget(&mut next);
            assert!((s.controls[0].envelope.level() - expected).abs() < 1e-6);
            s.event(false, None);
            assert_ne!(s.controls[0].envelope.stage(), daw_envelope::Stage::Release);
            s.event(false, None);
            assert_eq!(s.controls[0].envelope.stage(), daw_envelope::Stage::Release);
        }
    }
    #[test]
    fn shared_seek_reconstruction_and_fanout_do_not_advance_a_source_twice() {
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            let mut played = shared(mode);
            let mut sought = shared(mode);
            played.event(true, None);
            for _ in 0..25 {
                played.tick(Default::default());
            }
            sought.event(true, None);
            sought.advance_to(25);
            sought.tick(Default::default());
            played.tick(Default::default());
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
