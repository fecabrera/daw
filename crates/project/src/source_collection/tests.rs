use super::*;
use std::{cell::Cell, collections::BTreeSet, fs};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-collection-preflight-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn wav(&self, relative: &str, sample: f32) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..480 {
            writer.write_sample(sample).unwrap();
            writer.write_sample(-sample).unwrap();
        }
        writer.finalize().unwrap();
        path
    }
    fn saved(&self, paths: &[&Path]) -> Session {
        let mut session = Session::default();
        for path in paths {
            session.import(path, None, 0).unwrap();
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
fn preflight(session: &Session, history: Option<&SessionHistory>) -> CollectionPreflight {
    session
        .source_collection_snapshot(history, 7)
        .unwrap()
        .preflight(None, &|| false)
        .unwrap()
}
fn tree(path: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn visit(root: &Path, path: &Path, output: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for p in entries {
            let directory = p.is_dir();
            output.push((
                p.strip_prefix(root).unwrap().to_owned(),
                if directory {
                    None
                } else {
                    Some(fs::read(&p).unwrap())
                },
            ));
            if directory {
                visit(root, &p, output);
            }
        }
    }
    let mut output = Vec::new();
    visit(path, path, &mut output);
    output
}

#[test]
fn repeated_read_only_preflight_deduplicates_paths_and_uses_actual_container() {
    let f = Fixture::new();
    let a = f.wav("first/take.mp3", 0.2); // WAV bytes with a misleading filename.
    let b = f.wav("second/take.mp3", 0.2); // Identical bytes at an unrelated path stay distinct.
    let mut session = f.saved(&[&a, &a, &b]);
    session.project.tracks[2].clips.clear(); // Unused assets remain required.
    let snapshot = session.source_collection_snapshot(None, 10).unwrap();
    let before = tree(&f.0);
    let model = session.project.clone();
    let first = snapshot.preflight(Some(0), &|| false).unwrap();
    let second = snapshot.preflight(Some(0), &|| false).unwrap();
    assert!(first.is_ready());
    assert_eq!(first.summary(), second.summary());
    assert_eq!(first.verified_sources(), second.verified_sources());
    assert_eq!(first.summary().audio_references, 3);
    assert_eq!(first.summary().source_files, 2);
    assert_eq!(
        first.verified_sources()[0].revision,
        first.verified_sources()[1].revision
    );
    assert_eq!(first.summary().files_to_copy, 2);
    assert_eq!(
        first.summary().additional_bytes,
        fs::metadata(&a).unwrap().len() + fs::metadata(&b).unwrap().len()
    );
    assert_eq!(first.summary().space_appears_sufficient(), Some(false)); // Advisory, not a reservation.
    let targets = first
        .verified_sources()
        .iter()
        .flat_map(|s| &s.targets)
        .map(|t| t.relative_path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .all(|s| s.starts_with("assets/sources/") && s.ends_with(".wav"))
    );
    let plan = first.into_plan().unwrap();
    assert_eq!(plan.snapshot().project_revision(), 10);
    assert_eq!(
        plan.project_folder(),
        fs::canonicalize(session.folder.as_ref().unwrap()).unwrap()
    );
    assert_eq!(tree(&f.0), before);
    assert_eq!(session.project, model);
    let owner = &snapshot.owners()[0];
    assert!(Arc::ptr_eq(
        &owner.prepared_audio().unwrap().samples,
        &session.audio[&owner.asset_id].samples
    ));
}

#[test]
fn sampler_granulizer_and_embedded_midi_inventory_does_not_depend_on_timeline_clips() {
    let f = Fixture::new();
    let a = f.wav("sampler.wav", 0.2);
    let b = f.wav("granular.wav", 0.3);
    let mut session = Session::default();
    for (path, granular) in [(&a, false), (&b, true)] {
        let track = session.project.add_midi_track().unwrap();
        let layer = &mut session
            .project
            .tracks
            .last_mut()
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        let id = layer.id;
        layer.sampler = Some(if granular {
            daw_core::SamplerSettings::granular()
        } else {
            daw_core::SamplerSettings::default()
        });
        session = session.with_sampler_file(track, id, path).unwrap();
    }
    let track = session.project.tracks[0].id;
    session.project.add_midi_pattern(track, 0).unwrap();
    session.project.tracks[0].clips.clear();
    session.save(&f.0.join("project")).unwrap();
    let report = preflight(&session, None);
    assert!(report.is_ready());
    assert_eq!(report.summary().audio_references, 2);
    assert_eq!(report.summary().embedded_references, 1);
    assert_eq!(report.summary().source_files, 2);
    assert!(session.project.tracks.iter().all(|t| t.clips.is_empty()));
}

#[test]
fn current_pending_undo_and_redo_sources_keep_scope_and_shared_audio_owners() {
    let f = Fixture::new();
    let a = f.wav("a.wav", 0.1);
    let b = f.wav("b.wav", 0.2);
    let c = f.wav("c.wav", 0.3);
    let mut live = f.saved(&[&a]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.name = "Edited".into();
    history.record(&live).unwrap();
    live.import(&b, None, 0).unwrap();
    history.record(&live).unwrap();
    live = history.undo(&live).unwrap();
    history.begin_group();
    live.import(&c, None, 0).unwrap();
    history.record(&live).unwrap();
    let snapshot = live.source_collection_snapshot(Some(&history), 9).unwrap();
    let scopes = snapshot
        .owners()
        .iter()
        .map(|o| o.scope)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        scopes,
        BTreeSet::from([
            SourceScope::Current,
            SourceScope::HistoryCurrent,
            SourceScope::Pending,
            SourceScope::Undo(0),
            SourceScope::Redo(0)
        ])
    );
    let report = snapshot.preflight(None, &|| false).unwrap();
    assert!(report.is_ready());
    assert_eq!(report.summary().source_files, 3);
    assert_eq!(report.summary().history_only_sources, 1);
    assert_eq!(
        report.summary().history_only_copy_bytes,
        fs::metadata(&b).unwrap().len()
    );
    let a_owner = snapshot
        .owners()
        .iter()
        .find(|o| o.scope == SourceScope::Current && o.asset_name == "a.wav")
        .unwrap();
    let a_live = &live.audio[&a_owner.asset_id];
    assert!(Arc::ptr_eq(
        &a_owner.prepared_audio().unwrap().samples,
        &a_live.samples
    ));
    let retained = Arc::downgrade(&a_live.samples);
    drop(report);
    drop(live);
    drop(history);
    assert!(retained.upgrade().is_some());
    drop(snapshot);
    assert!(retained.upgrade().is_none());
}

#[test]
fn saved_comparison_metadata_is_not_a_restorable_media_root() {
    let f = Fixture::new();
    let source = f.wav("old.wav", 0.2);
    let mut live = f.saved(&[&source]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.assets.clear();
    live.project.tracks.clear();
    live.audio.clear();
    history.record(&live).unwrap();
    let protected = preflight(&live, Some(&history));
    assert_eq!(protected.summary().history_only_sources, 1);
    drop(protected);
    history.release_optional_memory();
    fs::remove_file(&source).unwrap();
    let empty = preflight(&live, Some(&history));
    assert!(empty.is_ready());
    assert_eq!(empty.summary().source_files, 0);
    assert_eq!(empty.snapshot().saved_metadata().unwrap().assets.len(), 1);
}

#[test]
fn stale_state_checks_ignore_playhead_but_cover_edits_history_revision_and_audio_identity() {
    let f = Fixture::new();
    let path = f.wav("a.wav", 0.2);
    let mut live = f.saved(&[&path]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.transport.playhead_frame = 123;
    let snapshot = live.source_collection_snapshot(Some(&history), 4).unwrap();
    assert_eq!(snapshot.project().transport.playhead_frame, 123);
    live.project.transport.playhead_frame = 200;
    assert!(snapshot.is_current(&live, Some(&history), 4));
    assert!(!snapshot.is_current(&live, Some(&history), 5));
    live.project.name = "Changed".into();
    assert!(!snapshot.is_current(&live, Some(&history), 4));
    live.project.name = snapshot.roots[0].session.project.name.clone();
    history.record(&live).unwrap();
    live.project.assets[0].name = "Renamed source".into();
    history.record(&live).unwrap();
    live.project.assets[0].name = snapshot.roots[0].session.project.assets[0].name.clone();
    assert!(!snapshot.is_current(&live, Some(&history), 4));
    let solo = live.source_collection_snapshot(None, 4).unwrap();
    let id = live.project.assets[0].id;
    live.audio.insert(
        id,
        daw_media::decode_audio_cancellable(&path, 48000, &|| false).unwrap(),
    );
    assert!(!solo.is_current(&live, None, 4));
    let foreign = SessionHistory::new(&Session::default(), false).unwrap();
    assert!(live.source_collection_snapshot(Some(&foreign), 4).is_err());
}

#[test]
fn diagnostics_include_all_missing_unreadable_missing_buffers_and_conflicting_history_versions() {
    let f = Fixture::new();
    let a = f.wav("changed.wav", 0.1);
    let b = f.wav("missing.wav", 0.2);
    let c = f.wav("directory.wav", 0.3);
    let d = f.wav("unprepared.wav", 0.4);
    let mut live = f.saved(&[&a, &b, &c, &d]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    f.wav("changed.wav", 0.8);
    let id = live.project.assets[0].id;
    live.audio.insert(
        id,
        daw_media::decode_audio_cancellable(&a, 48000, &|| false).unwrap(),
    );
    live.project.name = "New source version".into();
    history.record(&live).unwrap();
    fs::remove_file(&b).unwrap();
    fs::remove_file(&c).unwrap();
    fs::create_dir(&c).unwrap();
    let id = live.project.assets[3].id;
    live.audio.remove(&id);
    let snapshot = live.source_collection_snapshot(Some(&history), 4).unwrap();
    let report = snapshot.preflight(None, &|| false).unwrap();
    assert!(!report.is_ready());
    for kind in [
        SourceIssue::PreparedAudioMismatch,
        SourceIssue::RevisionConflict,
        SourceIssue::MissingSource,
        SourceIssue::NotRegularFile,
        SourceIssue::MissingPreparedAudio,
    ] {
        assert!(
            report.diagnostics().iter().any(|d| d.issue == kind),
            "{kind:?}"
        );
    }
    let conflict = report
        .diagnostics()
        .iter()
        .find(|d| d.issue == SourceIssue::RevisionConflict)
        .unwrap();
    assert!(
        conflict
            .owners
            .iter()
            .any(|&i| snapshot.owners()[i].scope == SourceScope::Current)
    );
    assert!(
        conflict
            .owners
            .iter()
            .any(|&i| matches!(snapshot.owners()[i].scope, SourceScope::Undo(_)))
    );
    assert!(report.into_plan().is_err());
}

#[test]
fn existing_included_recordings_and_stems_are_retained_and_temporary_stems_get_stem_targets() {
    let f = Fixture::new();
    let input = f.wav("recording.wav", 0.2);
    let stem = f.wav("outside-stems/vocals.wav", 0.3);
    let mut live = f.saved(&[&input, &stem]);
    let root = live.folder.clone().unwrap();
    fs::create_dir_all(root.join("assets/sources")).unwrap();
    fs::copy(&input, root.join("assets/sources/included.wav")).unwrap();
    let source = &mut live.project.assets[0].source;
    source.storage = Some(daw_core::SourceStorage::Project);
    source.path_kind = "relative".into();
    source.path = "assets/sources/included.wav".into();
    live.project.assets[1].source.kind = "generated".into();
    let report = preflight(&live, None);
    assert!(report.is_ready());
    assert_eq!(report.summary().files_already_included, 1);
    assert_eq!(report.summary().files_to_copy, 1);
    assert!(
        report
            .verified_sources()
            .iter()
            .flat_map(|s| &s.targets)
            .any(|t| t.kind == CollectionTargetKind::GeneratedStem
                && t.relative_path.starts_with("stems/")
                && t.relative_path.ends_with("/vocals.wav"))
    );
    fs::create_dir_all(root.join("stems/group")).unwrap();
    fs::copy(&stem, root.join("stems/group/vocals.wav")).unwrap();
    live.project.assets[1].source.path_kind = "relative".into();
    live.project.assets[1].source.path = "stems/group/vocals.wav".into();
    let retained = preflight(&live, None);
    assert!(retained.is_ready());
    assert_eq!(retained.summary().files_already_included, 2);
    assert_eq!(retained.summary().additional_bytes, 0);
    live.save(&root).unwrap();
}

#[test]
fn occupied_proposals_and_case_collisions_do_not_overwrite_or_reserve_files() {
    let f = Fixture::new();
    let input = f.wav("recording.wav", 0.2);
    let live = f.saved(&[&input]);
    let snapshot = live.source_collection_snapshot(None, 4).unwrap();
    let report = snapshot.preflight(None, &|| false).unwrap();
    let name = &report.verified_sources()[0].targets[0].relative_path;
    let target = live.folder.as_ref().unwrap().join(name);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"unrelated content").unwrap();
    let occupied = snapshot.preflight(None, &|| false).unwrap();
    assert!(
        occupied
            .diagnostics()
            .iter()
            .any(|d| d.issue == SourceIssue::DestinationOccupied)
    );
    assert_eq!(fs::read(&target).unwrap(), b"unrelated content");
    fs::remove_file(&target).unwrap();
    let uppercase =
        target.with_file_name(target.file_name().unwrap().to_str().unwrap().to_uppercase());
    fs::write(&uppercase, b"unrelated content").unwrap();
    assert!(!snapshot.preflight(None, &|| false).unwrap().is_ready());
    assert_eq!(fs::read(&uppercase).unwrap(), b"unrelated content");
    fs::remove_file(&uppercase).unwrap();
    let staging = live
        .folder
        .as_ref()
        .unwrap()
        .join(format!("assets/.collect-{}", snapshot.operation_id()));
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("unrelated"), b"keep").unwrap();
    let blocked = snapshot.preflight(None, &|| false).unwrap();
    assert!(
        blocked
            .diagnostics()
            .iter()
            .any(|d| d.issue == SourceIssue::DestinationOccupied
                && d.path == fs::canonicalize(&staging).unwrap())
    );
    assert_eq!(fs::read(staging.join("unrelated")).unwrap(), b"keep");
}

#[test]
fn cancellation_unsaved_or_foreign_manifests_never_create_outputs() {
    let f = Fixture::new();
    let source = f.wav("a.wav", 0.2);
    let live = f.saved(&[&source]);
    let snapshot = live.source_collection_snapshot(None, 1).unwrap();
    let before = tree(&f.0);
    let calls = Cell::new(0);
    assert!(
        snapshot
            .preflight(None, &|| {
                calls.set(calls.get() + 1);
                calls.get() >= 8
            })
            .is_err()
    );
    assert_eq!(tree(&f.0), before);
    assert!(
        Session::default()
            .source_collection_snapshot(None, 0)
            .unwrap()
            .preflight(None, &|| false)
            .is_err()
    );
    let manifest = live.folder.as_ref().unwrap().join("project.json");
    fs::write(manifest, serde_json::to_vec(&Project::default()).unwrap()).unwrap();
    assert!(snapshot.preflight(None, &|| false).is_err());
}

#[test]
fn cancellation_during_verification_and_source_mutation_leave_session_and_manifest_intact() {
    let f = Fixture::new();
    let source = f.wav("a.wav", 0.2);
    let live = f.saved(&[&source]);
    let snapshot = live.source_collection_snapshot(None, 5).unwrap();
    let calls = Cell::new(0);
    assert!(
        snapshot
            .preflight(None, &|| {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap()
            .is_ready()
    );
    let midway = calls.get() * 3 / 4;
    let before = tree(&f.0);
    let model = live.project.clone();
    calls.set(0);
    assert!(
        snapshot
            .preflight(None, &|| {
                calls.set(calls.get() + 1);
                calls.get() >= midway
            })
            .is_err()
    );
    assert_eq!(tree(&f.0), before);
    let manifest = fs::read(live.folder.as_ref().unwrap().join("project.json")).unwrap();
    calls.set(0);
    let changed = snapshot
        .preflight(None, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == midway {
                f.wav("a.wav", 0.8);
            }
            false
        })
        .unwrap();
    assert!(!changed.is_ready());
    assert_eq!(live.project, model);
    assert_eq!(
        fs::read(live.folder.as_ref().unwrap().join("project.json")).unwrap(),
        manifest
    );
    assert!(
        !live
            .folder
            .as_ref()
            .unwrap()
            .join("assets/sources")
            .exists()
    );
    assert_eq!(live.audio.values().next().unwrap().samples[0], [0.2, -0.2]);
}

#[test]
fn history_resolves_original_folders_after_save_as_without_using_the_live_folder_as_base() {
    let f = Fixture::new();
    let source = f.wav("original/relative.wav", 0.2);
    let mut live = f.saved(&[&source]);
    assert_eq!(live.project.assets[0].source.path_kind, "relative");
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.assets.clear();
    live.project.tracks.clear();
    live.audio.clear();
    history.record(&live).unwrap();
    live.save(&f.0.join("new/destination")).unwrap();
    let snapshot = live.source_collection_snapshot(Some(&history), 8).unwrap();
    let report = snapshot.preflight(None, &|| false).unwrap();
    assert!(report.is_ready());
    assert_eq!(report.summary().history_only_sources, 1);
    assert_eq!(
        report.verified_sources()[0].path,
        fs::canonicalize(&source).unwrap()
    );
    assert!(
        report.verified_sources()[0]
            .owners
            .iter()
            .all(|&i| snapshot.owners()[i].folder != live.folder)
    );
    assert_eq!(report.summary().files_to_copy, 1);
}

#[test]
fn one_source_at_multiple_history_processing_rates_stays_compatible() {
    let f = Fixture::new();
    let source = f.wav("a.wav", 0.2);
    let mut live = f.saved(&[&source]);
    let mut history = SessionHistory::new(&live, true).unwrap();
    live = live.with_audio_settings(96000, 512).unwrap();
    history.record(&live).unwrap();
    let report = preflight(&live, Some(&history));
    assert!(report.is_ready());
    assert_eq!(report.summary().source_files, 1);
    assert_eq!(report.summary().files_to_copy, 1);
    assert_eq!(
        report
            .snapshot()
            .owners()
            .iter()
            .map(|o| o.prepared_audio().unwrap().sample_rate_hz)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([48000, 96000])
    );
}

#[test]
fn mixed_provenance_shares_one_verified_source_with_compatible_separate_targets() {
    let f = Fixture::new();
    let source = f.wav("a.wav", 0.2);
    let mut live = f.saved(&[&source, &source]);
    live.project.assets[1].source.kind = "generated".into();
    let report = preflight(&live, None);
    assert!(report.is_ready());
    assert_eq!(report.summary().source_files, 1);
    assert_eq!(report.summary().duplicate_references, 1);
    // Imported ownership requires assets/sources; generated ownership retains stems/.
    assert_eq!(report.summary().files_to_copy, 2);
    assert_eq!(report.verified_sources()[0].targets.len(), 2);
}

#[cfg(unix)]
#[test]
fn output_and_included_symlinks_are_rejected_but_external_aliases_deduplicate() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let source = f.wav("a.wav", 0.2);
    let alias = f.0.join("alias.wav");
    symlink(&source, &alias).unwrap();
    let mut live = f.saved(&[&source, &alias]);
    let snapshot = live.source_collection_snapshot(None, 1).unwrap();
    assert_eq!(
        snapshot
            .preflight(None, &|| false)
            .unwrap()
            .summary()
            .source_files,
        1
    );
    let root = live.folder.clone().unwrap();
    symlink(&f.0, root.join("assets/sources")).unwrap();
    assert!(!snapshot.preflight(None, &|| false).unwrap().is_ready());
    fs::remove_file(root.join("assets/sources")).unwrap();
    fs::create_dir(root.join("assets/sources")).unwrap();
    symlink(&source, root.join("assets/sources/included.wav")).unwrap();
    let asset = &mut live.project.assets[0];
    asset.source.storage = Some(daw_core::SourceStorage::Project);
    asset.source.path_kind = "relative".into();
    asset.source.path = "assets/sources/included.wav".into();
    assert!(!preflight(&live, None).is_ready());
    // An unreadable external loop reports an issue rather than hanging or hiding other owners.
    let loop_path = f.0.join("loop.wav");
    symlink("loop.wav", &loop_path).unwrap();
    live.project.assets[0].source.storage = None;
    live.project.assets[0].source.path_kind = "absolute".into();
    live.project.assets[0].source.path = loop_path.to_string_lossy().into_owned();
    assert!(
        preflight(&live, None)
            .diagnostics()
            .iter()
            .any(|d| d.issue == SourceIssue::UnreadableSource)
    );
}
