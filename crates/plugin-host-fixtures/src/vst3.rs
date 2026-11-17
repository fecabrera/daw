#![allow(non_snake_case)] // SDK ABI names.
use super::*;
use daw_plugin_vst3::{
    Stream, class_id,
    sys::{
        Class, ComWrapper,
        Steinberg::{Vst::*, *},
        com_scrape_types::Unknown,
    },
    write_text, write_utf16,
};
use std::{
    ffi::c_void,
    io::{Read, Write},
    ptr,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
};
const EFFECT: TUID = class_id(EFFECT_VST3_ID);
const INSTRUMENT: TUID = class_id(INSTRUMENT_VST3_ID);
struct Component {
    processor: Processor,
    mode: AtomicI32,
    stereo: AtomicBool,
}
impl Class for Component {
    type Interfaces = (IComponent, IAudioProcessor, IEditController);
}
impl IPluginBaseTrait for Component {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}
impl Component {
    unsafe fn read_state(&self, pointer: *mut IBStream) -> tresult {
        let Some(mut stream) = (unsafe { Stream::new(pointer) }) else {
            return kInvalidArgument;
        };
        let mut bytes = [0; 16];
        if stream.read_exact(&mut bytes).is_ok() && self.processor.restore(bytes) {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn write_state(&self, pointer: *mut IBStream) -> tresult {
        let Some(mut stream) = (unsafe { Stream::new(pointer) }) else {
            return kInvalidArgument;
        };
        if stream.write_all(&self.processor.state()).is_ok() {
            kResultOk
        } else {
            kResultFalse
        }
    }
}
impl IComponentTrait for Component {
    unsafe fn getControllerClassId(&self, _: *mut TUID) -> tresult {
        kResultFalse
    }
    unsafe fn setIoMode(&self, _: IoMode) -> tresult {
        kResultOk
    }
    unsafe fn getBusCount(&self, media: MediaType, direction: BusDirection) -> i32 {
        if media == 0 {
            if direction == 0 && !self.processor.instrument {
                2
            } else {
                1
            }
        } else if media == 1 && direction == 0 && self.processor.instrument {
            1
        } else {
            0
        }
    }
    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        direction: BusDirection,
        index: i32,
        out: *mut BusInfo,
    ) -> tresult {
        if out.is_null() || index < 0 || index >= unsafe { self.getBusCount(media, direction) } {
            return kInvalidArgument;
        }
        let main = media == 1 || direction == 1 || !self.processor.instrument && index == 0;
        let mut info: BusInfo = unsafe { std::mem::zeroed() };
        info.mediaType = media;
        info.direction = direction;
        info.channelCount = if media == 1 {
            16
        } else if self.stereo.load(Ordering::Acquire) {
            2
        } else {
            6
        };
        info.busType = if main { 0 } else { 1 };
        info.flags = 1;
        write_utf16(
            if media == 1 {
                "Notes"
            } else if main {
                "Main"
            } else {
                "Sidechain"
            },
            &mut info.name,
        );
        unsafe { out.write(info) };
        kResultOk
    }
    unsafe fn getRoutingInfo(&self, _: *mut RoutingInfo, _: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }
    unsafe fn activateBus(&self, _: MediaType, _: BusDirection, _: i32, _: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setActive(&self, _: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setState(&self, stream: *mut IBStream) -> tresult {
        unsafe { self.read_state(stream) }
    }
    unsafe fn getState(&self, stream: *mut IBStream) -> tresult {
        unsafe { self.write_state(stream) }
    }
}
impl IAudioProcessorTrait for Component {
    unsafe fn setBusArrangements(
        &self,
        input: *mut SpeakerArrangement,
        inputs: i32,
        output: *mut SpeakerArrangement,
        outputs: i32,
    ) -> tresult {
        if inputs != if self.processor.instrument { 1 } else { 2 }
            || outputs != 1
            || input.is_null()
            || output.is_null()
        {
            return kResultFalse;
        }
        if unsafe { std::slice::from_raw_parts(input, inputs as usize) }
            .iter()
            .all(|a| *a == SpeakerArr::kStereo)
            && unsafe { *output } == SpeakerArr::kStereo
        {
            self.stereo.store(true, Ordering::Release);
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn getBusArrangement(
        &self,
        direction: BusDirection,
        index: i32,
        out: *mut SpeakerArrangement,
    ) -> tresult {
        if out.is_null() || index < 0 || index >= unsafe { self.getBusCount(0, direction) } {
            return kInvalidArgument;
        }
        unsafe {
            out.write(if self.stereo.load(Ordering::Acquire) {
                SpeakerArr::kStereo
            } else {
                SpeakerArr::k51
            })
        };
        kResultOk
    }
    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == 0 { kResultOk } else { kResultFalse }
    }
    unsafe fn getLatencySamples(&self) -> u32 {
        LATENCY as u32
    }
    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if setup.is_null() {
            return kInvalidArgument;
        }
        self.mode
            .store(unsafe { (*setup).processMode }, Ordering::Release);
        kResultOk
    }
    unsafe fn setProcessing(&self, _: TBool) -> tresult {
        kResultOk
    }
    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        if data.is_null() {
            return kInvalidArgument;
        }
        let data = unsafe { &mut *data };
        if data.processMode != self.mode.load(Ordering::Acquire) {
            return kResultFalse;
        }
        let changes = unsafe { daw_plugin_vst3::sys::ComRef::from_raw(data.inputParameterChanges) };
        let notes = unsafe { daw_plugin_vst3::sys::ComRef::from_raw(data.inputEvents) };
        let process_events = |frame: i32| {
            if let Some(changes) = changes {
                for queue in 0..unsafe { changes.getParameterCount() } {
                    if let Some(queue) = unsafe {
                        daw_plugin_vst3::sys::ComRef::from_raw(changes.getParameterData(queue))
                    } {
                        for point in 0..unsafe { queue.getPointCount() } {
                            let mut offset = 0;
                            let mut value = 0.0;
                            if unsafe { queue.getPoint(point, &mut offset, &mut value) }
                                == kResultOk
                                && offset == frame
                                && unsafe { queue.getParameterId() } == 10
                            {
                                self.processor.set_gain(value * 2.0);
                            }
                        }
                    }
                }
            }
            if let Some(notes) = notes {
                for index in 0..unsafe { notes.getEventCount() } {
                    let mut event: Event = unsafe { std::mem::zeroed() };
                    if unsafe { notes.getEvent(index, &mut event) } == kResultOk
                        && event.sampleOffset == frame
                    {
                        if event.r#type == 0 {
                            let note = unsafe { event.__field0.noteOn };
                            unsafe { &mut *self.processor.audio.get() }.note(
                                true,
                                note.noteId,
                                note.pitch,
                                note.channel,
                                f64::from(note.velocity),
                            );
                        } else if event.r#type == 1 {
                            let note = unsafe { event.__field0.noteOff };
                            unsafe { &mut *self.processor.audio.get() }.note(
                                false,
                                note.noteId,
                                note.pitch,
                                note.channel,
                                0.0,
                            );
                        }
                    }
                }
            }
        };
        if data.numSamples == 0 {
            process_events(0);
            return kResultOk;
        }
        for frame in 0..data.numSamples {
            process_events(frame);
            let bus = |index: usize| {
                let input = unsafe { &*data.inputs.add(index) };
                std::array::from_fn(|ch| unsafe {
                    *(*input.__field0.channelBuffers32.add(ch)).add(frame as usize)
                })
            };
            let main = if self.processor.instrument {
                [0.0; 2]
            } else {
                bus(0)
            };
            let aux = bus(usize::from(!self.processor.instrument));
            let output = unsafe { &mut *self.processor.audio.get() }.sample(
                self.processor.instrument,
                self.processor.gain(),
                main,
                aux,
            );
            for (ch, sample) in output.into_iter().enumerate() {
                unsafe {
                    *(*(*data.outputs).__field0.channelBuffers32.add(ch)).add(frame as usize) =
                        sample;
                }
            }
        }
        kResultOk
    }
    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}
