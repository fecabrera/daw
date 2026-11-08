use std::ops::ControlFlow;
use timestretch::{Error, Ratio, Request, TransientDetector as Mode, detect_transients};
#[test]
fn optional_detector_obeys_identity_errors_cancellation_and_default_compatibility() {
    let input: Vec<_> = (0..4800).map(|i| ((i as f32) * 0.13).sin() * 0.5).collect();
    for mode in [Mode::Energy, Mode::Combined] {
        let request = Request::new(48000, 1, Ratio::new(1, 1).unwrap());
        let analysis = audio_analysis::TransientRequest {
            detector: mode,
            ..audio_analysis::TransientRequest::new(48000, 1)
        };
        assert_eq!(
            detect_transients(request, &input, mode, |_| ControlFlow::Continue(())).unwrap(),
            analysis.detect(&input).unwrap()
        );
        let mut exact = input.clone();
        exact[0] = -0.0;
        let output = request
            .process_with_transient_detector(&exact, mode, |_| ControlFlow::Continue(()))
            .unwrap();
        assert!(
            exact
                .iter()
                .zip(output)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert!(
            detect_transients(request, &[], mode, |_| ControlFlow::Continue(()))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            detect_transients(request, &[f32::NAN], mode, |_| ControlFlow::Continue(())),
            Err(Error::NonfiniteInput)
        );
        assert_eq!(
            detect_transients(request, &input, mode, |p| if p >= 0.12 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }),
            Err(Error::Cancelled)
        );
        let mut low = request;
        low.memory_limit_bytes = 1;
        assert_eq!(
            detect_transients(low, &input, mode, |_| ControlFlow::Continue(())),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            low.process_with_transient_detector(&input, mode, |_| ControlFlow::Continue(())),
            Err(Error::ResourceLimit)
        );
    }
    let request = Request::new(48000, 1, Ratio::new(3, 2).unwrap());
    assert_eq!(
        request.process(&input).unwrap(),
        request
            .process_with_transient_detector(&input, Mode::Combined, |_| ControlFlow::Continue(()))
            .unwrap()
    );
    let output = request
        .process_with_transient_detector(&input, Mode::Combined, |_| ControlFlow::Continue(()))
        .unwrap();
    assert_eq!(output.len(), 7200);
    assert!(output.iter().all(|x| x.is_finite()));
    let invalid = Request::new(48000, 2, Ratio::new(1, 1).unwrap());
    assert_eq!(
        detect_transients(invalid, &[1.0], Mode::Combined, |_| ControlFlow::Continue(
            ()
        )),
        Err(Error::InvalidLayout)
    );
}

#[test]
fn combined_short_sources_compression_and_linked_stereo_keep_contracts() {
    for rate in timestretch::SAMPLE_RATES {
        let input: Vec<_> = (0..rate as usize / 10)
            .map(|i| {
                if i > rate as usize / 20 && i < rate as usize / 20 + 20 {
                    0.4
                } else {
                    0.0
                }
            })
            .collect();
        let short = &input[..10];
        let request = Request::new(rate, 1, Ratio::new(3, 2).unwrap());
        let energy =
            detect_transients(request, short, Mode::Energy, |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(
            energy,
            detect_transients(
                request,
                short,
                Mode::Combined,
                |_| ControlFlow::Continue(())
            )
            .unwrap()
        );
        let compression = Request::new(rate, 1, Ratio::new(3, 4).unwrap());
        assert_eq!(
            compression.process(&input).unwrap(),
            compression
                .process_with_transient_detector(&input, Mode::Combined, |_| ControlFlow::Continue(
                    ()
                ))
                .unwrap()
        );
        let stereo: Vec<_> = input.iter().flat_map(|s| [*s, -*s]).collect();
        let output = Request::new(rate, 2, Ratio::new(3, 2).unwrap())
            .process_with_transient_detector(&stereo, Mode::Combined, |_| ControlFlow::Continue(()))
            .unwrap();
        assert_eq!(
            output.len(),
            2 * Ratio::new(3, 2)
                .unwrap()
                .output_frames(input.len())
                .unwrap()
        );
        assert!(output.chunks_exact(2).all(|s| (s[0] + s[1]).abs() < 1e-6));
    }
}

#[test]
fn narrow_harmonic_rises_do_not_reset_a_sustained_note() {
    // A quiet picked partial over a held note is spectral growth, but not a
    // broadband attack. Resetting every partial there bends the held note.
    for rate in timestretch::SAMPLE_RATES {
        let input: Vec<_> = (0..rate as usize)
            .map(|i| {
                let t = i as f64 / f64::from(rate);
                let mut value = 0.7 * (std::f64::consts::TAU * 65.40639133 * t).sin();
                for at in [0.2, 0.5, 0.8] {
                    let age = t - at;
                    if (0.0..0.025).contains(&age) {
                        value += 0.06
                            * (age / 0.002).min(1.0)
                            * (-age * 100.0).exp()
                            * (std::f64::consts::TAU * 3000.0 * age).sin();
                    }
                }
                value as f32
            })
            .collect();
        let request = Request::new(rate, 1, Ratio::new(3, 2).unwrap());
        let detect =
            |mode| detect_transients(request, &input, mode, |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(detect(Mode::Combined), detect(Mode::Energy), "{rate}");
        if matches!(rate, 48000 | 96000) {
            for ratio in [Ratio::new(3, 2).unwrap(), Ratio::new(2, 1).unwrap()] {
                let request = Request::new(rate, 1, ratio);
                let render =
                    |mode| {
                        request
                            .process_with_transient_detector(&input, mode, |_| {
                                ControlFlow::Continue(())
                            })
                            .unwrap()
                    };
                assert_eq!(render(Mode::Combined), render(Mode::Energy));
            }
        }
    }
}
