//! Main-thread meter messages through the host's component/controller connection.
//! Audio only publishes atomics; no GUI pointers or SDK calls cross processing.
use daw_drive_plugin::METER_VALUES;
use daw_plugin_vst3::{
    boundary,
    sys::{
        Class, ComPtr, ComRef, ComWrapper,
        Steinberg::{Vst::*, *},
    },
};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_void},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Request,
    Reading,
    Reset,
}
impl Kind {
    fn id(self) -> &'static CStr {
        match self {
            Self::Request => c"daw.output-meter.request.v1",
            Self::Reading => c"daw.output-meter.reading.v1",
            Self::Reset => c"daw.output-meter.reset.v1",
        }
    }
}
#[derive(Default)]
pub(crate) struct Connection(RefCell<Option<ComPtr<IConnectionPoint>>>);
impl Connection {
    /// SDK connection methods run on the control thread and own a COM reference.
    pub unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        let Some(other) = (unsafe { ComRef::from_raw(other) }) else {
            return kInvalidArgument;
        };
        let mut peer = self.0.borrow_mut();
        if peer.is_some() {
            return kResultFalse;
        }
        *peer = Some(other.to_com_ptr());
        kResultOk
    }
    pub unsafe fn disconnect(&self, other: *mut IConnectionPoint) -> tresult {
        let mut peer = self.0.borrow_mut();
        if !peer.as_ref().is_some_and(|p| p.as_ptr() == other) {
            return kInvalidArgument;
        }
        let old = peer.take();
        drop(peer);
        drop(old);
        kResultOk
    }
    pub fn clear(&self) {
        let old = self.0.borrow_mut().take();
        drop(old);
    }
    pub fn send(&self, kind: Kind, values: [f64; METER_VALUES]) -> tresult {
        let Some(peer) = self.0.borrow().clone() else {
            return kResultFalse;
        };
        let attributes = ComWrapper::new(Attributes(values.map(Cell::new)))
            .to_com_ptr::<IAttributeList>()
            .expect("attribute interface");
        let message = ComWrapper::new(Message {
            kind: Cell::new(Some(kind)),
            attributes,
        })
        .to_com_ptr::<IMessage>()
        .expect("message interface");
        // SAFETY: owned interfaces and attributes remain live throughout the synchronous call.
        unsafe { peer.notify(message.as_ptr()) }
    }
}
struct Message {
    kind: Cell<Option<Kind>>,
    attributes: ComPtr<IAttributeList>,
}
impl Class for Message {
    type Interfaces = (IMessage,);
}
impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.kind.get().map_or(c"", Kind::id).as_ptr()
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        if id.is_null() {
            self.kind.set(None);
            return;
        }
        // SAFETY: SDK supplies a live NUL-terminated message ID.
        self.kind.set(
            [Kind::Request, Kind::Reading, Kind::Reset]
                .into_iter()
                .find(|k| k.id() == unsafe { CStr::from_ptr(id) }),
        );
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}
struct Attributes([Cell<f64>; METER_VALUES]);
impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}
fn slot(id: IAttributeList_::AttrID) -> Option<usize> {
    if id.is_null() {
        return None;
    }
    // SAFETY: the SDK supplies a live attribute ID for the synchronous call.
    match unsafe { CStr::from_ptr(id) }.to_bytes() {
        b"current" => Some(0),
        b"peak" => Some(1),
        key => std::str::from_utf8(key)
            .ok()?
            .strip_prefix("meter.")?
            .parse::<usize>()
            .ok()
            .filter(|i| (2..METER_VALUES).contains(i)),
    }
}
impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, _: IAttributeList_::AttrID, _: int64) -> tresult {
        kNotImplemented
    }
    unsafe fn getInt(&self, _: IAttributeList_::AttrID, _: *mut int64) -> tresult {
        kNotImplemented
    }
    unsafe fn setFloat(&self, id: IAttributeList_::AttrID, value: f64) -> tresult {
        boundary(kInternalError, || {
            let Some(slot) = slot(id) else {
                return kInvalidArgument;
            };
            if !value.is_finite()
                || value.abs() > f64::from(f32::MAX)
                || (!(4..7).contains(&slot) && value < 0.0)
            {
                return kInvalidArgument;
            }
            self.0[slot].set(value);
            kResultOk
        })
    }
    unsafe fn getFloat(&self, id: IAttributeList_::AttrID, value: *mut f64) -> tresult {
        boundary(kInternalError, || {
            let Some(slot) = slot(id) else {
                return kInvalidArgument;
            };
            if value.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: caller provides storage for one SDK double.
            unsafe {
                value.write(self.0[slot].get());
            }
            kResultOk
        })
    }
    unsafe fn setString(&self, _: IAttributeList_::AttrID, _: *const TChar) -> tresult {
        kNotImplemented
    }
    unsafe fn getString(&self, _: IAttributeList_::AttrID, _: *mut TChar, _: uint32) -> tresult {
        kNotImplemented
    }
    unsafe fn setBinary(&self, _: IAttributeList_::AttrID, _: *const c_void, _: uint32) -> tresult {
        kNotImplemented
    }
    unsafe fn getBinary(
        &self,
        _: IAttributeList_::AttrID,
        _: *mut *const c_void,
        _: *mut uint32,
    ) -> tresult {
        kNotImplemented
    }
}
/// # Safety
/// The host owns a live message and borrowed attributes for this control-thread call.
pub unsafe fn read(message: *mut IMessage) -> Option<(Kind, [f32; METER_VALUES])> {
    let message = unsafe { ComRef::from_raw(message) }?;
    let id = unsafe { message.getMessageID() };
    if id.is_null() {
        return None;
    }
    let kind = [Kind::Request, Kind::Reading, Kind::Reset]
        .into_iter()
        .find(|k| k.id() == unsafe { CStr::from_ptr(id) })?;
    let mut values = [0.0; METER_VALUES];
    if kind == Kind::Reading || kind == Kind::Reset {
        let attributes = unsafe { ComRef::from_raw(message.getAttributes()) }?;
        for (index, value) in values.iter_mut().enumerate() {
            let key = attribute_key(index);
            let mut plain = 0.0;
            if unsafe { attributes.getFloat(key.as_ptr(), &mut plain) } != kResultOk
                || !plain.is_finite()
                || plain.abs() > f64::from(f32::MAX)
                || (!(4..7).contains(&index) && plain < 0.0)
            {
                return None;
            }
            *value = plain as f32;
        }
    }
    Some((kind, values))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_messages_reject_unknown_ids_nonfinite_values_and_out_of_range_attributes() {
        let attributes = ComWrapper::new(Attributes(std::array::from_fn(|i| {
            Cell::new(if i == 0 {
                0.25
            } else if i == 1 {
                2.0
            } else {
                0.0
            })
        })))
        .to_com_ptr::<IAttributeList>()
        .unwrap();
        let message = ComWrapper::new(Message {
            kind: Cell::new(Some(Kind::Reading)),
            attributes: attributes.clone(),
        })
        .to_com_ptr::<IMessage>()
        .unwrap();
        // SAFETY: all owned COM objects, C strings and output storage live through each call.
        unsafe {
            let Some((kind, values)) = read(message.as_ptr()) else {
                panic!("valid message");
            };
            assert!(kind == Kind::Reading);
            assert_eq!(&values[..2], &[0.25, 2.0]);
            message.setMessageID(c"unrelated-message".as_ptr());
            assert!(read(message.as_ptr()).is_none());
            message.setMessageID(Kind::Reading.id().as_ptr());
            assert_eq!(
                attributes.setFloat(c"current".as_ptr(), -1.0),
                kInvalidArgument
            );
            assert_eq!(
                attributes.setFloat(c"peak".as_ptr(), f64::INFINITY),
                kInvalidArgument
            );
            assert_eq!(
                attributes.setFloat(c"unknown".as_ptr(), 1.0),
                kInvalidArgument
            );
            assert!(read(message.as_ptr()).is_some());
            let invalid = ComWrapper::new(Attributes(std::array::from_fn(|i| {
                Cell::new(if i == 0 { f64::NAN } else { 1.0 })
            })))
            .to_com_ptr::<IAttributeList>()
            .unwrap();
            let invalid = ComWrapper::new(Message {
                kind: Cell::new(Some(Kind::Reading)),
                attributes: invalid,
            })
            .to_com_ptr::<IMessage>()
            .unwrap();
            assert!(read(invalid.as_ptr()).is_none());
        }
    }
}

