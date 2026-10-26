use daw_drive_plugin::{Parameter, PluginState, Processor, StateExchange};
use daw_plugin_clap::{
    boundary,
    sys::{
        ext::{audio_ports::*, latency::*, params::*, state::*, tail::*},
        host::*,
        plugin::*,
    },
};
use std::{
    cell::UnsafeCell,
    ffi::{CStr, c_char, c_void},
    ptr,
    sync::atomic::{AtomicPtr, AtomicU8, AtomicU32, Ordering},
};

pub(crate) struct Audio {
    pub processor: Option<Processor>,
    pub state: PluginState,
    pub applied: u64,
    pub minimum: u32,
    pub maximum: u32,
}
impl Audio {
    pub fn set(&mut self, parameter: Parameter, value: f64) {
        if self.state.set(parameter, value).is_ok()
            && let Some(processor) = &mut self.processor
        {
            processor.apply_parameter(parameter, &self.state);
        }
    }
}
pub(crate) struct Instance {
    pub plugin: clap_plugin,
    pub host: *const clap_host,
    // 0 created, 1 initialized/inactive, 2 active/stopped, 3 processing.
    pub lifecycle: AtomicU8,
    pub rate: AtomicU32,
    pub host_params: AtomicPtr<clap_host_params>,
    pub exchange: StateExchange,
    pub editor: crate::editor::Bridge,
    audio: UnsafeCell<Audio>,
}
impl Instance {
    pub fn create(
        host: *const clap_host,
        descriptor: *const clap_plugin_descriptor,
    ) -> *const clap_plugin {
        let state = PluginState::default();
        let mut instance = Box::new(Self {
            plugin: clap_plugin {
                desc: descriptor,
                plugin_data: ptr::null_mut(),
                init: Some(init),
                destroy: Some(destroy),
                activate: Some(activate),
                deactivate: Some(deactivate),
                start_processing: Some(start),
                stop_processing: Some(stop),
                reset: Some(reset),
                process: Some(crate::audio::process),
                get_extension: Some(extension),
                on_main_thread: Some(on_main),
            },
            host,
            lifecycle: AtomicU8::new(0),
            rate: AtomicU32::new(48000),
            host_params: AtomicPtr::new(ptr::null_mut()),
            exchange: StateExchange::default(),
            editor: crate::editor::Bridge::default(),
            audio: UnsafeCell::new(Audio {
                processor: None,
                state,
                applied: 0,
                minimum: 1,
                maximum: 1,
            }),
        });
        instance.plugin.plugin_data = (&mut *instance as *mut Self).cast();
        let pointer = Box::into_raw(instance);
        // SAFETY: the host owns this live Box until destroy; its address is stable.
        unsafe { &(*pointer).plugin }
    }
    /// SAFETY: CLAP serializes mutable callbacks; caller follows main-inactive/audio-active ownership.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn audio(&self) -> &mut Audio {
        unsafe { &mut *self.audio.get() }
    }
    pub fn apply_pending(&self, audio: &mut Audio) {
        if let Some(state) = self.exchange.consume(&mut audio.applied) {
            audio.state = state;
            if let Some(processor) = &mut audio.processor {
                processor.restore(state);
            }
        }
    }
    pub fn publish(&self, audio: &Audio) {
        self.exchange.publish(audio.state, audio.applied);
        // SDK callback serialization owns both DSP and editor acknowledgements here.
        if unsafe { self.editor.publish() }
            && let Some(request) = unsafe { (*self.host).request_callback }
        {
            // CLAP permits request_callback from the audio thread; it schedules
            // the next control drain without touching GUI objects or their mutex.
            unsafe { request(self.host) };
        }
    }
    pub fn current(&self) -> Option<PluginState> {
        self.exchange.current()
    }
}
/// SAFETY: non-null plugin points to our live plugin structure until destroy.
pub(crate) unsafe fn instance<'a>(plugin: *const clap_plugin) -> Option<&'a Instance> {
    if plugin.is_null() {
        return None;
    }
    unsafe { ((*plugin).plugin_data as *const Instance).as_ref() }
}
unsafe extern "C" fn init(plugin: *const clap_plugin) -> bool {
    boundary(false, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        if i.lifecycle
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        // SAFETY: init is on the main thread, host remains live through destroy.
        if let Some(get) = unsafe { (*i.host).get_extension } {
            let params = unsafe { get(i.host, CLAP_EXT_PARAMS.as_ptr()) };
            i.host_params
                .store(params.cast_mut().cast(), Ordering::Release);
        }
        true
    })
}
unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    boundary((), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return;
        };
        if i.lifecycle.load(Ordering::Acquire) > 1 {
            return;
        }
        // SAFETY: CLAP requires exclusive main-thread inactive destruction, once.
        unsafe {
            drop(Box::from_raw((*plugin).plugin_data.cast::<Instance>()));
        }
    });
}
unsafe extern "C" fn activate(
    plugin: *const clap_plugin,
    rate: f64,
    minimum: u32,
    maximum: u32,
) -> bool {
    boundary(false, || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return false;
        };
        if i.lifecycle.load(Ordering::Acquire) != 1 {
            return false;
        }
        let audio = unsafe { i.audio() };
        i.apply_pending(audio);
        let Ok(mut processor) = Processor::new(rate, minimum, maximum, audio.state) else {
            return false;
        };
        processor.set_output_meter(i.editor.model.output_meter.clone());
        audio.processor = Some(processor);
        audio.minimum = minimum;
        audio.maximum = maximum;
        i.rate.store(rate as u32, Ordering::Release);
        i.lifecycle.store(2, Ordering::Release);
        true
    })
}
unsafe extern "C" fn deactivate(plugin: *const clap_plugin) {
    boundary((), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return;
        };
        if i.lifecycle.load(Ordering::Acquire) != 2 {
            return;
        }
        let audio = unsafe { i.audio() };
        i.apply_pending(audio);
        audio.processor = None;
        i.editor.model.output_meter.current()[0].store(0, Ordering::Relaxed); // Final DSP retirement stays on the main thread.
        i.publish(audio);
        i.lifecycle.store(1, Ordering::Release);
    });
}
unsafe extern "C" fn start(plugin: *const clap_plugin) -> bool {
    boundary(false, || {
        unsafe { instance(plugin) }.is_some_and(|i| {
            i.lifecycle
                .compare_exchange(2, 3, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        })
    })
}
unsafe extern "C" fn stop(plugin: *const clap_plugin) {
    boundary((), || {
        if let Some(i) = unsafe { instance(plugin) } {
            let _ = i
                .lifecycle
                .compare_exchange(3, 2, Ordering::AcqRel, Ordering::Acquire);
            i.editor.model.output_meter.current()[0].store(0, Ordering::Relaxed);
        }
    });
}
unsafe extern "C" fn reset(plugin: *const clap_plugin) {
    boundary((), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return;
        };
        if i.lifecycle.load(Ordering::Acquire) < 2 {
            return;
        }
        let audio = unsafe { i.audio() };
        i.apply_pending(audio);
        if let Some(processor) = &mut audio.processor {
            processor.reset();
        }
        i.publish(audio);
    });
}
unsafe extern "C" fn extension(plugin: *const clap_plugin, id: *const c_char) -> *const c_void {
    boundary(ptr::null(), || {
        let Some(i) = (unsafe { instance(plugin) }) else {
            return ptr::null();
        };
        if id.is_null() || i.lifecycle.load(Ordering::Acquire) == 0 {
            return ptr::null();
        }
        // SAFETY: non-null extension ID is a host-provided C string.
        let id = unsafe { CStr::from_ptr(id) };
        if id == daw_plugin_clap::sys::ext::gui::CLAP_EXT_GUI {
            (&crate::editor::GUI as *const daw_plugin_clap::sys::ext::gui::clap_plugin_gui).cast()
        } else if id == CLAP_EXT_PARAMS {
            (&crate::params::PARAMS as *const clap_plugin_params).cast()
        } else if id == CLAP_EXT_STATE {
            (&crate::state::STATE as *const clap_plugin_state).cast()
        } else if id == CLAP_EXT_AUDIO_PORTS {
            (&crate::audio::PORTS as *const clap_plugin_audio_ports).cast()
        } else if id == CLAP_EXT_LATENCY {
            (&LATENCY as *const clap_plugin_latency).cast()
        } else if id == CLAP_EXT_TAIL {
            (&TAIL as *const clap_plugin_tail).cast()
        } else {
            ptr::null()
        }
    })
}
unsafe extern "C" fn on_main(plugin: *const clap_plugin) {
    boundary((), || {
        if let Some(i) = unsafe { instance(plugin) } {
            i.editor.main(i);
        }
    });
}
unsafe extern "C" fn latency(plugin: *const clap_plugin) -> u32 {
    boundary(0, || {
        daw_drive_plugin::latency_frames(
            unsafe { instance(plugin) }.map_or(48000, |i| i.rate.load(Ordering::Acquire)),
        )
    })
}
unsafe extern "C" fn tail(plugin: *const clap_plugin) -> u32 {
    boundary(48000, || {
        daw_drive_plugin::tail_frames(
            unsafe { instance(plugin) }.map_or(48000, |i| i.rate.load(Ordering::Acquire)),
        )
    })
}
static LATENCY: clap_plugin_latency = clap_plugin_latency { get: Some(latency) };
static TAIL: clap_plugin_tail = clap_plugin_tail { get: Some(tail) };
