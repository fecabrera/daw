use daw_auto_filter::AutoFilter;
use daw_compressor::Compressor;
use daw_core::{EffectInstance, EffectSettings, Id, Track};
use daw_drive::Drive;
use daw_equalizer::Equalizer;
use daw_limiter::Limiter;
use daw_ott::Ott;
use daw_unit_core::{EffectUnit, ProcessingContext};
use std::sync::{Arc, atomic::AtomicU32};

enum RuntimeEffect {
    Eq3(Box<Equalizer>),
    Drive(Box<Drive>, Vec<super::modulation::Route>),
    AutoFilter(Box<AutoFilter>, Vec<super::modulation::Route>),
    Compressor(Box<Compressor>),
    Ott(Box<Ott>),
    Limiter(Box<Limiter>),
}
impl RuntimeEffect {
    fn sample(
        &mut self,
        input: [f32; 2],
        frame: u64,
        tempo: f32,
        values: &[daw_core::ModulationValue],
    ) -> [f32; 2] {
        match self {
            Self::Eq3(unit) => unit.process_sample(input),
            Self::Drive(unit, routes) => unit.process_modulated(
                input,
                super::modulation::offsets(routes, values, daw_core::ModulationParameter::DRIVE),
            ),
            Self::AutoFilter(unit, routes) => {
                let offsets = super::modulation::offsets(
                    routes,
                    values,
                    daw_core::ModulationParameter::FILTER,
                );
                unit.process_modulated_at(input, frame, tempo, offsets)
            }
            Self::Compressor(unit) => unit.process_sample(input),
            Self::Ott(unit) => unit.process_sample(input),
            Self::Limiter(unit) => unit.process_sample(input),
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Eq3(unit) => unit.reset(),

            Self::Drive(unit, routes) => {
                unit.reset();
                for r in routes {
                    r.reset();
                }
            }
            Self::AutoFilter(unit, routes) => {
                unit.reset();
                for r in routes {
                    r.reset();
                }
            }
            Self::Compressor(unit) => unit.reset(),
            Self::Ott(unit) => unit.reset(),
            Self::Limiter(unit) => unit.reset(),
        }
    }
    fn retarget(&mut self, settings: &EffectSettings) -> bool {
        match (self, settings) {
            (Self::Eq3(unit), EffectSettings::Eq3(settings)) => unit.retarget(settings),
            (Self::Ott(unit), EffectSettings::Ott(settings)) => unit.retarget(settings),
            (Self::Limiter(unit), EffectSettings::Limiter(settings)) => unit.retarget(settings),
            (Self::Drive(unit, _), EffectSettings::Drive(settings)) => unit.retarget(settings),
            (Self::Compressor(unit), EffectSettings::Compressor(settings)) => {
                unit.retarget(settings)
            }
            (Self::AutoFilter(unit, _), EffectSettings::AutoFilter(settings)) => {
                unit.retarget(settings)
            }
            _ => return false,
        }
        true
    }
}

