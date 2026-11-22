//! Worker-only, versioned recovery generations. Manual manifests are never written here.
use crate::{Result, SaveSnapshot, Session, error};
use daw_core::Id;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

const LIMIT: u64 = 512 * 1024 * 1024;
const WORKSPACE_LIMIT: u64 = 20;
pub fn default_root() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "linux")]
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    root.map(|p| p.join("DAW/recovery"))
}

#[derive(Clone)]
pub struct RecoveryStore {
    pub folder: PathBuf,
    _lease: Arc<File>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryEntry {
    pub version: u32,
    pub id: Id,
    pub project: Id,
    pub revision: u64,
    #[serde(default)]
    pub order: u64,
    pub name: String,
    pub original_folder: Option<PathBuf>,
    pub generation: PathBuf,
    pub digest: String,
    pub applied_take: Option<Id>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub before_generation: Option<PathBuf>,
}
pub(super) fn ordinary(path: &Path, directory: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(error)?;
    if crate::file_publish::is_reparse(&meta)
        || if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
        }
    {
        return Err(error(
            "Recovery requires ordinary owned files and directories",
        ));
    }
    Ok(())
}
pub(crate) fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path).and_then(|f| f.sync_all()).map_err(error)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(error)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(error)?;
    sync_dir(
        path.parent()
            .ok_or_else(|| error("Recovery path has no parent"))?,
    )
}
pub(crate) fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    ordinary(path, false)?;
    let file = File::open(path).map_err(error)?;
    if file.metadata().map_err(error)?.len() > LIMIT {
        return Err(error("Recovery metadata exceeds 512 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > LIMIT {
        return Err(error("Recovery metadata exceeds 512 MiB"));
    }
    Ok(bytes)
}
pub fn digest(project: &daw_core::Project) -> Result<String> {
    let mut project = project.clone();
    project.transport.playhead_frame = 0;
    project.display = Default::default();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&project).map_err(error)?)
    ))
}
pub fn mark_recovered_pending(store: &RecoveryStore) -> Result<()> {
    write_new(&store.folder.join("recovered-pending"), b"pending\n")
}
impl RecoveryStore {
    pub fn create(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(error)?;
        ordinary(root, true)?;
        let root = fs::canonicalize(root).map_err(error)?;
        let folder = root.join(Id::new_v4().to_string());
        fs::create_dir(&folder).map_err(error)?;
        write_new(&folder.join("owner"), b"DAW recovery v1\n")?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(folder.join("lease"))
            .map_err(error)?;
        lease.try_lock().map_err(error)?;
        sync_dir(&folder)?;
        Ok(Self {
            folder,
            _lease: Arc::new(lease),
        })
    }
    /// A scan skips live instances and never follows links or touches unknown directories.
    pub fn scan(root: &Path) -> Result<Vec<RecoveryEntry>> {
        Self::scan_entries(root, true)
    }
    /// Inspect already verified saved generations while leaving pending takes alone.
    pub fn inspect(root: &Path) -> Result<Vec<RecoveryEntry>> {
        Self::scan_entries(root, false)
    }
    fn scan_entries(root: &Path, recover_pending: bool) -> Result<Vec<RecoveryEntry>> {
        if !root.exists() {
            return Ok(Vec::new());
        }
        ordinary(root, true)?;
        let root = fs::canonicalize(root).map_err(error)?;
        let mut result = Vec::new();
        for item in fs::read_dir(&root).map_err(error)? {
            let path = item.map_err(error)?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| Id::parse_str(n).ok())
                .is_none()
                || ordinary(&path, true).is_err()
            {
                continue;
            }
            if ordinary(&path.join("owner"), false).is_err()
                || ordinary(&path.join("lease"), false).is_err()
                || fs::read(path.join("owner")).ok().as_deref() != Some(b"DAW recovery v1\n")
            {
                continue;
            }
            let Ok(lease) = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.join("lease"))
            else {
                continue;
            };
            if lease.try_lock().is_err() {
                continue;
            }
            let store = Self {
                folder: path.clone(),
                _lease: Arc::new(lease),
            };
            if recover_pending {
                Self::cleanup_closed(&path)?;
            }
            if path.join("clean-exit").exists() && !path.join("active-take").exists() {
                continue;
            }
            let pending_error = if recover_pending {
                crate::midi_recording::TakeJournal::recover_pending(&store)
                    .err()
                    .map(|e| e.to_string())
            } else {
                None
            };
            let mut entries = Self::entries(&path)?;
            entries.sort_by_key(|e| (e.project, e.order, e.revision, e.id));
            // Present one latest valid generation per project; corrupted newest entries
            // are absent and the preceding valid generation remains discoverable.
            for mut entry in entries {
                entry.warning = pending_error.clone();
                result.retain(|e: &RecoveryEntry| {
                    e.project != entry.project || e.generation.parent() != entry.generation.parent()
                });
                result.push(entry);
            }
        }
        Ok(result)
    }
    pub(crate) fn valid_entries(folder: &Path) -> Result<Vec<RecoveryEntry>> {
        Self::entries(folder)
    }
    fn entries(folder: &Path) -> Result<Vec<RecoveryEntry>> {
        let mut result = Vec::new();
        for item in fs::read_dir(folder).map_err(error)? {
            let generation = item.map_err(error)?.path();
            if ordinary(&generation, true).is_err() || generation.join("discarded").exists() {
                continue;
            }
            let Ok(bytes) = read_bounded(&generation.join("recovery.json")) else {
                continue;
            };
            let Ok(entry) = serde_json::from_slice::<RecoveryEntry>(&bytes) else {
                continue;
            };
            if entry.version != 1
                || entry.generation != generation
                || folder.join(format!("discard-{}", entry.project)).exists()
            {
                continue;
            }
            let Ok(manifest) = read_bounded(&generation.join("project.json")) else {
                continue;
            };
            if format!("{:x}", Sha256::digest(&manifest)) != entry.digest {
                continue;
            }
            if entry.id.to_string() != generation.file_name().unwrap().to_string_lossy() {
                continue;
            }
            result.push(entry);
        }
        Ok(result)
    }
    // Called with the owning instance's exclusive lease. Only UUID generations
    // carrying our matching metadata and marked disposable working copies qualify.
    fn cleanup_closed(folder: &Path) -> Result<()> {
        let active = folder.join("active-take");
        if let Ok(bytes) = read_bounded(&active)
            && let Ok(header) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && let (Some(id), Some(project)) = (
                header
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Id::parse_str(s).ok()),
                header
                    .get("base")
                    .and_then(|v| v.get("project"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| Id::parse_str(s).ok()),
            )
            && folder.join(format!("discard-{project}")).exists()
        {
            let journal = folder.join(format!("take-{id}.journal"));
            if ordinary(&journal, false).is_ok() {
                fs::remove_file(journal).map_err(error)?;
            }
            fs::remove_file(&active).map_err(error)?;
        }
        let all = folder.join("clean-exit").exists() && !folder.join("active-take").exists();
        for item in fs::read_dir(folder).map_err(error)? {
            let path = item.map_err(error)?.path();
            let name = path.file_name().unwrap().to_string_lossy();
            if all
                && ordinary(&path, false).is_ok()
                && name
                    .strip_prefix("take-")
                    .and_then(|s| s.strip_suffix(".journal"))
                    .and_then(|s| Id::parse_str(s).ok())
                    .is_some()
            {
                fs::remove_file(&path).map_err(error)?;
                continue;
            }
            if ordinary(&path, true).is_err() {
                continue;
            }

            let project = if Id::parse_str(&name).is_ok() {
                let Ok(bytes) = read_bounded(&path.join("recovery.json")) else {
                    continue;
                };
                let Ok(entry) = serde_json::from_slice::<RecoveryEntry>(&bytes) else {
                    continue;
                };
                if entry.version != 1 || entry.generation != path || entry.id.to_string() != name {
                    continue;
                }
                entry.project
            } else if name
                .strip_prefix("restored-")
                .and_then(|s| Id::parse_str(s).ok())
                .is_some()
            {
                let Ok(bytes) = read_bounded(&path.join("working-owner")) else {
                    continue;
                };
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    continue;
                };
                let Ok(id) = Id::parse_str(text) else {
                    continue;
                };
                id
            } else {
                continue;
            };
            if all
                || folder.join(format!("discard-{project}")).exists()
                || path.join("discarded").exists()
            {
                fs::remove_dir_all(&path).map_err(error)?;
            }
        }
        Ok(())
    }
    pub fn clean_exit(&self) -> Result<()> {
        if !self.folder.join("active-take").exists() && !self.folder.join("clean-exit").exists() {
            write_new(
                &self.folder.join("clean-exit"),
                b"closed
",
            )?;
        }
        Self::cleanup_closed(&self.folder)?;
        Ok(())
    }
    pub fn discard_project(&self, project: Id) -> Result<()> {
        if self.folder.join("active-take").exists() {
            return Ok(());
        }
        for entry in Self::entries(&self.folder)?
            .into_iter()
            .filter(|e| e.project == project)
        {
            write_new(
                &entry.generation.join("discarded"),
                b"closed
",
            )?;
        }
        Self::cleanup_closed(&self.folder)?;
        Ok(())
    }
    /// Bound recoverable metadata before reserving another take. Protected old
    /// work is never evicted to make a new recording appear successful.
    pub fn admit_take(&self) -> Result<()> {
        self.admit_metadata(0)
    }
    pub fn reserve_take(&self, project: &daw_core::Project) -> Result<()> {
        struct Counter(u64);
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self.0.saturating_add(bytes.len() as u64);
                if self.0 > LIMIT {
                    return Err(std::io::Error::other(
                        "Project recovery metadata exceeds 512 MiB",
                    ));
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut size = Counter(0);
        serde_json::to_writer(&mut size, project).map_err(error)?;
        self.admit_metadata(448 * 1024 * 1024 + size.0.saturating_mul(2))
    }
    fn admit_metadata(&self, reserve: u64) -> Result<()> {
        let root = self
            .folder
            .parent()
            .ok_or_else(|| error("Missing recovery root"))?;
        let mut workspaces = 0_u64;
        let mut bytes = 0_u64;
        let mut current_counted = false;
        for owner in fs::read_dir(root).map_err(error)? {
            let path = owner.map_err(error)?.path();
            if ordinary(&path, true).is_err()
                || fs::read(path.join("owner")).ok().as_deref() != Some(b"DAW recovery v1\n")
            {
                continue;
            }
            let active_take = path.join("active-take").exists();
            if path.join("clean-exit").exists() && !active_take {
                continue;
            }
            let entries = Self::entries(&path)?;
            let mut has_recovery = active_take || !entries.is_empty();
            for entry in entries {
                bytes = bytes.saturating_add(
                    fs::metadata(entry.generation.join("project.json"))
                        .map_err(error)?
                        .len(),
                );
            }
            for item in fs::read_dir(&path).map_err(error)? {
                let p = item.map_err(error)?.path();
                if p.extension().is_some_and(|e| e == "journal") && ordinary(&p, false).is_ok() {
                    has_recovery = true;
                    bytes = bytes.saturating_add(fs::metadata(p).map_err(error)?.len());
                }
            }
            if has_recovery {
                workspaces += 1;
                current_counted |= path == self.folder;
            }
        }
        // The next write needs a slot even when its workspace starts empty.
        workspaces += u64::from(!current_counted);
        if workspaces > WORKSPACE_LIMIT || bytes.saturating_add(reserve) > LIMIT {
            return Err(error(
                "Recovery storage limit reached. Recover or discard older work before recording.",
            ));
        }
        Ok(())
    }
    pub fn checkpoint(
        &self,
        snapshot: SaveSnapshot,
        revision: u64,
        original_folder: Option<PathBuf>,
        applied_take: Option<Id>,
    ) -> Result<RecoveryEntry> {
        self.admit_take()?;
        self.write_checkpoint(snapshot, revision, original_folder, applied_take, None)
    }
    pub fn checkpoint_take(
        &self,
        snapshot: SaveSnapshot,
        revision: u64,
        original_folder: Option<PathBuf>,
        take: Id,
        before: PathBuf,
    ) -> Result<RecoveryEntry> {
        self.write_checkpoint(
            snapshot,
            revision,
            original_folder,
            Some(take),
            Some(before),
        )
    }
    fn write_checkpoint(
        &self,
        snapshot: SaveSnapshot,
        revision: u64,
        original_folder: Option<PathBuf>,
        applied_take: Option<Id>,
        before_generation: Option<PathBuf>,
    ) -> Result<RecoveryEntry> {
        let last_order = Self::valid_entries(&self.folder)?
            .iter()
            .map(|e| e.order)
            .max()
            .unwrap_or(0);
        let order = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(error)?
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64)
            .max(last_order.saturating_add(1));
        let id = Id::new_v4();
        let generation = self.folder.join(id.to_string());
        fs::create_dir(&generation).map_err(error)?;
        let result = (|| {
            // The existing save transaction copies owned media, protects history roots,
            // normalizes external references and keeps final owners on this worker.
            let saved = snapshot.save(&generation)?;
            let manifest = read_bounded(&generation.join("project.json"))?;
            let entry = RecoveryEntry {
                version: 1,
                id,
                project: saved.session.project.project_id,
                revision,
                order,
                name: saved.session.project.name.clone(),
                original_folder,
                generation: generation.clone(),
                digest: format!("{:x}", Sha256::digest(&manifest)),
                applied_take,
                warning: None,
                before_generation,
            };
            write_new(
                &generation.join("recovery.json"),
                &serde_json::to_vec(&entry).map_err(error)?,
            )?;
            sync_dir(&self.folder)?;
            self.prune(entry.project)?;
            Ok(entry)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&generation);
        }
        result
    }
    fn prune(&self, project: Id) -> Result<()> {
        // An unresolved take pins its base and every generation conservatively.
        if self.folder.join("active-take").exists() {
            return Ok(());
        }
        let mut entries = Self::entries(&self.folder)?;
        entries.retain(|e| e.project == project);
        entries.sort_by_key(|e| (e.order, e.revision, e.id));
        let remove = entries.len().saturating_sub(3);
        for entry in entries.into_iter().take(remove) {
            ordinary(&entry.generation, true)?;
            fs::remove_dir_all(entry.generation).map_err(error)?;
        }
        Ok(())
    }
    /// Claim a verified generation for the duration of a copy or recovery operation.
    fn claim(entry: &RecoveryEntry) -> Result<Self> {
        let parent = entry
            .generation
            .parent()
            .ok_or_else(|| error("Missing recovery owner"))?;
        ordinary(parent, true)?;
        ordinary(&parent.join("lease"), false)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(parent.join("lease"))
            .map_err(error)?;
        lease
            .try_lock()
            .map_err(|_| error("Recovery is in use by another instance"))?;
        let valid = Self::entries(parent)?
            .into_iter()
            .any(|e| e.id == entry.id && e.digest == entry.digest);
        if !valid {
            return Err(error("Recovery generation is unavailable or corrupt"));
        }
        Ok(Self {
            folder: parent.to_path_buf(),
            _lease: Arc::new(lease),
        })
    }

    pub fn copy_entry(entry: &RecoveryEntry, parent: &Path, name: &str) -> Result<Id> {
        let _claim = Self::claim(entry)?;
        let mut source = Session::open(&entry.generation)?;
        source.project.project_id = Id::new_v4();
        source.save_new(parent, name)?;
        Ok(source.project.project_id)
    }

    pub fn recover(entry: &RecoveryEntry) -> Result<Session> {
        let owner = Arc::new(Self::claim(entry)?);
        let parent = &owner.folder;
        let source = Session::open_document(&entry.generation)?.into_editor_session();
        let pending =
            parent.join("active-take").exists() || parent.join("recovered-pending").exists();
        let mut history = if pending {
            if let Some(before) = &entry.before_generation {
                if before.parent() != Some(parent)
                    || !Self::entries(parent)?
                        .iter()
                        .any(|e| &e.generation == before)
                {
                    return Err(error("Pending take base is unavailable"));
                }
                let before = Session::open_document(before)?.into_editor_session();
                let mut history = crate::SessionHistory::new(&before, false)?;
                history.record(&source)?;
                Some(history)
            } else {
                None
            }
        } else {
            None
        };
        // Save a distinct working copy and relocate every retained before/after owner.
        let destination = parent.join(format!("restored-{}", Id::new_v4()));
        let saved = source.save_snapshot(history.as_ref())?.save(&destination)?;
        let mut session = saved.session.clone();
        write_new(
            &destination.join("working-owner"),
            entry.project.to_string().as_bytes(),
        )?;
        saved.apply(&mut session, history.as_mut())?;
        session.recovery_owner = Some(owner.clone());
        if let Some(mut history) = history
            && let Some(mut before) = history.undo(&session)
        {
            before.recovery_owner = Some(owner);
            session.recovered_take_before = Some(Box::new(before));
        }
        Ok(session)
    }
    /// Called only while the restored Session retains this instance's lease.
    pub fn discard_claimed(entry: &RecoveryEntry, session: &Session) -> Result<()> {
        let parent = entry
            .generation
            .parent()
            .ok_or_else(|| error("Missing recovery owner"))?;
        if !session
            .recovery_owner
            .as_ref()
            .is_some_and(|owner| owner.folder == parent)
        {
            return Err(error("Recovery discard requires its live session lease"));
        }
        Self::mark_discarded(entry)
    }
    fn mark_discarded(entry: &RecoveryEntry) -> Result<()> {
        let parent = entry
            .generation
            .parent()
            .ok_or_else(|| error("Missing recovery owner"))?;
        let marker = parent.join(format!("discard-{}", entry.project));
        if !marker.exists() {
            write_new(&marker, b"discarded\n")?;
        }
        Ok(())
    }
    pub fn discard(entry: &RecoveryEntry) -> Result<()> {
        let parent = entry
            .generation
            .parent()
            .ok_or_else(|| error("Missing recovery owner"))?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(parent.join("lease"))
            .map_err(error)?;
        lease
            .try_lock()
            .map_err(|_| error("Recovery is in use by another instance"))?;
        if !Self::entries(parent)?
            .iter()
            .any(|e| e.id == entry.id && e.digest == entry.digest)
        {
            return Err(error("Recovery is unavailable or corrupt"));
        }
        Self::mark_discarded(entry)?;
        // Tombstone every generation for this project, so an older one cannot return.
        for generation in Self::entries(parent)?
            .into_iter()
            .filter(|e| e.project == entry.project)
        {
            write_new(&generation.generation.join("discarded"), b"discarded\n")?;
        }
        Self::cleanup_closed(parent)?;
        sync_dir(parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_instances_are_isolated_and_corrupt_latest_falls_back_without_touching_manual_save() {
        let root = std::env::temp_dir().join(format!("daw-recovery-{}", Id::new_v4()));
        let manual = root.join("manual");
        let mut s = Session::default();
        s.save(&manual).unwrap();
        let original = fs::read(manual.join("project.json")).unwrap();
        let store = RecoveryStore::create(&root).unwrap();
        let mut last = None;
        for revision in 0..4 {
            s.project.name = format!("Revision {revision}");
            last = Some(
                store
                    .checkpoint(
                        s.save_snapshot(None).unwrap(),
                        revision,
                        Some(manual.clone()),
                        None,
                    )
                    .unwrap(),
            );
        }
        assert!(RecoveryStore::scan(&root).unwrap().is_empty());
        assert_eq!(SelfTest::entries(&store).len(), 3);
        fs::write(last.unwrap().generation.join("project.json"), b"broken").unwrap();
        drop(store);
        let entries = RecoveryStore::scan(&root).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].revision, 2);
        assert_eq!(fs::read(manual.join("project.json")).unwrap(), original);
        let recovered = RecoveryStore::recover(&entries[0]).unwrap();
        assert_eq!(recovered.project.name, "Revision 2");
        assert!(RecoveryStore::scan(&root).unwrap().is_empty());
        drop(recovered);
        RecoveryStore::discard(&entries[0]).unwrap();
        assert!(RecoveryStore::scan(&root).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn admission_and_owned_cleanup_preserve_unknown_data_and_order_survives_clock_regression() {
        let root = std::env::temp_dir().join(format!("daw-recovery-bounds-{}", Id::new_v4()));
        let store = RecoveryStore::create(&root).unwrap();
        let session = Session::default();
        let mut first = store
            .checkpoint(session.save_snapshot(None).unwrap(), 100, None, None)
            .unwrap();
        first.order = first.order.saturating_add(1_000_000_000);
        fs::write(
            first.generation.join("recovery.json"),
            serde_json::to_vec(&first).unwrap(),
        )
        .unwrap();
        let next = store
            .checkpoint(session.save_snapshot(None).unwrap(), 0, None, None)
            .unwrap();
        assert!(next.order > first.order);
        assert!(store.admit_metadata(LIMIT + 1).is_err());
        let unknown = store.folder.join(Id::new_v4().to_string());
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join("unrelated"), b"preserve").unwrap();
        store.clean_exit().unwrap();
        assert!(!first.generation.exists());
        assert!(!next.generation.exists());
        assert_eq!(fs::read(unknown.join("unrelated")).unwrap(), b"preserve");
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_workspaces_do_not_consume_recording_slots() {
        let root = std::env::temp_dir().join(format!("daw-recovery-slots-{}", Id::new_v4()));
        let empty: Vec<_> = (0..23)
            .map(|_| RecoveryStore::create(&root).unwrap())
            .collect();
        fs::write(empty[0].folder.join("discard-old-project"), b"closed").unwrap();
        let session = Session::default();
        let current = RecoveryStore::create(&root).unwrap();
        current.reserve_take(&session.project).unwrap();

        let entry = current
            .checkpoint(session.save_snapshot(None).unwrap(), 1, None, None)
            .unwrap();
        let manifest = entry.generation.join("project.json");
        let original = fs::read(&manifest).unwrap();
        let journal = empty[0].folder.join("take-test.journal");
        fs::write(&journal, b"journal data").unwrap();
        // An interrupted take still occupies its workspace even if its
        // journal or checkpoint cannot currently be read.
        for store in &empty[1..19] {
            fs::write(store.folder.join("active-take"), b"interrupted").unwrap();
        }
        current.reserve_take(&session.project).unwrap();
        assert!(empty[19].reserve_take(&session.project).is_err());
        assert_eq!(fs::read(manifest).unwrap(), original);
        assert_eq!(fs::read(journal).unwrap(), b"journal data");
        assert!(empty[1].folder.join("active-take").exists());
        assert!(empty[22].folder.join("owner").exists());

        drop(current);
        drop(empty);
        fs::remove_dir_all(root).unwrap();
    }

    struct SelfTest;
    impl SelfTest {
        fn entries(store: &RecoveryStore) -> Vec<RecoveryEntry> {
            RecoveryStore::entries(&store.folder).unwrap()
        }
    }
}
