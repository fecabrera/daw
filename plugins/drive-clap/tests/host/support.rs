// Test-only CLAP host. All pointers below refer to owned storage kept alive for
// each callback; returned plugin/extension pointers live through plugin destroy.
use daw_drive_plugin::{IDENTITY, Parameter as P, PluginState, SUPPORTED_RATES};
use daw_plugin_clap::sys::{
    audio_buffer::*,
    entry::*,
    events::*,
    ext::{audio_ports::*, latency::*, params::*, state::*, tail::*},
    factory::plugin_factory::*,
    host::*,
    plugin::*,
    process::*,
    stream::*,
    version::*,
};
use daw_unit_core::{EffectUnit, ProcessingContext};
use std::{
    ffi::{CStr, c_void},
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
static SERIAL: Mutex<()> = Mutex::new(());
type Begin = unsafe extern "C" fn();
type End = unsafe extern "C" fn(*mut u64, *mut u64);
struct HostData {
    rescans: AtomicU32,
    requests: AtomicU32,
}
unsafe extern "C" fn rescan(host: *const clap_host, flags: u32) {
    assert_eq!(flags, CLAP_PARAM_RESCAN_VALUES);
    unsafe { &*((*host).host_data.cast::<HostData>()) }
        .rescans
        .fetch_add(1, Ordering::Relaxed);
}
unsafe extern "C" fn request(host: *const clap_host) {
    unsafe { &*((*host).host_data.cast::<HostData>()) }
        .requests
        .fetch_add(1, Ordering::Relaxed);
}
static HOST_PARAMS: clap_host_params = clap_host_params {
    rescan: Some(rescan),
    clear: None,
    request_flush: None,
};
unsafe extern "C" fn host_extension(
    _: *const clap_host,
    id: *const std::ffi::c_char,
) -> *const c_void {
    if unsafe { CStr::from_ptr(id) } == CLAP_EXT_PARAMS {
        (&HOST_PARAMS as *const clap_host_params).cast()
    } else {
        ptr::null()
    }
}
struct Plugin {
    pointer: *const clap_plugin,
    entry: clap_plugin_entry,
    _host: Box<clap_host>,
    data: Box<HostData>,
    _library: Option<libloading::Library>,
    counts: Option<(Begin, End)>,
    active: bool,
    processing: bool,
}
impl Plugin {
    fn new() -> Self {
        let (entry, library, counts) =
            if let Some(path) = std::env::var_os("DRIVE_CLAP_TEST_LIBRARY") {
                unsafe {
                    let library = libloading::Library::new(path).unwrap();
                    let entry = **library
                        .get::<*const clap_plugin_entry>(b"clap_entry\0")
                        .unwrap();
                    let counts = library
                        .get::<Begin>(b"drive_clap_allocation_begin\0")
                        .ok()
                        .map(|begin| {
                            (
                                *begin,
                                *library.get::<End>(b"drive_clap_allocation_end\0").unwrap(),
                            )
                        });
                    if let Some((begin, end)) = counts {
                        let probe = *library
                            .get::<unsafe extern "C" fn()>(b"drive_clap_allocation_probe\0")
                            .expect("allocator positive control");
                        let (mut allocations, mut deallocations) = (0, 0);
                        begin();
                        probe();
                        end(&mut allocations, &mut deallocations);
                        assert!(
                            allocations > 0 && deallocations > 0,
                            "loaded-library allocator guard must detect its positive control"
                        );
                    }
                    if std::env::var_os("DRIVE_CLAP_REQUIRE_ALLOCATION_CHECK").is_some() {
                        assert!(
                            counts.is_some(),
                            "test library must have allocation-check enabled"
                        );
                    }
                    (entry, Some(library), counts)
                }
            } else {
                (plugin_export::clap_entry, None, None)
            };
        unsafe {
            assert!(entry.init.unwrap()(c"development-test".as_ptr()));
        }
        let factory = unsafe {
            &*entry.get_factory.unwrap()(CLAP_PLUGIN_FACTORY_ID.as_ptr())
                .cast::<clap_plugin_factory>()
        };
        assert_eq!(unsafe { factory.get_plugin_count.unwrap()(factory) }, 1);
        let descriptor = unsafe { factory.get_plugin_descriptor.unwrap()(factory, 0) };
        assert_eq!(
            unsafe { CStr::from_ptr((*descriptor).id) }
                .to_str()
                .unwrap(),
            IDENTITY.clap_id
        );
        assert!(unsafe { factory.get_plugin_descriptor.unwrap()(factory, 1) }.is_null());
        let mut data = Box::new(HostData {
            rescans: AtomicU32::new(0),
            requests: AtomicU32::new(0),
        });
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&mut *data as *mut HostData).cast(),
            name: c"Drive ABI tests".as_ptr(),
            vendor: c"Test".as_ptr(),
            version: c"1".as_ptr(),
            url: c"".as_ptr(),
            get_extension: Some(host_extension),
            request_restart: Some(request),
            request_process: Some(request),
            request_callback: Some(request),
        });
        assert!(
            unsafe { factory.create_plugin.unwrap()(factory, &*host, c"wrong.id".as_ptr()) }
                .is_null()
        );
        let pointer = unsafe { factory.create_plugin.unwrap()(factory, &*host, (*descriptor).id) };
        assert!(!pointer.is_null());
        assert!(unsafe { (*pointer).init.unwrap()(pointer) });
        Self {
            pointer,
            entry,
            _host: host,
            data,
            _library: library,
            counts,
            active: false,
            processing: false,
        }
    }
    fn params(&self) -> &clap_plugin_params {
        unsafe {
            &*(*self.pointer).get_extension.unwrap()(self.pointer, CLAP_EXT_PARAMS.as_ptr()).cast()
        }
    }
    fn state_ext(&self) -> &clap_plugin_state {
        unsafe {
            &*(*self.pointer).get_extension.unwrap()(self.pointer, CLAP_EXT_STATE.as_ptr()).cast()
        }
    }
    fn activate(&mut self, rate: u32, maximum: u32) {
        assert!(unsafe {
            (*self.pointer).activate.unwrap()(self.pointer, rate as f64, 1, maximum)
        });
        self.active = true;
        assert!(unsafe { (*self.pointer).start_processing.unwrap()(self.pointer) });
        self.processing = true;
    }
    fn stop(&mut self) {
        if self.processing {
            unsafe {
                (*self.pointer).stop_processing.unwrap()(self.pointer);
            }
            self.processing = false;
        }
    }
    fn deactivate(&mut self) {
        self.stop();
        if self.active {
            unsafe {
                (*self.pointer).deactivate.unwrap()(self.pointer);
            }
            self.active = false;
        }
    }
    fn reset(&self) {
        self.measured(|| unsafe { (*self.pointer).reset.unwrap()(self.pointer) });
    }
    fn get(&self, parameter: P) -> f64 {
        let mut value = f64::NAN;
        assert!(unsafe {
            self.params().get_value.unwrap()(self.pointer, parameter as u32, &mut value)
        });
        value
    }
    fn measured<T>(&self, call: impl FnOnce() -> T) -> T {
        if let Some((begin, _)) = self.counts {
            unsafe {
                begin();
            }
        }
        let value = call();
        if let Some((_, end)) = self.counts {
            let (mut alloc, mut dealloc) = (0, 0);
            unsafe {
                end(&mut alloc, &mut dealloc);
            }
            assert_eq!(
                (alloc, dealloc),
                (0, 0),
                "callback allocation/final-owner retirement"
            );
        }
        value
    }
    fn flush(&self, events: &EventList) {
        self.measured(|| unsafe {
            self.params().flush.unwrap()(self.pointer, &events.abi(), ptr::null())
        });
    }
    fn load_bytes(&self, bytes: &[u8], stride: usize, fail: bool) -> bool {
        let mut input = Input {
            bytes,
            offset: 0,
            stride,
            fail,
        };
        let stream = clap_istream {
            ctx: (&mut input as *mut Input).cast(),
            read: Some(read),
        };
        unsafe { self.state_ext().load.unwrap()(self.pointer, &stream) }
    }
    fn load(&self, state: PluginState) {
        assert!(self.load_bytes(&state.encode(), 1, false));
    }
    fn save(&self, stride: usize) -> Option<Vec<u8>> {
        let mut output = Output {
            bytes: vec![],
            stride,
        };
        let stream = clap_ostream {
            ctx: (&mut output as *mut Output).cast(),
            write: Some(write),
        };
        unsafe { self.state_ext().save.unwrap()(self.pointer, &stream) }.then_some(output.bytes)
    }
}
impl Drop for Plugin {
    fn drop(&mut self) {
        self.deactivate();
        unsafe {
            (*self.pointer).destroy.unwrap()(self.pointer);
            self.entry.deinit.unwrap()();
        }
    }
}
struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
    stride: usize,
    fail: bool,
}
unsafe extern "C" fn read(stream: *const clap_istream, data: *mut c_void, size: u64) -> i64 {
    let input = unsafe { &mut *((*stream).ctx.cast::<Input>()) };
    if input.fail {
        return -1;
    }
    let count = (size as usize)
        .min(input.stride)
        .min(input.bytes.len() - input.offset);
    unsafe {
        ptr::copy_nonoverlapping(input.bytes.as_ptr().add(input.offset), data.cast(), count);
    }
    input.offset += count;
    count as i64
}
struct Output {
    bytes: Vec<u8>,
    stride: usize,
}
unsafe extern "C" fn write(stream: *const clap_ostream, data: *const c_void, size: u64) -> i64 {
    let output = unsafe { &mut *((*stream).ctx.cast::<Output>()) };
    let count = (size as usize).min(output.stride);
    output
        .bytes
        .extend_from_slice(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), count) });
    count as i64
}
struct EventList(Vec<clap_event_param_value>);
impl EventList {
    fn new(events: &[(u32, P, f64)]) -> Self {
        Self(
            events
                .iter()
                .map(|&(time, p, value)| clap_event_param_value {
                    header: clap_event_header {
                        size: size_of::<clap_event_param_value>() as u32,
                        time,
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_PARAM_VALUE,
                        flags: 0,
                    },
                    param_id: p as u32,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value,
                })
                .collect(),
        )
    }
    fn abi(&self) -> clap_input_events {
        clap_input_events {
            ctx: (self as *const Self).cast_mut().cast(),
            size: Some(event_count),
            get: Some(event_get),
        }
    }
}
unsafe extern "C" fn event_count(events: *const clap_input_events) -> u32 {
    unsafe { &*((*events).ctx.cast::<EventList>()) }.0.len() as u32
}
unsafe extern "C" fn event_get(
    events: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    unsafe { &*((*events).ctx.cast::<EventList>()) }
        .0
        .get(index as usize)
        .map_or(ptr::null(), |e| &e.header)
}
fn run(
    plugin: &Plugin,
    left: &mut [f32],
    right: &mut [f32],
    events: &EventList,
    in_place: bool,
    mask: u64,
) -> (Vec<f32>, Vec<f32>) {
    run_with_transport(plugin, left, right, events, in_place, mask, ptr::null())
}
fn run_with_transport(
    plugin: &Plugin,
    left: &mut [f32],
    right: &mut [f32],
    events: &EventList,
    in_place: bool,
    mask: u64,
    transport: *const clap_event_transport,
) -> (Vec<f32>, Vec<f32>) {
    assert_eq!(left.len(), right.len());
    let mut out_left = vec![f32::NAN; left.len()];
    let mut out_right = out_left.clone();
    let mut inputs = [left.as_mut_ptr(), right.as_mut_ptr()];
    let mut outputs = if in_place {
        inputs
    } else {
        [out_left.as_mut_ptr(), out_right.as_mut_ptr()]
    };
    let input = clap_audio_buffer {
        data32: inputs.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: mask,
    };
    let mut output = clap_audio_buffer {
        data32: outputs.as_mut_ptr(),
        data64: ptr::null_mut(),
        channel_count: 2,
        latency: 0,
        constant_mask: u64::MAX,
    };
    let events = events.abi();
    let process = clap_process {
        steady_time: -1,
        frames_count: left.len() as u32,
        transport,
        audio_inputs: &input,
        audio_outputs: &mut output,
        audio_inputs_count: 1,
        audio_outputs_count: 1,
        in_events: &events,
        out_events: ptr::null(),
    };
    assert_eq!(
        plugin.measured(|| unsafe { (*plugin.pointer).process.unwrap()(plugin.pointer, &process) }),
        CLAP_PROCESS_CONTINUE
    );
    assert_eq!(output.constant_mask, 0);
    if in_place {
        (left.to_vec(), right.to_vec())
    } else {
        (out_left, out_right)
    }
}
fn reference(
    drive: &mut Drive,
    state: &mut PluginState,
    left: &[f32],
    right: &[f32],
    events: &EventList,
) -> (Vec<f32>, Vec<f32>) {
    let mut output = (vec![], vec![]);
    let mut cursor = 0;
    for frame in 0..left.len() {
        while cursor < events.0.len() && events.0[cursor].header.time as usize == frame {
            let event = events.0[cursor];
            state
                .set(P::from_id(event.param_id).unwrap(), event.value)
                .unwrap();
            drive.retarget(&state.to_settings());
            cursor += 1;
        }
        let sample = drive.process_sample([left[frame], right[frame]]);
        output.0.push(sample[0]);
        output.1.push(sample[1]);
    }
    output
}
fn assert_samples(actual: &(Vec<f32>, Vec<f32>), expected: &(Vec<f32>, Vec<f32>)) {
    for (a, b) in actual
        .0
        .iter()
        .chain(&actual.1)
        .zip(expected.0.iter().chain(&expected.1))
    {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "sample-for-sample parity: {a} != {b}"
        );
    }
}

fn enable_color(state: &mut PluginState) {
    if let Some(p) = P::ALL.into_iter().find(|p| p.info().key == "color_enabled") {
        state.set(p, 1.0).unwrap();
    }
}
