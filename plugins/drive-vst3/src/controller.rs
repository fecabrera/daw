use daw_drive_plugin::{Parameter, PluginState};
use daw_plugin_vst3::{
    Stream, boundary,
    sys::{
        Class, ComRef,
        Steinberg::{Vst::*, *},
    },
    write_utf16,
};
use std::{cell::Cell, ffi::CStr, ptr, rc::Rc};
pub(crate) struct Controller {
    initialized: Cell<bool>,
    data: Rc<crate::editor::ControllerData>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            initialized: Cell::new(false),
            data: Rc::default(),
        }
    }
}
impl Class for Controller {
    type Interfaces = (IEditController, IConnectionPoint);
}
impl IPluginBaseTrait for Controller {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        if self.initialized.replace(true) {
            kResultFalse
        } else {
            kResultOk
        }
    }
    unsafe fn terminate(&self) -> tresult {
        boundary(kInternalError, || {
            self.initialized.set(false);
            self.data.meter_connection.clear();
            drop(self.data.handler.borrow_mut().take());
            kResultOk
        })
    }
}
impl IEditControllerTrait for Controller {
    unsafe fn setComponentState(&self, stream: *mut IBStream) -> tresult {
        boundary(kInternalError, || {
            let Some(mut stream) = (unsafe { Stream::new(stream) }) else {
                return kInvalidArgument;
            };
            let Ok(state) = PluginState::read_from(&mut stream) else {
                return kResultFalse;
            };
            self.data.state.set(state);
            kResultOk
        })
    }
    // The component owns base parameters. No independent editor preferences exist
    // yet, so controller-specific state is explicitly unsupported, not a second
    // conflicting copy of component state.
    unsafe fn setState(&self, _: *mut IBStream) -> tresult {
        kNotImplemented
    }
    unsafe fn getState(&self, _: *mut IBStream) -> tresult {
        kNotImplemented
    }
    unsafe fn getParameterCount(&self) -> i32 {
        Parameter::COUNT as i32
    }
    unsafe fn getParameterInfo(&self, index: i32, output: *mut ParameterInfo) -> tresult {
        boundary(kInternalError, || {
            if output.is_null() {
                return kInvalidArgument;
            }
            let Some(parameter) = Parameter::ALL.get(index as usize) else {
                return kInvalidArgument;
            };
            let meta = parameter.info();
            let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
            info.id = *parameter as u32;
            info.stepCount = meta.steps as i32;
            info.defaultNormalizedValue = parameter.normalize(meta.default).unwrap_or(0.0);
            info.unitId = kRootUnitId;
            info.flags = ParameterInfo_::ParameterFlags_::kCanAutomate;
            if *parameter == Parameter::Bypass {
                info.flags |= ParameterInfo_::ParameterFlags_::kIsBypass;
            }
            if meta.scale == daw_drive_plugin::Scale::Choice {
                info.flags |= ParameterInfo_::ParameterFlags_::kIsList;
            }
            write_utf16(meta.name, &mut info.title);
            write_utf16(meta.name, &mut info.shortTitle);
            write_utf16(
                match meta.scale {
                    daw_drive_plugin::Scale::Gain => "dB",
                    daw_drive_plugin::Scale::Logarithmic => "Hz",
                    daw_drive_plugin::Scale::Linear => "%",
                    _ => "",
                },
                &mut info.units,
            );
            unsafe {
                output.write(info);
            }
            kResultOk
        })
    }
    unsafe fn getParamStringByValue(&self, id: u32, value: f64, output: *mut String128) -> tresult {
        boundary(kInternalError, || {
            if output.is_null() {
                return kInvalidArgument;
            }
            let Ok(parameter) = Parameter::from_id(id) else {
                return kInvalidArgument;
            };
            let Ok(plain) = parameter.denormalize(value) else {
                return kInvalidArgument;
            };
            let Ok(text) = parameter.format(plain) else {
                return kInvalidArgument;
            };
            let text = text
                .strip_suffix(" dB")
                .or_else(|| text.strip_suffix(" Hz"))
                .or_else(|| text.strip_suffix('%'))
                .unwrap_or(&text);
            if write_utf16(text, unsafe { &mut *output }) {
                kResultOk
            } else {
                kResultFalse
            }
        })
    }
    unsafe fn getParamValueByString(&self, id: u32, text: *mut TChar, output: *mut f64) -> tresult {
        boundary(kInternalError, || {
            if text.is_null() || output.is_null() {
                return kInvalidArgument;
            }
            let Ok(parameter) = Parameter::from_id(id) else {
                return kInvalidArgument;
            };
            // SDK parameter strings have String128 extent; reject missing terminators.
            let mut units = [0; 128];
            let mut length = 0;
            while length < 128 {
                let value = unsafe { *text.add(length) };
                if value == 0 {
                    break;
                }
                units[length] = value;
                length += 1;
            }
            if length == 128 {
                return kInvalidArgument;
            }
            let Ok(text) = String::from_utf16(&units[..length]) else {
                return kInvalidArgument;
            };
            let Ok(plain) = parameter.parse(&text) else {
                return kResultFalse;
            };
            let Ok(value) = parameter.normalize(plain) else {
                return kResultFalse;
            };
            unsafe {
                output.write(value);
            }
            kResultOk
        })
    }
    unsafe fn normalizedParamToPlain(&self, id: u32, value: f64) -> f64 {
        boundary(0.0, || {
            Parameter::from_id(id)
                .ok()
                .and_then(|p| p.denormalize(value).ok())
                .unwrap_or(0.0)
        })
    }
    unsafe fn plainParamToNormalized(&self, id: u32, value: f64) -> f64 {
        boundary(0.0, || {
            Parameter::from_id(id)
                .ok()
                .and_then(|p| p.normalize(value).ok())
                .unwrap_or(0.0)
        })
    }
    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        boundary(0.0, || {
            Parameter::from_id(id)
                .ok()
                .and_then(|p| p.normalize(self.data.state.get().get(p)).ok())
                .unwrap_or(0.0)
        })
    }
    unsafe fn setParamNormalized(&self, id: u32, value: f64) -> tresult {
        boundary(kInternalError, || {
            let Ok(parameter) = Parameter::from_id(id) else {
                return kInvalidArgument;
            };
            let mut state = self.data.state.get();
            if state.set_normalized(parameter, value).is_err() {
                return kInvalidArgument;
            }
            self.data.state.set(state);
            kResultOk
        })
    }
    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        boundary(kInternalError, || {
            let next = unsafe { ComRef::from_raw(handler) }.map(|h| h.to_com_ptr());
            let previous = self.data.handler.replace(next);
            drop(previous);
            kResultOk
        })
    }
    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        boundary(ptr::null_mut(), || {
            if name.is_null() || unsafe { CStr::from_ptr(name) } != c"editor" {
                return ptr::null_mut();
            }
            crate::editor::View::create(self.data.clone())
        })
    }
}

