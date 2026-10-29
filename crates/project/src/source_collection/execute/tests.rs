use super::*;
use std::cell::Cell;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("daw-collection-execute-{}", Id::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn wav(&self, name: &str, value: f32) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut w = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..12000 {
            w.write_sample(value).unwrap();
            w.write_sample(-value).unwrap();
        }
        w.finalize().unwrap();
        path
    }
    fn session(&self, paths: &[&Path]) -> Session {
        let mut session = Session::default();
        for p in paths {
            session.import(p, None, 0).unwrap();
        }
        session.save(&self.0.join("project")).unwrap();
        session
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn plan(live: &Session, history: Option<&SessionHistory>) -> SourceCollectionPlan {
    live.source_collection_snapshot(history, 9)
        .unwrap()
        .preflight(None, &|| false)
        .unwrap()
        .into_plan()
        .unwrap()
}
fn collect(
    plan: &SourceCollectionPlan,
    live: &Session,
    history: Option<&SessionHistory>,
) -> CollectionOutcome {
    plan.execute(&|s| s.is_current(live, history, 9), &|| false, &mut |_| {
        Ok(())
    })
    .unwrap()
}
fn old_manifest(live: &Session) -> Vec<u8> {
    fs::read(live.folder.as_ref().unwrap().join("project.json")).unwrap()
}
fn assert_clean_failure(live: &Session, before: &[u8]) {
    let root = live.folder.as_ref().unwrap();
    assert_eq!(old_manifest(live), before);
    assert_eq!(fs::read_dir(root).unwrap().count(), 2); // Existing project.json and assets/ only.
    assert_eq!(fs::read_dir(root.join("assets")).unwrap().count(), 0);
}
fn assert_committed(outcome: &CollectionOutcome, live: &Session) {
    assert_eq!(outcome.status(), CollectionStatus::Committed);
    assert_eq!(
        &crate::inspect_project(live.folder.as_ref().unwrap()).unwrap(),
        outcome.candidate_project()
    );
    for entry in outcome.receipt().entries() {
        let path = live.folder.as_ref().unwrap().join(&entry.source.path);
        assert_eq!(
            SourceRevision::read(&path, &|| false).unwrap(),
            entry.revision
        );
    }
}

#[test]
fn complete_original_bytes_shared_sources_metadata_pcm_and_live_state_are_preserved() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let bytes = fs::read(&input).unwrap();
    let mut live = f.session(&[&input, &input]);
    live.project.transport.playhead_frame = 123;
    live.project.tracks[0].clips[0].source_offset_frame = 100;
    live.project.tracks[0].clips[0].length_frames = 300;
    let original = live.project.clone();
    let history = SessionHistory::new(&live, true).unwrap();
    let p = plan(&live, Some(&history));
    let mut events = Vec::new();
    let result = p
        .execute(
            &|s| s.is_current(&live, Some(&history), 9),
            &|| false,
            &mut |event| {
                events.push(event);
                Ok(())
            },
        )
        .unwrap();
    assert_committed(&result, &live);
    assert_eq!(live.project, original);
    assert_eq!(fs::read(&input).unwrap(), bytes);
    assert_eq!(result.candidate_project().transport.playhead_frame, 123);
    assert_eq!(result.progress().files_copied, 1);
    assert_eq!(result.progress().copied_bytes, bytes.len() as u64);
    assert_eq!(result.progress().staged_bytes, bytes.len() as u64);
    assert!(
        events
            .iter()
            .filter(|p| p.phase == CollectionPhase::Staging)
            .count()
            > 2
    );
    assert_eq!(events.last().unwrap().phase, CollectionPhase::Committed);
    let path = live
        .folder
        .as_ref()
        .unwrap()
        .join(&result.candidate_project().assets[0].source.path);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        result.candidate_project().assets[0].source.path,
        result.candidate_project().assets[1].source.path
    );
    let decoded = daw_media::decode_audio_at_rate(&path, 48000).unwrap();
    assert_eq!(
        *decoded.samples,
        *live.audio[&live.project.assets[0].id].samples
    );
    let mut restored = result.candidate_project().clone();
    for (asset, old) in restored.assets.iter_mut().zip(&original.assets) {
        asset.source = old.source.clone();
    }
    assert_eq!(restored, original);
    assert!(Session::open(live.folder.as_ref().unwrap()).is_err()); // Stage 3 gate remains.
    assert_eq!(
        result.receipt().entries().len(),
        p.snapshot().owners().len()
    );
    assert!(
        result
            .receipt()
            .entries()
            .iter()
            .any(|e| e.scope == SourceScope::HistoryCurrent)
    );
}

