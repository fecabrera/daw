use daw_auto_filter::AutoFilter;
use daw_bitcrusher::Bitcrusher;
use daw_compressor::Compressor;
use daw_core::{EffectInstance, EffectSettings, Id, Track};
use daw_direction_mixer::DirectionMixer;
use daw_drive::Drive;
use daw_equalizer::Equalizer;
use daw_limiter::Limiter;
use daw_ott::Ott;
use daw_stereo_spread::StereoSpread;
use daw_unit_core::{EffectUnit, ModulationContext, ProcessingContext, process_modulation};
use std::sync::{Arc, atomic::AtomicU32};

enum RuntimeEffect {
    Eq3(Box<Equalizer>, Vec<super::modulation::Route>),
    Drive(Box<Drive>, Vec<super::modulation::Route>),
    AutoFilter(Box<AutoFilter>, Vec<super::modulation::Route>),
    Compressor(Box<Compressor>, Vec<super::modulation::Route>),
    Ott(Box<Ott>, Vec<super::modulation::Route>),
    Limiter(Box<Limiter>, Vec<super::modulation::Route>),
    DirectionMixer(Box<DirectionMixer>, Vec<super::modulation::Route>),
    StereoSpread(Box<StereoSpread>, Vec<super::modulation::Route>),
    Bitcrusher(Box<Bitcrusher>, Vec<super::modulation::Route>),
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
            Self::Eq3(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Drive(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::AutoFilter(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Compressor(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Ott(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Limiter(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::StereoSpread(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Bitcrusher(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::DirectionMixer(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Eq3(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }

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
            Self::Compressor(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
            Self::Ott(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
            Self::Limiter(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
            Self::StereoSpread(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
            Self::Bitcrusher(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
            Self::DirectionMixer(unit, routes) => {
                unit.reset();
                for route in routes {
                    route.reset();
                }
            }
        }
    }
    fn retarget(&mut self, settings: &EffectSettings) -> bool {
        match (self, settings) {
            (Self::Eq3(unit, _), EffectSettings::Eq3(settings)) => unit.retarget(settings),
            (Self::Ott(unit, _), EffectSettings::Ott(settings)) => unit.retarget(settings),
            (Self::Limiter(unit, _), EffectSettings::Limiter(settings)) => unit.retarget(settings),
            (Self::StereoSpread(unit, _), EffectSettings::StereoSpread(settings)) => {
                unit.retarget(settings)
            }
            (Self::Bitcrusher(unit, _), EffectSettings::Bitcrusher(settings)) => {
                unit.retarget(settings)
            }
            (Self::DirectionMixer(unit, _), EffectSettings::DirectionMixer(settings)) => {
                unit.retarget(settings)
            }
            (Self::Drive(unit, _), EffectSettings::Drive(settings)) => unit.retarget(settings),
            (Self::Compressor(unit, _), EffectSettings::Compressor(settings)) => {
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
                            EffectSettings::Eq3(settings) => Some(RuntimeEffect::Eq3(
                                Box::new(Equalizer::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
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
                            EffectSettings::Ott(settings) => Some(RuntimeEffect::Ott(
                                Box::new(Ott::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
                            EffectSettings::Limiter(settings) => Some(RuntimeEffect::Limiter(
                                Box::new(Limiter::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
                            EffectSettings::StereoSpread(settings) => {
                                Some(RuntimeEffect::StereoSpread(
                                    Box::new(StereoSpread::new(context, settings)),
                                    settings
                                        .modulations
                                        .iter()
                                        .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                        .collect(),
                                ))
                            }
                            EffectSettings::Bitcrusher(settings) => {
                                Some(RuntimeEffect::Bitcrusher(
                                    Box::new(Bitcrusher::new(context, settings)),
                                    settings
                                        .modulations
                                        .iter()
                                        .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                        .collect(),
                                ))
                            }
                            EffectSettings::DirectionMixer(settings) => {
                                Some(RuntimeEffect::DirectionMixer(
                                    Box::new(DirectionMixer::new(context, settings)),
                                    settings
                                        .modulations
                                        .iter()
                                        .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                        .collect(),
                                ))
                            }
                            EffectSettings::Missing { .. } => None,
                            EffectSettings::Compressor(settings) => {
                                Some(RuntimeEffect::Compressor(
                                    Box::new(Compressor::new(context, settings)),
                                    settings
                                        .modulations
                                        .iter()
                                        .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                        .collect(),
                                ))
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
                | (RuntimeEffect::Drive(_, old_routes), RuntimeEffect::Drive(_, new_routes))
                | (RuntimeEffect::Eq3(_, old_routes), RuntimeEffect::Eq3(_, new_routes))
                | (
                    RuntimeEffect::Compressor(_, old_routes),
                    RuntimeEffect::Compressor(_, new_routes),
                )
                | (RuntimeEffect::Ott(_, old_routes), RuntimeEffect::Ott(_, new_routes))
                | (
                    RuntimeEffect::Limiter(_, old_routes),
                    RuntimeEffect::Limiter(_, new_routes),
                )
                | (
                    RuntimeEffect::DirectionMixer(_, old_routes),
                    RuntimeEffect::DirectionMixer(_, new_routes),
                )
                | (
                    RuntimeEffect::StereoSpread(_, old_routes),
                    RuntimeEffect::StereoSpread(_, new_routes),
                )
                | (
                    RuntimeEffect::Bitcrusher(_, old_routes),
                    RuntimeEffect::Bitcrusher(_, new_routes),
                ) = (&mut *previous, &mut *next)
                {
                    daw_unit_core::retarget_modulation_routes(old_routes, new_routes);
                }
                if previous.retarget(&settings.unit) {
                    std::mem::swap(next, previous);
                }
            }
        }
    }
    pub fn modulation_values(&self, id: Id) -> Option<super::ModulationSnapshot> {
        match self.units.iter().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::AutoFilter(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Drive(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Eq3(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Compressor(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Limiter(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::StereoSpread(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Bitcrusher(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::DirectionMixer(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Ott(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            _ => None,
        }
    }
    pub fn set_meter(&mut self, id: Id, meter: &Arc<AtomicU32>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Compressor(unit, _)))) => {
                unit.set_gain_reduction_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Limiter(unit, _)))) => {
                unit.set_gain_reduction_meter(meter.clone())
            }
            _ => {}
        }
    }
    pub fn set_band_meter(&mut self, id: Id, meter: &Arc<[AtomicU32; 3]>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Ott(unit, _)))) => {
                unit.set_gain_change_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Eq3(unit, _)))) => {
                unit.set_band_level_meter(meter.clone())
            }
            _ => {}
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
                if let RuntimeEffect::Limiter(unit, _) = unit {
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
