use daw_core::{EffectSettings, Id, Track};
use daw_equalizer::Equalizer;
use daw_unit_core::{EffectUnit, ProcessingContext};

pub(super) struct TrackEffects {
    id: Id,
    mono: bool,
    units: Vec<(Id, Option<Equalizer>)>,
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
                            EffectSettings::Eq3(settings) => {
                                Some(Equalizer::new(context, settings))
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
                && let EffectSettings::Eq3(settings) = &settings.unit
                && let Some(next) = next
            {
                previous.retarget(settings);
                std::mem::swap(next, previous);
            }
        }
    }
    pub fn sample(&mut self, mut input: [f32; 2], mono: Option<bool>) -> ([f32; 2], bool) {
        if let Some(mono) = mono {
            self.mono = mono;
        }
        for (_, unit) in &mut self.units {
            if let Some(unit) = unit {
                input = unit.process_sample(input);
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
