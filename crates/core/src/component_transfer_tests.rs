use crate::*;
use InstrumentComponent::*;

#[test]
fn transferred_units_insert_by_resolved_identity_and_reject_invalid_positions_atomically() {
    for main in [false, true] {
        for copy in [false, true] {
            let (mut p, track, from, regular) = fixture();
            let i = p.tracks[0].instrument.as_mut().unwrap();
            let id = i.layers[0].envelopes[0].id;
            let to = if main {
                i.main.lfos.push(LfoInstance::default());
                i.main.id
            } else {
                i.layers[1].oscillators.push(OscillatorSettings::default());
                i.layers[1].lfos.push(LfoInstance::default());
                regular
            };
            let first = usize::from(!main);
            let snapshot = serde_json::to_value(&p).unwrap();
            for index in [999, if main { 999 } else { 0 }] {
                assert!(
                    p.edit(Edit::TransferInstrumentComponent {
                        track_id: track,
                        source: from,
                        destination: to,
                        component: Envelope(id),
                        destination_index: Some(index),
                        duplicate: copy,
                    })
                    .is_err()
                );
                assert_eq!(serde_json::to_value(&p).unwrap(), snapshot);
            }
            p.edit(Edit::TransferInstrumentComponent {
                track_id: track,
                source: from,
                destination: to,
                component: Envelope(id),
                destination_index: Some(first),
                duplicate: copy,
            })
            .unwrap();
            let i = p.tracks[0].instrument.as_ref().unwrap();
            let (order, moved) = if main {
                (
                    i.main.visible_components(),
                    i.main.envelopes.last().unwrap().id,
                )
            } else {
                (
                    i.layers[1].visible_components(),
                    i.layers[1].envelopes.last().unwrap().id,
                )
            };
            assert_eq!(order[first], Envelope(moved));
            if copy || !main {
                assert_ne!(moved, id);
            }
            p.validate().unwrap();
        }
    }
}

