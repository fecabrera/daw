use super::*;
#[test]
fn shimmer_reverb_shared_destinations_reach_track_layer_and_main_without_allocating() {
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
            let mut effect = EffectInstance::shimmer_reverb();
            let target = daw_engine::ModulationTarget::Effect(effect.id);
            let EffectSettings::ShimmerReverb(s) = &mut effect.unit else {
                panic!()
            };
            let bases = s.values();
            s.modulations = ModulationParameter::SHIMMER_REVERB
                .into_iter()
                .map(|parameter| {
                    let mut route = Modulation::new(parameter);
                    route.source = Some(source);
                    route.amount = parameter.limit() / 100.0;
                    route
                })
                .collect();
            match owner {
                "layer" => instrument.layers[0].effects.push(effect),
                "main" => instrument.main.effects.push(effect),
                _ => p.project.tracks[0].effects.push(effect),
            }
            p.project.validate().unwrap();
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
            let snapshot = live.modulation_snapshot(target).unwrap();
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            for (i, parameter) in ModulationParameter::SHIMMER_REVERB.into_iter().enumerate() {
                let value = (bases[i] + parameter.limit() / 100.0).clamp(
                    0.0,
                    if i == 1 {
                        1000.0
                    } else if i == 10 {
                        12.0
                    } else {
                        1.0
                    },
                );
                assert!(
                    (snapshot[i] - value).abs() < 0.002,
                    "{owner}, {parameter:?}, {} != {value}",
                    snapshot[i]
                );
            }
        }
    }
}