impl IConnectionPointTrait for Controller {
    unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        boundary(kInternalError, || unsafe {
            self.data.meter_connection.connect(other)
        })
    }
    unsafe fn disconnect(&self, other: *mut IConnectionPoint) -> tresult {
        boundary(kInternalError, || unsafe {
            self.data.meter_connection.disconnect(other)
        })
    }
    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        boundary(kInternalError, || {
            let Some((crate::telemetry::Kind::Reading, values)) =
                (unsafe { crate::telemetry::read(message) })
            else {
                return kInvalidArgument;
            };
            self.data.model.receive_output(values);
            kResultOk
        })
    }
}

#[cfg(test)]
mod output_meter_tests {
    use super::*;
    use daw_plugin_vst3::sys::ComWrapper;
    use daw_unit_core::MeterStorage;
    #[test]
    fn connection_transfers_only_meter_values_and_balances_peak_reset_without_host_edits() {
        let component = ComWrapper::new(crate::component::Component::default());
        let controller = ComWrapper::new(Controller::default());
        let a = component.to_com_ptr::<IConnectionPoint>().unwrap();
        let b = controller.to_com_ptr::<IConnectionPoint>().unwrap();
        // SAFETY: both wrappers and their owned interfaces remain live through disconnect.
        unsafe {
            assert_eq!(a.connect(b.as_ptr()), kResultOk);
            assert_eq!(b.connect(a.as_ptr()), kResultOk);
        }
        component
            .processing
            .store(true, std::sync::atomic::Ordering::Release);
        component.output_meter.store([2.0, 0.0, 0.0]);
        component.output_meter.initialize([0.25, 0.0, 0.0]);
        let state = controller.data.state.get();
        controller.data.main();
        assert_eq!(
            controller.data.model.output_meter.readings(),
            [0.25, 0.0, 0.0]
        );
        assert_eq!(
            controller.data.model.output_meter.boost_peaks()[0].peak(),
            2.0
        );
        assert_eq!(controller.data.state.get(), state);
        controller
            .data
            .model
            .output_reset
            .store(true, std::sync::atomic::Ordering::Release);
        controller.data.main();
        assert_eq!(component.output_meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(component.output_meter.readings()[0], 0.25);
        assert_eq!(
            controller.data.model.output_meter.boost_peaks()[0].peak(),
            0.0
        );
        component
            .processing
            .store(false, std::sync::atomic::Ordering::Release);
        controller.data.main();
        assert_eq!(controller.data.model.output_meter.readings()[0], 0.0);
        // SAFETY: null is explicitly rejected; connections are then removed before object release.
        unsafe {
            assert_eq!(b.notify(std::ptr::null_mut()), kInvalidArgument);
            assert_eq!(a.disconnect(b.as_ptr()), kResultOk);
            assert_eq!(b.disconnect(a.as_ptr()), kResultOk);
        }
    }
}
