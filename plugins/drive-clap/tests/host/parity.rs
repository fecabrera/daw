use super::*;

#[test]
fn six_rates_all_shapes_block_sizes_and_live_automation_match_direct_drive() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for shape in 0..4 {
            for in_place in [false, true] {
                let mut plugin = Plugin::new();
                let mut state = PluginState::default();
                state.set(P::ColorShape, shape as f64).unwrap();
                plugin.load(state);
                plugin.activate(rate, 4096);
                let mut drive = Drive::new(
                    ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 4096,
                    },
                    &state.to_settings(),
                );
                let mut offset = 0;
                for frames in [1, 17, 32, 64, 128, 512, 4096, 31] {
                    let left: Vec<f32> = (0..frames)
                        .map(|i| ((i + offset) as f32 * 0.17).sin() * 0.8)
                        .collect();
                    let right: Vec<f32> = left.iter().map(|v| v * -0.37 + 0.1).collect();
                    let events = if frames > 17 {
                        EventList::new(&[
                            (0, P::Volume, -6.0),
                            (17, P::Gain, 0.95),
                            (17, P::Tone, 0.05),
                            ((frames - 1) as u32, P::ColorFrequency, 1800.0),
                            ((frames - 1) as u32, P::ColorStrength, 0.9),
                        ])
                    } else {
                        EventList::new(&[])
                    };
                    let expected = reference(&mut drive, &mut state, &left, &right, &events);
                    let actual = run(
                        &plugin,
                        &mut left.clone(),
                        &mut right.clone(),
                        &events,
                        in_place,
                        0,
                    );
                    assert_samples(&actual, &expected);
                    offset += frames;
                }
                plugin.reset();
                drive.reset();
                let events = EventList::new(&[]);
                let left = vec![0.0; 512];
                assert_samples(
                    &run(
                        &plugin,
                        &mut left.clone(),
                        &mut left.clone(),
                        &events,
                        false,
                        0,
                    ),
                    &reference(&mut drive, &mut state, &left, &left, &events),
                );
            }
        }
    }
}

#[test]
fn dense_events_flush_bypass_retained_color_and_state_recall_match_direct_drive() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 4096);
    let mut state = PluginState::default();
    let mut drive = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 4096,
        },
        &state.to_settings(),
    );
    let events = EventList::new(
        &(0..4096)
            .map(|i| (i, P::Gain, (i % 101) as f64 / 100.0))
            .collect::<Vec<_>>(),
    );
    let left = vec![0.4; 4096];
    let right = vec![-0.2; 4096];
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
    plugin.stop();
    let events = EventList::new(&[
        (0, P::Bypass, 1.0),
        (0, P::ColorStrength, 0.8),
        (0, P::ColorFrequency, 1500.0),
    ]);
    plugin.flush(&events);
    for e in &events.0 {
        state.set(P::from_id(e.param_id).unwrap(), e.value).unwrap();
        drive.retarget(&state.to_settings());
    }
    assert_eq!(plugin.get(P::ColorStrength), 0.8);
    assert!(unsafe { (*plugin.pointer).start_processing.unwrap()(plugin.pointer) });
    plugin.processing = true;
    for _ in 0..4 {
        assert_samples(
            &run(
                &plugin,
                &mut left.clone(),
                &mut right.clone(),
                &EventList::new(&[]),
                true,
                0,
            ),
            &reference(&mut drive, &mut state, &left, &right, &EventList::new(&[])),
        );
    }
    state.set(P::Bypass, 0.0).unwrap();
    state.set(P::ColorShape, 3.0).unwrap();
    state.set(P::Volume, -1000.0).unwrap();
    plugin.load(state);
    drive.retarget(&state.to_settings());
    let expected = reference(&mut drive, &mut state, &left, &right, &EventList::new(&[]));
    let actual = run(
        &plugin,
        &mut left.clone(),
        &mut right.clone(),
        &EventList::new(&[]),
        false,
        0,
    );
    assert_samples(&actual, &expected);
    assert!(actual.0[300..].iter().all(|v| *v == 0.0));
    assert!(plugin.data.requests.load(Ordering::Relaxed) > 0);
    assert!(plugin.data.rescans.load(Ordering::Relaxed) > 0);
}

#[test]
fn constants_in_place_and_zero_frame_parameter_calls_are_safe() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 512);
    let mut state = PluginState::default();
    let mut drive = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        },
        &state.to_settings(),
    );
    let events = EventList::new(&[]);
    let expected = reference(
        &mut drive,
        &mut state,
        &vec![0.25; 512],
        &vec![-0.125; 512],
        &events,
    );
    let mut left = vec![999.0; 512];
    left[0] = 0.25;
    let mut right = vec![999.0; 512];
    right[0] = -0.125;
    assert_samples(
        &run(&plugin, &mut left, &mut right, &events, true, 3),
        &expected,
    );
    let events = EventList::new(&[(0, P::Tone, 0.9)]);
    let abi = events.abi();
    let process = clap_process {
        steady_time: -1,
        frames_count: 0,
        transport: ptr::null(),
        audio_inputs: ptr::null(),
        audio_outputs: ptr::null_mut(),
        audio_inputs_count: 0,
        audio_outputs_count: 0,
        in_events: &abi,
        out_events: ptr::null(),
    };
    assert_eq!(
        plugin.measured(|| unsafe { (*plugin.pointer).process.unwrap()(plugin.pointer, &process) }),
        CLAP_PROCESS_CONTINUE
    );
    assert_eq!(plugin.get(P::Tone), 0.9);
}

#[test]
fn exported_impulse_tail_has_no_adapter_delay_and_decays_within_policy() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for shape in 0..4 {
            let mut plugin = Plugin::new();
            let mut state = PluginState::default();
            state.set(P::ColorShape, shape as f64).unwrap();
            state.set(P::ColorStrength, 1.0).unwrap();
            state.set(P::ColorFrequency, 250.0).unwrap();
            state.set(P::Volume, 12.0).unwrap();
            plugin.load(state);
            plugin.activate(rate, 512);
            let mut drive = Drive::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &state.to_settings(),
            );
            let events = EventList::new(&[]);
            let mut processed = 0;
            while processed < rate as usize * 2 {
                let frames = 512.min(rate as usize * 2 - processed);
                let mut left = vec![0.0; frames];
                let mut right = left.clone();
                if processed == 0 {
                    left[0] = 4.0;
                    right[0] = -2.0;
                }
                let expected = reference(&mut drive, &mut state, &left, &right, &events);
                let actual = run(&plugin, &mut left, &mut right, &events, false, 0);
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

#[test]
fn in_place_audio_buffer_header_may_alias_itself() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 32);
    let mut left = vec![0.2; 32];
    let mut right = vec![-0.3; 32];
    let mut state = PluginState::default();
    let mut drive = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 32,
        },
        &state.to_settings(),
    );
    let expected = reference(&mut drive, &mut state, &left, &right, &EventList::new(&[]));
    let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
    let mut buffer = clap_audio_buffer {
        data32: pointers.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: 3,
    };
    let pointer = &mut buffer as *mut clap_audio_buffer;
    let process = clap_process {
        steady_time: -1,
        frames_count: 32,
        transport: ptr::null(),
        audio_inputs: pointer,
        audio_outputs: pointer,
        audio_inputs_count: 1,
        audio_outputs_count: 1,
        in_events: ptr::null(),
        out_events: ptr::null(),
    };
    assert_eq!(
        plugin.measured(|| unsafe { (*plugin.pointer).process.unwrap()(plugin.pointer, &process) }),
        CLAP_PROCESS_CONTINUE
    );
    assert_samples(&(left, right), &expected);
}
