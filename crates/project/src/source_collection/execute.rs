//! The manifest replacement is the commit point. No live Session/history is
//! mutated here; Stage 3 consumes the prevalidated relocation receipt.
use super::owned_fs::{CleanupMode, Store};
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
};
mod copy;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionPhase {
    Validating,
    Staging,
    Verifying,
    Publishing,
    Committed,
    RecoveryRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectionProgress {
    pub phase: CollectionPhase,
    pub staged_bytes: u64,
    pub total_staging_bytes: u64,
    pub copied_bytes: u64,
    pub total_copy_bytes: u64,
    pub files_copied: u64,
    pub total_files: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionStatus {
    Committed,
    RecoveryRequired,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelocatedSource {
    pub owner: usize,
    pub scope: SourceScope,
    pub asset_id: Id,
    pub original_source: Source,
    pub original_folder: Option<PathBuf>,
    pub revision: SourceRevision,
    pub source: Source,
}
pub struct RelocationReceipt {
    project_id: Id,
    project_revision: u64,
    folder: PathBuf,
    entries: Vec<RelocatedSource>,
}
impl RelocationReceipt {
    pub fn project_id(&self) -> Id {
        self.project_id
    }
    pub fn project_revision(&self) -> u64 {
        self.project_revision
    }
    pub fn project_folder(&self) -> &Path {
        &self.folder
    }
    pub fn entries(&self) -> &[RelocatedSource] {
        &self.entries
    }
}
pub struct CollectionOutcome {
    status: CollectionStatus,
    project: Project,
    receipt: RelocationReceipt,
    snapshot: SourceCollectionSnapshot,
    progress: CollectionProgress,
    warnings: Vec<String>,
    recovery_manifest: Option<PathBuf>,
}
impl CollectionOutcome {
    /// Apply a confirmed commit without an edit entry, DSP preparation or file I/O.
    /// The host must retain its collection edit/job exclusion through this call.
    pub fn apply(
        &self,
        live: &mut Session,
        history: Option<&mut SessionHistory>,
        revision: u64,
    ) -> Result<()> {
        if self.status != CollectionStatus::Committed {
            return Err(error(
                "Unconfirmed collection requires recovery before publication",
            ));
        }
        if !self.snapshot.is_current(live, history.as_deref(), revision) {
            return Err(error(
                "Committed collection no longer matches the active session",
            ));
        }
        let relocations = receipt_entries(&self.snapshot, &self.receipt)?;
        let mut written = self.snapshot.roots[0].session.clone();
        written.project = self.project.clone();
        written.folder = Some(self.receipt.folder.clone());
        crate::SavedSession {
            session: written,
            relocations,
        }
        .apply(live, history)
    }
    pub fn status(&self) -> CollectionStatus {
        self.status
    }
    /// Authoritative only for Committed; otherwise this is a recovery candidate.
    pub fn candidate_project(&self) -> &Project {
        &self.project
    }
    pub fn receipt(&self) -> &RelocationReceipt {
        &self.receipt
    }
    pub fn snapshot(&self) -> &SourceCollectionSnapshot {
        &self.snapshot
    }
    pub fn progress(&self) -> CollectionProgress {
        self.progress
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn recovery_manifest(&self) -> Option<&Path> {
        self.recovery_manifest.as_deref()
    }
}

/// Deterministic fault injection around real I/O, not a replacement filesystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IoStep {
    CreateDirectory,
    ReserveFile,
    Read,
    Write,
    Flush,
    FileSync,
    DirectorySync,
    BeforeCommit,
    Replace,
    AfterReplace,
}

struct Control<'a> {
    plan: &'a SourceCollectionPlan,
    current: &'a dyn Fn(&SourceCollectionSnapshot) -> bool,
    cancelled: &'a dyn Fn() -> bool,
    progress: &'a mut dyn FnMut(CollectionProgress) -> Result<()>,
    hook: &'a mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
    state: CollectionProgress,
    warnings: Vec<String>,
}
impl Control<'_> {
    fn check_cancel(&self) -> Result<()> {
        if (self.cancelled)() {
            Err(error("Source collection cancelled"))
        } else {
            Ok(())
        }
    }
    fn check_current(&self) -> Result<()> {
        self.check_cancel()?;
        if !(self.current)(self.plan.snapshot()) {
            return Err(error("Source collection plan is stale"));
        }
        Ok(())
    }
    fn notify(&mut self, phase: CollectionPhase) -> Result<()> {
        self.state.phase = phase;
        (self.progress)(self.state)
    }
}

impl SourceCollectionPlan {
    /// Blocking worker API. The host must exclude edits/competing file jobs and
    /// supply a live job/revision guard, checked before copying and publication.
    /// Progress failures before commit abort; failures after commit are warnings.
    pub fn execute(
        &self,
        still_current: &dyn Fn(&SourceCollectionSnapshot) -> bool,
        cancelled: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(CollectionProgress) -> Result<()>,
    ) -> Result<CollectionOutcome> {
        execute_with(self, still_current, cancelled, progress, &mut |_, _| Ok(()))
    }
}

fn execute_with(
    plan: &SourceCollectionPlan,
    current: &dyn Fn(&SourceCollectionSnapshot) -> bool,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(CollectionProgress) -> Result<()>,
    hook: &mut dyn FnMut(IoStep, &Path) -> io::Result<()>,
) -> Result<CollectionOutcome> {
    let total_staging_bytes = plan.sources().iter().try_fold(0_u64, |n, s| {
        n.checked_add(s.revision.byte_length)
            .ok_or_else(|| error("Collection staging totals overflow"))
    })?;
    // Both raw private copies and final files can coexist. Metadata/FS overhead
    // is additional; this checked bound is advisory, never a space reservation.
    total_staging_bytes
        .checked_add(plan.summary().additional_bytes)
        .ok_or_else(|| error("Collection peak byte totals overflow"))?;
    let mut control = Control {
        plan,
        current,
        cancelled,
        progress,
        hook,
        state: CollectionProgress {
            phase: CollectionPhase::Validating,
            staged_bytes: 0,
            total_staging_bytes,
            copied_bytes: 0,
            total_copy_bytes: plan.summary().additional_bytes,
            files_copied: 0,
            total_files: plan.summary().files_to_copy,
        },
        warnings: Vec::new(),
    };
    control.check_current()?;
    control.notify(CollectionPhase::Validating)?;
    let mut store = Store::new(plan.project_folder())?;
    let result = run(&mut store, &mut control);
    let cleanup = store.cleanup();
    match result {
        Ok(mut outcome) => {
            outcome.warnings.extend(cleanup);
            outcome.warnings.sort();
            outcome.warnings.dedup();
            Ok(outcome)
        }
        Err(e) if cleanup.is_empty() => Err(e),
        Err(e) => Err(error(format!(
            "{e}; cleanup warnings: {}",
            cleanup.join("; ")
        ))),
    }
}

fn check_manifest(store: &Store, control: &Control<'_>) -> Result<()> {
    control.check_current()?;
    store.check_root()?;
    paths::DestinationTree::default().check(
        &store.root,
        Path::new("project.json"),
        true,
        &|| (control.cancelled)(),
    )?;
    let revision =
        SourceRevision::read(&store.root.join("project.json"), &|| (control.cancelled)())
            .map_err(error)?;
    if revision != control.plan.manifest_revision() {
        return Err(error("Saved project changed since collection preflight"));
    }
    Ok(())
}

fn check_source(control: &Control<'_>, source: &CollectionSource) -> Result<()> {
    control.check_current()?;
    for &index in &source.owners {
        let owner = &control.plan.snapshot().owners[index];
        let session = &control.plan.snapshot().roots[owner.root_index].session;
        let resolved =
            fs::canonicalize(session.source_path(&session.project.assets[owner.asset_index])?)
                .map_err(error)?;
        if resolved != source.path {
            return Err(error("Source reference changed since collection preflight"));
        }
    }
    if SourceRevision::read(&source.path, &|| (control.cancelled)()).map_err(error)?
        != source.revision
    {
        return Err(error(format!(
            "Source changed since preflight: {}",
            source.path.display()
        )));
    }
    Ok(())
}

fn run(store: &mut Store, control: &mut Control<'_>) -> Result<CollectionOutcome> {
    check_manifest(store, control)?;
    for source in control.plan.sources() {
        check_source(control, source)?;
    }
    let mut stage = PathBuf::from(format!(
        "assets/.collect-{}",
        control.plan.snapshot().operation_id()
    ));
    while !store.reserve_directory(&stage, true, control.hook)? {
        control.check_current()?;
        stage = PathBuf::from(format!("assets/.collect-{}", Id::new_v4()));
    }
    let mut destinations = Vec::new();
    let mut stem_groups = BTreeMap::new();
    // Missing paths referenced by the old manifest are protected too. A new
    // uncommitted file must never fill an old reference by UUID coincidence.
    let mut destination_names = saved_reference_paths(control.plan, &store.root)?;
    for (index, source) in control.plan.sources().iter().enumerate() {
        check_source(control, source)?;
        let prepared = control.plan.snapshot().owners[source.owners[0]]
            .prepared_audio()
            .ok_or_else(|| error("Missing prepared source owner"))?;
        let extension =
            daw_media::source_file_extension(&source.path, &prepared.metadata).map_err(error)?;
        let raw = stage.join(format!("source-{index}.{extension}"));
        let raw_index = store
            .reserve_file(&raw, true, control.hook)?
            .ok_or_else(|| error("Private staging filename was occupied"))?;
        copy::stream(
            &source.path,
            raw_index,
            source.revision.byte_length,
            true,
            store,
            control,
        )?;
        control.notify(CollectionPhase::Verifying)?;
        verify_file(store, raw_index, source.revision, control)?;
        let mut checked = Vec::<&AudioData>::new();
        for &owner in &source.owners {
            let prepared = control.plan.snapshot().owners[owner]
                .prepared_audio()
                .ok_or_else(|| error("Missing prepared source owner"))?;
            if checked.iter().any(|old| {
                old.sample_rate_hz == prepared.sample_rate_hz
                    && old.metadata == prepared.metadata
                    && Arc::ptr_eq(&old.samples, &prepared.samples)
            }) {
                continue;
            }
            if daw_media::verify_prepared_source(&store.files[raw_index].path, prepared, &|| {
                (control.cancelled)()
            })
            .map_err(error)?
                != source.revision
            {
                return Err(error("Staged content revision mismatch"));
            }
            checked.push(prepared);
        }
        for target in &source.targets {
            control.check_current()?;
            let relative = if target.needs_copy {
                let (relative, file_index) = copy::reserve_target(
                    target,
                    store,
                    control,
                    &mut stem_groups,
                    &mut destination_names,
                )?;
                let input = store.files[raw_index].path.clone();
                copy::stream(
                    &input,
                    file_index,
                    source.revision.byte_length,
                    false,
                    store,
                    control,
                )?;
                verify_file(store, file_index, source.revision, control)?;
                store.seal_file(file_index)?;
                control.state.files_copied = control
                    .state
                    .files_copied
                    .checked_add(1)
                    .ok_or_else(|| error("Copied file count overflow"))?;
                control.notify(CollectionPhase::Staging)?;
                relative
            } else {
                paths::DestinationTree::default().check(
                    &store.root,
                    Path::new(&target.relative_path),
                    true,
                    &|| (control.cancelled)(),
                )?;
                if fs::canonicalize(store.root.join(&target.relative_path)).map_err(error)?
                    != source.path
                {
                    return Err(error("Included source location changed"));
                }
                destination_names.insert(target.relative_path.to_lowercase());
                target.relative_path.clone()
            };
            destinations.push((source.revision, target.owners.clone(), relative));
        }
        store.remove_temporary(raw_index)?;
    }
    let (project, receipt) = build_receipt(control.plan, &destinations)?;
    let relocations = receipt_entries(control.plan.snapshot(), &receipt)?;
    for root in &control.plan.snapshot().roots {
        crate::source_save::relocated_project(&root.session, &relocations, &receipt.folder)?;
    }
    let bytes = serde_json::to_vec_pretty(&project).map_err(error)?;
    let mut manifest_name = PathBuf::from(format!(
        ".project-{}.tmp",
        control.plan.snapshot().operation_id()
    ));
    let manifest_index = loop {
        control.check_current()?;
        if let Some(index) = store.reserve_file(&manifest_name, true, control.hook)? {
            break index;
        }
        manifest_name = PathBuf::from(format!(".project-{}.tmp", Id::new_v4()));
    };
    copy::write_manifest(&bytes, manifest_index, store, control)?;
    let candidate_path = store.files[manifest_index].path.clone();
    let candidate_revision =
        SourceRevision::read(&candidate_path, &|| (control.cancelled)()).map_err(error)?;
    control.notify(CollectionPhase::Publishing)?;
    (control.hook)(IoStep::BeforeCommit, &candidate_path).map_err(error)?;
    store.sync_directories(control.hook, &mut control.warnings)?;
    check_manifest(store, control)?;
    for source in control.plan.sources() {
        check_source(control, source)?;
    }
    // Check the actual final entries, including reused files, not only their private proofs.
    for (revision, _, relative) in &destinations {
        paths::DestinationTree::default().check(&store.root, Path::new(relative), true, &|| {
            (control.cancelled)()
        })?;
        if SourceRevision::read(&store.root.join(relative), &|| (control.cancelled)())
            .map_err(error)?
            != *revision
        {
            return Err(error("Final media changed before manifest publication"));
        }
    }
    for (index, file) in store.files.iter().enumerate() {
        if file.removed {
            continue;
        }
        store.check_file(index)?;
        if file.path == candidate_path
            && SourceRevision::read(&candidate_path, &|| (control.cancelled)()).map_err(error)?
                != candidate_revision
        {
            return Err(error("Candidate manifest changed before publication"));
        }
    }
    check_manifest(store, control)?; // Includes the last cancellation/current-state check.
    let target = store.root.join("project.json");
    let replacement = (control.hook)(IoStep::Replace, &candidate_path)
        .and_then(|_| crate::file_publish::replace(&candidate_path, &target));
    let status = match replacement {
        Ok(()) => {
            store.mode = CleanupMode::Committed;
            CollectionStatus::Committed
        }
        Err(e) => match store.check_root().and_then(|_| {
            paths::ordinary(&store.root, Path::new("project.json"), true)?;
            SourceRevision::read(&target, &|| false).map_err(error)
        }) {
            Ok(actual)
                if actual == candidate_revision
                    && candidate_revision != control.plan.manifest_revision() =>
            {
                store.mode = CleanupMode::Committed;
                control.warnings.push(format!("Manifest replacement reported an error, but the committed bytes were confirmed: {e}"));
                CollectionStatus::Committed
            }
            Ok(actual)
                if actual == control.plan.manifest_revision()
                    && candidate_revision != control.plan.manifest_revision() =>
            {
                return Err(error(format!(
                    "Manifest replacement failed before commit: {e}"
                )));
            }
            _ => {
                store.mode = CleanupMode::KeepAll;
                control.warnings.push(format!("Manifest commit could not be confirmed; retain all media and the candidate manifest for recovery: {e}"));
                CollectionStatus::RecoveryRequired
            }
        },
    };
    // Rollback is already disarmed before post-commit work and callbacks.
    if status == CollectionStatus::Committed {
        if let Err(e) = (control.hook)(IoStep::AfterReplace, &target) {
            control
                .warnings
                .push(format!("Collection committed; notification failed: {e}"));
        }
        // The temporary manifest path was moved. Do not include it in later directory sync discovery.
        if let Err(e) = store.sync_directories(control.hook, &mut control.warnings) {
            control.warnings.push(format!(
                "Collection committed; directory durability is uncertain: {e}"
            ));
        }
    }
    let phase = if status == CollectionStatus::Committed {
        CollectionPhase::Committed
    } else {
        CollectionPhase::RecoveryRequired
    };
    if let Err(e) = control.notify(phase) {
        control.warnings.push(format!(
            "Collection publication is {:?}; progress delivery failed: {e}",
            status
        ));
    }
    Ok(CollectionOutcome {
        status,
        project,
        receipt,
        snapshot: control.plan.snapshot().clone(),
        progress: control.state,
        warnings: std::mem::take(&mut control.warnings),
        recovery_manifest: if status == CollectionStatus::RecoveryRequired {
            Some(candidate_path)
        } else {
            None
        },
    })
}

fn verify_file(
    store: &Store,
    index: usize,
    revision: SourceRevision,
    control: &Control<'_>,
) -> Result<()> {
    store.check_file(index)?;
    if SourceRevision::read(&store.files[index].path, &|| (control.cancelled)()).map_err(error)?
        != revision
    {
        return Err(error(
            "Copied source content differs from its verified revision",
        ));
    }
    Ok(())
}

fn build_receipt(
    plan: &SourceCollectionPlan,
    destinations: &[(SourceRevision, Vec<usize>, String)],
) -> Result<(Project, RelocationReceipt)> {
    let mut entries = (0..plan.snapshot().owners.len())
        .map(|_| None)
        .collect::<Vec<Option<RelocatedSource>>>();
    for (revision, owners, relative) in destinations {
        for &index in owners {
            let owner = &plan.snapshot().owners[index];
            if entries[index].is_some() {
                return Err(error("Collection owner has multiple relocation targets"));
            }
            let mut source = owner.source.clone();
            source.path = relative.clone();
            source.path_kind = "relative".into();
            if source.kind == "external" {
                source.storage = Some(daw_core::SourceStorage::Project);
            }
            source.validate_storage().map_err(error)?;
            entries[index] = Some(RelocatedSource {
                owner: index,
                scope: owner.scope,
                asset_id: owner.asset_id,
                original_source: owner.source.clone(),
                original_folder: owner.folder.clone(),
                revision: *revision,
                source,
            });
        }
    }
    let entries = entries
        .into_iter()
        .map(|e| e.ok_or_else(|| error("Collection owner is missing a relocation target")))
        .collect::<Result<Vec<_>>>()?;
    let mut project = plan.snapshot().project().clone();
    for entry in entries.iter().filter(|e| e.scope == SourceScope::Current) {
        let asset = &mut project.assets[plan.snapshot().owners[entry.owner].asset_index];
        if asset.id != entry.asset_id || asset.source != entry.original_source {
            return Err(error("Collection manifest source identity changed"));
        }
        asset.source = entry.source.clone();
    }
    project.validate().map_err(error)?;
    Ok((
        project,
        RelocationReceipt {
            project_id: plan.snapshot().project_id(),
            project_revision: plan.snapshot().project_revision(),
            folder: plan.project_folder().to_owned(),
            entries,
        },
    ))
}

fn saved_reference_paths(plan: &SourceCollectionPlan, root: &Path) -> Result<BTreeSet<String>> {
    let (project, _) = crate::inspection::load_manifest(root)?;
    let mut original = plan.snapshot().roots[0].session.clone();
    original.project = project;
    original.folder = Some(root.to_owned());
    let mut paths = BTreeSet::new();
    for asset in original
        .project
        .assets
        .iter()
        .filter(|a| a.midi_pattern.is_none())
    {
        let path = original.source_path(asset)?;
        let mut existing = path.as_path();
        let mut missing = Vec::new();
        loop {
            match fs::symlink_metadata(existing) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    missing.push(
                        existing
                            .file_name()
                            .ok_or_else(|| error("Invalid saved source path"))?,
                    );
                    existing = existing
                        .parent()
                        .ok_or_else(|| error("Invalid saved source path"))?;
                }
                Err(e) => return Err(error(e)),
            }
        }
        let mut resolved = fs::canonicalize(existing).map_err(error)?;
        for component in missing.iter().rev() {
            resolved.push(component);
        }
        if resolved.starts_with(root) {
            paths.insert(super::paths::relative(root, &resolved)?.to_lowercase());
        }
    }
    Ok(paths)
}

fn receipt_entries(
    snapshot: &SourceCollectionSnapshot,
    receipt: &RelocationReceipt,
) -> Result<Vec<crate::source_save::Relocation>> {
    receipt
        .entries
        .iter()
        .map(|entry| {
            let owner = &snapshot.owners[entry.owner];
            let session = &snapshot.roots[owner.root_index].session;
            Ok(crate::source_save::Relocation {
                asset_id: entry.asset_id,
                original: session
                    .source_path_unchecked(&session.project.assets[owner.asset_index])?,
                original_source: entry.original_source.clone(),
                audio: owner.prepared.clone(),
                source: entry.source.clone(),
            })
        })
        .collect()
}
