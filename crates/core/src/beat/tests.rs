use super::*;
use crate::{LfoInstance, Modulation, ModulationParameter, ModulationSourceId};

fn kit() -> BeatInstrument {
    let mut kit = BeatInstrument::default();
    for name in ["Kick", "Snare", "Open hat", "Closed hat"] {
        kit.edit(BeatEdit::Add {
            subtrack: Box::new(BeatSubtrack::new(name.into())),
            index: kit.subtracks.len(),
        })
        .unwrap();
    }
    kit
}

fn source(kit: &BeatInstrument) -> BeatPatternSource {
    let mut pattern = BeatPattern::default();
    for row in &kit.subtracks {
        pattern = pattern.with_step(row.id, 0, Some(100)).unwrap();
    }
    BeatPatternSource::new(kit.id, pattern).unwrap()
}

#[test]
fn defaults_reuse_independent_instruments_and_one_canonical_output() {
    let kit = kit();
    kit.validate().unwrap();
    let owner_ids: HashSet<_> = kit
        .subtracks
        .iter()
        .flat_map(|r| {
            r.instrument
                .layers
                .iter()
                .map(|l| l.id)
                .chain(r.instrument.buses().map(|b| b.id))
        })
        .collect();
    assert_eq!(owner_ids.len(), 16);
    assert!(!owner_ids.contains(&kit.output.id));
    for row in &kit.subtracks {
        assert_eq!(row.trigger_pitch, 60);
        assert_eq!(row.instrument.main.gain_db, 0.0);
        assert_eq!(row.instrument.main.pan, 0.0);
    }
    let pattern = BeatPattern::default();
    assert_eq!(pattern.length_ticks(), 3840);
    assert_eq!(pattern.resolution.step_ticks(), 240);
}

#[test]
fn shrinking_and_coarsening_preserve_step_identity_velocity_and_ticks() {
    let row = Id::new_v4();
    let original = BeatPattern::default()
        .with_grid(16, BeatResolution::ThirtySecond)
        .unwrap()
        .with_step(row, 120, Some(37))
        .unwrap()
        .with_step(row, MAX_TICKS - 120, Some(127))
        .unwrap();
    let small = original.with_grid(1, BeatResolution::Quarter).unwrap();
    assert_eq!(small.visible_steps().count(), 0);
    assert_eq!(small.steps, original.steps);
    let restored = small.with_grid(16, BeatResolution::ThirtySecond).unwrap();
    assert_eq!(restored, original);
    assert_eq!(restored.visible_steps().count(), 2);
}

#[test]
fn source_edits_are_isolated_and_cross_track_transfers_are_rejected() {
    let kit = kit();
    let original = source(&kit);
    let shared = original.clone();
    let replacement = original
        .replaced(
            original
                .pattern
                .with_step(kit.subtracks[0].id, 0, Some(28))
                .unwrap(),
        )
        .unwrap();
    assert_ne!(replacement.id, original.id);
    assert_eq!(shared, original);
    assert_eq!(
        replacement.pattern.steps[0].id,
        original.pattern.steps[0].id
    );
    original.check_destination(kit.id).unwrap();
    assert!(
        original
            .check_destination(BeatInstrument::default().id)
            .is_err()
    );
}

#[test]
fn all_grids_accept_exactly_their_visible_step_positions() {
    let row = Id::new_v4();
    for bars in 1..=16 {
        for resolution in BeatResolution::ALL {
            let grid = BeatPattern::default().with_grid(bars, resolution).unwrap();
            let step = resolution.step_ticks();
            for tick in (0..grid.length_ticks()).step_by(step as usize) {
                let pattern = grid.with_step(row, tick, Some(127)).unwrap();
                assert_eq!(pattern.visible_steps().count(), 1);
            }
            assert!(grid.with_step(row, grid.length_ticks(), Some(100)).is_err());
            assert!(grid.with_step(row, 1, Some(100)).is_err());
        }
    }
}