#[test]
fn compressed_sources_keep_full_bitstreams_and_container_extensions() {
    let f = Fixture::new();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media/tests/fixtures");
    let inputs = ["tone-2.mp3", "tone-2.aac", "tone-2.m4a"].map(|name| fixture.join(name));
    let live = f.session(&inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>());
    let result = collect(&plan(&live, None), &live, None);
    assert_committed(&result, &live);
    for ((asset, input), extension) in result
        .candidate_project()
        .assets
        .iter()
        .zip(&inputs)
        .zip(["mp3", "aac", "mp4"])
    {
        assert!(asset.source.path.ends_with(extension));
        assert_eq!(
            fs::read(live.folder.as_ref().unwrap().join(&asset.source.path)).unwrap(),
            fs::read(input).unwrap()
        );
    }
}

#[test]
fn history_only_media_has_a_receipt_but_is_not_resurrected_in_the_manifest() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let mut live = f.session(&[&input]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.assets.clear();
    live.project.tracks.clear();
    live.audio.clear();
    history.record(&live).unwrap();
    let p = plan(&live, Some(&history));
    let result = collect(&p, &live, Some(&history));
    assert_committed(&result, &live);
    assert!(result.candidate_project().assets.is_empty());
    assert!(!result.receipt().entries().is_empty());
    assert!(
        result
            .receipt()
            .entries()
            .iter()
            .all(|e| e.scope != SourceScope::Current)
    );
    assert!(history.can_undo());
    assert_eq!(live.project.assets.len(), 0);
}

#[test]
fn mixed_provenance_targets_are_independent_regular_copies() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let mut live = f.session(&[&input, &input]);
    live.project.assets[1].source.kind = "generated".into();
    let result = collect(&plan(&live, None), &live, None);
    assert_committed(&result, &live);
    assert_eq!(result.progress().files_copied, 2);
    let paths = result
        .candidate_project()
        .assets
        .iter()
        .map(|a| live.folder.as_ref().unwrap().join(&a.source.path))
        .collect::<Vec<_>>();
    assert_ne!(paths[0], paths[1]);
    fs::write(&paths[0], b"changed one copy").unwrap();
    assert_eq!(fs::read(&paths[1]).unwrap(), fs::read(&input).unwrap());
}

