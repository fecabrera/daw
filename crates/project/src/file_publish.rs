//! Same-directory manifest/export replacement. Never move the old target aside.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(windows)]
#[path = "file_publish/windows.rs"]
mod windows;

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
        windows::replace(&source, &target)
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
    fn replacement_keeps_the_pinned_candidate_handle_valid_with_unicode_paths() {
        use std::io::{Seek, SeekFrom};
        let f = Fixture::new();
        let source = f.0.join("candidate café.tmp");
        let target = f.0.join("project 移動.json");
        fs::write(&source, b"complete candidate").unwrap();
        fs::write(&target, b"old manifest").unwrap();
        let mut candidate = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&source)
            .unwrap();
        let mut old = fs::File::open(&target).unwrap();
        replace(&source, &target).unwrap();
        let mut bytes = Vec::new();
        old.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"old manifest");
        candidate.seek(SeekFrom::Start(0)).unwrap();
        bytes.clear();
        candidate.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"complete candidate");
        assert_eq!(fs::read(target).unwrap(), bytes);
        assert!(!source.exists());
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }
    #[test]
    fn replacement_can_publish_the_first_manifest() {
        let f = Fixture::new();
        let source = f.0.join("candidate.tmp");
        let target = f.0.join("project.json");
        fs::write(&source, b"first manifest").unwrap();
        replace(&source, &target).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"first manifest");
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