#[test]
fn unit_reorder_is_atomic_preserves_settings_and_prunes_transferred_provider_positions() {
    let (mut project, track, owner, destination) = fixture();
    let before = project.tracks[0].instrument.clone().unwrap();
    project
        .edit(Edit::TransferInstrumentComponent {
            track_id: track,
            source: owner,
            destination: owner,
            destination_index: Some(1),
            component: Amplitude,
            duplicate: false,
        })
        .unwrap();
    let after = project.tracks[0].instrument.clone().unwrap();
    assert_eq!(
        after.layers[0].visible_components()[..2],
        [Oscillators, Amplitude]
    );
    let mut expected = before;
    expected.layers[0].component_order = after.layers[0].component_order.clone();
    assert_eq!(after, expected);
    for (unit, index) in [
        (Amplitude, Some(0)),
        (Oscillators, Some(1)),
        (Amplitude, Some(999)),
        (Amplitude, None),
    ] {
        let snapshot = serde_json::to_value(&project).unwrap();
        assert!(
            project
                .edit(Edit::TransferInstrumentComponent {
                    track_id: track,
                    source: owner,
                    destination: owner,
                    destination_index: index,
                    component: unit,
                    duplicate: false
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), snapshot);
    }
    let id = after.layers[0].envelopes[0].id;
    project
        .edit(Edit::TransferInstrumentComponent {
            track_id: track,
            source: owner,
            destination,
            destination_index: None,
            component: Envelope(id),
            duplicate: false,
        })
        .unwrap();
    assert!(
        !project.tracks[0].instrument.as_ref().unwrap().layers[0]
            .component_order
            .contains(&Envelope(id))
    );
    project.validate().unwrap();
}

fn fixture() -> (Project, Id, Id, Id) {
    let mut p = Project::default();
    let track = p.add_midi_track().unwrap();
    let i = p.tracks[0].instrument.as_mut().unwrap();
    i.layers.push(InstrumentLayerSettings::default());
    i.layers[0].oscillators = vec![OscillatorSettings::default(), OscillatorSettings::default()];
    i.layers[0].envelopes[0].present = true;
    i.layers[0].envelopes[0].mode = EnvelopeMode::Legato;
    let source = ModulationSourceId {
        layer: i.layers[0].id,
        unit: i.layers[0].envelopes[0].id,
    };
    i.layers[0].amplitude.present = true;
    i.layers[0].amplitude.modulations.push(Modulation {
        source: Some(source),
        amount: 1.0,
        ..Modulation::new(ModulationParameter::Amplitude)
    });
    let oscillator = i.layers[0].oscillators[0].id;
    i.layers[0].oscillator_modulations.push(Modulation {
        source: Some(source),
        amount: 12.0,
        target: Some(oscillator),
        ..Modulation::new(ModulationParameter::Pitch)
    });
    let owners = [i.layers[0].id, i.layers[1].id];
    let mut filter = EffectInstance::auto_filter();
    if let EffectSettings::AutoFilter(s) = &mut filter.unit {
        s.modulations.push(Modulation {
            source: Some(source),
            amount: 2.0,
            ..Modulation::new(ModulationParameter::Frequency)
        });
    }
    p.tracks[0].effects.push(filter);
    p.validate().unwrap();
    (p, track, owners[0], owners[1])
}
fn transfer(p: &mut Project, t: Id, a: Id, b: Id, c: InstrumentComponent, duplicate: bool) {
    p.edit(Edit::TransferInstrumentComponent {
        track_id: t,
        source: a,
        destination: b,
        destination_index: None,
        component: c,
        duplicate,
    })
    .unwrap();
}
#[test]
fn provider_move_updates_every_consumer_and_resolves_legacy_identity_collisions() {
    let (mut p, t, a, b) = fixture();
    let before = p.tracks[0].instrument.as_ref().unwrap().layers[0].envelopes[0].clone();
    let id = before.id;
    let mut effect = p.tracks[0].effects[0].clone();
    effect.id = Id::new_v4();
    p.tracks[0].instrument.as_mut().unwrap().layers[1]
        .effects
        .push(effect);
    transfer(&mut p, t, a, b, Envelope(id), false);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert!(!i.layers[0].envelopes.iter().any(|e| e.id == id));
    let moved = i.layers[1].envelopes.last().unwrap();
    assert_ne!(moved.id, id); // Other layer retains legacy ID 1 even while absent.
    let mut expected = before;
    expected.id = moved.id;
    assert_eq!(moved, &expected);
    let new = Some(ModulationSourceId {
        layer: b,
        unit: moved.id,
    });
    assert_eq!(i.layers[0].amplitude.modulations[0].source, new);
    assert_eq!(i.layers[0].oscillator_modulations[0].source, new);
    assert_eq!(p.tracks[0].effects[0].unit.modulations()[0].source, new);
    assert_eq!(i.layers[1].effects[0].unit.modulations()[0].source, new);
    let saved = serde_json::to_string(&p).unwrap();
    let loaded: Project = serde_json::from_str(&saved).unwrap();
    loaded.validate().unwrap();
    assert_eq!(serde_json::to_string(&loaded).unwrap(), saved);
}
#[test]
fn provider_copies_have_new_identity_and_leave_all_existing_connections_on_original() {
    let (mut p, t, a, b) = fixture();
    let before = p.tracks[0].clone();
    let id = before.instrument.as_ref().unwrap().layers[0].envelopes[0].id;
    transfer(&mut p, t, a, b, Envelope(id), true);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.layers[0], before.instrument.as_ref().unwrap().layers[0]);
    assert_ne!(i.layers[1].envelopes.last().unwrap().id, id);
    assert_eq!(p.tracks[0].effects, before.effects);
}
#[test]
fn oscillator_moves_merge_and_take_only_their_targeted_routes() {
    let (mut p, t, a, b) = fixture();
    let before = p.tracks[0].instrument.as_ref().unwrap().layers[0].clone();
    p.tracks[0].instrument.as_mut().unwrap().layers[1]
        .oscillators
        .push(OscillatorSettings::default());
    transfer(&mut p, t, a, b, Oscillator(before.oscillators[0].id), false);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.layers[0].oscillators, vec![before.oscillators[1].clone()]);
    assert!(i.layers[0].oscillator_modulations.is_empty());
    assert_eq!(i.layers[1].oscillators[1], before.oscillators[0]);
    assert_eq!(
        i.layers[1].oscillator_modulations,
        before.oscillator_modulations
    );
}
#[test]
fn oscillator_copy_remaps_targets_routes_and_ids_without_changing_sources() {
    for whole in [false, true] {
        let (mut p, t, a, _) = fixture();
        let before = p.tracks[0].instrument.as_ref().unwrap().layers[0].clone();
        transfer(
            &mut p,
            t,
            a,
            a,
            if whole {
                Oscillators
            } else {
                Oscillator(before.oscillators[0].id)
            },
            true,
        );
        let l = &p.tracks[0].instrument.as_ref().unwrap().layers[0];
        assert_eq!(l.oscillators.len(), if whole { 4 } else { 3 });
        assert_eq!(&l.oscillators[..2], &before.oscillators);
        assert_ne!(l.oscillators[2].id, before.oscillators[0].id);
        assert_eq!(
            l.oscillator_modulations[1].target,
            Some(l.oscillators[2].id)
        );
        assert_ne!(
            l.oscillator_modulations[1].id,
            before.oscillator_modulations[0].id
        );
        assert_eq!(
            l.oscillator_modulations[1].source,
            before.oscillator_modulations[0].source
        );
    }
}
#[test]
fn main_accepts_shared_providers_and_rejects_audio_and_per_note_components_atomically() {
    let (mut p, t, a, b) = fixture();
    let main = p.tracks[0].instrument.as_ref().unwrap().main.id;
    let e = p.tracks[0].instrument.as_ref().unwrap().layers[0].envelopes[0].id;
    for c in [Oscillators, Amplitude, Envelope(Id::new_v4())] {
        let before = serde_json::to_value(&p).unwrap();
        assert!(!p.can_transfer_component(t, a, main, c, false));
        assert!(
            p.edit(Edit::TransferInstrumentComponent {
                track_id: t,
                source: a,
                destination: main,
                destination_index: None,
                component: c,
                duplicate: false
            })
            .is_err()
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
    }
    p.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].mode = EnvelopeMode::PerNote;
    assert!(!p.can_transfer_component(t, a, main, Envelope(e), true));
    p.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].mode = EnvelopeMode::Legato;
    transfer(&mut p, t, a, main, Envelope(e), false);
    assert_eq!(
        p.tracks[0].instrument.as_ref().unwrap().main.envelopes[0].id,
        e
    );
    transfer(&mut p, t, main, b, Envelope(e), false);
    assert!(
        p.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .main
            .envelopes
            .is_empty()
    );
}
#[test]
fn amplitude_move_preserves_routes_and_rejects_occupied_slots_without_overwriting() {
    let (mut p, t, a, b) = fixture();
    let original = p.tracks[0].instrument.as_ref().unwrap().layers[0]
        .amplitude
        .clone();
    transfer(&mut p, t, a, b, Amplitude, true);
    assert!(!p.can_transfer_component(t, a, b, Amplitude, false));
    let i = p.tracks[0].instrument.as_ref().unwrap();
    let mut copied = i.layers[1].amplitude.clone();
    assert_ne!(copied.modulations[0].id, original.modulations[0].id);
    copied.modulations[0].id = original.modulations[0].id;
    assert_eq!(copied, original);
    p.tracks[0].instrument.as_mut().unwrap().layers[1]
        .amplitude
        .present = false;
    transfer(&mut p, t, a, b, Amplitude, false);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert!(!i.layers[0].amplitude.present);
    assert_eq!(i.layers[1].amplitude, original);
}
#[test]
fn lfo_moves_and_copies_support_main_and_regular_layers() {
    let (mut p, t, a, b) = fixture();
    let main = p.tracks[0].instrument.as_ref().unwrap().main.id;
    let l = LfoInstance::default();
    let id = l.id;
    p.tracks[0].instrument.as_mut().unwrap().layers[0]
        .lfos
        .push(l.clone());
    transfer(&mut p, t, a, main, Lfo(id), false);
    assert_eq!(
        p.tracks[0].instrument.as_ref().unwrap().main.lfos,
        vec![l.clone()]
    );
    transfer(&mut p, t, main, b, Lfo(id), true);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.main.lfos, vec![l.clone()]);
    let mut copy = i.layers[1].lfos[0].clone();
    assert_ne!(copy.id, id);
    copy.id = id;
    assert_eq!(copy, l);
}

