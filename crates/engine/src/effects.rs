use daw_auto_filter::AutoFilter;
use daw_bitcrusher::Bitcrusher;
use daw_chorus::Chorus;
use daw_compressor::Compressor;
use daw_core::{EffectInstance, EffectSettings, Id, Track};
use daw_direction_mixer::DirectionMixer;
use daw_drive::{Drive, DriveV2};
use daw_equalizer::{Equalizer, MultibandEq};
use daw_filter::Filter;
use daw_limiter::Limiter;
use daw_ott::Ott;
use daw_saturator::Saturator;
use daw_stereo_spread::StereoSpread;
use daw_unit_core::{EffectUnit, ModulationContext, ProcessingContext, process_modulation};
use std::sync::Arc;

#[derive(Clone)]
enum RuntimeEffect {
    MultibandEq(Box<MultibandEq>),
    Eq3(Box<Equalizer>, Vec<super::modulation::Route>),
    Drive(Box<Drive>, Vec<super::modulation::Route>),
    DriveV2(Box<DriveV2>, Vec<super::modulation::Route>),
    AutoFilter(Box<AutoFilter>, Vec<super::modulation::Route>),
    Filter(Box<Filter>, Vec<super::modulation::Route>),
    Compressor(Box<Compressor>, Vec<super::modulation::Route>),
    Ott(Box<Ott>, Vec<super::modulation::Route>),
    Limiter(Box<Limiter>, Vec<super::modulation::Route>),
    DirectionMixer(Box<DirectionMixer>, Vec<super::modulation::Route>),
    StereoSpread(Box<StereoSpread>, Vec<super::modulation::Route>),
    Saturator(Box<Saturator>, Vec<super::modulation::Route>),
    Bitcrusher(Box<Bitcrusher>, Vec<super::modulation::Route>),
    Chorus(Box<Chorus>, Vec<super::modulation::Route>),
}

