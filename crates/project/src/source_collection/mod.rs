//! Worker-side source collection planning and transactional execution.
use crate::{Result, Session, SessionHistory, error};
use daw_core::{Id, Project, Source};
use daw_media::{AudioData, SourceRevision};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
pub(crate) mod execute;
pub(crate) mod owned_fs;
mod paths;
mod preflight;
mod targets;
pub use execute::{
    CollectionOutcome, CollectionPhase, CollectionProgress, CollectionStatus, RelocatedSource,
    RelocationReceipt,
};
#[cfg(test)]
mod tests;

/// Indices follow bounded history storage order, not labels or track positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceScope {
    Current,
    HistoryCurrent,
    Pending,
    Undo(usize),
    Redo(usize),
}

#[derive(Clone)]
pub struct SourceOwner {
    pub scope: SourceScope,
    pub asset_id: Id,
    pub asset_name: String,
    pub source: Source,
    pub folder: Option<PathBuf>,
    root_index: usize,
    asset_index: usize,
    proposal: Id,
    prepared: Option<AudioData>,
}
impl SourceOwner {
    pub fn prepared_audio(&self) -> Option<&AudioData> {
        self.prepared.as_ref()
    }
}

#[derive(Clone)]
struct Root {
    scope: SourceScope,
    session: Session,
}

/// Capture once per job. The caller's revision identifies edits; stale checks ignore playhead.
/// Clones retain PCM and temporary-stem owners; they do not duplicate sample buffers.
#[derive(Clone)]
pub struct SourceCollectionSnapshot {
    operation_id: Id,
    project_revision: u64,
    roots: Vec<Root>,
    saved_metadata: Option<Project>,
    owners: Vec<SourceOwner>,
    embedded_references: u64,
}

impl Session {
    /// No file I/O. Run preflight on a worker, never while painting or in the audio callback.
    pub fn source_collection_snapshot(
        &self,
        history: Option<&SessionHistory>,
        project_revision: u64,
    ) -> Result<SourceCollectionSnapshot> {
        self.project.validate().map_err(error)?;
        let mut roots = vec![Root {
            scope: SourceScope::Current,
            session: self.clone(),
        }];
        if let Some(history) = history {
            if history.project_id() != self.project.project_id {
                return Err(error("Source history belongs to a different project"));
            }
            if history
                .saved_collection_metadata()
                .is_some_and(|p| p.project_id != self.project.project_id)
            {
                return Err(error(
                    "Saved source metadata belongs to a different project",
                ));
            }
            roots.extend(history.collection_roots().map(|(scope, session)| Root {
                scope,
                session: session.clone(),
            }));
        }
        let mut owners = Vec::new();
        let mut embedded_references = 0_u64;
        for (root_index, root) in roots.iter_mut().enumerate() {
            if root.session.project.project_id != self.project.project_id {
                return Err(error("Source history contains a different project"));
            }
            root.session.project.validate().map_err(error)?;
            for (asset_index, asset) in root.session.project.assets.iter().enumerate() {
                if asset.midi_pattern.is_some() {
                    embedded_references = embedded_references
                        .checked_add(1)
                        .ok_or_else(|| error("Source reference count overflow"))?;
                    continue;
                }
                owners.push(SourceOwner {
                    scope: root.scope,
                    asset_id: asset.id,
                    asset_name: asset.name.clone(),
                    source: asset.source.clone(),
                    folder: root.session.folder.clone(),
                    root_index,
                    asset_index,
                    proposal: Id::new_v4(),
                    prepared: root.session.audio.get(&asset.id).cloned(),
                });
            }
        }
        Ok(SourceCollectionSnapshot {
            operation_id: Id::new_v4(),
            project_revision,
            roots,
            saved_metadata: history
                .and_then(SessionHistory::saved_collection_metadata)
                .cloned(),
            owners,
            embedded_references,
        })
    }
}

impl SourceCollectionSnapshot {
    pub fn operation_id(&self) -> Id {
        self.operation_id
    }
    pub fn project_id(&self) -> Id {
        self.roots[0].session.project.project_id
    }
    pub fn project(&self) -> &Project {
        &self.roots[0].session.project
    }
    pub fn project_revision(&self) -> u64 {
        self.project_revision
    }
    pub fn owners(&self) -> &[SourceOwner] {
        &self.owners
    }
    /// Comparison metadata only. It is not an extra audio/file ownership root.
    pub fn saved_metadata(&self) -> Option<&Project> {
        self.saved_metadata.as_ref()
    }