#[test]
fn oscillator_reorder_preserves_route_order_and_rejects_invalid_indices_atomically() {
    let (mut p, t, a, _) = fixture();
    let before = p.tracks[0].instrument.as_ref().unwrap().layers[0].clone();
    p.edit(Edit::TransferInstrumentComponent {
        track_id: t,
        source: a,
        destination: a,
        destination_index: Some(1),
        component: Oscillator(before.oscillators[0].id),
        duplicate: false,
    })
    .unwrap();
    let l = &p.tracks[0].instrument.as_ref().unwrap().layers[0];
    assert_eq!(
        l.oscillators,
        [before.oscillators[1].clone(), before.oscillators[0].clone()]
    );
    assert_eq!(l.oscillator_modulations, before.oscillator_modulations);
    let saved = serde_json::to_value(&p).unwrap();
    assert!(
        p.edit(Edit::TransferInstrumentComponent {
            track_id: t,
            source: a,
            destination: a,
            destination_index: Some(12),
            component: Oscillator(before.oscillators[0].id),
            duplicate: false
        })
        .is_err()
    );
    assert_eq!(serde_json::to_value(&p).unwrap(), saved);
}

#[test]
fn lfo_destinations_move_with_routes_copy_fresh_routes_and_follow_provider_moves() {
    let (mut p, t, a, b) = fixture();
    let i = p.tracks[0].instrument.as_mut().unwrap();
    let source = ModulationSourceId {
        layer: a,
        unit: i.layers[0].envelopes[0].id,
    };
    let envelope = source.unit;
    let mut lfo = LfoInstance {
        modulation_open: true,
        ..Default::default()
    };
    lfo.modulations.push(Modulation {
        source: Some(source),
        amount: 1.0,
        ..Modulation::new(ModulationParameter::LfoDepth)
    });
    let id = lfo.id;
    let route = lfo.modulations[0];
    i.main.lfos.push(lfo);
    transfer(&mut p, t, a, b, Envelope(envelope), false);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.main.lfos[0].modulations[0].source.unwrap().layer, b);
    let main = i.main.id;
    transfer(&mut p, t, main, a, Lfo(id), true);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    let copy = &i.layers[0].lfos[0];
    assert_ne!(copy.id, id);
    assert_ne!(copy.modulations[0].id, route.id);
    assert!(copy.modulation_open);
    assert_eq!(
        copy.modulations[0].source,
        i.main.lfos[0].modulations[0].source
    );
    p.validate().unwrap();
    let before = serde_json::to_value(&p).unwrap();
    let mut invalid = p.tracks[0].instrument.clone().unwrap();
    invalid.main.lfos[0].modulations[0].source = Some(ModulationSourceId {
        layer: main,
        unit: id,
    });
    assert!(
        p.edit(Edit::SetInstrument {
            track_id: t,
            instrument: Some(invalid)
        })
        .is_err()
    );
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
}

