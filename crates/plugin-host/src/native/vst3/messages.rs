//! Fixed host message pool. Factory, getters and setters never wait or allocate.
//! Broker references keep final COM destruction on the instance's main thread.
use super::*;
use daw_plugin_vst3::sys::Steinberg::Vst::IAttributeList_::AttrID;
use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const KEYS: usize = 32;
const TEXT: usize = 128;
const BYTES: usize = 65536;
#[derive(Clone, Copy)]
struct Attribute {
    key: [u8; TEXT],
    kind: u8,
    scalar: u64,
    offset: usize,
    length: usize,
    reserved: usize,
}
struct Data {
    id: [u8; TEXT],
    attributes: Vec<Attribute>,
    bytes: Box<[u8]>,
    used: usize,
}
struct Storage {
    busy: AtomicBool,
    data: UnsafeCell<Data>,
}
// SAFETY: every method uses a nonblocking exclusive guard. Borrowed ABI text
// and binary views remain valid until the caller modifies that message, as
// required by the SDK; callers must synchronize such modifications themselves.
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}
struct Guard<'a>(&'a Storage);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::Release);
    }
}
impl Storage {
    fn lock(&self) -> Option<Guard<'_>> {
        self.busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Guard(self))
    }
    fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            data: UnsafeCell::new(Data {
                id: [0; TEXT],
                attributes: Vec::with_capacity(KEYS),
                bytes: vec![0; BYTES].into_boxed_slice(),
                used: 0,
            }),
        }
    }
    fn reset(&self) -> bool {
        let Some(_guard) = self.lock() else {
            return false;
        };
        let data = unsafe { &mut *self.data.get() };
        data.id.fill(0);
        data.attributes.clear();
        data.used = 0;
        true
    }
}
unsafe fn key(pointer: *const std::ffi::c_char) -> Option<[u8; TEXT]> {
    if pointer.is_null() {
        return None;
    }
    let mut key = [0u8; TEXT];
    for (i, out) in key.iter_mut().enumerate() {
        let b = unsafe { *pointer.add(i) } as u8;
        *out = b;
        if b == 0 {
            return Some(key);
        }
    }
    None
}
fn slot(data: &mut Data, key: [u8; TEXT]) -> Option<usize> {
    if let Some(index) = data.attributes.iter().position(|a| a.key == key) {
        return Some(index);
    }
    if data.attributes.len() == KEYS {
        return None;
    }
    let index = data.attributes.len();
    data.attributes.push(Attribute {
        key,
        kind: 0,
        scalar: 0,
        offset: 0,
        length: 0,
        reserved: 0,
    });
    Some(index)
}
struct Attributes(Arc<Storage>);
impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}
impl Attributes {
    unsafe fn scalar(&self, id: AttrID, kind: u8, value: Option<u64>) -> Option<u64> {
        let key = unsafe { key(id) }?;
        let _guard = self.0.lock()?;
        let data = unsafe { &mut *self.0.data.get() };
        if let Some(value) = value {
            let index = slot(data, key)?;
            data.attributes[index].kind = kind;
            data.attributes[index].scalar = value;
            Some(value)
        } else {
            data.attributes
                .iter()
                .find(|a| a.key == key && a.kind == kind)
                .map(|a| a.scalar)
        }
    }
    unsafe fn set_bytes(&self, id: AttrID, kind: u8, bytes: *const u8, length: usize) -> tresult {
        if bytes.is_null() && length > 0 || length > BYTES {
            return kInvalidArgument;
        }
        let Some(key) = (unsafe { key(id) }) else {
            return kInvalidArgument;
        };
        let Some(_guard) = self.0.lock() else {
            return kResultFalse;
        };
        let data = unsafe { &mut *self.0.data.get() };
        let Some(index) = slot(data, key) else {
            return kResultFalse;
        };
        if length > data.attributes[index].reserved {
            if length > BYTES - data.used {
                return kResultFalse;
            }
            data.attributes[index].offset = data.used;
            data.attributes[index].reserved = length;
            data.used += length;
        }
        let attr = &mut data.attributes[index];
        attr.kind = kind;
        attr.length = length;
        if length > 0 {
            unsafe {
                ptr::copy_nonoverlapping(bytes, data.bytes.as_mut_ptr().add(attr.offset), length);
            }
        }
        kResultOk
    }
}
impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: AttrID, value: i64) -> tresult {
        if unsafe { self.scalar(id, 1, Some(value as u64)) }.is_some() {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn getInt(&self, id: AttrID, out: *mut i64) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        match unsafe { self.scalar(id, 1, None) } {
            Some(v) => {
                unsafe { out.write(v as i64) };
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn setFloat(&self, id: AttrID, value: f64) -> tresult {
        if unsafe { self.scalar(id, 2, Some(value.to_bits())) }.is_some() {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn getFloat(&self, id: AttrID, out: *mut f64) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        match unsafe { self.scalar(id, 2, None) } {
            Some(v) => {
                unsafe { out.write(f64::from_bits(v)) };
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn setString(&self, id: AttrID, string: *const u16) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        for n in 0..BYTES / 2 {
            if unsafe { *string.add(n) } == 0 {
                return unsafe { self.set_bytes(id, 3, string.cast(), (n + 1) * 2) };
            }
        }
        kInvalidArgument
    }
    unsafe fn getString(&self, id: AttrID, out: *mut u16, size: u32) -> tresult {
        if out.is_null() || size < 2 {
            return kInvalidArgument;
        }
        let Some(key) = (unsafe { key(id) }) else {
            return kInvalidArgument;
        };
        let Some(_guard) = self.0.lock() else {
            return kResultFalse;
        };
        let data = unsafe { &*self.0.data.get() };
        let Some(attr) = data.attributes.iter().find(|a| a.key == key && a.kind == 3) else {
            return kResultFalse;
        };
        if attr.length > size as usize {
            return kResultFalse;
        }
        unsafe {
            ptr::copy_nonoverlapping(
                data.bytes.as_ptr().add(attr.offset),
                out.cast(),
                attr.length,
            );
        }
        kResultOk
    }
    unsafe fn setBinary(&self, id: AttrID, data: *const c_void, size: u32) -> tresult {
        unsafe { self.set_bytes(id, 4, data.cast(), size as usize) }
    }
    unsafe fn getBinary(&self, id: AttrID, out: *mut *const c_void, size: *mut u32) -> tresult {
        if out.is_null() || size.is_null() {
            return kInvalidArgument;
        }
        let Some(key) = (unsafe { key(id) }) else {
            return kInvalidArgument;
        };
        let Some(_guard) = self.0.lock() else {
            return kResultFalse;
        };
        let data = unsafe { &*self.0.data.get() };
        let Some(attr) = data.attributes.iter().find(|a| a.key == key && a.kind == 4) else {
            return kResultFalse;
        };
        unsafe {
            out.write(data.bytes.as_ptr().add(attr.offset).cast());
            size.write(attr.length as u32);
        }
        kResultOk
    }
}
struct Message {
    storage: Arc<Storage>,
    attributes: ComPtr<IAttributeList>,
}
impl Class for Message {
    type Interfaces = (IMessage,);
}
impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        unsafe { (*self.storage.data.get()).id.as_ptr().cast() }
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        if let Some(key) = (unsafe { key(id) })
            && let Some(_guard) = self.storage.lock()
        {
            unsafe {
                (*self.storage.data.get()).id = key;
            }
        }
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}
struct Entry {
    storage: Arc<Storage>,
    message: ComPtr<IMessage>,
    attributes: ComPtr<IAttributeList>,
    busy: AtomicBool,
}
impl Entry {
    fn new() -> Self {
        let storage = Arc::new(Storage::new());
        let attributes = ComWrapper::new(Attributes(storage.clone()))
            .to_com_ptr::<IAttributeList>()
            .expect("declared attributes");
        let message = ComWrapper::new(Message {
            storage: storage.clone(),
            attributes: attributes.clone(),
        })
        .to_com_ptr::<IMessage>()
        .expect("declared message");
        Self {
            storage,
            message,
            attributes,
            busy: AtomicBool::new(false),
        }
    }
}
pub(super) struct Pool {
    entries: [Entry; 8],
}
impl Pool {
    pub fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| Entry::new()),
        }
    }
    pub unsafe fn create(
        &self,
        class: *mut TUID,
        interface: *mut TUID,
        out: *mut *mut c_void,
    ) -> tresult {
        if out.is_null() || class.is_null() || interface.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            out.write(ptr::null_mut());
        }
        let class = unsafe { *class };
        let interface = unsafe { *interface };
        let message = class == IMessage_iid && interface == IMessage_iid;
        let attributes = class == IAttributeList_iid && interface == IAttributeList_iid;
        if !message && !attributes {
            return kNoInterface;
        }
        for entry in &self.entries {
            if entry
                .busy
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                continue;
            }
            unsafe fn references(pointer: *mut FUnknown) -> u32 {
                unsafe {
                    ((*(*pointer).vtbl).addRef)(pointer);
                    ((*(*pointer).vtbl).release)(pointer)
                }
            }
            let (message_refs, attribute_refs) = unsafe {
                (
                    references(entry.message.as_ptr().cast()),
                    references(entry.attributes.as_ptr().cast()),
                )
            };
            let available = message_refs == 1 && attribute_refs == 2;
            // Always pair both temporary references, including an in-use message.
            // The explicit counts below avoid short-circuiting release calls.
            if available && entry.storage.reset() {
                let pointer = if message {
                    entry.message.clone().into_raw().cast()
                } else {
                    entry.attributes.clone().into_raw().cast()
                };
                unsafe {
                    out.write(pointer);
                }
                entry.busy.store(false, Ordering::Release);
                return kResultOk;
            }
            entry.busy.store(false, Ordering::Release);
        }
        kResultFalse
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(pool: &Pool) -> Option<ComPtr<IMessage>> {
        let mut class = IMessage_iid;
        let mut interface = IMessage_iid;
        let mut raw = ptr::null_mut();
        if unsafe { pool.create(&mut class, &mut interface, &mut raw) } != kResultOk {
            return None;
        }
        unsafe { ComPtr::from_raw(raw.cast()) }
    }
    #[test]
    fn pool_preserves_attributes_and_reuses_only_unowned_messages() {
        let pool = Pool::new();
        let first = message(&pool).unwrap();
        let pointer = first.as_ptr();
        unsafe {
            first.setMessageID(c"meter".as_ptr());
            assert_eq!(std::ffi::CStr::from_ptr(first.getMessageID()), c"meter");
            let attributes = ComRef::from_raw(first.getAttributes()).unwrap();
            assert_eq!(attributes.setInt(c"channel".as_ptr(), -7), kResultOk);
            let mut number = 0;
            assert_eq!(
                attributes.getInt(c"channel".as_ptr(), &mut number),
                kResultOk
            );
            assert_eq!(number, -7);
            assert_eq!(attributes.setFloat(c"level".as_ptr(), 0.75), kResultOk);
            let mut value = 0.0;
            assert_eq!(
                attributes.getFloat(c"level".as_ptr(), &mut value),
                kResultOk
            );
            assert_eq!(value, 0.75);
            let text = [65u16, 0x03a9, 0];
            assert_eq!(
                attributes.setString(c"title".as_ptr(), text.as_ptr()),
                kResultOk
            );
            let mut output = [0u16; 3];
            assert_eq!(
                attributes.getString(c"title".as_ptr(), output.as_mut_ptr(), 6),
                kResultOk
            );
            assert_eq!(output, text);
            assert_eq!(
                attributes.getString(c"title".as_ptr(), output.as_mut_ptr(), 2),
                kResultFalse
            );
            let bytes = [0u8, 255, 4];
            assert_eq!(
                attributes.setBinary(c"data".as_ptr(), bytes.as_ptr().cast(), 3),
                kResultOk
            );
            let mut data = ptr::null();
            let mut size = 0;
            assert_eq!(
                attributes.getBinary(c"data".as_ptr(), &mut data, &mut size),
                kResultOk
            );
            assert_eq!(
                std::slice::from_raw_parts(data.cast::<u8>(), size as usize),
                bytes
            );
        }
        let mut held = vec![first];
        for _ in 1..8 {
            held.push(message(&pool).unwrap());
        }
        assert!(message(&pool).is_none());
        held.remove(0);
        let reused = message(&pool).unwrap();
        assert_eq!(reused.as_ptr(), pointer);
        unsafe {
            assert_eq!(std::ffi::CStr::from_ptr(reused.getMessageID()), c"");
        }
    }
}
