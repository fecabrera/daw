use daw_drive_plugin::{Parameter, PluginState, Processor, StateExchange};
use daw_plugin_auv2::{boundary, sys::*};
use std::{
    cell::UnsafeCell,
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
};
mod editor;
mod events;
mod properties;
mod render;
use events::Events;
const BAD: OSStatus = kAudioUnitErr_InvalidPropertyValue;
const BUSY: OSStatus = kAudioUnitErr_CannotDoInCurrentContext;
const PRIVATE_MODEL: u32 = 0x5369654d;
pub const MAX_FRAMES: u32 = 65536;
struct Shared {
    values: [AtomicU64; Parameter::COUNT],
    exchange: StateExchange,
    control: AtomicBool,
    recall_revision: AtomicU64,
    host_gate: AtomicBool,
    preset_dirty: AtomicBool,
    model: Arc<daw_drive_editor::Model>,
    closed: AtomicBool,
    gestures: [AtomicUsize; Parameter::COUNT],
    pending_ends: [AtomicUsize; Parameter::COUNT],
    handle: AtomicUsize,
}
impl Shared {
    fn new() -> Self {
        Self {
            values: Parameter::ALL.map(|p| AtomicU64::new(p.info().default.to_bits())),
            exchange: StateExchange::default(),
            control: AtomicBool::new(false),
            recall_revision: AtomicU64::new(0),
            host_gate: AtomicBool::new(false),
            preset_dirty: AtomicBool::new(false),
            model: Arc::default(),
            closed: AtomicBool::new(false),
            gestures: std::array::from_fn(|_| AtomicUsize::new(0)),
            pending_ends: std::array::from_fn(|_| AtomicUsize::new(0)),
            handle: AtomicUsize::new(0),
        }
    }
    fn parameter(&self, p: Parameter) -> f64 {
        f64::from_bits(self.values[index(p)].load(Ordering::Acquire))
    }
    fn set(&self, p: Parameter, v: f64) -> OSStatus {
        if p.validate(v).is_err() {
            return kAudioUnitErr_InvalidParameterValue;
        }
        let Some(_guard) = token(&self.control) else {
            return BUSY;
        };
        if self.values[index(p)].swap(v.to_bits(), Ordering::AcqRel) != v.to_bits() {
            self.preset_dirty.store(true, Ordering::Release);
        }
        0
    }
}
fn index(p: Parameter) -> usize {
    Parameter::ALL.iter().position(|v| *v == p).unwrap_or(0)
}
struct Configuration {
    rate: f64,
    maximum: u32,
    input: AURenderCallbackStruct,
    connection: AudioUnitConnection,
    host: HostCallbackInfo,
    offline: bool,
    allocate_input: bool,
    allocate_output: bool,
    preset_number: i32,
    preset_name: String,
    listeners: Vec<(u32, AudioUnitPropertyListenerProc, *mut c_void)>,
    notifications: Vec<(AURenderCallback, *mut c_void)>,
}
struct Engine {
    processor: Processor,
    state: PluginState,
    applied: u64,
    seen: [u64; Parameter::COUNT],
    input: [Vec<f32>; 2],
    output: [Vec<f32>; 2],
    stamp: Option<(u64, u32)>,
    next_frame: Option<f64>,
    scheduled: Vec<events::Event>,
    ramps: [Option<events::Event>; Parameter::COUNT],
    active_ramps: Vec<usize>,
}
#[repr(C)]
struct Instance {
    interface: AudioComponentPlugInInterface,
    shared: Arc<Shared>,
    gate: AtomicBool,
    config: UnsafeCell<Configuration>,
    engine: UnsafeCell<Option<Engine>>,
    events: Events,
    fault: AtomicBool,
    phase: AtomicU8,
    last_error: AtomicI32,
    #[cfg(test)]
    inject_fault: AtomicBool,
    #[cfg(test)]
    prepare_hook: UnsafeCell<Option<Arc<std::sync::Barrier>>>,
}
// Host pointers are only called under the native lifecycle and thread contracts.
// `gate` excludes overlapping topology/DSP access; parameter storage is atomic.
unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}
struct Token<'a>(&'a AtomicBool);
impl Drop for Token<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn token(value: &AtomicBool) -> Option<Token<'_>> {
    value
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .ok()
        .map(|_| Token(value))
}
unsafe fn instance<'a>(p: *mut c_void) -> Option<&'a Instance> {
    unsafe { p.cast::<Instance>().as_ref() }
}
/// # Safety
/// `desc` must point to a live C component description for the duration of this call.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DAWAudioUnitFactory(
    desc: *const AudioComponentDescription,
) -> *mut AudioComponentPlugInInterface {
    boundary::call(std::ptr::null_mut(), || {
        if !boundary::install() {
            return std::ptr::null_mut();
        }
        let Some(desc) = (unsafe { desc.as_ref() }) else {
            return std::ptr::null_mut();
        };
        if desc.componentType != daw_drive_plugin::AU_TYPE
            || desc.componentSubType != daw_drive_plugin::AU_SUBTYPE
            || desc.componentManufacturer != daw_drive_plugin::AU_MANUFACTURER
        {
            return std::ptr::null_mut();
        }
        editor::register();
        let value = Box::new(Instance {
            interface: AudioComponentPlugInInterface {
                Open: Some(open),
                Close: Some(close),
                Lookup: Some(lookup),
                reserved: std::ptr::null_mut(),
            },
            shared: Arc::new(Shared::new()),
            gate: AtomicBool::new(false),
            config: UnsafeCell::new(Configuration {
                rate: 48000.0,
                maximum: 4096,
                input: unsafe { std::mem::zeroed() },
                connection: unsafe { std::mem::zeroed() },
                host: unsafe { std::mem::zeroed() },
                offline: false,
                allocate_input: true,
                allocate_output: true,
                preset_number: 0,
                preset_name: "Default".into(),
                listeners: Vec::new(),
                notifications: Vec::new(),
            }),
            engine: UnsafeCell::new(None),
            events: Events::new(),
            fault: AtomicBool::new(false),
            phase: AtomicU8::new(0),
            last_error: AtomicI32::new(0),
            #[cfg(test)]
            inject_fault: AtomicBool::new(false),
            #[cfg(test)]
            prepare_hook: UnsafeCell::new(None),
        });
        Box::into_raw(value).cast()
    })
}
unsafe extern "C" fn open(p: *mut c_void, handle: AudioComponentInstance) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        if handle.is_null()
            || i.shared
                .handle
                .compare_exchange(0, handle as usize, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
        {
            return BAD;
        }
        0
    })
}
unsafe extern "C" fn close(p: *mut c_void) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        // A view may outlive Close. Do not retire the native handle while a
        // main-thread host notification is in flight. Contention never waits.
        let shared = i.shared.clone();
        let Some(_host_guard) = token(&shared.host_gate) else {
            return BUSY;
        };
        let Some(guard) = token(&i.gate) else {
            return BUSY;
        };
        editor::finish_gestures(&i.shared);
        i.shared.closed.store(true, Ordering::Release);
        i.shared.handle.store(0, Ordering::Release);
        std::mem::forget(guard); // Instance, including this guard's atomic, retires below.
        unsafe {
            drop(Box::from_raw(p.cast::<Instance>()));
        }
        0
    })
}
unsafe extern "C" fn initialize(p: *mut c_void) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_guard) = token(&i.gate) else {
            return BUSY;
        };
        let engine = unsafe { &mut *i.engine.get() };
        if engine.is_some() {
            return 0;
        }
        let c = unsafe { &*i.config.get() };
        let mut state = i.shared.exchange.current().unwrap_or_default();
        for p in Parameter::ALL {
            if state.set(p, i.shared.parameter(p)).is_err() {
                return BAD;
            }
        }
        #[cfg(test)]
        if let Some(barrier) = unsafe { &*i.prepare_hook.get() } {
            barrier.wait();
            barrier.wait();
        }
        let Ok(mut processor) = Processor::new(c.rate, 1, c.maximum, state) else {
            return BAD;
        };
        processor.set_output_meter(i.shared.model.output_meter.clone());
        *engine = Some(Engine {
            processor,
            state,
            applied: 0,
            seen: Parameter::ALL.map(|p| state.get(p).to_bits()),
            input: std::array::from_fn(|_| vec![0.0; c.maximum as usize]),
            output: std::array::from_fn(|_| vec![0.0; c.maximum as usize]),
            stamp: None,
            next_frame: None,
            scheduled: events::storage(),
            ramps: [None; Parameter::COUNT],
            active_ramps: Vec::with_capacity(Parameter::COUNT),
        });
        i.fault.store(false, Ordering::Release);
        0
    })
}
unsafe extern "C" fn uninitialize(p: *mut c_void) -> OSStatus {
    boundary::call(BUSY, || {
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        unsafe {
            *i.engine.get() = None;
        }
        i.events.clear();
        0
    })
}
unsafe extern "C" fn reset(p: *mut c_void, scope: u32, element: u32) -> OSStatus {
    boundary::call(BUSY, || {
        if element != 0
            || ![
                kAudioUnitScope_Global,
                kAudioUnitScope_Input,
                kAudioUnitScope_Output,
            ]
            .contains(&scope)
        {
            return kAudioUnitErr_InvalidScope;
        }
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Some(_g) = token(&i.gate) else {
            return BUSY;
        };
        if let Some(e) = unsafe { &mut *i.engine.get() } {
            e.processor.reset();
            e.stamp = None;
            e.next_frame = None;
        }
        i.events.clear();
        0
    })
}
unsafe extern "C" fn get_parameter(
    p: *mut c_void,
    id: u32,
    scope: u32,
    element: u32,
    out: *mut f32,
) -> OSStatus {
    boundary::call(BUSY, || {
        if scope != kAudioUnitScope_Global || element != 0 {
            return kAudioUnitErr_InvalidScope;
        }
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Ok(param) = Parameter::from_id(id) else {
            return kAudioUnitErr_InvalidParameter;
        };
        if out.is_null() {
            return BAD;
        }
        unsafe {
            out.write(i.shared.parameter(param) as f32);
        }
        0
    })
}
unsafe extern "C" fn set_parameter(
    p: *mut c_void,
    id: u32,
    scope: u32,
    element: u32,
    value: f32,
    offset: u32,
) -> OSStatus {
    boundary::call(BUSY, || {
        if scope != kAudioUnitScope_Global || element != 0 {
            return kAudioUnitErr_InvalidScope;
        }
        let Some(i) = (unsafe { instance(p) }) else {
            return BAD;
        };
        let Ok(param) = Parameter::from_id(id) else {
            return kAudioUnitErr_InvalidParameter;
        };
        let Some(value) = events::plain(param, value) else {
            return kAudioUnitErr_InvalidParameterValue;
        };
        if offset == 0 {
            i.shared.set(param, value)
        } else {
            if i.phase.load(Ordering::Acquire) >= 2 {
                return BUSY;
            }
            i.events.push_immediate(param, value, offset)
        }
    })
}
unsafe extern "C" fn lookup(selector: i16) -> AudioComponentMethod {
    // The C API deliberately erases selector signatures to a variadic function.
    // Every branch is checked against its generated typed alias at compilation.
    macro_rules! method {
        ($f:expr, $t:ty) => {{
            let typed: $t = Some($f);
            unsafe { std::mem::transmute::<$t, AudioComponentMethod>(typed) }
        }};
    }
    match selector as u32 {
        kAudioUnitInitializeSelect => method!(initialize, AudioUnitInitializeProc),
        kAudioUnitUninitializeSelect => method!(uninitialize, AudioUnitUninitializeProc),
        kAudioUnitGetPropertyInfoSelect => method!(properties::info, AudioUnitGetPropertyInfoProc),
        kAudioUnitGetPropertySelect => method!(properties::get, AudioUnitGetPropertyProc),
        kAudioUnitSetPropertySelect => method!(properties::set, AudioUnitSetPropertyProc),
        kAudioUnitGetParameterSelect => method!(get_parameter, AudioUnitGetParameterProc),
        kAudioUnitSetParameterSelect => method!(set_parameter, AudioUnitSetParameterProc),
        kAudioUnitScheduleParametersSelect => {
            method!(events::schedule, AudioUnitScheduleParametersProc)
        }
        kAudioUnitRenderSelect => method!(render::render, AudioUnitRenderProc),
        kAudioUnitResetSelect => method!(reset, AudioUnitResetProc),
        kAudioUnitAddPropertyListenerSelect => {
            method!(properties::add_listener, AudioUnitAddPropertyListenerProc)
        }
        kAudioUnitRemovePropertyListenerSelect => method!(
            properties::remove_listener,
            AudioUnitRemovePropertyListenerProc
        ),
        kAudioUnitRemovePropertyListenerWithUserDataSelect => method!(
            properties::remove_listener_data,
            AudioUnitRemovePropertyListenerWithUserDataProc
        ),
        kAudioUnitAddRenderNotifySelect => {
            method!(render::add_notify, AudioUnitAddRenderNotifyProc)
        }
        kAudioUnitRemoveRenderNotifySelect => {
            method!(render::remove_notify, AudioUnitRemoveRenderNotifyProc)
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests;
