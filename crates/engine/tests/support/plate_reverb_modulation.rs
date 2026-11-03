use super::*;
#[test]
fn plate_reverb_shared_modulation_reaches_track_layer_and_main_and_roundtrips() {
    for rate in SAMPLE_RATES {
        for owner in ["track", "layer", "main"] {
            let mut p = plan(Waveform::Saw, rate);
            let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
            instrument.main.macro_module.add();
            instrument.main.macro_module.controls[0].settings.value = 1.0;
            let source = ModulationSourceId {
                layer: instrument.main.id,
                unit: instrument.main.macro_module.controls[0].id,
            };
            let mut effect = EffectInstance::plate_reverb();
            let target = daw_engine::ModulationTarget::Effect(effect.id);
            let EffectSettings::PlateReverb(settings) = &mut effect.unit else {
                panic!()
            };
            let bases = settings.values();
            settings.modulations = ModulationParameter::PLATE_REVERB
                .into_iter()
                .map(|parameter| (parameter, parameter.limit() / 100.0))
                .map(|(parameter, amount)| {
                    let mut r = Modulation::new(parameter);
                    r.source = Some(source);
                    r.amount = amount;
                    r
                })
                .collect();
            match owner {
                "layer" => instrument.layers[0].effects.push(effect),
                "main" => instrument.main.effects.push(effect),
                _ => p.project.tracks[0].effects.push(effect),
            }
            p.project.validate().unwrap();
            let reopened: Project =
                serde_json::from_value(serde_json::to_value(&p.project).unwrap()).unwrap();
            assert_eq!(reopened, p.project);
            let mut expected = vec![[0.0; 2]; (rate / 10) as usize];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p);
            live.play();
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame > rate as usize / 200 {
                    assert!((0..2).all(|ch| (actual[ch] - sample[ch]).abs() < 2e-6));
                }
            }
            let values = live.modulation_snapshot(target).unwrap();
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            for (actual, expected) in values.iter().zip([
                (bases[0] + ModulationParameter::PlatePreDelay.limit() / 100.0).clamp(0.0, 300.0),
                (bases[1] * 2.0_f32.powf(ModulationParameter::PlateDecay.limit() / 100.0))
                    .clamp(0.3, 15.0),
                (bases[2] * 2.0_f32.powf(ModulationParameter::PlateHighPass.limit() / 100.0))
                    .clamp(20.0, 1000.0),
                (bases[3] * 2.0_f32.powf(ModulationParameter::PlateLowPass.limit() / 100.0))
                    .clamp(500.0, 15000.0),
                (bases[4] + ModulationParameter::PlateDrive.limit() / 100.0).clamp(0.0, 12.0),
                (bases[5] + ModulationParameter::ReverbWidth.limit() / 100.0).clamp(0.0, 1.0),
                (bases[6] + ModulationParameter::Level.limit() / 100.0).clamp(-1000.0, 12.0),
                (bases[7] + ModulationParameter::DryWet.limit() / 100.0).clamp(0.0, 1.0),
                (bases[8] + ModulationParameter::Tone.limit() / 100.0).clamp(0.0, 1.0),
            ]) {
                assert!(
                    (actual - expected).abs() < 0.001,
                    "{owner}: {actual} != {expected}"
                );
            }
        }
    }
}
