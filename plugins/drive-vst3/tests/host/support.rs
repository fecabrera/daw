use daw_drive_plugin::{IDENTITY, Parameter as P, PluginState, SUPPORTED_RATES};
use daw_plugin_vst3::{
    class_id,
    sys::{
        Class, ComPtr, ComWrapper,
        Steinberg::{Vst::*, *},
    },
    write_utf16,
};
use daw_unit_core::{EffectUnit, ProcessingContext};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
static SERIAL: Mutex<()> = Mutex::new(());
type Begin = unsafe extern "C" fn();
type End = unsafe extern "C" fn(*mut u64, *mut u64);
struct Host(Arc<AtomicUsize>);
impl Class for Host {
    type Interfaces = (IHostApplication, IComponentHandler);
}
impl IHostApplicationTrait for Host {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        write_utf16("Drive VST3 ABI test host", unsafe { &mut *name });
        kResultOk
    }
    unsafe fn createInstance(&self, _: *mut TUID, _: *mut TUID, _: *mut *mut c_void) -> tresult {
        kNotImplemented
    }
}
impl IComponentHandlerTrait for Host {
    unsafe fn beginEdit(&self, _: u32) -> tresult {
        self.0.fetch_add(1, Ordering::Relaxed);
        kResultOk
    }
    unsafe fn performEdit(&self, _: u32, _: f64) -> tresult {
        self.0.fetch_add(1, Ordering::Relaxed);
        kResultOk
    }
    unsafe fn endEdit(&self, _: u32) -> tresult {
        self.0.fetch_add(1, Ordering::Relaxed);
        kResultOk
    }
    unsafe fn restartComponent(&self, _: i32) -> tresult {
        self.0.fetch_add(1, Ordering::Relaxed);
        kResultOk
    }
}
struct Stream {
    bytes: RefCell<Vec<u8>>,
    offset: Cell<usize>,
    stride: usize,
    fail: bool,
}
impl Stream {
    fn new(bytes: Vec<u8>, stride: usize) -> Self {
        Self {
            bytes: RefCell::new(bytes),
            offset: Cell::new(0),
            stride,
            fail: false,
        }
    }
}
impl Class for Stream {
    type Interfaces = (IBStream,);
}
impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, size: i32, count: *mut i32) -> tresult {
        if self.fail || size < 0 {
            return kInternalError;
        }
        let bytes = self.bytes.borrow();
        let available = bytes.len().saturating_sub(self.offset.get());
        let length = (size as usize).min(available).min(self.stride);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr().add(self.offset.get()), buffer.cast(), length);
            if !count.is_null() {
                count.write(length as i32);
            }
        }
        self.offset.set(self.offset.get() + length);
        if length == 0 { kResultFalse } else { kResultOk }
    }
    unsafe fn write(&self, buffer: *mut c_void, size: i32, count: *mut i32) -> tresult {
        if self.fail || size < 0 {
            return kInternalError;
        }
        let length = (size as usize).min(self.stride);
        let mut bytes = self.bytes.borrow_mut();
        let end = self.offset.get() + length;
        let size = bytes.len().max(end);
        bytes.resize(size, 0);
        unsafe {
            ptr::copy_nonoverlapping(
                buffer.cast::<u8>(),
                bytes.as_mut_ptr().add(self.offset.get()),
                length,
            );
            if !count.is_null() {
                count.write(length as i32);
            }
        }
        self.offset.set(end);
        kResultOk
    }
    unsafe fn seek(&self, position: i64, mode: i32, result: *mut i64) -> tresult {
        let base = match mode {
            0 => 0,
            1 => self.offset.get() as i64,
            2 => self.bytes.borrow().len() as i64,
            _ => return kInvalidArgument,
        };
        let Some(next) = base.checked_add(position).filter(|v| *v >= 0) else {
            return kInvalidArgument;
        };
        self.offset.set(next as usize);
        if !result.is_null() {
            unsafe {
                result.write(next);
            }
        }
        kResultOk
    }
    unsafe fn tell(&self, result: *mut i64) -> tresult {
        if result.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            result.write(self.offset.get() as i64);
        }
        kResultOk
    }
}
struct Queue {
    id: u32,
    points: Vec<(i32, f64)>,
}
impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> u32 {
        self.id
    }
    unsafe fn getPointCount(&self) -> i32 {
        self.points.len() as i32
    }
    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        if let Some((time, point)) = self.points.get(index as usize) {
            unsafe {
                offset.write(*time);
                value.write(*point);
            }
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn addPoint(&self, _: i32, _: f64, _: *mut i32) -> tresult {
        kNotImplemented
    }
}
struct Changes {
    queues: Vec<ComPtr<IParamValueQueue>>,
}
impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        self.queues.len() as i32
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        self.queues
            .get(index as usize)
            .map_or(ptr::null_mut(), ComPtr::as_ptr)
    }
    unsafe fn addParameterData(&self, _: *const u32, _: *mut i32) -> *mut IParamValueQueue {
        ptr::null_mut()
    }
}
fn changes(input: &[(P, Vec<(i32, f64)>)]) -> ComPtr<IParameterChanges> {
    let queues = input
        .iter()
        .map(|(parameter, points)| {
            ComWrapper::new(Queue {
                id: *parameter as u32,
                points: points.clone(),
            })
            .to_com_ptr::<IParamValueQueue>()
            .unwrap()
        })
        .collect();
    ComWrapper::new(Changes { queues })
        .to_com_ptr::<IParameterChanges>()
        .unwrap()
}
type Exit = unsafe extern "system" fn() -> bool;
struct Module {
    _library: libloading::Library,
    exit: Exit,
}
impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            (self.exit)();
        }
    }
}