pub(super) struct EffectChain {
    id: Option<Id>,
    mono: bool,
    units: Vec<(Id, Option<RuntimeEffect>)>,
}
impl EffectChain {
    pub fn new(track: &Track, sample_rate_hz: u32, max_block_frames: u32) -> Self {
        Self::chain(
            Some(track.id),
            &track.effects,
            sample_rate_hz,
            max_block_frames,
        )
    }
    pub fn chain(
        id: Option<Id>,
        effects: &[EffectInstance],
        sample_rate_hz: u32,
        max_block_frames: u32,
    ) -> Self {
        let context = ProcessingContext {
            sample_rate_hz,
            max_block_frames,
        };
        Self {
            id,
            mono: false,
            units: effects
                .iter()
                .map(|effect| {
                    (
                        effect.id,
                        match &effect.unit {
                            EffectSettings::Eq3(settings) => Some(RuntimeEffect::Eq3(Box::new(
                                Equalizer::new(context, settings),
                            ))),
                            EffectSettings::Drive(settings) => Some(RuntimeEffect::Drive(
                                Box::new(Drive::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
                            EffectSettings::AutoFilter(settings) => {
                                Some(RuntimeEffect::AutoFilter(
                                    Box::new(AutoFilter::new(context, settings)),
                                    settings
                                        .modulations
                                        .iter()
                                        .map(|r| {
                                            super::modulation::Route::new(
                                                *r,
                                                context.sample_rate_hz,
                                            )
                                        })
                                        .collect(),
                                ))
                            }
                            EffectSettings::Ott(settings) => {
                                Some(RuntimeEffect::Ott(Box::new(Ott::new(context, settings))))
                            }
                            EffectSettings::Limiter(settings) => Some(RuntimeEffect::Limiter(
                                Box::new(Limiter::new(context, settings)),
                            )),
                            EffectSettings::Missing { .. } => None,
                            EffectSettings::Compressor(settings) => {
                                Some(RuntimeEffect::Compressor(Box::new(Compressor::new(
                                    context, settings,
                                ))))
                            }
                        },
                    )
                })
                .collect(),
        }
    }
    pub fn inherit(&mut self, old: &mut [Self], track: &Track) {
        let Some(previous) = old.iter_mut().find(|item| item.id == self.id) else {
            return;
        };
        self.inherit_chain(previous, &track.effects);
    }
    pub fn inherit_chain(&mut self, previous: &mut Self, settings: &[EffectInstance]) {
        self.mono = previous.mono;
        for ((id, next), settings) in self.units.iter_mut().zip(settings) {
            if let Some((_, Some(previous))) =
                previous.units.iter_mut().find(|(old_id, _)| id == old_id)
                && let Some(next) = next
            {
                if let (
                    RuntimeEffect::AutoFilter(_, old_routes),
                    RuntimeEffect::AutoFilter(_, new_routes),
                )
                | (RuntimeEffect::Drive(_, old_routes), RuntimeEffect::Drive(_, new_routes)) =
                    (&mut *previous, &mut *next)
                {
                    for new in new_routes.iter_mut() {
                        if let Some(old) = old_routes
                            .iter_mut()
                            .find(|r| r.settings.id == new.settings.id)
                        {
                            old.retarget(new);
                            std::mem::swap(old, new);
                        }
                    }
                    std::mem::swap(old_routes, new_routes);
                }
                if previous.retarget(&settings.unit) {
                    std::mem::swap(next, previous);
                }
            }
        }
    }
    pub fn modulation_values(&self, id: Id) -> Option<[f32; 5]> {
        match self.units.iter().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::AutoFilter(unit, _)))) => {
                Some(unit.effective_parameters())
            }
            Some((_, Some(RuntimeEffect::Drive(unit, _)))) => Some(unit.effective_parameters()),
            _ => None,
        }
    }
    pub fn set_meter(&mut self, id: Id, meter: &Arc<AtomicU32>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Compressor(unit)))) => {
                unit.set_gain_reduction_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Limiter(unit)))) => {
                unit.set_gain_reduction_meter(meter.clone())
            }
            _ => {}
        }
    }
    pub fn set_ott_meter(&mut self, id: Id, meter: &Arc<[AtomicU32; 3]>) {
        if let Some((_, Some(RuntimeEffect::Ott(unit)))) =
            self.units.iter_mut().find(|(effect, _)| *effect == id)
        {
            unit.set_gain_change_meter(meter.clone());
        }
    }
    pub fn sample(
        &mut self,
        mut input: [f32; 2],
        mono: Option<bool>,
        mut frame: u64,
        tempo: f32,
        loop_region: Option<(u64, u64)>,
        values: &[daw_core::ModulationValue],
    ) -> ([f32; 2], bool) {
        if let Some(mono) = mono {
            self.mono = mono;
        }
        for (_, unit) in &mut self.units {
            if let Some(unit) = unit {
                input = unit.sample(input, frame, tempo, values);
                if let RuntimeEffect::Limiter(unit) = unit {
                    frame = super::latency::delayed_frame(
                        frame,
                        u64::from(unit.latency_frames()),
                        loop_region,
                    );
                }
            }
        }
        (input, self.mono)
    }
    pub fn reset(&mut self) {
        for (_, unit) in &mut self.units {
            if let Some(unit) = unit {
                unit.reset();
            }
        }
    }
}
