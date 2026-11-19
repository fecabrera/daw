#[cfg(target_os = "linux")]
#[path = "clap_fds.rs"]
mod fds;
use super::buffers::Planar;
use super::*;
use daw_plugin_clap::sys::{
    audio_buffer::*,
    events::*,
    ext::{
        audio_ports::*, gui::*, latency::*, note_ports::*, params::*, render::*, state::*, tail::*,
        thread_check::*, timer_support::*,
    },
    host::*,
    plugin::*,
    process::*,
    stream::*,
    version::*,
};
use daw_unit_core::{HostedParameter, HostedParameterDomain};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    ptr,
    time::{Duration, Instant},
};

pub(super) use crate::clap_module::ClapModule;

struct Timer {
    id: u32,
    period: Duration,
    next: Instant,
}
struct HostData {
    shared: *const Shared,
    timers: RefCell<Vec<Timer>>,
    #[cfg(target_os = "linux")]
    fds: RefCell<Vec<(i32, u32)>>,
    next_timer: Cell<u32>,
    gui_destroyed: AtomicBool,
}
impl HostData {
    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }
}
unsafe fn data<'a>(host: *const clap_host) -> &'a HostData {
    unsafe { &*((*host).host_data.cast::<HostData>()) }
}
unsafe extern "C" fn request_restart(h: *const clap_host) {
    unsafe { data(h) }
        .shared()
        .restart
        .fetch_or(1, Ordering::Release);
}
unsafe extern "C" fn request_process(h: *const clap_host) {
    unsafe { data(h) }
        .shared()
        .flush
        .store(true, Ordering::Release);
}
unsafe extern "C" fn request_callback(h: *const clap_host) {
    unsafe { data(h) }
        .shared()
        .callback
        .store(true, Ordering::Release);
}
unsafe extern "C" fn is_main(h: *const clap_host) -> bool {
    thread_key() == unsafe { data(h) }.shared().main_thread
}
unsafe extern "C" fn is_audio(h: *const clap_host) -> bool {
    thread_key()
        == unsafe { data(h) }
            .shared()
            .audio_thread
            .load(Ordering::Acquire)
}
unsafe extern "C" fn rescan(h: *const clap_host, flags: u32) {
    let shared = unsafe { data(h) }.shared();
    if flags & (CLAP_PARAM_RESCAN_VALUES | CLAP_PARAM_RESCAN_ALL) != 0 {
        shared.values_changed.store(true, Ordering::Release);
    }
    if flags & CLAP_PARAM_RESCAN_TEXT != 0 {
        shared.text_changed.store(true, Ordering::Release);
    }
    if flags & (CLAP_PARAM_RESCAN_INFO | CLAP_PARAM_RESCAN_ALL) != 0 {
        shared.restart.fetch_or(flags | 1, Ordering::Release);
    }
}
unsafe extern "C" fn ports_changed(h: *const clap_host, _flags: u32) {
    unsafe { request_restart(h) };
}
unsafe extern "C" fn clear(h: *const clap_host, _: u32, _: u32) {
    unsafe { data(h) }
        .shared()
        .restart
        .fetch_or(1, Ordering::Release);
}
unsafe extern "C" fn dirty(h: *const clap_host) {
    unsafe { data(h) }
        .shared()
        .dirty
        .store(true, Ordering::Release);
}
unsafe extern "C" fn flags_supported(_: *const clap_host, _: u32) -> bool {
    true
}
unsafe extern "C" fn dialects(_: *const clap_host) -> u32 {
    CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI
}
unsafe extern "C" fn latency_changed(h: *const clap_host) {
    unsafe { request_restart(h) };
}
unsafe extern "C" fn register_timer(h: *const clap_host, period: u32, id: *mut u32) -> bool {
    if id.is_null() || period == 0 || !unsafe { is_main(h) } {
        return false;
    }
    let d = unsafe { data(h) };
    let mut timers = d.timers.borrow_mut();
    if timers.len() >= 128 {
        return false;
    }
    let next = d.next_timer.get();
    if next == u32::MAX {
        return false;
    }
    d.next_timer.set(next + 1);
    let period = Duration::from_millis(u64::from(period));
    timers.push(Timer {
        id: next,
        period,
        next: Instant::now() + period,
    });
    unsafe { id.write(next) };
    true
}
unsafe extern "C" fn unregister_timer(h: *const clap_host, id: u32) -> bool {
    if !unsafe { is_main(h) } {
        return false;
    }
    let mut timers = unsafe { data(h) }.timers.borrow_mut();
    let before = timers.len();
    timers.retain(|t| t.id != id);
    before != timers.len()
}
unsafe extern "C" fn resize(h: *const clap_host, w: u32, v: u32) -> bool {
    if !(1..=16384).contains(&w) || !(1..=16384).contains(&v) || !unsafe { is_main(h) } {
        return false;
    }
    unsafe { data(h) }
        .shared()
        .emit(NativeNotification::Resize(w, v));
    true
}
unsafe extern "C" fn gui_closed(h: *const clap_host, destroyed: bool) {
    if destroyed {
        unsafe { data(h) }
            .gui_destroyed
            .store(true, Ordering::Release);
    }
    unsafe { data(h) }
        .shared()
        .emit(NativeNotification::EditorClosed);
}
unsafe extern "C" fn no_gui(_: *const clap_host) -> bool {
    false
}
unsafe extern "C" fn hints(_: *const clap_host) {}
static THREAD_CHECK: clap_host_thread_check = clap_host_thread_check {
    is_main_thread: Some(is_main),
    is_audio_thread: Some(is_audio),
};
static PARAMS: clap_host_params = clap_host_params {
    rescan: Some(rescan),
    clear: Some(clear),
    request_flush: Some(request_process),
};
static STATE: clap_host_state = clap_host_state {
    mark_dirty: Some(dirty),
};
static PORTS: clap_host_audio_ports = clap_host_audio_ports {
    is_rescan_flag_supported: Some(flags_supported),
    rescan: Some(ports_changed),
};
static NOTES: clap_host_note_ports = clap_host_note_ports {
    supported_dialects: Some(dialects),
    rescan: Some(ports_changed),
};
static LATENCY: clap_host_latency = clap_host_latency {
    changed: Some(latency_changed),
};
static TIMERS: clap_host_timer_support = clap_host_timer_support {
    register_timer: Some(register_timer),
    unregister_timer: Some(unregister_timer),
};
static GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(hints),
    request_resize: Some(resize),
    request_show: Some(no_gui),
    request_hide: Some(no_gui),
    closed: Some(gui_closed),
};
unsafe extern "C" fn extension(_: *const clap_host, id: *const c_char) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id = unsafe { CStr::from_ptr(id) };
    #[cfg(target_os = "linux")]
    if id == daw_plugin_clap::sys::ext::posix_fd_support::CLAP_EXT_POSIX_FD_SUPPORT {
        return (&fds::HOST
            as *const daw_plugin_clap::sys::ext::posix_fd_support::clap_host_posix_fd_support)
            .cast();
    }
    if id == CLAP_EXT_THREAD_CHECK {
        (&THREAD_CHECK as *const clap_host_thread_check).cast()
    } else if id == CLAP_EXT_PARAMS {
        (&PARAMS as *const clap_host_params).cast()
    } else if id == CLAP_EXT_STATE {
        (&STATE as *const clap_host_state).cast()
    } else if id == CLAP_EXT_AUDIO_PORTS {
        (&PORTS as *const clap_host_audio_ports).cast()
    } else if id == CLAP_EXT_NOTE_PORTS {
        (&NOTES as *const clap_host_note_ports).cast()
    } else if id == CLAP_EXT_LATENCY {
        (&LATENCY as *const clap_host_latency).cast()
    } else if id == CLAP_EXT_TIMER_SUPPORT {
        (&TIMERS as *const clap_host_timer_support).cast()
    } else if id == CLAP_EXT_GUI {
        (&GUI as *const clap_host_gui).cast()
    } else {
        ptr::null()
    }
}

