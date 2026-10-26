use super::*;
fn choices() -> Vec<PluginState> {
    let mut states = vec![PluginState::default()];
    for p in P::ALL
        .into_iter()
        .filter(|p| p.info().steps > 0 && *p != P::Bypass)
    {
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
        unsafe { plugin.params().count.unwrap()(plugin.pointer) },
        P::COUNT as u32
    );
    for (index, p) in P::ALL.into_iter().enumerate() {
        let mut info = std::mem::MaybeUninit::uninit();
        assert!(unsafe {
            plugin.params().get_info.unwrap()(plugin.pointer, index as u32, info.as_mut_ptr())
        });
        let info = unsafe { info.assume_init() };
        assert_eq!(info.id, p as u32);
        assert_eq!(info.min_value, p.info().minimum);
        assert_eq!(info.max_value, p.info().maximum);
    }
    let ports = unsafe {
        &*((*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_AUDIO_PORTS.as_ptr())
            .cast::<clap_plugin_audio_ports>())
    };
    for input in [false, true] {
        assert_eq!(unsafe { ports.count.unwrap()(plugin.pointer, input) }, 1);
        let mut info = std::mem::MaybeUninit::uninit();
        assert!(unsafe { ports.get.unwrap()(plugin.pointer, 0, input, info.as_mut_ptr()) });
        assert_eq!(unsafe { info.assume_init() }.channel_count, 2);
    }
    for rate in SUPPORTED_RATES {
        plugin.activate(rate, 4096);
        let latency = unsafe {
            &*((*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_LATENCY.as_ptr())
                .cast::<clap_plugin_latency>())
        };
        let direct = Drive::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &PluginState::default().to_settings(),
        );
        assert_eq!(
            unsafe { latency.get.unwrap()(plugin.pointer) },
            direct.latency_frames()
        );
        let tail = unsafe {
            &*((*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_TAIL.as_ptr())
                .cast::<clap_plugin_tail>())
        };
        assert_eq!(unsafe { tail.get.unwrap()(plugin.pointer) }, rate);
        plugin.deactivate();
    }
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
                plugin.activate(rate, 4096);
                let mut drive = Drive::new(
                    ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 4096,
                    },
                    &state.to_settings(),
                );
                for frames in [1, 17, 128, 512, 4096, 31] {
                    let mut left: Vec<_> =
                        (0..frames).map(|i| (i as f32 * 0.17).sin() * 0.8).collect();
                    let mut right: Vec<_> = left.iter().map(|v| -0.37 * v + 0.1).collect();
                    let events = EventList::new(&[]);
                    let expected = reference(&mut drive, &mut state, &left, &right, &events);
                    let actual = run(&plugin, &mut left, &mut right, &events, in_place, 0);
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
        plugin.activate(rate, 4096);
        let mut drive = Drive::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &state.to_settings(),
        );
        let left = vec![0.4; 4096];
        let right = vec![-0.2; 4096];
        for p in P::ALL {
            let events = EventList::new(&[
                (0, p, p.info().maximum),
                (17, p, p.info().minimum),
                (4095, p, p.info().default),
            ]);
            let expected = reference(&mut drive, &mut state, &left, &right, &events);
            assert_samples(
                &run(
                    &plugin,
                    &mut left.clone(),
                    &mut right.clone(),
                    &events,
                    false,
                    0,
                ),
                &expected,
            );
        }
        let events = EventList::new(
            &(0..4096)
                .map(|i| {
                    (
                        i,
                        P::Drive,
                        P::Drive.denormalize(f64::from(i % 101) / 100.0).unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let expected = reference(&mut drive, &mut state, &left, &right, &events);
        assert_samples(
            &run(
                &plugin,
                &mut left.clone(),
                &mut right.clone(),
                &events,
                true,
                0,
            ),
            &expected,
        );
        // Legal zero-frame flush is measured by the instrumented library.
        plugin.flush(&EventList::new(&[(0, P::Level, -6.0)]));
        assert_eq!(plugin.get(P::Level), -6.0);
    }
}
#[test]
fn state_roundtrips_reject_corruption_and_active_recall_keeps_latest_values() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::Drive, 12.0).unwrap();
    state.set(P::DryWet, 0.37).unwrap();
    plugin.load(state);
    assert_eq!(
        PluginState::decode(&plugin.save(1).unwrap()).unwrap(),
        state
    );
    let mut broken = state.encode();
    broken[0] ^= 255;
    assert!(!plugin.load_bytes(&broken, 1, false));
    assert_eq!(
        PluginState::decode(&plugin.save(1).unwrap()).unwrap(),
        state
    );
    plugin.activate(48000, 512);
    state.set(P::Level, -6.0).unwrap();
    plugin.load(state);
    state.set(P::Level, 3.0).unwrap();
    plugin.load(state);
    let mut left = vec![0.2; 512];
    let mut right = vec![-0.3; 512];
    run(
        &plugin,
        &mut left,
        &mut right,
        &EventList::new(&[]),
        false,
        0,
    );
    assert_eq!(
        PluginState::decode(&plugin.save(1).unwrap()).unwrap(),
        state
    );
}
