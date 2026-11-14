use super::*;
use std::{mem::size_of, ptr};
fn format(rate: f64) -> AudioStreamBasicDescription {
    AudioStreamBasicDescription {
        mSampleRate: rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat
            | kAudioFormatFlagIsPacked
            | kAudioFormatFlagIsNonInterleaved
            | kAudioFormatFlagsNativeEndian,
        mBytesPerPacket: 4,
        mFramesPerPacket: 1,
        mBytesPerFrame: 4,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    }
}
fn descriptor(id: u32, scope: u32, element: u32) -> Result<(usize, bool), OSStatus> {
    if element != 0
        && ![
            kAudioUnitProperty_ParameterInfo,
            kAudioUnitProperty_ParameterValueStrings,
        ]
        .contains(&id)
    {
        return Err(kAudioUnitErr_InvalidElement);
    }
    if ![
        kAudioUnitScope_Global,
        kAudioUnitScope_Input,
        kAudioUnitScope_Output,
    ]
    .contains(&scope)
    {
        return Err(kAudioUnitErr_InvalidScope);
    }
    let global = scope == kAudioUnitScope_Global;
    let io = scope == kAudioUnitScope_Input || scope == kAudioUnitScope_Output;
    let value = match id {
        kAudioUnitProperty_StreamFormat if io => (size_of::<AudioStreamBasicDescription>(), true),
        kAudioUnitProperty_SampleRate if io || global => (8, true),
        kAudioUnitProperty_AudioChannelLayout if io => (12, true),
        kAudioUnitProperty_SupportedChannelLayoutTags if io => (4, false),
        kAudioUnitProperty_ElementCount if io => (4, false),
        kAudioUnitProperty_ElementName if io => (size_of::<CFStringRef>(), false),
        kAudioUnitProperty_ShouldAllocateBuffer if io => (4, true),
        kAudioUnitProperty_MakeConnection if scope == kAudioUnitScope_Input => {
            (size_of::<AudioUnitConnection>(), true)
        }
        kAudioUnitProperty_SetRenderCallback if scope == kAudioUnitScope_Input => {
            (size_of::<AURenderCallbackStruct>(), true)
        }
        kAudioUnitProperty_SupportedNumChannels if global => (size_of::<AUChannelInfo>(), false),
        kAudioUnitProperty_ParameterList if global => (Parameter::COUNT * 4, false),
        kAudioUnitProperty_ParameterInfo if global && Parameter::from_id(element).is_ok() => {
            (size_of::<AudioUnitParameterInfo>(), false)
        }
        kAudioUnitProperty_ParameterValueStrings
            if global && Parameter::from_id(element).is_ok_and(|p| p.info().steps > 0) =>
        {
            (size_of::<CFArrayRef>(), false)
        }
        kAudioUnitProperty_ParameterStringFromValue if global => {
            (size_of::<AudioUnitParameterStringFromValue>(), false)
        }
        kAudioUnitProperty_ParameterValueFromString if global => {
            (size_of::<AudioUnitParameterValueFromString>(), false)
        }
        kAudioUnitProperty_ParameterIDName if global => {
            (size_of::<AudioUnitParameterNameInfo>(), false)
        }
        kAudioUnitProperty_ClassInfo if global => (size_of::<CFPropertyListRef>(), true),
        kAudioUnitProperty_PresentPreset | kAudioUnitProperty_CurrentPreset if global => {
            (size_of::<AUPreset>(), true)
        }
        kAudioUnitProperty_FactoryPresets if global => (size_of::<CFArrayRef>(), false),
        kAudioUnitProperty_CocoaUI if global => (size_of::<AudioUnitCocoaViewInfo>(), false),
        kAudioUnitProperty_HostCallbacks if global => (size_of::<HostCallbackInfo>(), true),
        kAudioUnitProperty_Latency | kAudioUnitProperty_TailTime if global => (8, false),
        kAudioUnitProperty_MaximumFramesPerSlice
        | kAudioUnitProperty_BypassEffect
        | kAudioUnitProperty_RenderQuality
        | kAudioUnitProperty_OfflineRender
            if global =>
        {
            (4, true)
        }
        kAudioUnitProperty_LastRenderError if global => (4, false),
        PRIVATE_MODEL if global => (size_of::<usize>(), false),
        _ => return Err(kAudioUnitErr_InvalidProperty),
    };
    Ok(value)
}
pub(super) unsafe extern "C" fn info(
    p: *mut c_void,
    id: u32,
    scope: u32,
    element: u32,
    size: *mut u32,
    writable: *mut u8,
) -> OSStatus {
    boundary::call(BUSY, || {
        if unsafe { instance(p) }.is_none() {
            return BAD;
        }
        match descriptor(id, scope, element) {
            Ok((n, w)) => {
                if !size.is_null() {
                    unsafe {
                        size.write(n as u32);
                    }
                }
                if !writable.is_null() {
                    unsafe {
                        writable.write(u8::from(w));
                    }
                }
                0
            }
            Err(e) => e,
        }
    })
}
unsafe fn write<T: Copy>(out: *mut c_void, value: T) {
    unsafe {
        out.cast::<T>().write_unaligned(value);
    }
}
unsafe fn read<T: Copy>(input: *const c_void) -> T {
    unsafe { input.cast::<T>().read_unaligned() }
}
pub(super) unsafe extern "C" fn get(
    p: *mut c_void,
    id: u32,
    scope: u32,
    element: u32,
    out: *mut c_void,
    size: *mut u32,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let (needed, _) = match descriptor(id, scope, element) {
            Ok(v) => v,
            Err(e) => return e,
        };
        if out.is_null() || size.is_null() {
            return BAD;
        }
        if (unsafe { *size } as usize) < needed {
            unsafe {
                *size = needed as u32;
            }
            return kAudioUnitErr_InvalidPropertyValue;
        }
        if id == kAudioUnitProperty_BypassEffect {
            unsafe {
                *size = 4;
                write(out, i.shared.parameter(Parameter::Bypass) as u32);
            }
            return 0;
        }
        if id == kAudioUnitProperty_LastRenderError {
            unsafe {
                *size = 4;
                write(out, i.last_error.swap(0, Ordering::AcqRel));
            }
            return 0;
        }
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        let c = unsafe { &*i.config.get() };
        unsafe {
            *size = needed as u32;
        }
        unsafe {
            match id {
                kAudioUnitProperty_StreamFormat => write(out, format(c.rate)),
                kAudioUnitProperty_SampleRate => write(out, c.rate),
                kAudioUnitProperty_AudioChannelLayout => {
                    write(out, [kAudioChannelLayoutTag_Stereo, 0u32, 0u32])
                }
                kAudioUnitProperty_SupportedChannelLayoutTags => {
                    write(out, kAudioChannelLayoutTag_Stereo)
                }
                kAudioUnitProperty_ElementCount => write(out, 1u32),
                kAudioUnitProperty_ElementName => write(
                    out,
                    string(if scope == kAudioUnitScope_Input {
                        "Input"
                    } else {
                        "Output"
                    }),
                ),
                kAudioUnitProperty_ShouldAllocateBuffer => write(
                    out,
                    u32::from(if scope == kAudioUnitScope_Input {
                        c.allocate_input
                    } else {
                        c.allocate_output
                    }),
                ),
                kAudioUnitProperty_MakeConnection => write(out, c.connection),
                kAudioUnitProperty_SetRenderCallback => write(out, c.input),
                kAudioUnitProperty_SupportedNumChannels => write(
                    out,
                    AUChannelInfo {
                        inChannels: 2,
                        outChannels: 2,
                    },
                ),
                kAudioUnitProperty_ParameterList => {
                    for (slot, p) in Parameter::ALL.into_iter().enumerate() {
                        out.cast::<u32>().add(slot).write_unaligned(p as u32);
                    }
                }
                kAudioUnitProperty_ParameterInfo => {
                    let param = Parameter::from_id(element).unwrap();
                    let metadata = param.info();
                    let mut value: AudioUnitParameterInfo = std::mem::zeroed();
                    daw_plugin_support::write_text(metadata.name, &mut value.name);
                    value.cfNameString = string(metadata.name);
                    value.minValue = metadata.minimum as f32;
                    value.maxValue = metadata.maximum as f32;
                    value.defaultValue = metadata.default as f32;
                    value.unit = if metadata.steps > 0 {
                        kAudioUnitParameterUnit_Indexed
                    } else {
                        match param.unit() {
                            "dB" => kAudioUnitParameterUnit_Decibels,
                            "Hz" => kAudioUnitParameterUnit_Hertz,
                            "ms" => kAudioUnitParameterUnit_Milliseconds,
                            _ => kAudioUnitParameterUnit_Generic,
                        }
                    };
                    value.flags = kAudioUnitParameterFlag_IsReadable
                        | kAudioUnitParameterFlag_IsWritable
                        | kAudioUnitParameterFlag_HasCFNameString
                        | kAudioUnitParameterFlag_CFNameRelease;
                    if metadata.steps == 0 {
                        value.flags |= kAudioUnitParameterFlag_CanRamp;
                    }
                    if param == Parameter::Bypass {
                        value.flags |= kAudioUnitParameterFlag_IsGlobalMeta;
                    }
                    write(out, value);
                }
                kAudioUnitProperty_ParameterValueStrings => {
                    let param = Parameter::from_id(element).unwrap();
                    let metadata = param.info();
                    let values: Vec<_> = (0..=metadata.steps)
                        .map(|n| {
                            string(
                                &param
                                    .format(metadata.minimum + f64::from(n))
                                    .unwrap_or_default(),
                            )
                        })
                        .collect();
                    let array = CFArrayCreate(
                        ptr::null(),
                        values.as_ptr().cast_mut().cast(),
                        values.len() as _,
                        &kCFTypeArrayCallBacks,
                    );
                    for value in values {
                        CFRelease(value.cast());
                    }
                    write(out, array);
                }
                kAudioUnitProperty_ParameterIDName => {
                    let mut v: AudioUnitParameterNameInfo = read(out);
                    let Ok(p) = Parameter::from_id(v.inID) else {
                        return kAudioUnitErr_InvalidParameter;
                    };
                    v.outName = string(p.info().name);
                    write(out, v);
                }
                kAudioUnitProperty_ParameterStringFromValue => {
                    let mut v: AudioUnitParameterStringFromValue = read(out);
                    let Ok(p) = Parameter::from_id(v.inParamID) else {
                        return kAudioUnitErr_InvalidParameter;
                    };
                    let value = if v.inValue.is_null() {
                        i.shared.parameter(p)
                    } else {
                        let Some(v) = events::plain(p, *v.inValue) else {
                            return BAD;
                        };
                        v
                    };
                    v.outString = string(&p.format(value).unwrap_or_default());
                    write(out, v);
                }
                kAudioUnitProperty_ParameterValueFromString => {
                    let mut v: AudioUnitParameterValueFromString = read(out);
                    let Ok(p) = Parameter::from_id(v.inParamID) else {
                        return kAudioUnitErr_InvalidParameter;
                    };
                    let Some(text) = text(v.inString) else {
                        return BAD;
                    };
                    let Ok(value) = p.parse(&text) else {
                        return BAD;
                    };
                    v.outValue = value as f32;
                    write(out, v);
                }
                kAudioUnitProperty_ClassInfo => {
                    let Some(_control) = token(&i.shared.control) else {
                        return BUSY;
                    };
                    let Some(state) = current(i) else {
                        return BUSY;
                    };
                    write(
                        out,
                        class_info(
                            state,
                            &c.preset_name,
                            if i.shared.preset_dirty.load(Ordering::Acquire) {
                                -1
                            } else {
                                c.preset_number
                            },
                        ),
                    );
                }
                kAudioUnitProperty_CurrentPreset | kAudioUnitProperty_PresentPreset => write(
                    out,
                    AUPreset {
                        presetNumber: if i.shared.preset_dirty.load(Ordering::Acquire) {
                            -1
                        } else {
                            c.preset_number
                        },
                        presetName: string(&c.preset_name),
                    },
                ),
                kAudioUnitProperty_FactoryPresets => {
                    let preset = AUPreset {
                        presetNumber: 0,
                        presetName: string("Default"),
                    };
                    // AU factory preset arrays contain pointers to stable AUPreset records.
                    write(out, default_presets(preset));
                }
                kAudioUnitProperty_CocoaUI => match editor::cocoa_info() {
                    Some(value) => write(out, value),
                    None => return kAudioUnitErr_InvalidProperty,
                },
                kAudioUnitProperty_HostCallbacks => write(out, c.host),
                kAudioUnitProperty_Latency => write(
                    out,
                    f64::from(daw_drive_plugin::latency_frames(c.rate as u32)) / c.rate,
                ),
                kAudioUnitProperty_TailTime => write(
                    out,
                    f64::from(daw_drive_plugin::tail_frames(c.rate as u32)) / c.rate,
                ),
                kAudioUnitProperty_MaximumFramesPerSlice => write(out, c.maximum),
                kAudioUnitProperty_BypassEffect => {
                    write(out, i.shared.parameter(Parameter::Bypass) as u32)
                }
                kAudioUnitProperty_OfflineRender => write(out, u32::from(c.offline)),
                kAudioUnitProperty_RenderQuality => write(out, 127u32),
                kAudioUnitProperty_LastRenderError => {
                    write(out, i.last_error.swap(0, Ordering::AcqRel))
                }
                PRIVATE_MODEL => write(out, Arc::into_raw(i.shared.clone()) as usize),
                _ => return kAudioUnitErr_InvalidProperty,
            }
        }
        0
    })
}
fn current(i: &Instance) -> Option<PluginState> {
    let mut state = i.shared.exchange.current()?;
    for p in Parameter::ALL {
        state.set(p, i.shared.parameter(p)).ok()?;
    }
    Some(state)
}
pub(super) unsafe extern "C" fn set(
    p: *mut c_void,
    id: u32,
    scope: u32,
    element: u32,
    input: *const c_void,
    size: u32,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_host_lease) = token(&i.shared.host_gate) else {
            return BUSY;
        };
        let (needed, writable) = match descriptor(id, scope, element) {
            Ok(v) => v,
            Err(e) => return e,
        };
        if !writable {
            return kAudioUnitErr_PropertyNotWritable;
        }
        let legacy_callbacks = id == kAudioUnitProperty_HostCallbacks
            && size as usize == std::mem::offset_of!(HostCallbackInfo, transportStateProc2);
        if input.is_null() || (size as usize != needed && !legacy_callbacks) {
            return BAD;
        }
        if id == kAudioUnitProperty_BypassEffect {
            let v: u32 = unsafe { read(input) };
            if v > 1 {
                return BAD;
            }
            return i.shared.set(Parameter::Bypass, f64::from(v));
        }
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        let c = unsafe { &mut *i.config.get() };
        let initialized = unsafe { (*i.engine.get()).is_some() };
        unsafe {
            match id {
                kAudioUnitProperty_AudioChannelLayout => {
                    let layout: [u32; 3] = read(input);
                    if layout != [kAudioChannelLayoutTag_Stereo, 0, 0] {
                        return kAudioUnitErr_FormatNotSupported;
                    }
                }
                kAudioUnitProperty_StreamFormat => {
                    if initialized {
                        return kAudioUnitErr_Initialized;
                    }
                    let value: AudioStreamBasicDescription = read(input);
                    let expected = format(value.mSampleRate);
                    if value.mFormatID != expected.mFormatID
                        || value.mFormatFlags != expected.mFormatFlags
                        || value.mChannelsPerFrame != 2
                        || value.mBitsPerChannel != 32
                        || value.mBytesPerFrame != 4
                        || value.mFramesPerPacket != 1
                        || value.mBytesPerPacket != 4
                        || !daw_drive_plugin::SUPPORTED_RATES
                            .iter()
                            .any(|v| f64::from(*v) == value.mSampleRate)
                    {
                        return kAudioUnitErr_FormatNotSupported;
                    }
                    c.rate = value.mSampleRate;
                }
                kAudioUnitProperty_SampleRate => {
                    if initialized {
                        return kAudioUnitErr_Initialized;
                    }
                    let rate: f64 = read(input);
                    if !daw_drive_plugin::SUPPORTED_RATES
                        .iter()
                        .any(|r| f64::from(*r) == rate)
                    {
                        return kAudioUnitErr_FormatNotSupported;
                    }
                    c.rate = rate;
                }
                kAudioUnitProperty_MaximumFramesPerSlice => {
                    if initialized {
                        return kAudioUnitErr_Initialized;
                    }
                    let value: u32 = read(input);
                    if value == 0 || value > MAX_FRAMES {
                        return BAD;
                    }
                    c.maximum = value;
                }
                kAudioUnitProperty_ShouldAllocateBuffer => {
                    if initialized {
                        return kAudioUnitErr_Initialized;
                    }
                    let value: u32 = read(input);
                    if value > 1 {
                        return BAD;
                    }
                    if scope == kAudioUnitScope_Input {
                        c.allocate_input = value != 0;
                    } else {
                        c.allocate_output = value != 0;
                    }
                }
                kAudioUnitProperty_MakeConnection => {
                    let v: AudioUnitConnection = read(input);
                    if v.destInputNumber != 0 {
                        return kAudioUnitErr_InvalidElement;
                    }
                    c.connection = v;
                    c.input = std::mem::zeroed();
                    if let Some(engine) = &mut *i.engine.get() {
                        engine.stamp = None;
                    }
                }
                kAudioUnitProperty_SetRenderCallback => {
                    c.input = read(input);
                    c.connection = std::mem::zeroed();
                    if let Some(engine) = &mut *i.engine.get() {
                        engine.stamp = None;
                    }
                }
                kAudioUnitProperty_HostCallbacks => {
                    c.host = std::mem::zeroed();
                    ptr::copy_nonoverlapping(
                        input.cast::<u8>(),
                        (&mut c.host as *mut HostCallbackInfo).cast(),
                        size as usize,
                    );
                }
                kAudioUnitProperty_OfflineRender => {
                    let v: u32 = read(input);
                    if v > 1 {
                        return BAD;
                    }
                    c.offline = v != 0;
                }
                kAudioUnitProperty_RenderQuality => {
                    if read::<u32>(input) > 127 {
                        return BAD;
                    }
                }
                kAudioUnitProperty_ClassInfo => {
                    let plist: CFPropertyListRef = read(input);
                    let Some(state) = decode_class_info(plist) else {
                        return BAD;
                    };
                    let Some((name, number)) = preset_metadata(plist) else {
                        return BAD;
                    };
                    let Some(_control) = token(&i.shared.control) else {
                        return BUSY;
                    };
                    c.preset_name = name;
                    c.preset_number = number;
                    i.shared.preset_dirty.store(false, Ordering::Release);
                    i.shared.exchange.recall(state);
                    for p in Parameter::ALL {
                        i.shared.values[index(p)].store(state.get(p).to_bits(), Ordering::Release);
                    }
                    i.shared.recall_revision.fetch_add(1, Ordering::Release);
                }
                kAudioUnitProperty_CurrentPreset | kAudioUnitProperty_PresentPreset => {
                    let value: AUPreset = read(input);
                    if value.presetNumber == 0 {
                        let state = PluginState::default();
                        let Some(_control) = token(&i.shared.control) else {
                            return BUSY;
                        };
                        i.shared.exchange.recall(state);
                        i.shared.recall_revision.fetch_add(1, Ordering::Release);
                        for p in Parameter::ALL {
                            i.shared.values[index(p)]
                                .store(state.get(p).to_bits(), Ordering::Release);
                        }
                        c.preset_number = 0;
                        c.preset_name = "Default".into();
                        i.shared.preset_dirty.store(false, Ordering::Release);
                    } else if value.presetNumber < 0 {
                        let Some(name) = text(value.presetName) else {
                            return BAD;
                        };
                        c.preset_number = value.presetNumber;
                        c.preset_name = name;
                        i.shared.preset_dirty.store(false, Ordering::Release);
                    } else {
                        return BAD;
                    }
                }
                _ => return kAudioUnitErr_InvalidProperty,
            }
        }
        let listeners = c.listeners.clone();
        let shared = i.shared.clone();
        drop(_g);
        for (prop, callback, data) in listeners {
            if prop == id
                && let Some(callback) = callback
            {
                unsafe {
                    callback(
                        data,
                        shared.handle.load(Ordering::Acquire) as AudioUnit,
                        id,
                        scope,
                        element,
                    );
                }
            }
        }
        editor::flush_pending_ends(&shared);
        0
    })
}
pub(super) unsafe fn string(s: &str) -> CFStringRef {
    unsafe {
        CFStringCreateWithBytes(
            ptr::null(),
            s.as_ptr(),
            s.len() as _,
            kCFStringEncodingUTF8,
            0,
        )
    }
}
unsafe fn text(s: CFStringRef) -> Option<String> {
    if s.is_null() || unsafe { CFGetTypeID(s.cast()) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    let len =
        unsafe { CFStringGetMaximumSizeForEncoding(CFStringGetLength(s), kCFStringEncodingUTF8) };
    if !(0..=65536).contains(&len) {
        return None;
    }
    let mut bytes = vec![0u8; len as usize + 1];
    if unsafe {
        CFStringGetCString(
            s,
            bytes.as_mut_ptr().cast(),
            bytes.len() as _,
            kCFStringEncodingUTF8,
        )
    } == 0
    {
        return None;
    }
    Some(
        std::ffi::CStr::from_bytes_until_nul(&bytes)
            .ok()?
            .to_str()
            .ok()?
            .into(),
    )
}
unsafe fn class_info(state: PluginState, name: &str, number: i32) -> CFPropertyListRef {
    unsafe {
        let dict = CFDictionaryCreateMutable(
            ptr::null(),
            0,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        for (key, value) in [
            ("version", 0),
            ("type", daw_drive_plugin::AU_TYPE),
            ("subtype", daw_drive_plugin::AU_SUBTYPE),
            ("manufacturer", daw_drive_plugin::AU_MANUFACTURER),
        ] {
            let k = string(key);
            let v = CFNumberCreate(
                ptr::null(),
                kCFNumberSInt64Type as _,
                (&i64::from(value) as *const i64).cast(),
            );
            CFDictionarySetValue(dict, k.cast(), v.cast());
            CFRelease(k.cast());
            CFRelease(v.cast());
        }
        let key = string("name");
        let name = string(name);
        CFDictionarySetValue(dict, key.cast(), name.cast());
        CFRelease(key.cast());
        CFRelease(name.cast());
        let key = string("preset-number");
        let value = i64::from(number);
        let value = CFNumberCreate(
            ptr::null(),
            kCFNumberSInt64Type as _,
            (&value as *const i64).cast(),
        );
        CFDictionarySetValue(dict, key.cast(), value.cast());
        CFRelease(key.cast());
        CFRelease(value.cast());
        let key = string("data");
        let bytes = state.encode();
        let data = CFDataCreate(ptr::null(), bytes.as_ptr(), bytes.len() as _);
        CFDictionarySetValue(dict, key.cast(), data.cast());
        CFRelease(key.cast());
        CFRelease(data.cast());
        dict.cast()
    }
}
unsafe fn decode_class_info(plist: CFPropertyListRef) -> Option<PluginState> {
    unsafe {
        if plist.is_null() || CFGetTypeID(plist) != CFDictionaryGetTypeID() {
            return None;
        }
        for (key, expected) in [
            ("version", 0),
            ("type", daw_drive_plugin::AU_TYPE),
            ("subtype", daw_drive_plugin::AU_SUBTYPE),
            ("manufacturer", daw_drive_plugin::AU_MANUFACTURER),
        ] {
            let key = string(key);
            let value = CFDictionaryGetValue(plist.cast(), key.cast());
            CFRelease(key.cast());
            if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
                return None;
            }
            let mut number = 0i64;
            if CFNumberGetValue(
                value.cast(),
                kCFNumberSInt64Type as _,
                (&mut number as *mut i64).cast(),
            ) == 0
                || number != i64::from(expected)
            {
                return None;
            }
        }
        let key = string("data");
        let value = CFDictionaryGetValue(plist.cast(), key.cast());
        CFRelease(key.cast());
        if value.is_null() || CFGetTypeID(value) != CFDataGetTypeID() {
            return None;
        }
        let len = CFDataGetLength(value.cast());
        if len < 0 || len as usize > daw_drive_plugin::MAX_STATE_BYTES {
            return None;
        }
        PluginState::decode(std::slice::from_raw_parts(
            CFDataGetBytePtr(value.cast()),
            len as usize,
        ))
        .ok()
    }
}
unsafe fn preset_metadata(plist: CFPropertyListRef) -> Option<(String, i32)> {
    unsafe {
        let key = string("name");
        let value = CFDictionaryGetValue(plist.cast(), key.cast());
        CFRelease(key.cast());
        let name = if value.is_null() {
            "User".into()
        } else {
            text(value.cast())?
        };
        let key = string("preset-number");
        let value = CFDictionaryGetValue(plist.cast(), key.cast());
        CFRelease(key.cast());
        let mut number = -1i64;
        if !value.is_null()
            && (CFGetTypeID(value) != CFNumberGetTypeID()
                || CFNumberGetValue(
                    value.cast(),
                    kCFNumberSInt64Type as _,
                    (&mut number as *mut i64).cast(),
                ) == 0)
        {
            return None;
        }
        let number = i32::try_from(number).ok()?;
        (number <= 0).then_some((name, number))
    }
}
unsafe fn default_presets(preset: AUPreset) -> CFArrayRef {
    // Process-lifetime immutable preset metadata, intentionally pinned with the
    // registered Cocoa class/image. It is not allocated or retired in render.
    static PRESET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let address = *PRESET.get_or_init(|| {
        unsafe {
            CFRetain(preset.presetName.cast());
        }
        Box::into_raw(Box::new(preset)) as usize
    });
    unsafe {
        CFRelease(preset.presetName.cast());
        CFArrayCreate(
            ptr::null(),
            (&address as *const usize).cast_mut().cast(),
            1,
            ptr::null(),
        )
    }
}
pub(super) unsafe extern "C" fn add_listener(
    p: *mut c_void,
    id: u32,
    callback: AudioUnitPropertyListenerProc,
    data: *mut c_void,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        if callback.is_none() {
            return BAD;
        }
        let c = unsafe { &mut *i.config.get() };
        if !c.listeners.iter().any(|(prop, f, d)| {
            *prop == id && f.map(|f| f as usize) == callback.map(|f| f as usize) && *d == data
        }) {
            c.listeners.push((id, callback, data));
        }
        0
    })
}
pub(super) unsafe extern "C" fn remove_listener(
    p: *mut c_void,
    id: u32,
    callback: AudioUnitPropertyListenerProc,
) -> OSStatus {
    unsafe { remove(p, id, callback, None) }
}
pub(super) unsafe extern "C" fn remove_listener_data(
    p: *mut c_void,
    id: u32,
    callback: AudioUnitPropertyListenerProc,
    data: *mut c_void,
) -> OSStatus {
    unsafe { remove(p, id, callback, Some(data)) }
}
unsafe fn remove(
    p: *mut c_void,
    id: u32,
    callback: AudioUnitPropertyListenerProc,
    data: Option<*mut c_void>,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        unsafe {
            (*i.config.get()).listeners.retain(|(prop, f, d)| {
                !(*prop == id
                    && f.map(|f| f as usize) == callback.map(|f| f as usize)
                    && data.is_none_or(|data| data == *d))
            });
        }
        0
    })
}
