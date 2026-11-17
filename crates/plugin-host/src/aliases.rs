//! Native alias/shortcut resolution runs before bounded directory traversal.
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::Error;
use crate::Result;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
pub(crate) fn resolve(path: &Path) -> Result<PathBuf> {
    use objc2_foundation::{NSString, NSURL, NSURLBookmarkResolutionOptions};
    let Some(text) = path.to_str() else {
        return Ok(path.to_owned());
    };
    // Normal files return their own URL. Finder aliases resolve without UI or
    // mounting network volumes. The traversal's canonical set detects cycles.
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    match NSURL::URLByResolvingAliasFileAtURL_options_error(
        &url,
        NSURLBookmarkResolutionOptions::WithoutUI | NSURLBookmarkResolutionOptions::WithoutMounting,
    ) {
        Ok(url) => url
            .path()
            .map(|p| PathBuf::from(p.to_string()))
            .ok_or_else(|| Error::Invalid("Alias has no local filesystem path".into())),
        Err(_) => Ok(path.to_owned()),
    }
}
#[cfg(target_os = "windows")]
pub(crate) fn resolve(path: &Path) -> Result<PathBuf> {
    use std::{
        ffi::c_void,
        os::windows::ffi::{OsStrExt, OsStringExt},
        ptr,
    };
    use windows_sys::{
        Win32::{System::Com::*, UI::Shell::ShellLink},
        core::GUID,
    };
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
    {
        return Ok(path.to_owned());
    }
    // These are the system IShellLinkW and IPersistFile vtables. Keep the
    // uncalled slots opaque; release every acquired COM owner on this thread.
    #[repr(C)]
    struct Unknown {
        query: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    #[repr(C)]
    struct Link {
        unknown: Unknown,
        get_path: unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> i32,
        unused: [usize; 15],
        resolve: unsafe extern "system" fn(*mut c_void, isize, u32) -> i32,
    }
    #[repr(C)]
    struct Persist {
        unknown: Unknown,
        unused: [usize; 2],
        load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> i32,
    }
    struct Owner(*mut c_void);
    impl Drop for Owner {
        fn drop(&mut self) {
            unsafe {
                let table = &**(self.0 as *const *const Unknown);
                (table.release)(self.0);
            }
        }
    }
    // The scan worker owns an STA for this operation, independent of the app's.
    let status = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    let _apartment = Apartment(status >= 0);
    if status < 0 && status != 0x80010106u32 as i32 {
        return Err(Error::Invalid(
            "Cannot initialize shortcut resolution".into(),
        ));
    }
    let mut link = ptr::null_mut();
    let iid = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
    let status = unsafe {
        CoCreateInstance(
            &ShellLink,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &iid,
            &mut link,
        )
    };
    if status < 0 || link.is_null() {
        return Err(Error::Invalid("Cannot create shortcut resolver".into()));
    }
    let link = Owner(link);
    let table = unsafe { &**(link.0 as *const *const Link) };
    let mut file = ptr::null_mut();
    let iid = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);
    if unsafe { (table.unknown.query)(link.0, &iid, &mut file) } < 0 || file.is_null() {
        return Err(Error::Invalid(
            "Shortcut has no persistent file interface".into(),
        ));
    }
    let file = Owner(file);
    let persist = unsafe { &**(file.0 as *const *const Persist) };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SLR_NO_UI | SLR_NOUPDATE | SLR_NOSEARCH | SLR_NOTRACK. Do not repair or
    // search for moved targets, write to the shortcut, or display a dialog.
    if unsafe { (persist.load)(file.0, wide.as_ptr(), 0) } < 0
        || unsafe { (table.resolve)(link.0, 0, 1 | 8 | 16 | 32) } < 0
    {
        return Err(Error::Invalid("Cannot resolve plugin shortcut".into()));
    }
    let mut output = vec![0u16; 32768];
    if unsafe {
        (table.get_path)(
            link.0,
            output.as_mut_ptr(),
            output.len() as i32,
            ptr::null_mut(),
            0,
        )
    } < 0
    {
        return Err(Error::Invalid("Cannot read plugin shortcut target".into()));
    }
    let count = output
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| Error::Invalid("Unterminated shortcut target".into()))?;
    if count == 0 {
        return Err(Error::Invalid("Shortcut target is empty".into()));
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &output[..count],
    )))
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn resolve(path: &Path) -> Result<PathBuf> {
    Ok(path.to_owned())
}