impl IEditControllerTrait for Component {
    unsafe fn setComponentState(&self, _: *mut IBStream) -> tresult {
        // A combined processor/controller already restored its state through
        // IComponent. Exercise hosts that incorrectly reject this SDK result.
        kNotImplemented
    }
    unsafe fn setState(&self, stream: *mut IBStream) -> tresult {
        unsafe { self.read_state(stream) }
    }
    unsafe fn getState(&self, stream: *mut IBStream) -> tresult {
        unsafe { self.write_state(stream) }
    }
    unsafe fn getParameterCount(&self) -> i32 {
        1
    }
    unsafe fn getParameterInfo(&self, index: i32, out: *mut ParameterInfo) -> tresult {
        if index != 0 || out.is_null() {
            return kInvalidArgument;
        }
        let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
        info.id = 10;
        info.defaultNormalizedValue = 0.5;
        info.flags = 1;
        write_utf16("Gain", &mut info.title);
        unsafe { out.write(info) };
        kResultOk
    }
    unsafe fn getParamStringByValue(&self, id: u32, value: f64, out: *mut String128) -> tresult {
        if id != 10 || out.is_null() {
            return kInvalidArgument;
        }
        write_utf16(&format!("{:.3}", value * 2.0), unsafe { &mut *out });
        kResultOk
    }
    unsafe fn getParamValueByString(&self, _: u32, _: *mut TChar, _: *mut f64) -> tresult {
        kNotImplemented
    }
    unsafe fn normalizedParamToPlain(&self, _: u32, value: f64) -> f64 {
        value * 2.0
    }
    unsafe fn plainParamToNormalized(&self, _: u32, value: f64) -> f64 {
        value / 2.0
    }
    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        if id == 10 {
            self.processor.gain() / 2.0
        } else {
            0.0
        }
    }
    unsafe fn setParamNormalized(&self, id: u32, value: f64) -> tresult {
        if id != 10 || !value.is_finite() {
            return kInvalidArgument;
        }
        self.processor.set_gain(value * 2.0);
        kResultOk
    }
    unsafe fn setComponentHandler(&self, _: *mut IComponentHandler) -> tresult {
        kResultOk
    }
    unsafe fn createView(&self, _: FIDString) -> *mut IPlugView {
        ptr::null_mut()
    }
}
struct Factory;
impl Class for Factory {
    type Interfaces = (IPluginFactory2,);
}
impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, out: *mut PFactoryInfo) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        let mut info: PFactoryInfo = unsafe { std::mem::zeroed() };
        write_text("DAW Tests", &mut info.vendor);
        unsafe { out.write(info) };
        kResultOk
    }
    unsafe fn countClasses(&self) -> i32 {
        2
    }
    unsafe fn getClassInfo(&self, index: i32, out: *mut PClassInfo) -> tresult {
        if out.is_null() || !(0..2).contains(&index) {
            return kInvalidArgument;
        }
        let mut info: PClassInfo = unsafe { std::mem::zeroed() };
        info.cid = if index == 0 { EFFECT } else { INSTRUMENT };
        info.cardinality = i32::MAX;
        write_text("Audio Module Class", &mut info.category);
        write_text(
            if index == 0 {
                "Host Fixture Effect"
            } else {
                "Host Fixture Instrument"
            },
            &mut info.name,
        );
        unsafe { out.write(info) };
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        class: FIDString,
        interface: FIDString,
        out: *mut *mut c_void,
    ) -> tresult {
        if class.is_null() || interface.is_null() || out.is_null() {
            return kInvalidArgument;
        }
        unsafe { out.write(ptr::null_mut()) };
        let class = unsafe { ptr::read_unaligned(class.cast::<TUID>()) };
        let iid = unsafe { ptr::read_unaligned(interface.cast::<[u8; 16]>()) };
        if class != EFFECT && class != INSTRUMENT {
            return kInvalidArgument;
        }
        let component = ComWrapper::new(Component {
            processor: Processor::new(class == INSTRUMENT),
            mode: AtomicI32::new(0),
            stereo: AtomicBool::new(false),
        })
        .to_com_ptr::<IComponent>()
        .expect("declared fixture component");
        if let Some(pointer) = unsafe { IComponent::query_interface(component.as_ptr(), &iid) } {
            unsafe { out.write(pointer) };
            kResultOk
        } else {
            kNoInterface
        }
    }
}
impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: i32, out: *mut PClassInfo2) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        let mut base: PClassInfo = unsafe { std::mem::zeroed() };
        let status = unsafe { self.getClassInfo(index, &mut base) };
        if status != kResultOk {
            return status;
        }
        let mut info: PClassInfo2 = unsafe { std::mem::zeroed() };
        info.cid = base.cid;
        info.cardinality = base.cardinality;
        info.category = base.category;
        info.name = base.name;
        write_text(
            if index == 0 {
                "Fx|Dynamics"
            } else {
                "Instrument|Synth"
            },
            &mut info.subCategories,
        );
        write_text("DAW Tests", &mut info.vendor);
        write_text("1", &mut info.version);
        unsafe { out.write(info) };
        kResultOk
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory)
        .to_com_ptr::<IPluginFactory>()
        .expect("declared fixture factory")
        .into_raw()
}
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn bundleEntry(_: *mut c_void) -> bool {
    true
}
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn bundleExit() -> bool {
    true
}
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "system" fn InitDll() -> bool {
    true
}
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "system" fn ExitDll() -> bool {
    true
}
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
pub extern "C" fn ModuleEntry(_: *mut c_void) -> bool {
    true
}
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
pub extern "C" fn ModuleExit() -> bool {
    true
}
