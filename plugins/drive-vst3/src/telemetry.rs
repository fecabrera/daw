//! Main-thread meter messages through the host's component/controller connection.
//! Audio only publishes atomics; no GUI pointers or SDK calls cross processing.
use daw_drive_plugin::METER_VALUES;
// EQ v1 has a scalar/rate prefix plus legacy bins; compressor v1 has 269 scalar/band values.
// Extended snapshots append their spectrum metadata in the final fourteen slots.
const LEGACY_VALUES: usize = if METER_VALUES == 796 { 269 } else { 206 };
const LEGACY_PREFIX: usize = if METER_VALUES == 796 { 269 } else { 142 };
const SPECTRUM_RATE: usize = if METER_VALUES == 796 { 269 } else { 141 };
// Exact EQ and Multiband Compressor schema-2 sizes, never a threshold.
// Other products retain their full v1 payloads.
const EXTENDED: bool = METER_VALUES == 668 || METER_VALUES == 796;
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
    RequestV2,
    ReadingV2,
    Reset,
    Solo,
    BandReset,
}
impl Kind {
    fn id(self) -> &'static CStr {
        match self {
            Self::Request => c"daw.output-meter.request.v1",
            Self::Reading => c"daw.output-meter.reading.v1",
            Self::RequestV2 => c"daw.output-meter.request.v2",
            Self::ReadingV2 => c"daw.output-meter.reading.v2",
            Self::BandReset => c"daw.band-peak.reset.v1",
            Self::Solo => c"daw.band-solo.v1",
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
    pub fn request_reading(&self) -> tresult {
        if EXTENDED && self.send(Kind::RequestV2, [0.0; METER_VALUES]) == kResultOk {
            return kResultOk;
        }
        self.send(Kind::Request, [0.0; METER_VALUES])
    }
    pub fn send(&self, kind: Kind, mut values: [f64; METER_VALUES]) -> tresult {
        let extended = matches!(kind, Kind::ReadingV2 | Kind::RequestV2);
        if extended && !EXTENDED {
            return kInvalidArgument;
        }
        let length = if EXTENDED && !extended {
            LEGACY_VALUES
        } else {
            METER_VALUES
        };
        if EXTENDED && kind == Kind::Reading {
            let prefix = LEGACY_PREFIX.min(METER_VALUES);
            values[prefix..].fill(0.0); // Never send new bin meanings under the v1 ID.
        }
        let Some(peer) = self.0.borrow().clone() else {
            return kResultFalse;
        };
        let attributes =
            ComWrapper::new(Attributes::complete(values.map(Cell::new)).with_length(length))
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
            [
                Kind::Request,
                Kind::Reading,
                Kind::RequestV2,
                Kind::ReadingV2,
                Kind::Reset,
                Kind::Solo,
                Kind::BandReset,
            ]
            .into_iter()
            .find(|k| {
                k.id() == unsafe { CStr::from_ptr(id) }
                    && (EXTENDED || !matches!(k, Kind::RequestV2 | Kind::ReadingV2))
            }),
        );
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}
struct Attributes {
    values: [Cell<f64>; METER_VALUES],
    length: usize,
}
impl Attributes {
    fn complete(values: [Cell<f64>; METER_VALUES]) -> Self {
        Self {
            values,
            length: METER_VALUES,
        }
    }
    fn with_length(mut self, length: usize) -> Self {
        self.length = length;
        self
    }
}
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
    unsafe fn getInt(&self, id: IAttributeList_::AttrID, value: *mut int64) -> tresult {
        if id.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: caller supplies a live attribute name and writable integer.
        if unsafe { CStr::from_ptr(id) } != c"value-count" {
            return kResultFalse;
        }
        unsafe {
            value.write(self.length as int64);
        }
        kResultOk
    }
    unsafe fn setFloat(&self, id: IAttributeList_::AttrID, value: f64) -> tresult {
        boundary(kInternalError, || {
            let Some(slot) = slot(id).filter(|slot| *slot < self.length) else {
                return kInvalidArgument;
            };
            if !value.is_finite()
                || value.abs() > f64::from(f32::MAX)
                || (!(4..7).contains(&slot) && value < 0.0)
            {
                return kInvalidArgument;
            }
            self.values[slot].set(value);
            kResultOk
        })
    }
    unsafe fn getFloat(&self, id: IAttributeList_::AttrID, value: *mut f64) -> tresult {
        boundary(kInternalError, || {
            let Some(slot) = slot(id).filter(|slot| *slot < self.length) else {
                return kInvalidArgument;
            };
            if value.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: caller provides storage for one SDK double.
            unsafe {
                value.write(self.values[slot].get());
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
    let kind = [
        Kind::Request,
        Kind::Reading,
        Kind::RequestV2,
        Kind::ReadingV2,
        Kind::Reset,
        Kind::Solo,
        Kind::BandReset,
    ]
    .into_iter()
    .find(|k| {
        k.id() == unsafe { CStr::from_ptr(id) }
            && (EXTENDED || !matches!(k, Kind::RequestV2 | Kind::ReadingV2))
    })?;
    let mut values = [0.0; METER_VALUES];
    if matches!(
        kind,
        Kind::Reading | Kind::ReadingV2 | Kind::Reset | Kind::Solo | Kind::BandReset
    ) {
        let attributes = unsafe { ComRef::from_raw(message.getAttributes()) }?;
        let length = if kind == Kind::ReadingV2 {
            let mut count = 0;
            if unsafe { attributes.getInt(c"value-count".as_ptr(), &mut count) } != kResultOk
                || count != METER_VALUES as i64
            {
                return None;
            }
            METER_VALUES
        } else if EXTENDED {
            LEGACY_VALUES
        } else {
            METER_VALUES
        };
        for (index, value) in values.iter_mut().enumerate().take(length) {
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
    if EXTENDED && kind == Kind::Reading {
        let prefix = LEGACY_PREFIX.min(METER_VALUES);
        values[prefix..].fill(0.0);
        let schema = METER_VALUES.saturating_sub(14);
        values[schema] = 2.0;
        if let Some(rate) = values.get_mut(SPECTRUM_RATE)
            && ![44100.0, 48000.0, 88200.0, 96000.0, 176400.0, 192000.0].contains(rate)
        {
            *rate = 48000.0;
        }
    }
    Some((kind, values))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_messages_reject_unknown_ids_nonfinite_values_and_out_of_range_attributes() {
        let attributes = ComWrapper::new(Attributes::complete(std::array::from_fn(|i| {
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
            let invalid = ComWrapper::new(Attributes::complete(std::array::from_fn(|i| {
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

/// Control-thread name table. No per-value allocation during polling.
fn attribute_key(index: usize) -> &'static CStr {
    static KEYS: std::sync::OnceLock<Vec<std::ffi::CString>> = std::sync::OnceLock::new();
    if index == 0 {
        return c"current";
    }
    if index == 1 {
        return c"peak";
    }
    &KEYS.get_or_init(|| {
        (0..METER_VALUES)
            .map(|i| std::ffi::CString::new(format!("meter.{i}")).expect("fixed attribute name"))
            .collect()
    })[index]
}

pub(crate) fn mask_stopped(values: &mut [f64; METER_VALUES]) {
    values[0] = 0.0;
    if EXTENDED {
        let first = (SPECTRUM_RATE + 1).min(METER_VALUES);
        let schema = METER_VALUES.saturating_sub(14);
        values[first..schema].fill(0.0);
        values[schema + 1] = 0.0;
        values[schema + 6..].fill(0.0);
    }
}

#[cfg(test)]
mod extended_tests {
    use super::*;
    #[test]
    fn extended_snapshots_retain_signed_band_values_and_reset_masks() {
        let values = std::array::from_fn(|i| if i == 4 { -6.0 } else { i as f64 });
        let attributes = ComWrapper::new(Attributes::complete(values.map(Cell::new)))
            .to_com_ptr::<IAttributeList>()
            .unwrap();
        let message = ComWrapper::new(Message {
            kind: Cell::new(Some(if EXTENDED {
                Kind::ReadingV2
            } else {
                Kind::Reading
            })),
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

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    #[test]
    fn schema_two_requires_exact_count_and_legacy_spectrum_is_masked() {
        if !EXTENDED {
            return;
        }
        for length in [LEGACY_VALUES, METER_VALUES - 1, METER_VALUES] {
            let attributes = ComWrapper::new(
                Attributes::complete(std::array::from_fn(|i| Cell::new(i as f64)))
                    .with_length(length),
            )
            .to_com_ptr::<IAttributeList>()
            .unwrap();
            let message = ComWrapper::new(Message {
                kind: Cell::new(Some(Kind::ReadingV2)),
                attributes,
            })
            .to_com_ptr::<IMessage>()
            .unwrap();
            // SAFETY: owned COM objects remain live throughout synchronous calls.
            unsafe {
                assert_eq!(read(message.as_ptr()).is_some(), length == METER_VALUES);
                message.setMessageID(Kind::Reading.id().as_ptr());
                let (_, values) = read(message.as_ptr()).unwrap();
                assert_eq!(*values.get(140).unwrap(), 140.0);
                assert!(
                    values
                        .get(SPECTRUM_RATE + 1..METER_VALUES.saturating_sub(14))
                        .unwrap()
                        .iter()
                        .all(|v| *v == 0.0)
                );
                assert_eq!(*values.get(METER_VALUES.saturating_sub(14)).unwrap(), 2.0);
                assert_eq!(*values.get(METER_VALUES.saturating_sub(13)).unwrap(), 0.0);
            }
        }
    }
}
