//! Check protected file paths and identities before staging and before publication.
use crate::{Result, Session, error, file_publish, source_collection::owned_fs::Identity};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

// A missing recording may still be named by a dangling symlink. Protect the
// future backing path while allowing the renderer's normal silence fallback.
fn protected_path(path: &Path) -> Result<Option<PathBuf>> {
    let mut entry = path.to_owned();
    for _ in 0..40 {
        match crate::source_storage::canonical_or_missing(&entry) {
            Ok(path) => return Ok(Some(path)),
            Err(e) if fs::metadata(&entry).is_err_and(|e| e.kind() == io::ErrorKind::NotFound) => {
                match fs::read_link(&entry) {
                    Ok(link) => entry = entry.with_file_name(link),
                    // A missing ancestor cannot alias a file in the already existing output folder.
                    Err(_)
                        if fs::symlink_metadata(&entry)
                            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound) =>
                    {
                        return Ok(None);
                    }
                    Err(_) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(error("Too many symlinks in protected export path"))
}

fn file_identity(path: &Path) -> Result<Option<Identity>> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Identity::read(&fs::File::open(path).map_err(error)?)
            .map(Some)
            .map_err(error),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(e)),
    }
}

impl Session {
    pub(crate) fn check_export_destination(&self, target: &Path, overwrite: bool) -> Result<()> {
        let target_id = match fs::symlink_metadata(target) {
            Ok(metadata) if metadata.is_file() && !file_publish::is_reparse(&metadata) => {
                if !overwrite {
                    return Err(error(
                        "Output already exists; confirm overwrite or use --overwrite",
                    ));
                }
                file_identity(target)?
            }
            Ok(_) => return Err(error("Export output must be an ordinary file")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(error(e)),
        };
        let protect = |path: &Path, kind: &str| -> Result<()> {
            let same_path = protected_path(path)?.as_deref() == Some(target);
            if same_path || target_id.is_some() && file_identity(path)? == target_id {
                return Err(error(format!("Export cannot overwrite {kind}")));
            }
            Ok(())
        };
        if let Some(folder) = &self.folder {
            let root = crate::source_storage::canonical_or_missing(folder)?;
            if target.parent() == Some(root.as_path())
                && target
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case(crate::write_guard::LOCK_NAME))
            {
                return Err(error(
                    "Export cannot use the reserved project writer lock path",
                ));
            }
            protect(&root.join("project.json"), "project.json")?;
            protect(
                &root.join(crate::write_guard::LOCK_NAME),
                "the project writer lock",
            )?;
            for (_, unit) in self.project.hosted_units() {
                if let Some(state) = &unit.state {
                    for blob in std::iter::once(&state.component).chain(&state.controller) {
                        protect(
                            &root
                                .join("assets/plugin-state")
                                .join(format!("{}.bin", blob.sha256)),
                            "a saved plugin state file",
                        )?;
                    }
                }
            }
        }
        for asset in self
            .project
            .assets
            .iter()
            .filter(|asset| asset.is_file_audio())
        {
            protect(&self.source_path(asset)?, "a source audio file")?;
        }
        Ok(())
    }
}