fn attribute_key(index: usize) -> std::ffi::CString {
    std::ffi::CString::new(match index {
        0 => "current".into(),
        1 => "peak".into(),
        _ => format!("meter.{index}"),
    })
    .expect("fixed attribute name")
}

#[cfg(test)]
mod extended_tests {
    use super::*;
    #[test]
    fn extended_snapshots_retain_signed_band_values_and_reset_masks() {
        let values = std::array::from_fn(|i| if i == 4 { -6.0 } else { i as f64 });
        let attributes = ComWrapper::new(Attributes(values.map(Cell::new)))
            .to_com_ptr::<IAttributeList>()
            .unwrap();
        let message = ComWrapper::new(Message {
            kind: Cell::new(Some(Kind::Reading)),
            attributes: attributes.clone(),
        })
        .to_com_ptr::<IMessage>()
        .unwrap();
        // SAFETY: owned COM interfaces and output storage remain live for the calls.
        unsafe {
            let (_, received) = read(message.as_ptr()).unwrap();
            assert_eq!(received, values.map(|v| v as f32));
            message.setMessageID(Kind::Reset.id().as_ptr());
            attributes.setFloat(c"current".as_ptr(), 6.0);
            let (kind, received) = read(message.as_ptr()).unwrap();
            assert!(kind == Kind::Reset);
            assert_eq!(received[0], 6.0);
        }
    }
}
