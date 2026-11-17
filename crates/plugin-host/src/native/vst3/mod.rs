#![allow(non_snake_case)] // SDK ABI names.
mod connections;
mod layout;
mod messages;
mod queues;
#[cfg(target_os = "linux")]
mod run_loop;
mod stream;
use super::buffers::Planar;
use super::*;
use crate::scan::VstModule;
use daw_plugin_vst3::{
    class_id,
    sys::{
        Class, ComPtr, ComRef, ComWrapper,
        Steinberg::{Vst::*, *},
    },
    write_utf16,
};
use daw_unit_core::{HostedParameter, HostedParameterDomain};
use std::{ffi::c_void, ptr};
use stream::StateStream;
struct Bridge {
    shared: *const Shared,
    messages: messages::Pool,
    #[cfg(target_os = "linux")]
    run_loop: run_loop::RunLoop,
}
impl Bridge {
    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }
}
impl Class for Bridge {
    #[cfg(not(target_os = "linux"))]
    type Interfaces = (IHostApplication, IComponentHandler, IPlugFrame);
    #[cfg(target_os = "linux")]
    type Interfaces = (
        IHostApplication,
        IComponentHandler,
        IPlugFrame,
        daw_plugin_vst3::sys::Steinberg::Linux::IRunLoop,
    );
}
impl IHostApplicationTrait for Bridge {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        write_utf16("DAW", unsafe { &mut *name });
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        class: *mut TUID,
        interface: *mut TUID,
        out: *mut *mut c_void,
    ) -> tresult {
        unsafe { self.messages.create(class, interface, out) }
    }
}
impl IComponentHandlerTrait for Bridge {
    unsafe fn beginEdit(&self, id: u32) -> tresult {
        self.shared().emit(NativeNotification::Begin(id));
        kResultOk
    }
    unsafe fn performEdit(&self, id: u32, value: f64) -> tresult {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return kInvalidArgument;
        }
        self.shared().emit(NativeNotification::Value(id, value));
        self.shared().dirty.store(true, Ordering::Release);
        kResultOk
    }
    unsafe fn endEdit(&self, id: u32) -> tresult {
        self.shared().emit(NativeNotification::End(id));
        kResultOk
    }
    unsafe fn restartComponent(&self, flags: i32) -> tresult {
        if flags & 5 != 0 {
            self.shared().values_changed.store(true, Ordering::Release);
        }
        // Parameter value rescans preserve the active processor and its voices.
        let structural = flags as u32 & !4;
        if structural != 0 {
            self.shared()
                .restart
                .fetch_or(structural, Ordering::Release);
        }
        kResultOk
    }
}
impl IPlugFrameTrait for Bridge {
    unsafe fn resizeView(&self, _: *mut IPlugView, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        let r = unsafe { &*size };
        let w = i64::from(r.right) - i64::from(r.left);
        let h = i64::from(r.bottom) - i64::from(r.top);
        if !(1..=16384).contains(&w) || !(1..=16384).contains(&h) {
            return kInvalidArgument;
        }
        self.shared()
            .emit(NativeNotification::Resize(w as u32, h as u32));
        kResultOk
    }
}
fn check(status: tresult, operation: &str) -> Result<()> {
    if status == kResultOk {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "VST3 {operation} failed ({status})"
        )))
    }
}
fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}

