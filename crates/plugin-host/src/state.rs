//! Immutable opaque state ownership. Run persistence on a worker, never on audio.
use crate::{Error, MAX_HOSTED_STATE_BYTES, NativeState, PluginFormat, Result};
use daw_unit_core::{HostedStateBlob, HostedStateReference};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Default)]
pub struct StateBank {
    blobs: HashMap<String, Arc<[u8]>>,
}
impl StateBank {
    fn insert(&mut self, bytes: Vec<u8>) -> Result<HostedStateBlob> {
        if bytes.len() > MAX_HOSTED_STATE_BYTES {
            return Err(Error::Invalid("Plugin state exceeds 16 MiB".into()));
        }
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let blob = HostedStateBlob {
            sha256: hash.clone(),
            length_bytes: bytes.len() as u64,
        };
        self.blobs.entry(hash).or_insert_with(|| bytes.into());
        Ok(blob)
    }
    pub fn capture(&mut self, state: NativeState, revision: u64) -> Result<HostedStateReference> {
        let (component, controller) = match state {
            NativeState::Clap(bytes) => (self.insert(bytes)?, None),
            NativeState::Vst3 {
                component,
                controller,
            } => (self.insert(component)?, Some(self.insert(controller)?)),
        };
        Ok(HostedStateReference {
            component,
            controller,
            revision,
        })
    }
    fn bytes(&self, blob: &HostedStateBlob) -> Result<Vec<u8>> {
        if !blob.valid() {
            return Err(Error::Invalid("Invalid plugin state reference".into()));
        }
        let bytes = self
            .blobs
            .get(&blob.sha256)
            .ok_or_else(|| Error::Invalid(format!("Missing plugin state {}", blob.sha256)))?;
        if bytes.len() as u64 != blob.length_bytes {
            return Err(Error::Invalid("Plugin state length mismatch".into()));
        }
        Ok(bytes.to_vec())
    }
    pub fn restore(
        &self,
        format: PluginFormat,
        state: &HostedStateReference,
    ) -> Result<NativeState> {
        let component = self.bytes(&state.component)?;
        match format {
            PluginFormat::Clap if state.controller.is_none() => Ok(NativeState::Clap(component)),
            PluginFormat::Vst3 => Ok(NativeState::Vst3 {
                component,
                controller: state
                    .controller
                    .as_ref()
                    .map(|s| self.bytes(s))
                    .transpose()?
                    .unwrap_or_default(),
            }),
            _ => Err(Error::Invalid("Plugin state format mismatch".into())),
        }
    }
    /// Load only manifest-owned blobs. Hashes are filenames, never caller paths.
    fn directory(folder: &Path, create: bool) -> Result<std::path::PathBuf> {
        if create {
            std::fs::create_dir_all(folder)?;
        }
        let mut path = folder.canonicalize()?;
        for name in ["assets", "plugin-state"] {
            path.push(name);
            if create {
                match std::fs::create_dir(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.into()),
                }
            }
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::Invalid(
                    "Plugin state directory must be a real project directory".into(),
                ));
            }
        }
        Ok(path)
    }
    pub fn retain<'a>(&mut self, references: impl IntoIterator<Item = &'a HostedStateReference>) {
        let hashes: std::collections::HashSet<_> = references
            .into_iter()
            .flat_map(|r| std::iter::once(&r.component).chain(&r.controller))
            .map(|b| b.sha256.as_str())
            .collect();
        self.blobs.retain(|hash, _| hashes.contains(hash.as_str()));
    }
    pub fn load(&mut self, folder: &Path, reference: &HostedStateReference) -> Result<()> {
        let directory = Self::directory(folder, false)?;
        for blob in std::iter::once(&reference.component).chain(&reference.controller) {
            if !blob.valid() {
                return Err(Error::Invalid("Invalid plugin state reference".into()));
            }
            if self.blobs.contains_key(&blob.sha256) {
                self.bytes(blob)?;
                continue;
            }
            let path = directory.join(format!("{}.bin", blob.sha256));
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.len() != blob.length_bytes {
                return Err(Error::Invalid("Invalid plugin state file".into()));
            }
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(MAX_HOSTED_STATE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != blob.length_bytes
                || format!("{:x}", Sha256::digest(&bytes)) != blob.sha256
            {
                return Err(Error::Invalid("Plugin state hash mismatch".into()));
            }
            self.insert(bytes)?;
        }
        Ok(())
    }
    /// Publish all retained owners before the project manifest. Save As therefore
    /// retains state referenced by history as well as the visible project.
    pub fn save(&self, folder: &Path) -> Result<()> {
        if self.blobs.is_empty() {
            return Ok(());
        }
        let directory = Self::directory(folder, true)?;
        for (hash, bytes) in &self.blobs {
            let path = directory.join(format!("{hash}.bin"));
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if !metadata.is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.len() as usize != bytes.len()
                {
                    return Err(Error::Invalid("Invalid existing plugin state file".into()));
                }
                let mut existing = Vec::new();
                std::fs::File::open(&path)?
                    .take(MAX_HOSTED_STATE_BYTES as u64 + 1)
                    .read_to_end(&mut existing)?;
                if existing.as_slice() != bytes.as_ref() {
                    return Err(Error::Invalid(
                        "Existing plugin state blob is corrupt".into(),
                    ));
                }
                continue;
            }
            let temporary = directory.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
            let result = (|| -> Result<()> {
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)?;
                file.write_all(bytes)?;
                file.sync_all()?;
                drop(file);
                // A hard link publishes without replacing an existing owner.
                match std::fs::hard_link(&temporary, &path) {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if std::fs::read(&path)?.as_slice() == bytes.as_ref() {
                            Ok(())
                        } else {
                            Err(Error::Invalid("Plugin state publication conflict".into()))
                        }
                    }
                    Err(e) => Err(e.into()),
                }
            })();
            let _ = std::fs::remove_file(temporary);
            result?;
        }
        Ok(())
    }
    pub fn merge(&mut self, other: &Self) {
        self.blobs
            .extend(other.blobs.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    pub fn retained_bytes(&self) -> u64 {
        self.blobs.values().map(|b| b.len() as u64).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_owners_round_trip_and_reject_corruption() {
        let root = std::env::temp_dir().join(format!("daw-state-{}", uuid::Uuid::new_v4()));
        let mut bank = StateBank::default();
        let state = NativeState::Vst3 {
            component: vec![0, 255, 4],
            controller: vec![],
        };
        let reference = bank.capture(state.clone(), 7).unwrap();
        let history = bank.clone();
        drop(bank);
        history.save(&root).unwrap();
        let mut reopened = StateBank::default();
        reopened.load(&root, &reference).unwrap();
        assert_eq!(
            reopened.restore(PluginFormat::Vst3, &reference).unwrap(),
            state
        );
        assert!(reopened.restore(PluginFormat::Clap, &reference).is_err());
        std::fs::write(
            root.join("assets/plugin-state")
                .join(format!("{}.bin", reference.component.sha256)),
            [9, 9, 9],
        )
        .unwrap();
        assert!(StateBank::default().load(&root, &reference).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn state_publication_rejects_symlinked_directories_and_files() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("daw-state-links-{}", uuid::Uuid::new_v4()));
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        symlink(&outside, project.join("assets")).unwrap();
        let mut bank = StateBank::default();
        let reference = bank.capture(NativeState::Clap(vec![1, 2, 3]), 0).unwrap();
        assert!(bank.save(&project).is_err());
        std::fs::remove_file(project.join("assets")).unwrap();
        bank.save(&project).unwrap();
        let path = project
            .join("assets/plugin-state")
            .join(format!("{}.bin", reference.component.sha256));
        let destination = outside.join("blob");
        std::fs::rename(&path, &destination).unwrap();
        symlink(&destination, &path).unwrap();
        assert!(bank.save(&project).is_err());
        assert!(StateBank::default().load(&project, &reference).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pruning_one_snapshot_keeps_other_immutable_owners() {
        let mut bank = StateBank::default();
        let old = bank.capture(NativeState::Clap(vec![1, 2]), 0).unwrap();
        let history = bank.clone();
        let new = bank.capture(NativeState::Clap(vec![3]), 1).unwrap();
        bank.retain([&new]);
        assert_eq!(bank.retained_bytes(), 1);
        assert_eq!(
            history.restore(PluginFormat::Clap, &old).unwrap(),
            NativeState::Clap(vec![1, 2])
        );
        assert!(bank.restore(PluginFormat::Clap, &old).is_err());
    }
}