    /// In-memory stale-job check. Files must be rechecked separately before copying/commit.
    pub fn is_current(
        &self,
        live: &Session,
        history: Option<&SessionHistory>,
        revision: u64,
    ) -> bool {
        if self.project_revision != revision {
            return false;
        }
        if self.saved_metadata.as_ref()
            != history.and_then(SessionHistory::saved_collection_metadata)
        {
            return false;
        }
        let mut current = live.project.clone();
        current.transport.playhead_frame = self.project().transport.playhead_frame;
        let mut actual = std::iter::once((SourceScope::Current, live)).chain(
            history
                .into_iter()
                .flat_map(SessionHistory::collection_roots),
        );
        for expected in &self.roots {
            let Some((scope, session)) = actual.next() else {
                return false;
            };
            let actual_project = if scope == SourceScope::Current {
                &current
            } else {
                &session.project
            };
            if expected.scope != scope
                || expected.session.folder != session.folder
                || &expected.session.project != actual_project
            {
                return false;
            }
            for asset in expected
                .session
                .project
                .assets
                .iter()
                .filter(|a| a.midi_pattern.is_none())
            {
                match (
                    expected.session.audio.get(&asset.id),
                    session.audio.get(&asset.id),
                ) {
                    (None, None) => {}
                    (Some(a), Some(b))
                        if a.sample_rate_hz == b.sample_rate_hz
                            && a.metadata == b.metadata
                            && Arc::ptr_eq(&a.samples, &b.samples) => {}
                    _ => return false,
                }
            }
        }
        actual.next().is_none()
    }

    pub fn preflight(
        &self,
        available_bytes: Option<u64>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<CollectionPreflight> {
        preflight::run(self, available_bytes, cancelled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceIssue {
    InvalidPath,
    MissingSource,
    UnreadableSource,
    NotRegularFile,
    MissingPreparedAudio,
    PreparedAudioMismatch,
    RevisionConflict,
    VerificationFailed,
    UnsafeDestination,
    DestinationOccupied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionDiagnostic {
    pub issue: SourceIssue,
    pub path: PathBuf,
    /// Indices into the snapshot's owners(), retaining scope, asset identity and original path.
    pub owners: Vec<usize>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionTargetKind {
    IncludedRecording,
    GeneratedStem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionTarget {
    pub relative_path: String,
    pub kind: CollectionTargetKind,
    pub needs_copy: bool,
    pub owners: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionSource {
    pub path: PathBuf,
    pub revision: SourceRevision,
    pub owners: Vec<usize>,
    pub targets: Vec<CollectionTarget>,
}

/// Reference counts cover the full inventory. File/copy/byte totals cover verified
/// groups and are incomplete when diagnostics are present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectionSummary {
    pub audio_references: u64,
    pub embedded_references: u64,
    pub source_files: u64,
    pub duplicate_references: u64,
    pub files_to_copy: u64,
    pub files_already_included: u64,
    pub additional_bytes: u64,
    pub history_only_sources: u64,
    pub history_only_copy_bytes: u64,
    /// Advisory caller-provided filesystem estimate. None means not measured.
    pub available_bytes: Option<u64>,
}
impl CollectionSummary {
    /// Final encoded media only; staging/manifest/filesystem overhead is excluded.
    /// This advisory result is not an admission check or a disk-space reservation.
    pub fn space_appears_sufficient(&self) -> Option<bool> {
        self.available_bytes.map(|n| n >= self.additional_bytes)
    }
}

pub struct CollectionPreflight {
    snapshot: SourceCollectionSnapshot,
    root: PathBuf,
    manifest_revision: SourceRevision,
    sources: Vec<CollectionSource>,
    diagnostics: Vec<CollectionDiagnostic>,
    summary: CollectionSummary,
}
impl CollectionPreflight {
    pub fn is_ready(&self) -> bool {
        self.diagnostics.is_empty()
    }
    pub fn snapshot(&self) -> &SourceCollectionSnapshot {
        &self.snapshot
    }
    pub fn diagnostics(&self) -> &[CollectionDiagnostic] {
        &self.diagnostics
    }
    pub fn verified_sources(&self) -> &[CollectionSource] {
        &self.sources
    }
    pub fn summary(&self) -> CollectionSummary {
        self.summary
    }
    /// A blocked preflight can never become an executable plan.
    pub fn into_plan(self) -> Result<SourceCollectionPlan> {
        if !self.is_ready() {
            return Err(error(
                "Source collection preflight has unresolved diagnostics",
            ));
        }
        Ok(SourceCollectionPlan { preflight: self })
    }
}

pub struct SourceCollectionPlan {
    preflight: CollectionPreflight,
}
impl SourceCollectionPlan {
    pub fn snapshot(&self) -> &SourceCollectionSnapshot {
        &self.preflight.snapshot
    }
    pub fn project_folder(&self) -> &Path {
        &self.preflight.root
    }
    pub fn manifest_revision(&self) -> SourceRevision {
        self.preflight.manifest_revision
    }
    /// Proposal only. The executor must reserve this directory exclusively.
    pub fn staging_directory(&self) -> PathBuf {
        self.preflight.root.join(format!(
            "assets/.collect-{}",
            self.snapshot().operation_id()
        ))
    }
    pub fn sources(&self) -> &[CollectionSource] {
        &self.preflight.sources
    }
    pub fn summary(&self) -> CollectionSummary {
        self.preflight.summary
    }
}

pub(crate) fn check_included_components(folder: &Path, path: &str) -> Result<()> {
    paths::ordinary(folder, Path::new(path), true)
}