#[test]
fn malformed_pattern_data_is_rejected_without_changing_the_original() {
    let row = Id::new_v4();
    let pattern = BeatPattern::default().with_step(row, 0, Some(100)).unwrap();
    for velocity in [0, 128, 255] {
        assert!(pattern.with_step(row, 0, Some(velocity)).is_err());
    }
    assert!(pattern.with_grid(0, BeatResolution::Quarter).is_err());
    assert!(pattern.with_grid(17, BeatResolution::Quarter).is_err());
    assert!(pattern.with_step(Id::nil(), 0, Some(100)).is_err());
    let mut bad = pattern.clone();
    bad.steps.push(bad.steps[0].clone());
    assert!(bad.validate().is_err());
    bad.steps[1].id = Id::new_v4();
    assert!(
        bad.validate().is_err(),
        "duplicate row/tick with a fresh ID"
    );
    bad.steps[1].start_tick = MAX_TICKS;
    assert!(bad.validate().is_err());
    bad.steps.pop();
    bad.version += 1;
    assert!(bad.validate().unwrap_err().0.contains("version"));
    assert_eq!(pattern.steps.len(), 1);
    assert!(pattern.with_step(row, 0, None).unwrap().steps.is_empty());
}

#[test]
fn group_reorder_preserves_internal_order_instruments_steps_and_stop_links() {
    let mut kit = kit();
    let [a, b, c, d]: [Id; 4] = kit
        .subtracks
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    kit.edit(BeatEdit::SetStops {
        id: d,
        targets: vec![c],
    })
    .unwrap();
    let original = kit.clone();
    let source = source(&kit);
    let events = kit.triggers(&source).unwrap();
    // Input selection order must not reverse the displayed group.
    kit.edit(BeatEdit::Reorder {
        ids: vec![c, a],
        index: 1,
    })
    .unwrap();
    assert_eq!(
        kit.subtracks.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![b, a, c, d]
    );
    assert_eq!(kit.triggers(&source).unwrap(), events);
    for row in &kit.subtracks {
        assert_eq!(
            row,
            original.subtracks.iter().find(|s| s.id == row.id).unwrap()
        );
    }
    assert_eq!(kit.output, original.output);
}

#[test]
fn failed_batches_roll_back_and_final_state_validation_allows_link_reversal() {
    let mut kit = kit();
    let (a, b) = (kit.subtracks[0].id, kit.subtracks[1].id);
    kit.edit(BeatEdit::SetStops {
        id: a,
        targets: vec![b],
    })
    .unwrap();
    let before = kit.clone();
    assert!(
        kit.edit(BeatEdit::Batch(vec![
            BeatEdit::Rename {
                id: a,
                name: "Changed".into()
            },
            BeatEdit::SetStops {
                id: b,
                targets: vec![a]
            }
        ]))
        .is_err()
    );
    assert_eq!(kit, before);
    kit.edit(BeatEdit::Batch(vec![
        BeatEdit::SetStops {
            id: b,
            targets: vec![a],
        },
        BeatEdit::SetStops {
            id: a,
            targets: vec![],
        },
    ]))
    .unwrap();
    assert_eq!(kit.subtracks[1].stops, vec![a]);
    let before = kit.clone();
    assert!(
        kit.edit(BeatEdit::Reorder {
            ids: vec![a],
            index: usize::MAX
        })
        .is_err()
    );
    assert_eq!(kit, before);
}

#[test]
fn deleting_subtracks_removes_links_but_retains_dormant_immutable_steps() {
    let mut kit = kit();
    let source = source(&kit);
    let (open, closed) = (kit.subtracks[2].id, kit.subtracks[3].id);
    kit.edit(BeatEdit::SetStops {
        id: closed,
        targets: vec![open],
    })
    .unwrap();
    kit.edit(BeatEdit::Remove { ids: vec![open] }).unwrap();
    assert!(kit.subtracks[2].stops.is_empty());
    assert_eq!(source.pattern.steps.len(), 4);
    assert_eq!(kit.triggers(&source).unwrap().len(), 3);
    let ids = kit.subtracks.iter().map(|s| s.id).collect();
    kit.edit(BeatEdit::Remove { ids }).unwrap();
    assert!(kit.triggers(&source).unwrap().is_empty());
    kit.edit(BeatEdit::Add {
        subtrack: Box::new(BeatSubtrack::new("New".into())),
        index: 0,
    })
    .unwrap();
    assert!(
        kit.triggers(&source).unwrap().is_empty(),
        "new row cannot inherit old row's hits"
    );
}

