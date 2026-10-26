use super::*;

#[test]
fn metadata_configuration_failures_reactivation_and_instance_isolation() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let second = Plugin::new();
    assert_eq!(unsafe { plugin.params().count.unwrap()(plugin.pointer) }, 7);
    for (index, parameter) in P::ALL.into_iter().enumerate() {
        let mut info = std::mem::MaybeUninit::uninit();
        assert!(unsafe {
            plugin.params().get_info.unwrap()(plugin.pointer, index as u32, info.as_mut_ptr())
        });
        let info = unsafe { info.assume_init() };
        assert_eq!(info.id, parameter as u32);
        assert_eq!(info.default_value, parameter.info().default);
        assert_eq!(info.flags & CLAP_PARAM_IS_MODULATABLE, 0);
        assert!(info.cookie.is_null());
        assert_eq!(
            info.flags & CLAP_PARAM_IS_BYPASS != 0,
            parameter == P::Bypass
        );
    }
    let ports = unsafe {
        &*(*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_AUDIO_PORTS.as_ptr())
            .cast::<clap_plugin_audio_ports>()
    };
    for input in [false, true] {
        let mut port = std::mem::MaybeUninit::uninit();
        assert_eq!(unsafe { ports.count.unwrap()(plugin.pointer, input) }, 1);
        assert!(unsafe { ports.get.unwrap()(plugin.pointer, 0, input, port.as_mut_ptr()) });
        let port = unsafe { port.assume_init() };
        assert_eq!(port.channel_count, 2);
        assert_eq!(port.flags & CLAP_AUDIO_PORT_SUPPORTS_64BITS, 0);
        assert_ne!(port.id, port.in_place_pair);
    }
    for rate in [
        0.0,
        8000.0,
        22050.0,
        384000.0,
        48000.5,
        f64::NAN,
        f64::INFINITY,
    ] {
        assert!(!unsafe { (*plugin.pointer).activate.unwrap()(plugin.pointer, rate, 1, 512) });
    }
    for (minimum, maximum) in [(0, 512), (2, 1), (1, u32::MAX)] {
        assert!(!unsafe {
            (*plugin.pointer).activate.unwrap()(plugin.pointer, 48000.0, minimum, maximum)
        });
    }
    plugin.flush(&EventList::new(&[(0, P::Gain, 0.875)]));
    assert_eq!(plugin.get(P::Gain), 0.875);
    assert_eq!(second.get(P::Gain), P::Gain.info().default);
    for rate in SUPPORTED_RATES {
        plugin.activate(rate, 512);
        let tail = unsafe {
            &*(*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_TAIL.as_ptr())
                .cast::<clap_plugin_tail>()
        };
        assert_eq!(unsafe { tail.get.unwrap()(plugin.pointer) }, rate);
        let latency = unsafe {
            &*(*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_LATENCY.as_ptr())
                .cast::<clap_plugin_latency>()
        };
        assert_eq!(unsafe { latency.get.unwrap()(plugin.pointer) }, 0);
        plugin.deactivate();
        assert_eq!(plugin.get(P::Gain), 0.875);
    }
    assert!(
        !unsafe { (*plugin.pointer).get_extension.unwrap()(plugin.pointer, c"clap.gui".as_ptr()) }
            .is_null()
    );
}

