use daw_core::{EnvelopeInstance, EnvelopeMode, EnvelopeSource, Modulation, ModulationValue};
use daw_envelope::Envelope;
use daw_unit_core::ProcessingContext;
#[derive(Clone, Copy)]
pub(crate) struct Smooth {
    value: f32,
    target: f32,
    left: u32,
}
impl Smooth {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            left: 0,
        }
    }
    pub fn set(&mut self, value: f32, frames: u32) {
        if self.target != value {
            self.target = value;
            self.left = frames;
        }
    }
    pub fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.value += (self.target - self.value) / self.left as f32;
            self.left -= 1;
        }
        self.value
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.left = 0;
    }
}
pub(crate) struct Control {
    pub source: EnvelopeSource,
    pub mode: EnvelopeMode,
    pub envelope: Envelope,
    weight: Smooth,
    frames: u32,
}
impl Control {
    pub fn new(
        source: EnvelopeSource,
        settings: &EnvelopeInstance,
        context: ProcessingContext,
    ) -> Self {
        Self {
            source,
            mode: settings.mode,
            envelope: Envelope::new(settings.settings, context),
            weight: Smooth::new(f32::from(settings.present && settings.enabled)),
            frames: (context.sample_rate_hz / 200).max(1),
        }
    }
    pub fn retarget(&mut self, next: &Self) {
        self.envelope.set_settings(next.envelope.settings());
        self.weight.set(next.weight.target, self.frames);
    }
    pub fn reset(&mut self) {
        self.envelope.reset();
        self.weight.reset();
    }
    pub fn tick(&mut self) -> ModulationValue {
        ModulationValue {
            source: self.source,
            value: self.envelope.next_sample(),
            weight: self.weight.next(),
            release_age: None,
        }
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
/// Resolve shared effect offsets once per source/route sample, then apply destination bounds.
pub(crate) fn offsets<const N: usize>(
    routes: &mut [Route],
    values: &[ModulationValue],
    parameters: [daw_core::ModulationParameter; N],
) -> [f32; N] {
    let mut offsets = [0.0; N];
    for route in routes {
        if let Some(index) = parameters
            .iter()
            .position(|p| *p == route.settings.parameter)
            && let Some((value, amount, _)) = route.sample(&[], values)
        {
            offsets[index] += value * amount;
        }
    }
    offsets
}
pub(crate) struct Route {
    pub settings: Modulation,
    amount: Smooth,
    frames: u32,
}
impl Route {
    pub fn new(settings: Modulation, rate: u32) -> Self {
        Self {
            settings,
            amount: Smooth::new(if settings.enabled {
                settings.amount
            } else {
                0.0
            }),
            frames: (rate / 200).max(1),
        }
    }
    pub fn reset(&mut self) {
        self.amount.reset();
    }
    pub fn retarget(&mut self, next: &Self) {
        self.settings = next.settings;
        self.amount.set(next.amount.target, self.frames);
    }
    pub fn sample(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
    ) -> Option<(f32, f32, bool)> {
        let amount = self.amount.next();
        let source = self.settings.source?;
        let (value, per_note) = if let Some(v) = local.iter().find(|v| v.source == source) {
            (v, true)
        } else {
            (shared.iter().find(|v| v.source == source)?, false)
        };
        if amount == 0.0 || value.weight <= 0.0 {
            None
        } else {
            Some((value.value, amount * value.weight, per_note))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            EnvelopeSource {
                layer: daw_core::Id::from_u128(1),
                envelope: e.id,
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
