use super::*;
use daw_plugin_auv2::buffers::{StereoList, stereo};
pub(super) unsafe extern "C" fn render(
    pointer: *mut c_void,
    flags: *mut u32,
    stamp: *const AudioTimeStamp,
    bus: u32,
    frames: u32,
    output: *mut AudioBufferList,
) -> OSStatus {
    let result = boundary::call(BUSY, || unsafe {
        process(pointer, flags, stamp, bus, frames, output)
    });
    if let Some(i) = unsafe { instance(pointer) } {
        i.last_error.store(result, Ordering::Relaxed);
        // A contained panic leaves the marker set. Silence only validated extents.
        if frames <= MAX_FRAMES
            && i.fault.load(Ordering::Acquire)
            && let Some(channels) = unsafe { stereo(output, frames) }
        {
            for p in channels {
                for n in 0..frames as usize {
                    unsafe {
                        p.add(n).write_unaligned(0.0);
                    }
                }
            }
        }
    }
    result
}
unsafe fn process(
    pointer: *mut c_void,
    flags: *mut u32,
    stamp: *const AudioTimeStamp,
    bus: u32,
    frames: u32,
    output: *mut AudioBufferList,
) -> OSStatus {
    let Some(i) = (unsafe { instance(pointer) }) else {
        return BAD;
    };
    if bus != 0 {
        return kAudioUnitErr_InvalidElement;
    }
    if flags.is_null() || stamp.is_null() || output.is_null() {
        return BAD;
    }
    if i.fault.load(Ordering::Acquire) {
        return BUSY;
    }
    let Some(_guard) = token(&i.gate) else {
        return BUSY;
    };
    let c = unsafe { &*i.config.get() };
    if frames > c.maximum {
        return kAudioUnitErr_TooManyFramesToProcess;
    }
    let Some(e) = (unsafe { &mut *i.engine.get() }) else {
        return kAudioUnitErr_Uninitialized;
    };
    let timestamp = unsafe { stamp.read_unaligned() };
    struct Phase<'a>(&'a AtomicU8);
    impl Drop for Phase<'_> {
        fn drop(&mut self) {
            self.0.store(0, Ordering::Release);
        }
    }
    i.phase.store(1, Ordering::Release);
    let _phase = Phase(&i.phase);
    let cached = timestamp.mFlags & kAudioTimeStampSampleTimeValid != 0
        && timestamp.mSampleTime.is_finite()
        && e.stamp == Some((timestamp.mSampleTime.to_bits(), frames));
    // Set before entering product/host code; normal returns clear the marker.
    i.fault.store(true, Ordering::Release);
    let result = unsafe { block(i, c, e, flags, &timestamp, frames, output) };
    i.phase.store(3, Ordering::Release);
    let mut post_flags = unsafe { *flags } | kAudioUnitRenderAction_PostRender;
    if result != 0 {
        post_flags |= kAudioUnitRenderAction_PostRenderError;
    }
    if !cached {
        for (callback, data) in &c.notifications {
            if let Some(callback) = callback {
                unsafe {
                    callback(*data, &mut post_flags, &timestamp, 0, frames, output);
                }
            }
        }
    }
    i.fault.store(false, Ordering::Release);
    result
}
unsafe fn block(
    i: &Instance,
    c: &Configuration,
    e: &mut Engine,
    flags: *mut u32,
    stamp: &AudioTimeStamp,
    frames: u32,
    output: *mut AudioBufferList,
) -> OSStatus {
    #[cfg(test)]
    if i.inject_fault.swap(false, Ordering::AcqRel) {
        panic!("AU contained fault sentinel");
    }
    let n = frames as usize;
    // A NULL plane requests unit-owned output. Copy headers, never form an
    // out-of-bounds Rust reference to AudioBufferList's one-element tail.
    if unsafe { (*output).mNumberBuffers } != 2 {
        return kAudioUnitErr_FormatNotSupported;
    }
    let headers = unsafe {
        output
            .cast::<u8>()
            .add(std::mem::offset_of!(AudioBufferList, mBuffers))
            .cast::<AudioBuffer>()
    };
    for channel in 0..2 {
        let mut h = unsafe { headers.add(channel).read_unaligned() };
        if h.mNumberChannels != 1 {
            return kAudioUnitErr_FormatNotSupported;
        }
        if h.mData.is_null() && c.allocate_output {
            h.mData = e.output[channel].as_mut_ptr().cast();
            h.mDataByteSize = frames * 4;
            unsafe {
                headers.add(channel).write_unaligned(h);
            }
        }
    }
    let Some(dest) = (unsafe { stereo(output, frames) }) else {
        return BAD;
    };
    let valid_clock =
        stamp.mFlags & kAudioTimeStampSampleTimeValid != 0 && stamp.mSampleTime.is_finite();
    let key = valid_clock.then_some((stamp.mSampleTime.to_bits(), frames));
    if key.is_some() && key == e.stamp {
        unsafe {
            copy_output(e, dest, n);
        }
        return 0;
    }
    e.stamp = None;
    // Notifications run before event draining. They may schedule this block.
    let mut notify_flags = unsafe { *flags } | kAudioUnitRenderAction_PreRender;
    for (callback, data) in &c.notifications {
        if let Some(callback) = callback {
            let code = unsafe { callback(*data, &mut notify_flags, stamp, 0, frames, output) };
            if code != 0 {
                return code;
            }
        }
    }
    if !i.events.take(&mut e.scheduled) {
        return BUSY;
    }
    i.phase.store(2, Ordering::Release);
    e.ramps.fill(None);
    e.active_ramps.clear();
    if let Some(state) = i.shared.exchange.consume(&mut e.applied) {
        e.state = state;
        e.processor.restore(state);
    }
    for (slot, p) in Parameter::ALL.into_iter().enumerate() {
        let bits = i.shared.values[slot].load(Ordering::Acquire);
        if bits != e.seen[slot] {
            if e.state.set(p, f64::from_bits(bits)).is_ok() {
                e.processor.apply_parameter(p, &e.state);
            }
            e.seen[slot] = bits;
        }
    }
    let mut tempo = None;
    let mut position = None;
    let mut playing = None;
    if let Some(callback) = c.host.beatAndTempoProc {
        let mut beat = 0.0;
        let mut bpm = 0.0;
        if unsafe { callback(c.host.hostUserData, &mut beat, &mut bpm) } == 0
            && bpm.is_finite()
            && bpm > 0.0
            && bpm <= f64::from(f32::MAX)
        {
            tempo = Some(bpm as f32);
        }
    }
    if let Some(callback) = c.host.transportStateProc2 {
        let mut run = 0;
        let mut record = 0;
        let mut changed = 0;
        let mut sample = f64::NAN;
        let mut cycling = 0;
        let mut start = 0.0;
        let mut end = 0.0;
        if unsafe {
            callback(
                c.host.hostUserData,
                &mut run,
                &mut record,
                &mut changed,
                &mut sample,
                &mut cycling,
                &mut start,
                &mut end,
            )
        } == 0
        {
            playing = Some(run != 0);
            if sample.is_finite() {
                position = Some(sample.max(0.0) as u64);
            }
        }
    }
    if playing.is_none()
        && let Some(callback) = c.host.transportStateProc
    {
        let mut run = 0;
        let mut changed = 0;
        let mut sample = f64::NAN;
        let mut cycling = 0;
        let mut start = 0.0;
        let mut end = 0.0;
        if unsafe {
            callback(
                c.host.hostUserData,
                &mut run,
                &mut changed,
                &mut sample,
                &mut cycling,
                &mut start,
                &mut end,
            )
        } == 0
        {
            playing = Some(run != 0);
            if sample.is_finite() {
                position = Some(sample.max(0.0) as u64);
            }
        }
    }
    if position.is_none() && valid_clock {
        position = Some(stamp.mSampleTime.max(0.0) as u64);
    }
    if valid_clock && e.next_frame.is_some_and(|v| v != stamp.mSampleTime) {
        e.processor.reset();
    }
    e.processor.set_transport_state(position, tempo, playing);
    use daw_drive_plugin::BandAudition;
    let offline = c.offline || unsafe { *flags } & kAudioOfflineUnitRenderAction_Render != 0;
    if offline || playing == Some(false) || e.state.get(Parameter::Bypass) != 0.0 {
        for flag in i.shared.model.output_meter.band_solos() {
            flag.store(false, Ordering::Relaxed);
        }
    }
    i.shared
        .model
        .output_meter
        .set_audition_allowed(!offline && playing != Some(false));
    for (requests, level) in [
        (i.shared.model.output_meter.band_reset_requests(), false),
        (
            i.shared.model.output_meter.band_level_reset_requests(),
            true,
        ),
    ] {
        if let Some(requests) = requests {
            let mut pending = requests.swap(0, Ordering::AcqRel);
            while pending != 0 {
                let slot = pending.trailing_zeros() as usize;
                if level {
                    i.shared.model.output_meter.reset_band_level_peak(slot);
                } else {
                    i.shared.model.output_meter.reset_band_peak(slot);
                }
                pending &= pending - 1;
            }
        }
    }
    let reset = i.shared.model.meter_reset.swap(0, Ordering::AcqRel);
    let output_reset = i.shared.model.output_reset.swap(false, Ordering::AcqRel);
    if reset != 0 || output_reset {
        daw_drive_plugin::reset_meter(
            &i.shared.model.output_meter,
            if output_reset { u32::MAX } else { reset },
        );
    }
    for channel in &mut e.input {
        channel[..n].fill(0.0);
    }
    let planes = if c.allocate_input {
        [e.input[0].as_mut_ptr(), e.input[1].as_mut_ptr()]
    } else {
        // Same-format effects can borrow the caller's output planes for their
        // input pull. Hosts can disable both element allocations in a graph;
        // forwarding NULL here would leave an upstream unit with no buffer.
        dest
    };
    let mut input = StereoList::new(planes[0], planes[1], frames);
    let mut input_flags = 0;
    let input_code = if let Some(callback) = c.input.inputProc {
        unsafe {
            callback(
                c.input.inputProcRefCon,
                &mut input_flags,
                stamp,
                0,
                frames,
                input.as_mut_ptr(),
            )
        }
    } else if !c.connection.sourceAudioUnit.is_null() {
        unsafe {
            AudioUnitRender(
                c.connection.sourceAudioUnit,
                &mut input_flags,
                stamp,
                c.connection.sourceOutputNumber,
                frames,
                input.as_mut_ptr(),
            )
        }
    } else {
        if !c.allocate_input {
            for plane in planes {
                for frame in 0..n {
                    unsafe {
                        plane.add(frame).write_unaligned(0.0);
                    }
                }
            }
        }
        0
    };
    if input_code != 0 {
        for channel in &mut e.output {
            channel[..n].fill(0.0);
        }
        unsafe {
            copy_output(e, dest, n);
            *flags |= kAudioUnitRenderAction_OutputIsSilence;
        }
        return input_code;
    }
    let Some(source) = (unsafe { stereo(input.as_mut_ptr(), frames) }) else {
        return BAD;
    };
    let mut event_index = 0;
    for frame in 0..n.max(1) {
        while event_index < e.scheduled.len() && e.scheduled[event_index].start <= frame as i64 {
            let event = e.scheduled[event_index];
            let slot = index(event.parameter);
            if e.ramps[slot].is_none() {
                e.active_ramps.push(slot);
            }
            e.ramps[slot] = Some(event);
            event_index += 1;
        }
        let mut active = 0;
        while active < e.active_ramps.len() {
            let slot = e.active_ramps[active];
            if let Some(event) = e.ramps[slot] {
                let elapsed = (frame as i64 - event.start).max(0) as f64;
                let fraction = if event.duration == 0 {
                    1.0
                } else {
                    (elapsed / f64::from(event.duration)).min(1.0)
                };
                let value = event.from + (event.to - event.from) * fraction;
                if e.state.set(event.parameter, value).is_ok() {
                    e.processor.apply_parameter(event.parameter, &e.state);
                }
                if fraction >= 1.0 {
                    e.ramps[slot] = None;
                    e.active_ramps.swap_remove(active);
                } else {
                    active += 1;
                }
                // Do not overwrite a concurrent control write.
                let old = e.seen[slot];
                if i.shared.values[slot]
                    .compare_exchange(old, value.to_bits(), Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
                {
                    if old != value.to_bits() {
                        i.shared.preset_dirty.store(true, Ordering::Release);
                    }
                    e.seen[slot] = value.to_bits();
                }
            }
        }
        if n == 0 {
            break;
        }
        let input = if input_flags & kAudioUnitRenderAction_OutputIsSilence != 0 {
            [0.0; 2]
        } else {
            unsafe {
                [
                    source[0].add(frame).read_unaligned(),
                    source[1].add(frame).read_unaligned(),
                ]
            }
        };
        let result = e.processor.process_sample(input);
        for (channel, value) in result.into_iter().enumerate() {
            e.output[channel][frame] = value;
        }
    }
    i.shared.exchange.publish(e.state, e.applied);
    e.stamp = key;
    e.next_frame = valid_clock.then_some(stamp.mSampleTime + f64::from(frames));
    unsafe {
        copy_output(e, dest, n);
        *flags &= !kAudioUnitRenderAction_OutputIsSilence;
    }

    0
}
unsafe fn copy_output(e: &Engine, output: [*mut f32; 2], frames: usize) {
    for (channel, pointer) in output.into_iter().enumerate() {
        for frame in 0..frames {
            unsafe {
                pointer.add(frame).write_unaligned(e.output[channel][frame]);
            }
        }
    }
}
pub(super) unsafe extern "C" fn add_notify(
    p: *mut c_void,
    callback: AURenderCallback,
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
        if !c
            .notifications
            .iter()
            .any(|(f, d)| f.map(|f| f as usize) == callback.map(|f| f as usize) && *d == data)
        {
            c.notifications.push((callback, data));
        }
        0
    })
}
pub(super) unsafe extern "C" fn remove_notify(
    p: *mut c_void,
    callback: AURenderCallback,
    data: *mut c_void,
) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        unsafe {
            (*i.config.get()).notifications.retain(|(f, d)| {
                !(f.map(|f| f as usize) == callback.map(|f| f as usize) && *d == data)
            });
        }
        0
    })
}