#[test]
fn streams_fail_atomically_and_latest_active_load_wins_before_block_events() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::Gain, 0.9).unwrap();
    plugin.load(state);
    for bytes in [
        vec![],
        state.encode()[..71].to_vec(),
        vec![7; 72],
        vec![0; 4096],
    ] {
        assert!(!plugin.load_bytes(&bytes, 1, false));
        assert_eq!(plugin.get(P::Gain), 0.9);
    }
    assert!(!plugin.load_bytes(&state.encode(), 1, true));
    assert!(plugin.save(0).is_none());
    assert_eq!(
        PluginState::decode(&plugin.save(1).unwrap()).unwrap(),
        state
    );
    plugin.activate(48000, 512);
    state.set(P::Tone, 0.1).unwrap();
    plugin.load(state);
    state.set(P::Tone, 0.9).unwrap();
    plugin.load(state);
    assert_eq!(plugin.get(P::Tone), 0.9);
    assert_eq!(
        PluginState::decode(&plugin.save(3).unwrap()).unwrap(),
        state
    );
    let events = EventList::new(&[(0, P::Tone, 0.3)]);
    run(
        &plugin,
        &mut vec![0.0; 512],
        &mut vec![0.0; 512],
        &events,
        false,
        0,
    );
    assert_eq!(plugin.get(P::Tone), 0.3);
    let mut invalid = state.encode();
    invalid[16..24].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(!plugin.load_bytes(&invalid, 8, false));
    assert_eq!(plugin.get(P::Tone), 0.3);
}

#[test]
fn wrong_namespace_targets_and_invalid_values_do_not_change_parameters() {
    let _serial = SERIAL.lock().unwrap();
    let plugin = Plugin::new();
    let mut events = EventList::new(&[(0, P::Gain, 0.9); 5]);
    events.0[0].header.space_id = 123;
    events.0[1].note_id = 1;
    events.0[2].value = f64::NAN;
    events.0[3].value = 2.0;
    events.0[4].param_id = u32::MAX;
    plugin.flush(&events);
    assert_eq!(plugin.get(P::Gain), P::Gain.info().default);
    plugin.flush(&EventList::new(&[(0, P::ColorShape, 2.9)]));
    assert_eq!(plugin.get(P::ColorShape), 2.0);
}

#[test]
fn concurrent_active_recall_and_save_publish_coherent_snapshots() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    assert!(unsafe { (*plugin.pointer).activate.unwrap()(plugin.pointer, 48000.0, 1, 32) });
    plugin.active = true;
    let pointer = plugin.pointer as usize;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_done = done.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let ready = barrier.clone();
    let counts = plugin.counts;
    let worker = std::thread::spawn(move || {
        let pointer = pointer as *const clap_plugin;
        assert!(unsafe { (*pointer).start_processing.unwrap()(pointer) });
        let mut channels = [[0.25_f32; 32], [-0.125; 32]];
        let mut pointers = [channels[0].as_mut_ptr(), channels[1].as_mut_ptr()];
        let input = clap_audio_buffer {
            data32: pointers.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut output = input;
        let process = clap_process {
            steady_time: -1,
            frames_count: 32,
            transport: ptr::null(),
            audio_inputs: &input,
            audio_outputs: &mut output,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: ptr::null(),
            out_events: ptr::null(),
        };
        ready.wait();
        let mut blocks = 0;
        while !worker_done.load(Ordering::Acquire) {
            if let Some((begin, _)) = counts {
                unsafe {
                    begin();
                }
            }
            let status = unsafe { (*pointer).process.unwrap()(pointer, &process) };
            if let Some((_, end)) = counts {
                let (mut a, mut d) = (0, 0);
                unsafe {
                    end(&mut a, &mut d);
                }
                assert_eq!((a, d), (0, 0));
            }
            assert_eq!(status, CLAP_PROCESS_CONTINUE);
            blocks += 1;
        }
        unsafe {
            (*pointer).stop_processing.unwrap()(pointer);
        }
        blocks
    });
    barrier.wait();
    let mut expected = PluginState::default();
    let mut saved = 0;
    for index in 0..1000 {
        let value = index as f64 / 1000.0;
        for parameter in [P::Gain, P::Tone, P::ColorStrength] {
            expected.set(parameter, value).unwrap();
        }
        expected
            .set(P::ColorFrequency, 250.0 + 1750.0 * value)
            .unwrap();
        plugin.load(expected);
        if let Some(bytes) = plugin.save(8) {
            assert_eq!(PluginState::decode(&bytes).unwrap(), expected);
            saved += 1;
        }
    }
    done.store(true, Ordering::Release);
    assert!(worker.join().unwrap() > 0);
    assert!(saved > 0);
    assert_eq!(
        PluginState::decode(&plugin.save(8).unwrap()).unwrap(),
        expected
    );
}

