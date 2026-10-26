//! Narrow VST3 bindings and main-thread stream/identifier helpers.
pub use daw_plugin_support::{boundary, write_text};
use std::{
    io::{self, Read, Write},
    marker::PhantomData,
};
pub use vst3 as sys;
use vst3::{
    ComRef,
    Steinberg::{IBStream, IBStreamTrait, TUID, kResultFalse, kResultOk},
};

/// Convert a canonical UUID into the SDK's platform-specific TUID byte order.
pub const fn class_id(bytes: [u8; 16]) -> TUID {
    vst3::uid(
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
    )
}
pub fn write_utf16(text: &str, output: &mut [u16]) -> bool {
    if output.is_empty() {
        return false;
    }
    let mut length = 0;
    for character in text.chars() {
        let mut encoded = [0; 2];
        let encoded = character.encode_utf16(&mut encoded);
        if length + encoded.len() >= output.len() {
            output[length] = 0;
            return false;
        }
        output[length..length + encoded.len()].copy_from_slice(encoded);
        length += encoded.len();
    }
    output[length] = 0;
    true
}

/// Borrow a host-owned stream only for the synchronous UI-thread call.
pub struct Stream<'a> {
    stream: ComRef<'a, IBStream>,
    _main: PhantomData<*mut ()>,
}
impl<'a> Stream<'a> {
    /// # Safety
    /// Pointer must be a live IBStream for this entire synchronous borrow.
    pub unsafe fn new(pointer: *mut IBStream) -> Option<Self> {
        Some(Self {
            stream: unsafe { ComRef::from_raw(pointer) }?,
            _main: PhantomData,
        })
    }
}
impl Read for Stream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let size = i32::try_from(buffer.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
        let mut count = 0;
        // SAFETY: host stream is borrowed and destination has the advertised extent.
        let status = unsafe {
            self.stream
                .read(buffer.as_mut_ptr().cast(), size, &mut count)
        };
        if (status != kResultOk && status != kResultFalse) || count < 0 || count > size {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(count as usize)
    }
}
impl Write for Stream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let size = i32::try_from(buffer.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
        let mut count = 0;
        // SAFETY: host stream is borrowed and source remains live until return.
        let status = unsafe {
            self.stream
                .write(buffer.as_ptr().cast_mut().cast(), size, &mut count)
        };
        if status != kResultOk || count < 0 || count > size {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(count as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tuid_keeps_frozen_uuid_bytes_with_sdk_windows_encoding() {
        let uuid = [
            0xf3, 0x99, 0xb9, 0x2b, 0x5d, 0x26, 0x4c, 0x7d, 0x98, 0xc6, 0xde, 0x3c, 0x26, 0x54,
            0xd5, 0x34,
        ];
        let expected = if cfg!(target_os = "windows") {
            [
                0x2b, 0xb9, 0x99, 0xf3, 0x26, 0x5d, 0x7d, 0x4c, 0x98, 0xc6, 0xde, 0x3c, 0x26, 0x54,
                0xd5, 0x34,
            ]
        } else {
            uuid
        };
        assert_eq!(class_id(uuid).map(|byte| byte as u8), expected);
    }
    #[test]
    fn utf16_text_does_not_split_a_surrogate_pair() {
        let mut short = [42; 2];
        assert!(!write_utf16("😀", &mut short));
        assert_eq!(short[0], 0);
        let mut full = [0; 3];
        assert!(write_utf16("😀", &mut full));
        assert_eq!(full, [0xd83d, 0xde00, 0]);
    }
}
