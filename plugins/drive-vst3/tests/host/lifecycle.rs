use super::*;
#[test]
fn identity_factory_metadata_negotiation_and_lifecycle() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    assert_eq!(
        daw_drive_vst3::PROCESSOR_ID,
        class_id(IDENTITY.processor_id)
    );
    let mut controller_id = [0; 16];
    assert_eq!(
        unsafe { plugin.component.getControllerClassId(&mut controller_id) },
        kResultOk
    );
    assert_eq!(controller_id, class_id(IDENTITY.controller_id));
    assert_eq!(unsafe { plugin.controller.getParameterCount() }, 7);
    let requirements = plugin
        .component
        .cast::<IProcessContextRequirements>()
        .unwrap();
    assert_eq!(unsafe { requirements.getProcessContextRequirements() }, 0);
    for (index, parameter) in P::ALL.iter().enumerate() {
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
        assert_eq!(info.id, *parameter as u32);
        assert_eq!(info.stepCount, parameter.info().steps as i32);
        assert_eq!(
            info.flags & ParameterInfo_::ParameterFlags_::kIsBypass != 0,
            *parameter == P::Bypass
        );
    }
    assert_eq!(
        unsafe { plugin.processor.canProcessSampleSize(1) },
        kResultFalse
    );
    for rate in [0.0, 8000.0, 48000.5, 384000.0, f64::NAN, f64::INFINITY] {
        let mut setup = ProcessSetup {
            processMode: 0,
            symbolicSampleSize: 0,
            maxSamplesPerBlock: 512,
            sampleRate: rate,
        };
        assert_ne!(
            unsafe { plugin.processor.setupProcessing(&mut setup) },
            kResultOk
        );
    }
    for rate in SUPPORTED_RATES {
        plugin.activate(rate, 512, 0);
        assert_eq!(unsafe { plugin.processor.getTailSamples() }, rate);
        assert_eq!(unsafe { plugin.processor.getLatencySamples() }, 0);
        plugin.deactivate();
    }
    // Creating the COM view and querying geometry must not initialize graphics.
    let view =
        unsafe { ComPtr::<IPlugView>::from_raw(plugin.controller.createView(c"editor".as_ptr())) }
            .unwrap();
    let mut size: ViewRect = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { view.getSize(&mut size) }, kResultOk);
    assert_eq!((size.right, size.bottom), (180, 218));
    assert_eq!(unsafe { view.canResize() }, kResultFalse);
    assert!(unsafe { plugin.controller.createView(c"unknown".as_ptr()) }.is_null());
}
#[test]
fn component_state_and_host_controller_sync_are_atomic_and_do_not_echo_edits() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let second = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::Gain, 0.875).unwrap();
    plugin.load(state);
    assert_eq!(plugin.save().unwrap(), state);
    assert_eq!(second.save().unwrap(), PluginState::default());
    assert_eq!(
        unsafe { plugin.controller.getParamNormalized(P::Gain as u32) },
        0.875
    );
    assert_eq!(plugin.host_events.load(Ordering::Relaxed), 0);
    for bytes in [
        vec![],
        vec![7; 72],
        state.encode()[..71].to_vec(),
        vec![0; 4096],
    ] {
        let stream = ComWrapper::new(Stream::new(bytes, 1));
        let pointer = stream.to_com_ptr::<IBStream>().unwrap();
        assert_ne!(
            unsafe { plugin.component.setState(pointer.as_ptr()) },
            kResultOk
        );
        assert_eq!(plugin.save().unwrap(), state);
    }
    plugin.activate(48000, 32, 0);
    state.set(P::ColorStrength, 0.9).unwrap();
    state.set(P::ColorFrequency, 1500.0).unwrap();
    plugin.load(state);
    assert_eq!(plugin.save().unwrap(), state);
    let points = vec![(P::Gain, vec![(0, 0.25)])];
    run(
        &plugin,
        &mut [0.1; 32],
        &mut [-0.2; 32],
        &points,
        0,
        false,
        0,
    );
    assert_eq!(plugin.save().unwrap().get(P::Gain), 0.25);
    // Host must forward the processor's state/automation to the separate controller.
    assert_eq!(
        unsafe { plugin.controller.setParamNormalized(P::Gain as u32, 0.25) },
        kResultOk
    );
    assert_eq!(
        unsafe { plugin.controller.getParamNormalized(P::Gain as u32) },
        0.25
    );
    assert_eq!(plugin.host_events.load(Ordering::Relaxed), 0);
}
#[test]
fn no_audio_parameter_flush_and_text_mapping_are_available_while_stopped() {
    let _serial = SERIAL.lock().unwrap();
    let plugin = Plugin::new();
    let points = changes(&[(P::Gain, vec![(0, 0.5)])]);
    let mut data: ProcessData = unsafe { std::mem::zeroed() };
    data.inputParameterChanges = points.as_ptr();
    assert_eq!(
        plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
        kResultOk
    );
    assert_eq!(plugin.save().unwrap().get(P::Gain), 0.5);
    let mut text = [0; 128];
    assert_eq!(
        unsafe {
            plugin
                .controller
                .getParamStringByValue(P::Volume as u32, 0.5, &mut text)
        },
        kResultOk
    );
    let length = text.iter().position(|v| *v == 0).unwrap();
    assert_eq!(String::from_utf16(&text[..length]).unwrap(), "0.0");
    let mut value = 0.0;
    assert_eq!(
        unsafe {
            plugin
                .controller
                .getParamValueByString(P::Volume as u32, text.as_mut_ptr(), &mut value)
        },
        kResultOk
    );
    assert_eq!(value, 0.5);
    assert_ne!(
        unsafe {
            plugin
                .controller
                .setParamNormalized(P::Gain as u32, f64::NAN)
        },
        kResultOk
    );
}

