use crate::CONTROLLER_ID;
use daw_drive_plugin::{Parameter, PluginState, Processor, SUPPORTED_RATES, StateExchange};
use daw_plugin_vst3::{
    Stream, boundary,
    sys::{
        Class,
        Steinberg::{Vst::*, *},
    },
    write_utf16,
};
use std::{
    cell::{Cell, UnsafeCell},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

pub(crate) struct Audio {
    pub processor: Option<Processor>,
    pub state: PluginState,
    pub applied: u64,
    pub normalized: [f64; 7],
}
impl Default for Audio {
    fn default() -> Self {
        let state = PluginState::default();
        Self {
            processor: None,
            state,
            applied: 0,
            normalized: Parameter::ALL.map(|p| p.normalize(state.get(p)).unwrap_or(0.0)),
        }
    }
}
pub(crate) struct Component {
    pub initialized: AtomicBool,
    pub active: AtomicBool,
    pub processing: AtomicBool,
    pub rate: AtomicU32,
    pub setup: Cell<Option<ProcessSetup>>,
    pub input_active: Cell<bool>,
    pub output_active: Cell<bool>,
    pub exchange: StateExchange,
    audio: UnsafeCell<Audio>,
}
impl Default for Component {
    fn default() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            rate: AtomicU32::new(48000),
            setup: Cell::new(None),
            input_active: Cell::new(false),
            output_active: Cell::new(false),
            exchange: StateExchange::default(),
            audio: UnsafeCell::new(Audio::default()),
        }
    }
}
impl Class for Component {
    type Interfaces = (IComponent, IAudioProcessor, IProcessContextRequirements);
}
impl Component {
    /// # Safety
    /// SDK main-inactive / processing-thread callbacks serialize mutable DSP access.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn audio(&self) -> &mut Audio {
        unsafe { &mut *self.audio.get() }
    }
    pub fn recall(&self, audio: &mut Audio) {
        if let Some(state) = self.exchange.consume(&mut audio.applied) {
            audio.state = state;
            audio.normalized = Parameter::ALL.map(|p| p.normalize(state.get(p)).unwrap_or(0.0));
            if let Some(processor) = &mut audio.processor {
                processor.restore(state);
            }
        }
    }
    pub fn publish(&self, audio: &Audio) {
        self.exchange.publish(audio.state, audio.applied);
    }
}
impl IPluginBaseTrait for Component {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        if self.initialized.swap(true, Ordering::AcqRel) {
            kResultFalse
        } else {
            kResultOk
        }
    }
    unsafe fn terminate(&self) -> tresult {
        boundary(kInternalError, || {
            if self.active.load(Ordering::Acquire) {
                return kResultFalse;
            }
            self.setup.set(None);
            self.initialized.store(false, Ordering::Release);
            kResultOk
        })
    }
}
impl IComponentTrait for Component {
    unsafe fn getControllerClassId(&self, output: *mut TUID) -> tresult {
        if output.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            output.write(CONTROLLER_ID);
        }
        kResultOk
    }
    unsafe fn setIoMode(&self, mode: IoMode) -> tresult {
        if (0..=2).contains(&mode) {
            kResultOk
        } else {
            kInvalidArgument
        }
    }
    unsafe fn getBusCount(&self, kind: MediaType, direction: BusDirection) -> i32 {
        i32::from(kind == 0 && (0..=1).contains(&direction))
    }
    unsafe fn getBusInfo(
        &self,
        kind: MediaType,
        direction: BusDirection,
        index: i32,
        output: *mut BusInfo,
    ) -> tresult {
        boundary(kInternalError, || {
            if kind != 0 || !(0..=1).contains(&direction) || index != 0 || output.is_null() {
                return kInvalidArgument;
            }
            let mut bus: BusInfo = unsafe { std::mem::zeroed() };
            bus.mediaType = 0;
            bus.direction = direction;
            bus.channelCount = 2;
            bus.busType = 0;
            bus.flags = BusInfo_::BusFlags_::kDefaultActive;
            write_utf16(
                if direction == 0 {
                    "Stereo input"
                } else {
                    "Stereo output"
                },
                &mut bus.name,
            );
            unsafe {
                output.write(bus);
            }
            kResultOk
        })
    }
    unsafe fn getRoutingInfo(&self, _: *mut RoutingInfo, _: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }
    unsafe fn activateBus(
        &self,
        kind: MediaType,
        direction: BusDirection,
        index: i32,
        state: TBool,
    ) -> tresult {
        if self.active.load(Ordering::Acquire)
            || kind != 0
            || index != 0
            || !(0..=1).contains(&direction)
        {
            return kInvalidArgument;
        }
        if direction == 0 {
            self.input_active.set(state != 0);
        } else {
            self.output_active.set(state != 0);
        }
        kResultOk
    }
    unsafe fn setActive(&self, state: TBool) -> tresult {
        boundary(kInternalError, || {
            if !self.initialized.load(Ordering::Acquire) {
                return kNotInitialized;
            }
            let active = state != 0;
            if self.active.load(Ordering::Acquire) == active {
                return kResultOk;
            }
            if self.processing.load(Ordering::Acquire) {
                return kResultFalse;
            }
            // SAFETY: SDK setActive runs on UI while no processing callback runs.
            let audio = unsafe { self.audio() };
            self.recall(audio);
            if active {
                let Some(setup) = self.setup.get() else {
                    return kResultFalse;
                };
                let Ok(processor) = Processor::new(
                    setup.sampleRate,
                    1,
                    setup.maxSamplesPerBlock as u32,
                    audio.state,
                ) else {
                    return kResultFalse;
                };
                audio.processor = Some(processor);
            } else {
                audio.processor = None;
            }
            self.publish(audio);
            self.active.store(active, Ordering::Release);
            kResultOk
        })
    }
    unsafe fn setState(&self, stream: *mut IBStream) -> tresult {
        boundary(kInternalError, || {
            let Some(mut stream) = (unsafe { Stream::new(stream) }) else {
                return kInvalidArgument;
            };
            let Ok(state) = PluginState::read_from(&mut stream) else {
                return kResultFalse;
            };
            self.exchange.recall(state);
            if !self.active.load(Ordering::Acquire) {
                let audio = unsafe { self.audio() };
                self.recall(audio);
                self.publish(audio);
            }
            kResultOk
        })
    }
    unsafe fn getState(&self, stream: *mut IBStream) -> tresult {
        boundary(kInternalError, || {
            let Some(mut stream) = (unsafe { Stream::new(stream) }) else {
                return kInvalidArgument;
            };
            let Some(state) = self.exchange.current() else {
                return kResultFalse;
            };
            if state.write_to(&mut stream).is_ok() {
                kResultOk
            } else {
                kResultFalse
            }
        })
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
        if self.active.load(Ordering::Acquire)
            || inputs != 1
            || outputs != 1
            || input.is_null()
            || output.is_null()
        {
            return kResultFalse;
        }
        if unsafe { *input == SpeakerArr::kStereo && *output == SpeakerArr::kStereo } {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn getBusArrangement(
        &self,
        direction: BusDirection,
        index: i32,
        output: *mut SpeakerArrangement,
    ) -> tresult {
        if !(0..=1).contains(&direction) || index != 0 || output.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            output.write(SpeakerArr::kStereo);
        }
        kResultOk
    }
    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == 0 { kResultOk } else { kResultFalse }
    }
    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }
    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        boundary(kInternalError, || {
            if setup.is_null() || self.active.load(Ordering::Acquire) {
                return kInvalidArgument;
            }
            let setup = unsafe { *setup };
            if setup.symbolicSampleSize != 0
                || !(0..=2).contains(&setup.processMode)
                || setup.maxSamplesPerBlock < 1
                || !SUPPORTED_RATES
                    .iter()
                    .any(|r| f64::from(*r) == setup.sampleRate)
            {
                return kResultFalse;
            }
            self.setup.set(Some(setup));
            self.rate.store(setup.sampleRate as u32, Ordering::Release);
            kResultOk
        })
    }
    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        if !self.active.load(Ordering::Acquire) {
            return kResultFalse;
        }
        self.processing.store(state != 0, Ordering::Release);
        kResultOk
    }
    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        boundary(kInternalError, || unsafe {
            crate::process::process(self, data)
        })
    }
    unsafe fn getTailSamples(&self) -> u32 {
        self.rate.load(Ordering::Acquire)
    }
}

impl IProcessContextRequirementsTrait for Component {
    unsafe fn getProcessContextRequirements(&self) -> u32 {
        0
    }
}