#[test]
fn macro_module_moves_merge_strips_remap_every_consumer_and_copy_fresh_targets() {
    let (mut p, track, a, b) = fixture();
    let i = p.tracks[0].instrument.as_mut().unwrap();
    let collision = Id::new_v4();
    i.layers[1].envelopes[0].id = collision;
    let module = &mut i.layers[0].macro_module;
    module.add();
    module.add();
    module.modulation_open = true;
    module.controls[0].name = Some("Tone".into());
    // Force a collision with a retained Envelope on the destination owner.
    module.controls[0].id = collision;
    let id = module.controls[0].id;
    let source = ModulationSourceId { layer: a, unit: id };
    module.modulations.push(Modulation {
        target: Some(id),
        source: None,
        amount: 0.4,
        ..Modulation::new(ModulationParameter::MacroValue)
    });
    i.layers[0].amplitude.modulations[0].source = Some(source);
    let mut lfo = LfoInstance::default();
    lfo.modulations.push(Modulation {
        source: Some(source),
        amount: 0.5,
        ..Modulation::new(ModulationParameter::LfoDepth)
    });
    i.main.lfos.push(lfo);
    i.layers[1].macro_module.add();
    let destination_id = i.layers[1].macro_module.controls[0].id;
    transfer(&mut p, track, a, b, Macros, false);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    let moved = i.layers[1].macro_module.controls[1].id;
    assert_eq!(
        i.layers[1].macro_module.controls[1].name.as_deref(),
        Some("Tone")
    );
    assert_ne!(id, moved);
    assert_eq!(i.layers[1].macro_module.controls[0].id, destination_id);
    assert!(!i.layers[0].macro_module.present);
    let new = Some(ModulationSourceId {
        layer: b,
        unit: moved,
    });
    assert_eq!(i.layers[0].amplitude.modulations[0].source, new);
    assert_eq!(i.main.lfos[0].modulations[0].source, new);
    assert_eq!(i.layers[1].macro_module.modulations[0].target, Some(moved));
    let main = i.main.id;
    let before = i.layers[1].macro_module.clone();
    transfer(&mut p, track, b, main, Macros, true);
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.layers[1].macro_module, before);
    assert_eq!(i.main.macro_module.controls.len(), before.controls.len());
    for (copy, original) in i.main.macro_module.controls.iter().zip(&before.controls) {
        assert_ne!(copy.id, original.id);
        assert_eq!(copy.settings, original.settings);
        assert_eq!(copy.name, original.name);
    }
    assert_ne!(
        i.main.macro_module.modulations[0].id,
        before.modulations[0].id
    );
    assert_eq!(
        i.main.macro_module.modulations[0].target,
        Some(i.main.macro_module.controls[1].id)
    );
    p.validate().unwrap();
    let snapshot = serde_json::to_vec(&p).unwrap();
    let mut invalid = p.tracks[0].instrument.clone().unwrap();
    invalid.main.macro_module.controls[0].settings.value = f32::NAN;
    assert!(
        p.edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(invalid)
        })
        .is_err()
    );
    assert_eq!(serde_json::to_vec(&p).unwrap(), snapshot);
}

