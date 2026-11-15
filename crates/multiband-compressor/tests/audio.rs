use daw_multiband_compressor::MultibandCompressor;
use daw_unit_core::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn tone(frame: u32, rate: u32, hz: f32) -> f32 {
    (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.25
}
fn power(rate: u32, hz: f32, s: &MultibandCompressorSettings, solo: Option<usize>) -> f64 {
    let mut p = MultibandCompressor::new(context(rate), s);
    if let Some(i) = solo {
        p.set_band_solo(s.bands[i].id, Arc::new(AtomicBool::new(true)));
    }
    let mut energy = [0.0; 2];
    for i in 0..rate / 5 {
        let x = tone(i, rate, hz);
        let y = p.process_sample([x, -x]);
        assert!((y[0] + y[1]).abs() < 1e-6);
        assert!(y.iter().all(|v| v.is_finite()));
        if i > rate / 10 {
            energy[0] += f64::from(x).powi(2);
            energy[1] += f64::from(y[0]).powi(2);
        }
    }
    10.0 * (energy[1] / energy[0]).log10()
}
#[test]
fn crossovers_recombine_flat_and_solo_isolates_adjacent_ranges_at_six_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut s = MultibandCompressorSettings::default();
        for b in &mut s.bands {
            b.ratio = 1.0;
        }
        for hz in [40.0, 200.0, 600.0, 2000.0, 10000.0] {
            assert!(power(rate, hz, &s, None).abs() < 0.03, "{rate} {hz}");
        }
        assert!(power(rate, 40.0, &s, Some(0)).abs() < 0.1);
        assert!(power(rate, 10000.0, &s, Some(0)) < -65.0);
        assert!(power(rate, 40.0, &s, Some(2)) < -65.0);
        assert!(power(rate, 10000.0, &s, Some(2)).abs() < 0.1);
    }
}
#[test]
fn independent_compression_power_and_audition_ignore_full_range_dry() {
    let mut s = MultibandCompressorSettings::default();
    for b in &mut s.bands {
        b.ratio = 1.0;
    }
    s.bands[1].ratio = 20.0;
    s.bands[1].threshold_db = -40.0;
    s.bands[1].attack_ms = 0.1;
    assert!(power(48000, 600.0, &s, None) < -18.0);
    assert!(power(48000, 40.0, &s, None).abs() < 0.1);
    s.bands[1].enabled = false;
    assert!(power(48000, 600.0, &s, None).abs() < 0.03);
    s.dry_wet = 0.0;
    assert!(power(48000, 10000.0, &s, Some(0)) < -65.0);
}
#[test]
fn solo_is_live_only_clone_detaches_bypass_clears_and_multiple_solos_recombine() {
    let mut s = MultibandCompressorSettings::default();
    for b in &mut s.bands {
        b.ratio = 1.0;
    }
    let mut live = MultibandCompressor::new(context(48000), &s);
    let mut normal = live.clone();
    let flags: Vec<_> = s
        .bands
        .iter()
        .map(|b| {
            let f = Arc::new(AtomicBool::new(true));
            live.set_band_solo(b.id, f.clone());
            f
        })
        .collect();
    for i in 0..4096 {
        let input = [tone(i, 48000, 700.0); 2];
        assert_eq!(live.process_sample(input), normal.process_sample(input));
    }
    flags[1].store(false, Ordering::Relaxed);
    flags[2].store(false, Ordering::Relaxed);
    for i in 0..4096 {
        let input = [tone(i, 48000, 700.0); 2];
        live.process_sample(input);
        normal.process_sample(input);
    }
    let mut detached = live.clone();
    for i in 0..1024 {
        let input = [tone(i, 48000, 700.0); 2];
        assert_eq!(detached.process_sample(input), normal.process_sample(input));
    }
    s.enabled = false;
    assert!(live.retarget(&s));
    assert!(flags.iter().all(|f| !f.load(Ordering::Relaxed)));
    for _ in 0..240 {
        live.process_sample([0.1, -0.1]);
    }
    assert_eq!(live.process_sample([0.1, -0.1]), [0.1, -0.1]);
}

#[test]
fn curve_histories_capture_each_split_input_and_clear_on_band_bypass_without_changing_audio() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut s = MultibandCompressorSettings::default();
        let mut p = MultibandCompressor::new(context(rate), &s);
        let mut reference = p.clone();
        let meters: Vec<_> = s
            .bands
            .iter()
            .map(|_| Arc::new(BandMeter::with_curve_history()))
            .collect();
        for (band, meter) in s.bands.iter().zip(&meters) {
            p.set_band_level_meter(band.id, meter.clone());
        }
        for frame in 0..rate / 5 {
            let x = tone(frame, rate, 60.0);
            assert_eq!(p.process_sample([x, -x]), reference.process_sample([x, -x]));
        }
        let histories: Vec<_> = meters
            .iter()
            .map(|m| m.curve().unwrap().history().unwrap().reading().unwrap())
            .collect();
        assert!(histories.iter().all(|h| h.count == 10));
        assert!(histories[0].bins[9][0] > histories[2].bins[9][0] * 10.0);
        assert!(meters[0].curve().unwrap().readings()[0] > 0.01);
        let reduction = meters[0].curve().unwrap().curve_reduction().unwrap();
        assert!(
            reduction.reading() > 1.0,
            "GR must be compression dB, not linear output amplitude"
        );
        let before_reduction = reduction.reading();
        let mut prepared = MultibandCompressor::new(context(rate), &s);
        prepared.set_band_level_meter(s.bands[0].id, meters[0].clone());
        assert_eq!(
            reduction.reading(),
            before_reduction,
            "worker binding must not clear live readings"
        );
        let before = histories[0];
        s.bands[0].threshold_db = -30.0;
        assert!(p.retarget(&s));
        assert_eq!(
            meters[0]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading()
                .unwrap(),
            before
        );
        s.bands[0].enabled = false;
        assert!(p.retarget(&s));
        assert_eq!(meters[0].curve().unwrap().readings()[0], 0.0);
        assert_eq!(reduction.reading(), 0.0);
        assert_eq!(
            meters[0]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading()
                .unwrap()
                .count,
            0
        );
        assert_eq!(
            meters[1]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading()
                .unwrap(),
            histories[1]
        );
    }
}
