//! Fixed instrument-wide provider lanes. Reuse bus component state, never its audio mix path.
use crate::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationLaneKind {
    Lfo,
    Envelope,
}
impl ModulationLaneKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lfo => "LFO",
            Self::Envelope => "Envelope",
        }
    }
    /// Base lane identity for a legacy host namespace; the host probes saved-owner collisions.
    pub fn legacy_owner_id(self, namespace: Uuid) -> Uuid {
        Uuid::from_u128(migration_hash(
            ModulationSourceId {
                layer: namespace,
                unit: Uuid::nil(),
            },
            match self {
                Self::Lfo => b'l',
                Self::Envelope => b'e',
            },
        ))
    }
    pub const fn accepts(self, component: InstrumentComponent) -> bool {
        matches!(
            (self, component),
            (Self::Lfo, InstrumentComponent::Lfo(_))
                | (Self::Envelope, InstrumentComponent::Envelope(_))
        )
    }
}
impl InstrumentSettings {
    pub fn modulation_lane_kind(&self, owner: Uuid) -> Option<ModulationLaneKind> {
        if owner == self.lfo_lane.id {
            Some(ModulationLaneKind::Lfo)
        } else if owner == self.envelope_lane.id {
            Some(ModulationLaneKind::Envelope)
        } else {
            None
        }
    }

    pub fn buses(&self) -> impl Iterator<Item = &InstrumentBusSettings> {
        [
            &self.main,
            self.lfo_lane.as_ref(),
            self.envelope_lane.as_ref(),
        ]
        .into_iter()
    }
    pub fn bus(&self, id: Uuid) -> Option<&InstrumentBusSettings> {
        self.buses().find(|b| b.id == id)
    }
    pub fn bus_mut(&mut self, id: Uuid) -> Option<&mut InstrumentBusSettings> {
        [
            &mut self.main,
            self.lfo_lane.as_mut(),
            self.envelope_lane.as_mut(),
        ]
        .into_iter()
        .find(|b| b.id == id)
    }
    pub fn accepts_component(&self, owner: Uuid, component: InstrumentComponent) -> bool {
        self.modulation_lane_kind(owner).map_or_else(
            || {
                !matches!(
                    component,
                    InstrumentComponent::Lfo(_) | InstrumentComponent::Envelope(_)
                ) && (owner == self.main.id || self.layers.iter().any(|l| l.id == owner))
            },
            |lane| lane.accepts(component),
        )
    }

