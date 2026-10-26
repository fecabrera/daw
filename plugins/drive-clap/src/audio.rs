use crate::{
    instance::instance,
    params::{Events, apply},
};
use daw_plugin_clap::{
    boundary,
    sys::{ext::audio_ports::*, plugin::*, process::*},
    write_text,
};
use std::{ptr, sync::atomic::Ordering};
unsafe extern "C" fn count(_: *const clap_plugin, _: bool) -> u32 {
    1
}
unsafe extern "C" fn get(
    _: *const clap_plugin,
    index: u32,
    input: bool,
    output: *mut clap_audio_port_info,
) -> bool {
    boundary(false, || {
        if index != 0 || output.is_null() {
            return false;
        }
        let mut port = clap_audio_port_info {
            id: u32::from(!input),
            name: [0; 256],
            flags: CLAP_AUDIO_PORT_IS_MAIN | CLAP_AUDIO_PORT_REQUIRES_COMMON_SAMPLE_SIZE,
            channel_count: 2,
            port_type: CLAP_PORT_STEREO.as_ptr(),
            in_place_pair: u32::from(input),
        };
        write_text(
            if input {
                "Stereo input"
            } else {
                "Stereo output"
            },
            &mut port.name,
        );
        // SAFETY: host provides writable storage for one port descriptor.
        unsafe {
            output.write(port);
        }
        true
    })
}
pub(crate) static PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(count),
    get: Some(get),
};

pub(crate) unsafe extern "C" fn process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    boundary(CLAP_PROCESS_ERROR, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return CLAP_PROCESS_ERROR;
        };
        if process.is_null() || i.lifecycle.load(Ordering::Acquire) != 3 {
            return CLAP_PROCESS_ERROR;
        }
        // SAFETY: process owns the audio cell exclusively until return. The host
        // keeps its process/event/buffer structures alive for this entire call.
        let audio = unsafe { i.audio() };
        let process = unsafe { &*process };
        let frames = process.frames_count;
        if frames > audio.maximum || (frames != 0 && frames < audio.minimum) {
            return CLAP_PROCESS_ERROR;
        }
        let Some(events) = (unsafe { Events::new(process.in_events) }) else {
            return CLAP_PROCESS_ERROR;
        };
        let mut inputs = [ptr::null_mut(); 2];
        let mut outputs = [ptr::null_mut(); 2];
        let mut constants = [None; 2];
        if frames > 0 {
            if process.audio_inputs_count != 1
                || process.audio_outputs_count != 1
                || process.audio_inputs.is_null()
                || process.audio_outputs.is_null()
            {
                return CLAP_PROCESS_ERROR;
            }
            // Copy the headers: hosts may alias input/output buffer structures too.
            let input = unsafe { process.audio_inputs.read() };
            let output = unsafe { process.audio_outputs.read() };
            if input.channel_count != 2
                || output.channel_count != 2
                || input.data32.is_null()
                || output.data32.is_null()
                || !input.data64.is_null()
                || !output.data64.is_null()
            {
                return CLAP_PROCESS_ERROR;
            }
            for channel in 0..2 {
                inputs[channel] = unsafe { *input.data32.add(channel) };
                outputs[channel] = unsafe { *output.data32.add(channel) };
                if inputs[channel].is_null() || outputs[channel].is_null() {
                    return CLAP_PROCESS_ERROR;
                }
                if input.constant_mask & (1 << channel) != 0 {
                    constants[channel] = Some(unsafe { *inputs[channel] });
                }
            }
            if outputs[0] == outputs[1] {
                return CLAP_PROCESS_ERROR;
            }
            unsafe {
                (*process.audio_outputs).constant_mask = 0;
            } // All output frames are written; no quietness assumption.
        }
        i.apply_pending(audio);
        unsafe {
            i.editor.flush(audio, process.out_events);
        }
        let mut position = 0;
        for index in 0..events.count {
            let Some((pointer, header)) = (unsafe { events.get(index) }) else {
                return CLAP_PROCESS_ERROR;
            };
            if header.time < position
                || (frames > 0 && header.time >= frames)
                || (frames == 0 && header.time != 0)
            {
                return CLAP_PROCESS_ERROR;
            }
            while position < header.time {
                if !unsafe { sample(audio, inputs, outputs, constants, position as usize) } {
                    return CLAP_PROCESS_ERROR;
                }
                position += 1;
            }
            unsafe {
                apply(audio, pointer, header);
            }
        }
        while position < frames {
            if !unsafe { sample(audio, inputs, outputs, constants, position as usize) } {
                return CLAP_PROCESS_ERROR;
            }
            position += 1;
        }
        i.publish(audio);
        // Keep histories advancing even for silence/bypass. Never infer sleep
        // from a quiet input block; the tail extension reports one second.
        CLAP_PROCESS_CONTINUE
    })
}
unsafe fn sample(
    audio: &mut crate::instance::Audio,
    inputs: [*mut f32; 2],
    outputs: [*mut f32; 2],
    constants: [Option<f32>; 2],
    frame: usize,
) -> bool {
    // SAFETY: both channels are read before either is written. Raw pointers avoid
    // forming aliased Rust slices for legal in-place CLAP buffers. Constant input
    // values were captured before any in-place output could overwrite sample 0.
    let input = std::array::from_fn(|channel| {
        constants[channel].unwrap_or_else(|| unsafe { *inputs[channel].add(frame) })
    });
    if input.iter().any(|value| !value.is_finite()) {
        return false;
    }
    if let Some(processor) = &mut audio.processor {
        let output = processor.process_sample(input);
        unsafe {
            outputs[0].add(frame).write(output[0]);
            outputs[1].add(frame).write(output[1]);
        }
    }
    true
}
