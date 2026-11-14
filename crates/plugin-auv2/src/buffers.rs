use crate::sys::*;
use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
};
/// Owned storage for the C trailing-one array, with two planar channels.
#[repr(C)]
pub struct StereoList {
    pub count: u32,
    pub buffers: [AudioBuffer; 2],
}
impl StereoList {
    pub fn new(left: *mut f32, right: *mut f32, frames: u32) -> Self {
        Self {
            count: 2,
            buffers: [left, right].map(|p| AudioBuffer {
                mNumberChannels: 1,
                mDataByteSize: frames * 4,
                mData: p.cast::<c_void>(),
            }),
        }
    }
    pub fn as_mut_ptr(&mut self) -> *mut AudioBufferList {
        (self as *mut Self).cast()
    }
}
/// # Safety
/// Host supplies a live ABL with storage for its declared buffer count and data extents.
/// This copies C headers; it creates no references to trailing arrays or aliased samples.
pub unsafe fn stereo(list: *mut AudioBufferList, frames: u32) -> Option<[*mut f32; 2]> {
    if list.is_null() || frames > u32::MAX / 4 {
        return None;
    }
    if unsafe { (*list).mNumberBuffers } != 2 {
        return None;
    }
    let base = unsafe {
        list.cast::<u8>()
            .add(offset_of!(AudioBufferList, mBuffers))
            .cast::<AudioBuffer>()
    };
    let mut result = [std::ptr::null_mut(); 2];
    for (index, out) in result.iter_mut().enumerate() {
        let header = unsafe { base.add(index).read_unaligned() };
        if header.mNumberChannels != 1
            || header.mDataByteSize < frames * 4
            || (frames != 0 && header.mData.is_null())
        {
            return None;
        }
        *out = header.mData.cast();
    }
    let bytes = frames as usize * size_of::<f32>();
    if bytes != 0 {
        let a = result[0] as usize;
        let b = result[1] as usize;
        if a.checked_add(bytes)? > b && b.checked_add(bytes)? > a {
            return None;
        }
    }
    Some(result)
}