#[test]
fn malformed_queues_streams_and_configurations_fail_without_callback_allocations() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    plugin.activate(48000, 32, 0);
    for points in [
        vec![(P::Gain, vec![(0, f64::NAN)])],
        vec![(P::Gain, vec![(-1, 0.5)])],
        vec![(P::Gain, vec![(32, 0.5)])],
        vec![(P::Gain, vec![(0, 1.1)])],
        vec![(P::Gain, vec![(0, 0.1)]), (P::Gain, vec![(0, 0.9)])],
    ] {
        let changes = changes(&points);
        let mut data: ProcessData = unsafe { std::mem::zeroed() };
        data.inputParameterChanges = changes.as_ptr();
        assert_ne!(
            plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
            kResultOk
        );
    }
    let mut data: ProcessData = unsafe { std::mem::zeroed() };
    data.numSamples = 33;
    assert_ne!(
        plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
        kResultOk
    );
    data.numSamples = 0;
    data.symbolicSampleSize = 1;
    assert_ne!(
        plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
        kResultOk
    );
    let before = plugin.save().unwrap();
    let stream = ComWrapper::new(Stream {
        fail: true,
        ..Stream::new(before.encode().to_vec(), 1)
    });
    let pointer = stream.to_com_ptr::<IBStream>().unwrap();
    assert_ne!(
        unsafe { plugin.component.setState(pointer.as_ptr()) },
        kResultOk
    );
    assert_eq!(plugin.save().unwrap(), before);
    let stream = ComWrapper::new(Stream::new(vec![], 0));
    let pointer = stream.to_com_ptr::<IBStream>().unwrap();
    assert_ne!(
        unsafe { plugin.component.getState(pointer.as_ptr()) },
        kResultOk
    );
}