#[test]
fn component_group_reorder_preserves_source_prefix_and_all_processing_settings() {
    let (mut p, t, a, _) = fixture();
    let i = p.tracks[0].instrument.as_mut().unwrap();
    i.layers[0].lfos.push(LfoInstance::default());
    i.layers[0].macro_module.add();
    let before = i.clone();
    let envelope = Envelope(i.layers[0].envelopes[0].id);
    let lfo = Lfo(i.layers[0].lfos[0].id);
    // Input selection order must not determine the resulting display order.
    let prepared = p
        .component_group_transfer(t, a, a, &[Macros, envelope], 3, false)
        .unwrap();
    assert_eq!(prepared.components, [envelope, Macros]);
    assert!(prepared.changed);
    p.edit(prepared.edit).unwrap();
    let actual = p.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(
        actual.layers[0].visible_components(),
        [Oscillators, lfo, Amplitude, envelope, Macros]
    );
    let mut expected = before;
    expected.layers[0].component_order = actual.layers[0].component_order.clone();
    assert_eq!(actual, &expected);
    let prepared = p
        .component_group_transfer(t, a, a, &[envelope, Oscillators], 4, false)
        .unwrap();
    assert!(prepared.changed);
    p.edit(prepared.edit).unwrap();
    assert_eq!(
        p.tracks[0].instrument.as_ref().unwrap().layers[0].visible_components(),
        [Oscillators, lfo, Amplitude, Macros, envelope]
    );
    // Pinned sources do not make an otherwise unchanged group look like a move.
    assert!(
        !p.component_group_transfer(t, a, a, &[Oscillators, lfo], 1, false)
            .unwrap()
            .changed
    );
    let snapshot = serde_json::to_value(&p).unwrap();
    for (units, index) in [
        (vec![envelope, Macros], 0),
        (vec![envelope, envelope], 1),
        (vec![], 1),
        (vec![envelope, Macros], 999),
    ] {
        assert!(
            p.component_group_transfer(t, a, a, &units, index, false)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), snapshot);
    }
}

