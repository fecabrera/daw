use super::*;
use daw_engine::ModulationTarget;
use daw_unit_core::BandMeter;
use std::sync::Arc;

#[test]
fn harmonic_branch_meters_publish_by_id_without_changing_audio_or_allocating() {
    for rate in SAMPLE_RATES {
        for quality in [
            daw_core::HarmonicQuality::Normal,
            daw_core::HarmonicQuality::Hq,
        ] {
            let mut p = plan(Waveform::Sine, rate);
            let mut effect = daw_core::EffectInstance::harmonic_lift();
            let daw_core::EffectSettings::HarmonicLift(settings) = &mut effect.unit else {
                unreachable!()
            };
            settings.quality = quality;
            for band in &mut settings.bands {
                band.enabled = true;
                band.gain_db = 6.0;
            }
            let keys: [_; 6] = std::array::from_fn(|i| ModulationTarget::EffectBand {
                effect: effect.id,
                band: settings.bands[i].id,
            });
            p.project.tracks[0].effects.push(effect);
            let meters: [_; 6] = std::array::from_fn(|_| Arc::new(BandMeter::<3>::default()));
            let mut live = Renderer::new(p.clone());
            let mut reference = Renderer::new(p.clone());
            for (key, meter) in keys.into_iter().zip(&meters) {
                live.set_target_band_peak_meter(key, meter.clone());
            }
            live.play();
            reference.play();
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|a| a.set(true));
            for _ in 0..2048 {
                assert_eq!(live.next_sample(), reference.next_sample());
            }
            TRACK_ALLOCATIONS.with(|a| a.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            for meter in &meters {
                assert!(meter.boost_peaks()[0].peak() > 0.0);
            }
            let held = meters.each_ref().map(|m| m.boost_peaks()[0].peak());
            let mut snapshot = live.clone();
            for _ in 0..128 {
                snapshot.next_sample();
            }
            assert_eq!(meters.each_ref().map(|m| m.boost_peaks()[0].peak()), held);
            let daw_core::EffectSettings::HarmonicLift(settings) =
                &mut p.project.tracks[0].effects[0].unit
            else {
                unreachable!()
            };
            settings.enabled = false;
            let mut off = Renderer::new(p);
            for (key, meter) in keys.into_iter().zip(&meters) {
                off.set_target_band_peak_meter(key, meter.clone());
            }
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|a| a.set(true));
            off.inherit(&mut live);
            for _ in 0..128 {
                off.next_sample();
            }
            TRACK_ALLOCATIONS.with(|a| a.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            for meter in &meters {
                assert_eq!(meter.readings(), [0.0; 3]);
                assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
            }
        }
    }
}
