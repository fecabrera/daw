//! Cooperative CLI writer exclusion. The desktop and external writers may bypass it.
use crate::source_collection::owned_fs::{Identity, open_entry};
use crate::{ProjectDocument, Result, Session, error};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(crate) const LOCK_NAME: &str = ".daw-cli-write.lock";

pub struct ManifestRevision {
    root: Identity,
    digest: [u8; 32],
}

pub struct ProjectWriteGuard {
    requested: PathBuf,
    root: PathBuf,
    root_identity: Identity,
    _root_file: File,
    _lock: File,
    lock_identity: Identity,
}
impl ProjectWriteGuard {
    pub fn acquire(folder: &Path) -> Result<Self> {
        let requested = crate::absolute(folder)?;
        let root = fs::canonicalize(&requested).map_err(error)?;
        let root_file = open_entry(&root, true).map_err(error)?;
        let root_identity = Identity::read(&root_file).map_err(error)?;
        // Never create an empty project as a side effect of a write command.
        open_entry(&root.join("project.json"), false).map_err(error)?;

        let path = root.join(LOCK_NAME);
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let lock = match options.clone().create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let entry = open_entry(&path, false).map_err(error)?;
                let file = options.open(&path).map_err(error)?;
                if Identity::read(&entry).map_err(error)? != Identity::read(&file).map_err(error)? {
                    return Err(error("stale-project: writer lock changed"));
                }
                file
            }
            Err(e) => return Err(error(e)),
        };
        let lock_identity = Identity::read(&lock).map_err(error)?;
        let guard = Self {
            requested,
            root,
            root_identity,
            _root_file: root_file,
            _lock: lock,
            lock_identity,
        };
        guard.check()?;
        guard._lock.try_lock().map_err(|e| match e {
            fs::TryLockError::WouldBlock => error("busy: project already has a CLI writer"),
            fs::TryLockError::Error(e) => error(format!("Cannot lock project: {e}")),
        })?;
        guard.check()?;
        // Keep the reserved entry after unlocking so cooperating processes
        // cannot acquire locks on two different files with the same path.
        Ok(guard)
    }

    pub fn folder(&self) -> &Path {
        &self.root
    }

    pub fn check(&self) -> Result<()> {
        let identity = |path: &Path, directory| {
            open_entry(path, directory).and_then(|file| Identity::read(&file))
        };
        if fs::canonicalize(&self.requested).ok().as_ref() != Some(&self.root)
            || identity(&self.root, true).ok() != Some(self.root_identity)
            || identity(&self.root.join(LOCK_NAME), false).ok() != Some(self.lock_identity)
        {
            return Err(error(
                "stale-project: project folder or writer lock changed",
            ));
        }
        Ok(())
    }

    fn manifest_bytes(&self) -> Result<Vec<u8>> {
        self.check()?;
        let mut bytes = Vec::new();
        open_entry(&self.root.join("project.json"), false)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(error)?;
        Ok(bytes)
    }

    pub fn open_document(&self) -> Result<(ProjectDocument, ManifestRevision)> {
        let bytes = self.manifest_bytes()?;
        let (project, previous) = crate::inspection::parse_manifest(&bytes)?;
        let document = Session::document_from_manifest(self.root.clone(), project, previous)?;
        let revision = ManifestRevision {
            root: self.root_identity,
            digest: Sha256::digest(bytes).into(),
        };
        Ok((document, revision))
    }

    pub(crate) fn check_manifest(&self, expected: &ManifestRevision) -> Result<()> {
        let digest: [u8; 32] = Sha256::digest(self.manifest_bytes()?).into();
        if digest != expected.digest || self.root_identity != expected.root {
            return Err(error("stale-project: project.json changed since load"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_manifest_rejects_a_candidate_loaded_under_the_writer_lock() {
        let folder =
            std::env::temp_dir().join(format!("daw-write-guard-{}", daw_core::Id::new_v4()));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(folder.clone());
        Session::default().save(&folder).unwrap();
        let guard = ProjectWriteGuard::acquire(&folder).unwrap();
        let (document, revision) = guard.open_document().unwrap();
        let mut session = document.into_editor_session();
        session.project.add_track().unwrap();
        let manifest = folder.join("project.json");
        let mut changed_bytes = fs::read(&manifest).unwrap();
        changed_bytes.push(b'\n');
        fs::write(&manifest, &changed_bytes).unwrap();
        let result = session
            .save_snapshot(None)
            .unwrap()
            .save_guarded(&guard, &revision);
        assert!(matches!(result, Err(error) if error.to_string().contains("stale-project")));
        assert_eq!(fs::read(&manifest).unwrap(), changed_bytes);
        drop(guard);
        assert!(folder.join(LOCK_NAME).is_file());
        assert!(ProjectWriteGuard::acquire(&folder).is_ok());
    }
}