pub(super) struct VstInstance {
    view: Option<ComPtr<IPlugView>>,
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: Option<ComPtr<IEditController>>,
    initialized: bool,
    controller_initialized: bool,
    connections: Option<connections::Connections>,
    active: bool,
    bridge: ComWrapper<Bridge>,
    _factory: ComPtr<IPluginFactory>,
    _module: Rc<VstModule>,
}
impl VstInstance {
    pub unsafe fn create(
        module: Rc<VstModule>,
        id: [u8; 16],
        shared: &Shared,
        configuration: NativeConfiguration,
        state: Option<&NativeState>,
        commands: rtrb::Consumer<NativeEvent>,
    ) -> Result<(Self, VstAudio, NativeMetadata)> {
        let rate = configuration.sample_rate_hz;
        let maximum = configuration.max_block_frames;
        let mode = if configuration.offline { 2 } else { 0 };
        let factory = unsafe {
            ComPtr::from_raw(module
                .library
                .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(
                b"GetPluginFactory\0",
            )?())
        }
        .ok_or_else(|| Error::Invalid("Null VST3 factory".into()))?;
        let mut raw = ptr::null_mut();
        let uid = class_id(id);
        check(
            unsafe { factory.createInstance(uid.as_ptr(), IComponent_iid.as_ptr(), &mut raw) },
            "component creation",
        )?;
        let component = unsafe { ComPtr::<IComponent>::from_raw(raw.cast()) }
            .ok_or_else(|| Error::Invalid("Null VST3 component".into()))?;
        let processor = component
            .cast::<IAudioProcessor>()
            .ok_or_else(|| Error::Invalid("VST3 component has no processor".into()))?;
        let mut instance = Self {
            view: None,
            component,
            processor,
            controller: None,
            initialized: false,
            controller_initialized: false,
            connections: None,
            active: false,
            bridge: ComWrapper::new(Bridge {
                shared,
                messages: messages::Pool::new(),
                #[cfg(target_os = "linux")]
                run_loop: run_loop::RunLoop::new(),
            }),
            _factory: factory,
            _module: module,
        };
        let host = instance
            .bridge
            .to_com_ptr::<IHostApplication>()
            .expect("declared host application");
        let context = host.cast::<FUnknown>().expect("COM base");
        check(
            unsafe { instance.component.initialize(context.as_ptr()) },
            "initialize",
        )?;
        instance.initialized = true;
        instance.controller = instance.component.cast::<IEditController>();
        if instance.controller.is_none() {
            let mut cid: TUID = [0; 16];
            if unsafe { instance.component.getControllerClassId(&mut cid) } == kResultOk {
                let mut raw = ptr::null_mut();
                check(
                    unsafe {
                        instance._factory.createInstance(
                            cid.as_ptr(),
                            IEditController_iid.as_ptr(),
                            &mut raw,
                        )
                    },
                    "controller creation",
                )?;
                let controller = unsafe { ComPtr::<IEditController>::from_raw(raw.cast()) }
                    .ok_or_else(|| Error::Invalid("Null VST3 controller".into()))?;
                check(
                    unsafe { controller.initialize(context.as_ptr()) },
                    "controller initialize",
                )?;
                instance.controller_initialized = true;
                instance.controller = Some(controller);
            }
        }
        if let Some(controller) = &instance.controller {
            let handler = instance
                .bridge
                .to_com_ptr::<IComponentHandler>()
                .expect("declared component handler");
            check(
                unsafe { controller.setComponentHandler(handler.as_ptr()) },
                "component handler",
            )?;
            if let (Some(a), Some(b)) = (
                instance.component.cast::<IConnectionPoint>(),
                controller.cast::<IConnectionPoint>(),
            ) && a.as_ptr() != b.as_ptr()
            {
                instance.connections = Some(connections::Connections::connect(a, b, shared)?);
            }
        }
        if let Some(state) = state {
            instance.restore(state)?;
        }
        instance.negotiate_audio_layout()?;
        let parameters = instance.parameters()?;
        let inputs = instance.ports(true)?;
        let outputs = instance.ports(false)?;
        if outputs.len() != 1 {
            return Err(Error::Invalid(
                "VST3 requires one mono/stereo output bus".into(),
            ));
        }
        check(
            unsafe { instance.processor.canProcessSampleSize(0) },
            "float32 support",
        )?;
        let mut setup = ProcessSetup {
            processMode: mode,
            symbolicSampleSize: 0,
            maxSamplesPerBlock: maximum as i32,
            sampleRate: f64::from(rate),
        };
        check(
            unsafe { instance.processor.setupProcessing(&mut setup) },
            "processing setup",
        )?;
        for (direction, ports) in [(0, &inputs), (1, &outputs)] {
            for n in 0..ports.len() {
                check(
                    unsafe { instance.component.activateBus(0, direction, n as i32, 1) },
                    "audio bus activation",
                )?;
            }
        }
        let note_count = unsafe { instance.component.getBusCount(1, 0) };
        if !(0..=1).contains(&note_count) {
            return Err(Error::Invalid(
                "VST3 requires unsupported note inputs".into(),
            ));
        }
        if note_count == 1 {
            check(
                unsafe { instance.component.activateBus(1, 0, 0, 1) },
                "note input activation",
            )?;
        }
        check(unsafe { instance.component.setActive(1) }, "activation")?;
        instance.active = true;
        let metadata = NativeMetadata {
            sample_rate_hz: rate,
            parameters,
            inputs,
            outputs,
            latency_frames: unsafe { instance.processor.getLatencySamples() },
            tail_frames: u64::from(unsafe { instance.processor.getTailSamples() }),
            state_supported: true,
            editor_supported: instance.controller.is_some(),
        };
        let mut audio = VstAudio::new(
            instance.processor.as_ptr(),
            shared,
            &metadata,
            rate,
            maximum,
            commands,
            note_count != 0,
        );
        if let Some(mapping) = instance
            .controller
            .as_ref()
            .and_then(|c| c.cast::<IMidiMapping>())
        {
            for channel in 0..16 {
                for number in 0..130 {
                    let mut id = 0;
                    if unsafe {
                        mapping.getMidiControllerAssignment(
                            0,
                            channel as i16,
                            number as i16,
                            &mut id,
                        )
                    } == kResultOk
                        && metadata
                            .parameters
                            .iter()
                            .any(|p| p.id == id && !p.read_only)
                    {
                        audio.midi_mapping[channel * 130 + number] = Some(id);
                    }
                }
            }
        }
        audio.mode = mode;
        audio.messages = instance
            .connections
            .as_ref()
            .map_or(ptr::null(), |c| c.audio_pointer());
        Ok((instance, audio, metadata))
    }
    fn parameters(&self) -> Result<Vec<HostedParameter>> {
        let Some(controller) = &self.controller else {
            return Ok(Vec::new());
        };
        let count = unsafe { controller.getParameterCount() };
        if !(0..=4096).contains(&count) {
            return Err(Error::Invalid("VST3 parameter count exceeds 4096".into()));
        }
        let mut parameters = Vec::with_capacity(count as usize);
        for n in 0..count {
            let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
            check(
                unsafe { controller.getParameterInfo(n, &mut info) },
                "parameter enumeration",
            )?;
            if info.stepCount < 0 {
                return Err(Error::Invalid("Negative VST3 parameter step count".into()));
            }
            let p = HostedParameter {
                id: info.id,
                name: utf16(&info.title),
                module: info.unitId.to_string(),
                domain: HostedParameterDomain::Normalized {
                    step_count: info.stepCount as u32,
                },
                default_value: info.defaultNormalizedValue,
                automatable: info.flags & 1 != 0,
                modulatable: false,
                read_only: info.flags & 2 != 0,
                hidden: info.flags & 16 != 0,
            };
            if !p.valid() || parameters.iter().any(|v: &HostedParameter| v.id == p.id) {
                return Err(Error::Invalid("Invalid VST3 parameter metadata".into()));
            }
            parameters.push(p);
        }
        Ok(parameters)
    }
    fn ports(&self, input: bool) -> Result<Vec<NativeAudioPort>> {
        let direction = i32::from(!input);
        let count = unsafe { self.component.getBusCount(0, direction) };
        if !(0..=32).contains(&count) {
            return Err(Error::Invalid("VST3 audio bus count exceeds 32".into()));
        }
        let mut ports = Vec::with_capacity(count as usize);
        for n in 0..count {
            let mut info: BusInfo = unsafe { std::mem::zeroed() };
            check(
                unsafe { self.component.getBusInfo(0, direction, n, &mut info) },
                "audio bus enumeration",
            )?;
            if !(1..=2).contains(&info.channelCount) {
                return Err(Error::Invalid(format!(
                    "VST3 {} bus {} ({}) has {} channels after mono/stereo negotiation; supported range is 1–2",
                    if input { "input" } else { "output" },
                    n,
                    utf16(&info.name),
                    info.channelCount
                )));
            }
            ports.push(NativeAudioPort {
                id: n as u32,
                name: utf16(&info.name),
                channels: info.channelCount as u32,
                main: info.busType == 0,
            });
        }
        Ok(ports)
    }
    fn restore(&self, state: &NativeState) -> Result<()> {
        let NativeState::Vst3 {
            component,
            controller,
        } = state
        else {
            return Err(Error::Invalid("State format does not match VST3".into()));
        };
        if component.len() > MAX_HOSTED_STATE_BYTES || controller.len() > MAX_HOSTED_STATE_BYTES {
            return Err(Error::Invalid("VST3 state exceeds quota".into()));
        }
        let stream = StateStream::new(component.clone());
        let pointer = stream
            .to_com_ptr::<IBStream>()
            .expect("declared state stream");
        check(
            unsafe { self.component.setState(pointer.as_ptr()) },
            "component state restore",
        )?;
        if let Some(edit) = &self.controller {
            // Use an independent reader: component.setState may retain or alter
            // its stream. Some controllers share processor state and leave this
            // synchronization method unimplemented (including SDK defaults).
            let stream = StateStream::new(component.clone());
            let pointer = stream
                .to_com_ptr::<IBStream>()
                .expect("declared state stream");
            let status = unsafe { edit.setComponentState(pointer.as_ptr()) };
            if status != kNotImplemented {
                check(status, "controller component state")?;
            }
            if !controller.is_empty() {
                let stream = StateStream::new(controller.clone());
                let pointer = stream
                    .to_com_ptr::<IBStream>()
                    .expect("declared state stream");
                check(
                    unsafe { edit.setState(pointer.as_ptr()) },
                    "controller state restore",
                )?;
            }
        }
        Ok(())
    }
    pub fn text(&self, id: u32, value: f64) -> Result<String> {
        let controller = self
            .controller
            .as_ref()
            .ok_or_else(|| Error::Invalid("Plugin has no parameter formatter".into()))?;
        let mut text = [0u16; 128];
        check(
            unsafe { controller.getParamStringByValue(id, value, &mut text) },
            "parameter formatting",
        )?;
        Ok(utf16(&text))
    }
    pub fn save(&self) -> Result<NativeState> {
        let a = StateStream::new(Vec::new());
        let pointer = a.to_com_ptr::<IBStream>().expect("declared state stream");
        check(
            unsafe { self.component.getState(pointer.as_ptr()) },
            "component state capture",
        )?;
        if a.failed.get() {
            return Err(Error::Invalid("VST3 component state exceeds quota".into()));
        }
        let mut controller = Vec::new();
        if let Some(edit) = &self.controller {
            let b = StateStream::new(Vec::new());
            let pointer = b.to_com_ptr::<IBStream>().expect("declared state stream");
            let status = unsafe { edit.getState(pointer.as_ptr()) };
            if b.failed.get() {
                return Err(Error::Invalid("VST3 controller state exceeds quota".into()));
            }
            if status == kResultOk {
                controller = b.bytes.borrow().clone();
            } else if status != kNotImplemented {
                return Err(Error::Invalid(format!(
                    "VST3 controller state capture failed ({status})"
                )));
            }
        }
        let component = a.bytes.borrow().clone();
        Ok(NativeState::Vst3 {
            component,
            controller,
        })
    }
    pub fn value(&self, id: u32) -> Result<f64> {
        let value = unsafe {
            self.controller
                .as_ref()
                .ok_or_else(|| Error::Invalid("Plugin has no edit controller".into()))?
                .getParamNormalized(id)
        };
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(value)
        } else {
            Err(Error::Invalid(
                "VST3 returned an invalid parameter value".into(),
            ))
        }
    }
    pub fn set_base(&self, id: u32, value: f64) -> Result<()> {
        if let Some(controller) = &self.controller {
            // Capture restores processor bases through inputParameterChanges.
            // Do not issue redundant controller writes: some plugins return
            // kResultFalse for an unchanged value, including utility parameters.
            if unsafe { controller.getParamNormalized(id) } != value {
                let status = unsafe { controller.setParamNormalized(id, value) };
                if status != kResultOk && unsafe { controller.getParamNormalized(id) } != value {
                    return Err(Error::Invalid(format!(
                        "VST3 controller parameter {id} update failed ({status})"
                    )));
                }
            }
        }
        Ok(())
    }
    pub fn pump(&mut self) {
        #[cfg(target_os = "linux")]
        self.bridge.run_loop.pump();
        if let Some(connections) = &self.connections {
            connections.main();
        }
    }
    pub unsafe fn open_editor(&mut self, parent: NativeParent, scale: f64) -> Result<(u32, u32)> {
        self.close_editor();
        let controller = self
            .controller
            .as_ref()
            .ok_or_else(|| Error::Invalid("Plugin has no controller editor".into()))?;
        let view = unsafe { ComPtr::from_raw(controller.createView(c"editor".as_ptr())) }
            .ok_or_else(|| Error::Invalid("Plugin has no native editor".into()))?;
        let (handle, api) = match parent {
            NativeParent::Cocoa(p) => (p, c"NSView"),
            NativeParent::Win32(p) => (p, c"HWND"),
            NativeParent::X11(p) => (p as usize as *mut c_void, c"X11EmbedWindowID"),
            NativeParent::Floating => {
                return Err(Error::Invalid(
                    "VST3 editors require a host parent window".into(),
                ));
            }
        };
        check(
            unsafe { view.isPlatformTypeSupported(api.as_ptr()) },
            "editor platform",
        )?;
        let frame = self
            .bridge
            .to_com_ptr::<IPlugFrame>()
            .expect("declared plugin frame");
        check(unsafe { view.setFrame(frame.as_ptr()) }, "editor frame")?;
        if let Err(e) = check(
            unsafe { view.attached(handle, api.as_ptr()) },
            "editor attachment",
        ) {
            unsafe {
                view.setFrame(ptr::null_mut());
            }
            return Err(e);
        }
        if let Some(scaling) = view.cast::<IPlugViewContentScaleSupport>()
            && api != c"NSView"
            && scale.is_finite()
            && scale > 0.0
        {
            unsafe {
                scaling.setContentScaleFactor(scale as f32);
            }
        }
        let mut rect = ViewRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let status = unsafe { view.getSize(&mut rect) };
        let w = i64::from(rect.right) - i64::from(rect.left);
        let h = i64::from(rect.bottom) - i64::from(rect.top);
        if status != kResultOk || !(1..=16384).contains(&w) || !(1..=16384).contains(&h) {
            unsafe {
                view.onFocus(0);
                view.removed();
                view.setFrame(ptr::null_mut());
            }
            return Err(Error::Invalid("Invalid native editor size".into()));
        }
        self.view = Some(view);
        Ok((w as u32, h as u32))
    }
    pub fn editor_scale(&self, scale: f64) {
        if !cfg!(target_os = "macos")
            && scale.is_finite()
            && scale > 0.0
            && let Some(view) = &self.view
            && let Some(scaling) = view.cast::<IPlugViewContentScaleSupport>()
        {
            unsafe {
                scaling.setContentScaleFactor(scale as f32);
            }
        }
    }
    pub fn editor_resizable(&self) -> bool {
        self.view
            .as_ref()
            .is_some_and(|view| unsafe { view.canResize() } == kResultOk)
    }
    pub fn editor_focus(&self, focus: bool) {
        if let Some(view) = &self.view {
            unsafe {
                view.onFocus(u8::from(focus));
            }
        }
    }
    pub fn resize_editor(&mut self, width: u32, height: u32) -> Result<(u32, u32)> {
        let view = self
            .view
            .as_ref()
            .ok_or_else(|| Error::Invalid("Plugin editor is closed".into()))?;
        let mut rect = ViewRect {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        if self.editor_resizable() {
            unsafe {
                view.checkSizeConstraint(&mut rect);
            }
        }
        let width = i64::from(rect.right) - i64::from(rect.left);
        let height = i64::from(rect.bottom) - i64::from(rect.top);
        if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
            return Err(Error::Invalid(
                "Plugin returned invalid editor size constraints".into(),
            ));
        }
        check(unsafe { view.onSize(&mut rect) }, "editor resize")?;
        Ok((width as u32, height as u32))
    }
    pub fn close_editor(&mut self) {
        if let Some(view) = self.view.take() {
            unsafe {
                view.removed();
                view.setFrame(ptr::null_mut());
            }
        }
    }
}
impl Drop for VstInstance {
    fn drop(&mut self) {
        self.close_editor();
        unsafe {
            if self.active {
                self.component.setActive(0);
            }
            if let Some(controller) = &self.controller {
                if let Some(connections) = &self.connections {
                    connections.disconnect();
                }
                controller.setComponentHandler(ptr::null_mut());
                if self.controller_initialized {
                    controller.terminate();
                }
            }
            if self.initialized {
                self.component.terminate();
            }
        }
    }
}

