//! Same-directory manifest/export replacement. Never move the old target aside.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn is_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn ordinary_file(path: &Path, optional: bool) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_reparse(&metadata) => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Publication requires an ordinary file",
        )),
        Err(e) if optional && e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn canonical_entry(path: &Path) -> io::Result<PathBuf> {
    let parent = fs::canonicalize(path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Missing publication folder")
    })?)?;
    Ok(parent.join(path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Missing publication filename")
    })?))
}

pub(crate) fn replace(source: &Path, target: &Path) -> io::Result<()> {
    let source = canonical_entry(source)?;
    let target = canonical_entry(target)?;
    if source == target || source.parent() != target.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Replacement requires distinct files in the same folder",
        ));
    }
    ordinary_file(&source, false)?;
    ordinary_file(&target, true)?;
    #[cfg(not(windows))]
    {
        fs::rename(source, target)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        fn wide(path: &Path) -> io::Result<Vec<u16>> {
            let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
            if value.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "NUL in publication path",
                ));
            }
            value.push(0);
            Ok(value)
        }
        let source = wide(&source)?;
        let target = wide(&target)?;
        // Both paths have one canonical parent. Do not permit a cross-volume
        // copy/delete fallback or a backup/rename gap. The UTF-16 arrays live
        // through this synchronous call and both have one terminating NUL.
        let success = unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if success == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p =
                std::env::temp_dir().join(format!("daw-file-publish-{}", daw_core::Id::new_v4()));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn replacement_preserves_open_old_data_and_has_no_backup_artifacts() {
        let f = Fixture::new();
        let source = f.0.join("temp");
        let target = f.0.join("project.json");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        let mut old = fs::File::open(&target).unwrap();
        replace(&source, &target).unwrap();
        let mut bytes = Vec::new();
        old.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"old");
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }
    #[test]
    fn invalid_destination_retains_both_original_files() {
        let f = Fixture::new();
        let source = f.0.join("temp");
        let target = f.0.join("project.json");
        fs::write(&source, b"new").unwrap();
        fs::create_dir(&target).unwrap();
        assert!(replace(&source, &target).is_err());
        assert_eq!(fs::read(source).unwrap(), b"new");
        assert!(target.is_dir());
    }
    #[cfg(windows)]
    #[test]
    fn sharing_violation_does_not_move_the_old_manifest_aside() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let source = f.0.join("temp");
        let target = f.0.join("project.json");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        let _blocker = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&target)
            .unwrap();
        assert!(replace(&source, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(fs::read(&source).unwrap(), b"new");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_destination_is_not_followed_or_replaced() {
        let f = Fixture::new();
        let source = f.0.join("temp");
        let target = f.0.join("project.json");
        let other = f.0.join("other");
        fs::write(&source, b"new").unwrap();
        fs::write(&other, b"old").unwrap();
        std::os::unix::fs::symlink(&other, &target).unwrap();
        assert!(replace(&source, &target).is_err());
        assert_eq!(fs::read(other).unwrap(), b"old");
        assert!(
            fs::symlink_metadata(target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
