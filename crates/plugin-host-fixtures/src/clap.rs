//! Synthetic CLAP effect/instrument with a real auxiliary input and fixed latency.
use super::*;
use daw_plugin_clap::{
    sys::{
        audio_buffer::*,
        entry::*,
        events::*,
        ext::{audio_ports::*, latency::*, note_ports::*, params::*, render::*, state::*},
        factory::plugin_factory::*,
        host::*,
        plugin::*,
        process::*,
        stream::*,
        version::*,
    },
    write_text,
};
use std::{
    ffi::{CStr, c_char, c_void},
    ptr,
};
struct Features([*const c_char; 3]);
// SAFETY: every pointer addresses an immutable static string.
unsafe impl Sync for Features {}
static FX: Features = Features([c"audio-effect".as_ptr(), c"stereo".as_ptr(), ptr::null()]);
static SYNTH: Features = Features([c"instrument".as_ptr(), c"stereo".as_ptr(), ptr::null()]);
static EFFECT: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: c"org.daw.host-fixture.effect".as_ptr(),
    name: c"Host Fixture Effect".as_ptr(),
    vendor: c"DAW Tests".as_ptr(),
    url: c"".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"1".as_ptr(),
    description: c"Owned synthetic hosting fixture".as_ptr(),
    features: FX.0.as_ptr(),
};
static INSTRUMENT: clap_plugin_descriptor = clap_plugin_descriptor {
    id: c"org.daw.host-fixture.instrument".as_ptr(),
    name: c"Host Fixture Instrument".as_ptr(),
    features: SYNTH.0.as_ptr(),
    ..EFFECT
};
static LAST: std::sync::atomic::AtomicPtr<Instance> =
    std::sync::atomic::AtomicPtr::new(ptr::null_mut());