#[test]
fn output_cannot_be_selected_as_a_subtrack_or_replaced_with_a_new_identity() {
    let mut kit = kit();
    let before = kit.clone();
    assert!(
        kit.edit(BeatEdit::Remove {
            ids: vec![kit.output.id]
        })
        .is_err()
    );
    assert!(
        kit.edit(BeatEdit::Reorder {
            ids: vec![kit.output.id],
            index: 0
        })
        .is_err()
    );
    assert!(kit.edit(BeatEdit::SetOutput(Box::default())).is_err());
    assert_eq!(kit, before);
}

#[test]
fn mix_edits_bind_to_the_named_output_and_reject_invalid_values_atomically() {
    let mut kit = kit();
    let id = kit.subtracks[0].id;
    kit.edit(BeatEdit::SetMix {
        id,
        gain_db: -6.0,
        pan: 0.25,
        muted: true,
        soloed: true,
    })
    .unwrap();
    assert_eq!(kit.subtracks[0].instrument.main.gain_db, -6.0);
    assert_eq!(kit.subtracks[0].instrument.main.pan, 0.25);
    let before = kit.clone();
    assert!(
        kit.edit(BeatEdit::SetMix {
            id,
            gain_db: f32::NAN,
            pan: 0.0,
            muted: false,
            soloed: false
        })
        .is_err()
    );
    assert!(
        kit.edit(BeatEdit::SetMix {
            id,
            gain_db: 0.0,
            pan: 1.1,
            muted: false,
            soloed: false
        })
        .is_err()
    );
    assert_eq!(kit, before);
}

#[test]
fn closed_hat_wins_simultaneous_open_hat_and_mute_solo_precede_stops() {
    let mut kit = kit();
    let source = source(&kit);
    let (open, closed) = (kit.subtracks[2].id, kit.subtracks[3].id);
    kit.edit(BeatEdit::SetStops {
        id: closed,
        targets: vec![open],
    })
    .unwrap();
    let hits = kit.triggers(&source).unwrap();
    assert!(
        hits.iter()
            .find(|h| h.subtrack_id == open)
            .unwrap()
            .suppressed
    );
    assert!(
        !hits
            .iter()
            .find(|h| h.subtrack_id == closed)
            .unwrap()
            .suppressed
    );
    assert_eq!(hits.iter().filter(|h| h.suppressed).count(), 1);
    kit.edit(BeatEdit::SetMix {
        id: closed,
        gain_db: 0.0,
        pan: 0.0,
        muted: true,
        soloed: false,
    })
    .unwrap();
    assert!(kit.triggers(&source).unwrap().iter().all(|h| !h.suppressed));
    kit.edit(BeatEdit::SetMix {
        id: open,
        gain_db: 0.0,
        pan: 0.0,
        muted: false,
        soloed: true,
    })
    .unwrap();
    let hits = kit.triggers(&source).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].subtrack_id, open);
    assert_eq!(hits[0].length_ticks, 240);
    assert_eq!(hits[0].pitch, 60);
}

#[test]
fn stop_links_reject_missing_self_duplicate_and_cyclic_targets() {
    let mut kit = kit();
    let (a, b) = (kit.subtracks[0].id, kit.subtracks[1].id);
    let before = kit.clone();
    for targets in [vec![a], vec![Id::new_v4()], vec![b, b]] {
        assert!(kit.edit(BeatEdit::SetStops { id: a, targets }).is_err());
        assert_eq!(kit, before);
    }
    kit.edit(BeatEdit::SetStops {
        id: a,
        targets: vec![b],
    })
    .unwrap();
    assert!(
        kit.edit(BeatEdit::SetStops {
            id: b,
            targets: vec![a]
        })
        .is_err()
    );
}

#[test]
fn sibling_provider_names_do_not_grant_cross_instrument_scope() {
    let mut kit = kit();
    let own = LfoInstance::default();
    let sibling = LfoInstance::default();
    kit.subtracks[0].instrument.lfo_lane.lfos.push(own.clone());
    kit.subtracks[1]
        .instrument
        .lfo_lane
        .lfos
        .push(sibling.clone());
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(ModulationSourceId {
        layer: kit.subtracks[0].instrument.lfo_lane.id,
        unit: own.id,
    });
    kit.subtracks[0].instrument.layers[0]
        .amplitude
        .modulations
        .push(route);
    kit.validate().unwrap();
    kit.subtracks[0].instrument.layers[0].amplitude.modulations[0].source =
        Some(ModulationSourceId {
            layer: kit.subtracks[1].instrument.lfo_lane.id,
            unit: sibling.id,
        });
    assert!(kit.validate().is_err());
}