#[test]
fn component_group_transfers_resolve_ids_merge_units_and_reject_the_whole_invalid_drop() {
    for duplicate in [false, true] {
        for main in [false, true] {
            let (mut p, t, a, b) = fixture();
            let i = p.tracks[0].instrument.as_mut().unwrap();
            let envelope = Envelope(i.layers[0].envelopes[0].id);
            let source = i.layers[0].amplitude.modulations[0].source;
            let mut lfo = LfoInstance::default();
            lfo.modulations.push(Modulation {
                source,
                amount: 0.5,
                ..Modulation::new(ModulationParameter::LfoDepth)
            });
            let lfo_id = lfo.id;
            i.layers[0].lfos.push(lfo);
            i.layers[0].macro_module.add();
            let to = if main {
                i.main.macro_module.add();
                i.main.id
            } else {
                i.layers[1].oscillators.push(OscillatorSettings::default());
                i.layers[1].macro_module.add();
                b
            };
            let units = vec![Macros, Lfo(lfo_id), envelope];
            let snapshot = serde_json::to_value(&p).unwrap();
            let mut invalid = units.clone();
            invalid.push(Amplitude);
            if main {
                assert!(!p.can_transfer_components(t, a, to, &invalid, duplicate));
                assert!(
                    p.component_group_transfer(t, a, to, &invalid, 0, duplicate)
                        .is_err()
                );
            }
            assert!(
                p.component_group_transfer(t, a, to, &units, 999, duplicate)
                    .is_err()
            );
            assert_eq!(serde_json::to_value(&p).unwrap(), snapshot);
            let result = p
                .component_group_transfer(t, a, to, &units, usize::from(!main), duplicate)
                .unwrap();
            let placed = result.components.clone();
            assert_eq!(
                serde_json::to_value(&p).unwrap(),
                snapshot,
                "Preparation must not publish edits"
            );
            p.edit(result.edit).unwrap();
            let i = p.tracks[0].instrument.as_ref().unwrap();
            let (order, lfos, macros) = if main {
                (
                    i.main.visible_components(),
                    &i.main.lfos,
                    &i.main.macro_module,
                )
            } else {
                (
                    i.layers[1].visible_components(),
                    &i.layers[1].lfos,
                    &i.layers[1].macro_module,
                )
            };
            assert_eq!(&order[usize::from(!main)..], placed);
            assert_eq!(macros.controls.len(), 2);
            assert!(matches!(placed[0], Envelope(_)));
            assert!(matches!(placed[1], Lfo(_)));
            assert_eq!(placed[2], Macros);
            if duplicate {
                assert_ne!(placed[0], envelope);
                assert_ne!(placed[1], Lfo(lfo_id));
                assert_eq!(lfos.last().unwrap().modulations[0].source, source);
                let mut original: Project = serde_json::from_value(snapshot).unwrap();
                assert_eq!(
                    i.layers[0],
                    original.tracks[0].instrument.take().unwrap().layers[0]
                );
            } else {
                let new = Some(ModulationSourceId {
                    layer: to,
                    unit: match placed[0] {
                        Envelope(id) => id,
                        _ => unreachable!(),
                    },
                });
                assert_eq!(lfos.last().unwrap().modulations[0].source, new);
                assert_eq!(i.layers[0].amplitude.modulations[0].source, new);
                assert!(
                    !i.layers[0]
                        .envelopes
                        .iter()
                        .any(|e| Envelope(e.id) == envelope)
                );
            }
            p.validate().unwrap();
        }
    }
}

