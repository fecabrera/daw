use super::*;
use daw_unit_core::{MeterStorage, spectrum_frequency};
#[test]
fn preparing_repeated_updates_never_clears_the_visible_spectrum() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let meter = Arc::new(SpectrumMeter::default());
        let mut current = SpectrumAnalyzer::new(rate, true);
        current.attach(meter.clone());
        for i in 0..rate as usize * 2 {
            let v = (std::f32::consts::TAU * 1000.0 * i as f32 / rate as f32).sin() * 0.5;
            current.sample([v, v]);
        }
        let before = meter.reading();
        assert!(before.magnitudes.iter().any(|v| *v > 0.3));
        for _ in 0..20 {
            let mut prepared = SpectrumAnalyzer::new(rate, true);
            prepared.attach(meter.clone());
            assert_eq!(meter.reading().magnitudes, before.magnitudes);
            current.retarget(&mut prepared);
            assert_eq!(meter.reading().magnitudes, before.magnitudes);
        }
        current.set_enabled(false);
        assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
    }
}
#[test]
fn spectrum_tracks_tones_at_every_rate_without_stereo_cancellation_and_clears_bypass() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for hz in [80.0, 1000.0, 12000.0] {
            let mut spectrum = SpectrumAnalyzer::new(rate, true);
            let meter = Arc::new(SpectrumMeter::default());
            spectrum.attach(meter.clone());
            for i in 0..rate as usize * 2 {
                let v = 0.5 * (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin();
                spectrum.sample([v, -v]);
            }
            let reading = meter.reading();
            let peak = reading
                .magnitudes
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap();
            assert!(
                (spectrum_frequency(peak.0) / hz).log2().abs() < 0.25,
                "{rate} {hz} {}",
                spectrum_frequency(peak.0)
            );
            assert!(*peak.1 > 0.3 && *peak.1 < 0.55, "{rate} {hz} {}", peak.1);
            assert_eq!(reading.sample_rate_hz, rate);
            let clone = spectrum.clone();
            assert!(!clone.meter.is_connected());
            spectrum.set_enabled(false);
            assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
            for _ in 0..100 {
                spectrum.sample([1.0; 2]);
            }
            assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
            spectrum.set_enabled(true);
            spectrum.reset();
            assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
            meter.clear();
        }
    }
}

