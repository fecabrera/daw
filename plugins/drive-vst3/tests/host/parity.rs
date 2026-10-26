use super::*;
#[test]
fn six_rates_shapes_blocks_and_normalized_curves_match_direct_drive() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for shape in 0..=P::ColorShape.info().maximum as usize {
            for in_place in [false, true] {
                let mut plugin = Plugin::new();
                let mut state = PluginState::default();
                state.set(P::ColorShape, shape as f64).unwrap();
                enable_color(&mut state);
                plugin.load(state);
                plugin.activate(rate, 4096, 0);
                let mut drive = Drive::new(
                    ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 4096,
                    },
                    &state.to_settings(),
                );
                let mut values = normalized(state);
                for frames in [1, 17, 32, 64, 128, 512, 4096, 31] {
                    let left: Vec<_> = (0..frames).map(|i| (i as f32 * 0.19).sin() * 0.7).collect();
                    let right: Vec<_> = left.iter().map(|v| v * -0.3 + 0.1).collect();
                    let points = if frames > 17 {
                        vec![
                            (P::Gain, vec![(0, 0.9), (frames - 1, 0.1)]),
                            (P::Tone, vec![(17, 0.8)]),
                            (P::ColorFrequency, vec![(frames - 1, 0.75)]),
                        ]
                    } else {
                        vec![]
                    };
                    let expected =
                        reference(&mut drive, &mut state, &mut values, &left, &right, &points);
                    let actual = run(
                        &plugin,
                        &mut left.clone(),
                        &mut right.clone(),
                        &points,
                        0,
                        in_place,
                        0,
                    );
                    assert_samples(&actual, &expected);
                }
            }
        }
    }
}
#[test]
fn implicit_previous_block_points_jumps_and_dense_automation_are_reconstructed() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 4096, 0);
    let mut state = PluginState::default();
    let mut values = normalized(state);
    let mut drive = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 4096,
        },
        &state.to_settings(),
    );
    for points in [
        vec![(P::Gain, vec![(0, 1.0), (31, 0.0)])],
        vec![(P::Gain, vec![(31, 1.0)])],
        vec![],
        vec![(P::Bypass, vec![(16, 0.0), (17, 1.0)])],
        vec![(P::Bypass, vec![(0, 0.0)])],
    ] {
        let left = vec![0.5; 32];
        let right = vec![-0.2; 32];
        let expected = reference(&mut drive, &mut state, &mut values, &left, &right, &points);
        assert_samples(
            &run(
                &plugin,
                &mut left.clone(),
                &mut right.clone(),
                &points,
                0,
                true,
                0,
            ),
            &expected,
        );
    }
    let points = vec![(
        P::Gain,
        (0..4096).map(|i| (i, (i % 101) as f64 / 100.0)).collect(),
    )];
    let left = vec![0.5; 4096];
    let right = vec![-0.2; 4096];
    let expected = reference(&mut drive, &mut state, &mut values, &left, &right, &points);
    assert_samples(
        &run(
            &plugin,
            &mut left.clone(),
            &mut right.clone(),
            &points,
            0,
            false,
            0,
        ),
        &expected,
    );
}
#[test]
fn silence_flags_active_recall_and_offline_mode_keep_original_dsp_behavior() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 512, 2);
    let mut state = PluginState::default();
    state
        .set(P::ColorShape, P::ColorShape.info().maximum)
        .unwrap();
    state.set(P::ColorStrength, 0.9).unwrap();
    plugin.load(state);
    let mut drive = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        },
        &PluginState::default().to_settings(),
    );
    drive.retarget(&state.to_settings());
    let mut values = normalized(state);
    let zeros = vec![0.0; 512];
    let garbage = vec![f32::NAN; 512];
    let expected = reference(&mut drive, &mut state, &mut values, &zeros, &zeros, &[]);
    assert_samples(
        &run(
            &plugin,
            &mut garbage.clone(),
            &mut garbage.clone(),
            &[],
            2,
            true,
            3,
        ),
        &expected,
    );
    let left = vec![0.4; 512];
    let right = vec![-0.125; 512];
    let points = vec![(P::Volume, vec![(0, 0.0)])];
    let expected = reference(&mut drive, &mut state, &mut values, &left, &right, &points);
    let actual = run(
        &plugin,
        &mut left.clone(),
        &mut right.clone(),
        &points,
        2,
        false,
        0,
    );
    assert_samples(&actual, &expected);
    assert!(actual.0[300..].iter().all(|v| *v == 0.0));
}

#[test]
fn exported_impulses_have_no_adapter_delay_and_decay_within_tail_policy() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for shape in 0..=P::ColorShape.info().maximum as usize {
            let mut plugin = Plugin::new();
            let mut state = PluginState::default();
            state.set(P::ColorShape, shape as f64).unwrap();
            enable_color(&mut state);
            state.set(P::ColorStrength, 1.0).unwrap();
            state.set(P::ColorFrequency, 250.0).unwrap();
            state.set(P::Volume, 12.0).unwrap();
            plugin.load(state);
            plugin.activate(rate, 512, 0);
            let mut drive = Drive::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &state.to_settings(),
            );
            let mut values = normalized(state);
            let mut processed = 0;
            while processed < rate as usize * 2 {
                let frames = 512.min(rate as usize * 2 - processed);
                let mut left = vec![0.0; frames];
                let mut right = left.clone();
                if processed == 0 {
                    left[0] = 4.0;
                    right[0] = -2.0;
                }
                let expected = reference(&mut drive, &mut state, &mut values, &left, &right, &[]);
                let actual = run(&plugin, &mut left, &mut right, &[], 0, false, 0);
                assert_samples(&actual, &expected);
                assert!(actual.0.iter().chain(&actual.1).all(|v| v.is_finite()));
                if processed >= rate as usize {
                    assert!(actual.0.iter().chain(&actual.1).all(|v| v.abs() <= 1e-6));
                }
                processed += frames;
            }
        }
    }
}