#[test]
// Mutations are observed by the plugin through raw CLAP pointers.
#[allow(unused_assignments)]
fn invalid_audio_and_event_bounds_return_errors_without_callback_allocation() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 32);
    let mut left = [0.2; 33];
    let mut right = [0.3; 33];
    let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
    let mut input = clap_audio_buffer {
        data32: pointers.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: 0,
    };
    let mut output = input;
    let mut process = clap_process {
        steady_time: -1,
        frames_count: 33,
        transport: ptr::null(),
        audio_inputs: &input,
        audio_outputs: &mut output,
        audio_inputs_count: 1,
        audio_outputs_count: 1,
        in_events: ptr::null(),
        out_events: ptr::null(),
    };
    let call = |process: &clap_process| {
        plugin.measured(|| unsafe { (*plugin.pointer).process.unwrap()(plugin.pointer, process) })
    };
    assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    process.frames_count = 32;
    process.audio_inputs_count = 0;
    assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    process.audio_inputs_count = 1;
    input.channel_count = 1;
    assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    input.channel_count = 2;
    left[0] = f32::NAN;
    assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    left[0] = 0.2;
    input.data64 = std::ptr::dangling_mut();
    assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    input.data64 = ptr::null_mut();
    for events in [
        EventList::new(&[(32, P::Gain, 0.9)]),
        EventList::new(&[(17, P::Gain, 0.9), (1, P::Tone, 0.9)]),
    ] {
        let abi = events.abi();
        process.in_events = &abi;
        assert_eq!(call(&process), CLAP_PROCESS_ERROR);
    }
    plugin.reset();
    let left = vec![0.2; 32];
    let right = vec![0.3; 32];
    let result = run(
        &plugin,
        &mut left.clone(),
        &mut right.clone(),
        &EventList::new(&[]),
        false,
        0,
    );
    assert!(result.0.iter().chain(&result.1).all(|v| v.is_finite()));
}

#[test]
fn editor_contract_can_be_queried_and_closed_without_initializing_graphics() {
    use daw_plugin_clap::sys::ext::gui::*;
    let _serial = SERIAL.lock().unwrap();
    let plugin = Plugin::new();
    let gui = unsafe {
        &*((*plugin.pointer).get_extension.unwrap()(plugin.pointer, CLAP_EXT_GUI.as_ptr())
            .cast::<clap_plugin_gui>())
    };
    let mut api = ptr::null();
    let mut floating = true;
    assert!(unsafe { gui.get_preferred_api.unwrap()(plugin.pointer, &mut api, &mut floating) });
    assert!(!floating);
    assert!(unsafe { gui.is_api_supported.unwrap()(plugin.pointer, api, false) });
    assert!(!unsafe { gui.is_api_supported.unwrap()(plugin.pointer, api, true) });
    assert!(unsafe { gui.create.unwrap()(plugin.pointer, api, false) });
    assert!(!unsafe { gui.create.unwrap()(plugin.pointer, api, false) });
    let (mut width, mut height) = (0, 0);
    assert!(unsafe { gui.get_size.unwrap()(plugin.pointer, &mut width, &mut height) });
    assert_eq!((width, height), (180, 218));
    assert!(!unsafe { gui.can_resize.unwrap()(plugin.pointer) });
    assert!(!unsafe { gui.get_resize_hints.unwrap()(plugin.pointer, ptr::null_mut()) });
    assert!(!unsafe { gui.set_scale.unwrap()(plugin.pointer, f64::NAN) });
    assert!(!unsafe { gui.set_parent.unwrap()(plugin.pointer, ptr::null()) });
    assert!(!unsafe { gui.show.unwrap()(plugin.pointer) });
    assert!(!unsafe { gui.hide.unwrap()(plugin.pointer) });
    unsafe {
        gui.destroy.unwrap()(plugin.pointer);
    }
    assert!(!unsafe { gui.get_size.unwrap()(plugin.pointer, &mut width, &mut height) });
}
