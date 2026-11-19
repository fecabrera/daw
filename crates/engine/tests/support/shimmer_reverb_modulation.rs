use super::*;
#[test]
fn shimmer_reverb_shared_destinations_reach_track_layer_and_main_without_allocating() {
    for rate in SAMPLE_RATES {
        for owner in ["track", "layer", "main"] {
            effect_modulation_parity::check(
                rate,
                owner,
                EffectInstance::shimmer_reverb(),
                false,
                |unit, source| {
                    let EffectSettings::ShimmerReverb(s) = unit else {
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
                    bases
                },
                |bases, values| {
                    for (i, parameter) in
                        ModulationParameter::SHIMMER_REVERB.into_iter().enumerate()
                    {
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
                            (values[i] - value).abs() < 0.002,
                            "{owner}, {parameter:?}, {} != {value}",
                            values[i]
                        );
                    }
                },
            );
        }
    }
}