#[test]
fn component_group_source_copies_merge_first_and_keep_routes_on_their_targets() {
    let (mut p, t, a, b) = fixture();
    let envelope = Envelope(p.tracks[0].instrument.as_ref().unwrap().layers[0].envelopes[0].id);
    let original = p.tracks[0].instrument.as_ref().unwrap().layers[0].clone();
    for to in [a, b] {
        let mut copy = p.clone();
        let result = copy
            .component_group_transfer(t, a, to, &[envelope, Oscillators], 1, true)
            .unwrap();
        let placed = result.components.clone();
        assert_eq!(placed[0], Oscillators);
        copy.edit(result.edit).unwrap();
        let i = copy.tracks[0].instrument.as_ref().unwrap();
        let layer = i.layers.iter().find(|l| l.id == to).unwrap();
        assert_eq!(layer.visible_components()[0], Oscillators);
        assert_eq!(layer.visible_components()[1], placed[1]);
        assert_ne!(placed[1], envelope);
        let start = if to == a {
            original.oscillators.len()
        } else {
            0
        };
        for (source, original_source) in
            layer.oscillators[start..].iter().zip(&original.oscillators)
        {
            assert!(!original.oscillators.iter().any(|o| o.id == source.id));
            let mut expected = original_source.clone();
            expected.id = source.id;
            assert_eq!(source, &expected);
        }
        let route = layer
            .oscillator_modulations
            .iter()
            .find(|r| r.target == Some(layer.oscillators[start].id))
            .unwrap();
        assert_ne!(route.id, original.oscillator_modulations[0].id);
        assert_eq!(route.source, original.oscillator_modulations[0].source);
        copy.validate().unwrap();
    }
    let result = p
        .component_group_transfer(t, a, b, &[envelope, Oscillators], 1, false)
        .unwrap();
    p.edit(result.edit).unwrap();
    let i = p.tracks[0].instrument.as_ref().unwrap();
    assert!(i.layers[0].oscillators.is_empty());
    assert_eq!(i.layers[1].oscillators, original.oscillators);
    assert_eq!(i.layers[1].visible_components()[0], Oscillators);
}

#[test]
fn envelope_input_routes_follow_copies_and_provider_moves() {
    let mut p = Project::default();
    let track = p.add_midi_track().unwrap();
    let i = p.tracks[0].instrument.as_mut().unwrap();
    i.layers[0].macro_module.add();
    let owner = i.layers[0].id;
    let main = i.main.id;
    let macro_id = i.layers[0].macro_module.controls[0].id;
    let mut envelope = EnvelopeInstance {
        mode: EnvelopeMode::Legato,
        modulation_open: true,
        ..Default::default()
    };
    let mut route = Modulation::new(ModulationParameter::EnvelopeSustain);
    route.source = Some(ModulationSourceId {
        layer: owner,
        unit: macro_id,
    });
    route.amount = 0.5;
    envelope.modulations.push(route);
    let id = envelope.id;
    i.layers[0].envelopes.push(envelope);
    p.edit(Edit::TransferInstrumentComponent {
        track_id: track,
        source: owner,
        destination: main,
        component: Envelope(id),
        destination_index: None,
        duplicate: true,
    })
    .unwrap();
    let copied = &p.tracks[0].instrument.as_ref().unwrap().main.envelopes[0];
    assert_ne!(copied.id, id);
    assert_ne!(copied.modulations[0].id, route.id);
    assert_eq!(copied.modulations[0].source, route.source);
    assert!(copied.modulation_open);
    p.edit(Edit::TransferInstrumentComponent {
        track_id: track,
        source: owner,
        destination: main,
        component: Macros,
        destination_index: None,
        duplicate: false,
    })
    .unwrap();
    let i = p.tracks[0].instrument.as_ref().unwrap();
    for e in i.layers[0].envelopes.iter().chain(&i.main.envelopes) {
        for r in &e.modulations {
            assert_eq!(r.source.unwrap().layer, main);
        }
    }
}

#[test]
fn oscillator_unit_level_follows_moves_and_copies_into_empty_units() {
    for copy in [false, true] {
        let (mut p, track, from, to) = fixture();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.layers[0].oscillator_level_db = -9.0;
        i.layers[1].oscillators.clear();
        p.edit(Edit::TransferInstrumentComponent {
            track_id: track,
            source: from,
            destination: to,
            component: Oscillators,
            duplicate: copy,
            destination_index: None,
        })
        .unwrap();
        let i = p.tracks[0].instrument.as_ref().unwrap();
        assert_eq!(i.layers[1].oscillator_level_db, -9.0);
        if copy {
            assert_eq!(i.layers[0].oscillator_level_db, -9.0);
        }
        p.validate().unwrap();
    }
}
