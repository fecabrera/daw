use super::*;
use std::cell::{Cell, RefCell};
pub(super) struct StateStream {
    pub bytes: RefCell<Vec<u8>>,
    offset: Cell<usize>,
    pub failed: Cell<bool>,
}
impl StateStream {
    pub fn new(bytes: Vec<u8>) -> ComWrapper<Self> {
        ComWrapper::new(Self {
            bytes: RefCell::new(bytes),
            offset: Cell::new(0),
            failed: Cell::new(false),
        })
    }
}
impl Class for StateStream {
    type Interfaces = (IBStream,);
}
impl IBStreamTrait for StateStream {
    unsafe fn read(&self, out: *mut c_void, size: i32, count: *mut i32) -> tresult {
        if size < 0 || (size != 0 && out.is_null()) {
            return kInvalidArgument;
        }
        let bytes = self.bytes.borrow();
        let n = (size as usize).min(bytes.len().saturating_sub(self.offset.get()));
        if n != 0 {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr().add(self.offset.get()), out.cast(), n);
            }
        }
        self.offset.set(self.offset.get() + n);
        if !count.is_null() {
            unsafe {
                count.write(n as i32);
            }
        }
        if n == 0 { kResultFalse } else { kResultOk }
    }
    unsafe fn write(&self, input: *mut c_void, size: i32, count: *mut i32) -> tresult {
        if size < 0 || (size != 0 && input.is_null()) {
            return kInvalidArgument;
        }
        let Some(end) = self
            .offset
            .get()
            .checked_add(size as usize)
            .filter(|n| *n <= MAX_HOSTED_STATE_BYTES)
        else {
            self.failed.set(true);
            return kInternalError;
        };
        let mut bytes = self.bytes.borrow_mut();
        let additional = end.saturating_sub(bytes.len());
        if bytes.try_reserve(additional).is_err() {
            self.failed.set(true);
            return kInternalError;
        }
        if end > bytes.len() {
            bytes.resize(end, 0);
        }
        if size != 0 {
            unsafe {
                ptr::copy_nonoverlapping(
                    input.cast::<u8>(),
                    bytes.as_mut_ptr().add(self.offset.get()),
                    size as usize,
                );
            }
        }
        self.offset.set(end);
        if !count.is_null() {
            unsafe {
                count.write(size);
            }
        }
        kResultOk
    }
    unsafe fn seek(&self, offset: i64, mode: i32, result: *mut i64) -> tresult {
        let base = match mode {
            0 => 0,
            1 => self.offset.get() as i64,
            2 => self.bytes.borrow().len() as i64,
            _ => return kInvalidArgument,
        };
        let Some(next) = base
            .checked_add(offset)
            .filter(|n| *n >= 0 && *n <= MAX_HOSTED_STATE_BYTES as i64)
        else {
            return kInvalidArgument;
        };
        self.offset.set(next as usize);
        if !result.is_null() {
            unsafe {
                result.write(next);
            }
        }
        kResultOk
    }
    unsafe fn tell(&self, result: *mut i64) -> tresult {
        if result.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            result.write(self.offset.get() as i64);
        }
        kResultOk
    }
}