pub(super) struct ClapInstance {
    plugin: *const clap_plugin,
    active: bool,
    editor: bool,
    params: *const clap_plugin_params,
    gui: *const clap_plugin_gui,
    _host: Box<clap_host>,
    data: Box<HostData>,
    _module: Rc<ClapModule>,
}
impl ClapInstance {
    unsafe fn ext<T>(&self, id: &CStr) -> *const T {
        unsafe {
            (*self.plugin)
                .get_extension
                .map_or(ptr::null(), |get| get(self.plugin, id.as_ptr()).cast())
        }
    }
    pub unsafe fn create(
        module: Rc<ClapModule>,
        id: &str,
        shared: &Shared,
        configuration: NativeConfiguration,
        state: Option<&NativeState>,
        commands: rtrb::Consumer<NativeEvent>,
    ) -> Result<(Self, ClapAudio, NativeMetadata)> {
        let rate = configuration.sample_rate_hz;
        let maximum = configuration.max_block_frames;
        let mut data = Box::new(HostData {
            shared,
            timers: RefCell::new(Vec::with_capacity(128)),
            #[cfg(target_os = "linux")]
            fds: RefCell::new(Vec::with_capacity(128)),
            next_timer: Cell::new(1),
            gui_destroyed: AtomicBool::new(false),
        });
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&mut *data as *mut HostData).cast(),
            name: c"DAW".as_ptr(),
            vendor: c"DAW".as_ptr(),
            url: c"https://github.com/fecabrera/daw".as_ptr(),
            version: c"0.1.0".as_ptr(),
            get_extension: Some(extension),
            request_restart: Some(request_restart),
            request_process: Some(request_process),
            request_callback: Some(request_callback),
        });
        let id =
            CString::new(id).map_err(|_| Error::Invalid("CLAP identity contains NUL".into()))?;
        let factory = module.factory();
        let plugin = unsafe {
            factory
                .create_plugin
                .ok_or_else(|| Error::Invalid("Missing CLAP factory constructor".into()))?(
                factory,
                &*host,
                id.as_ptr(),
            )
        };
        if plugin.is_null() {
            return Err(Error::Invalid("CLAP instance creation failed".into()));
        }
        let mut instance = Self {
            plugin,
            active: false,
            editor: false,
            params: ptr::null(),
            gui: ptr::null(),
            _host: host,
            data,
            _module: module,
        };
        let p = unsafe { &*plugin };
        if p.destroy.is_none()
            || p.activate.is_none()
            || p.deactivate.is_none()
            || p.start_processing.is_none()
            || p.stop_processing.is_none()
            || p.reset.is_none()
            || p.process.is_none()
        {
            return Err(Error::Invalid("CLAP lifecycle is incomplete".into()));
        }
        if !unsafe {
            p.init
                .ok_or_else(|| Error::Invalid("Missing CLAP init".into()))?(plugin)
        } {
            return Err(Error::Invalid("CLAP instance initialization failed".into()));
        }
        instance.params = unsafe { instance.ext(CLAP_EXT_PARAMS) };
        instance.gui = unsafe { instance.ext(CLAP_EXT_GUI) };
        if let Some(state) = state {
            instance.restore(state)?;
        }
        let parameters = instance.parameters()?;
        let inputs = instance.ports(true)?;
        let outputs = instance.ports(false)?;
        if outputs.len() != 1 {
            return Err(Error::Invalid("Plugin needs one mono/stereo main output; additional fixed outputs are unsupported".into()));
        }
        let note_dialect = instance.note_dialect()?;
        let render = unsafe { instance.ext::<clap_plugin_render>(CLAP_EXT_RENDER) };
        if !render.is_null() {
            let mode = if configuration.offline {
                CLAP_RENDER_OFFLINE
            } else {
                CLAP_RENDER_REALTIME
            };
            if !unsafe { (*render).set.is_some_and(|set| set(plugin, mode)) } {
                return Err(Error::Invalid(
                    "CLAP plugin rejected the requested rendering mode".into(),
                ));
            }
        }
        if !unsafe { p.activate.unwrap()(plugin, f64::from(rate), 1, maximum) } {
            return Err(Error::Invalid("CLAP activation failed".into()));
        }
        instance.active = true;
        let latency = unsafe { instance.ext::<clap_plugin_latency>(CLAP_EXT_LATENCY) };
        let tail = unsafe { instance.ext::<clap_plugin_tail>(CLAP_EXT_TAIL) };
        let latency_frames = if latency.is_null() {
            0
        } else {
            unsafe { (*latency).get.map_or(0, |get| get(plugin)) }
        };
        let tail_frames = if tail.is_null() {
            0
        } else {
            u64::from(unsafe { (*tail).get.map_or(0, |get| get(plugin)) })
        };
        let metadata = NativeMetadata {
            sample_rate_hz: rate,
            parameters,
            inputs,
            outputs,
            latency_frames,
            tail_frames,
            state_supported: !unsafe { instance.ext::<clap_plugin_state>(CLAP_EXT_STATE) }
                .is_null(),
            editor_supported: !instance.gui.is_null(),
        };
        let mut audio = ClapAudio::new(plugin, shared, &metadata, maximum, commands, note_dialect);
        audio.rate = rate;
        Ok((instance, audio, metadata))
    }
    fn parameters(&self) -> Result<Vec<HostedParameter>> {
        if self.params.is_null() {
            return Ok(Vec::new());
        }
        let ext = unsafe { &*self.params };
        let count = unsafe {
            ext.count
                .ok_or_else(|| Error::Invalid("CLAP parameter count is missing".into()))?(
                self.plugin,
            )
        };
        if count > 4096 {
            return Err(Error::Invalid("Plugin exceeds 4096 parameters".into()));
        }
        let mut parameters = Vec::with_capacity(count as usize);
        for n in 0..count {
            let mut info: clap_param_info = unsafe { std::mem::zeroed() };
            if !unsafe {
                ext.get_info
                    .ok_or_else(|| Error::Invalid("CLAP parameter metadata is missing".into()))?(
                    self.plugin,
                    n,
                    &mut info,
                )
            } {
                return Err(Error::Invalid("CLAP parameter enumeration failed".into()));
            }
            let p = HostedParameter {
                id: info.id,
                name: crate::scan::array_text(&info.name),
                module: crate::scan::array_text(&info.module),
                domain: HostedParameterDomain::Plain {
                    minimum: info.min_value,
                    maximum: info.max_value,
                    stepped: info.flags & CLAP_PARAM_IS_STEPPED != 0,
                },
                default_value: info.default_value,
                automatable: info.flags & CLAP_PARAM_IS_AUTOMATABLE != 0,
                modulatable: info.flags & CLAP_PARAM_IS_MODULATABLE != 0,
                read_only: info.flags & CLAP_PARAM_IS_READONLY != 0,
                hidden: info.flags & CLAP_PARAM_IS_HIDDEN != 0,
            };
            if !p.valid() || parameters.iter().any(|v: &HostedParameter| v.id == p.id) {
                return Err(Error::Invalid(
                    "Invalid or duplicate CLAP parameter metadata".into(),
                ));
            }
            parameters.push(p);
        }
        Ok(parameters)
    }
    fn ports(&self, input: bool) -> Result<Vec<NativeAudioPort>> {
        let ext = unsafe { self.ext::<clap_plugin_audio_ports>(CLAP_EXT_AUDIO_PORTS) };
        if ext.is_null() {
            return if input {
                Ok(Vec::new())
            } else {
                Err(Error::Invalid("Plugin has no audio output ports".into()))
            };
        }
        let ext = unsafe { &*ext };
        let count = unsafe {
            ext.count
                .ok_or_else(|| Error::Invalid("Missing CLAP port count".into()))?(
                self.plugin,
                input,
            )
        };
        if count > 32 {
            return Err(Error::Invalid("Plugin exceeds 32 audio ports".into()));
        }
        let mut ports = Vec::with_capacity(count as usize);
        for n in 0..count {
            let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
            if !unsafe {
                ext.get
                    .ok_or_else(|| Error::Invalid("Missing CLAP port metadata".into()))?(
                    self.plugin,
                    n,
                    input,
                    &mut info,
                )
            } || !(1..=2).contains(&info.channel_count)
                || info.id == u32::MAX
                || ports.iter().any(|p: &NativeAudioPort| p.id == info.id)
            {
                return Err(Error::Invalid(
                    "Plugin requires an unsupported or invalid audio layout".into(),
                ));
            }
            ports.push(NativeAudioPort {
                id: info.id,
                name: crate::scan::array_text(&info.name),
                channels: info.channel_count,
                main: info.flags & CLAP_AUDIO_PORT_IS_MAIN != 0,
            });
        }
        Ok(ports)
    }
    fn note_dialect(&self) -> Result<u32> {
        let ext = unsafe { self.ext::<clap_plugin_note_ports>(CLAP_EXT_NOTE_PORTS) };
        if ext.is_null() {
            return Ok(0);
        }
        let ext = unsafe { &*ext };
        let count = unsafe {
            ext.count
                .ok_or_else(|| Error::Invalid("Missing CLAP note port count".into()))?(
                self.plugin,
                true,
            )
        };
        if count == 0 {
            return Ok(0);
        }
        if count != 1 {
            return Err(Error::Invalid(
                "Plugin requires multiple note input ports".into(),
            ));
        }
        let mut info: clap_note_port_info = unsafe { std::mem::zeroed() };
        if !unsafe {
            ext.get
                .ok_or_else(|| Error::Invalid("Missing CLAP note port metadata".into()))?(
                self.plugin,
                0,
                true,
                &mut info,
            )
        } {
            return Err(Error::Invalid("CLAP note port enumeration failed".into()));
        }
        if info.supported_dialects & CLAP_NOTE_DIALECT_CLAP != 0 {
            Ok(CLAP_NOTE_DIALECT_CLAP)
        } else if info.supported_dialects & CLAP_NOTE_DIALECT_MIDI != 0 {
            Ok(CLAP_NOTE_DIALECT_MIDI)
        } else {
            Err(Error::Invalid(
                "Plugin requires an unsupported note dialect".into(),
            ))
        }
    }
    pub fn value(&self, id: u32) -> Result<f64> {
        let mut value = 0.0;
        if self.params.is_null()
            || !unsafe {
                (*self.params)
                    .get_value
                    .is_some_and(|get| get(self.plugin, id, &mut value))
            }
            || !value.is_finite()
        {
            return Err(Error::Invalid("Cannot read plugin parameter".into()));
        }
        Ok(value)
    }
    fn restore(&self, state: &NativeState) -> Result<()> {
        let NativeState::Clap(bytes) = state else {
            return Err(Error::Invalid("State format does not match CLAP".into()));
        };
        if bytes.len() > MAX_HOSTED_STATE_BYTES {
            return Err(Error::Invalid("Plugin state exceeds 16 MiB".into()));
        }
        let ext = unsafe { self.ext::<clap_plugin_state>(CLAP_EXT_STATE) };
        if ext.is_null() {
            return Err(Error::Invalid(
                "CLAP plugin does not support state recall".into(),
            ));
        }
        let mut reader = StateReader { bytes, offset: 0 };
        let stream = clap_istream {
            ctx: (&mut reader as *mut StateReader).cast(),
            read: Some(read_state),
        };
        if !unsafe { (*ext).load.is_some_and(|load| load(self.plugin, &stream)) } {
            return Err(Error::Invalid("CLAP state restore failed".into()));
        }
        Ok(())
    }
    pub fn text(&self, id: u32, value: f64) -> Result<String> {
        let extension = unsafe { self.ext::<clap_plugin_params>(CLAP_EXT_PARAMS) };
        if extension.is_null() {
            return Err(Error::Invalid("Plugin has no parameter formatter".into()));
        }
        let mut buffer: [c_char; 4096] = [0; 4096];
        if !unsafe {
            (*extension).value_to_text.is_some_and(|format| {
                format(
                    self.plugin,
                    id,
                    value,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                )
            })
        } {
            return Err(Error::Invalid("Plugin parameter formatting failed".into()));
        }
        Ok(crate::scan::array_text(&buffer))
    }
    pub fn save(&self) -> Result<NativeState> {
        let ext = unsafe { self.ext::<clap_plugin_state>(CLAP_EXT_STATE) };
        if ext.is_null() {
            return Err(Error::Invalid(
                "CLAP plugin is non-recallable: no state extension".into(),
            ));
        }
        let mut writer = StateWriter {
            bytes: Vec::new(),
            failed: false,
        };
        let stream = clap_ostream {
            ctx: (&mut writer as *mut StateWriter).cast(),
            write: Some(write_state),
        };
        if !unsafe { (*ext).save.is_some_and(|save| save(self.plugin, &stream)) } || writer.failed {
            return Err(Error::Invalid(
                "CLAP state capture failed or exceeded 16 MiB".into(),
            ));
        }
        Ok(NativeState::Clap(writer.bytes))
    }
    pub fn pump(&mut self) {
        #[cfg(target_os = "linux")]
        fds::pump(self);
        if self.data.shared().callback.swap(false, Ordering::AcqRel)
            && let Some(call) = unsafe { (*self.plugin).on_main_thread }
        {
            unsafe { call(self.plugin) };
        }
        let ext = unsafe { self.ext::<clap_plugin_timer_support>(CLAP_EXT_TIMER_SUPPORT) };
        if !ext.is_null() {
            let now = Instant::now();
            let mut due = [0u32; 128];
            let mut count = 0;
            {
                let mut timers = self.data.timers.borrow_mut();
                for timer in timers.iter_mut() {
                    if now >= timer.next {
                        due[count] = timer.id;
                        count += 1;
                        timer.next = now + timer.period;
                    }
                }
            }
            if let Some(call) = unsafe { (*ext).on_timer } {
                for id in &due[..count] {
                    unsafe { call(self.plugin, *id) };
                }
            }
        }
    }
    pub unsafe fn open_editor(&mut self, parent: NativeParent, scale: f64) -> Result<(u32, u32)> {
        if self.gui.is_null() {
            return Err(Error::Invalid("Plugin has no native editor".into()));
        }
        self.close_editor();
        let gui = unsafe { &*self.gui };
        let (api, handle, floating) = match parent {
            NativeParent::Cocoa(p) => (
                CLAP_WINDOW_API_COCOA,
                clap_window_handle { cocoa: p },
                false,
            ),
            NativeParent::Win32(p) => (
                CLAP_WINDOW_API_WIN32,
                clap_window_handle { win32: p },
                false,
            ),
            NativeParent::X11(p) => (
                CLAP_WINDOW_API_X11,
                clap_window_handle { x11: p as _ },
                false,
            ),
            NativeParent::Floating => (
                if cfg!(target_os = "macos") {
                    CLAP_WINDOW_API_COCOA
                } else if cfg!(target_os = "windows") {
                    CLAP_WINDOW_API_WIN32
                } else {
                    CLAP_WINDOW_API_X11
                },
                clap_window_handle {
                    ptr: ptr::null_mut(),
                },
                true,
            ),
        };
        if !unsafe {
            gui.is_api_supported
                .is_some_and(|check| check(self.plugin, api.as_ptr(), floating))
        } || !unsafe {
            gui.create
                .is_some_and(|create| create(self.plugin, api.as_ptr(), floating))
        } {
            return Err(Error::Invalid(
                "Plugin editor does not support this native window API".into(),
            ));
        }
        self.editor = true;
        if !floating {
            if api != CLAP_WINDOW_API_COCOA
                && scale.is_finite()
                && scale > 0.0
                && let Some(set) = gui.set_scale
            {
                unsafe { set(self.plugin, scale) };
            }
            let window = clap_window {
                api: api.as_ptr(),
                specific: handle,
            };
            if !unsafe { gui.set_parent.is_some_and(|set| set(self.plugin, &window)) } {
                self.close_editor();
                return Err(Error::Invalid("Cannot attach plugin editor".into()));
            }
        }
        let (mut w, mut h) = (0, 0);
        let valid = unsafe {
            gui.get_size
                .is_some_and(|get| get(self.plugin, &mut w, &mut h))
        };
        if !valid
            || w == 0
            || h == 0
            || w > 16384
            || h > 16384
            || !unsafe { gui.show.is_some_and(|show| show(self.plugin)) }
        {
            self.close_editor();
            return Err(Error::Invalid(
                "Cannot show plugin editor or invalid editor size".into(),
            ));
        }
        Ok((w, h))
    }
    pub fn editor_scale(&self, scale: f64) {
        if self.editor
            && !cfg!(target_os = "macos")
            && scale.is_finite()
            && scale > 0.0
            && let Some(set) = unsafe { (*self.gui).set_scale }
        {
            unsafe {
                set(self.plugin, scale);
            }
        }
    }
    pub fn editor_resizable(&self) -> bool {
        self.editor && unsafe { (*self.gui).can_resize.is_some_and(|can| can(self.plugin)) }
    }
    pub fn resize_editor(&mut self, mut width: u32, mut height: u32) -> Result<(u32, u32)> {
        if !self.editor {
            return Err(Error::Invalid("Plugin editor is closed".into()));
        }
        let gui = unsafe { &*self.gui };
        if self.editor_resizable()
            && let Some(adjust) = gui.adjust_size
            && !unsafe { adjust(self.plugin, &mut width, &mut height) }
        {
            return Err(Error::Invalid("Plugin rejected the editor size".into()));
        }
        if !(1..=16384).contains(&width)
            || !(1..=16384).contains(&height)
            || !unsafe {
                gui.set_size
                    .is_some_and(|set| set(self.plugin, width, height))
            }
        {
            return Err(Error::Invalid("Plugin editor resize failed".into()));
        }
        Ok((width, height))
    }
    pub fn close_editor(&mut self) {
        if self.data.gui_destroyed.swap(false, Ordering::AcqRel) {
            self.editor = false;
        }
        if self.editor && !self.gui.is_null() {
            unsafe {
                if let Some(hide) = (*self.gui).hide {
                    hide(self.plugin);
                }
                if let Some(destroy) = (*self.gui).destroy {
                    destroy(self.plugin);
                }
            }
            self.editor = false;
        }
    }
}
impl Drop for ClapInstance {
    fn drop(&mut self) {
        self.close_editor();
        unsafe {
            if self.active
                && let Some(deactivate) = (*self.plugin).deactivate
            {
                deactivate(self.plugin);
            }
            if let Some(destroy) = (*self.plugin).destroy {
                destroy(self.plugin);
            }
        }
    }
}