const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn peak(reading: &SpectrumReading, low: f32, high: f32) -> f32 {
    reading
        .magnitudes
        .iter()
        .enumerate()
        .filter(|(i, _)| (low..=high).contains(&spectrum_frequency(*i)))
        .map(|(_, v)| *v)
        .fold(0.0, f32::max)
}
fn signal(rate: u32, tones: &[(f64, f64)], seconds: usize) -> SpectrumReading {
    let mut analyzer = SpectrumAnalyzer::new(rate, true);
    let meter = Arc::new(SpectrumMeter::default());
    analyzer.attach(meter.clone());
    for i in 0..rate as usize * seconds {
        let phase = std::f64::consts::TAU * i as f64 / f64::from(rate);
        let v = tones
            .iter()
            .map(|(hz, offset)| (phase * hz + offset).sin() * 0.25)
            .sum::<f64>() as f32;
        analyzer.sample([v, -v]);
    }
    assert_eq!(analyzer.overruns, 0);
    assert!(analyzer.max_work <= WORK_PER_FRAME);
    meter.reading()
}
#[test]
fn resolved_pairs_have_measured_valleys_at_all_rates_and_phases() {
    for rate in RATES {
        for (a, b) in [(40.0, 80.0), (80.0, 160.0), (200.0, 250.0)] {
            for offset in [0.0, 1.7] {
                let r = signal(rate, &[(a, 0.0), (b, offset)], 1);
                let a_peak = peak(&r, a as f32 - 2.0, a as f32 + 2.0);
                let b_peak = peak(&r, b as f32 - 2.0, b as f32 + 2.0);
                let valley = r
                    .magnitudes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        (a as f32 + 2.0..b as f32 - 2.0).contains(&spectrum_frequency(*i))
                    })
                    .map(|(_, v)| *v)
                    .fold(f32::MAX, f32::min);
                assert!(
                    a_peak > 0.17 && b_peak > 0.17,
                    "{rate} {a}/{b}: {a_peak}/{b_peak}"
                );
                assert!(
                    valley < a_peak.min(b_peak) * 0.5,
                    "{rate} {a}/{b} valley {valley}"
                );
            }
        }
    }
}
#[test]
fn cold_readiness_scheduler_memory_and_release_meet_the_bounds() {
    for rate in RATES {
        let mut s = SpectrumAnalyzer::new(rate, true);
        assert!(s.heap_bytes() + (std::mem::size_of::<SpectrumAnalyzer>() as u64) < 600 * 1024);
        let meter = Arc::new(SpectrumMeter::default());
        s.attach(meter.clone());
        let mut upper = 0;
        // Irregular block sizes do not change the sample scheduler.
        let mut frame = 0;
        for block in [1, 17, 32, 64, 128, 512, 4096].into_iter().cycle() {
            for _ in 0..block.min(rate as usize * 3 - frame) {
                let v = if frame < rate as usize {
                    (std::f64::consts::TAU * 80.0 * frame as f64 / f64::from(rate)).sin() as f32
                        * 0.5
                } else {
                    0.0
                };
                s.sample([v, v]);
                frame += 1;
                if s.reading.valid_branches & SPECTRUM_UPPER_READY != 0 && upper == 0 {
                    upper = frame;
                }
                if upper != 0 {
                    assert_eq!(s.reading.valid_branches, SPECTRUM_UPPER_READY);
                    assert_eq!(s.reading.bass_end_frame, 0);
                    if frame < rate as usize {
                        assert!(peak(&s.reading, 70.0, 90.0) > 0.3);
                    }
                }
            }
            if frame == rate as usize * 3 {
                break;
            }
        }
        println!("spectrum startup rate={rate} full_range_frame={upper}");
        assert!(
            upper > 0 && upper as f64 / f64::from(rate) <= 0.125,
            "{rate} upper {upper}"
        );
        assert!(peak(&s.reading, 70.0, 90.0) < 0.5 * 10.0_f32.powf(-26.0 / 20.0));
        assert_eq!(s.overruns, 0);
        assert!(s.max_work <= 80);
        assert!(s.reading.bass_end_frame <= s.reading.upper_end_frame);
        let generation = meter.generation();
        s.reset();
        assert!(meter.generation() > generation);
        assert_eq!(meter.reading().valid_branches, 0);
        s.sample([f32::NAN, f32::INFINITY]);
        assert!(s.reading.magnitudes.iter().all(|v| v.is_finite()));
    }
}
#[test]
fn compatible_retarget_preserves_pending_jobs_and_next_publication_exactly() {
    let mut current = SpectrumAnalyzer::new(48000, true);
    let meter = Arc::new(SpectrumMeter::default());
    current.attach(meter.clone());
    for i in 0..40001 {
        current.sample([(i as f32 * 0.017).sin(); 2]);
    }
    assert!(current.branch.pending);
    let mut reference = current.clone();
    reference.attach(Arc::new(SpectrumMeter::default()));
    reference.active = true;
    let mut prepared = SpectrumAnalyzer::new(48000, true);
    prepared.attach(meter);
    current.retarget(&mut prepared);
    for i in 0..12000 {
        let v = [(i as f32 * 0.021).sin(); 2];
        current.sample(v);
        reference.sample(v);
        assert_eq!(current.reading.magnitudes, reference.reading.magnitudes);
        assert_eq!(
            current.reading.upper_end_frame,
            reference.reading.upper_end_frame
        );
        assert_eq!(
            current.reading.bass_end_frame,
            reference.reading.bass_end_frame
        );
    }
}
#[test]
fn full_rate_cells_preserve_tones_between_log_centers() {
    for rate in RATES {
        for hz in [20.3, 43.7, 121.3, 242.0, 317.0, 997.3, 12345.0] {
            let r = signal(rate, &[(hz, 0.3)], 1);
            let amplitude = peak(&r, hz as f32 * 0.96 - 2.0, hz as f32 * 1.04 + 2.0);
            let db = 20.0 * (amplitude / 0.25).log10();
            assert!(
                (-3.0..=0.2).contains(&db),
                "{rate} {hz} calibration {db} dB"
            );
        }
    }
}
#[test]
#[ignore = "release-only analyzer CPU measurement; run without other CPU workloads"]
fn analyzer_cpu_and_32_frame_tails() {
    use std::{hint::black_box, time::Instant};
    if cfg!(debug_assertions) {
        panic!("run this benchmark with --release");
    }
    for rate in RATES {
        for count in [1, 8, 32] {
            let mut analyzers: Vec<_> = (0..count)
                .map(|_| {
                    let mut s = SpectrumAnalyzer::new(rate, true);
                    s.attach(Arc::new(SpectrumMeter::default()));
                    s
                })
                .collect();
            let mut worst = std::time::Duration::ZERO;
            let start = Instant::now();
            for block in 0..rate as usize / 32 {
                let tick = Instant::now();
                for i in 0..32 {
                    let v = (std::f32::consts::TAU * 83.0 * (block * 32 + i) as f32 / rate as f32)
                        .sin();
                    for analyzer in &mut analyzers {
                        analyzer.sample(black_box([v, -v]));
                    }
                }
                worst = worst.max(tick.elapsed());
            }
            let per_instance = start.elapsed().as_secs_f64() / count as f64;
            println!(
                "rate={rate} instances={count} core/instance={:.4}% worst32={:?}",
                per_instance * 100.0,
                worst
            );
            let limit = if rate <= 48000 { 0.03 } else { 0.08 };
            assert!(
                per_instance < limit,
                "analyzer CPU budget {rate}: {per_instance}"
            );
            assert!(analyzers.iter().all(|s| s.overruns == 0));
        }
    }
}

