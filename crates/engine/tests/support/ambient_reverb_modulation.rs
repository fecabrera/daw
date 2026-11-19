use super::*;
#[test]
fn ambient_reverb_shared_modulation_reaches_track_layer_and_main_and_roundtrips() {
    for rate in SAMPLE_RATES {
        for owner in ["track", "layer", "main"] {
            effect_modulation_parity::check(
                rate,
                owner,
                EffectInstance::ambient_reverb(),
                true,
                |unit, source| {
                    let EffectSettings::AmbientReverb(settings) = unit else {
                        panic!()
                    };
                    settings.lowpass_hz = 5000.0;
                    let bases = settings.values();
                    settings.modulations = ModulationParameter::AMBIENT_REVERB
                        .into_iter()
                        .map(|parameter| (parameter, parameter.limit() / 100.0))
                        .map(|(parameter, amount)| {
                            let mut r = Modulation::new(parameter);
                            r.source = Some(source);
                            r.amount = amount;
                            r
                        })
                        .collect();
                    bases
                },
                |bases, values| {
                    for (actual, expected) in values.iter().zip([
                        (bases[0]
                            * 2.0_f32.powf(ModulationParameter::AmbientDecay.limit() / 100.0))
                        .clamp(1.0, 60.0),
                        (bases[1] + ModulationParameter::AmbientBloom.limit() / 100.0)
                            .clamp(0.0, 1.0),
                        (bases[2] + ModulationParameter::AmbientTone.limit() / 100.0)
                            .clamp(0.0, 1.0),
                        (bases[3] + ModulationParameter::AmbientMotion.limit() / 100.0)
                            .clamp(0.0, 1.0),
                        (bases[4] + ModulationParameter::ReverbWidth.limit() / 100.0)
                            .clamp(0.0, 1.0),
                        (bases[5] + ModulationParameter::Level.limit() / 100.0)
                            .clamp(-1000.0, 12.0),
                        (bases[6] + ModulationParameter::DryWet.limit() / 100.0).clamp(0.0, 1.0),
                        (bases[7]
                            * 2.0_f32.powf(ModulationParameter::PlateLowPass.limit() / 100.0))
                        .clamp(500.0, 15000.0),
                    ]) {
                        assert!(
                            (actual - expected).abs() < 0.001,
                            "{owner}: {actual} != {expected}"
                        );
                    }
                },
            );
        }
    }
}
