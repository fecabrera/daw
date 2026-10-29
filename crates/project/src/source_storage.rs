//! Filesystem containment for included recordings. External paths retain legacy resolution.
use crate::{Result, error};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn included_path(folder: &Path, path: &str) -> Result<PathBuf> {
    daw_core::validate_included_source_path(path).map_err(error)?;
    let folder = fs::canonicalize(folder).map_err(error)?;
    crate::source_collection::check_included_components(&folder, path)?;
    let media = contained(&folder, &folder.join("assets/sources"))?;
    contained(&media, &folder.join(path))
}

/// Resolve the nearest existing ancestor, then append missing components. Missing
/// recordings still use normal missing-source warnings; existing symlinks must resolve.
fn contained(root: &Path, path: &Path) -> Result<PathBuf> {
    let resolved = canonical_or_missing(path)?;
    if !resolved.starts_with(root) {
        return Err(error(format!(
            "Included source escapes project media directory: {}",
            path.display()
        )));
    }
    Ok(resolved)
}

/// Canonicalize existing ancestors while retaining a missing leaf or suffix.
/// Callers must still enforce their own approved ownership root and path syntax.
pub(crate) fn canonical_or_missing(path: &Path) -> Result<PathBuf> {
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    existing
                        .file_name()
                        .ok_or_else(|| error("Invalid source path"))?,
                );
                existing = existing
                    .parent()
                    .ok_or_else(|| error("Invalid source path"))?;
            }
            Err(e) => return Err(error(format!("{}: {e}", existing.display()))),
        }
    }
    let mut resolved =
        fs::canonicalize(existing).map_err(|e| error(format!("{}: {e}", existing.display())))?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("daw-source-storage-{}", daw_core::Id::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn contained_and_missing_recordings_resolve_without_creating_files() {
        let fixture = Fixture::new();
        let root = fs::canonicalize(&fixture.0).unwrap();
        let path = "assets/sources/source.wav";
        assert_eq!(included_path(&fixture.0, path).unwrap(), root.join(path));
        assert!(!root.join("assets").exists());
        fs::create_dir_all(root.join("assets/sources")).unwrap();
        fs::write(root.join(path), b"recording").unwrap();
        assert_eq!(included_path(&fixture.0, path).unwrap(), root.join(path));
    }

    // Unix CI exercises symlinks without requiring Windows developer-mode privileges.
    #[cfg(unix)]
    #[test]
    fn directory_file_and_dangling_symlinks_cannot_escape() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = Fixture::new();
        fs::create_dir_all(fixture.0.join("assets")).unwrap();
        symlink(&outside.0, fixture.0.join("assets/sources")).unwrap();
        assert!(included_path(&fixture.0, "assets/sources/missing.wav").is_err());
        fs::remove_file(fixture.0.join("assets/sources")).unwrap();
        fs::create_dir_all(fixture.0.join("assets/sources")).unwrap();
        fs::write(outside.0.join("source.wav"), b"external").unwrap();
        symlink(
            outside.0.join("source.wav"),
            fixture.0.join("assets/sources/source.wav"),
        )
        .unwrap();
        assert!(included_path(&fixture.0, "assets/sources/source.wav").is_err());
        symlink(
            outside.0.join("missing.wav"),
            fixture.0.join("assets/sources/dangling.wav"),
        )
        .unwrap();
        assert!(included_path(&fixture.0, "assets/sources/dangling.wav").is_err());
    }
}
