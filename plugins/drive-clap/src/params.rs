use crate::instance::{Audio, instance};
use daw_drive_plugin::Parameter;
use daw_plugin_clap::{
    boundary,
    sys::{events::*, ext::params::*, plugin::*},
    write_text,
};
use std::{
    ffi::{CStr, c_char},
    ptr,
    sync::atomic::Ordering,
};

pub(crate) struct Events {
    source: *const clap_input_events,
    pub count: u32,
}
impl Events {
    /// SAFETY: source and returned event pointers stay valid for this CLAP call.
    pub unsafe fn new(source: *const clap_input_events) -> Option<Self> {
        if source.is_null() {
            return Some(Self { source, count: 0 });
        }
        let size = unsafe { (*source).size }?;
        unsafe { (*source).get }?;
        Some(Self {
            source,
            count: unsafe { size(source) },
        })
    }
    pub unsafe fn get(&self, index: u32) -> Option<(*const clap_event_header, clap_event_header)> {
        let get = unsafe { (*self.source).get }?;
        let pointer = unsafe { get(self.source, index) };
        if pointer.is_null() {
            return None;
        }
        let header = unsafe { ptr::read_unaligned(pointer) };
        (header.size as usize >= size_of::<clap_event_header>()).then_some((pointer, header))
    }
}
pub(crate) unsafe fn apply(
    audio: &mut Audio,
    pointer: *const clap_event_header,
    header: clap_event_header,
) {
    if header.space_id != CLAP_CORE_EVENT_SPACE_ID
        || header.type_ != CLAP_EVENT_PARAM_VALUE
        || (header.size as usize) < size_of::<clap_event_param_value>()
    {
        return;
    }
    // SAFETY: the header's checked byte extent includes this CLAP value event.
    let event = unsafe { ptr::read_unaligned(pointer.cast::<clap_event_param_value>()) };
    // Only global base values are advertised. Do not interpret cookies or notes.
    if event.note_id != -1 || event.port_index != -1 || event.channel != -1 || event.key != -1 {
        return;
    }
    if let Ok(parameter) = Parameter::from_id(event.param_id) {
        // CLAP specifies truncation for stepped plain values. The shared model
        // remains strict; normalized VST3 choice rounding is a different contract.
        let value = if parameter.info().steps > 0 {
            event.value.trunc()
        } else {
            event.value
        };
        audio.set(parameter, value);
    }
}
unsafe extern "C" fn count(_: *const clap_plugin) -> u32 {
    7
}
unsafe extern "C" fn info(
    _: *const clap_plugin,
    index: u32,
    destination: *mut clap_param_info,
) -> bool {
    boundary(false, || {
        let Some(parameter) = Parameter::ALL.get(index as usize) else {
            return false;
        };
        if destination.is_null() {
            return false;
        }
        let info = parameter.info();
        let mut output = clap_param_info {
            id: *parameter as u32,
            flags: CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_REQUIRES_PROCESS,
            cookie: ptr::null_mut(),
            name: [0; 256],
            module: [0; 1024],
            min_value: info.minimum,
            max_value: info.maximum,
            default_value: info.default,
        };
        if info.steps > 0 {
            output.flags |= CLAP_PARAM_IS_STEPPED;
        }
        if *parameter == Parameter::Bypass {
            output.flags |= CLAP_PARAM_IS_BYPASS;
        }
        if *parameter == Parameter::ColorShape {
            output.flags |= CLAP_PARAM_IS_ENUM;
        }
        write_text(info.name, &mut output.name);
        // SAFETY: host supplies writable storage for one clap_param_info.
        unsafe {
            destination.write(output);
        }
        true
    })
}
unsafe extern "C" fn value(plugin: *const clap_plugin, id: u32, output: *mut f64) -> bool {
    boundary(false, || {
        let (Some(i), Ok(parameter)) = (unsafe { instance(plugin) }, Parameter::from_id(id)) else {
            return false;
        };
        if output.is_null() {
            return false;
        }
        let Some(state) = i.current() else {
            return false;
        };
        unsafe {
            output.write(state.get(parameter));
        }
        true
    })
}
unsafe extern "C" fn format(
    _: *const clap_plugin,
    id: u32,
    value: f64,
    output: *mut c_char,
    capacity: u32,
) -> bool {
    boundary(false, || {
        if output.is_null() || capacity == 0 {
            return false;
        }
        let Ok(parameter) = Parameter::from_id(id) else {
            return false;
        };
        let value = if parameter.info().steps > 0 {
            value.trunc()
        } else {
            value
        };
        let Ok(text) = parameter.format(value) else {
            return false;
        };
        // SAFETY: the host declares this writable byte extent, used only on main.
        write_text(&text, unsafe {
            std::slice::from_raw_parts_mut(output, capacity as usize)
        })
    })
}
unsafe extern "C" fn parse(
    _: *const clap_plugin,
    id: u32,
    text: *const c_char,
    output: *mut f64,
) -> bool {
    boundary(false, || {
        if text.is_null() || output.is_null() {
            return false;
        }
        let Ok(parameter) = Parameter::from_id(id) else {
            return false;
        };
        // SAFETY: text is a host-provided NUL-terminated UTF-8 string.
        let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
            return false;
        };
        let Ok(value) = parameter.parse(text) else {
            return false;
        };
        unsafe {
            output.write(value);
        }
        true
    })
}
unsafe extern "C" fn flush(
    plugin: *const clap_plugin,
    input: *const clap_input_events,
    output: *const clap_output_events,
) {
    boundary((), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return;
        };
        if i.lifecycle.load(Ordering::Acquire) == 0 {
            return;
        }
        let Some(events) = (unsafe { Events::new(input) }) else {
            return;
        };
        // SAFETY: CLAP serializes flush with process; inactive flush is on main.
        let audio = unsafe { i.audio() };
        i.apply_pending(audio);
        unsafe {
            i.editor.flush(audio, output);
        }
        for index in 0..events.count {
            if let Some((pointer, header)) = unsafe { events.get(index) } {
                unsafe {
                    apply(audio, pointer, header);
                }
            }
        }
        i.publish(audio);
    });
}
pub(crate) static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(count),
    get_info: Some(info),
    get_value: Some(value),
    value_to_text: Some(format),
    text_to_value: Some(parse),
    flush: Some(flush),
};
