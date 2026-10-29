use super::*;
use std::{collections::BTreeMap, fs};

pub(super) fn cancelled(check: &impl Fn() -> bool) -> Result<()> {
    if check() {
        Err(error("Source collection preflight cancelled"))
    } else {
        Ok(())
    }
}

fn add(total: &mut u64, amount: u64) -> Result<()> {
    *total = total
        .checked_add(amount)
        .ok_or_else(|| error("Source collection totals overflow"))?;
    Ok(())
}

pub(super) fn issue(
    diagnostics: &mut Vec<CollectionDiagnostic>,
    kind: SourceIssue,
    path: &Path,
    owners: &[usize],
    message: impl std::fmt::Display,
) {
    diagnostics.push(CollectionDiagnostic {
        issue: kind,
        path: path.to_path_buf(),
        owners: owners.to_vec(),
        message: message.to_string(),
    });
}

pub(super) fn run(
    snapshot: &SourceCollectionSnapshot,
    available_bytes: Option<u64>,
    check: &impl Fn() -> bool,
) -> Result<CollectionPreflight> {
    cancelled(check)?;
    let live = &snapshot.roots[0].session;
    let root = fs::canonicalize(
        live.folder
            .as_deref()
            .ok_or_else(|| error("Save the project before source collection preflight"))?,
    )
    .map_err(error)?;
    let manifest = root.join("project.json");
    let manifest_revision = SourceRevision::read(&manifest, check).map_err(error)?;
    let (saved, _) = crate::inspection::load_manifest(&root)?;
    if saved.project_id != snapshot.project_id()
        || SourceRevision::read(&manifest, check).map_err(error)? != manifest_revision
    {
        return Err(error(
            "Saved project changed or belongs to a different project",
        ));
    }
    let mut diagnostics = Vec::new();
    let mut planner = super::targets::TargetPlanner::new(&root, snapshot);
    // The whole output tree must be safe even when all sources are already included.
    for directory in ["assets", "assets/sources", "stems"] {
        if let Err(e) = planner
            .tree
            .check(&root, Path::new(directory), false, check)
        {
            cancelled(check)?;
            issue(
                &mut diagnostics,
                SourceIssue::UnsafeDestination,
                &root.join(directory),
                &[],
                e,
            );
        }
    }
    let staging = format!("assets/.collect-{}", snapshot.operation_id);
    let staging_path = root.join(&staging);
    if let Err(e) = planner.tree.check(&root, Path::new(&staging), false, check) {
        cancelled(check)?;
        issue(
            &mut diagnostics,
            SourceIssue::UnsafeDestination,
            &staging_path,
            &[],
            e,
        );
    } else if fs::symlink_metadata(&staging_path).is_ok() {
        issue(
            &mut diagnostics,
            SourceIssue::DestinationOccupied,
            &staging_path,
            &[],
            "Operation directory is occupied; capture a fresh snapshot instead of reusing it",
        );
    }
    let mut grouped = BTreeMap::<PathBuf, Vec<usize>>::new();
    for (index, owner) in snapshot.owners.iter().enumerate() {
        cancelled(check)?;
        // Both indices were captured from these private, immutable root vectors.
        let session = &snapshot.roots[owner.root_index].session;
        let asset = &session.project.assets[owner.asset_index];
        let path = match session.source_path(asset) {
            Ok(path) => path,
            Err(e) => {
                issue(
                    &mut diagnostics,
                    SourceIssue::InvalidPath,
                    Path::new(&owner.source.path),
                    &[index],
                    e,
                );
                continue;
            }
        };
        if owner.source.kind == "generated" {
            let stored_root = crate::absolute(live.folder.as_deref().unwrap())?;
            let relative = path
                .strip_prefix(&root)
                .or_else(|_| path.strip_prefix(&stored_root));
            if let Ok(relative) = relative
                && relative.starts_with("stems")
                && let Err(e) = planner.tree.check(&root, relative, true, check)
            {
                cancelled(check)?;
                issue(
                    &mut diagnostics,
                    SourceIssue::UnsafeDestination,
                    &path,
                    &[index],
                    e,
                );
                continue;
            }
        }
        if owner.source.is_included_recording() {
            let owner_root = fs::canonicalize(
                owner
                    .folder
                    .as_deref()
                    .ok_or_else(|| error("Included source requires its owning project folder"))?,
            )
            .map_err(error)?;
            if let Err(e) =
                planner
                    .tree
                    .check(&owner_root, Path::new(&owner.source.path), true, check)
            {
                cancelled(check)?;
                issue(
                    &mut diagnostics,
                    SourceIssue::UnsafeDestination,
                    &path,
                    &[index],
                    e,
                );
                continue;
            }
        }
        match fs::canonicalize(&path)
            .and_then(|path| fs::metadata(&path).map(|metadata| (path, metadata)))
        {
            Ok((path, metadata)) if metadata.is_file() => {
                grouped.entry(path).or_default().push(index)
            }
            Ok(_) => issue(
                &mut diagnostics,
                SourceIssue::NotRegularFile,
                &path,
                &[index],
                "Source must be a regular file",
            ),
            Err(e) => issue(
                &mut diagnostics,
                if e.kind() == std::io::ErrorKind::NotFound {
                    SourceIssue::MissingSource
                } else {
                    SourceIssue::UnreadableSource
                },
                &path,
                &[index],
                e,
            ),
        }
    }
    let mut sources = Vec::new();
    for (path, owners) in grouped {
        cancelled(check)?;
        let start = diagnostics.len();
        let mut revision = None;
        let mut matching = false;
        let mut mismatching = false;
        // Verify each distinct PCM/rate/metadata owner once; keep all reference identities.
        let mut verified: Vec<(&AudioData, daw_media::PreparedSourceCheck)> = Vec::new();
        for &index in &owners {
            cancelled(check)?;
            let Some(prepared) = &snapshot.owners[index].prepared else {
                issue(
                    &mut diagnostics,
                    SourceIssue::MissingPreparedAudio,
                    &path,
                    &[index],
                    "Original prepared audio is unavailable; reload or restore the source before collection",
                );
                continue;
            };
            let owner = &snapshot.owners[index];
            if prepared.sample_rate_hz
                != snapshot.roots[owner.root_index]
                    .session
                    .project
                    .sample_rate_hz
            {
                issue(
                    &mut diagnostics,
                    SourceIssue::VerificationFailed,
                    &path,
                    &[index],
                    "Prepared audio rate does not match its owning project snapshot",
                );
                continue;
            }
            let prior = verified.iter().find(|(audio, _)| {
                audio.sample_rate_hz == prepared.sample_rate_hz
                    && audio.metadata == prepared.metadata
                    && Arc::ptr_eq(&audio.samples, &prepared.samples)
            });
            let result = if let Some((_, result)) = prior {
                *result
            } else {
                match daw_media::check_prepared_source(&path, prepared, check) {
                    Ok(result) => {
                        verified.push((prepared, result));
                        result
                    }
                    Err(e) => {
                        cancelled(check)?;
                        issue(
                            &mut diagnostics,
                            SourceIssue::VerificationFailed,
                            &path,
                            &[index],
                            e,
                        );
                        continue;
                    }
                }
            };
            if revision.is_some_and(|previous| previous != result.revision) {
                issue(
                    &mut diagnostics,
                    SourceIssue::RevisionConflict,
                    &path,
                    &owners,
                    "Source bytes changed between retained owners; retry with the required originals available",
                );
            }
            revision = Some(result.revision);
            if result.matches_audio {
                matching = true;
            } else {
                mismatching = true;
                issue(
                    &mut diagnostics,
                    SourceIssue::PreparedAudioMismatch,
                    &path,
                    &[index],
                    "Source differs from this owner's prepared audio; restore the required original recording",
                );
            }
        }
        if matching && mismatching {
            issue(
                &mut diagnostics,
                SourceIssue::RevisionConflict,
                &path,
                &owners,
                "One source path cannot satisfy the current and retained audio versions",
            );
        }
        if diagnostics.len() != start {
            continue;
        }
        let Some(revision) = revision else {
            continue;
        };
        let metadata = &snapshot.owners[owners[0]]
            .prepared
            .as_ref()
            .unwrap()
            .metadata;
        let extension = match daw_media::source_file_extension(&path, metadata) {
            Ok(extension) => extension,
            Err(e) => {
                issue(
                    &mut diagnostics,
                    SourceIssue::VerificationFailed,
                    &path,
                    &owners,
                    e,
                );
                continue;
            }
        };
        let targets = planner.plan(&path, &owners, extension, &mut diagnostics, check)?;
        cancelled(check)?;
        match SourceRevision::read(&path, check) {
            Ok(current) if current == revision => {}
            Ok(_) => issue(
                &mut diagnostics,
                SourceIssue::RevisionConflict,
                &path,
                &owners,
                "Source changed while its destinations were planned",
            ),
            Err(e) => {
                cancelled(check)?;
                issue(
                    &mut diagnostics,
                    SourceIssue::VerificationFailed,
                    &path,
                    &owners,
                    e,
                );
            }
        }
        if diagnostics.len() == start {
            sources.push(CollectionSource {
                path,
                revision,
                owners,
                targets,
            });
        }
    }
    let mut summary = CollectionSummary {
        audio_references: u64::try_from(snapshot.owners.len()).map_err(error)?,
        embedded_references: snapshot.embedded_references,
        available_bytes,
        ..Default::default()
    };
    for source in &sources {
        add(&mut summary.source_files, 1)?;
        add(
            &mut summary.duplicate_references,
            u64::try_from(source.owners.len() - 1).map_err(error)?,
        )?;
        let history_only = !source
            .owners
            .iter()
            .any(|&i| snapshot.owners[i].scope == SourceScope::Current);
        if history_only {
            add(&mut summary.history_only_sources, 1)?;
        }
        for target in &source.targets {
            if target.needs_copy {
                add(&mut summary.files_to_copy, 1)?;
                add(&mut summary.additional_bytes, source.revision.byte_length)?;
                if !target
                    .owners
                    .iter()
                    .any(|&i| snapshot.owners[i].scope == SourceScope::Current)
                {
                    add(
                        &mut summary.history_only_copy_bytes,
                        source.revision.byte_length,
                    )?;
                }
            } else {
                add(&mut summary.files_already_included, 1)?;
            }
        }
    }
    // Stable order, including diagnostics collected outside the canonical-path groups.
    diagnostics.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.owners.cmp(&b.owners))
            .then(a.message.cmp(&b.message))
    });
    diagnostics.dedup();
    cancelled(check)?;
    Ok(CollectionPreflight {
        snapshot: snapshot.clone(),
        root,
        manifest_revision,
        sources,
        diagnostics,
        summary,
    })
}

#[cfg(test)]
mod counter_tests {
    #[test]
    fn checked_totals_reject_overflow_without_wrapping() {
        let mut total = u64::MAX;
        assert!(super::add(&mut total, 1).is_err());
        assert_eq!(total, u64::MAX);
    }
}