impl RuntimeEffect {
    fn heap_bytes(&self) -> u64 {
        match self {
            Self::MultibandEq(unit) => {
                std::mem::size_of_val(unit.as_ref()) as u64 + unit.heap_bytes()
            }
            Self::Eq3(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Drive(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::DriveV2(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::AutoFilter(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Filter(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Compressor(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Ott(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Limiter(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::DirectionMixer(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::StereoSpread(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Saturator(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Bitcrusher(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
            Self::Chorus(unit, routes) => {
                std::mem::size_of_val(unit.as_ref()) as u64
                    + unit.heap_bytes()
                    + daw_unit_core::memory::vec_bytes(routes)
            }
        }
    }

    // Forward the shared processor contract for every variant. Bypassed units
    // retain their fixed delay, and each downstream clock must follow it.
    fn latency_frames(&self) -> u32 {
        match self {
            Self::MultibandEq(unit) => unit.latency_frames(),
            Self::Eq3(unit, _) => unit.latency_frames(),
            Self::Drive(unit, _) => unit.latency_frames(),
            Self::DriveV2(unit, _) => unit.latency_frames(),
            Self::AutoFilter(unit, _) => unit.latency_frames(),
            Self::Filter(unit, _) => unit.latency_frames(),
            Self::Compressor(unit, _) => unit.latency_frames(),
            Self::Ott(unit, _) => unit.latency_frames(),
            Self::Limiter(unit, _) => unit.latency_frames(),
            Self::DirectionMixer(unit, _) => unit.latency_frames(),
            Self::StereoSpread(unit, _) => unit.latency_frames(),
            Self::Saturator(unit, _) => unit.latency_frames(),
            Self::Bitcrusher(unit, _) => unit.latency_frames(),
            Self::Chorus(unit, _) => unit.latency_frames(),
        }
    }

    fn sample(
        &mut self,
        input: [f32; 2],
        frame: u64,
        tempo: f32,
        (values, sources): (
            &[daw_core::ModulationValue],
            Option<&daw_unit_core::ModulationSourceIndex>,
        ),
    ) -> [f32; 2] {
        match self {
            Self::MultibandEq(unit) => unit.process_at(
                input,
                values,
                ModulationContext {
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Eq3(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    shared_sources: sources,
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
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::DriveV2(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    shared_sources: sources,
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
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Filter(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    shared_sources: sources,
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
                    shared_sources: sources,
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
                    shared_sources: sources,
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
                    shared_sources: sources,
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
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Saturator(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    shared_sources: sources,
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
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
            Self::Chorus(unit, routes) => process_modulation(
                unit.as_mut(),
                routes,
                input,
                &[],
                values,
                ModulationContext {
                    shared_sources: sources,
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
                    shared_sources: sources,
                    frame,
                    tempo_bpm: tempo,
                    ..Default::default()
                },
            ),
        }
    }
    fn reset(&mut self) {
        match self {
            Self::MultibandEq(unit) => unit.reset(),
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
            Self::DriveV2(unit, routes) => {
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
            Self::Filter(unit, routes) => {
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
            Self::Saturator(unit, routes) => {
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
            Self::Chorus(unit, routes) => {
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
            (Self::Saturator(unit, _), EffectSettings::Saturator(settings)) => {
                unit.retarget(settings)
            }
            (Self::Bitcrusher(unit, _), EffectSettings::Bitcrusher(settings)) => {
                unit.retarget(settings)
            }
            (Self::Chorus(unit, _), EffectSettings::Chorus(settings)) => unit.retarget(settings),
            (Self::DirectionMixer(unit, _), EffectSettings::DirectionMixer(settings)) => {
                unit.retarget(settings)
            }
            (Self::Drive(unit, _), EffectSettings::Drive(settings)) => unit.retarget(settings),
            (Self::DriveV2(unit, _), EffectSettings::DriveV2(settings)) => unit.retarget(settings),
            (Self::Compressor(unit, _), EffectSettings::Compressor(settings)) => {
                unit.retarget(settings)
            }
            (Self::AutoFilter(unit, _), EffectSettings::AutoFilter(settings)) => {
                unit.retarget(settings)
            }
            (Self::Filter(unit, _), EffectSettings::Filter(settings)) => unit.retarget(settings),
            _ => return false,
        }
        true
    }
}

#[derive(Clone)]
pub(super) struct EffectChain {
    index: std::collections::HashMap<Id, usize>,
    mono: bool,
    units: Vec<(Id, Option<RuntimeEffect>)>,
}
impl EffectChain {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::map_bytes(&self.index)
            + daw_unit_core::memory::vec_bytes(&self.units)
            + self
                .units
                .iter()
                .filter_map(|(_, u)| u.as_ref())
                .map(RuntimeEffect::heap_bytes)
                .sum::<u64>()
    }

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
        let _ = id;
        Self {
            index: effects.iter().enumerate().map(|(i, e)| (e.id, i)).collect(),
            mono: false,
            units: effects
                .iter()
                .map(|effect| {
                    (
                        effect.id,
                        match &effect.unit {
                            EffectSettings::MultibandEq(settings) => {
                                Some(RuntimeEffect::MultibandEq(Box::new(MultibandEq::new(
                                    context, settings,
                                ))))
                            }
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
                            EffectSettings::DriveV2(settings) => Some(RuntimeEffect::DriveV2(
                                Box::new(DriveV2::new(context, settings)),
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
                            EffectSettings::Filter(settings) => Some(RuntimeEffect::Filter(
                                Box::new(Filter::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| {
                                        super::modulation::Route::new(*r, context.sample_rate_hz)
                                    })
                                    .collect(),
                            )),
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
                            EffectSettings::Saturator(settings) => Some(RuntimeEffect::Saturator(
                                Box::new(Saturator::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
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
                            EffectSettings::Chorus(settings) => Some(RuntimeEffect::Chorus(
                                Box::new(Chorus::new(context, settings)),
                                settings
                                    .modulations
                                    .iter()
                                    .map(|r| super::modulation::Route::new(*r, sample_rate_hz))
                                    .collect(),
                            )),
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
    pub fn inherit_chain(&mut self, previous: &mut Self, settings: &[EffectInstance]) {
        self.mono = previous.mono;
        for ((id, next), settings) in self.units.iter_mut().zip(settings) {
            if let Some((_, Some(previous))) = previous
                .index
                .get(id)
                .and_then(|&i| previous.units.get_mut(i))
                && let Some(next) = next
            {
                if let (RuntimeEffect::MultibandEq(next), RuntimeEffect::MultibandEq(previous)) =
                    (&mut *next, &mut *previous)
                {
                    next.inherit(previous);
                    continue;
                }
                if let (
                    RuntimeEffect::AutoFilter(_, old_routes),
                    RuntimeEffect::AutoFilter(_, new_routes),
                )
                | (
                    RuntimeEffect::Filter(_, old_routes),
                    RuntimeEffect::Filter(_, new_routes),
                )
                | (RuntimeEffect::Drive(_, old_routes), RuntimeEffect::Drive(_, new_routes))
                | (
                    RuntimeEffect::DriveV2(_, old_routes),
                    RuntimeEffect::DriveV2(_, new_routes),
                )
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
                    RuntimeEffect::Saturator(_, old_routes),
                    RuntimeEffect::Saturator(_, new_routes),
                )
                | (
                    RuntimeEffect::Bitcrusher(_, old_routes),
                    RuntimeEffect::Bitcrusher(_, new_routes),
                )
                | (
                    RuntimeEffect::Chorus(_, old_routes),
                    RuntimeEffect::Chorus(_, new_routes),
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
    pub fn band_modulation_values(&self, id: Id, band: Id) -> Option<super::ModulationSnapshot> {
        let (_, Some(RuntimeEffect::MultibandEq(unit))) = self.units.get(*self.index.get(&id)?)?
        else {
            return None;
        };
        unit.band_parameters(band).map(super::parameter_snapshot)
    }
    pub fn modulation_values(&self, id: Id) -> Option<super::ModulationSnapshot> {
        match self.index.get(&id).and_then(|&i| self.units.get(i)) {
            Some((_, Some(RuntimeEffect::AutoFilter(unit, _)))) => {
                Some(super::parameter_snapshot(unit.modulation_parameters()))
            }
            Some((_, Some(RuntimeEffect::Filter(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Drive(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::DriveV2(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::MultibandEq(unit)))) => {
                Some(super::parameter_snapshot([unit.output_gain_db()]))
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
            Some((_, Some(RuntimeEffect::Saturator(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Bitcrusher(unit, _)))) => {
                Some(super::parameter_snapshot(unit.effective_parameters()))
            }
            Some((_, Some(RuntimeEffect::Chorus(unit, _)))) => {
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
    pub fn set_meter(&mut self, id: Id, meter: &Arc<daw_unit_core::PeakMeter>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Compressor(unit, _)))) => {
                unit.set_gain_reduction_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Limiter(unit, _)))) => {
                unit.set_gain_reduction_peak_meter(meter.clone())
            }
            _ => {}
        }
    }
    pub fn set_band_level_meter(
        &mut self,
        effect: Id,
        band: Id,
        meter: &Arc<daw_unit_core::BandMeter<3>>,
    ) {
        if let Some((_, Some(RuntimeEffect::MultibandEq(unit)))) =
            self.index.get(&effect).and_then(|&i| self.units.get_mut(i))
        {
            unit.set_band_level_meter(band, meter.current().clone());
        }
    }
    pub fn set_output_meter(&mut self, id: Id, meter: &Arc<daw_unit_core::BandMeter<3>>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Compressor(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Limiter(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Ott(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone())
            }
            _ => {}
        }
    }
    pub fn set_band_meter(&mut self, id: Id, meter: &Arc<daw_unit_core::BandMeter<3>>) {
        match self.units.iter_mut().find(|(effect, _)| *effect == id) {
            Some((_, Some(RuntimeEffect::Ott(unit, _)))) => {
                unit.set_gain_change_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::Eq3(unit, _)))) => {
                unit.set_band_level_meter(meter.current().clone())
            }
            Some((_, Some(RuntimeEffect::Chorus(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone());
            }
            Some((_, Some(RuntimeEffect::Drive(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone());
            }
            Some((_, Some(RuntimeEffect::DriveV2(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone());
            }
            Some((_, Some(RuntimeEffect::Bitcrusher(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone());
            }
            Some((_, Some(RuntimeEffect::Saturator(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone());
            }
            Some((_, Some(RuntimeEffect::Filter(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::AutoFilter(unit, _)))) => {
                unit.set_output_peak_meter(meter.clone())
            }
            Some((_, Some(RuntimeEffect::MultibandEq(unit)))) => {
                unit.set_output_peak_meter(meter.clone())
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
        values: (
            &[daw_core::ModulationValue],
            Option<&daw_unit_core::ModulationSourceIndex>,
        ),
    ) -> ([f32; 2], bool) {
        if let Some(mono) = mono {
            self.mono = mono;
        }
        for (_, unit) in &mut self.units {
            if let Some(unit) = unit {
                input = unit.sample(input, frame, tempo, values);
                let delay = unit.latency_frames();
                if delay != 0 {
                    frame = super::latency::delayed_frame(frame, u64::from(delay), loop_region);
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

#[cfg(test)]
mod clock_tests {
    use super::*;
    use daw_unit_core::{AutoFilterSettings, LfoRateMode};

    #[test]
    fn serial_effect_clocks_follow_each_processors_delay_including_bypass_and_loops() {
        for rate in daw_core::SAMPLE_RATES {
            for enabled in [false, true] {
                for region in [None, Some((1000, 1017)), Some((1000, 1700))] {
                    let context = ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 512,
                    };
                    let filter_settings = AutoFilterSettings {
                        frequency_hz: 200.0,
                        q: 3.0,
                        rate_mode: LfoRateMode::Hz,
                        rate_hz: 20.0,
                        strength_octaves: 4.0,
                        stereo_phase_degrees: 90.0,
                        ..Default::default()
                    };
                    let mut effects = Vec::new();
                    let mut reference: Vec<(Box<dyn EffectUnit>, AutoFilter)> = Vec::new();
                    for mut effect in [
                        EffectInstance::saturator(),
                        EffectInstance::limiter(),
                        EffectInstance::saturator(),
                    ] {
                        let processor: Box<dyn EffectUnit> = match &mut effect.unit {
                            EffectSettings::Saturator(s) => {
                                s.enabled = enabled;
                                Box::new(Saturator::new(context, s))
                            }
                            EffectSettings::Limiter(s) => {
                                s.enabled = enabled;
                                Box::new(Limiter::new(context, s))
                            }
                            _ => unreachable!(),
                        };
                        effects.push(effect);
                        let mut filter = EffectInstance::auto_filter();
                        filter.unit = EffectSettings::AutoFilter(filter_settings.clone());
                        effects.push(filter);
                        reference.push((processor, AutoFilter::new(context, &filter_settings)));
                    }
                    let mut chain = EffectChain::chain(None, &effects, rate, 512);
                    let mut peak = 0.0_f32;
                    for tick in 0..u64::from(Limiter::latency_for_rate(rate)) + 4096 {
                        let frame =
                            region.map_or(tick, |(start, end)| start + tick % (end - start));
                        let tone =
                            (tick as f32 * 300.0 * std::f32::consts::TAU / rate as f32).sin() * 0.2;
                        let input = [tone, -tone * 0.5];
                        let mut expected = input;
                        let mut delay = 0_u64;
                        for (processor, filter) in &mut reference {
                            expected = processor.process_sample(expected);
                            delay += u64::from(processor.latency_frames());
                            // Independent clock oracle: signed offset from the loop origin.
                            let clock = region.map_or_else(
                                || frame.saturating_sub(delay),
                                |(start, end)| {
                                    start
                                        + (i128::from(frame - start) - i128::from(delay))
                                            .rem_euclid(i128::from(end - start))
                                            as u64
                                },
                            );
                            expected = filter.process_at(expected, clock, 120.0);
                        }
                        let actual = chain
                            .sample(input, Some(false), frame, 120.0, region, (&[], None))
                            .0;
                        assert_eq!(
                            actual, expected,
                            "rate={rate} enabled={enabled} loop={region:?} tick={tick}"
                        );
                        peak = peak.max(expected[0].abs());
                    }
                    assert!(peak > 1e-4, "reference must contain audible signal");
                }
            }
        }
    }
}
