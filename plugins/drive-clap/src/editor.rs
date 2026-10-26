//! Main-thread native GUI and bounded main-to-audio edit delivery.
use crate::instance::{Audio, Instance, instance};
use daw_drive_editor::{Edit, Editor, HEIGHT, Model, WIDTH};
use daw_plugin_clap::{
    boundary,
    sys::{events::*, ext::gui::*, plugin::*},
};
use daw_plugin_window::{Parent, Window};
use std::{
    cell::{Cell, RefCell, UnsafeCell},
    ffi::{CStr, c_char},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
const CAPACITY: usize = 512;
pub(crate) struct Gui {
    pub created: bool,
    pub scale: f64,
    pub window: Option<Window>,
}
impl Default for Gui {
    fn default() -> Self {
        Self {
            created: false,
            scale: 1.0,
            window: None,
        }
    }
}
pub(crate) struct Bridge {
    pub model: Arc<Model>,
    pub gui: RefCell<Gui>,
    producer: RefCell<rtrb::Producer<(Edit, u64)>>,
    consumer: UnsafeCell<rtrb::Consumer<(Edit, u64)>>,
    pending: UnsafeCell<Option<(Edit, u64)>>,
    sent: Cell<u64>,
    applied: AtomicU64,
    consumed: UnsafeCell<u64>,
    pending_values: Cell<[u64; 7]>,
    main_pending: AtomicBool,
    retry: UnsafeCell<bool>,
}
impl Default for Bridge {
    fn default() -> Self {
        let (producer, consumer) = rtrb::RingBuffer::new(CAPACITY);
        Self {
            model: Arc::default(),
            gui: RefCell::default(),
            producer: RefCell::new(producer),
            consumer: UnsafeCell::new(consumer),
            pending: UnsafeCell::new(None),
            sent: Cell::new(0),
            applied: AtomicU64::new(0),
            consumed: UnsafeCell::new(0),
            pending_values: Cell::new([0; 7]),
            main_pending: AtomicBool::new(false),
            retry: UnsafeCell::new(false),
        }
    }
}
impl Bridge {
    pub fn main(&self, i: &Instance) {
        if let Some(window) = self.gui.borrow().window.as_ref() {
            window.host_main_thread_callback();
        }
        let mut desired = self.model.current().unwrap_or_default();
        let mut producer = self.producer.borrow_mut();
        let blocked = loop {
            if producer.slots() == 0 {
                break true;
            }
            let Some(edit) = self.model.pop() else {
                break false;
            };
            let seq = self.sent.get().wrapping_add(1);
            if producer.push((edit, seq)).is_err() {
                break true;
            }
            self.sent.set(seq);
            if let Edit::Value(p, v) = edit {
                let _ = desired.set(p, v);
                let mut pending = self.pending_values.get();
                // Stable contiguous IDs are fixed by the shared Drive parameter contract.
                pending[p as usize - 100] = seq;
                self.pending_values.set(pending);
            }
        };
        self.main_pending.store(blocked, Ordering::Release);
        // A host may synchronously flush on the control thread. Do not retain
        // the producer borrow across any callback into that host.
        drop(producer);
        let applied = self.applied.load(Ordering::Acquire);
        if let Some(current) = i.current() {
            for (index, p) in daw_drive_plugin::Parameter::ALL.into_iter().enumerate() {
                if self.pending_values.get()[index] <= applied {
                    let _ = desired.set(p, current.get(p));
                }
            }
        }
        self.model.publish(desired);
        if applied != self.sent.get() || blocked {
            let params = i.host_params.load(Ordering::Acquire);
            if !params.is_null()
                && let Some(request) = unsafe { (*params).request_flush }
            {
                unsafe { request(i.host) };
            } else if let Some(request) = unsafe { (*i.host).request_process } {
                unsafe { request(i.host) };
            }
        }
    }
    /// SAFETY: called by the audio owner only after the matching state snapshot is published.
    pub unsafe fn publish(&self) -> bool {
        self.applied
            .store(unsafe { *self.consumed.get() }, Ordering::Release);
        // A closed editor no longer supplies frame wakeups. Keep control-side
        // draining and output retries alive until every accepted End is delivered.
        self.main_pending.load(Ordering::Acquire) || unsafe { *self.retry.get() }
    }
    /// SAFETY: same serialized process/flush ownership as Instance::audio. Never locks or allocates.
    pub unsafe fn flush(&self, audio: &mut Audio, output: *const clap_output_events) {
        let consumer = unsafe { &mut *self.consumer.get() };
        let pending = unsafe { &mut *self.pending.get() };
        let retry = unsafe { &mut *self.retry.get() };
        *retry = false;
        let push = if output.is_null() {
            None
        } else {
            unsafe { (*output).try_push }
        };
        let Some(push) = push else {
            *retry = pending.is_some() || !consumer.is_empty();
            return;
        };
        for _ in 0..CAPACITY {
            let Some((edit, seq)) = pending.take().or_else(|| consumer.pop().ok()) else {
                break;
            };
            let header = clap_event_header {
                size: 0,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: 0,
                flags: CLAP_EVENT_IS_LIVE,
            };
            let ok = match edit {
                Edit::Begin(p) | Edit::End(p) => {
                    let event = clap_event_param_gesture {
                        header: clap_event_header {
                            size: size_of::<clap_event_param_gesture>() as u32,
                            type_: if matches!(edit, Edit::Begin(_)) {
                                CLAP_EVENT_PARAM_GESTURE_BEGIN
                            } else {
                                CLAP_EVENT_PARAM_GESTURE_END
                            },
                            ..header
                        },
                        param_id: p as u32,
                    };
                    unsafe { push(output, &event.header) }
                }
                Edit::Value(p, v) => {
                    let event = clap_event_param_value {
                        header: clap_event_header {
                            size: size_of::<clap_event_param_value>() as u32,
                            type_: CLAP_EVENT_PARAM_VALUE,
                            ..header
                        },
                        param_id: p as u32,
                        cookie: ptr::null_mut(),
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        value: v,
                    };
                    let ok = unsafe { push(output, &event.header) };
                    if ok {
                        audio.set(p, v);
                    }
                    ok
                }
            };
            if !ok {
                *pending = Some((edit, seq));
                *retry = true;
                return;
            }
            unsafe {
                *self.consumed.get() = seq;
            }
        }
        *retry = pending.is_some() || !consumer.is_empty();
    }
}
#[cfg(target_os = "macos")]
const API: &CStr = CLAP_WINDOW_API_COCOA;
#[cfg(target_os = "windows")]
const API: &CStr = CLAP_WINDOW_API_WIN32;
#[cfg(target_os = "linux")]
const API: &CStr = CLAP_WINDOW_API_X11;
unsafe extern "C" fn supported(_: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    !floating && !api.is_null() && unsafe { CStr::from_ptr(api) } == API
}
unsafe extern "C" fn preferred(
    _: *const clap_plugin,
    api: *mut *const c_char,
    floating: *mut bool,
) -> bool {
    if api.is_null() || floating.is_null() {
        return false;
    }
    unsafe {
        api.write(API.as_ptr());
        floating.write(false);
    }
    true
}
unsafe extern "C" fn create(
    plugin: *const clap_plugin,
    api: *const c_char,
    floating: bool,
) -> bool {
    boundary(false, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        if !unsafe { supported(plugin, api, floating) } {
            return false;
        }
        let mut gui = i.editor.gui.borrow_mut();
        if gui.created {
            return false;
        }
        gui.created = true;
        if let Some(state) = i.current() {
            i.editor.model.publish(state);
        }
        true
    })
}
unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    boundary((), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return;
        };
        let window = {
            let mut gui = i.editor.gui.borrow_mut();
            gui.created = false;
            gui.window.take()
        };
        drop(window);
        i.editor.main(i);
    });
}
unsafe extern "C" fn scale(plugin: *const clap_plugin, value: f64) -> bool {
    boundary(false, || {
        // Cocoa sizes are logical points; the native window supplies its backing scale.
        if cfg!(target_os = "macos") || !value.is_finite() || !(0.5..=4.0).contains(&value) {
            return false;
        }
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        let mut gui = i.editor.gui.borrow_mut();
        if let Some(window) = &gui.window
            && !daw_plugin_window::resize(window, WIDTH as f64 * value, HEIGHT as f64 * value)
        {
            return false;
        }
        i.editor
            .model
            .scale
            .store((value as f32).to_bits(), Ordering::Relaxed);
        gui.scale = value;
        true
    })
}
unsafe extern "C" fn size(plugin: *const clap_plugin, w: *mut u32, h: *mut u32) -> bool {
    boundary(false, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        if w.is_null() || h.is_null() {
            return false;
        }
        let gui = i.editor.gui.borrow();
        if !gui.created {
            return false;
        }
        unsafe {
            w.write((WIDTH as f64 * gui.scale).round() as u32);
            h.write((HEIGHT as f64 * gui.scale).round() as u32);
        }
        true
    })
}
unsafe extern "C" fn parent(plugin: *const clap_plugin, window: *const clap_window) -> bool {
    boundary(false, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        if window.is_null() || !unsafe { supported(plugin, (*window).api, false) } {
            return false;
        }
        let pointer = unsafe { (*window).specific.ptr };
        let Some(parent) = (unsafe { Parent::new(pointer) }) else {
            return false;
        };
        let mut gui = i.editor.gui.borrow_mut();
        if !gui.created || gui.window.is_some() {
            return false;
        }
        let host = i.host as usize;
        i.editor
            .model
            .scale
            .store((gui.scale as f32).to_bits(), Ordering::Relaxed);
        let editor = Editor::new(i.editor.model.clone(), move || {
            // request_callback is thread-safe, including the X11 window thread.
            let host = host as *const daw_plugin_clap::sys::host::clap_host;
            if let Some(request) = unsafe { (*host).request_callback } {
                unsafe { request(host) };
            }
        });
        let Ok(window) = daw_plugin_window::create(
            editor,
            &parent,
            WIDTH as f64,
            HEIGHT as f64,
            gui.scale as f32,
            daw_plugin_window::host(move || {
                let host = host as *const daw_plugin_clap::sys::host::clap_host;
                if let Some(request) = unsafe { (*host).request_callback } {
                    unsafe { request(host) };
                }
            }),
        ) else {
            return false;
        };
        gui.window = Some(window);
        true
    })
}
unsafe extern "C" fn show(plugin: *const clap_plugin) -> bool {
    boundary(false, || {
        unsafe { instance(plugin) }.is_some_and(|i| {
            i.editor
                .gui
                .borrow()
                .window
                .as_ref()
                .is_some_and(|w| w.show().is_ok())
        })
    })
}
unsafe extern "C" fn hide(plugin: *const clap_plugin) -> bool {
    boundary(false, || {
        unsafe { instance(plugin) }.is_some_and(|i| {
            i.editor
                .gui
                .borrow()
                .window
                .as_ref()
                .is_some_and(|w| w.hide().is_ok())
        })
    })
}
unsafe extern "C" fn hints(_: *const clap_plugin, _: *mut clap_gui_resize_hints) -> bool {
    false
}
unsafe extern "C" fn no(_: *const clap_plugin) -> bool {
    false
}
unsafe extern "C" fn set_size(p: *const clap_plugin, w: u32, h: u32) -> bool {
    let (mut ew, mut eh) = (0, 0);
    (unsafe { size(p, &mut ew, &mut eh) }) && w == ew && h == eh
}
unsafe extern "C" fn transient(_: *const clap_plugin, _: *const clap_window) -> bool {
    false
}
unsafe extern "C" fn title(_: *const clap_plugin, _: *const c_char) {}
pub(crate) static GUI: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: Some(supported),
    get_preferred_api: Some(preferred),
    create: Some(create),
    destroy: Some(destroy),
    set_scale: Some(scale),
    get_size: Some(size),
    can_resize: Some(no),
    get_resize_hints: Some(hints),
    adjust_size: Some(size),
    set_size: Some(set_size),
    set_parent: Some(parent),
    set_transient: Some(transient),
    suggest_title: Some(title),
    show: Some(show),
    hide: Some(hide),
};

