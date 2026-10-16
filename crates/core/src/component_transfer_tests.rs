use crate::*;
use InstrumentComponent::*;

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
