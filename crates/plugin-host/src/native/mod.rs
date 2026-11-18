//! Main-thread instance ownership and exclusive, allocation-free processing leases.
mod buffers;
mod main_loop;
pub(crate) use main_loop::pump_application_events;
#[cfg(target_os = "linux")]
mod linux;
mod state_tag;
use state_tag::AtomicStateTag;
pub use state_tag::NativeStateTag;
mod clap;
mod vst3;
use crate::{Error, PluginCandidate, PluginIdentity, Result};
use daw_unit_core::HostedParameter;
use std::{
    cell::{Cell, RefCell, UnsafeCell},
    collections::HashMap,
    marker::PhantomData,
    path::PathBuf,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

pub const MAX_HOSTED_EVENTS: usize = 65536;
pub const MAX_HOSTED_STATE_BYTES: usize = 16 * 1024 * 1024;
const CONTROL_EVENTS: usize = 16384;
static NEXT_THREAD: AtomicU64 = AtomicU64::new(1);
thread_local! { static THREAD: Cell<u64> = const { Cell::new(0) }; }
fn thread_key() -> u64 {
    THREAD.with(|key| {
        let n = key.get();
        if n != 0 {
            n
        } else {
            let n = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
            key.set(n);
            n
        }
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeAudioPort {
    pub id: u32,
    pub name: String,
    pub channels: u32,
    pub main: bool,
}
#[derive(Clone, Debug)]
pub struct NativeMetadata {
    pub sample_rate_hz: u32,
    pub parameters: Vec<HostedParameter>,
    pub inputs: Vec<NativeAudioPort>,
    pub outputs: Vec<NativeAudioPort>,
    pub latency_frames: u32,
    pub tail_frames: u64,
    pub state_supported: bool,
    pub editor_supported: bool,
}
impl NativeMetadata {
    pub fn audio_layout_signature(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for ports in [&self.inputs, &self.outputs] {
            hash.update((ports.len() as u32).to_le_bytes());
            for port in ports {
                hash.update(port.id.to_le_bytes());
                hash.update(port.channels.to_le_bytes());
                hash.update([u8::from(port.main)]);
            }
        }
        format!("{:x}", hash.finalize())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum NativeState {
    Clap(Vec<u8>),
    Vst3 {
        component: Vec<u8>,
        controller: Vec<u8>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeEvent {
    Parameter {
        offset: u32,
        id: u32,
        value: f64,
    },
    Modulation {
        offset: u32,
        id: u32,
        amount: f64,
    },
    Note {
        offset: u32,
        on: bool,
        id: i32,
        channel: i16,
        key: i16,
        velocity: f64,
    },
    Midi {
        offset: u32,
        data: [u8; 3],
    },
}
impl NativeEvent {
    pub fn offset(self) -> u32 {
        match self {
            Self::Parameter { offset, .. }
            | Self::Modulation { offset, .. }
            | Self::Note { offset, .. }
            | Self::Midi { offset, .. } => offset,
        }
    }
    fn valid(self, frames: u32) -> bool {
        self.offset() < frames
            && match self {
                Self::Parameter { value, .. } => value.is_finite(),
                Self::Modulation { amount, .. } => amount.is_finite(),
                Self::Note {
                    channel,
                    key,
                    velocity,
                    ..
                } => {
                    (0..16).contains(&channel)
                        && (0..128).contains(&key)
                        && (0.0..=1.0).contains(&velocity)
                }
                Self::Midi { .. } => true,
            }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeNotification {
    Begin(u32),
    Value(u32, f64),
    Telemetry(u32, f64),
    End(u32),
    StateDirty,
    PresetBegin,
    PresetEnd,
    TextChanged,
    Restart(u32),
    Resize(u32, u32),
    EditorClosed,
    Fault,
}
#[derive(Clone, Copy, Debug)]
pub struct NativeTransport {
    pub frame: u64,
    pub quarter: f64,
    pub tempo: f64,
    pub playing: bool,
    pub numerator: u16,
    pub denominator: u16,
    pub loop_quarters: Option<(f64, f64)>,
}
impl Default for NativeTransport {
    fn default() -> Self {
        Self {
            frame: 0,
            quarter: 0.0,
            tempo: 120.0,
            playing: false,
            numerator: 4,
            denominator: 4,
            loop_quarters: None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct NativeConfiguration {
    pub sample_rate_hz: u32,
    pub max_block_frames: u32,
    pub offline: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeProcessStatus {
    Processed,
    Suspended,
    Fault,
}
#[derive(Clone, Copy, Debug)]
pub enum NativeParent {
    Cocoa(*mut std::ffi::c_void),
    Win32(*mut std::ffi::c_void),
    X11(u64),
    Floating,
}

struct ParameterBase {
    id: u32,
    value: AtomicU64,
    telemetry: AtomicU64,
    read_only: bool,
    modulatable: bool,
}
struct Shared {
    state_tag: AtomicStateTag,
    parameters: std::sync::OnceLock<Box<[ParameterBase]>>,
    parameter_epoch: AtomicU64,
    main_thread: u64,
    audio_thread: AtomicU64,
    callback: AtomicBool,
    restart: AtomicU32,
    configuration_valid: AtomicBool,
    values_changed: AtomicBool,
    text_changed: AtomicBool,
    dirty: AtomicBool,
    fault: AtomicBool,
    suspend: AtomicBool,
    suspended: AtomicBool,
    returned: AtomicBool,
    reset: AtomicBool,
    flush: AtomicBool,
    main_events: UnsafeCell<rtrb::Producer<NativeNotification>>,
    audio_events: UnsafeCell<rtrb::Producer<NativeNotification>>,
}
impl Shared {
    fn emit(&self, event: NativeNotification) {
        if let NativeNotification::Telemetry(id, value) | NativeNotification::Value(id, value) =
            event
            && let Some(parameters) = self.parameters.get()
            && let Ok(index) = parameters.binary_search_by_key(&id, |p| p.id)
            && value.is_finite()
            && (matches!(event, NativeNotification::Telemetry(..)) || parameters[index].read_only)
        {
            parameters[index]
                .telemetry
                .store(value.to_bits(), Ordering::Release);
            return;
        }
        if let NativeNotification::Value(id, value) = event
            && value.is_finite()
            && let Some(parameters) = self.parameters.get()
            && let Ok(index) = parameters.binary_search_by_key(&id, |p| p.id)
            && !parameters[index].read_only
        {
            parameters[index]
                .value
                .store(value.to_bits(), Ordering::Release);
        }
        let thread = thread_key();
        let queue = if thread == self.main_thread {
            Some(&self.main_events)
        } else if thread == self.audio_thread.load(Ordering::Acquire) {
            Some(&self.audio_events)
        } else {
            None
        };
        if let Some(queue) = queue {
            // SAFETY: each SPSC producer is accessed only by its designated thread.
            if unsafe { &mut *queue.get() }.push(event).is_err() {
                self.fault.store(true, Ordering::Release);
            }
        } else {
            self.fault.store(true, Ordering::Release);
        }
    }
}
struct Instance {
    backend: RefCell<Backend>,
    audio: UnsafeCell<Audio>,
    main_events: RefCell<rtrb::Consumer<NativeNotification>>,
    audio_events: RefCell<rtrb::Consumer<NativeNotification>>,
    commands: RefCell<rtrb::Producer<NativeEvent>>,
    metadata: NativeMetadata,
    shared: Box<Shared>,
    retire: Cell<bool>,
    fault_reported: Cell<bool>,
}

enum Backend {
    Clap(clap::ClapInstance),
    Vst3(vst3::VstInstance),
}
impl Backend {
    fn set_base(&self, id: u32, value: f64) -> Result<()> {
        match self {
            Self::Clap(_) => Ok(()),
            Self::Vst3(p) => p.set_base(id, value),
        }
    }
    fn save(&self) -> Result<NativeState> {
        match self {
            Self::Clap(p) => p.save(),
            Self::Vst3(p) => p.save(),
        }
    }
    fn text(&self, id: u32, value: f64) -> Result<String> {
        match self {
            Self::Clap(p) => p.text(id, value),
            Self::Vst3(p) => p.text(id, value),
        }
    }
    fn value(&self, id: u32) -> Result<f64> {
        match self {
            Self::Clap(p) => p.value(id),
            Self::Vst3(p) => p.value(id),
        }
    }
    fn resize_editor(&mut self, width: u32, height: u32) -> Result<(u32, u32)> {
        match self {
            Self::Clap(p) => p.resize_editor(width, height),
            Self::Vst3(p) => p.resize_editor(width, height),
        }
    }
    fn editor_resizable(&self) -> bool {
        match self {
            Self::Clap(p) => p.editor_resizable(),
            Self::Vst3(p) => p.editor_resizable(),
        }
    }
    fn editor_focus(&self, focus: bool) {
        if let Self::Vst3(p) = self {
            p.editor_focus(focus);
        }
    }
    fn pump(&mut self) {
        match self {
            Self::Clap(p) => p.pump(),
            Self::Vst3(p) => p.pump(),
        }
    }
    unsafe fn open_editor(&mut self, parent: NativeParent, scale: f64) -> Result<(u32, u32)> {
        match self {
            Self::Clap(p) => unsafe { p.open_editor(parent, scale) },
            Self::Vst3(p) => unsafe { p.open_editor(parent, scale) },
        }
    }
    fn close_editor(&mut self) {
        match self {
            Self::Clap(p) => p.close_editor(),
            Self::Vst3(p) => p.close_editor(),
        }
    }
}
enum Audio {
    Clap(clap::ClapAudio),
    Vst3(vst3::VstAudio),
}
impl Audio {
    fn flush(&mut self, processing: bool) -> bool {
        match self {
            Self::Clap(p) => p.flush(processing),
            Self::Vst3(p) => p.flush(processing),
        }
    }
    fn maximum(&self) -> u32 {
        match self {
            Self::Clap(p) => p.maximum,
            Self::Vst3(p) => p.maximum,
        }
    }
    fn start(&mut self) -> bool {
        match self {
            Self::Clap(p) => p.start(),
            Self::Vst3(p) => p.start(),
        }
    }
    fn stop(&mut self) {
        match self {
            Self::Clap(p) => p.stop(),
            Self::Vst3(p) => p.stop(),
        }
    }
    fn reset(&mut self) -> bool {
        match self {
            Self::Clap(p) => {
                p.reset();
                true
            }
            Self::Vst3(_) => false,
        }
    }
    fn process(
        &mut self,
        input: &[&[[f32; 2]]],
        out: &mut [[f32; 2]],
        events: &[NativeEvent],
        transport: NativeTransport,
    ) -> bool {
        match self {
            Self::Clap(p) => p.process(input, out, events, transport),
            Self::Vst3(p) => p.process(input, out, events, transport),
        }
    }
}

/// Own this on the application/CLI main thread. Loading native code remains unsafe:
/// a foreign ABI violation can corrupt or terminate the process.
pub struct NativeHost {
    owner: u64,
    modules: HashMap<PathBuf, (String, Rc<clap::ClapModule>)>,
    vst_modules: HashMap<PathBuf, (String, Rc<crate::scan::VstModule>)>,
    instances: HashMap<uuid::Uuid, Box<Instance>>,
    _main: PhantomData<Rc<()>>,
    platform: main_loop::MainServices,
}
impl Default for NativeHost {
    fn default() -> Self {
        Self::new()
    }
}
impl NativeHost {
    pub fn new() -> Self {
        Self {
            owner: thread_key(),
            modules: HashMap::new(),
            vst_modules: HashMap::new(),
            instances: HashMap::new(),
            _main: PhantomData,
            platform: main_loop::MainServices::new(),
        }
    }
    fn check_thread(&self) -> Result<()> {
        self.platform.check()?;
        if thread_key() == self.owner {
            Ok(())
        } else {
            Err(Error::Invalid(
                "Plugin host called outside its main thread".into(),
            ))
        }
    }
    /// # Safety
    /// Call on the actual application main thread with a native, trusted ABI module.
    /// Foreign constructors may hang or crash; active hosting is not a sandbox.
    pub unsafe fn load(
        &mut self,
        candidate: &PluginCandidate,
        identity: &PluginIdentity,
        rate: u32,
        maximum: u32,
        state: Option<&NativeState>,
    ) -> Result<uuid::Uuid> {
        unsafe {
            self.load_configured(
                candidate,
                identity,
                NativeConfiguration {
                    sample_rate_hz: rate,
                    max_block_frames: maximum,
                    offline: false,
                },
                state,
            )
        }
    }
    /// # Safety
    /// The module is a trusted native ABI and this is the application main thread.
    pub unsafe fn load_configured(
        &mut self,
        candidate: &PluginCandidate,
        identity: &PluginIdentity,
        configuration: NativeConfiguration,
        state: Option<&NativeState>,
    ) -> Result<uuid::Uuid> {
        unsafe { self.load_inner(candidate, identity, configuration, state) }.map_err(|error| {
            Error::Invalid(format!(
                "{} ({}): {error}",
                candidate.path.display(),
                candidate.format.extension()
            ))
        })
    }
    unsafe fn load_inner(
        &mut self,
        candidate: &PluginCandidate,
        identity: &PluginIdentity,
        configuration: NativeConfiguration,
        state: Option<&NativeState>,
    ) -> Result<uuid::Uuid> {
        let rate = configuration.sample_rate_hz;
        let maximum = configuration.max_block_frames;
        self.check_thread()?;
        let _scope = main_loop::MainFrame::new();
        if identity.format() != candidate.format {
            return Err(Error::Invalid(
                "Plugin identity format does not match its module".into(),
            ));
        }
        if ![44100, 48000, 88200, 96000, 176400, 192000].contains(&rate)
            || !(1..=4096).contains(&maximum)
        {
            return Err(Error::Invalid(
                "Invalid plugin processing configuration".into(),
            ));
        }
        if crate::discovery::candidate(candidate.format, candidate.path.clone())? != *candidate {
            return Err(Error::Invalid(
                "Plugin binary changed before activation".into(),
            ));
        }
        self.modules.retain(|_, (_, m)| Rc::strong_count(m) > 1);
        self.vst_modules.retain(|_, (_, m)| Rc::strong_count(m) > 1);
        let (mp, mc) = rtrb::RingBuffer::new(CONTROL_EVENTS);
        let (ap, ac) = rtrb::RingBuffer::new(CONTROL_EVENTS);
        let (cp, cc) = rtrb::RingBuffer::new(CONTROL_EVENTS);
        let shared = Box::new(Shared {
            state_tag: AtomicStateTag::new(),
            parameters: Default::default(),
            parameter_epoch: AtomicU64::new(0),
            main_thread: self.owner,
            audio_thread: AtomicU64::new(0),
            callback: AtomicBool::new(false),
            restart: AtomicU32::new(0),
            configuration_valid: AtomicBool::new(true),
            values_changed: AtomicBool::new(false),
            text_changed: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            fault: AtomicBool::new(false),
            suspend: AtomicBool::new(false),
            suspended: AtomicBool::new(true),
            returned: AtomicBool::new(true),
            reset: AtomicBool::new(false),
            flush: AtomicBool::new(false),
            main_events: UnsafeCell::new(mp),
            audio_events: UnsafeCell::new(ap),
        });
        let (backend, audio, metadata) = match identity {
            PluginIdentity::Clap(id) => {
                let module = if let Some((fingerprint, module)) = self.modules.get(&candidate.path)
                {
                    if *fingerprint != candidate.fingerprint {
                        return Err(Error::Invalid(
                            "Plugin changed while its previous module is still loaded".into(),
                        ));
                    }
                    module.clone()
                } else {
                    let module = Rc::new(unsafe { clap::ClapModule::open(&candidate.path) }?);
                    self.modules.insert(
                        candidate.path.clone(),
                        (candidate.fingerprint.clone(), module.clone()),
                    );
                    module
                };
                let (backend, audio, metadata) = unsafe {
                    clap::ClapInstance::create(module, id, &shared, configuration, state, cc)
                }?;
                (Backend::Clap(backend), Audio::Clap(audio), metadata)
            }
            PluginIdentity::Vst3(id) => {
                let module = if let Some((fingerprint, module)) =
                    self.vst_modules.get(&candidate.path)
                {
                    if *fingerprint != candidate.fingerprint {
                        return Err(Error::Invalid(
                            "Plugin changed while its previous module is still loaded".into(),
                        ));
                    }
                    module.clone()
                } else {
                    let module = Rc::new(unsafe { crate::scan::VstModule::open(&candidate.path) }?);
                    self.vst_modules.insert(
                        candidate.path.clone(),
                        (candidate.fingerprint.clone(), module.clone()),
                    );
                    module
                };
                let (backend, audio, metadata) = unsafe {
                    vst3::VstInstance::create(module, *id, &shared, configuration, state, cc)
                }?;
                (Backend::Vst3(backend), Audio::Vst3(audio), metadata)
            }
        };
        shared
            .state_tag
            .store(state.map(NativeStateTag::from_state));
        let mut parameters = metadata
            .parameters
            .iter()
            .map(|p| {
                backend.value(p.id).map(|value| ParameterBase {
                    id: p.id,
                    value: AtomicU64::new(value.to_bits()),
                    telemetry: AtomicU64::new(f64::NAN.to_bits()),
                    read_only: p.read_only,
                    modulatable: p.modulatable,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        parameters.sort_unstable_by_key(|p| p.id);
        let _ = shared.parameters.set(parameters.into_boxed_slice());
        // Initialization callbacks are already reflected in the enumerated
        // metadata. Only subsequent changes require a graph restart.
        shared.restart.store(0, Ordering::Release);
        shared.values_changed.store(false, Ordering::Release);
        shared.text_changed.store(false, Ordering::Release);
        let id = uuid::Uuid::new_v4();
        self.instances.insert(
            id,
            Box::new(Instance {
                backend: RefCell::new(backend),
                audio: UnsafeCell::new(audio),
                main_events: RefCell::new(mc),
                audio_events: RefCell::new(ac),
                commands: RefCell::new(cp),
                metadata,
                shared,
                retire: Cell::new(false),
                fault_reported: Cell::new(false),
            }),
        );
        Ok(id)
    }
    fn instance(&self, id: uuid::Uuid) -> Result<&Instance> {
        self.check_thread()?;
        self.instances
            .get(&id)
            .map(Box::as_ref)
            .ok_or_else(|| Error::Invalid("Plugin instance is no longer present".into()))
    }
    pub fn metadata(&self, id: uuid::Uuid) -> Result<&NativeMetadata> {
        Ok(&self.instance(id)?.metadata)
    }
    pub fn take_processor(&self, id: uuid::Uuid) -> Result<NativeProcessor> {
        let instance = self.instance(id)?;
        if instance.retire.get()
            || instance
                .shared
                .returned
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(Error::Invalid(
                "Plugin already has a processing owner".into(),
            ));
        }
        instance.shared.suspend.store(false, Ordering::Release);
        instance.shared.suspended.store(false, Ordering::Release);
        Ok(NativeProcessor {
            instance: NonNull::from(instance),
            started: false,
            _unique: PhantomData,
        })
    }
    pub fn set_parameter(&self, id: uuid::Uuid, parameter: u32, value: f64) -> Result<()> {
        let instance = self.instance(id)?;
        let parameter = instance
            .metadata
            .parameters
            .iter()
            .find(|p| p.id == parameter && !p.read_only)
            .ok_or_else(|| Error::Invalid("Plugin parameter is not writable".into()))?;
        let value = parameter
            .domain
            .constrain(value)
            .ok_or_else(|| Error::Invalid("Invalid plugin parameter value".into()))?;
        let mut queue = instance.commands.borrow_mut();
        if queue.slots() == 0 {
            return Err(Error::Invalid("Plugin control queue is full".into()));
        }
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow().set_base(parameter.id, value)?;
        queue
            .push(NativeEvent::Parameter {
                offset: 0,
                id: parameter.id,
                value,
            })
            .map_err(|_| Error::Invalid("Plugin control queue is full".into()))?;
        if let Some(parameters) = instance.shared.parameters.get()
            && let Ok(index) = parameters.binary_search_by_key(&parameter.id, |p| p.id)
        {
            parameters[index]
                .value
                .store(value.to_bits(), Ordering::Release);
        }
        instance.shared.flush.store(true, Ordering::Release);
        Ok(())
    }
    pub fn invalidate(&self, id: uuid::Uuid) -> Result<()> {
        let instance = self.instance(id)?;
        instance
            .shared
            .configuration_valid
            .store(false, Ordering::Release);
        instance.shared.suspend.store(true, Ordering::Release);
        Ok(())
    }
    pub fn request_suspend(&self, id: uuid::Uuid) -> Result<bool> {
        let i = self.instance(id)?;
        i.shared.suspend.store(true, Ordering::Release);
        Ok(i.shared.returned.load(Ordering::Acquire) || i.shared.suspended.load(Ordering::Acquire))
    }
    pub fn resume(&self, id: uuid::Uuid) -> Result<()> {
        self.instance(id)?
            .shared
            .suspend
            .store(false, Ordering::Release);
        Ok(())
    }
    pub fn save_state(&self, id: uuid::Uuid) -> Result<NativeState> {
        let i = self.instance(id)?;
        if !i.shared.returned.load(Ordering::Acquire) && !i.shared.suspended.load(Ordering::Acquire)
        {
            return Err(Error::Invalid(
                "Plugin state capture is waiting for a processing boundary".into(),
            ));
        }
        let _scope = main_loop::MainFrame::new();
        // Absolute VST3 modulation and CLAP automation are transient. Restore the
        // manual bases before asking the plugin for its opaque project state.
        if let Some(parameters) = i.shared.parameters.get() {
            let mut commands = i.commands.borrow_mut();
            let needed = parameters
                .iter()
                .filter(|p| !p.read_only)
                .map(|p| if p.modulatable { 2 } else { 1 })
                .sum::<usize>();
            if commands.slots() < needed {
                return Err(Error::Invalid(
                    "Plugin control queue cannot hold the state capture barrier".into(),
                ));
            }
            for p in parameters.iter().filter(|p| !p.read_only) {
                let value = f64::from_bits(p.value.load(Ordering::Acquire));
                i.backend.borrow().set_base(p.id, value)?;
                commands
                    .push(NativeEvent::Parameter {
                        offset: 0,
                        id: p.id,
                        value,
                    })
                    .map_err(|_| Error::Invalid("Plugin state capture queue overflow".into()))?;
                if p.modulatable {
                    commands
                        .push(NativeEvent::Modulation {
                            offset: 0,
                            id: p.id,
                            amount: 0.0,
                        })
                        .map_err(|_| {
                            Error::Invalid("Plugin state capture queue overflow".into())
                        })?;
                }
            }
            i.shared.flush.store(true, Ordering::Release);
            i.shared.parameter_epoch.fetch_add(1, Ordering::AcqRel);
        }
        if i.shared.flush.swap(false, Ordering::AcqRel) {
            i.shared.audio_thread.store(self.owner, Ordering::Release);
            if !unsafe { &mut *i.audio.get() }.flush(false) {
                i.shared.fault.store(true, Ordering::Release);
                return Err(Error::Invalid(
                    "Cannot flush plugin controls for exact state capture".into(),
                ));
            }
        }
        let state = i.backend.borrow().save()?;
        i.shared
            .state_tag
            .store(Some(NativeStateTag::from_state(&state)));
        Ok(state)
    }
    pub fn parameter_text(&self, id: uuid::Uuid, parameter: u32, value: f64) -> Result<String> {
        let instance = self.instance(id)?;
        if !value.is_finite()
            || !instance
                .metadata
                .parameters
                .iter()
                .any(|p| p.id == parameter)
        {
            return Err(Error::Invalid("Invalid parameter text request".into()));
        }
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow().text(parameter, value)
    }
    pub fn parameter_value(&self, id: uuid::Uuid, parameter: u32) -> Result<f64> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow().value(parameter)
    }
    /// Parent must remain alive until close_editor completes. Native handles stay in desktop.
    /// # Safety
    /// The parent belongs to the current main thread and uses the declared window API.
    pub unsafe fn open_editor(
        &self,
        id: uuid::Uuid,
        parent: NativeParent,
        scale: f64,
    ) -> Result<(u32, u32)> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        unsafe { instance.backend.borrow_mut().open_editor(parent, scale) }
    }
    pub fn resize_editor(&self, id: uuid::Uuid, width: u32, height: u32) -> Result<(u32, u32)> {
        if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
            return Err(Error::Invalid("Invalid native editor size".into()));
        }
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow_mut().resize_editor(width, height)
    }
    pub fn editor_scale(&self, id: uuid::Uuid, scale: f64) -> Result<()> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        match &*instance.backend.borrow() {
            Backend::Clap(p) => p.editor_scale(scale),
            Backend::Vst3(p) => p.editor_scale(scale),
        }
        Ok(())
    }
    pub fn editor_resizable(&self, id: uuid::Uuid) -> Result<bool> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        Ok(instance.backend.borrow().editor_resizable())
    }
    pub fn editor_focus(&self, id: uuid::Uuid, focus: bool) -> Result<()> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow().editor_focus(focus);
        Ok(())
    }
    pub fn close_editor(&self, id: uuid::Uuid) -> Result<()> {
        let instance = self.instance(id)?;
        let _scope = main_loop::MainFrame::new();
        instance.backend.borrow_mut().close_editor();
        Ok(())
    }
    /// Keep active processing and editors alive until the unique lease returns.
    /// This is ownership retirement, not a request to stop the audible instance.
    pub fn retire_when_returned(&self, id: uuid::Uuid) -> Result<()> {
        self.instance(id)?.retire.set(true);
        Ok(())
    }
    pub fn remove(&self, id: uuid::Uuid) -> Result<()> {
        let i = self.instance(id)?;
        i.retire.set(true);
        i.shared.suspend.store(true, Ordering::Release);
        let _scope = main_loop::MainFrame::new();
        i.backend.borrow_mut().close_editor();
        Ok(())
    }
    /// Call each main-thread tick. Notification delivery never executes on audio.
    pub fn pump(&mut self, mut notify: impl FnMut(uuid::Uuid, NativeNotification)) {
        if self.check_thread().is_err() {
            return;
        }
        let _scope = main_loop::MainFrame::new();
        for (id, i) in &self.instances {
            i.backend.borrow_mut().pump();
            if (i.shared.returned.load(Ordering::Acquire)
                || (i.shared.suspend.load(Ordering::Acquire)
                    && i.shared.suspended.load(Ordering::Acquire)))
                && i.shared.flush.swap(false, Ordering::AcqRel)
            {
                i.shared.audio_thread.store(self.owner, Ordering::Release);
                if !unsafe { &mut *i.audio.get() }.flush(false) {
                    i.shared.fault.store(true, Ordering::Release);
                }
            }
            if i.shared.fault.load(Ordering::Acquire) && !i.fault_reported.replace(true) {
                notify(*id, NativeNotification::Fault);
            }
            if i.shared.values_changed.swap(false, Ordering::AcqRel) {
                // A native preset rescan reports authored bases, not transient
                // processor automation. Bind the complete publication to one
                // history group before a capture can restore the base cache.
                notify(*id, NativeNotification::PresetBegin);
                if let Some(parameters) = i.shared.parameters.get() {
                    for parameter in parameters {
                        if let Ok(value) = i.backend.borrow().value(parameter.id)
                            && value.is_finite()
                        {
                            if parameter.read_only {
                                notify(*id, NativeNotification::Telemetry(parameter.id, value));
                            } else if parameter.value.swap(value.to_bits(), Ordering::AcqRel)
                                != value.to_bits()
                            {
                                notify(*id, NativeNotification::Value(parameter.id, value));
                            }
                        }
                    }
                }
                if i.shared.dirty.swap(false, Ordering::AcqRel) {
                    notify(*id, NativeNotification::StateDirty);
                }
                notify(*id, NativeNotification::PresetEnd);
            }
            if i.shared.text_changed.swap(false, Ordering::AcqRel) {
                notify(*id, NativeNotification::TextChanged);
            }
            let flags = i.shared.restart.swap(0, Ordering::AcqRel);
            if flags != 0 {
                i.shared.configuration_valid.store(false, Ordering::Release);
                i.shared.suspend.store(true, Ordering::Release);
                notify(*id, NativeNotification::Restart(flags));
            }
            if i.shared.dirty.swap(false, Ordering::AcqRel) {
                notify(*id, NativeNotification::StateDirty);
            }
            if let Some(parameters) = i.shared.parameters.get() {
                for parameter in parameters {
                    let value = f64::from_bits(
                        parameter
                            .telemetry
                            .swap(f64::NAN.to_bits(), Ordering::AcqRel),
                    );
                    if value.is_finite() {
                        notify(*id, NativeNotification::Telemetry(parameter.id, value));
                    }
                }
            }
            for queue in [&i.main_events, &i.audio_events] {
                while let Ok(event) = queue.borrow_mut().pop() {
                    notify(*id, event);
                }
            }
        }
        self.instances
            .retain(|_, i| !(i.retire.get() && i.shared.returned.load(Ordering::Acquire)));
        self.modules.retain(|_, (_, m)| Rc::strong_count(m) > 1);
        self.vst_modules.retain(|_, (_, m)| Rc::strong_count(m) > 1);
    }
}
impl Drop for NativeHost {
    fn drop(&mut self) {
        let _scope = main_loop::MainFrame::new();
        // A live lease must never become dangling. Normal shutdown returns leases
        // before dropping the broker. Misordered shutdown intentionally retains
        // native owners until process exit, rather than unloading under audio.
        for (_, instance) in self.instances.drain() {
            if !instance.shared.returned.load(Ordering::Acquire) {
                std::mem::forget(instance);
            }
        }
        self.modules.clear();
        self.vst_modules.clear();
    }
}

/// Unique audio ownership. Drop only stops processing and publishes return; the
/// broker retains all buffers, SDK interfaces and final library owners.
pub struct NativeProcessor {
    instance: NonNull<Instance>,
    started: bool,
    _unique: PhantomData<Cell<()>>,
}
// SAFETY: one lease exists per instance; only its audio cell is touched here.
// The main broker retains storage until the lease publishes returned on Drop.
unsafe impl Send for NativeProcessor {}
impl NativeProcessor {
    pub fn state_tag(&self) -> Option<NativeStateTag> {
        unsafe { self.instance.as_ref() }.shared.state_tag.load()
    }
    pub fn base_value(&self, id: u32) -> Option<f64> {
        let parameters = unsafe { self.instance.as_ref() }.shared.parameters.get()?;
        let index = parameters.binary_search_by_key(&id, |p| p.id).ok()?;
        Some(f64::from_bits(
            parameters[index].value.load(Ordering::Acquire),
        ))
    }
    /// Changes after an exact state barrier. Re-emit effective parameter values
    /// on the next block even if their modulation source stayed constant.
    pub fn configuration_valid(&self) -> bool {
        unsafe { self.instance.as_ref() }
            .shared
            .configuration_valid
            .load(Ordering::Acquire)
    }
    pub fn parameter_epoch(&self) -> u64 {
        unsafe { self.instance.as_ref() }
            .shared
            .parameter_epoch
            .load(Ordering::Acquire)
    }
    pub fn process(
        &mut self,
        inputs: &[&[[f32; 2]]],
        out: &mut [[f32; 2]],
        events: &[NativeEvent],
        transport: NativeTransport,
    ) -> NativeProcessStatus {
        // SAFETY: broker retains this allocation and no other lease accesses audio.
        let i = unsafe { self.instance.as_ref() };
        if i.shared.suspend.load(Ordering::Acquire)
            || i.shared.fault.load(Ordering::Acquire)
            || !i.shared.configuration_valid.load(Ordering::Acquire)
        {
            if self.started {
                i.shared.audio_thread.store(thread_key(), Ordering::Release);
                unsafe { &mut *i.audio.get() }.stop();
                self.started = false;
            }
            i.shared.suspended.store(true, Ordering::Release);
            out.fill([0.0; 2]);
            return if i.shared.fault.load(Ordering::Acquire) {
                NativeProcessStatus::Fault
            } else {
                NativeProcessStatus::Suspended
            };
        }
        i.shared.audio_thread.store(thread_key(), Ordering::Release);
        let audio = unsafe { &mut *i.audio.get() };
        if i.shared.reset.swap(false, Ordering::AcqRel) && !audio.reset() {
            if self.started {
                audio.stop();
                self.started = false;
            }
            i.shared.suspend.store(true, Ordering::Release);
            i.shared.suspended.store(true, Ordering::Release);
            i.shared.restart.fetch_or(0x80000000, Ordering::Release);
            out.fill([0.0; 2]);
            return NativeProcessStatus::Suspended;
        }
        if !self.started {
            if !audio.start() {
                i.shared.fault.store(true, Ordering::Release);
                out.fill([0.0; 2]);
                return NativeProcessStatus::Fault;
            }
            self.started = true;
        }
        i.shared.suspended.store(false, Ordering::Release);
        let valid = !out.is_empty()
            && out.len() <= audio.maximum() as usize
            && events.len() <= MAX_HOSTED_EVENTS
            && events.iter().all(|e| e.valid(out.len() as u32))
            && events.windows(2).all(|p| p[0].offset() <= p[1].offset());
        if !valid || !audio.process(inputs, out, events, transport) {
            i.shared.fault.store(true, Ordering::Release);
            out.fill([0.0; 2]);
            NativeProcessStatus::Fault
        } else {
            NativeProcessStatus::Processed
        }
    }
    pub fn reset(&mut self) {
        let i = unsafe { self.instance.as_ref() };
        i.shared.reset.store(true, Ordering::Release);
    }
    /// Service editor/control events while transport is stopped, without audio or
    /// advancing the host clock. This remains exclusive audio-thread work.
    pub fn flush_controls(&mut self) -> NativeProcessStatus {
        let i = unsafe { self.instance.as_ref() };
        if i.shared.suspend.load(Ordering::Acquire)
            || !i.shared.configuration_valid.load(Ordering::Acquire)
        {
            if self.started {
                i.shared.audio_thread.store(thread_key(), Ordering::Release);
                unsafe { &mut *i.audio.get() }.stop();
                self.started = false;
            }
            i.shared.suspended.store(true, Ordering::Release);
            return NativeProcessStatus::Suspended;
        }
        i.shared.audio_thread.store(thread_key(), Ordering::Release);
        if i.shared.flush.swap(false, Ordering::AcqRel)
            && !unsafe { &mut *i.audio.get() }.flush(self.started)
        {
            i.shared.fault.store(true, Ordering::Release);
        }
        if i.shared.fault.load(Ordering::Acquire) {
            NativeProcessStatus::Fault
        } else {
            NativeProcessStatus::Processed
        }
    }
}
impl Drop for NativeProcessor {
    fn drop(&mut self) {
        let i = unsafe { self.instance.as_ref() };
        if self.started {
            i.shared.audio_thread.store(thread_key(), Ordering::Release);
            unsafe { &mut *i.audio.get() }.stop();
        }
        i.shared.suspended.store(true, Ordering::Release);
        // This must be the final access. Main may retire the entire allocation now.
        i.shared.returned.store(true, Ordering::Release);
    }
}
