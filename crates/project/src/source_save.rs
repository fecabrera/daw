//! Worker save snapshots retain every protected project-owned source. A write
//! receipt is transient: callers merge it, then discard it rather than caching paths.
use crate::source_collection::{
    execute::IoStep,
    owned_fs::{CleanupMode, Store},
};
use crate::{Result, Session, SessionHistory, error};
use daw_core::{Id, Project, Source};
use daw_media::{AudioData, SourceRevision};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

mod verification;
use verification::PreparedVerification;

pub struct SaveSnapshot {
    session: Session,
    protected: Vec<Session>,
}
pub struct SavedSession {
    pub session: Session,
    pub(crate) relocations: Vec<Relocation>,
}
#[derive(Clone)]
pub(crate) struct Relocation {
    pub asset_id: Id,
    pub original: PathBuf,
    pub original_source: Source,
    pub audio: Option<AudioData>,
    pub source: Source,
}
impl Session {
    /// Capture on the control thread; write on a worker. Shares prepared buffers.
    pub fn save_snapshot(&self, history: Option<&SessionHistory>) -> Result<SaveSnapshot> {
        if history.is_some_and(|h| h.project_id() != self.project.project_id) {
            return Err(error("Save history belongs to a different project"));
        }
        Ok(SaveSnapshot {
            session: self.clone(),
            protected: history
                .into_iter()
                .flat_map(|h| h.collection_roots().map(|(_, s)| s.clone()))
                .collect(),
        })
    }
    /// Compatibility path for callers that saved only the current Session.
    /// New worker callers use SavedSession::apply to include history-only owners.
    pub fn retarget_project_sources(&mut self, written: &Session) -> Result<()> {
        if self.project.project_id != written.project.project_id {
            return Err(error("Saved sources belong to a different project"));
        }
        let mut next = self.project.clone();
        normalize_external(self, &mut next)?;
        for asset in &mut next.assets {
            if !owned(&asset.source) {
                continue;
            }
            if let Some(saved) = written
                .project
                .assets
                .iter()
                .find(|a| a.id == asset.id && owned(&a.source))
            {
                if !same_audio(self.audio.get(&asset.id), written.audio.get(&asset.id)) {
                    continue;
                }
                asset.source = saved.source.clone();
                if asset.source.kind == "generated" {
                    asset.source.path = written
                        .source_path_unchecked(saved)?
                        .to_string_lossy()
                        .into_owned();
                    asset.source.path_kind = "absolute".into();
                }
            } else if asset.source.is_included_recording() && self.folder != written.folder {
                return Err(error(
                    "Save As requires a save snapshot with all protected included owners",
                ));
            }
        }
        next.validate().map_err(error)?;
        self.project = next;
        self.folder = written.folder.clone();
        self.retain_temporary_stems();
        Ok(())
    }
}
pub(crate) fn owned(source: &Source) -> bool {
    source.kind == "generated" || source.is_included_recording()
}
fn same_audio(a: Option<&AudioData>, b: Option<&AudioData>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.sample_rate_hz == b.sample_rate_hz
                && a.metadata == b.metadata
                && Arc::ptr_eq(&a.samples, &b.samples)
        }
        (None, None) => true,
        _ => false,
    }
}
fn normalize_external(session: &Session, project: &mut Project) -> Result<()> {
    for asset in &mut project.assets {
        if asset.midi_pattern.is_none() && !asset.source.is_included_recording() {
            asset.source.path = session
                .source_path_unchecked(asset)?
                .to_string_lossy()
                .into_owned();
            asset.source.path_kind = "absolute".into();
        }
    }
    Ok(())
}
pub(crate) fn relocated_project(
    session: &Session,
    relocations: &[Relocation],
    folder: &Path,
) -> Result<Project> {
    let mut project = session.project.clone();
    normalize_external(session, &mut project)?;
    for asset in &mut project.assets {
        if asset.midi_pattern.is_some() {
            continue;
        }
        let original = session.source_path_unchecked(asset)?;
        if let Some(entry) = relocations.iter().find(|entry| {
            entry.asset_id == asset.id
                && entry.original_source.kind == asset.source.kind
                && entry.original_source.storage == asset.source.storage
                && entry.original == original
                && same_audio(entry.audio.as_ref(), session.audio.get(&asset.id))
        }) {
            asset.source = entry.source.clone();
            // Generated history can retain an absolute path. Included recordings
            // must retain a relative path and the matching new folder.
            if asset.source.kind == "generated" {
                asset.source.path = folder
                    .join(&entry.source.path)
                    .to_string_lossy()
                    .into_owned();
                asset.source.path_kind = "absolute".into();
            }
        } else if asset.source.is_included_recording() && session.folder.as_deref() != Some(folder)
        {
            return Err(error(
                "Save receipt does not cover an included source in the active session/history",
            ));
        }
    }
    project.validate().map_err(error)?;
    Ok(project)
}
impl SavedSession {
    /// Merge only matching source owners. Newer edits, PCM and playhead survive.
    /// All models are validated before any live/history mutation.
    pub fn apply(&self, live: &mut Session, history: Option<&mut SessionHistory>) -> Result<()> {
        if live.project.project_id != self.session.project.project_id
            || history
                .as_ref()
                .is_some_and(|h| h.project_id() != live.project.project_id)
        {
            return Err(error("Saved sources belong to a different project"));
        }
        let folder = self
            .session
            .folder
            .as_deref()
            .ok_or_else(|| error("Saved session has no folder"))?;
        let project = relocated_project(live, &self.relocations, folder)?;
        let saved = crate::history::saved_project(&self.session)?;
        let prepared = history
            .as_ref()
            .map(|h| h.prepare_source_relocation(&self.relocations, folder))
            .transpose()?;
        live.project = project;
        live.folder = Some(folder.to_owned());
        if let Some(history) = history {
            history.publish_source_relocation(prepared.unwrap(), &saved, folder);
        }
        live.retain_temporary_stems();
        Ok(())
    }
}
impl SaveSnapshot {
    pub fn save_new(mut self, parent: &Path, name: &str) -> Result<SavedSession> {
        let name = crate::project_folder_name(name)?;
        let folder = crate::absolute(parent)?.join(name);
        self.session.project.name = name.to_owned();
        fs::create_dir(&folder).map_err(|e| if e.kind()==std::io::ErrorKind::AlreadyExists { error("A folder with this project name already exists; choose another name or location") } else {error(e)})?;
        let result = self.save(&folder);
        if result.is_err() {
            let _ = fs::remove_dir(folder.join("assets"));
            let _ = fs::remove_dir(folder.join("stems"));
            let _ = fs::remove_dir(&folder);
        }
        result
    }
    pub fn save(self, folder: &Path) -> Result<SavedSession> {
        let folder = crate::absolute(folder)?;
        fs::create_dir_all(&folder).map_err(error)?;
        let canonical_folder = fs::canonicalize(&folder).map_err(error)?;
        let mut store = Store::new(&canonical_folder)?;
        store.ensure_directory(Path::new("assets"), &mut |_, _| Ok(()))?;
        let result = self.write(&canonical_folder, &mut store);
        let cleanup = store.cleanup();
        match result {
            Ok(mut saved) => {
                // All writes use the pinned physical root. Preserve the caller's
                // folder spelling only while it still names that same root.
                if fs::canonicalize(&folder).is_ok_and(|path| path == canonical_folder) {
                    saved.session.folder = Some(folder);
                } else {
                    saved.session.warnings.push(
                        "Project saved to its physical folder; the requested folder alias changed"
                            .into(),
                    );
                }
                saved.session.warnings.extend(cleanup);
                Ok(saved)
            }
            Err(e) if cleanup.is_empty() => Err(e),
            Err(e) => Err(error(format!(
                "{e}; cleanup warnings: {}",
                cleanup.join("; ")
            ))),
        }
    }
    fn write(self, folder: &Path, store: &mut Store) -> Result<SavedSession> {
        self.session.project.validate().map_err(error)?;
        let canonical_folder = fs::canonicalize(folder).map_err(error)?;
        match fs::symlink_metadata(folder.join("project.json")) {
            Ok(metadata) if metadata.is_file() && !crate::file_publish::is_reparse(&metadata) => {}
            Ok(_) => return Err(error("Save target must be an ordinary file")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(error(e)),
        }
        let mut relocations = Vec::new();
        let mut verified_files = BTreeMap::new();
        let mut prepared_checks = PreparedVerification::default();
        let mut targets = BTreeMap::<(PathBuf, bool), (Source, bool)>::new();
        let mut groups = BTreeMap::<PathBuf, PathBuf>::new();
        let mut hook = |_: IoStep, _: &Path| Ok(());
        for session in std::iter::once(&self.session).chain(&self.protected) {
            for asset in session
                .project
                .assets
                .iter()
                .filter(|a| a.midi_pattern.is_none() && owned(&a.source))
            {
                let source = session.source_path(asset)?;
                let canonical = match fs::canonicalize(&source) {
                    Ok(path) => path,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        let resolved = crate::source_storage::canonical_or_missing(&source)?;
                        if !resolved.starts_with(&canonical_folder) {
                            return Err(error(e));
                        }
                        resolved
                    }
                    Err(e) => return Err(error(e)),
                };
                let key = (canonical.clone(), asset.source.is_included_recording());
                let (target, copied) = if let Some(target) = targets.get(&key) {
                    target.clone()
                } else {
                    let reused = if asset.source.is_included_recording() {
                        canonical == canonical_folder.join(&asset.source.path)
                    } else {
                        canonical.starts_with(canonical_folder.join("stems"))
                    };
                    let mut target = asset.source.clone();
                    let relative = if reused {
                        canonical
                            .strip_prefix(&canonical_folder)
                            .map_err(error)?
                            .to_owned()
                    } else if asset.source.is_included_recording() {
                        PathBuf::from(format!(
                            "assets/sources/{}.{}",
                            Id::new_v4(),
                            daw_media::source_file_extension(
                                &canonical,
                                &session
                                    .audio
                                    .get(&asset.id)
                                    .ok_or_else(|| error("Missing prepared included source"))?
                                    .metadata
                            )
                            .map_err(error)?
                        ))
                    } else {
                        let parent = canonical
                            .parent()
                            .ok_or_else(|| error("Missing stem folder"))?;
                        let dir = if let Some(dir) = groups.get(parent) {
                            dir.clone()
                        } else {
                            let mut dir = PathBuf::from(format!("stems/{}", Id::new_v4()));
                            while !store.reserve_directory(&dir, false, &mut hook)? {
                                dir = PathBuf::from(format!("stems/{}", Id::new_v4()));
                            }
                            groups.insert(parent.to_owned(), dir.clone());
                            dir
                        };
                        dir.join(
                            canonical
                                .file_name()
                                .ok_or_else(|| error("Missing stem filename"))?,
                        )
                    };
                    let relative = if reused {
                        relative
                    } else {
                        let mut relative = relative;
                        let index = loop {
                            if let Some(index) = store.reserve_file(&relative, false, &mut hook)? {
                                break index;
                            }
                            relative = if asset.source.is_included_recording() {
                                PathBuf::from(format!(
                                    "assets/sources/{}.{}",
                                    Id::new_v4(),
                                    relative.extension().unwrap().to_string_lossy()
                                ))
                            } else {
                                let dir = PathBuf::from(format!("stems/{}", Id::new_v4()));
                                if !store.reserve_directory(&dir, false, &mut hook)? {
                                    continue;
                                }
                                dir.join(canonical.file_name().unwrap())
                            };
                        };
                        let revision =
                            SourceRevision::read(&canonical, &|| false).map_err(error)?;
                        let mut input = fs::File::open(&canonical).map_err(error)?;
                        let mut buffer = [0_u8; 64 * 1024];
                        let mut remaining = revision.byte_length;
                        while remaining > 0 {
                            let limit = remaining.min(buffer.len() as u64) as usize;
                            let n = input.read(&mut buffer[..limit]).map_err(error)?;
                            if n == 0 {
                                return Err(error(
                                    "Project-owned source was truncated during save",
                                ));
                            }
                            remaining -= n as u64;
                            store
                                .file_mut(index)?
                                .write_all(&buffer[..n])
                                .map_err(error)?;
                        }
                        if input.read(&mut buffer[..1]).map_err(error)? != 0 {
                            return Err(error("Project-owned source grew during save"));
                        }
                        store.file_mut(index)?.flush().map_err(error)?;
                        store.file_mut(index)?.sync_all().map_err(error)?;
                        if SourceRevision::read(&canonical, &|| false).map_err(error)? != revision
                            || SourceRevision::read(&store.files[index].path, &|| false)
                                .map_err(error)?
                                != revision
                        {
                            return Err(error("Project-owned source changed during save"));
                        }
                        verified_files.insert(canonical.clone(), revision);
                        verified_files.insert(store.files[index].path.clone(), revision);
                        store.seal_file(index)?;
                        relative
                    };
                    target.path = relative
                        .to_str()
                        .ok_or_else(|| error("Project media path is not UTF-8"))?
                        .replace('\\', "/");
                    target.path_kind = "relative".into();
                    target.validate_storage().map_err(error)?;
                    targets.insert(key, (target.clone(), !reused));
                    (target, !reused)
                };
                // Verify every retained prepared version, including cache-stale and
                // history-only owners, against the exact destination bytes.
                if !copied {
                    // Ordinary Save preserves included references, including
                    // missing-source warnings, without reopening their media.
                } else if let Some(audio) = session.audio.get(&asset.id) {
                    let destination = folder.join(&target.path);
                    let revision = *verified_files
                        .get(&destination)
                        .ok_or_else(|| error("Missing saved source revision"))?;
                    prepared_checks.verify(&destination, revision, audio)?;
                } else {
                    return Err(error("Missing prepared project-owned source"));
                }
                relocations.push(Relocation {
                    asset_id: asset.id,
                    original: session.source_path_unchecked(asset)?,
                    original_source: asset.source.clone(),
                    audio: session.audio.get(&asset.id).cloned(),
                    source: target,
                });
            }
        }
        for session in &self.protected {
            relocated_project(session, &relocations, folder)?;
        }
        let mut next = self.session.clone();
        next.project = relocated_project(&self.session, &relocations, folder)?;
        next.folder = Some(folder.to_owned());
        // Keep included paths portable; preserve normal external Save behavior.
        for asset in &mut next.project.assets {
            if asset.midi_pattern.is_some() || asset.source.is_included_recording() {
                continue;
            }
            let path = PathBuf::from(&asset.source.path);
            if let Some(relative) = crate::relative_path(folder, &path) {
                asset.source.path = relative.to_string_lossy().into_owned();
                if asset.source.kind == "generated" {
                    // Project-owned paths must reopen on hosts with a different
                    // native separator. External references retain host spelling.
                    asset.source.path = asset.source.path.replace('\\', "/");
                }
                asset.source.path_kind = "relative".into();
            }
        }
        next.project.validate().map_err(error)?;
        let bytes = serde_json::to_vec_pretty(&next.project).map_err(error)?;
        let mut temporary = PathBuf::from(format!(".project-{}.tmp", Id::new_v4()));
        let index = loop {
            if let Some(i) = store.reserve_file(&temporary, true, &mut hook)? {
                break i;
            }
            temporary = PathBuf::from(format!(".project-{}.tmp", Id::new_v4()));
        };
        store.file_mut(index)?.write_all(&bytes).map_err(error)?;
        store.file_mut(index)?.sync_all().map_err(error)?;
        store.sync_directories(&mut hook, &mut next.warnings)?;
        for i in 0..store.files.len() {
            store.check_file(i)?;
        }
        for (path, revision) in verified_files {
            if SourceRevision::read(&path, &|| false).map_err(error)? != revision {
                return Err(error(
                    "Project-owned source changed before save publication",
                ));
            }
        }
        let target = folder.join("project.json");
        let old = match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.is_file() && !crate::file_publish::is_reparse(&metadata) => {
                Some(SourceRevision::read(&target, &|| false).map_err(error)?)
            }
            Ok(_) => return Err(error("Save target must be an ordinary file")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(error(e)),
        };
        let candidate = SourceRevision::read(&store.files[index].path, &|| false).map_err(error)?;
        match crate::file_publish::replace(&store.files[index].path, &target) {
            Ok(()) => store.mode = CleanupMode::Committed,
            Err(e) => {
                let actual = store
                    .check_root()
                    .ok()
                    .and_then(|()| {
                        fs::symlink_metadata(&target)
                            .ok()
                            .filter(|m| m.is_file() && !crate::file_publish::is_reparse(m))
                    })
                    .and_then(|_| SourceRevision::read(&target, &|| false).ok());
                if actual == Some(candidate) && old != Some(candidate) {
                    store.mode = CleanupMode::Committed;
                    next.warnings
                        .push(format!("Project saved despite a replacement error: {e}"));
                } else if actual == old && old != Some(candidate) {
                    return Err(error(e));
                } else {
                    store.mode = CleanupMode::KeepAll;
                    return Err(error(format!(
                        "Save commit could not be confirmed; retain media and {} for recovery: {e}",
                        store.files[index].path.display()
                    )));
                }
            }
        }
        if let Err(e) = store.sync_directories(&mut hook, &mut next.warnings) {
            next.warnings.push(format!(
                "Project saved; directory durability is uncertain: {e}"
            ));
        }
        next.retain_temporary_stems();
        Ok(SavedSession {
            session: next,
            relocations,
        })
    }
}
