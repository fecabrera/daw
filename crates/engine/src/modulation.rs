use super::continuous_modulation::ProviderControl;
use daw_core::{EnvelopeInstance, EnvelopeMode, ModulationSourceId, ModulationValue};
use daw_envelope::Envelope;
pub(crate) use daw_unit_core::ModulationRoute as Route;
use daw_unit_core::{ModulationOutput, ModulationSourceContext, ProcessingContext};
#[derive(Clone)]
pub(crate) struct Control {
    pub source: ModulationSourceId,
    pub mode: EnvelopeMode,
    pub envelope: Envelope,
    output: ModulationOutput,
    pub routes: Vec<Route>,
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
            routes: settings
                .modulations
                .iter()
                .map(|r| Route::new(*r, context.sample_rate_hz))
                .collect(),
            envelope: Envelope::new(settings.settings, context)
                .with_modulation_scope(settings.mode.into()),
            output: ModulationOutput::new(source, settings.present && settings.enabled, context),
        }
    }
    pub fn retarget(&mut self, next: &mut Self) {
        self.envelope.set_settings(next.envelope.settings());
        self.output.retarget(&next.output);
        daw_unit_core::retarget_modulation_routes(&mut self.routes, &mut next.routes);
    }
    pub fn reset(&mut self) {
        self.envelope.reset();
        self.output.reset();
        for r in &mut self.routes {
            r.reset();
        }
    }
    pub fn note_on(&mut self, duration: Option<u64>) {
        for route in &mut self.routes {
            route.reset();
        }
        if self.routes.is_empty() {
            self.envelope.note_on_with_duration(duration);
        } else {
            self.envelope.note_on_modulated(duration);
        }
    }
    pub fn tick(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
        context: daw_unit_core::ModulationContext,
    ) -> ModulationValue {
        if self.routes.is_empty() && !self.envelope.has_modulation_offsets() {
            return self.output.sample(&mut self.envelope, Default::default());
        }
        super::continuous_modulation::sample_provider(
            &mut self.envelope,
            &mut self.output,
            &mut self.routes,
            (local, shared),
            context,
            ModulationSourceContext {
                frame: context.frame,
                tempo_bpm: context.tempo_bpm,
                ..Default::default()
            },
        )
    }
}