struct Instance {
    host: *const clap_host,
    plugin: clap_plugin,
    processor: Processor,
    modulation: AtomicU64,
}
unsafe fn instance<'a>(plugin: *const clap_plugin) -> &'a Instance {
    unsafe { &*((*plugin).plugin_data.cast::<Instance>()) }
}
unsafe extern "C" fn init(_: *const clap_plugin) -> bool {
    true
}
unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    let pointer = unsafe { (*plugin).plugin_data.cast::<Instance>() };
    let _ = LAST.compare_exchange(
        pointer,
        ptr::null_mut(),
        Ordering::AcqRel,
        Ordering::Relaxed,
    );
    drop(unsafe { Box::from_raw(pointer) });
}
unsafe extern "C" fn activate(
    _: *const clap_plugin,
    rate: f64,
    minimum: u32,
    maximum: u32,
) -> bool {
    rate > 0.0 && minimum > 0 && maximum >= minimum
}
unsafe extern "C" fn nothing(_: *const clap_plugin) {}
unsafe extern "C" fn start(_: *const clap_plugin) -> bool {
    true
}
unsafe extern "C" fn reset(plugin: *const clap_plugin) {
    unsafe {
        *instance(plugin).processor.audio.get() = Audio::new();
    }
}
unsafe fn event(instance: &Instance, event: *const clap_event_header) {
    let header = unsafe { &*event };
    if header.space_id != CLAP_CORE_EVENT_SPACE_ID {
        return;
    }
    match header.type_ {
        CLAP_EVENT_PARAM_VALUE => {
            let e = unsafe { &*event.cast::<clap_event_param_value>() };
            if e.param_id == 10 {
                instance.processor.set_gain(e.value);
            }
        }
        CLAP_EVENT_PARAM_MOD => {
            let e = unsafe { &*event.cast::<clap_event_param_mod>() };
            if e.param_id == 10 {
                instance
                    .modulation
                    .store(e.amount.to_bits(), Ordering::Release);
            }
        }
        CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF => {
            let e = unsafe { &*event.cast::<clap_event_note>() };
            unsafe { &mut *instance.processor.audio.get() }.note(
                header.type_ == CLAP_EVENT_NOTE_ON,
                e.note_id,
                e.key,
                e.channel,
                e.velocity,
            );
        }
        _ => {}
    }
}
unsafe fn audio(input: *const clap_audio_buffer, bus: usize, frame: usize) -> [f32; 2] {
    let bus = unsafe { &*input.add(bus) };
    if bus.data32.is_null() {
        return [0.0; 2];
    }
    std::array::from_fn(|ch| unsafe {
        *(*bus.data32.add(ch.min(bus.channel_count as usize - 1))).add(frame)
    })
}
unsafe extern "C" fn process(plugin: *const clap_plugin, process: *const clap_process) -> i32 {
    let instance = unsafe { instance(plugin) };
    let process = unsafe { &*process };
    let list = unsafe { &*process.in_events };
    let count = unsafe { list.size.unwrap()(process.in_events) };
    let mut next = 0;
    for frame in 0..process.frames_count as usize {
        while next < count {
            let pointer = unsafe { list.get.unwrap()(process.in_events, next) };
            if unsafe { (*pointer).time } > frame as u32 {
                break;
            }
            unsafe { event(instance, pointer) };
            next += 1;
        }
        let main = if instance.processor.instrument {
            [0.0; 2]
        } else {
            unsafe { audio(process.audio_inputs, 0, frame) }
        };
        let aux = unsafe {
            audio(
                process.audio_inputs,
                usize::from(!instance.processor.instrument),
                frame,
            )
        };
        let gain = (instance.processor.gain()
            + f64::from_bits(instance.modulation.load(Ordering::Acquire)))
        .clamp(0.0, 2.0);
        let output = unsafe { &mut *instance.processor.audio.get() }.sample(
            instance.processor.instrument,
            gain,
            main,
            aux,
        );
        for (ch, sample) in output.into_iter().enumerate() {
            unsafe {
                *(*(*process.audio_outputs).data32.add(ch)).add(frame) = sample;
            }
        }
    }
    CLAP_PROCESS_CONTINUE
}
unsafe extern "C" fn port_count(plugin: *const clap_plugin, input: bool) -> u32 {
    if !input || unsafe { instance(plugin) }.processor.instrument {
        1
    } else {
        2
    }
}
unsafe extern "C" fn port_get(
    plugin: *const clap_plugin,
    index: u32,
    input: bool,
    out: *mut clap_audio_port_info,
) -> bool {
    if out.is_null() || index >= unsafe { port_count(plugin, input) } {
        return false;
    }
    let instrument = unsafe { instance(plugin) }.processor.instrument;
    let main = !input || !instrument && index == 0;
    let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
    info.id = if input && !main { 11 } else { 10 };
    info.flags = if main { CLAP_AUDIO_PORT_IS_MAIN } else { 0 };
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    info.in_place_pair = u32::MAX;
    write_text(if main { "Main" } else { "Sidechain" }, &mut info.name);
    unsafe { out.write(info) };
    true
}
static PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(port_count),
    get: Some(port_get),
};
unsafe extern "C" fn note_count(plugin: *const clap_plugin, input: bool) -> u32 {
    u32::from(input && unsafe { instance(plugin) }.processor.instrument)
}
unsafe extern "C" fn note_get(
    plugin: *const clap_plugin,
    index: u32,
    input: bool,
    out: *mut clap_note_port_info,
) -> bool {
    if out.is_null() || index != 0 || unsafe { note_count(plugin, input) } == 0 {
        return false;
    }
    let mut info: clap_note_port_info = unsafe { std::mem::zeroed() };
    info.id = 0;
    info.supported_dialects = CLAP_NOTE_DIALECT_CLAP;
    info.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    write_text("Notes", &mut info.name);
    unsafe { out.write(info) };
    true
}
static NOTES: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(note_count),
    get: Some(note_get),
};
unsafe extern "C" fn param_count(_: *const clap_plugin) -> u32 {
    1
}
unsafe extern "C" fn param_info(
    _: *const clap_plugin,
    index: u32,
    out: *mut clap_param_info,
) -> bool {
    if out.is_null() || index != 0 {
        return false;
    }
    let mut info: clap_param_info = unsafe { std::mem::zeroed() };
    info.id = 10;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_MODULATABLE;
    write_text("Gain", &mut info.name);
    info.min_value = 0.0;
    info.max_value = 2.0;
    info.default_value = 1.0;
    unsafe { out.write(info) };
    true
}
unsafe extern "C" fn value(plugin: *const clap_plugin, id: u32, out: *mut f64) -> bool {
    if out.is_null() || id != 10 {
        return false;
    }
    unsafe { out.write(instance(plugin).processor.gain()) };
    true
}
unsafe extern "C" fn text(
    _: *const clap_plugin,
    id: u32,
    value: f64,
    out: *mut c_char,
    capacity: u32,
) -> bool {
    if id != 10 || out.is_null() {
        return false;
    }
    write_text(&format!("{value:.3}"), unsafe {
        std::slice::from_raw_parts_mut(out, capacity as usize)
    })
}
unsafe extern "C" fn from_text(
    _: *const clap_plugin,
    _: u32,
    _: *const c_char,
    _: *mut f64,
) -> bool {
    false
}
unsafe extern "C" fn flush(
    plugin: *const clap_plugin,
    input: *const clap_input_events,
    _: *const clap_output_events,
) {
    let instance = unsafe { instance(plugin) };
    let list = unsafe { &*input };
    for n in 0..unsafe { list.size.unwrap()(input) } {
        unsafe {
            event(instance, list.get.unwrap()(input, n));
        }
    }
}
static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(param_count),
    get_info: Some(param_info),
    get_value: Some(value),
    value_to_text: Some(text),
    text_to_value: Some(from_text),
    flush: Some(flush),
};
unsafe extern "C" fn save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let bytes = unsafe { instance(plugin) }.processor.state();
    (unsafe { (*stream).write.unwrap()(stream, bytes.as_ptr().cast(), 16) }) == 16
}
unsafe extern "C" fn load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    let mut bytes = [0; 16];
    let mut position = 0;
    while position < 16 {
        let count = unsafe {
            (*stream).read.unwrap()(
                stream,
                bytes.as_mut_ptr().add(position).cast(),
                (16 - position) as u64,
            )
        };
        if count <= 0 {
            return false;
        }
        position += count as usize;
    }
    unsafe { instance(plugin) }.processor.restore(bytes)
}
static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(save),
    load: Some(load),
};
unsafe extern "C" fn latency(_: *const clap_plugin) -> u32 {
    LATENCY as u32
}
static LATENCY_EXTENSION: clap_plugin_latency = clap_plugin_latency { get: Some(latency) };
unsafe extern "C" fn hard(_: *const clap_plugin) -> bool {
    false
}
unsafe extern "C" fn mode(_: *const clap_plugin, _: i32) -> bool {
    true
}
static RENDER: clap_plugin_render = clap_plugin_render {
    has_hard_realtime_requirement: Some(hard),
    set: Some(mode),
};
unsafe extern "C" fn extension(_: *const clap_plugin, id: *const c_char) -> *const c_void {
    let id = unsafe { CStr::from_ptr(id) };
    if id == CLAP_EXT_AUDIO_PORTS {
        (&PORTS as *const clap_plugin_audio_ports).cast()
    } else if id == CLAP_EXT_NOTE_PORTS {
        (&NOTES as *const clap_plugin_note_ports).cast()
    } else if id == CLAP_EXT_PARAMS {
        (&PARAMS as *const clap_plugin_params).cast()
    } else if id == CLAP_EXT_STATE {
        (&STATE as *const clap_plugin_state).cast()
    } else if id == CLAP_EXT_LATENCY {
        (&LATENCY_EXTENSION as *const clap_plugin_latency).cast()
    } else if id == CLAP_EXT_RENDER {
        (&RENDER as *const clap_plugin_render).cast()
    } else {
        ptr::null()
    }
}
unsafe extern "C" fn count(_: *const clap_plugin_factory) -> u32 {
    2
}
unsafe extern "C" fn descriptor(
    _: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    match index {
        0 => &EFFECT,
        1 => &INSTRUMENT,
        _ => ptr::null(),
    }
}
unsafe extern "C" fn create(
    _: *const clap_plugin_factory,
    host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    let id = unsafe { CStr::from_ptr(id) };
    let instrument = id.to_bytes() == INSTRUMENT_CLAP_ID.as_bytes();
    if !instrument && id.to_bytes() != EFFECT_CLAP_ID.as_bytes() {
        return ptr::null();
    }
    let mut instance = Box::new(Instance {
        host,
        processor: Processor::new(instrument),
        modulation: AtomicU64::new(0.0f64.to_bits()),
        plugin: clap_plugin {
            desc: if instrument { &INSTRUMENT } else { &EFFECT },
            plugin_data: ptr::null_mut(),
            init: Some(init),
            destroy: Some(destroy),
            activate: Some(activate),
            deactivate: Some(nothing),
            start_processing: Some(start),
            stop_processing: Some(nothing),
            reset: Some(reset),
            process: Some(process),
            get_extension: Some(extension),
            on_main_thread: Some(nothing),
        },
    });
    instance.plugin.plugin_data = (&mut *instance as *mut Instance).cast();
    let pointer = &instance.plugin as *const clap_plugin;
    LAST.store(Box::into_raw(instance), Ordering::Release);
    pointer
}
static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(count),
    get_plugin_descriptor: Some(descriptor),
    create_plugin: Some(create),
};
unsafe extern "C" fn entry_init(_: *const c_char) -> bool {
    true
}
unsafe extern "C" fn deinit() {}
unsafe extern "C" fn factory(id: *const c_char) -> *const c_void {
    if unsafe { CStr::from_ptr(id) } == CLAP_PLUGIN_FACTORY_ID {
        (&FACTORY as *const clap_plugin_factory).cast()
    } else {
        ptr::null()
    }
}
#[unsafe(no_mangle)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(deinit),
    get_factory: Some(factory),
};

/// Owned-test hook for a preset chosen through a plugin's own controls.
/// # Safety
/// Call on the host main thread with the latest fixture instance quiescent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn daw_fixture_clap_preset(value: f64) -> bool {
    let pointer = LAST.load(Ordering::Acquire);
    if pointer.is_null() || !value.is_finite() || !(0.0..=2.0).contains(&value) {
        return false;
    }
    let instance = unsafe { &*pointer };
    instance.processor.set_gain(value);
    let host = unsafe { &*instance.host };
    let params = unsafe { host.get_extension.unwrap()(instance.host, CLAP_EXT_PARAMS.as_ptr()) }
        .cast::<clap_host_params>();
    let state = unsafe { host.get_extension.unwrap()(instance.host, CLAP_EXT_STATE.as_ptr()) }
        .cast::<clap_host_state>();
    if params.is_null() || state.is_null() {
        return false;
    }
    unsafe {
        (*params).rescan.unwrap()(instance.host, CLAP_PARAM_RESCAN_VALUES);
        (*state).mark_dirty.unwrap()(instance.host);
    }
    true
}
