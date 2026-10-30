//! Legacy provider placement migrates together with its receiving track effects.
use crate::*;
impl Track {
    pub fn collect_modulation_lanes(&mut self) {
        if let Some(instrument) = &mut self.instrument {
            let remaps: std::collections::HashMap<_, _> =
                instrument.collect_modulation_lanes().into_iter().collect();
            for route in self
                .effects
                .iter_mut()
                .flat_map(|e| e.unit.modulations_mut())
            {
                if let Some(new) = route.source.and_then(|old| remaps.get(&old)) {
                    route.source = Some(*new);
                }
            }
        }
    }
}

/// The saved track is the stable namespace, even when older instruments share numeric owner IDs.
pub(super) fn legacy_owners(track: &mut Track, missing_lfo: bool, missing_envelope: bool) {
    let Some(i) = &mut track.instrument else {
        return;
    };
    let mut occupied: std::collections::HashSet<_> = i
        .layers
        .iter()
        .map(|l| l.id)
        .chain([i.main.id])
        .chain((!missing_lfo).then_some(i.lfo_lane.id))
        .chain((!missing_envelope).then_some(i.envelope_lane.id))
        .collect();
    for (lane, kind, missing) in [
        (i.lfo_lane.as_mut(), ModulationLaneKind::Lfo, missing_lfo),
        (
            i.envelope_lane.as_mut(),
            ModulationLaneKind::Envelope,
            missing_envelope,
        ),
    ] {
        if missing {
            let mut id = kind.legacy_owner_id(track.id);
            while id.is_nil() || !occupied.insert(id) {
                id = Id::from_u128(id.as_u128().wrapping_add(1));
            }
            lane.id = id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_project_providers_migrate_with_routes_modes_and_collision_safe_identities() {
        let mut p = Project::default();
        p.add_midi_track().unwrap();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.layers[0].id = Id::from_u128(10);
        let mut second = InstrumentLayerSettings {
            id: Id::from_u128(20),
            ..Default::default()
        };
        second.envelopes[0].present = true;
        second.envelopes[0].mode = EnvelopeMode::Legato;
        second.envelopes[0].settings.attack.time_ms = 123.0;
        let old_shared = ModulationSourceId {
            layer: second.id,
            unit: second.envelopes[0].id,
        };
        i.layers[0].envelopes[0].present = true;
        let old_note = ModulationSourceId {
            layer: i.layers[0].id,
            unit: i.layers[0].envelopes[0].id,
        };
        let route = Modulation {
            source: Some(old_note),
            parameter: ModulationParameter::Amplitude,
            amount: 1.0,
            ..Modulation::new(ModulationParameter::Amplitude)
        };
        let route_id = route.id;
        i.layers[0].amplitude.modulations.push(route);
        i.layers.push(second);
        i.main.lfos.push(Default::default());
        i.layers[0].lfos.push(Default::default());
        let ids: Vec<_> = i.lfo_destinations().map(|(id, _)| id.unit).collect();
        let mut effect = EffectInstance::auto_filter();
        if let EffectSettings::AutoFilter(s) = &mut effect.unit {
            s.modulations.push(Modulation {
                source: Some(old_shared),
                amount: 1.0,
                ..Modulation::new(ModulationParameter::Frequency)
            });
        }
        p.tracks[0].effects.push(effect);
        p.validate().unwrap();
        let mut saved = serde_json::to_value(&p).unwrap();
        let instrument = saved["tracks"][0]["instrument"].as_object_mut().unwrap();
        instrument.insert("version".into(), 3.into());
        instrument.remove("lfo_lane");
        instrument.remove("envelope_lane");
        let loaded: Project = serde_json::from_value(saved.clone()).unwrap();
        loaded.validate().unwrap();
        let i = loaded.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(i.version, daw_unit_core::INSTRUMENT_STATE_VERSION);
        assert!(
            i.layers
                .iter()
                .all(|l| l.envelopes.is_empty() && l.lfos.is_empty())
        );
        assert!(i.main.envelopes.is_empty() && i.main.lfos.is_empty());
        assert!(
            ids.iter()
                .all(|id| i.lfo_lane.lfos.iter().any(|l| l.id == *id))
        );
        let route = &i.layers[0].amplitude.modulations[0];
        assert_eq!(route.id, route_id);
        assert_eq!(
            route.source,
            Some(ModulationSourceId {
                layer: i.envelope_lane.id,
                unit: old_note.unit
            })
        );
        let shared = loaded.tracks[0].effects[0].unit.modulations()[0]
            .source
            .unwrap();
        assert_eq!(shared.layer, i.envelope_lane.id);
        assert_ne!(
            shared.unit, old_shared.unit,
            "duplicate legacy envelope IDs need distinct addresses"
        );
        let e = i
            .envelope_lane
            .envelopes
            .iter()
            .find(|e| e.id == shared.unit)
            .unwrap();
        assert_eq!(e.mode, EnvelopeMode::Legato);
        assert_eq!(e.settings.attack.time_ms, 123.0);
        let mut reordered = saved.clone();
        reordered["tracks"][0]["instrument"]["layers"]
            .as_array_mut()
            .unwrap()
            .swap(0, 1);
        let reordered: Project = serde_json::from_value(reordered).unwrap();
        let reordered_i = reordered.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(reordered.tracks[0].effects, loaded.tracks[0].effects);
        let reordered_route = reordered_i
            .layers
            .iter()
            .flat_map(|l| &l.amplitude.modulations)
            .find(|r| r.id == route_id)
            .unwrap();
        assert_eq!(
            reordered_route.source, route.source,
            "old layer order must not change provider identity"
        );
        let repeated: Project = serde_json::from_value(saved).unwrap();
        assert_eq!(repeated, loaded);
        assert_eq!(
            serde_json::from_value::<Project>(serde_json::to_value(&loaded).unwrap()).unwrap(),
            loaded
        );
    }
    #[test]
    fn fixed_provider_lanes_reject_other_components_and_copy_reorder_without_losing_identity() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        p.tracks[0].collect_modulation_lanes();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.lfo_lane.lfos = vec![Default::default(), Default::default()];
        let (owner, other, regular, output) =
            (i.lfo_lane.id, i.envelope_lane.id, i.layers[0].id, i.main.id);
        let (a, b) = (i.lfo_lane.lfos[0].id, i.lfo_lane.lfos[1].id);
        for target in [other, regular, output] {
            assert!(!p.can_transfer_component(
                track,
                owner,
                target,
                InstrumentComponent::Lfo(a),
                false
            ));
            assert!(!p.can_transfer_component(
                track,
                owner,
                target,
                InstrumentComponent::Lfo(a),
                true
            ));
        }
        assert!(!p.can_transfer_component(
            track,
            regular,
            owner,
            InstrumentComponent::Amplitude,
            false
        ));
        let reorder = p
            .component_group_transfer(
                track,
                owner,
                owner,
                &[InstrumentComponent::Lfo(a)],
                1,
                false,
            )
            .unwrap();
        p.edit(reorder.edit).unwrap();
        assert_eq!(
            p.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .lfo_lane
                .visible_components(),
            [InstrumentComponent::Lfo(b), InstrumentComponent::Lfo(a)]
        );
        let copy = p
            .component_group_transfer(track, owner, owner, &[InstrumentComponent::Lfo(a)], 2, true)
            .unwrap();
        p.edit(copy.edit).unwrap();
        assert_eq!(
            p.tracks[0].instrument.as_ref().unwrap().lfo_lane.lfos.len(),
            3
        );
        p.validate().unwrap();
    }
    #[test]
    fn malformed_lane_types_ids_and_missing_current_revision_fields_are_rejected() {
        let mut p = Project::default();
        p.add_midi_track().unwrap();
        p.tracks[0].collect_modulation_lanes();
        let saved = serde_json::to_value(&p).unwrap();
        for field in ["lfo_lane", "envelope_lane"] {
            let mut missing = saved.clone();
            missing["tracks"][0]["instrument"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(serde_json::from_value::<Project>(missing).is_err());
            let mut null = saved.clone();
            null["tracks"][0]["instrument"][field] = serde_json::Value::Null;
            assert!(serde_json::from_value::<Project>(null).is_err());
        }
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.lfo_lane.envelopes.push(Default::default());
        assert!(p.validate().is_err());
        p.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .lfo_lane
            .envelopes
            .clear();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.envelope_lane.id = i.lfo_lane.id;
        assert!(p.validate().is_err());
    }
    #[test]
    fn migrated_lane_owners_are_track_scoped_even_for_identical_legacy_instruments() {
        let mut p = Project::default();
        p.add_midi_track().unwrap();
        p.add_midi_track().unwrap();
        let mut legacy = p.tracks[0].instrument.clone().unwrap();
        legacy.layers[0].envelopes[0].present = true;
        legacy.layers[0].envelopes[0].mode = EnvelopeMode::Legato;
        for track in &mut p.tracks {
            track.instrument = Some(legacy.clone());
        }
        let mut saved = serde_json::to_value(&p).unwrap();
        for track in saved["tracks"].as_array_mut().unwrap() {
            let i = track["instrument"].as_object_mut().unwrap();
            i.insert("version".into(), 3.into());
            i.remove("lfo_lane");
            i.remove("envelope_lane");
        }
        let loaded: Project = serde_json::from_value(saved).unwrap();
        loaded.validate().unwrap();
        let a = loaded.tracks[0].instrument.as_ref().unwrap();
        let b = loaded.tracks[1].instrument.as_ref().unwrap();
        assert_ne!(a.lfo_lane.id, b.lfo_lane.id);
        assert_ne!(a.envelope_lane.id, b.envelope_lane.id);
        let source = a.modulation_sources().find(|s| s.present).unwrap().id;
        assert!(
            b.modulation_source(source).is_none(),
            "a source cannot become an alias in another instrument"
        );
    }
}