#[cfg(test)]
mod tests {
    use super::*;
    use daw_drive_plugin::{Parameter, PluginState};
    struct Output {
        room: usize,
        types: Vec<u16>,
    }
    unsafe extern "C" fn push(
        out: *const clap_output_events,
        event: *const clap_event_header,
    ) -> bool {
        let output = unsafe { &mut *((*out).ctx as *mut Output) };
        if output.room == 0 {
            return false;
        }
        output.room -= 1;
        output.types.push(unsafe { (*event).type_ });
        true
    }
    fn checked_flush(bridge: &Bridge, audio: &mut Audio, events: &clap_output_events) {
        #[cfg(feature = "allocation-check")]
        crate::allocations::drive_clap_allocation_begin();
        unsafe {
            bridge.flush(audio, events);
        }
        #[cfg(feature = "allocation-check")]
        {
            let (mut allocated, mut retired) = (0, 0);
            unsafe {
                crate::allocations::drive_clap_allocation_end(&mut allocated, &mut retired);
            }
            assert_eq!((allocated, retired), (0, 0));
        }
    }
    #[test]
    fn rejected_host_events_retry_in_order_without_losing_value_or_end() {
        let bridge = Bridge::default();
        for (index, edit) in [
            Edit::Begin(Parameter::Gain),
            Edit::Value(Parameter::Gain, 0.7),
            Edit::End(Parameter::Gain),
        ]
        .into_iter()
        .enumerate()
        {
            bridge
                .producer
                .borrow_mut()
                .push((edit, index as u64 + 1))
                .unwrap();
        }
        let mut audio = Audio {
            processor: None,
            state: PluginState::default(),
            applied: 0,
            minimum: 1,
            maximum: 64,
        };
        let mut output = Output {
            room: 0,
            types: Vec::with_capacity(3),
        };
        let events = clap_output_events {
            ctx: (&mut output as *mut Output).cast(),
            try_push: Some(push),
        };
        checked_flush(&bridge, &mut audio, &events);
        assert_eq!(
            audio.state.get(Parameter::Gain),
            PluginState::default().get(Parameter::Gain)
        );
        for expected in [
            CLAP_EVENT_PARAM_GESTURE_BEGIN,
            CLAP_EVENT_PARAM_VALUE,
            CLAP_EVENT_PARAM_GESTURE_END,
        ] {
            output.room = 1;
            checked_flush(&bridge, &mut audio, &events);
            assert_eq!(*output.types.last().unwrap(), expected);
        }
        assert_eq!(audio.state.get(Parameter::Gain), 0.7);
        assert_eq!(bridge.applied.load(Ordering::Acquire), 0);
        // An event acknowledgement cannot overtake its matching audio-state publication.
        unsafe {
            bridge.publish();
        }
        assert_eq!(bridge.applied.load(Ordering::Acquire), 3);
        assert_eq!(output.types.len(), 3);
    }
    #[test]
    fn closed_editor_gets_a_control_wakeup_to_drain_a_full_ring_and_deliver_end() {
        use daw_plugin_clap::sys::{host::clap_host, version::CLAP_VERSION};
        unsafe extern "C" fn wake(host: *const clap_host) {
            let count = unsafe { &*((*host).host_data.cast::<AtomicU64>()) };
            count.fetch_add(1, Ordering::Relaxed);
        }
        let wakeups = AtomicU64::new(0);
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&wakeups as *const AtomicU64).cast_mut().cast(),
            name: c"queue test".as_ptr(),
            vendor: c"Sie".as_ptr(),
            url: c"".as_ptr(),
            version: c"0.1".as_ptr(),
            get_extension: None,
            request_restart: None,
            request_process: None,
            request_callback: Some(wake),
        };
        let plugin = Instance::create(&host, ptr::null());
        let i = unsafe { instance(plugin) }.unwrap();
        {
            let mut producer = i.editor.producer.borrow_mut();
            for index in 0..CAPACITY {
                let edit = match index % 3 {
                    0 => Edit::Begin(Parameter::Gain),
                    1 => Edit::Value(Parameter::Gain, 0.7),
                    _ => Edit::End(Parameter::Gain),
                };
                producer.push((edit, index as u64 + 1)).unwrap();
            }
        }
        i.editor.sent.set(CAPACITY as u64);
        assert!(i.editor.model.push(&[Edit::End(Parameter::Gain)]));
        i.editor.main(i);
        assert!(i.editor.main_pending.load(Ordering::Acquire));
        assert!(!i.editor.gui.borrow().created);
        let mut output = Output {
            room: CAPACITY,
            types: Vec::with_capacity(CAPACITY + 1),
        };
        let events = clap_output_events {
            ctx: (&mut output as *mut Output).cast(),
            try_push: Some(push),
        };
        {
            let audio = unsafe { i.audio() };
            checked_flush(&i.editor, audio, &events);
            i.publish(audio);
        }
        assert_eq!(wakeups.load(Ordering::Relaxed), 1);
        // Simulate that scheduled callback after the editor has already closed.
        i.editor.main(i);
        assert!(!i.editor.main_pending.load(Ordering::Acquire));
        output.room = 1;
        {
            let audio = unsafe { i.audio() };
            checked_flush(&i.editor, audio, &events);
            i.publish(audio);
        }
        assert_eq!(output.types.len(), CAPACITY + 1);
        assert_eq!(output.types.last(), Some(&CLAP_EVENT_PARAM_GESTURE_END));
        assert_eq!(
            i.editor.applied.load(Ordering::Acquire),
            CAPACITY as u64 + 1
        );
        unsafe {
            ((*plugin).destroy.unwrap())(plugin);
        }
    }
}
