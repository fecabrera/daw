use audio_analysis::{Error, SAMPLE_RATES, TransientDetector as Mode, TransientRequest};
use std::{f64::consts::TAU, ops::ControlFlow};

#[test]
fn validation_budget_progress_and_cancellation() {
    let request = TransientRequest::new(48000, 1);
    assert_eq!(request.detector, Mode::Combined);
    assert_eq!(Mode::Combined.revision(), "energy-spectral-v5");
    assert_eq!(Mode::Energy.revision(), "energy-r4");
    assert!(request.detect(&[]).unwrap().is_empty());
    assert!(request.detect(&[0.0; 4800]).unwrap().is_empty());
    for channels in [0, 3, usize::MAX] {
        assert_eq!(
            TransientRequest::new(48000, channels).detect(&[]),
            Err(Error::InvalidLayout)
        );
    }
    assert_eq!(
        TransientRequest::new(123, 1).detect(&[]),
        Err(Error::InvalidRate)
    );
    assert_eq!(
        TransientRequest::new(48000, 2).detect(&[1.0]),
        Err(Error::InvalidLayout)
    );
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(request.detect(&[value]), Err(Error::NonfiniteInput));
    }
    assert_eq!(
        request.estimated_memory_bytes(usize::MAX),
        Err(Error::ResourceLimit)
    );
    let input = vec![0.0; 9600];
    let bytes = request.estimated_memory_bytes(input.len()).unwrap();
    assert_eq!(
        TransientRequest {
            memory_limit_bytes: bytes - 1,
            ..request
        }
        .detect(&input),
        Err(Error::ResourceLimit)
    );
    assert!(
        TransientRequest {
            memory_limit_bytes: bytes,
            ..request
        }
        .detect(&input)
        .is_ok()
    );
    for stop in [0.0, 0.05, 0.5, 1.0] {
        assert_eq!(
            request.detect_with_progress(&input, |p| if p >= stop {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }),
            Err(Error::Cancelled)
        );
    }
    let mut previous = 0.0;
    request
        .detect_with_progress(&input, |p| {
            assert!((previous..=1.0).contains(&p));
            previous = p;
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(previous, 1.0);
}

#[test]
fn masked_broadband_attacks_and_harmonic_rejection_are_portable() {
    for rate in SAMPLE_RATES {
        for broadband in [false, true] {
            let mut seed = 0x7ac3_9d21_u32;
            let input: Vec<_> = (0..rate as usize)
                .map(|i| {
                    let t = i as f64 / f64::from(rate);
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let noise = f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0;
                    let mut sample = 0.7 * (TAU * 65.40639133 * t).sin();
                    for event in [0.2, 0.5, 0.8] {
                        let age = t - event;
                        if broadband && (0.0..0.0125).contains(&age) {
                            sample += 0.025 * noise * (-age * 300.0).exp();
                        } else if !broadband && (0.0..0.025).contains(&age) {
                            sample += 0.06
                                * (age / 0.002).min(1.0)
                                * (-age * 100.0).exp()
                                * (TAU * 3000.0 * age).sin();
                        }
                    }
                    sample as f32
                })
                .collect();
            let request = TransientRequest::new(rate, 1);
            let found = request.detect(&input).unwrap();
            let energy = TransientRequest {
                detector: Mode::Energy,
                ..request
            }
            .detect(&input)
            .unwrap();
            if broadband {
                assert_eq!(energy, [0]);
                assert_eq!(found.len(), 4, "{rate}: {found:?}");
                for (&at, time) in found[1..].iter().zip([0.2, 0.5, 0.8]) {
                    assert!((at as f64 / f64::from(rate) - time).abs() < 0.01);
                }
            } else {
                assert_eq!(found, energy, "{rate}: {found:?}");
            }
            for scale in [0.01, 2.0] {
                let stereo: Vec<_> = input.iter().flat_map(|s| [s * scale, -s * scale]).collect();
                assert_eq!(
                    TransientRequest::new(rate, 2).detect(&stereo).unwrap(),
                    found
                );
            }
        }
    }
}