pub(super) struct VstAudio {
    processor: *mut IAudioProcessor,
    pub maximum: u32,
    rate: u32,
    inputs: Vec<Planar>,
    outputs: Vec<Planar>,
    input_abi: Vec<AudioBusBuffers>,
    output_abi: Vec<AudioBusBuffers>,
    events: Vec<NativeEvent>,
    commands: rtrb::Consumer<NativeEvent>,
    changes: queues::ParameterChanges,
    notes: queues::Events,
    note_input: bool,
    messages: *const c_void,
    mode: i32,
    midi_mapping: Vec<Option<u32>>,
}
impl VstAudio {
    fn new(
        processor: *mut IAudioProcessor,
        shared: &Shared,
        m: &NativeMetadata,
        rate: u32,
        maximum: u32,
        commands: rtrb::Consumer<NativeEvent>,
        note_input: bool,
    ) -> Self {
        let mut inputs: Vec<_> = m
            .inputs
            .iter()
            .map(|p| Planar::new(p.channels, maximum as usize))
            .collect();
        let mut outputs: Vec<_> = m
            .outputs
            .iter()
            .map(|p| Planar::new(p.channels, maximum as usize))
            .collect();
        let abi = |p: &mut Planar| AudioBusBuffers {
            numChannels: p.channels as i32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: p.pointers.as_mut_ptr(),
            },
        };
        let input_abi = inputs.iter_mut().map(abi).collect();
        let output_abi = outputs.iter_mut().map(abi).collect();
        Self {
            processor,
            maximum,
            rate,
            inputs,
            outputs,
            input_abi,
            output_abi,
            events: Vec::with_capacity(MAX_HOSTED_EVENTS),
            commands,
            changes: queues::ParameterChanges::new(&m.parameters, shared),
            notes: queues::Events::new(),
            note_input,
            messages: ptr::null(),
            mode: 0,
            midi_mapping: vec![None; 16 * 130],
        }
    }
    fn processor(&self) -> ComRef<'_, IAudioProcessor> {
        unsafe { ComRef::from_raw(self.processor) }.expect("prepared processor pointer")
    }
    pub fn start(&mut self) -> bool {
        (unsafe { self.processor().setProcessing(1) }) == kResultOk
    }
    pub fn stop(&mut self) {
        unsafe {
            self.processor().setProcessing(0);
        }
    }
    pub fn flush(&mut self, processing: bool) -> bool {
        if !processing && !self.start() {
            return false;
        }
        let valid = self.process(&[], &mut [], &[], NativeTransport::default());
        if !processing {
            self.stop();
        }
        valid
    }
    pub fn process(
        &mut self,
        input: &[&[[f32; 2]]],
        out: &mut [[f32; 2]],
        events: &[NativeEvent],
        t: NativeTransport,
    ) -> bool {
        unsafe {
            connections::Connections::audio(self.messages);
        }
        self.events.clear();
        while let Ok(e) = self.commands.pop() {
            if self.events.len() == self.events.capacity() {
                return false;
            }
            self.events.push(e);
        }
        if self.events.len() + events.len() > self.events.capacity() {
            return false;
        }
        self.events.extend_from_slice(events);
        let mapping = &self.midi_mapping;
        self.events.retain_mut(|event| {
            let NativeEvent::Midi { offset, data } = *event else {
                return true;
            };
            let status = data[0] & 0xf0;
            if matches!(status, 0x80 | 0x90 | 0xa0) {
                return true;
            }
            let (controller, value) = match status {
                0xb0 => (usize::from(data[1] & 127), f64::from(data[2] & 127) / 127.0),
                0xd0 => (128, f64::from(data[1] & 127) / 127.0),
                0xe0 => (
                    129,
                    f64::from(u16::from(data[1] & 127) | (u16::from(data[2] & 127) << 7)) / 16383.0,
                ),
                _ => return false,
            };
            if let Some(id) = mapping[usize::from(data[0] & 15) * 130 + controller] {
                *event = NativeEvent::Parameter { offset, id, value };
                true
            } else {
                false
            }
        });
        // Queued manual changes are at zero, followed by sorted graph events.
        // Retain this insertion order for equal-offset updates and retriggers.
        if !self.changes.prepare(&self.events) || !self.notes.prepare(&self.events, self.note_input)
        {
            return false;
        }
        for (n, p) in self.inputs.iter_mut().enumerate() {
            p.input(input.get(n).copied(), out.len());
        }
        for p in &mut self.outputs {
            p.clear(out.len());
        }
        for p in &mut self.output_abi {
            p.silenceFlags = 0;
        }
        if !t.quarter.is_finite() || !t.tempo.is_finite() || t.tempo <= 0.0 {
            return false;
        }
        let mut context: ProcessContext = unsafe { std::mem::zeroed() };
        context.state = 512 | 1024 | 8192 | 131072;
        context.sampleRate = f64::from(self.rate);
        context.projectTimeSamples = t.frame.min(i64::MAX as u64) as i64;
        context.continousTimeSamples = context.projectTimeSamples;
        context.projectTimeMusic = t.quarter;
        context.tempo = t.tempo;
        context.timeSigNumerator = i32::from(t.numerator);
        context.timeSigDenominator = i32::from(t.denominator);
        if t.playing {
            context.state |= 2;
        }
        if let Some((a, b)) = t.loop_quarters {
            if !a.is_finite() || !b.is_finite() || a >= b {
                return false;
            }
            context.state |= 4 | 4096;
            context.cycleStartMusic = a;
            context.cycleEndMusic = b;
        }
        let mut data = ProcessData {
            processMode: self.mode,
            symbolicSampleSize: 0,
            numSamples: out.len() as i32,
            numInputs: self.input_abi.len() as i32,
            numOutputs: self.output_abi.len() as i32,
            inputs: self.input_abi.as_mut_ptr(),
            outputs: self.output_abi.as_mut_ptr(),
            inputParameterChanges: self.changes.input.as_ptr(),
            outputParameterChanges: self.changes.output.as_ptr(),
            inputEvents: self.notes.pointer.as_ptr(),
            outputEvents: ptr::null_mut(),
            processContext: &mut context,
        };
        (unsafe { self.processor().process(&mut data) }) == kResultOk && self.outputs[0].output(out)
    }
}
