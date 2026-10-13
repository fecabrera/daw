use daw_auto_filter::AutoFilter;
use daw_core::{EffectSettings, Id, Track};
use daw_drive::Drive;
use daw_equalizer::Equalizer;
use daw_unit_core::{EffectUnit, ProcessingContext};

enum RuntimeEffect {
    Eq3(Box<Equalizer>),
    Drive(Box<Drive>),
    AutoFilter(Box<AutoFilter>),
}
impl RuntimeEffect {
    fn sample(&mut self, input: [f32; 2], frame: u64, tempo: f32) -> [f32; 2] {
        match self {
            Self::Eq3(unit) => unit.process_sample(input),
            Self::Drive(unit) => unit.process_sample(input),
            Self::AutoFilter(unit) => unit.process_at(input, frame, tempo),
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Eq3(unit) => unit.reset(),
            Self::Drive(unit) => unit.reset(),
            Self::AutoFilter(unit) => unit.reset(),
        }
    }
    fn retarget(&mut self, settings: &EffectSettings) -> bool {
        match (self, settings) {
            (Self::Eq3(unit), EffectSettings::Eq3(settings)) => unit.retarget(settings),
            (Self::Drive(unit), EffectSettings::Drive(settings)) => unit.retarget(settings),
            (Self::AutoFilter(unit), EffectSettings::AutoFilter(settings)) => {
                unit.retarget(settings)
            }
            _ => return false,
        }
        true
    }
}

pub(super) struct TrackEffects {
    id: Id,
    mono: bool,
    units: Vec<(Id, Option<RuntimeEffect>)>,
}
impl TrackEffects {
    pub fn new(track: &Track, sample_rate_hz: u32, max_block_frames: u32) -> Self {
        let context = ProcessingContext {
            sample_rate_hz,
            max_block_frames,
        };
        Self {
            id: track.id,
            mono: false,
            units: track
                .effects
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
                            )),
                            EffectSettings::AutoFilter(settings) => {
                                Some(RuntimeEffect::AutoFilter(Box::new(AutoFilter::new(
                                    context, settings,
                                ))))
                            }
                            EffectSettings::Missing { .. } => None,
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
        self.mono = previous.mono;
        for ((id, next), settings) in self.units.iter_mut().zip(&track.effects) {
            if let Some((_, Some(previous))) =
                previous.units.iter_mut().find(|(old_id, _)| id == old_id)
                && let Some(next) = next
                && previous.retarget(&settings.unit)
            {
                std::mem::swap(next, previous);
            }
        }
    }
    pub fn sample(
        &mut self,
        mut input: [f32; 2],
        mono: Option<bool>,
        frame: u64,
        tempo: f32,
    ) -> ([f32; 2], bool) {
        if let Some(mono) = mono {
            self.mono = mono;
        }
        for (_, unit) in &mut self.units {
            if let Some(unit) = unit {
                input = unit.sample(input, frame, tempo);
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