struct StateReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
unsafe extern "C" fn read_state(stream: *const clap_istream, out: *mut c_void, size: u64) -> i64 {
    if out.is_null() && size != 0 {
        return -1;
    }
    let reader = unsafe { &mut *((*stream).ctx.cast::<StateReader>()) };
    let count = (size.min(usize::MAX as u64) as usize).min(reader.bytes.len() - reader.offset);
    if count != 0 {
        unsafe {
            ptr::copy_nonoverlapping(reader.bytes.as_ptr().add(reader.offset), out.cast(), count);
        }
    }
    reader.offset += count;
    count as i64
}
struct StateWriter {
    bytes: Vec<u8>,
    failed: bool,
}
unsafe extern "C" fn write_state(
    stream: *const clap_ostream,
    input: *const c_void,
    size: u64,
) -> i64 {
    let writer = unsafe { &mut *((*stream).ctx.cast::<StateWriter>()) };
    if size > MAX_HOSTED_STATE_BYTES.saturating_sub(writer.bytes.len()) as u64
        || (input.is_null() && size != 0)
    {
        writer.failed = true;
        return -1;
    }
    if size != 0 {
        writer.bytes.extend_from_slice(unsafe {
            std::slice::from_raw_parts(input.cast::<u8>(), size as usize)
        });
    }
    size as i64
}

