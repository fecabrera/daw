//! Complete effect-chain ownership for validation and host diagnostics.
use crate::{EffectChainTarget, EffectInstance, Id, InstrumentSettings, Project};

fn instrument_chains(
    owner: Id,
    instrument: &InstrumentSettings,
) -> impl Iterator<Item = (EffectChainTarget, &[EffectInstance])> {
    instrument
        .layers
        .iter()
        .map(move |layer| {
            (
                EffectChainTarget::Layer {
                    track: owner,
                    layer: layer.id,
                },
                layer.effects.as_slice(),
            )
        })
        .chain(std::iter::once((
            EffectChainTarget::Layer {
                track: owner,
                layer: instrument.main.id,
            },
            instrument.main.effects.as_slice(),
        )))
}

impl Project {
    /// Includes ordinary and nested instrument buses without allocating owner labels.
    pub fn effect_chains(&self) -> impl Iterator<Item = (EffectChainTarget, &[EffectInstance])> {
        self.tracks
            .iter()
            .flat_map(|track| {
                std::iter::once((EffectChainTarget::Track(track.id), track.effects.as_slice()))
                    .chain(
                        track
                            .instrument
                            .iter()
                            .flat_map(move |i| instrument_chains(track.id, i)),
                    )
                    .chain(track.beat.iter().flat_map(|beat| {
                        std::iter::once((
                            EffectChainTarget::Layer {
                                track: beat.output.id,
                                layer: beat.output.id,
                            },
                            beat.output.effects.as_slice(),
                        ))
                        .chain(
                            beat.subtracks
                                .iter()
                                .flat_map(|row| instrument_chains(row.id, &row.instrument)),
                        )
                    }))
            })
            .chain(std::iter::once((
                EffectChainTarget::Master,
                self.master.effects.as_slice(),
            )))
    }
}