/// Prepare the common provider dependency graph once, outside sample processing.
pub(crate) fn provider_order<'a>(
    nodes: impl Iterator<Item = (ModulationSourceId, &'a [Route])>,
) -> Vec<usize> {
    let nodes: Vec<_> = nodes
        .map(|(id, routes)| (id, routes.iter().map(|r| r.settings).collect::<Vec<_>>()))
        .collect();
    daw_unit_core::modulation_dependency_order(
        &nodes
            .iter()
            .map(|(id, routes)| (*id, routes.as_slice()))
            .collect::<Vec<_>>(),
    )
    .expect("validated modulation graph")
}
/// Event-prefix metadata. Envelope note-on resets its contour, so exact reconstruction
/// needs only the latest retrigger, the current/last legato group, and its final release.
#[derive(Clone, Copy, Default)]
pub(crate) struct NoteState {
    held: usize,
    durations: [Option<u64>; 2],
    starts: [Option<u64>; 2],
    released: Option<u64>,
}
impl NoteState {
    pub fn record(&mut self, on: bool, frame: u64, duration: u64) {
        if on {
            self.starts[0] = Some(frame);
            self.durations[0] = Some(duration);
            if self.held == 0 {
                self.starts[1] = Some(frame);
                self.durations[1] = Some(duration);
            }
            self.held += 1;
        } else {
            self.held = self.held.saturating_sub(1);
            if self.held == 0 {
                self.released = Some(frame);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct SharedControls {
    pub controls: Vec<Control>,
    pub values: Vec<ModulationValue>,
    pub index: daw_unit_core::ModulationSourceIndex,
    providers: Vec<ProviderControl>,
    order: Vec<usize>,
    held: usize,
    note_durations: [Option<u64>; 2],
    note_ages: [u64; 2],
    frame: u64,
    release_frames: u64,
}
impl SharedControls {
    pub fn new(controls: Vec<Control>) -> Self {
        let values: Vec<_> = controls
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
        let order = provider_order(controls.iter().map(|e| (e.source, e.routes.as_slice())));
        Self {
            controls,
            order,
            index: daw_unit_core::ModulationSourceIndex::from_values(&values),
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
        self.order = provider_order(
            self.controls
                .iter()
                .map(|e| (e.source, e.routes.as_slice()))
                .chain(providers.iter().map(|p| (p.source, p.routes.as_slice()))),
        );
        self.values
            .extend(providers.iter().map(|l| ModulationValue {
                source: l.source,
                value: 0.0,
                weight: 0.0,
                range: daw_unit_core::ModulationRange::Unipolar,
                behavior: daw_unit_core::ModulationBehavior::Continuous,
                release_age: None,
            }));
        self.index = daw_unit_core::ModulationSourceIndex::from_values(&self.values);
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
    pub fn reconstruct(&mut self, state: NoteState, frame: u64) {
        self.reset_notes();
        self.held = state.held;
        self.note_durations = state.durations;
        let end = if state.held == 0 {
            state.released.unwrap_or(frame)
        } else {
            frame
        };
        self.note_ages = state
            .starts
            .map(|start| start.map_or(0, |start| end.saturating_sub(start)));
        self.release_frames = if state.held == 0 {
            frame.saturating_sub(state.released.unwrap_or(0))
        } else {
            0
        };
        self.frame = frame;
        for control in &mut self.controls {
            let index = usize::from(control.mode == EnvelopeMode::Legato);
            if let Some(start) = state.starts[index] {
                control.note_on(state.durations[index]);
                let frames = end.saturating_sub(start);
                if frames > 0 {
                    control.envelope.advance(frames);
                }
                if state.held == 0 {
                    control.envelope.note_off();
                    control.envelope.advance(frame.saturating_sub(end));
                }
            }
        }
    }
    pub fn end(&mut self) {
        self.held = 0;
        self.release_frames = 0;
        for e in &mut self.controls {
            e.envelope.note_off();
        }
    }
    #[cfg(test)]
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
                    e.note_on(duration_frames);
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
        for &slot in &self.order {
            self.values[slot] = if slot < self.controls.len() {
                let mut value = self.controls[slot].tick(
                    &[],
                    &self.values,
                    daw_unit_core::ModulationContext {
                        shared_sources: Some(&self.index),
                        frame: context.frame,
                        tempo_bpm: context.tempo_bpm,
                        ..Default::default()
                    },
                );
                if self.held == 0 {
                    value.release_age = Some(self.release_frames);
                }
                value
            } else {
                self.providers[slot - self.controls.len()].tick(&self.values, &self.index, context)
            };
        }
    }
    pub fn envelope_values(&self, id: ModulationSourceId) -> Option<[f32; 9]> {
        self.controls
            .iter()
            .find(|e| e.source == id)
            .map(|e| e.envelope.modulation_values())
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
        std::mem::swap(&mut self.index, &mut next.index);
        std::mem::swap(&mut self.order, &mut next.order);
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
    fn indexed_shared_restart_matches_complete_event_replay() {
        // Includes simultaneous notes, a long release, retriggers, idle gaps and new legato groups.
        let events = [
            (0, true, 80),
            (30, true, 40),
            (70, false, 0),
            (80, false, 0),
            (100, true, 20),
            (120, false, 0),
            (400, true, 30),
            (430, false, 0),
        ];
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            for frame in [
                0, 29, 30, 70, 80, 99, 100, 119, 120, 180, 399, 400, 430, 1000,
            ] {
                let mut reference = shared(mode);
                let mut state = NoteState::default();
                for &(at, on, duration) in events.iter().filter(|e| e.0 <= frame) {
                    reference.advance_to(at);
                    reference.event(on, on.then_some(duration));
                    state.record(on, at, duration);
                }
                reference.advance_to(frame);
                let mut indexed = shared(mode);
                indexed.reconstruct(state, frame);
                for _ in 0..10 {
                    reference.tick(Default::default());
                    indexed.tick(Default::default());
                    for (a, b) in indexed.values.iter().zip(&reference.values) {
                        assert_eq!(
                            (a.value, a.weight, a.release_age),
                            (b.value, b.weight, b.release_age)
                        );
                    }
                }
                assert_eq!(indexed.note_ages, reference.note_ages);
                assert_eq!(indexed.note_durations, reference.note_durations);
            }
        }
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
