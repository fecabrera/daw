use super::*;

#[test]
fn migrated_instrument_lanes_preserve_per_note_and_shared_audio_at_all_rates_without_allocations() {
    for rate in SAMPLE_RATES {
        for mode in [
            EnvelopeMode::PerNote,
            EnvelopeMode::Retrigger,
            EnvelopeMode::Legato,
        ] {
            let mut original = plan(Waveform::Sine, rate);
            let i = original.project.tracks[0].instrument.as_mut().unwrap();
            i.layers[0].envelopes[0].mode = mode;
            let mut lfo = daw_unit_core::LfoInstance::default();
            lfo.settings.restart = match mode {
                EnvelopeMode::PerNote => daw_unit_core::LfoRestart::Transport,
                EnvelopeMode::Retrigger => daw_unit_core::LfoRestart::Retrigger,
                EnvelopeMode::Legato => daw_unit_core::LfoRestart::Legato,
            };
            let source = ModulationSourceId {
                layer: i.layers[0].id,
                unit: lfo.id,
            };
            i.layers[0].lfos.push(lfo);
            i.layers[0].oscillator_modulations.push(Modulation {
                source: Some(source),
                amount: -3.0,
                ..Modulation::new(ModulationParameter::Level)
            });
            i.layers[0].envelopes[0].modulations.push(Modulation {
                source: Some(source),
                amount: 0.5,
                ..Modulation::new(ModulationParameter::EnvelopeAttack)
            });
            original.project.validate().unwrap();
            let mut migrated = original.clone();
            let mut saved = serde_json::to_value(&original.project).unwrap();
            let state = saved["tracks"][0]["instrument"].as_object_mut().unwrap();
            state.insert("version".into(), 3.into());
            state.remove("lfo_lane");
            state.remove("envelope_lane");
            migrated.project = serde_json::from_value(saved).unwrap();
            migrated.project.validate().unwrap();
            let i = migrated.project.tracks[0].instrument.as_ref().unwrap();
            assert!(i.layers[0].envelopes.is_empty() && i.layers[0].lfos.is_empty());
            assert_eq!(i.envelope_lane.envelopes[0].mode, mode);
            let mut a = Renderer::new(original);
            let mut b = Renderer::new(migrated);
            a.play();
            b.play();
            let mut peak = 0.0_f32;
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|on| on.set(true));
            let mut tail = [0.0; 2];
            for _ in 0..rate * 6 / 5 {
                let expected = a.next_sample();
                let actual = b.next_sample();
                assert_eq!(actual, expected, "rate {rate}, mode {mode:?}");
                peak = peak.max(actual[0].abs());
                tail = actual;
            }
            TRACK_ALLOCATIONS.with(|on| on.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            assert!(peak > 0.001);
            assert_eq!(
                tail, [0.0; 2],
                "global providers must not prolong released notes"
            );
        }
    }
}