#[test]
fn every_precommit_io_failure_rolls_back_only_new_entries() {
    for point in [
        IoStep::CreateDirectory,
        IoStep::ReserveFile,
        IoStep::Read,
        IoStep::Write,
        IoStep::Flush,
        IoStep::FileSync,
        IoStep::DirectorySync,
        IoStep::BeforeCommit,
        IoStep::Replace,
    ] {
        let f = Fixture::new();
        let input = f.wav("input.wav", 0.2);
        let live = f.session(&[&input]);
        let before = old_manifest(&live);
        let p = plan(&live, None);
        let mut hit = false;
        let result = execute_with(
            &p,
            &|s| s.is_current(&live, None, 9),
            &|| false,
            &mut |_| Ok(()),
            &mut |step, _| {
                if !hit && step == point {
                    hit = true;
                    Err(io::Error::other("injected I/O failure"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(hit, "{point:?}");
        assert!(result.is_err(), "{point:?}");
        assert_clean_failure(&live, &before);
    }
}

#[test]
fn disk_full_during_final_media_and_manifest_sync_failure_keep_the_previous_project() {
    for manifest_sync in [false, true] {
        let f = Fixture::new();
        let input = f.wav("input.wav", 0.2);
        let live = f.session(&[&input]);
        let before = old_manifest(&live);
        let p = plan(&live, None);
        let mut hit = false;
        let result = execute_with(
            &p,
            &|_| true,
            &|| false,
            &mut |_| Ok(()),
            &mut |step, path| {
                let manifest = path.extension().is_some_and(|e| e == "tmp");
                let final_media = path.parent().is_some_and(|p| p.ends_with("sources"));
                if (manifest_sync && manifest && step == IoStep::FileSync)
                    || (!manifest_sync && final_media && step == IoStep::Write)
                {
                    hit = true;
                    Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "injected disk full",
                    ))
                } else {
                    Ok(())
                }
            },
        );
        assert!(hit);
        assert!(result.is_err());
        assert_clean_failure(&live, &before);
    }
}

#[test]
fn cancellation_and_state_invalidation_before_commit_preserve_session_and_files() {
    for phase in [
        CollectionPhase::Validating,
        CollectionPhase::Staging,
        CollectionPhase::Verifying,
        CollectionPhase::Publishing,
    ] {
        for stale in [false, true] {
            let f = Fixture::new();
            let input = f.wav("input.wav", 0.2);
            let live = f.session(&[&input]);
            let before = old_manifest(&live);
            let p = plan(&live, None);
            let flag = Cell::new(false);
            let result = p.execute(
                &|s| s.is_current(&live, None, 9) && (!stale || !flag.get()),
                &|| !stale && flag.get(),
                &mut |event| {
                    if event.phase == phase {
                        flag.set(true);
                    }
                    Ok(())
                },
            );
            assert!(flag.get());
            assert!(result.is_err(), "{phase:?} stale={stale}");
            assert_clean_failure(&live, &before);
        }
    }
}

#[test]
fn source_mutation_during_copy_and_before_commit_is_rejected() {
    for mutation_step in [IoStep::Read, IoStep::BeforeCommit, IoStep::DirectorySync] {
        let f = Fixture::new();
        let input = f.wav("input.wav", 0.2);
        let live = f.session(&[&input]);
        let before = old_manifest(&live);
        let p = plan(&live, None);
        let mut hit = false;
        let result = execute_with(
            &p,
            &|_| true,
            &|| false,
            &mut |_| Ok(()),
            &mut |step, path| {
                if !hit
                    && step == mutation_step
                    && (step != IoStep::Read || path.file_name() == input.file_name())
                {
                    hit = true;
                    f.wav("input.wav", 0.8);
                }
                Ok(())
            },
        );
        assert!(hit);
        assert!(result.is_err());
        assert_clean_failure(&live, &before);
        assert_eq!(live.audio.values().next().unwrap().samples[0], [0.2, -0.2]);
    }
}

#[test]
fn changed_manifest_and_wrong_job_revision_are_not_overwritten() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let before = old_manifest(&live);
    assert!(
        p.execute(&|s| s.is_current(&live, None, 10), &|| false, &mut |_| Ok(
            ()
        ))
        .is_err()
    );
    assert_clean_failure(&live, &before);
    let mut other = live.project.clone();
    other.name = "External edit".into();
    let bytes = serde_json::to_vec_pretty(&other).unwrap();
    let result = execute_with(&p, &|_| true, &|| false, &mut |_| Ok(()), &mut |step, _| {
        if step == IoStep::BeforeCommit {
            fs::write(live.folder.as_ref().unwrap().join("project.json"), &bytes).unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_clean_failure(&live, &bytes);
}

#[test]
fn races_with_occupied_names_retry_without_overwriting_or_reusing_unrelated_entries() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let proposal = live
        .folder
        .as_ref()
        .unwrap()
        .join(&p.sources()[0].targets[0].relative_path);
    fs::create_dir_all(proposal.parent().unwrap()).unwrap();
    fs::write(&proposal, b"unrelated").unwrap();
    let stage = p.staging_directory();
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("keep"), b"unrelated stage").unwrap();
    let result = collect(&p, &live, None);
    assert_committed(&result, &live);
    assert_ne!(
        live.folder
            .as_ref()
            .unwrap()
            .join(&result.candidate_project().assets[0].source.path),
        proposal
    );
    assert_eq!(fs::read(&proposal).unwrap(), b"unrelated");
    assert_eq!(fs::read(stage.join("keep")).unwrap(), b"unrelated stage");
}

#[test]
fn generated_siblings_retry_the_group_directory_and_keep_provenance() {
    let f = Fixture::new();
    let a = f.wav("stems-input/left.wav", 0.2);
    let b = f.wav("stems-input/right.wav", 0.3);
    let mut live = f.session(&[&a, &b]);
    for asset in &mut live.project.assets {
        asset.source.kind = "generated".into();
    }
    let p = plan(&live, None);
    let group = Path::new(&p.sources()[0].targets[0].relative_path)
        .parent()
        .unwrap();
    let occupied = live.folder.as_ref().unwrap().join(group);
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join("keep"), b"unrelated").unwrap();
    let result = collect(&p, &live, None);
    assert_committed(&result, &live);
    let paths = result
        .candidate_project()
        .assets
        .iter()
        .map(|a| {
            assert_eq!(a.source.kind, "generated");
            Path::new(&a.source.path).parent().unwrap().to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(paths[0], paths[1]);
    assert_ne!(paths[0], group);
    assert_eq!(fs::read(occupied.join("keep")).unwrap(), b"unrelated");
}

#[test]
fn replaced_owned_file_is_preserved_by_identity_checked_rollback() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let before = old_manifest(&live);
    let target = live
        .folder
        .as_ref()
        .unwrap()
        .join(&p.sources()[0].targets[0].relative_path);
    let result = execute_with(&p, &|_| true, &|| false, &mut |_| Ok(()), &mut |step, _| {
        if step == IoStep::BeforeCommit {
            fs::remove_file(&target).unwrap();
            fs::write(&target, b"foreign replacement").unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(old_manifest(&live), before);
    assert_eq!(fs::read(target).unwrap(), b"foreign replacement");
}

#[test]
fn cleanup_never_recurses_into_unrelated_staging_contents() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let before = old_manifest(&live);
    let unrelated = p.staging_directory().join("unrelated");
    let result = execute_with(&p, &|_| true, &|| false, &mut |_| Ok(()), &mut |step, _| {
        if step == IoStep::BeforeCommit {
            fs::write(&unrelated, b"keep").unwrap();
            return Err(io::Error::other("stop"));
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(old_manifest(&live), before);
    assert_eq!(fs::read(unrelated).unwrap(), b"keep");
}

#[test]
fn postcommit_notifications_directory_sync_and_cancellation_never_roll_back_media() {
    for after_sync in [false, true] {
        let f = Fixture::new();
        let input = f.wav("input.wav", 0.2);
        let live = f.session(&[&input]);
        let p = plan(&live, None);
        let committed = Cell::new(false);
        let cancelled = Cell::new(false);
        let result = execute_with(
            &p,
            &|_| true,
            &|| cancelled.get(),
            &mut |event| {
                if event.phase == CollectionPhase::Committed {
                    cancelled.set(true);
                    return Err(error("progress receiver closed"));
                }
                Ok(())
            },
            &mut |step, _| {
                if step == IoStep::AfterReplace {
                    committed.set(true);
                    if !after_sync {
                        return Err(io::Error::other("notification failed"));
                    }
                }
                if after_sync && committed.get() && step == IoStep::DirectorySync {
                    return Err(io::Error::other("directory sync failed"));
                }
                Ok(())
            },
        )
        .unwrap();
        assert_committed(&result, &live);
        assert!(!result.warnings().is_empty());
        assert!(cancelled.get());
        assert!(
            result
                .warnings()
                .iter()
                .any(|w| w.contains("progress delivery"))
        );
    }
}

#[test]
fn replacement_error_after_real_commit_returns_a_committed_receipt() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let result = execute_with(
        &p,
        &|_| true,
        &|| false,
        &mut |_| Ok(()),
        &mut |step, path| {
            if step == IoStep::Replace {
                crate::file_publish::replace(
                    path,
                    &live.folder.as_ref().unwrap().join("project.json"),
                )?;
                return Err(io::Error::other("reply lost"));
            }
            Ok(())
        },
    )
    .unwrap();
    assert_committed(&result, &live);
    assert!(
        result
            .warnings()
            .iter()
            .any(|w| w.contains("committed bytes were confirmed"))
    );
}

#[test]
fn unconfirmed_replacement_retains_candidate_and_all_media_for_recovery() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let p = plan(&live, None);
    let result = execute_with(&p, &|_| true, &|| false, &mut |_| Ok(()), &mut |step, _| {
        if step == IoStep::Replace {
            fs::remove_file(live.folder.as_ref().unwrap().join("project.json"))?;
            return Err(io::Error::other("unconfirmed remote operation"));
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(result.status(), CollectionStatus::RecoveryRequired);
    assert!(result.recovery_manifest().unwrap().is_file());
    let candidate: Project =
        serde_json::from_slice(&fs::read(result.recovery_manifest().unwrap()).unwrap()).unwrap();
    assert_eq!(&candidate, result.candidate_project());
    for entry in result.receipt().entries() {
        assert!(
            live.folder
                .as_ref()
                .unwrap()
                .join(&entry.source.path)
                .is_file()
        );
    }
    assert!(input.is_file());
}

#[test]
fn already_included_media_is_verified_and_reused_without_another_final_copy() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let mut live = f.session(&[&input]);
    let first = collect(&plan(&live, None), &live, None);
    live.project = first.candidate_project().clone();
    drop(first);
    let p = plan(&live, None);
    assert_eq!(p.summary().files_to_copy, 0);
    let before = fs::read(
        live.folder
            .as_ref()
            .unwrap()
            .join(&live.project.assets[0].source.path),
    )
    .unwrap();
    let result = collect(&p, &live, None);
    assert_committed(&result, &live);
    assert_eq!(result.progress().files_copied, 0);
    assert_eq!(result.candidate_project(), &live.project);
    assert_eq!(
        fs::read(
            live.folder
                .as_ref()
                .unwrap()
                .join(&live.project.assets[0].source.path)
        )
        .unwrap(),
        before
    );
}

#[test]
fn peak_space_overflow_aborts_before_creating_files() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let before = old_manifest(&live);
    let mut p = plan(&live, None);
    p.preflight.summary.additional_bytes = u64::MAX;
    assert!(p.execute(&|_| true, &|| false, &mut |_| Ok(())).is_err());
    assert_clean_failure(&live, &before);
}

#[test]
fn a_missing_saved_reference_is_not_filled_before_commit() {
    let f = Fixture::new();
    let input = f.wav("input.wav", 0.2);
    let live = f.session(&[&input]);
    let root = live.folder.as_ref().unwrap();
    let relative = format!("assets/sources/{}.wav", Id::new_v4());
    let protected = root.join(&relative);
    let mut saved = live.project.clone();
    saved.assets[0].source.path = protected.to_str().unwrap().into();
    saved.assets[0].source.path_kind = "absolute".into();
    fs::write(
        root.join("project.json"),
        serde_json::to_vec_pretty(&saved).unwrap(),
    )
    .unwrap();
    let mut p = plan(&live, None);
    // Force the rare UUID collision with an absent path in the saved model.
    p.preflight.sources[0].targets[0].relative_path = relative;
    let result = execute_with(&p, &|_| true, &|| false, &mut |_| Ok(()), &mut |step, _| {
        if step == IoStep::BeforeCommit {
            assert!(!protected.exists());
        }
        Ok(())
    })
    .unwrap();
    assert_committed(&result, &live);
    assert!(!protected.exists());
}
