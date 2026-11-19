use super::*;
#[test]
fn plate_reverb_track_master_export_seek_and_compatible_edits_share_history_without_allocating() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let p = plan(rate, false);
            let effect = EffectInstance::plate_reverb();
            effect_history::check(
                rate,
                master,
                p,
                effect,
                |unit| {
                    let EffectSettings::PlateReverb(settings) = unit else {
                        panic!()
                    };
                    settings.collapsed = true;
                },
                0.0,
            );
        }
    }
}
