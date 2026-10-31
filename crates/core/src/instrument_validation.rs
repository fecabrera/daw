//! Host-side source ownership checks shared by MIDI and beat subtrack instruments.
use crate::{EffectInstance, Error, InstrumentSettings, Result};

pub(crate) fn validate_routes(
    instrument: Option<&InstrumentSettings>,
    effects: &[EffectInstance],
) -> Result<()> {
    for route in instrument
        .into_iter()
        .flat_map(|i| &i.layers)
        .flat_map(|l| {
            l.amplitude
                .modulations
                .iter()
                .chain(&l.oscillator_modulations)
                .chain(l.sampler.iter().flat_map(|s| &s.modulations))
        })
    {
        if route
            .source
            .is_some_and(|source| instrument.is_none_or(|i| i.modulation_source(source).is_none()))
        {
            return Err(Error(
                "Instrument modulation source does not exist in this instrument".into(),
            ));
        }
    }
    for route in effects
        .iter()
        .chain(instrument.into_iter().flat_map(|i| {
            i.layers
                .iter()
                .flat_map(|l| &l.effects)
                .chain(&i.main.effects)
        }))
        .flat_map(|e| e.unit.modulations())
    {
        if route
            .source
            .is_some_and(|source| !crate::effect_transfer::source_available(instrument, source))
        {
            return Err(Error(
                "Effect modulation requires a shared source in the same instrument".into(),
            ));
        }
    }
    Ok(())
}