#[repr(C)]
#[derive(Clone, Copy)]
union Packet {
    value: clap_event_param_value,
    modulation: clap_event_param_mod,
    note: clap_event_note,
    midi: clap_event_midi,
}
fn header<T>(offset: u32, kind: u16) -> clap_event_header {
    clap_event_header {
        size: std::mem::size_of::<T>() as u32,
        time: offset,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_: kind,
        flags: 0,
    }
}
unsafe extern "C" fn event_count(list: *const clap_input_events) -> u32 {
    unsafe { &*((*list).ctx.cast::<Vec<Packet>>()) }.len() as u32
}
unsafe extern "C" fn event_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    unsafe { &*((*list).ctx.cast::<Vec<Packet>>()) }
        .get(index as usize)
        .map_or(ptr::null(), |p| (p as *const Packet).cast())
}
unsafe extern "C" fn output_event(
    list: *const clap_output_events,
    event: *const clap_event_header,
) -> bool {
    if event.is_null() {
        return false;
    }
    let h = unsafe { &*event };
    if h.space_id != CLAP_CORE_EVENT_SPACE_ID {
        return true;
    }
    let shared = unsafe { &*((*list).ctx.cast::<Shared>()) };
    match h.type_ {
        CLAP_EVENT_PARAM_VALUE
            if h.size as usize >= std::mem::size_of::<clap_event_param_value>() =>
        {
            let e = unsafe { &*event.cast::<clap_event_param_value>() };
            if !e.value.is_finite() {
                return false;
            }
            shared.emit(if h.flags & CLAP_EVENT_DONT_RECORD != 0 {
                NativeNotification::Telemetry(e.param_id, e.value)
            } else {
                NativeNotification::Value(e.param_id, e.value)
            });
        }
        CLAP_EVENT_PARAM_GESTURE_BEGIN | CLAP_EVENT_PARAM_GESTURE_END
            if h.size as usize >= std::mem::size_of::<clap_event_param_gesture>() =>
        {
            let e = unsafe { &*event.cast::<clap_event_param_gesture>() };
            shared.emit(if h.type_ == CLAP_EVENT_PARAM_GESTURE_BEGIN {
                NativeNotification::Begin(e.param_id)
            } else {
                NativeNotification::End(e.param_id)
            });
        }
        _ => {}
    }
    true
}