struct Plugin {
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: ComPtr<IEditController>,
    _host: ComPtr<IHostApplication>,
    host_events: Arc<AtomicUsize>,
    _factory: ComPtr<IPluginFactory>,
    active: bool,
    processing: bool,
    counts: Option<(Begin, End)>,
    _library: Option<Module>,
}
impl Plugin {
    fn new() -> Self {
        let (factory, library, counts) =
            if let Some(path) = std::env::var_os("DRIVE_VST3_TEST_LIBRARY") {
                unsafe {
                    let library = libloading::Library::new(path).unwrap();
                    #[cfg(target_os = "macos")]
                    assert!((*library
                        .get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"bundleEntry\0")
                        .unwrap())(ptr::null_mut()));
                    #[cfg(target_os = "linux")]
                    assert!((*library
                        .get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"ModuleEntry\0")
                        .unwrap())(ptr::null_mut()));
                    #[cfg(target_os = "windows")]
                    assert!((*library
                        .get::<unsafe extern "system" fn() -> bool>(b"InitDll\0")
                        .unwrap())());
                    let exit_name = if cfg!(target_os = "macos") {
                        b"bundleExit\0".as_slice()
                    } else if cfg!(target_os = "windows") {
                        b"ExitDll\0".as_slice()
                    } else {
                        b"ModuleExit\0".as_slice()
                    };
                    let exit = *library.get::<Exit>(exit_name).unwrap();
                    let get = *library
                        .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(
                            b"GetPluginFactory\0",
                        )
                        .unwrap();
                    let factory = ComPtr::from_raw(get()).unwrap();
                    let counts = library
                        .get::<Begin>(b"drive_vst3_allocation_begin\0")
                        .ok()
                        .map(|begin| {
                            (
                                *begin,
                                *library.get::<End>(b"drive_vst3_allocation_end\0").unwrap(),
                            )
                        });
                    if std::env::var_os("DRIVE_VST3_REQUIRE_ALLOCATION_CHECK").is_some() {
                        assert!(counts.is_some());
                    }
                    if let Some((begin, end)) = counts {
                        let probe = *library
                            .get::<unsafe extern "C" fn()>(b"drive_vst3_allocation_probe\0")
                            .unwrap();
                        let (mut a, mut d) = (0, 0);
                        begin();
                        probe();
                        end(&mut a, &mut d);
                        assert!(a > 0 && d > 0);
                    }
                    (
                        factory,
                        Some(Module {
                            _library: library,
                            exit,
                        }),
                        counts,
                    )
                }
            } else {
                (
                    unsafe { ComPtr::from_raw(plugin_export::GetPluginFactory()).unwrap() },
                    None,
                    None,
                )
            };
        assert_eq!(unsafe { factory.countClasses() }, 2);
        let processor_id = class_id(IDENTITY.processor_id);
        let controller_id = class_id(IDENTITY.controller_id);
        let create = |class: TUID, iid: TUID| {
            let mut output = ptr::null_mut();
            assert_eq!(
                unsafe { factory.createInstance(class.as_ptr(), iid.as_ptr(), &mut output) },
                kResultOk
            );
            output
        };
        let component = unsafe {
            ComPtr::<IComponent>::from_raw(create(processor_id, IComponent_iid).cast()).unwrap()
        };
        let processor = component.cast::<IAudioProcessor>().unwrap();
        let controller = unsafe {
            ComPtr::<IEditController>::from_raw(create(controller_id, IEditController_iid).cast())
                .unwrap()
        };
        let events = Arc::new(AtomicUsize::new(0));
        let host = ComWrapper::new(Host(events.clone()));
        let app = host.to_com_ptr::<IHostApplication>().unwrap();
        let context = app.cast::<FUnknown>().unwrap();
        assert_eq!(unsafe { component.initialize(context.as_ptr()) }, kResultOk);
        assert_eq!(
            unsafe { controller.initialize(context.as_ptr()) },
            kResultOk
        );
        let handler = host.to_com_ptr::<IComponentHandler>().unwrap();
        assert_eq!(
            unsafe { controller.setComponentHandler(handler.as_ptr()) },
            kResultOk
        );
        Self {
            component,
            processor,
            controller,
            _host: app,
            host_events: events,
            _factory: factory,
            active: false,
            processing: false,
            counts,
            _library: library,
        }
    }
    fn activate(&mut self, rate: u32, frames: i32, mode: i32) {
        let mut setup = ProcessSetup {
            processMode: mode,
            symbolicSampleSize: 0,
            maxSamplesPerBlock: frames,
            sampleRate: rate as f64,
        };
        assert_eq!(
            unsafe { self.processor.setupProcessing(&mut setup) },
            kResultOk
        );
        for direction in [0, 1] {
            assert_eq!(
                unsafe { self.component.activateBus(0, direction, 0, 1) },
                kResultOk
            );
        }
        assert_eq!(unsafe { self.component.setActive(1) }, kResultOk);
        self.active = true;
        assert_eq!(unsafe { self.processor.setProcessing(1) }, kResultOk);
        self.processing = true;
    }
    fn stop(&mut self) {
        if self.processing {
            assert_eq!(unsafe { self.processor.setProcessing(0) }, kResultOk);
            self.processing = false;
        }
    }
    fn deactivate(&mut self) {
        self.stop();
        if self.active {
            assert_eq!(unsafe { self.component.setActive(0) }, kResultOk);
            self.active = false;
        }
    }
    fn measured<T>(&self, call: impl FnOnce() -> T) -> T {
        if let Some((begin, _)) = self.counts {
            unsafe {
                begin();
            }
        }
        let result = call();
        if let Some((_, end)) = self.counts {
            let (mut a, mut d) = (0, 0);
            unsafe {
                end(&mut a, &mut d);
            }
            assert_eq!((a, d), (0, 0), "audio callback heap operations");
        }
        result
    }
    fn load(&self, state: PluginState) {
        let stream = ComWrapper::new(Stream::new(state.encode().to_vec(), 1));
        let pointer = stream.to_com_ptr::<IBStream>().unwrap();
        assert_eq!(
            unsafe { self.component.setState(pointer.as_ptr()) },
            kResultOk
        );
        stream.offset.set(0);
        assert_eq!(
            unsafe { self.controller.setComponentState(pointer.as_ptr()) },
            kResultOk
        );
    }
    fn save(&self) -> Option<PluginState> {
        let stream = ComWrapper::new(Stream::new(vec![], 1));
        let pointer = stream.to_com_ptr::<IBStream>().unwrap();
        if unsafe { self.component.getState(pointer.as_ptr()) } != kResultOk {
            return None;
        }
        PluginState::decode(&stream.bytes.borrow()).ok()
    }
}
impl Drop for Plugin {
    fn drop(&mut self) {
        self.deactivate();
        unsafe {
            self.controller.setComponentHandler(ptr::null_mut());
            self.controller.terminate();
            self.component.terminate();
        }
        // COM pointers must be dropped while the DSO is live. Field order places
        // the optional library last, after all owned interfaces and host references.
    }
}
fn run(
    plugin: &Plugin,
    left: &mut [f32],
    right: &mut [f32],
    points: &[(P, Vec<(i32, f64)>)],
    mode: i32,
    in_place: bool,
    silence: u64,
) -> (Vec<f32>, Vec<f32>) {
    let queues = changes(points);
    let mut out_left = vec![f32::NAN; left.len()];
    let mut out_right = out_left.clone();
    let mut inputs = [left.as_mut_ptr(), right.as_mut_ptr()];
    let mut outputs = if in_place {
        inputs
    } else {
        [out_left.as_mut_ptr(), out_right.as_mut_ptr()]
    };
    let mut input = AudioBusBuffers {
        numChannels: 2,
        silenceFlags: silence,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: inputs.as_mut_ptr(),
        },
    };
    let mut output = AudioBusBuffers {
        numChannels: 2,
        silenceFlags: u64::MAX,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: outputs.as_mut_ptr(),
        },
    };
    let mut data = ProcessData {
        processMode: mode,
        symbolicSampleSize: 0,
        numSamples: left.len() as i32,
        numInputs: 1,
        numOutputs: 1,
        inputs: &mut input,
        outputs: &mut output,
        inputParameterChanges: queues.as_ptr(),
        outputParameterChanges: ptr::null_mut(),
        inputEvents: ptr::null_mut(),
        outputEvents: ptr::null_mut(),
        processContext: ptr::null_mut(),
    };
    assert_eq!(
        plugin.measured(|| unsafe { plugin.processor.process(&mut data) }),
        kResultOk
    );
    assert_eq!(output.silenceFlags, 0);
    if in_place {
        (left.to_vec(), right.to_vec())
    } else {
        (out_left, out_right)
    }
}
fn reference(
    drive: &mut Drive,
    state: &mut PluginState,
    normalized: &mut [f64; P::COUNT],
    left: &[f32],
    right: &[f32],
    points: &[(P, Vec<(i32, f64)>)],
) -> (Vec<f32>, Vec<f32>) {
    let previous = *normalized;
    let mut output = (vec![], vec![]);
    for frame in 0..left.len() {
        for (parameter, points) in points {
            let before = points
                .iter()
                .rev()
                .find(|(offset, _)| *offset <= frame as i32)
                .copied()
                .unwrap_or((-1, previous[*parameter as usize - 100]));
            let after = points
                .iter()
                .find(|(offset, _)| *offset > frame as i32)
                .copied();
            let value = if let Some(after) = after {
                if before.1 == after.1 {
                    before.1
                } else {
                    before.1
                        + (after.1 - before.1) * (frame as f64 - before.0 as f64)
                            / (after.0 - before.0) as f64
                }
            } else {
                before.1
            };
            let slot = *parameter as usize - 100;
            if value != normalized[slot] {
                normalized[slot] = value;
                state.set_normalized(*parameter, value).unwrap();
                drive.retarget(&state.to_settings());
            }
        }
        let sample = drive.process_sample([left[frame], right[frame]]);
        output.0.push(sample[0]);
        output.1.push(sample[1]);
    }
    output
}
fn normalized(state: PluginState) -> [f64; P::COUNT] {
    P::ALL.map(|p| p.normalize(state.get(p)).unwrap())
}
fn assert_samples(actual: &(Vec<f32>, Vec<f32>), expected: &(Vec<f32>, Vec<f32>)) {
    for (a, b) in actual
        .0
        .iter()
        .chain(&actual.1)
        .zip(expected.0.iter().chain(&expected.1))
    {
        assert_eq!(a.to_bits(), b.to_bits(), "{a} != {b}");
    }
}

fn enable_color(state: &mut PluginState) {
    if let Some(p) = P::ALL.into_iter().find(|p| p.info().key == "color_enabled") { state.set(p, 1.0).unwrap(); }
}
