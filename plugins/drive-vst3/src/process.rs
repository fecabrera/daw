use crate::{component::Component, queues::Queues};
use daw_plugin_vst3::sys::Steinberg::{Vst::*, *};
use std::{ptr, sync::atomic::Ordering};
/// # Safety
/// VST3 serializes process/parameter-flush calls; all host pointers remain live.
pub(crate) unsafe fn process(component: &Component, data: *mut ProcessData) -> tresult {
    if data.is_null() || !component.initialized.load(Ordering::Acquire) {
        return kInvalidArgument;
    }
    let data = unsafe { *data };
    if data.numSamples < 0 || data.symbolicSampleSize != 0 || !(0..=2).contains(&data.processMode) {
        return kInvalidArgument;
    }
    let frames = data.numSamples;
    if frames > 0
        && (!component.active.load(Ordering::Acquire)
            || !component.processing.load(Ordering::Acquire))
    {
        return kResultFalse;
    }
    if let Some(setup) = component.setup.get()
        && frames > setup.maxSamplesPerBlock
    {
        return kInvalidArgument;
    }
    // SDK guarantees process calls (including no-audio parameter flush) are not
    // concurrent with mutable UI-thread inactive lifecycle/state calls.
    let audio = unsafe { component.audio() };
    component.recall(audio);
    let Ok(mut queues) =
        (unsafe { Queues::new(data.inputParameterChanges, audio.normalized, frames) })
    else {
        return kInvalidArgument;
    };
    let (mut inputs, mut outputs) = ([ptr::null_mut(); 2], [ptr::null_mut(); 2]);
    let mut silent = [true; 2];
    if frames > 0 {
        if !(0..=1).contains(&data.numInputs) || !(0..=1).contains(&data.numOutputs) {
            return kInvalidArgument;
        }
        if component.input_active.get() {
            if data.numInputs != 1 || data.inputs.is_null() {
                return kInvalidArgument;
            }
            // Copy headers so a legal input/output header alias creates no Rust alias.
            let input = unsafe { data.inputs.read() };
            if input.numChannels != 2 {
                return kInvalidArgument;
            }
            let input_ptrs = unsafe { input.__field0.channelBuffers32 };
            for channel in 0..2 {
                silent[channel] = input.silenceFlags & (1 << channel) != 0;
                inputs[channel] = if input_ptrs.is_null() {
                    ptr::null_mut()
                } else {
                    unsafe { *input_ptrs.add(channel) }
                };
                if !silent[channel] && inputs[channel].is_null() {
                    return kInvalidArgument;
                }
            }
        }
        if component.output_active.get() {
            if data.numOutputs != 1 || data.outputs.is_null() {
                return kInvalidArgument;
            }
            let output = unsafe { data.outputs.read() };
            if output.numChannels != 2 {
                return kInvalidArgument;
            }
            let output_ptrs = unsafe { output.__field0.channelBuffers32 };
            if output_ptrs.is_null() {
                return kInvalidArgument;
            }
            for (channel, destination) in outputs.iter_mut().enumerate() {
                *destination = unsafe { *output_ptrs.add(channel) };
                if destination.is_null() {
                    return kInvalidArgument;
                }
            }
            if outputs[0] == outputs[1] {
                return kInvalidArgument;
            }
            unsafe {
                (*data.outputs).silenceFlags = 0;
            }
        }
    }
    for frame in 0..frames.max(1) {
        for cursor in queues.cursors[..queues.count].iter_mut().flatten() {
            let (parameter, cursor) = cursor;
            let Ok(value) = (unsafe { cursor.at(frame) }) else {
                return kInvalidArgument;
            };
            let slot = *parameter as usize - 100;
            if value != audio.normalized[slot] {
                if audio.state.set_normalized(*parameter, value).is_err() {
                    return kInvalidArgument;
                }
                audio.normalized[slot] = value;
                if let Some(processor) = &mut audio.processor {
                    processor.restore(audio.state);
                }
            }
        }
        if frames == 0 {
            break;
        }
        let input = std::array::from_fn(|channel| {
            if silent[channel] {
                0.0
            } else {
                unsafe { *inputs[channel].add(frame as usize) }
            }
        });
        if input.iter().any(|v| !v.is_finite()) {
            return kInvalidArgument;
        }
        let Some(processor) = &mut audio.processor else {
            return kNotInitialized;
        };
        let output = processor.process_sample(input);
        if component.output_active.get() {
            for channel in 0..2 {
                unsafe {
                    outputs[channel].add(frame as usize).write(output[channel]);
                }
            }
        }
    }
    component.publish(audio);
    kResultOk
}