#[test]
fn warmed_onsets_remain_responsive_and_resolved_stereo_tones_remain_distinct() {
    for rate in RATES {
        for hz in [80.0, 1000.0] {
            let mut s = SpectrumAnalyzer::new(rate, true);
            s.attach(Arc::new(SpectrumMeter::default()));
            for _ in 0..rate {
                s.sample([0.0; 2]);
            }
            let mut reached = None;
            for i in 0..rate {
                let phase = std::f64::consts::TAU * hz * f64::from(i) / f64::from(rate);
                s.sample([phase.sin() as f32 * 0.5, phase.cos() as f32 * 0.5]);
                if peak(&s.reading, hz as f32 * 0.95, hz as f32 * 1.05)
                    >= 0.5 * 10.0_f32.powf(-3.0 / 20.0)
                {
                    reached = Some(i);
                    break;
                }
            }
            let seconds = f64::from(reached.expect("warmed onset")) / f64::from(rate);
            // Reserve 35 ms for ordinary display easing on top of producer age.
            assert!(seconds + 0.035 <= 0.150, "{rate} {hz}: {seconds}");
        }
        let mut s = SpectrumAnalyzer::new(rate, true);
        s.attach(Arc::new(SpectrumMeter::default()));
        for i in 0..rate {
            let phase = std::f64::consts::TAU * f64::from(i) / f64::from(rate);
            s.sample([
                (phase * 80.0).sin() as f32 * 0.5,
                (phase * 160.0).sin() as f32 * 0.05,
            ]);
        }
        // Independent channels must preserve the weaker tone as mean stereo energy.
        let weak = peak(&s.reading, 155.0, 165.0);
        assert!(weak > 0.05 / 2.0_f32.sqrt() * 0.70, "{rate}: {weak}");
        let valley = peak(&s.reading, 118.0, 122.0);
        assert!(valley < weak * 0.5, "{rate} weak/valley {weak}/{valley}");
    }
}

#[test]
fn bass_keeps_the_fast_resolution_without_a_later_handoff() {
    for rate in RATES {
        for hz in [20.3, 40.0, 80.0, 240.0, 317.0] {
            let mut s = SpectrumAnalyzer::new(rate, true);
            s.attach(Arc::new(SpectrumMeter::default()));
            let mut early_peak = 0.0;
            for frame in 0..rate * 2 {
                let phase = std::f64::consts::TAU * hz * f64::from(frame) / f64::from(rate);
                let sample = 0.25 * phase.sin() as f32;
                s.sample([sample, -sample]);
                if frame == rate / 4 {
                    early_peak = peak(&s.reading, hz as f32 - 12.0, hz as f32 + 12.0);
                    assert!(early_peak > 0.1, "{rate} {hz}");
                }
                if frame >= rate / 4 {
                    assert_eq!(s.reading.valid_branches, SPECTRUM_UPPER_READY);
                    assert_eq!(s.reading.bass_end_frame, 0);
                }
            }
            let late_peak = peak(&s.reading, hz as f32 - 12.0, hz as f32 + 12.0);
            assert!(
                (20.0 * (late_peak / early_peak).log10()).abs() < 0.5,
                "no resolution handoff: {rate} {hz}"
            );
            s.reset();
            for _ in 0..rate / 4 {
                s.sample([0.0; 2]);
            }
            assert_eq!(s.reading.valid_branches, SPECTRUM_UPPER_READY);
            assert!(s.reading.magnitudes.iter().all(|v| *v == 0.0));
            assert_eq!(s.overruns, 0);
            assert!(s.max_work <= WORK_PER_FRAME);
        }
    }
}