#[test]
fn versioned_data_round_trips_with_hidden_hits_and_rejects_unknown_revisions() {
    let kit = kit();
    let source = source(&kit);
    let decoded: BeatInstrument =
        serde_json::from_slice(&serde_json::to_vec(&kit).unwrap()).unwrap();
    assert_eq!(decoded, kit);
    decoded.validate().unwrap();
    let decoded: BeatPatternSource =
        serde_json::from_slice(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert_eq!(decoded, source);
    decoded.validate().unwrap();
    let mut future = kit.clone();
    future.version += 1;
    assert!(future.validate().unwrap_err().0.contains("version"));
    assert!(kit.heap_bytes() > 0);
    assert!(source.pattern.heap_bytes() >= 4 * std::mem::size_of::<BeatStep>() as u64);
}

#[test]
fn appending_and_readding_subtracks_uses_fresh_identity_and_available_names() {
    let mut kit = BeatInstrument::default();
    let first = kit.add_subtrack().unwrap();
    kit.add_subtrack().unwrap();
    assert_eq!(kit.subtracks[0].name, "Subtrack 1");
    assert_eq!(kit.subtracks[1].name, "Subtrack 2");
    kit.edit(BeatEdit::Remove { ids: vec![first] }).unwrap();
    let replacement = kit.add_subtrack().unwrap();
    assert_ne!(first, replacement);
    assert_eq!(kit.subtracks[1].name, "Subtrack 1");
}

#[test]
fn group_owner_identity_collisions_are_rejected_even_without_installed_providers() {
    let mut invalid = kit();
    invalid.subtracks[1].instrument.lfo_lane.id = invalid.subtracks[0].instrument.lfo_lane.id;
    assert!(invalid.validate().is_err());
    let mut invalid = kit();
    invalid.subtracks[0].instrument.main.id = invalid.output.id;
    assert!(invalid.validate().is_err());
}

#[test]
fn stop_chains_are_order_independent_and_suppressed_stoppers_still_issue_stops() {
    let mut kit = kit();
    let (a, b, c) = (
        kit.subtracks[0].id,
        kit.subtracks[1].id,
        kit.subtracks[2].id,
    );
    kit.edit(BeatEdit::SetStops {
        id: a,
        targets: vec![b],
    })
    .unwrap();
    kit.edit(BeatEdit::SetStops {
        id: b,
        targets: vec![c],
    })
    .unwrap();
    let source = source(&kit);
    let hits = kit.triggers(&source).unwrap();
    for id in [b, c] {
        assert!(
            hits.iter()
                .find(|h| h.subtrack_id == id)
                .unwrap()
                .suppressed
        );
    }
    kit.subtracks.reverse();
    assert_eq!(hits, kit.triggers(&source).unwrap());
}

#[test]
fn sampler_identities_cannot_share_prepared_banks_across_siblings() {
    let mut kit = kit();
    let sample = crate::SamplerSettings::default();
    kit.subtracks[0].instrument.layers[0].sampler = Some(sample.clone());
    kit.subtracks[1].instrument.layers[0].sampler = Some(sample);
    assert!(kit.validate().is_err());
    kit.subtracks[1].instrument.layers[0]
        .sampler
        .as_mut()
        .unwrap()
        .id = Id::new_v4();
    kit.validate().unwrap();
}

#[test]
fn final_output_reuses_macro_scope_checks_and_dependency_order() {
    let mut kit = kit();
    kit.output.macro_module.add();
    let control = kit.output.macro_module.controls[0].id;
    let mut route = Modulation::new(ModulationParameter::MacroValue);
    route.target = Some(control);
    route.source = Some(ModulationSourceId {
        layer: kit.output.id,
        unit: control,
    });
    kit.output.macro_module.modulations.push(route);
    assert!(kit.validate().unwrap_err().0.contains("cycle"));
    kit.output.macro_module.modulations[0].source = Some(ModulationSourceId {
        layer: kit.subtracks[0].instrument.main.id,
        unit: Id::new_v4(),
    });
    assert!(kit.validate().unwrap_err().0.contains("subtrack"));
    kit.output.macro_module.modulations[0].source = None;
    kit.validate().unwrap();
    kit.output.lfos.push(LfoInstance::default());
    assert!(kit.validate().is_err(), "no kit-wide LFO lane");
}
