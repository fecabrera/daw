use super::*;
use daw_drive_plugin::IDENTITY;
use daw_plugin_auv2::buffers::StereoList;
use std::ptr;
struct Unit(*mut c_void);
impl Unit {
    fn new() -> Self {
        let desc = AudioComponentDescription {
            componentType: daw_drive_plugin::AU_TYPE,
            componentSubType: daw_drive_plugin::AU_SUBTYPE,
            componentManufacturer: daw_drive_plugin::AU_MANUFACTURER,
            componentFlags: 0,
            componentFlagsMask: 0,
        };
        let pointer = unsafe { DAWAudioUnitFactory(&desc) };
        assert!(!pointer.is_null());
        assert_eq!(unsafe { open(pointer.cast(), 1usize as AudioUnit) }, 0);
        Self(pointer.cast())
    }
    fn set<T>(&self, id: u32, scope: u32, value: &T) -> i32 {
        unsafe {
            properties::set(
                self.0,
                id,
                scope,
                0,
                (value as *const T).cast(),
                std::mem::size_of::<T>() as u32,
            )
        }
    }
    fn get<T: Copy>(&self, id: u32, scope: u32) -> T {
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        let mut size = std::mem::size_of::<T>() as u32;
        assert_eq!(
            unsafe { properties::get(self.0, id, scope, 0, value.as_mut_ptr().cast(), &mut size) },
            0
        );
        unsafe { value.assume_init() }
    }
    fn render(&self, start: f64, left: &mut [f32], right: &mut [f32]) -> i32 {
        let mut list = StereoList::new(left.as_mut_ptr(), right.as_mut_ptr(), left.len() as u32);
        let mut stamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
        stamp.mFlags = kAudioTimeStampSampleTimeValid;
        stamp.mSampleTime = start;
        let mut flags = 0;
        unsafe {
            render::render(
                self.0,
                &mut flags,
                &stamp,
                0,
                left.len() as u32,
                list.as_mut_ptr(),
            )
        }
    }
}
impl Drop for Unit {
    fn drop(&mut self) {
        assert_eq!(unsafe { close(self.0) }, 0);
    }
}
unsafe extern "C" fn input(
    _: *mut c_void,
    _: *mut u32,
    _: *const AudioTimeStamp,
    _: u32,
    n: u32,
    list: *mut AudioBufferList,
) -> i32 {
    let Some(channels) = (unsafe { daw_plugin_auv2::buffers::stereo(list, n) }) else {
        return BAD;
    };
    for channel in channels {
        for frame in 0..n as usize {
            unsafe {
                channel.add(frame).write(0.1);
            }
        }
    }
    0
}
unsafe extern "C" {
    fn daw_au_invoke_reset(p: *mut AudioComponentPlugInInterface, scope: u32, element: u32) -> i32;
}
#[test]
fn factory_lifecycle_properties_and_c_selector_dispatch() {
    let unit = Unit::new();
    let i = unsafe { instance(unit.0).unwrap() };
    assert!(i.interface.reserved.is_null());
    assert!(unsafe { lookup(i16::MAX) }.is_none());
    assert_eq!(
        unit.get::<f64>(kAudioUnitProperty_SampleRate, kAudioUnitScope_Input),
        48000.0
    );
    assert_eq!(
        unit.get::<u32>(kAudioUnitProperty_ElementCount, kAudioUnitScope_Output),
        1
    );
    assert_eq!(
        unit.set(
            kAudioUnitProperty_MaximumFramesPerSlice,
            kAudioUnitScope_Global,
            &(MAX_FRAMES + 1)
        ),
        BAD
    );
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SampleRate,
            kAudioUnitScope_Input,
            &12345.0
        ),
        kAudioUnitErr_FormatNotSupported
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SampleRate,
            kAudioUnitScope_Input,
            &44100.0
        ),
        kAudioUnitErr_Initialized
    );
    assert_eq!(
        unsafe { daw_au_invoke_reset(unit.0.cast(), kAudioUnitScope_Global, 0) },
        0
    );
    assert_eq!(
        unsafe { daw_au_invoke_reset(unit.0.cast(), kAudioUnitScope_Global, 1) },
        kAudioUnitErr_InvalidScope
    );
    assert_eq!(unsafe { uninitialize(unit.0) }, 0);
    assert_eq!(unsafe { uninitialize(unit.0) }, 0);
}
#[test]
fn pull_output_matches_existing_processor_at_all_rates_and_cached_requests() {
    for rate in daw_drive_plugin::SUPPORTED_RATES {
        let unit = Unit::new();
        assert_eq!(
            unit.set(
                kAudioUnitProperty_SampleRate,
                kAudioUnitScope_Input,
                &f64::from(rate)
            ),
            0
        );
        assert_eq!(
            unit.set(
                kAudioUnitProperty_SetRenderCallback,
                kAudioUnitScope_Input,
                &AURenderCallbackStruct {
                    inputProc: Some(input),
                    inputProcRefCon: ptr::null_mut()
                }
            ),
            0
        );
        assert_eq!(unsafe { initialize(unit.0) }, 0);
        let mut reference =
            Processor::new(f64::from(rate), 1, 4096, PluginState::default()).unwrap();
        let mut start = 0.0;
        for frames in [0, 1, 17, 511, 1024] {
            let mut left = vec![0.0; frames];
            let mut right = vec![0.0; frames];
            assert_eq!(unit.render(start, &mut left, &mut right), 0);
            for n in 0..frames {
                let expected = reference.process_sample([0.1; 2]);
                assert_eq!([left[n], right[n]], expected, "rate {rate}, frame {n}");
            }
            if frames > 0 {
                let saved = left.clone();
                left.fill(0.0);
                assert_eq!(unit.render(start, &mut left, &mut right), 0);
                assert_eq!(left, saved);
            }
            start += frames as f64;
        }
    }
}
#[test]
fn state_roundtrip_rejects_foreign_or_corrupt_data_and_preserves_parameters() {
    let unit = Unit::new();
    let p = Parameter::ALL
        .into_iter()
        .find(|p| p.info().steps == 0)
        .unwrap_or(Parameter::Bypass);
    let info = p.info();
    let value = if info.steps == 0 {
        (info.minimum + (info.maximum - info.minimum) * 0.37) as f32
    } else {
        info.maximum as f32
    };
    assert_eq!(
        unsafe { set_parameter(unit.0, p as u32, 0, 0, value, 0) },
        0
    );
    let state: CFPropertyListRef = unit.get(kAudioUnitProperty_ClassInfo, 0);
    assert_eq!(
        unsafe { set_parameter(unit.0, p as u32, 0, 0, info.default as f32, 0) },
        0
    );
    assert_eq!(unit.set(kAudioUnitProperty_ClassInfo, 0, &state), 0);
    let mut actual = 0.0;
    assert_eq!(
        unsafe { get_parameter(unit.0, p as u32, 0, 0, &mut actual) },
        0
    );
    assert_eq!(actual, value);
    assert_eq!(
        unit.set(kAudioUnitProperty_ClassInfo, 0, &ptr::null::<c_void>()),
        BAD
    );
    unsafe {
        CFRelease(state);
    }
}
#[test]
fn schedule_validation_is_atomic_and_capacity_is_bounded() {
    let unit = Unit::new();
    let p = Parameter::ALL[0];
    let event = AudioUnitParameterEvent {
        scope: 0,
        element: 0,
        parameter: p as u32,
        eventType: kParameterEvent_Immediate,
        eventValues: AudioUnitParameterEvent__bindgen_ty_1 {
            immediate: AudioUnitParameterEvent__bindgen_ty_1__bindgen_ty_2 {
                bufferOffset: 0,
                value: 0.0,
            },
        },
    };
    let mut invalid = event;
    invalid.parameter = u32::MAX;
    assert_eq!(
        unsafe { events::schedule(unit.0, [event, invalid].as_ptr(), 2) },
        kAudioUnitErr_InvalidParameter
    );
    let i = unsafe { instance(unit.0).unwrap() };
    let mut collected = events::storage();
    assert!(i.events.take(&mut collected));
    assert!(collected.is_empty());
    let full = vec![event; 65536];
    assert_eq!(
        unsafe { events::schedule(unit.0, full.as_ptr(), full.len() as u32) },
        0
    );
    assert_eq!(unsafe { events::schedule(unit.0, &event, 1) }, BUSY);
    assert!(i.events.take(&mut collected));
    assert_eq!(collected.len(), 65536);
}
#[test]
fn native_audio_component_registration_and_host_calls() {
    unsafe {
        let desc = AudioComponentDescription {
            componentType: daw_drive_plugin::AU_TYPE,
            componentSubType: daw_drive_plugin::AU_SUBTYPE,
            componentManufacturer: daw_drive_plugin::AU_MANUFACTURER,
            componentFlags: 0,
            componentFlagsMask: 0,
        };
        let name = properties::string(&format!("{}: {}", IDENTITY.vendor, IDENTITY.product));
        let component = AudioComponentRegister(
            &desc,
            name,
            daw_drive_plugin::AU_VERSION,
            Some(DAWAudioUnitFactory),
        );
        CFRelease(name.cast());
        assert!(!component.is_null());
        let mut instance = ptr::null_mut();
        assert_eq!(AudioComponentInstanceNew(component, &mut instance), 0);
        assert!(!instance.is_null());
        let mut count = 0u32;
        let mut size = 4;
        assert_eq!(
            AudioUnitGetProperty(
                instance,
                kAudioUnitProperty_ElementCount,
                kAudioUnitScope_Input,
                0,
                (&mut count as *mut u32).cast(),
                &mut size
            ),
            0
        );
        assert_eq!(count, 1);
        assert_eq!(AudioComponentInstanceDispose(instance), 0);
    }
}
#[test]
fn prepared_render_reset_and_dense_automation_do_not_allocate_or_retire() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            kAudioUnitScope_Input,
            &AURenderCallbackStruct {
                inputProc: Some(input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let mut left = [0.0; 37];
    let mut right = [0.0; 37];
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    let events: Vec<_> = Parameter::ALL
        .into_iter()
        .map(|p| AudioUnitParameterEvent {
            scope: 0,
            element: 0,
            parameter: p as u32,
            eventType: kParameterEvent_Immediate,
            eventValues: AudioUnitParameterEvent__bindgen_ty_1 {
                immediate: AudioUnitParameterEvent__bindgen_ty_1__bindgen_ty_2 {
                    bufferOffset: 1,
                    value: p.info().default as f32,
                },
            },
        })
        .collect();
    crate::allocations::daw_auv2_allocation_begin();
    let scheduled = unsafe { events::schedule(unit.0, events.as_ptr(), events.len() as u32) };
    let rendered = unit.render(37.0, &mut left, &mut right);
    let reset = unsafe { super::reset(unit.0, 0, 0) };
    let mut allocations = 0;
    let mut drops = 0;
    unsafe {
        crate::allocations::daw_auv2_allocation_end(&mut allocations, &mut drops);
    }
    assert_eq!((scheduled, rendered, reset), (0, 0, 0));
    assert_eq!((allocations, drops), (0, 0));
}
#[test]
fn parameter_ramps_match_typed_processing_at_sample_offsets() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            kAudioUnitScope_Input,
            &AURenderCallbackStruct {
                inputProc: Some(input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let Some(p) = Parameter::ALL.into_iter().find(|p| p.info().steps == 0) else {
        let event = AudioUnitParameterEvent {
            scope: 0,
            element: 0,
            parameter: Parameter::Bypass as u32,
            eventType: kParameterEvent_Ramped,
            eventValues: AudioUnitParameterEvent__bindgen_ty_1 {
                ramp: AudioUnitParameterEvent__bindgen_ty_1__bindgen_ty_1 {
                    startBufferOffset: 0,
                    durationInFrames: 20,
                    startValue: 0.0,
                    endValue: 1.0,
                },
            },
        };
        assert_eq!(
            unsafe { events::schedule(unit.0, &event, 1) },
            kAudioUnitErr_InvalidParameterValue
        );
        return;
    };
    let metadata = p.info();
    let from = (metadata.minimum + (metadata.maximum - metadata.minimum) * 0.3) as f32;
    let to = (metadata.minimum + (metadata.maximum - metadata.minimum) * 0.6) as f32;
    let event = AudioUnitParameterEvent {
        scope: 0,
        element: 0,
        parameter: p as u32,
        eventType: kParameterEvent_Ramped,
        eventValues: AudioUnitParameterEvent__bindgen_ty_1 {
            ramp: AudioUnitParameterEvent__bindgen_ty_1__bindgen_ty_1 {
                startBufferOffset: -3,
                durationInFrames: 20,
                startValue: from,
                endValue: to,
            },
        },
    };
    assert_eq!(unsafe { events::schedule(unit.0, &event, 1) }, 0);
    let mut left = [0.0; 37];
    let mut right = [0.0; 37];
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    let mut state = PluginState::default();
    let mut reference = Processor::new(48000.0, 1, 4096, state).unwrap();
    for n in 0..37 {
        let fraction = ((n + 3) as f64 / 20.0).min(1.0);
        state
            .set(
                p,
                f64::from(from) + (f64::from(to) - f64::from(from)) * fraction,
            )
            .unwrap();
        reference.apply_parameter(p, &state);
        let expected = reference.process_sample([0.1; 2]);
        assert_eq!([left[n], right[n]], expected);
    }
}
#[test]
fn unexpected_render_fault_is_silent_contained_and_requires_reinitialization() {
    if std::env::var_os("DAW_AU_FAULT_CHILD").is_none() {
        let result=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","native::tests::unexpected_render_fault_is_silent_contained_and_requires_reinitialization","--nocapture"]).env("DAW_AU_FAULT_CHILD","1").output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!String::from_utf8_lossy(&result.stderr).contains("AU contained fault sentinel"));
        return;
    }
    let unit = Unit::new();
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let i = unsafe { instance(unit.0).unwrap() };
    i.inject_fault.store(true, Ordering::Release);
    let mut left = [1.0; 17];
    let mut right = [1.0; 17];
    assert_eq!(unit.render(0.0, &mut left, &mut right), BUSY);
    assert_eq!(left, [0.0; 17]);
    assert_eq!(right, [0.0; 17]);
    assert_eq!(unit.render(17.0, &mut left, &mut right), BUSY);
    assert_eq!(unsafe { uninitialize(unit.0) }, 0);
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
}
struct NotifyState {
    unit: *mut c_void,
    pre: AtomicUsize,
    post: AtomicUsize,
    pre_result: AtomicI32,
    post_result: AtomicI32,
}
unsafe extern "C" fn notification(
    data: *mut c_void,
    flags: *mut u32,
    _: *const AudioTimeStamp,
    _: u32,
    _: u32,
    _: *mut AudioBufferList,
) -> i32 {
    let state = unsafe { &*data.cast::<NotifyState>() };
    let event = AudioUnitParameterEvent {
        scope: 0,
        element: 0,
        parameter: Parameter::Bypass as u32,
        eventType: kParameterEvent_Immediate,
        eventValues: AudioUnitParameterEvent__bindgen_ty_1 {
            immediate: AudioUnitParameterEvent__bindgen_ty_1__bindgen_ty_2 {
                bufferOffset: 0,
                value: 1.0,
            },
        },
    };
    if unsafe { *flags } & kAudioUnitRenderAction_PreRender != 0 {
        state.pre.fetch_add(1, Ordering::Relaxed);
        state.pre_result.store(
            unsafe { events::schedule(state.unit, &event, 1) },
            Ordering::Relaxed,
        );
    }
    if unsafe { *flags } & kAudioUnitRenderAction_PostRender != 0 {
        state.post.fetch_add(1, Ordering::Relaxed);
        state.post_result.store(
            unsafe { events::schedule(state.unit, &event, 1) },
            Ordering::Relaxed,
        );
    }
    0
}
#[test]
fn pre_render_scheduling_applies_to_this_block_and_post_render_does_not_leak_events() {
    let unit = Unit::new();
    let mut state = NotifyState {
        unit: unit.0,
        pre: AtomicUsize::new(0),
        post: AtomicUsize::new(0),
        pre_result: AtomicI32::new(-1),
        post_result: AtomicI32::new(-1),
    };
    assert_eq!(
        unsafe {
            render::add_notify(
                unit.0,
                Some(notification),
                (&mut state as *mut NotifyState).cast(),
            )
        },
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let mut left = [0.0; 17];
    let mut right = [0.0; 17];
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    assert_eq!(state.pre_result.load(Ordering::Relaxed), 0);
    assert_eq!(state.post_result.load(Ordering::Relaxed), BUSY);
    let mut value = 0.0;
    assert_eq!(
        unsafe { get_parameter(unit.0, Parameter::Bypass as u32, 0, 0, &mut value) },
        0
    );
    assert_eq!(value, 1.0);
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    assert_eq!(state.pre.load(Ordering::Relaxed), 1);
    assert_eq!(state.post.load(Ordering::Relaxed), 1);
    assert_eq!(
        unsafe {
            render::remove_notify(
                unit.0,
                Some(notification),
                (&mut state as *mut NotifyState).cast(),
            )
        },
        0
    );
}
unsafe extern "C" fn failing_input(
    _: *mut c_void,
    _: *mut u32,
    _: *const AudioTimeStamp,
    _: u32,
    _: u32,
    _: *mut AudioBufferList,
) -> i32 {
    -1234
}
#[test]
fn upstream_failure_silences_validated_outputs_and_reports_native_error() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            1,
            &AURenderCallbackStruct {
                inputProc: Some(failing_input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let mut left = [1.0; 17];
    let mut right = [1.0; 17];
    assert_eq!(unit.render(0.0, &mut left, &mut right), -1234);
    assert_eq!(left, [0.0; 17]);
    assert_eq!(right, [0.0; 17]);
    assert_eq!(
        unit.get::<i32>(kAudioUnitProperty_LastRenderError, 0),
        -1234
    );
    assert_eq!(unit.get::<i32>(kAudioUnitProperty_LastRenderError, 0), 0);
}
#[test]
fn close_cannot_retire_a_native_handle_during_view_notifications() {
    let unit = Unit::new();
    let shared = unsafe { instance(unit.0).unwrap() }.shared.clone();
    let lease = token(&shared.host_gate).unwrap();
    assert_eq!(unsafe { close(unit.0) }, BUSY);
    assert!(!shared.closed.load(Ordering::Acquire));
    assert_ne!(shared.handle.load(Ordering::Acquire), 0);
    drop(lease);
    drop(unit);
    assert!(shared.closed.load(Ordering::Acquire));
    assert_eq!(shared.handle.load(Ordering::Acquire), 0);
}
#[test]
fn active_recall_applies_a_complete_state_at_the_next_boundary_without_allocation() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            1,
            &AURenderCallbackStruct {
                inputProc: Some(input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let preset: CFPropertyListRef = unit.get(kAudioUnitProperty_ClassInfo, 0);
    let p = Parameter::ALL
        .into_iter()
        .find(|p| p.info().steps == 0)
        .unwrap_or(Parameter::Bypass);
    assert_eq!(
        unsafe { set_parameter(unit.0, p as u32, 0, 0, p.info().maximum as f32, 0) },
        0
    );
    let mut left = [0.0; 37];
    let mut right = [0.0; 37];
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    assert_eq!(unit.set(kAudioUnitProperty_ClassInfo, 0, &preset), 0);
    unsafe {
        CFRelease(preset);
    }
    crate::allocations::daw_auv2_allocation_begin();
    let rendered = unit.render(37.0, &mut left, &mut right);
    let mut allocations = 0;
    let mut retired = 0;
    unsafe {
        crate::allocations::daw_auv2_allocation_end(&mut allocations, &mut retired);
    }
    assert_eq!(rendered, 0);
    assert_eq!((allocations, retired), (0, 0));
    let i = unsafe { instance(unit.0).unwrap() };
    let engine = unsafe { (&*i.engine.get()).as_ref().unwrap() };
    assert_eq!(engine.state, PluginState::default());
}
#[test]
fn named_user_presets_keep_their_metadata_through_class_info_recall() {
    let unit = Unit::new();
    let name = unsafe { properties::string("User preset") };
    assert_eq!(
        unit.set(
            kAudioUnitProperty_PresentPreset,
            0,
            &AUPreset {
                presetNumber: -1,
                presetName: name
            }
        ),
        0
    );
    unsafe {
        CFRelease(name.cast());
    }
    let preset: CFPropertyListRef = unit.get(kAudioUnitProperty_ClassInfo, 0);
    let default_name = unsafe { properties::string("Default") };
    assert_eq!(
        unit.set(
            kAudioUnitProperty_PresentPreset,
            0,
            &AUPreset {
                presetNumber: 0,
                presetName: default_name
            }
        ),
        0
    );
    unsafe {
        CFRelease(default_name.cast());
    }
    assert_eq!(unit.set(kAudioUnitProperty_ClassInfo, 0, &preset), 0);
    unsafe {
        CFRelease(preset);
    }
    let current: AUPreset = unit.get(kAudioUnitProperty_PresentPreset, 0);
    assert_eq!(current.presetNumber, -1);
    let mut name = [0i8; 64];
    assert_ne!(
        unsafe {
            CFStringGetCString(
                current.presetName,
                name.as_mut_ptr(),
                64,
                kCFStringEncodingUTF8,
            )
        },
        0
    );
    assert_eq!(
        unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
            .to_str()
            .unwrap(),
        "User preset"
    );
    unsafe {
        CFRelease(current.presetName.cast());
    }
}
#[test]
fn parameter_updates_during_preparation_are_not_mistaken_for_applied_state() {
    let unit = Unit::new();
    let i = unsafe { instance(unit.0).unwrap() };
    let barrier = Arc::new(std::sync::Barrier::new(2));
    unsafe {
        *i.prepare_hook.get() = Some(barrier.clone());
    }
    let address = unit.0 as usize;
    let worker = std::thread::spawn(move || unsafe { initialize(address as *mut c_void) });
    barrier.wait();
    let p = Parameter::ALL[0];
    let value = if p.info().default == p.info().maximum {
        p.info().minimum
    } else {
        p.info().maximum
    };
    assert_eq!(
        unsafe { set_parameter(unit.0, p as u32, 0, 0, value as f32, 0) },
        0
    );
    barrier.wait();
    assert_eq!(worker.join().unwrap(), 0);
    unsafe {
        *i.prepare_hook.get() = None;
    }
    let mut left = [0.0; 17];
    let mut right = [0.0; 17];
    assert_eq!(unit.render(0.0, &mut left, &mut right), 0);
    assert_eq!(
        unsafe { (&*i.engine.get()).as_ref().unwrap() }.state.get(p),
        value
    );
}
#[test]
fn input_callback_can_be_attached_after_initialize_without_replacing_dsp() {
    let unit = Unit::new();
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let i = unsafe { instance(unit.0).unwrap() };
    let address = unsafe { (&*i.engine.get()).as_ref().unwrap() } as *const Engine;
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            1,
            &AURenderCallbackStruct {
                inputProc: Some(input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    let mut left = [0.0; 511];
    let mut right = [0.0; 511];
    let mut audible = false;
    for block in 0..16 {
        assert_eq!(unit.render((block * 511) as f64, &mut left, &mut right), 0);
        audible |= left.iter().chain(&right).any(|s| s.abs() > 1e-6);
    }
    assert!(audible, "active effect must receive its host input");
    assert_eq!(
        unsafe { (&*i.engine.get()).as_ref().unwrap() } as *const Engine,
        address
    );
}
struct BorrowedInput {
    left: [f32; 37],
    right: [f32; 37],
}
unsafe extern "C" fn borrowed_input(
    data: *mut c_void,
    _: *mut u32,
    _: *const AudioTimeStamp,
    _: u32,
    frames: u32,
    list: *mut AudioBufferList,
) -> i32 {
    let input = unsafe { &mut *data.cast::<BorrowedInput>() };
    if frames > 37 {
        return BAD;
    }
    let planes = unsafe {
        list.cast::<u8>()
            .add(std::mem::offset_of!(AudioBufferList, mBuffers))
            .cast::<AudioBuffer>()
    };
    for (index, data) in [input.left.as_mut_ptr(), input.right.as_mut_ptr()]
        .into_iter()
        .enumerate()
    {
        unsafe {
            planes.add(index).write(AudioBuffer {
                mNumberChannels: 1,
                mDataByteSize: frames * 4,
                mData: data.cast(),
            });
        }
    }
    0
}
#[test]
fn input_allocation_does_not_disable_unit_owned_output_and_accepts_borrowed_input() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(kAudioUnitProperty_ShouldAllocateBuffer, 1, &0u32),
        0
    );
    assert_eq!(
        unit.get::<u32>(kAudioUnitProperty_ShouldAllocateBuffer, 1),
        0
    );
    assert_eq!(
        unit.get::<u32>(kAudioUnitProperty_ShouldAllocateBuffer, 2),
        1
    );
    let mut input = BorrowedInput {
        left: [0.1; 37],
        right: [0.1; 37],
    };
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            1,
            &AURenderCallbackStruct {
                inputProc: Some(borrowed_input),
                inputProcRefCon: (&mut input as *mut BorrowedInput).cast()
            }
        ),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    let mut list = StereoList::new(ptr::null_mut(), ptr::null_mut(), 37);
    let mut stamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
    stamp.mFlags = kAudioTimeStampSampleTimeValid;
    let mut flags = 0;
    assert_eq!(
        unsafe { render::render(unit.0, &mut flags, &stamp, 0, 37, list.as_mut_ptr()) },
        0
    );
    assert!(unsafe { daw_plugin_auv2::buffers::stereo(list.as_mut_ptr(), 37) }.is_some());
}
#[test]
fn disabled_input_allocation_reuses_host_output_planes_for_a_callback_pull() {
    let unit = Unit::new();
    assert_eq!(
        unit.set(kAudioUnitProperty_ShouldAllocateBuffer, 1, &0u32),
        0
    );
    assert_eq!(
        unit.set(kAudioUnitProperty_ShouldAllocateBuffer, 2, &0u32),
        0
    );
    assert_eq!(unsafe { initialize(unit.0) }, 0);
    assert_eq!(
        unit.set(
            kAudioUnitProperty_SetRenderCallback,
            1,
            &AURenderCallbackStruct {
                inputProc: Some(input),
                inputProcRefCon: ptr::null_mut()
            }
        ),
        0
    );
    let mut left = [0.0; 511];
    let mut right = [0.0; 511];
    let mut audible = false;
    for block in 0..16 {
        assert_eq!(unit.render((block * 511) as f64, &mut left, &mut right), 0);
        audible |= left.iter().chain(&right).any(|s| s.abs() > 1e-6);
    }
    assert!(
        audible,
        "host-owned in-place buffers must carry active audio"
    );
}
#[test]
fn au_indexed_parameters_quantize_fractional_host_values() {
    let unit = Unit::new();
    for p in Parameter::ALL {
        let info = p.info();
        if info.steps == 0 {
            continue;
        }
        let requested = (info.minimum + (info.maximum - info.minimum) * 0.37) as f32;
        assert_eq!(
            unsafe { set_parameter(unit.0, p as u32, 0, 0, requested, 0) },
            0
        );
        let mut actual = 0.0;
        assert_eq!(
            unsafe { get_parameter(unit.0, p as u32, 0, 0, &mut actual) },
            0
        );
        assert_eq!(actual, requested.round());
    }
}
