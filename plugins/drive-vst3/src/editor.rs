//! COM view ownership. Host parameter callbacks stay on the host UI thread.
use daw_drive_editor::{Edit, Editor, HEIGHT, Model, WIDTH};
use daw_drive_plugin::BandAudition;
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
    pub meter_connection: crate::telemetry::Connection,
    pub state: Cell<PluginState>,
    attached: Cell<bool>,
    sent_solos: Cell<u64>,
    pub handler: RefCell<Option<ComPtr<IComponentHandler>>>,
    pub model: Arc<Model>,
}
impl ControllerData {
    pub fn main(&self) {
        // Clone before calling the host: its callback may synchronously reenter the controller.
        let handler = self.handler.borrow().clone();
        while let Some(edit) = self.model.pop() {
            match edit {
                Edit::Presentation(order) => {
                    let mut state = self.state.get();
                    if state.set_presentation(order) {
                        self.state.set(state);
                        if let Some(h) = &handler
                            && let Some(h2) = h.cast::<IComponentHandler2>()
                        {
                            unsafe {
                                h2.setDirty(1);
                            }
                        }
                    }
                }
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
        let mut sent = self.sent_solos.get();
        for (index, flag) in self
            .model
            .output_meter
            .band_solos()
            .iter()
            .enumerate()
            .take(64)
        {
            if self.state.get().get(daw_drive_plugin::Parameter::Bypass) != 0.0 {
                flag.store(false, std::sync::atomic::Ordering::Relaxed);
            }
            let selected = flag.load(std::sync::atomic::Ordering::Relaxed);
            if selected != (sent & (1_u64 << index) != 0) {
                let mut values = [0.0; daw_drive_plugin::METER_VALUES];
                values[0] = index as f64;
                values[1] = f64::from(selected);
                if self
                    .meter_connection
                    .send(crate::telemetry::Kind::Solo, values)
                    == kResultOk
                {
                    if selected {
                        sent |= 1_u64 << index;
                    } else {
                        sent &= !(1_u64 << index);
                    }
                }
            }
        }
        self.sent_solos.set(sent);
        for (requests, kind) in [
            (self.model.output_meter.band_reset_requests(), 0.0),
            (self.model.output_meter.band_level_reset_requests(), 1.0),
        ] {
            if let Some(requests) = requests {
                let mut pending = requests.swap(0, std::sync::atomic::Ordering::AcqRel);
                while pending != 0 {
                    let index = pending.trailing_zeros();
                    let bit = 1_u64 << index;
                    let mut values = [0.0; daw_drive_plugin::METER_VALUES];
                    values[0] = f64::from(index);
                    values[1] = kind;
                    if self
                        .meter_connection
                        .send(crate::telemetry::Kind::BandReset, values)
                        != kResultOk
                    {
                        requests.fetch_or(pending, std::sync::atomic::Ordering::Release);
                        break;
                    }
                    pending &= !bit;
                }
            }
        }
        let mask = self
            .model
            .meter_reset
            .swap(0, std::sync::atomic::Ordering::AcqRel);
        if mask != 0 {
            let mut values = [0.0; daw_drive_plugin::METER_VALUES];
            values[0] = f64::from(mask);
            if self
                .meter_connection
                .send(crate::telemetry::Kind::Reset, values)
                != kResultOk
            {
                self.model
                    .meter_reset
                    .fetch_or(mask, std::sync::atomic::Ordering::Release);
                return;
            }
        }
        if self
            .model
            .output_reset
            .swap(false, std::sync::atomic::Ordering::AcqRel)
            && self.meter_connection.send(
                crate::telemetry::Kind::Reset,
                [0.0; daw_drive_plugin::METER_VALUES],
            ) != kResultOk
        {
            self.model
                .output_reset
                .store(true, std::sync::atomic::Ordering::Release);
            return;
        }
        self.meter_connection.request_reading();
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

#[cfg(test)]
mod audition_tests {
    use super::*;
    #[test]
    fn solo_messages_reach_prepared_band_flags_without_parameters_and_reject_invalid_slots() {
        use std::sync::atomic::Ordering;
        let component = crate::component::Component::default();
        let meter = component.output_meter.clone();
        if meter.band_solos().is_empty() {
            return;
        }
        let peer = ComWrapper::new(component)
            .to_com_ptr::<IConnectionPoint>()
            .unwrap();
        let data = ControllerData::default();
        assert_eq!(
            unsafe { data.meter_connection.connect(peer.as_ptr()) },
            kResultOk
        );
        let before = data.state.get();
        for slot in [0, 31, 63] {
            data.model.output_meter.band_solos()[slot].store(true, Ordering::Relaxed);
        }
        data.main();
        for slot in [0, 31, 63] {
            assert!(meter.band_solos()[slot].load(Ordering::Relaxed));
        }
        assert_eq!(data.state.get(), before);
        let mut invalid = [0.0; daw_drive_plugin::METER_VALUES];
        invalid[0] = 64.0;
        invalid[1] = 1.0;
        assert_eq!(
            data.meter_connection
                .send(crate::telemetry::Kind::Solo, invalid),
            kInvalidArgument
        );
        invalid[0] = 0.5;
        assert_eq!(
            data.meter_connection
                .send(crate::telemetry::Kind::Solo, invalid),
            kInvalidArgument
        );
        invalid[0] = 0.0;
        invalid[1] = 0.5;
        assert_eq!(
            data.meter_connection
                .send(crate::telemetry::Kind::Solo, invalid),
            kInvalidArgument
        );
        if let Some(requests) = data.model.output_meter.band_reset_requests() {
            let mut values = [0.0; daw_drive_plugin::METER_VALUES];
            values[0] = 0.2;
            values[1] = 0.7;
            for index in [0, 63] {
                *values.get_mut(77 + index).unwrap() = 3.0;
                *values.get_mut(141 + index).unwrap() = 6.0;
            }
            if daw_drive_plugin::METER_VALUES == 668 {
                *values.get_mut(141).unwrap() = 48_000.0;
                *values
                    .get_mut(daw_drive_plugin::METER_VALUES.saturating_sub(14))
                    .unwrap() = 2.0;
            }
            daw_drive_plugin::receive_meter(&meter, values);
            requests.fetch_or(1 | (1_u64 << 63), Ordering::Release);
            data.main();
            let values = daw_drive_plugin::meter_values(&meter);
            for index in [0, 63] {
                assert_eq!(*values.get(77 + index).unwrap(), 3.0);
                assert_eq!(*values.get(141 + index).unwrap(), 0.0);
            }
            assert!((values[1] - 0.7).abs() < 1e-6);
        }
        if let Some(requests) = data.model.output_meter.band_level_reset_requests() {
            let level_offset = if data.model.output_meter.band_reset_requests().is_none() {
                77
            } else {
                205
            };
            let has_reduction = data.model.output_meter.band_reset_requests().is_some();
            let mut values = [0.0; daw_drive_plugin::METER_VALUES];
            values[0] = 0.2;
            values[1] = 0.7;
            for index in [0, 63] {
                *values.get_mut(13 + index).unwrap() = 0.1;
                *values.get_mut(level_offset + index).unwrap() = 0.8;
                if has_reduction {
                    *values.get_mut(141 + index).unwrap() = 6.0;
                }
            }
            if daw_drive_plugin::METER_VALUES == 668 {
                *values.get_mut(141).unwrap() = 48_000.0;
                *values
                    .get_mut(daw_drive_plugin::METER_VALUES.saturating_sub(14))
                    .unwrap() = 2.0;
            }
            daw_drive_plugin::receive_meter(&meter, values);
            requests.fetch_or(1 | (1_u64 << 63), Ordering::Release);
            data.main();
            let values = daw_drive_plugin::meter_values(&meter);
            for index in [0, 63] {
                assert_eq!(*values.get(level_offset + index).unwrap(), 0.0);
                assert!((*values.get(13 + index).unwrap() - 0.1).abs() < 1e-6);
                if has_reduction {
                    assert_eq!(*values.get(141 + index).unwrap(), 6.0);
                }
            }
            assert!((values[1] - 0.7).abs() < 1e-6);
        }
        let mut bypass = before;
        bypass
            .set(daw_drive_plugin::Parameter::Bypass, 1.0)
            .unwrap();
        data.state.set(bypass);
        data.main();
        assert!(
            meter
                .band_solos()
                .iter()
                .all(|f| !f.load(Ordering::Relaxed))
        );
    }
}
