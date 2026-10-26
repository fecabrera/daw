use crate::{CONTROLLER_ID, PROCESSOR_ID, component::Component, controller::Controller};
use daw_drive_plugin::IDENTITY;
use daw_plugin_vst3::{
    boundary,
    sys::{Class, ComWrapper, Steinberg::*, com_scrape_types::Unknown},
    write_text,
};
use std::{ffi::c_void, ptr};
struct Factory;
impl Class for Factory {
    type Interfaces = (IPluginFactory2,);
}
impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, output: *mut PFactoryInfo) -> tresult {
        boundary(kInternalError, || {
            if output.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: the SDK supplies one writable factory-info structure.
            let mut info: PFactoryInfo = unsafe { std::mem::zeroed() };
            write_text(IDENTITY.vendor, &mut info.vendor);
            write_text("https://github.com/fecabrera/daw", &mut info.url);
            unsafe {
                output.write(info);
            }
            kResultOk
        })
    }
    unsafe fn countClasses(&self) -> i32 {
        2
    }
    unsafe fn getClassInfo(&self, index: i32, output: *mut PClassInfo) -> tresult {
        boundary(kInternalError, || {
            if output.is_null() || !(0..2).contains(&index) {
                return kInvalidArgument;
            }
            let mut info: PClassInfo = unsafe { std::mem::zeroed() };
            info.cid = if index == 0 {
                PROCESSOR_ID
            } else {
                CONTROLLER_ID
            };
            info.cardinality = i32::MAX;
            write_text(
                if index == 0 {
                    "Audio Module Class"
                } else {
                    "Component Controller Class"
                },
                &mut info.category,
            );
            let name = if index == 0 {
                IDENTITY.product.to_owned()
            } else {
                format!("{} Controller", IDENTITY.product)
            };
            write_text(&name, &mut info.name);
            unsafe {
                output.write(info);
            }
            kResultOk
        })
    }
    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        output: *mut *mut c_void,
    ) -> tresult {
        boundary(kInternalError, || {
            if cid.is_null() || iid.is_null() || output.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: VST3 supplies 16-byte IDs and writable pointer storage.
            unsafe {
                output.write(ptr::null_mut());
            }
            let class = unsafe { ptr::read_unaligned(cid.cast::<TUID>()) };
            let requested = unsafe { ptr::read_unaligned(iid.cast::<[u8; 16]>()) };
            let result = if class == PROCESSOR_ID {
                let object = ComWrapper::new(Component::default());
                let pointer = object
                    .to_com_ptr::<Vst::IComponent>()
                    .expect("declared component interface");
                unsafe { Vst::IComponent::query_interface(pointer.as_ptr(), &requested) }
            } else if class == CONTROLLER_ID {
                let object = ComWrapper::new(Controller::default());
                let pointer = object
                    .to_com_ptr::<Vst::IEditController>()
                    .expect("declared controller interface");
                unsafe { Vst::IEditController::query_interface(pointer.as_ptr(), &requested) }
            } else {
                return kInvalidArgument;
            };
            if let Some(pointer) = result {
                unsafe {
                    output.write(pointer);
                }
                kResultOk
            } else {
                kNoInterface
            }
        })
    }
}
impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: i32, output: *mut PClassInfo2) -> tresult {
        boundary(kInternalError, || {
            if output.is_null() {
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
            write_text(IDENTITY.vendor, &mut info.vendor);
            write_text(IDENTITY.version, &mut info.version);
            write_text("VST 3.8.1", &mut info.sdkVersion);
            if index == 0 {
                write_text("Fx|Distortion|Stereo", &mut info.subCategories);
            }
            unsafe {
                output.write(info);
            }
            kResultOk
        })
    }
}
/// Each host call receives its own owned COM factory reference.
#[unsafe(no_mangle)]
pub extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    boundary(ptr::null_mut(), || {
        ComWrapper::new(Factory)
            .to_com_ptr::<IPluginFactory>()
            .expect("declared factory interface")
            .into_raw()
    })
}
