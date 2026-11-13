use daw_multiband_compressor::MultibandCompressor;
use daw_unit_core::{EffectUnit, MultibandCompressorSettings, ProcessingContext, SpectrumMeter};
use std::sync::Arc;
#[test]
fn prepared_replacements_keep_live_spectrum_and_compressor_history_at_every_rate() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let c = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut s = MultibandCompressorSettings::default();
        let meter = Arc::new(SpectrumMeter::default());
        let mut live = MultibandCompressor::new(c, &s);
        live.set_spectrum_meter(meter.clone());
        for i in 0..rate / 4 {
            live.process_sample([(i as f32 * 0.02).sin() * 0.5; 2]);
        }
        let warm = meter.reading();
        assert_eq!(warm.valid_branches, 1);
        let mut reference = live.clone();
        s.bands[0].threshold_db = -30.0;
        let mut prepared = MultibandCompressor::new(c, &s);
        prepared.set_spectrum_meter(meter.clone());
        assert_eq!(
            meter.reading(),
            warm,
            "preparation must not publish or clear live data"
        );
        prepared.inherit(&mut live);
        assert_eq!(meter.reading(), warm);
        assert!(reference.retarget(&s));
        for i in 0..rate / 4 {
            let x = (i as f32 * 0.02).sin() * 0.5;
            assert_eq!(
                prepared.process_sample([x, x]),
                reference.process_sample([x, x])
            );
        }
        assert_eq!(meter.reading().generation, warm.generation);
        prepared.retarget_output(false, 0.0, 1.0);
        assert_eq!(meter.reading().valid_branches, 0);
        prepared.retarget_output(true, 0.0, 1.0);
        for _ in 0..rate / 4 {
            prepared.process_sample([0.1, -0.1]);
        }
        assert_eq!(meter.reading().valid_branches, 1);
        prepared.reset();
        assert_eq!(meter.reading().valid_branches, 0);
    }
}

#[test]
fn band_level_rails_use_shared_release_without_reholding_display_decay() {
    use daw_unit_core::{BandMeter, LEVEL_METER_RELEASE_SECONDS};
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let c = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut settings = MultibandCompressorSettings::default();
        settings.bands.truncate(1);
        settings.bands[0].ratio = 1.0;
        let id = settings.bands[0].id;
        let meter = Arc::new(BandMeter::<3>::default());
        let mut actual = MultibandCompressor::new(c, &settings);
        let mut reference = MultibandCompressor::new(c, &settings);
        actual.set_band_level_meter(id, meter.clone());
        assert_eq!(
            actual.process_sample([0.5, -0.25]),
            reference.process_sample([0.5, -0.25])
        );
        assert_eq!(meter.readings(), [0.5, 0.5, 0.25]);
        for _ in 0..(rate as f32 * LEVEL_METER_RELEASE_SECONDS).round() as u32 {
            assert_eq!(
                actual.process_sample([0.0; 2]),
                reference.process_sample([0.0; 2])
            );
        }
        let expected = 0.5 * (-1.0_f32).exp();
        assert!(
            (meter.readings()[0] - expected).abs() < 0.001,
            "{rate} release {:?}",
            meter.readings()
        );
        assert_eq!(meter.boost_peaks()[0].peak(), 0.5);
        meter.boost_peaks()[0].reset_peak();
        actual.process_sample([0.0; 2]);
        assert_eq!(
            meter.boost_peaks()[0].peak(),
            0.0,
            "display release cannot relatch held peaks"
        );
        let warm = meter.readings()[0];
        let mut next = MultibandCompressor::new(c, &settings);
        next.set_band_level_meter(id, meter.clone());
        assert_eq!(
            meter.readings()[0],
            warm,
            "worker preparation must not blank the live band rail"
        );
        next.inherit(&mut actual);
        next.process_sample([0.0; 2]);
        assert!(
            meter.readings()[0] > warm * 0.99 && meter.readings()[0] < warm,
            "inherit release history"
        );
        for _ in 0..8 {
            next.process_sample([0.5, -0.25]);
            for _ in 0..512 {
                next.process_sample([0.0; 2]);
            }
            assert!(
                meter.readings()[0] > 0.47,
                "alternating buffers must not blank the rail: {rate}"
            );
        }
        next.reset();
        assert_eq!(meter.readings(), [0.0; 3]);
        settings.bands[0].enabled = false;
        assert!(next.retarget(&settings));
        next.process_sample([0.5, -0.25]);
        assert_eq!(
            meter.readings()[0],
            0.5,
            "compression bypass retains the audible band's level"
        );
        next.retarget_output(false, 0.0, 1.0);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        next.retarget_output(true, 0.0, 1.0);
        next.process_sample([0.5, -0.25]);
        assert_eq!(meter.readings()[0], 0.5);
        settings.enabled = false;
        let previous = meter.readings();
        let mut disabled = MultibandCompressor::new(c, &settings);
        disabled.set_band_level_meter(id, meter.clone());
        assert_eq!(
            meter.readings(),
            previous,
            "disabled preparation must also preserve live readings"
        );
        disabled.process_sample([0.0; 2]);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
    }
}
