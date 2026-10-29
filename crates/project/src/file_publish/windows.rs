//! Windows handle-based rename supports existing readers without a backup gap.
use super::*;
use std::{
    mem::{offset_of, size_of},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_CALL_NOT_IMPLEMENTED, ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER,
        ERROR_NOT_SUPPORTED,
    },
    Storage::FileSystem::{
        DELETE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileRenameInfoEx,
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, SYNCHRONIZE,
        SetFileInformationByHandle,
    },
};

fn open_for_rename(path: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .access_mode(DELETE | SYNCHRONIZE | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Publication requires an ordinary file",
        ));
    }
    Ok(file)
}
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in publication path",
        ));
    }
    value.push(0);
    Ok(value)
}

pub(super) fn replace(source: &Path, target: &Path) -> io::Result<()> {
    let source_file = open_for_rename(source)?;
    // Require delete sharing from existing readers. Holding this guard also keeps
    // a new nonsharing open from racing our operation on this target object.
    let target_guard = match open_for_rename(target) {
        Ok(file) => Some(file),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let target_wide = wide(target)?;
    let name_bytes = u32::try_from(target_wide.len() - 1)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Publication path is too long")
        })?;
    let size = offset_of!(FILE_RENAME_INFO, FileName)
        .checked_add(name_bytes as usize)
        .and_then(|n| n.checked_add(size_of::<u16>()))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Publication path is too long")
        })?;
    // Allocate whole SDK structures for header alignment and enough writable tail
    // storage for the variable UTF-16 name, including its terminator.
    // SAFETY: FILE_RENAME_INFO contains only integer/union fields; zero is valid.
    let empty: FILE_RENAME_INFO = unsafe { std::mem::zeroed() };
    let mut information = vec![empty; (size as usize).div_ceil(size_of::<FILE_RENAME_INFO>())];
    // SDK 0.52 omits these flags. Values are defined by MS-FSCC FileRenameInformationEx.
    const REPLACE_IF_EXISTS: u32 = 0x1;
    const POSIX_SEMANTICS: u32 = 0x2;
    information[0].Anonymous.Flags = REPLACE_IF_EXISTS | POSIX_SEMANTICS;
    information[0].RootDirectory = 0;
    information[0].FileNameLength = name_bytes;
    // SAFETY: The aligned allocation covers the header and entire UTF-16 tail.
    // Source and destination do not overlap; the API borrows both buffers/handle
    // synchronously. FileNameLength excludes the trailing NUL.
    let success = unsafe {
        std::ptr::copy_nonoverlapping(
            target_wide.as_ptr(),
            std::ptr::addr_of_mut!((*information.as_mut_ptr()).FileName).cast::<u16>(),
            target_wide.len(),
        );
        SetFileInformationByHandle(
            source_file.as_raw_handle() as isize,
            FileRenameInfoEx,
            information.as_ptr().cast(),
            size,
        )
    };
    if success != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    // Older filesystems can lack the extended rename class. Keep the conservative
    // same-folder native fallback, but never retry permission/sharing failures by
    // deleting or moving the old target away.
    if !matches!(
        error.raw_os_error().map(|n| n as u32),
        Some(
            ERROR_INVALID_FUNCTION
                | ERROR_INVALID_PARAMETER
                | ERROR_NOT_SUPPORTED
                | ERROR_CALL_NOT_IMPLEMENTED
        )
    ) {
        return Err(error);
    }
    drop(target_guard);
    let source_wide = wide(source)?;
    // SAFETY: These terminated UTF-16 buffers live through the synchronous call.
    let success = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