    pub(crate) fn saved_modulation_lanes(
        layers: &[InstrumentLayerSettings],
        main: &InstrumentBusSettings,
        lfo: Option<InstrumentBusSettings>,
        envelope: Option<InstrumentBusSettings>,
    ) -> (InstrumentBusSettings, InstrumentBusSettings) {
        let mut ids: std::collections::HashSet<_> = layers
            .iter()
            .map(|l| l.id)
            .chain([main.id])
            .chain(lfo.as_ref().map(|b| b.id))
            .chain(envelope.as_ref().map(|b| b.id))
            .collect();
        let mut missing = || {
            let mut raw = u128::MAX;
            while !ids.insert(Uuid::from_u128(raw)) {
                raw -= 1;
            }
            InstrumentBusSettings {
                id: Uuid::from_u128(raw),
                ..Default::default()
            }
        };
        (
            lfo.unwrap_or_else(&mut missing),
            envelope.unwrap_or_else(missing),
        )
    }
    pub(crate) fn valid_modulation_lanes(&self) -> bool {
        let neutral = |b: &InstrumentBusSettings| {
            !b.id.is_nil()
                && b.effects.is_empty()
                && b.gain_db == 0.0
                && b.pan == 0.0
                && !b.macro_module.present
                && b.macro_module.controls.is_empty()
                && b.macro_module.modulations.is_empty()
                && b.valid_component_order()
        };
        let mut ids = std::collections::HashSet::new();
        neutral(&self.lfo_lane)
            && neutral(&self.envelope_lane)
            && self.lfo_lane.envelopes.is_empty()
            && self.envelope_lane.lfos.is_empty()
            && self
                .lfo_lane
                .lfos
                .iter()
                .all(|l| ids.insert(l.id) && l.valid())
            && {
                ids.clear();
                self.envelope_lane
                    .envelopes
                    .iter()
                    .all(|e| !e.id.is_nil() && ids.insert(e.id) && e.valid())
            }
    }
    /// Move legacy providers and retain route identities. Returned remaps also apply to track FX.
    /// Deterministic collision IDs keep repeated legacy opens and history snapshots consistent.
    pub fn collect_modulation_lanes(&mut self) -> Vec<(ModulationSourceId, ModulationSourceId)> {
        let mut used: std::collections::HashSet<_> =
            self.modulation_sources().map(|s| s.id.unit).collect();
        let mut remaps = Vec::new();
        let mut old = Vec::new();
        for layer in &mut self.layers {
            let order = layer.retained_units();
            layer.envelopes.sort_by_key(|e| {
                order
                    .iter()
                    .position(|c| *c == InstrumentComponent::Envelope(e.id))
            });
            layer.lfos.sort_by_key(|l| {
                order
                    .iter()
                    .position(|c| *c == InstrumentComponent::Lfo(l.id))
            });
            old.push((
                layer.id,
                std::mem::take(&mut layer.envelopes),
                std::mem::take(&mut layer.lfos),
            ));
            layer.prune_component_order();
        }
        let order = self.main.retained_units();
        self.main.envelopes.sort_by_key(|e| {
            order
                .iter()
                .position(|c| *c == InstrumentComponent::Envelope(e.id))
        });
        self.main.lfos.sort_by_key(|l| {
            order
                .iter()
                .position(|c| *c == InstrumentComponent::Lfo(l.id))
        });
        old.push((
            self.main.id,
            std::mem::take(&mut self.main.envelopes),
            std::mem::take(&mut self.main.lfos),
        ));
        self.main.prune_component_order();
        let envelope_ids = provider_ids(
            old.iter().flat_map(|(owner, envelopes, _)| {
                envelopes.iter().map(move |e| ModulationSourceId {
                    layer: *owner,
                    unit: e.id,
                })
            }),
            self.envelope_lane.envelopes.iter().map(|e| e.id),
            &mut used,
            b'E',
        );
        let lfo_ids = provider_ids(
            old.iter().flat_map(|(owner, _, lfos)| {
                lfos.iter().map(move |l| ModulationSourceId {
                    layer: *owner,
                    unit: l.id,
                })
            }),
            self.lfo_lane.lfos.iter().map(|l| l.id),
            &mut used,
            b'L',
        );
        for (owner, envelopes, lfos) in old {
            for mut e in envelopes {
                let old = ModulationSourceId {
                    layer: owner,
                    unit: e.id,
                };
                e.id = envelope_ids[&old];
                let new = ModulationSourceId {
                    layer: self.envelope_lane.id,
                    unit: e.id,
                };
                remaps.push((old, new));
                self.envelope_lane.envelopes.push(e);
            }
            for mut l in lfos {
                let old = ModulationSourceId {
                    layer: owner,
                    unit: l.id,
                };
                l.id = lfo_ids[&old];
                let new = ModulationSourceId {
                    layer: self.lfo_lane.id,
                    unit: l.id,
                };
                remaps.push((old, new));
                self.lfo_lane.lfos.push(l);
            }
        }
        // Apply all mappings once: a remapped identity must never be interpreted as another old ID.
        let mappings: std::collections::HashMap<_, _> = remaps.iter().copied().collect();
        for layer in &mut self.layers {
            for r in layer.modulation_routes_mut() {
                remap_route(r, &mappings);
            }
        }
        for bus in [
            &mut self.main,
            self.lfo_lane.as_mut(),
            self.envelope_lane.as_mut(),
        ] {
            for r in bus
                .envelopes
                .iter_mut()
                .flat_map(|e| &mut e.modulations)
                .chain(bus.lfos.iter_mut().flat_map(|l| &mut l.modulations))
                .chain(&mut bus.macro_module.modulations)
                .chain(
                    bus.effects
                        .iter_mut()
                        .flat_map(|e| e.unit.modulations_mut()),
                )
            {
                remap_route(r, &mappings);
            }
        }
        remaps
    }
}
fn remap_route(
    route: &mut Modulation,
    remaps: &std::collections::HashMap<ModulationSourceId, ModulationSourceId>,
) {
    if let Some(new) = route.source.and_then(|old| remaps.get(&old)) {
        route.source = Some(*new);
    }
}

// Identity resolution is independent of saved display order. Existing lane IDs take priority;
// otherwise the smallest former owner keeps the original unit ID. Probe hashes in stable order.
fn provider_ids(
    originals: impl Iterator<Item = ModulationSourceId>,
    occupied: impl Iterator<Item = Uuid>,
    used: &mut std::collections::HashSet<Uuid>,
    kind: u8,
) -> std::collections::HashMap<ModulationSourceId, Uuid> {
    let mut originals: Vec<_> = originals.collect();
    originals.sort_by_key(|id| (id.unit, id.layer));
    let mut claimed: std::collections::HashSet<_> = occupied.collect();
    originals
        .into_iter()
        .map(|old| {
            let id = if claimed.insert(old.unit) {
                old.unit
            } else {
                let mut raw = migration_hash(old, kind);
                while raw == 0 || !used.insert(Uuid::from_u128(raw)) {
                    raw = raw.wrapping_add(1);
                }
                let id = Uuid::from_u128(raw);
                claimed.insert(id);
                id
            };
            (old, id)
        })
        .collect()
}

// Fixed byte folding, not DefaultHasher: migration must match across platforms/toolchains.
fn migration_hash(source: ModulationSourceId, domain: u8) -> u128 {
    let mut raw = 0x6c62272e07bb014262b821756295c58d_u128;
    for byte in source
        .layer
        .as_bytes()
        .iter()
        .chain(source.unit.as_bytes())
        .copied()
        .chain([domain])
    {
        raw = (raw ^ u128::from(byte)).wrapping_mul(0x0000000001000000000000000000013b);
    }
    raw
}