pub(super) struct ClapAudio {
    plugin: *const clap_plugin,
    shared: *const Shared,
    pub maximum: u32,
    pub rate: u32,
    inputs: Vec<Planar>,
    outputs: Vec<Planar>,
    input_abi: Vec<clap_audio_buffer>,
    output_abi: Vec<clap_audio_buffer>,
    events: Vec<Packet>,
    commands: rtrb::Consumer<NativeEvent>,
    parameters: Vec<AudioParameter>,
    note_dialect: u32,
}
struct AudioParameter {
    id: u32,
    domain: daw_unit_core::HostedParameterDomain,
    read_only: bool,
    modulatable: bool,
}
impl ClapAudio {
    fn new(
        plugin: *const clap_plugin,
        shared: &Shared,
        metadata: &NativeMetadata,
        maximum: u32,
        commands: rtrb::Consumer<NativeEvent>,
        note_dialect: u32,
    ) -> Self {
        let mut inputs: Vec<_> = metadata
            .inputs
            .iter()
            .map(|p| Planar::new(p.channels, maximum as usize))
            .collect();
        let mut outputs: Vec<_> = metadata
            .outputs
            .iter()
            .map(|p| Planar::new(p.channels, maximum as usize))
            .collect();
        let abi = |p: &mut Planar| clap_audio_buffer {
            data32: p.pointers.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: p.channels,
            latency: 0,
            constant_mask: 0,
        };
        let input_abi = inputs.iter_mut().map(abi).collect();
        let output_abi = outputs.iter_mut().map(abi).collect();
        Self {
            plugin,
            shared,
            maximum,
            rate: 48000,
            inputs,
            outputs,
            input_abi,
            output_abi,
            events: Vec::with_capacity(MAX_HOSTED_EVENTS),
            commands,
            parameters: {
                let mut parameters: Vec<_> = metadata
                    .parameters
                    .iter()
                    .map(|p| AudioParameter {
                        id: p.id,
                        domain: p.domain,
                        read_only: p.read_only,
                        modulatable: p.modulatable,
                    })
                    .collect();
                parameters.sort_unstable_by_key(|p| p.id);
                parameters
            },
            note_dialect,
        }
    }
    pub fn start(&mut self) -> bool {
        unsafe { (*self.plugin).start_processing.unwrap()(self.plugin) }
    }
    pub fn stop(&mut self) {
        unsafe { (*self.plugin).stop_processing.unwrap()(self.plugin) }
    }
    pub fn reset(&mut self) {
        unsafe { (*self.plugin).reset.unwrap()(self.plugin) }
    }
    pub fn flush(&mut self, _: bool) -> bool {
        self.events.clear();
        while let Ok(event) = self.commands.pop() {
            if !self.push(event) {
                return false;
            }
        }
        let params = unsafe {
            (*self.plugin).get_extension.map_or(ptr::null(), |get| {
                get(self.plugin, CLAP_EXT_PARAMS.as_ptr()).cast::<clap_plugin_params>()
            })
        };
        if params.is_null() {
            return self.events.is_empty();
        }
        let input = clap_input_events {
            ctx: (&self.events as *const Vec<Packet>).cast_mut().cast(),
            size: Some(event_count),
            get: Some(event_get),
        };
        let output = clap_output_events {
            ctx: self.shared.cast_mut().cast(),
            try_push: Some(output_event),
        };
        if let Some(flush) = unsafe { (*params).flush } {
            unsafe {
                flush(self.plugin, &input, &output);
            }
            true
        } else {
            self.events.is_empty()
        }
    }
    fn push(&mut self, event: NativeEvent) -> bool {
        if self.events.len() == self.events.capacity() {
            return false;
        }
        let packet = match event {
            NativeEvent::Parameter { offset, id, value } => {
                let Some(p) = self
                    .parameters
                    .binary_search_by_key(&id, |p| p.id)
                    .ok()
                    .map(|i| &self.parameters[i])
                    .filter(|p| !p.read_only)
                else {
                    return false;
                };
                let Some(value) = p.domain.constrain(value) else {
                    return false;
                };
                Packet {
                    value: clap_event_param_value {
                        header: header::<clap_event_param_value>(offset, CLAP_EVENT_PARAM_VALUE),
                        param_id: id,
                        cookie: ptr::null_mut(),
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        value,
                    },
                }
            }
            NativeEvent::Modulation { offset, id, amount } => {
                if !self
                    .parameters
                    .binary_search_by_key(&id, |p| p.id)
                    .ok()
                    .is_some_and(|i| self.parameters[i].modulatable)
                {
                    return false;
                }
                Packet {
                    modulation: clap_event_param_mod {
                        header: header::<clap_event_param_mod>(offset, CLAP_EVENT_PARAM_MOD),
                        param_id: id,
                        cookie: ptr::null_mut(),
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        amount,
                    },
                }
            }
            NativeEvent::Note {
                offset,
                on,
                id,
                channel,
                key,
                velocity,
            } => {
                if self.note_dialect == 0 {
                    return false;
                }
                if self.note_dialect == CLAP_NOTE_DIALECT_MIDI {
                    Packet {
                        midi: clap_event_midi {
                            header: header::<clap_event_midi>(offset, CLAP_EVENT_MIDI),
                            port_index: 0,
                            data: [
                                (if on { 0x90 } else { 0x80 }) | channel as u8,
                                key as u8,
                                (velocity * 127.0).round() as u8,
                            ],
                        },
                    }
                } else {
                    Packet {
                        note: clap_event_note {
                            header: header::<clap_event_note>(
                                offset,
                                if on {
                                    CLAP_EVENT_NOTE_ON
                                } else {
                                    CLAP_EVENT_NOTE_OFF
                                },
                            ),
                            note_id: id,
                            port_index: 0,
                            channel,
                            key,
                            velocity,
                        },
                    }
                }
            }
            NativeEvent::Midi { offset, data } => {
                if self.note_dialect == 0 {
                    return false;
                }
                Packet {
                    midi: clap_event_midi {
                        header: header::<clap_event_midi>(offset, CLAP_EVENT_MIDI),
                        port_index: 0,
                        data,
                    },
                }
            }
        };
        self.events.push(packet);
        true
    }
    pub fn process(
        &mut self,
        input: &[&[[f32; 2]]],
        out: &mut [[f32; 2]],
        events: &[NativeEvent],
        t: NativeTransport,
    ) -> bool {
        self.events.clear();
        while let Ok(event) = self.commands.pop() {
            if !self.push(event) {
                return false;
            }
        }
        for event in events {
            if !self.push(*event) {
                return false;
            }
        }
        // Commands have time zero; callers supply sorted sample-offset events.
        for (n, port) in self.inputs.iter_mut().enumerate() {
            port.input(input.get(n).copied(), out.len());
        }
        for port in &mut self.outputs {
            port.clear(out.len());
        }
        for p in &mut self.output_abi {
            p.constant_mask = 0;
        }
        let fixed = |q: f64| (q * 2147483648.0).clamp(i64::MIN as f64, i64::MAX as f64) as i64;
        if !t.quarter.is_finite()
            || !t.tempo.is_finite()
            || t.tempo <= 0.0
            || t.numerator == 0
            || t.denominator == 0
        {
            return false;
        }
        let mut transport: clap_event_transport = unsafe { std::mem::zeroed() };
        transport.header = header::<clap_event_transport>(0, CLAP_EVENT_TRANSPORT);
        transport.flags = CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
            | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
            | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;
        if t.playing {
            transport.flags |= CLAP_TRANSPORT_IS_PLAYING;
        }
        transport.song_pos_beats = fixed(t.quarter);
        transport.song_pos_seconds = fixed(t.frame as f64 / f64::from(self.rate));
        transport.tempo = t.tempo;
        transport.tsig_num = t.numerator;
        transport.tsig_denom = t.denominator;
        let bar = 4.0 * f64::from(t.numerator) / f64::from(t.denominator);
        transport.bar_start = fixed((t.quarter / bar).floor() * bar);
        transport.bar_number = (t.quarter / bar).floor().min(f64::from(i32::MAX)) as i32;
        if let Some((a, b)) = t.loop_quarters {
            if !a.is_finite() || !b.is_finite() || a >= b {
                return false;
            }
            transport.flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
            transport.loop_start_beats = fixed(a);
            transport.loop_end_beats = fixed(b);
        }
        let input_events = clap_input_events {
            ctx: (&self.events as *const Vec<Packet>).cast_mut().cast(),
            size: Some(event_count),
            get: Some(event_get),
        };
        let output_events = clap_output_events {
            ctx: self.shared.cast_mut().cast(),
            try_push: Some(output_event),
        };
        let process = clap_process {
            steady_time: t.frame.min(i64::MAX as u64) as i64,
            frames_count: out.len() as u32,
            transport: &transport,
            audio_inputs: self.input_abi.as_ptr(),
            audio_outputs: self.output_abi.as_mut_ptr(),
            audio_inputs_count: self.input_abi.len() as u32,
            audio_outputs_count: self.output_abi.len() as u32,
            in_events: &input_events,
            out_events: &output_events,
        };
        let status = unsafe { (*self.plugin).process.unwrap()(self.plugin, &process) };
        status != CLAP_PROCESS_ERROR && self.outputs[0].output(out)
    }
}
