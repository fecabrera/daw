//! CoreFoundation metadata lookup. This does not load or activate plugin code.
use crate::{Error, Result};
use std::{
    ffi::{CStr, OsString, c_void},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

pub(crate) struct Bundle(pub(crate) *const c_void);
impl Bundle {
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: live filesystem bytes and default allocator, synchronous metadata calls.
        let url = unsafe {
            CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len() as isize,
                true,
            )
        };
        if url.is_null() {
            return Err(Error::Invalid("Cannot create plugin bundle URL".into()));
        }
        let bundle = unsafe { CFBundleCreate(std::ptr::null(), url) };
        unsafe {
            CFRelease(url);
        }
        if bundle.is_null() {
            Err(Error::Invalid("Cannot read plugin bundle metadata".into()))
        } else {
            Ok(Self(bundle))
        }
    }
    pub fn executable(&self) -> Result<PathBuf> {
        // SAFETY: self owns a live CFBundle reference; Copy returns an owned URL.
        let url = unsafe { CFBundleCopyExecutableURL(self.0) };
        if url.is_null() {
            return Err(Error::Invalid("Plugin bundle has no executable".into()));
        }
        let mut bytes = [0; 32768];
        let valid = unsafe {
            CFURLGetFileSystemRepresentation(url, true, bytes.as_mut_ptr(), bytes.len() as isize)
        };
        unsafe {
            CFRelease(url);
        }
        if !valid {
            return Err(Error::Invalid(
                "Plugin executable path exceeds supported length".into(),
            ));
        }
        // A successful CFURL conversion writes a terminated filesystem representation.
        let path = unsafe { CStr::from_ptr(bytes.as_ptr().cast()) }.to_bytes();
        Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
    }
}
impl Drop for Bundle {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: *const c_void,
        bytes: *const u8,
        length: isize,
        directory: bool,
    ) -> *const c_void;
    fn CFBundleCreate(allocator: *const c_void, url: *const c_void) -> *const c_void;
    fn CFBundleCopyExecutableURL(bundle: *const c_void) -> *const c_void;
    fn CFURLGetFileSystemRepresentation(
        url: *const c_void,
        resolve: bool,
        bytes: *mut u8,
        length: isize,
    ) -> bool;
    fn CFRelease(value: *const c_void);
}