#[test]
fn active_recall_and_state_capture_are_coherent_across_ui_and_audio_threads() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut setup = ProcessSetup {
        processMode: 0,
        symbolicSampleSize: 0,
        maxSamplesPerBlock: 32,
        sampleRate: 48000.0,
    };
    assert_eq!(
        unsafe { plugin.processor.setupProcessing(&mut setup) },
        kResultOk
    );
    for direction in [0, 1] {
        assert_eq!(
            unsafe { plugin.component.activateBus(0, direction, 0, 1) },
            kResultOk
        );
    }
    assert_eq!(unsafe { plugin.component.setActive(1) }, kResultOk);
    plugin.active = true;
    let pointer = plugin.processor.as_ptr() as usize;
    let counts = plugin.counts;
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = done.clone();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let ready = barrier.clone();
    let worker = std::thread::spawn(move || {
        let processor = unsafe {
            daw_plugin_vst3::sys::ComRef::from_raw(pointer as *mut IAudioProcessor).unwrap()
        };
        assert_eq!(unsafe { processor.setProcessing(1) }, kResultOk);
        let mut left = [0.25_f32; 32];
        let mut right = [-0.125_f32; 32];
        let mut pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
        let mut buffer = AudioBusBuffers {
            numChannels: 2,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: pointers.as_mut_ptr(),
            },
        };
        let mut data: ProcessData = unsafe { std::mem::zeroed() };
        data.numSamples = 32;
        data.numInputs = 1;
        data.numOutputs = 1;
        data.inputs = &mut buffer;
        data.outputs = &mut buffer;
        let mut blocks = 0;
        ready.wait();
        while blocks == 0 || !stop.load(Ordering::Acquire) {
            if let Some((begin, _)) = counts {
                unsafe {
                    begin();
                }
            }
            let status = unsafe { processor.process(&mut data) };
            if let Some((_, end)) = counts {
                let (mut a, mut d) = (0, 0);
                unsafe {
                    end(&mut a, &mut d);
                }
                assert_eq!((a, d), (0, 0));
            }
            assert_eq!(status, kResultOk);
            blocks += 1;
        }
        assert_eq!(unsafe { processor.setProcessing(0) }, kResultOk);
        blocks
    });
    barrier.wait();
    let mut last = PluginState::default();
    let mut saved = 0;
    for index in 0..1000 {
        let value = index as f64 / 1000.0;
        for parameter in [P::Gain, P::Tone, P::ColorStrength] {
            last.set(parameter, value).unwrap();
        }
        last.set(P::ColorFrequency, 250.0 + 1750.0 * value).unwrap();
        plugin.load(last);
        if let Some(state) = plugin.save() {
            assert_eq!(state, last);
            saved += 1;
        }
    }
    done.store(true, Ordering::Release);
    assert!(worker.join().unwrap() > 0);
    assert!(saved > 0);
    assert_eq!(plugin.save().unwrap(), last);
}

#[test]
fn inactive_buses_may_be_omitted_and_reactivation_clears_dsp_history() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut setup = ProcessSetup {
        processMode: 0,
        symbolicSampleSize: 0,
        maxSamplesPerBlock: 32,
        sampleRate: 48000.0,
    };
    assert_eq!(
        unsafe { plugin.processor.setupProcessing(&mut setup) },
        kResultOk
    );
    assert_eq!(unsafe { plugin.component.setActive(1) }, kResultOk);
    plugin.active = true;
    assert_eq!(unsafe { plugin.processor.setProcessing(1) }, kResultOk);
    plugin.processing = true;
    let mut data: ProcessData = unsafe { std::mem::zeroed() };
    data.numSamples = 32;
    assert_eq!(
        plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
        kResultOk
    );
    plugin.deactivate();
    plugin.activate(48000, 32, 0);
    let left = vec![0.25; 32];
    let right = vec![-0.125; 32];
    let first = run(
        &plugin,
        &mut left.clone(),
        &mut right.clone(),
        &[],
        0,
        true,
        0,
    );
    plugin.deactivate();
    plugin.activate(48000, 32, 0);
    let second = run(
        &plugin,
        &mut left.clone(),
        &mut right.clone(),
        &[],
        0,
        true,
        0,
    );
    assert_samples(&first, &second);
}
