//! COM view ownership. Host parameter callbacks stay on the host UI thread.
use daw_drive_editor::{Edit, Editor, HEIGHT, Model, WIDTH};
use daw_drive_plugin::PluginState;
#[cfg(target_os = "linux")]
use daw_plugin_vst3::sys::Steinberg::Linux::IRunLoopTrait;
use daw_plugin_vst3::{
    boundary,
    sys::{
        Class, ComPtr, ComRef, ComWrapper,
        Steinberg::{Vst::*, *},
    },
};
use daw_plugin_window::{Parent, Window};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_void},
    rc::Rc,
    sync::Arc,
};
#[derive(Default)]
pub(crate) struct ControllerData {
    pub state: Cell<PluginState>,
    attached: Cell<bool>,
    pub handler: RefCell<Option<ComPtr<IComponentHandler>>>,
    pub model: Arc<Model>,
}
impl ControllerData {
    pub fn main(&self) {
        // Clone before calling the host: its callback may synchronously reenter the controller.
        let handler = self.handler.borrow().clone();
        while let Some(edit) = self.model.pop() {
            match edit {
                Edit::Begin(p) => {
                    if let Some(h) = &handler {
                        unsafe {
                            h.beginEdit(p as u32);
                        }
                    }
                }
                Edit::Value(p, v) => {
                    let mut state = self.state.get();
                    if state.set(p, v).is_ok() {
                        self.state.set(state);
                    }
                    if let (Some(h), Ok(value)) = (&handler, p.normalize(v)) {
                        unsafe {
                            h.performEdit(p as u32, value);
                        }
                    }
                }
                Edit::End(p) => {
                    if let Some(h) = &handler {
                        unsafe {
                            h.endEdit(p as u32);
                        }
                    }
                }
            }
        }
        self.model.publish(self.state.get());
    }
}
#[cfg(target_os = "macos")]
const PLATFORM: &CStr = c"NSView";
#[cfg(target_os = "windows")]
const PLATFORM: &CStr = c"HWND";
#[cfg(target_os = "linux")]
const PLATFORM: &CStr = c"X11EmbedWindowID";
pub(crate) struct View {
    data: Rc<ControllerData>,
    window: Rc<RefCell<Option<Window>>>,
    frame: RefCell<Option<ComPtr<IPlugFrame>>>,
    scale: Cell<f32>,
    pointer: Cell<*mut IPlugView>,
    #[cfg(target_os = "linux")]
    timer: RefCell<Option<(ComPtr<Linux::IRunLoop>, ComPtr<Linux::ITimerHandler>)>>,
}
impl View {
    pub fn create(data: Rc<ControllerData>) -> *mut IPlugView {
        let wrapper = ComWrapper::new(Self {
            data,
            window: Rc::default(),
            frame: RefCell::default(),
            scale: Cell::new(1.0),
            pointer: Cell::new(std::ptr::null_mut()),
            #[cfg(target_os = "linux")]
            timer: RefCell::default(),
        });
        let pointer = wrapper
            .to_com_ptr::<IPlugView>()
            .map_or(std::ptr::null_mut(), |p| p.into_raw());
        wrapper.pointer.set(pointer);
        pointer
    }
    fn close(&self) {
        #[cfg(target_os = "linux")]
        if let Some((runloop, timer)) = self.timer.borrow_mut().take() {
            unsafe {
                runloop.unregisterTimer(timer.as_ptr());
            }
        }
        let window = self.window.borrow_mut().take();
        if window.is_some() {
            self.data.attached.set(false);
        }
        drop(window);
        self.data.main();
    }
    fn size(&self) -> ViewRect {
        let scale = if cfg!(target_os = "macos") {
            1.0
        } else {
            self.scale.get()
        };
        ViewRect {
            left: 0,
            top: 0,
            right: (WIDTH as f32 * scale).round() as i32,
            bottom: (HEIGHT as f32 * scale).round() as i32,
        }
    }
}
impl Drop for View {
    fn drop(&mut self) {
        self.close();
    }
}
impl Class for View {
    type Interfaces = (IPlugView, IPlugViewContentScaleSupport);
}
impl IPlugViewTrait for View {
    unsafe fn isPlatformTypeSupported(&self, kind: FIDString) -> tresult {
        if !kind.is_null() && unsafe { CStr::from_ptr(kind) } == PLATFORM {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn attached(&self, parent: *mut c_void, kind: FIDString) -> tresult {
        boundary(kInternalError, || {
            if unsafe { self.isPlatformTypeSupported(kind) } != kResultOk
                || self.data.attached.get()
            {
                return kResultFalse;
            }
            let Some(parent) = (unsafe { Parent::new(parent) }) else {
                return kInvalidArgument;
            };
            self.data.model.publish(self.data.state.get());
            #[cfg(target_os = "linux")]
            if self.timer.borrow().is_none() {
                let Some(frame) = self.frame.borrow().clone() else {
                    return kResultFalse;
                };
                let Some(runloop) = frame.cast::<Linux::IRunLoop>() else {
                    return kResultFalse;
                };
                let timer = ComWrapper::new(Timer {
                    data: self.data.clone(),
                    window: self.window.clone(),
                })
                .to_com_ptr::<Linux::ITimerHandler>()
                .unwrap();
                if unsafe { runloop.registerTimer(timer.as_ptr(), 16) } != kResultOk {
                    return kResultFalse;
                }
                *self.timer.borrow_mut() = Some((runloop, timer));
            }
            #[cfg(not(target_os = "linux"))]
            let address = Rc::as_ptr(&self.data) as usize;
            let app = Editor::new(self.data.model.clone(), move || {
                #[cfg(not(target_os = "linux"))]
                // Cocoa/Win32 paint runs on the host UI thread. View owns this Rc
                // until Window::drop has synchronously retired the callback.
                unsafe {
                    (&*(address as *const ControllerData)).main();
                }
            });
            self.data.model.scale.store(
                self.scale.get().to_bits(),
                std::sync::atomic::Ordering::Relaxed,
            );
            let zoom = if cfg!(target_os = "macos") {
                1.0
            } else {
                self.scale.get()
            };
            let Ok(window) = daw_plugin_window::create(
                app,
                &parent,
                WIDTH as f64,
                HEIGHT as f64,
                zoom,
                daw_plugin_window::host(|| {}),
            ) else {
                return kResultFalse;
            };
            if window.show().is_err() {
                return kResultFalse;
            }
            *self.window.borrow_mut() = Some(window);
            self.data.attached.set(true);
            kResultOk
        })
    }
    unsafe fn removed(&self) -> tresult {
        boundary(kInternalError, || {
            self.close();
            kResultOk
        })
    }
    unsafe fn onWheel(&self, _: f32) -> tresult {
        kResultFalse
    }
    unsafe fn onKeyDown(&self, _: char16, _: int16, _: int16) -> tresult {
        kResultFalse
    }
    unsafe fn onKeyUp(&self, _: char16, _: int16, _: int16) -> tresult {
        kResultFalse
    }
    unsafe fn getSize(&self, output: *mut ViewRect) -> tresult {
        if output.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            output.write(self.size());
        }
        kResultOk
    }
    unsafe fn onSize(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let rect = unsafe { &*rect };
        let size = self.size();
        if rect.right - rect.left != size.right || rect.bottom - rect.top != size.bottom {
            kResultFalse
        } else {
            kResultOk
        }
    }
    unsafe fn onFocus(&self, _: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setFrame(&self, frame: *mut IPlugFrame) -> tresult {
        boundary(kInternalError, || {
            *self.frame.borrow_mut() = unsafe { ComRef::from_raw(frame) }.map(|f| f.to_com_ptr());
            kResultOk
        })
    }
    unsafe fn canResize(&self) -> tresult {
        kResultFalse
    }
    unsafe fn checkSizeConstraint(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let size = self.size();
        let rect = unsafe { &mut *rect };
        rect.right = rect.left + size.right;
        rect.bottom = rect.top + size.bottom;
        kResultOk
    }
}
impl IPlugViewContentScaleSupportTrait for View {
    unsafe fn setContentScaleFactor(&self, factor: f32) -> tresult {
        boundary(kInternalError, || {
            if !factor.is_finite() || !(0.5..=4.0).contains(&factor) {
                return kInvalidArgument;
            }
            if cfg!(target_os = "macos") {
                return kResultOk;
            }
            let previous = self.scale.replace(factor);
            let frame = self.frame.borrow().clone();
            if self.window.borrow().is_some() {
                let Some(frame) = frame else {
                    self.scale.set(previous);
                    return kResultFalse;
                };
                let mut rect = self.size();
                if unsafe { frame.resizeView(self.pointer.get(), &mut rect) } != kResultOk {
                    self.scale.set(previous);
                    return kResultFalse;
                }
                if let Some(window) = self.window.borrow().as_ref()
                    && !daw_plugin_window::resize(window, rect.right as f64, rect.bottom as f64)
                {
                    self.scale.set(previous);
                    return kResultFalse;
                }
            }
            self.data
                .model
                .scale
                .store(factor.to_bits(), std::sync::atomic::Ordering::Relaxed);
            kResultOk
        })
    }
}
#[cfg(target_os = "linux")]
struct Timer {
    data: Rc<ControllerData>,
    window: Rc<RefCell<Option<Window>>>,
}
#[cfg(target_os = "linux")]
impl Class for Timer {
    type Interfaces = (Linux::ITimerHandler,);
}
#[cfg(target_os = "linux")]
impl Linux::ITimerHandlerTrait for Timer {
    unsafe fn onTimer(&self) {
        boundary((), || {
            self.data.main();
            if let Some(window) = self.window.borrow().as_ref() {
                window.host_main_thread_callback();
            }
        });
    }
}
