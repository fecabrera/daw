use super::*;
fn choices() -> Vec<PluginState> {
    let mut states = vec![PluginState::default()];
    for p in P::ALL.into_iter().filter(|p| {
        p.info().steps > 0
            && *p != P::Bypass
            && (!p.info().key.starts_with("band_")
                || p.info().key.starts_with("band_1_")
                || p.info().key.starts_with("band_64_"))
    }) {
        for value in p.info().minimum as u32..=p.info().maximum as u32 {
            let mut s = PluginState::default();
            s.set(p, f64::from(value)).unwrap();
            states.push(s);
        }
    }
    states
}
#[test]
fn native_metadata_latency_and_editor_geometry_follow_the_unit_contract() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();

    assert_eq!(
        unsafe { plugin.controller.getParameterCount() },
        P::COUNT as i32
    );
    for (index, p) in P::ALL.into_iter().enumerate() {
        let mut info = std::mem::MaybeUninit::uninit();
        assert_eq!(
            unsafe {
                plugin
                    .controller
                    .getParameterInfo(index as i32, info.as_mut_ptr())
            },
            kResultOk
        );
        let info = unsafe { info.assume_init() };
        assert_eq!(info.id, p as u32);
        assert_eq!(info.stepCount, p.info().steps as i32);
    }
    for rate in SUPPORTED_RATES {
        plugin.activate(rate, 4096, 0);
        let direct = Drive::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &PluginState::default().to_settings(),
        );
        assert_eq!(
            unsafe { plugin.processor.getLatencySamples() },
            direct.latency_frames()
        );
        plugin.deactivate();
    }
    let view =
        unsafe { ComPtr::<IPlugView>::from_raw(plugin.controller.createView(c"editor".as_ptr())) }
            .unwrap();
    let mut rect: ViewRect = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { view.getSize(&mut rect) }, kResultOk);
    assert_eq!(
        (rect.right, rect.bottom),
        (
            daw_drive_editor::WIDTH as i32,
            daw_drive_editor::HEIGHT as i32
        )
    );
    assert_eq!(unsafe { view.canResize() }, kResultFalse);
}
#[test]
fn every_choice_and_six_rates_match_direct_dsp_for_irregular_and_in_place_blocks() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for initial in choices() {
            for in_place in [false, true] {
                let mut plugin = Plugin::new();
                let mut state = initial;
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
                for frames in [1, 17, 128, 512, 4096, 31] {
                    let mut left: Vec<_> =
                        (0..frames).map(|i| (i as f32 * 0.17).sin() * 0.8).collect();
                    let mut right: Vec<_> = left.iter().map(|v| -0.37 * v + 0.1).collect();
                    let expected =
                        reference(&mut drive, &mut state, &mut values, &left, &right, &[]);
                    let actual = run(&plugin, &mut left, &mut right, &[], 0, in_place, 0);
                    assert_samples(&actual, &expected);
                }
            }
        }
    }
}
#[test]
fn all_parameters_and_dense_automation_follow_format_timing_without_callback_heap_work() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        let mut plugin = Plugin::new();
        let mut state = PluginState::default();
        plugin.activate(rate, 4096, 0);
        let mut drive = Drive::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &state.to_settings(),
        );
        let mut values = normalized(state);
        let left = vec![0.4; 4096];
        let right = vec![-0.2; 4096];
        for p in P::ALL {
            let left = vec![0.4; 64];
            let right = vec![-0.2; 64];
            let points = [(
                p,
                vec![
                    (0, 1.0),
                    (17, 0.0),
                    (63, p.normalize(p.info().default).unwrap()),
                ],
            )];
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
        let points = [(
            test_parameter(),
            (0..4096).map(|i| (i, f64::from(i % 101) / 100.0)).collect(),
        )];
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
}
#[test]
fn state_roundtrips_reject_corruption_and_active_recall_keeps_latest_values() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(test_parameter(), test_value(0.7)).unwrap();
    state.set(test_parameter(), test_value(0.37)).unwrap();
    plugin.load(state);
    assert_eq!(plugin.save().unwrap(), state);
    let mut broken = state.encode();
    broken[0] ^= 255;
    let stream = ComWrapper::new(Stream::new(broken.to_vec(), 1));
    let pointer = stream.to_com_ptr::<IBStream>().unwrap();
    assert_ne!(
        unsafe { plugin.component.setState(pointer.as_ptr()) },
        kResultOk
    );
    assert_eq!(plugin.save().unwrap(), state);
    plugin.activate(48000, 512, 0);
    state.set(test_parameter(), test_value(0.2)).unwrap();
    plugin.load(state);
    state.set(test_parameter(), test_value(0.9)).unwrap();
    plugin.load(state);
    let mut left = vec![0.2; 512];
    let mut right = vec![-0.3; 512];
    run(&plugin, &mut left, &mut right, &[], 0, false, 0);
    assert_eq!(plugin.save().unwrap(), state);
}

fn test_parameter() -> P {
    P::ALL
        .into_iter()
        .find(|p| *p != P::Bypass && p.info().steps == 0)
        .unwrap()
}
fn test_value(value: f64) -> f64 {
    test_parameter().denormalize(value).unwrap()
}
