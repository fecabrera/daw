use daw_core::{Id, SamplerSettings};
use daw_project::{
    Session, SessionHistory,
    source_collection::{CollectionOutcome, CollectionStatus},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-source-bundling-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn wav(&self, name: &str, value: f32) -> PathBuf {
        let path = self.0.join(name);
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
        for _ in 0..4800 {
            writer.write_sample(value).unwrap();
            writer.write_sample(-value).unwrap();
        }
        writer.finalize().unwrap();
        path
    }
    fn session(&self) -> Session {
        let mut s = Session::default();
        s.import(&self.wav("a.wav", 0.2), None, 0).unwrap();
        s.save(&self.0.join("original")).unwrap();
        s
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn collect(session: &Session, history: Option<&SessionHistory>) -> CollectionOutcome {
    session
        .source_collection_snapshot(history, 7)
        .unwrap()
        .preflight(None, &|| false)
        .unwrap()
        .into_plan()
        .unwrap()
        .execute(
            &|s| s.is_current(session, history, 7),
            &|| false,
            &mut |_| Ok(()),
        )
        .unwrap()
}
fn render(session: &Session) -> Vec<[f32; 2]> {
    let mut out = vec![[0.; 2]; session.project.end() as usize];
    session.plan().render(0, &mut out);
    out
}
fn sources_inside(session: &Session, folder: &Path) {
    let root = fs::canonicalize(folder).unwrap();
    for asset in session
        .project
        .assets
        .iter()
        .filter(|a| a.midi_pattern.is_none())
    {
        let path = fs::canonicalize(session.source_path(asset).unwrap()).unwrap();
        assert!(path.starts_with(&root), "{path:?}");
        assert!(path.is_file());
    }
}
#[test]
fn collection_and_save_as_protect_redo_only_media_and_keep_render_and_pcm() {
    for generated in [false, true] {
        let f = Fixture::new();
        let mut live = f.session();
        let old_folder = live.folder.clone().unwrap();
        let baseline = render(&live);
        let id = live.project.assets[0].id;
        let pcm = live.audio[&id].samples.clone();
        let mut history = SessionHistory::new(&live, true).unwrap();
        live.import(&f.wav("b.wav", 0.4), None, 9600).unwrap();
        if generated {
            live.project.assets[1].source.kind = "generated".into();
        }
        history.record(&live).unwrap();
        live = history.undo(&live).unwrap();
        assert!(history.can_redo());
        live.project.transport.playhead_frame = 321;
        let outcome = collect(&live, Some(&history));
        assert_eq!(outcome.status(), CollectionStatus::Committed);
        live.project.transport.playhead_frame = 654; // Playback does not invalidate collection.
        outcome.apply(&mut live, Some(&mut history), 7).unwrap();
        assert_eq!(live.project.transport.playhead_frame, 654);
        assert!(history.is_saved());
        assert!(history.can_redo());
        assert!(Arc::ptr_eq(&pcm, &live.audio[&id].samples));
        assert_eq!(render(&live), baseline);
        fs::remove_file(f.0.join("a.wav")).unwrap();
        fs::remove_file(f.0.join("b.wav")).unwrap();
        let mut redone = history.redo(&live).unwrap();
        sources_inside(&redone, &old_folder);
        live = history.undo(&redone).unwrap();
        let destination = f.0.join("copy");
        let saved = live
            .save_snapshot(Some(&history))
            .unwrap()
            .save(&destination)
            .unwrap();
        saved.apply(&mut live, Some(&mut history)).unwrap();
        assert!(history.is_saved());
        assert!(history.can_redo());
        assert!(Arc::ptr_eq(&pcm, &live.audio[&id].samples));
        drop(outcome);
        drop(saved);
        fs::remove_dir_all(&old_folder).unwrap();
        redone = history.redo(&live).unwrap();
        assert_eq!(redone.project.assets.len(), 2);
        sources_inside(&redone, &destination);
        live = history.undo(&redone).unwrap();
        assert!(history.is_saved());
        assert_eq!(render(&live), baseline);
        live.save(&destination).unwrap();
        let reopened = Session::open(&destination).unwrap();
        assert!(reopened.warnings.is_empty());
        assert_eq!(render(&reopened), baseline);
        redone.save(&destination).unwrap();
        for asset in &redone.project.assets {
            if asset.source.kind == "generated" {
                assert!(!asset.source.path.contains('\\'));
            }
        }
        let reopened = Session::open(&destination).unwrap();
        assert_eq!(render(&reopened), render(&redone));
    }
}

#[test]
fn collection_retargets_pending_and_undo_without_finishing_the_group() {
    let f = Fixture::new();
    let mut live = f.session();
    let mut history = SessionHistory::new(&live, true).unwrap();
    live.project.name = "Committed edit".into();
    history.record(&live).unwrap();
    history.begin_group();
    live.project.master.gain_db = -3.;
    history.record(&live).unwrap();
    let out = collect(&live, Some(&history));
    out.apply(&mut live, Some(&mut history), 7).unwrap();
    assert!(history.is_saved());
    fs::remove_file(f.0.join("a.wav")).unwrap();
    history.finish_group();
    assert!(history.is_saved());
    let old = history.undo(&live).unwrap();
    assert_eq!(old.project.master.gain_db, 0.);
    sources_inside(&old, live.folder.as_ref().unwrap());
    assert_eq!(history.redo(&old).unwrap().project.master.gain_db, -3.);
}
#[test]
fn committed_receipts_reject_stale_or_foreign_targets_without_partial_changes() {
    let f = Fixture::new();
    let mut live = f.session();
    let mut history = SessionHistory::new(&live, true).unwrap();
    let out = collect(&live, Some(&history));
    let before = live.project.clone();
    assert!(out.apply(&mut live, Some(&mut history), 8).is_err());
    assert_eq!(live.project, before);
    assert!(history.is_saved());
    live.project.name = "New edit".into();
    assert!(out.apply(&mut live, Some(&mut history), 7).is_err());
    assert_eq!(live.project.name, "New edit");
    let mut foreign = Session::default();
    let before = foreign.project.clone();
    assert!(out.apply(&mut foreign, None, 7).is_err());
    assert_eq!(foreign.project, before);
}
#[test]
fn older_save_snapshot_preserves_newer_edits_audio_navigation_and_dirty_state() {
    let f = Fixture::new();
    let mut live = f.session();
    let mut history = SessionHistory::new(&live, true).unwrap();
    collect(&live, Some(&history))
        .apply(&mut live, Some(&mut history), 7)
        .unwrap();
    let snapshot = live.save_snapshot(Some(&history)).unwrap();
    let id = live.project.assets[0].id;
    let pcm = live.audio[&id].samples.clone();
    history.begin_group();
    live.project.tracks[0].name = "Newer edit".into();
    history.record(&live).unwrap();
    live.project.transport.playhead_frame = 912;
    let saved = snapshot.save(&f.0.join("later-save")).unwrap();
    saved.apply(&mut live, Some(&mut history)).unwrap();
    assert_eq!(live.project.tracks[0].name, "Newer edit");
    assert_eq!(live.project.transport.playhead_frame, 912);
    assert!(!history.is_saved());
    assert!(Arc::ptr_eq(&pcm, &live.audio[&id].samples));
    history.finish_group();
    let old = history.undo(&live).unwrap();
    assert!(history.is_saved());
    assert_ne!(old.project.tracks[0].name, "Newer edit");
    sources_inside(&old, live.folder.as_ref().unwrap());
}
#[test]
fn collection_and_save_as_keep_sampler_and_granulizer_inputs_without_clips() {
    let f = Fixture::new();
    let path = f.wav("sample.wav", 0.3);
    let mut live = Session::default();
    for granular in [false, true] {
        let track = live.project.add_midi_track().unwrap();
        let layer = &mut live
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
            SamplerSettings::granular()
        } else {
            SamplerSettings::default()
        });
        live = live.with_sampler_file(track, id, &path).unwrap();
    }
    live.save(&f.0.join("original")).unwrap();
    let mut history = SessionHistory::new(&live, true).unwrap();
    collect(&live, Some(&history))
        .apply(&mut live, Some(&mut history), 7)
        .unwrap();
    let destination = f.0.join("copy");
    let saved = live
        .save_snapshot(Some(&history))
        .unwrap()
        .save(&destination)
        .unwrap();
    saved.apply(&mut live, Some(&mut history)).unwrap();
    fs::remove_file(&path).unwrap();
    fs::remove_dir_all(f.0.join("original")).unwrap();
    let reopened = Session::open(&destination).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.project.assets.len(), 2);
    assert!(reopened.project.tracks.iter().all(|t| t.clips.is_empty()));
    sources_inside(&reopened, &destination);
}
#[test]
fn ordinary_save_keeps_later_imports_external_and_retains_missing_included_warnings() {
    let f = Fixture::new();
    let mut live = f.session();
    collect(&live, None).apply(&mut live, None, 7).unwrap();
    let included = live.source_path(&live.project.assets[0]).unwrap();
    let later = f.wav("later.wav", 0.1);
    live.import(&later, None, 9600).unwrap();
    let folder = live.folder.clone().unwrap();
    live.save(&folder).unwrap();
    assert!(!live.project.assets[1].source.is_included_recording());
    assert_eq!(
        fs::canonicalize(live.source_path(&live.project.assets[1]).unwrap()).unwrap(),
        fs::canonicalize(&later).unwrap()
    );
    fs::remove_file(included).unwrap();
    let lexical_folder = f.0.join("original");
    let mut reopened = Session::open(&lexical_folder).unwrap();
    assert!(!reopened.warnings.is_empty());
    reopened.save(&lexical_folder).unwrap();
    assert!(reopened.project.assets[0].source.is_included_recording());
}
#[test]
fn failed_save_as_preserves_live_history_and_existing_destination_entries() {
    let f = Fixture::new();
    let mut live = f.session();
    let mut history = SessionHistory::new(&live, true).unwrap();
    collect(&live, Some(&history))
        .apply(&mut live, Some(&mut history), 7)
        .unwrap();
    let before = live.project.clone();
    let original = fs::read(live.source_path(&live.project.assets[0]).unwrap()).unwrap();
    let destination = f.0.join("blocked");
    fs::create_dir_all(destination.join("project.json")).unwrap();
    fs::write(destination.join("project.json/keep"), b"unrelated").unwrap();
    assert!(
        live.save_snapshot(Some(&history))
            .unwrap()
            .save(&destination)
            .is_err()
    );
    assert_eq!(live.project, before);
    assert!(history.is_saved());
    assert_eq!(
        fs::read(destination.join("project.json/keep")).unwrap(),
        b"unrelated"
    );
    assert_eq!(
        fs::read(live.source_path(&live.project.assets[0]).unwrap()).unwrap(),
        original
    );
    assert!(
        fs::read_dir(destination.join("assets"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn save_as_rejects_owned_bytes_that_no_longer_match_prepared_audio() {
    let f = Fixture::new();
    let mut live = f.session();
    collect(&live, None).apply(&mut live, None, 7).unwrap();
    let input = live.source_path(&live.project.assets[0]).unwrap();
    let changed = f.wav("changed.wav", 0.8);
    fs::copy(&changed, &input).unwrap();
    let before = live.project.clone();
    let folder = live.folder.as_ref().unwrap();
    let manifest = fs::read(folder.join("project.json")).unwrap();
    let destination = f.0.join("bad-copy");
    assert!(
        live.save_snapshot(None)
            .unwrap()
            .save(&destination)
            .is_err()
    );
    assert_eq!(live.project, before);
    assert_eq!(fs::read(folder.join("project.json")).unwrap(), manifest);
    assert_eq!(fs::read(input).unwrap(), fs::read(changed).unwrap());
    assert!(!destination.join("project.json").exists());
    assert!(
        fs::read_dir(destination.join("assets"))
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn save_as_receipt_cannot_rebase_uncaptured_included_owners() {
    let f = Fixture::new();
    let mut live = f.session();
    collect(&live, None).apply(&mut live, None, 7).unwrap();
    let snapshot = live.save_snapshot(None).unwrap();
    let mut duplicate = live.project.assets[0].clone();
    let old = duplicate.id;
    duplicate.id = Id::new_v4();
    live.audio.insert(duplicate.id, live.audio[&old].clone());
    live.project.assets.push(duplicate);
    let before = live.project.clone();
    let folder = live.folder.clone();
    let saved = snapshot.save(&f.0.join("copy")).unwrap();
    assert!(saved.apply(&mut live, None).is_err());
    assert_eq!(live.project, before);
    assert_eq!(live.folder, folder);
}

#[test]
fn trimmed_repeated_stretched_and_pitched_clips_keep_prepared_audio_and_exports() {
    for engine in [
        daw_core::StretchEngine::RUBBER_BAND,
        daw_core::StretchEngine::INTEGRATED,
    ] {
        let f = Fixture::new();
        let mut live = f.session();
        live.project.stretch_engine = engine;
        let clip = &mut live.project.tracks[0].clips[0];
        clip.source_offset_frame = 120;
        clip.length_frames = 3000;
        clip.stretch = daw_core::ClipStretch::new(3, 2);
        clip.repeat = Some(daw_core::ClipLoop {
            length_frames: 1500,
            phase_frame: 80,
        });
        let id = clip.id;
        live = live
            .with_edit(daw_core::Edit::SetClipTranspose {
                clip_id: id,
                transpose: 5,
            })
            .unwrap();
        let original_clip = live.project.tracks[0].clips[0].clone();
        let prepared = live.audio_for_clip(&original_clip).unwrap().samples.clone();
        let before = render(&live);
        live.export(&f.0.join("before.wav"), false).unwrap();
        collect(&live, None).apply(&mut live, None, 7).unwrap();
        assert_eq!(live.project.tracks[0].clips[0], original_clip);
        assert!(Arc::ptr_eq(
            &prepared,
            &live.audio_for_clip(&original_clip).unwrap().samples
        ));
        let destination = f.0.join("copy");
        live.save(&destination).unwrap();
        assert!(Arc::ptr_eq(
            &prepared,
            &live.audio_for_clip(&original_clip).unwrap().samples
        ));
        fs::remove_file(f.0.join("a.wav")).unwrap();
        fs::remove_dir_all(f.0.join("original")).unwrap();
        let reopened = Session::open(&destination).unwrap();
        assert!(reopened.warnings.is_empty());
        assert_eq!(reopened.project.tracks[0].clips[0], original_clip);
        assert_eq!(render(&reopened), before);
        reopened.export(&f.0.join("after.wav"), false).unwrap();
        assert_eq!(
            fs::read(f.0.join("before.wav")).unwrap(),
            fs::read(f.0.join("after.wav")).unwrap()
        );
    }
}

#[test]
fn collected_compressed_media_keeps_full_original_bytes_through_save_as() {
    let f = Fixture::new();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media/tests/fixtures");
    let mut live = Session::default();
    let names = ["tone-2.mp3", "tone-2.aac", "tone-2.m4a"];
    for name in names {
        live.import(&fixtures.join(name), None, 0).unwrap();
    }
    live.save(&f.0.join("original")).unwrap();
    collect(&live, None).apply(&mut live, None, 7).unwrap();
    // Valid included manifests do not require a filename extension. Save As
    // derives it from verified codec/container metadata, as collection does.
    let root = live.folder.clone().unwrap();
    let old = live.source_path(&live.project.assets[0]).unwrap();
    fs::rename(old, root.join("assets/sources/no-extension")).unwrap();
    live.project.assets[0].source.path = "assets/sources/no-extension".into();
    let baseline = render(&live);
    let destination = f.0.join("copy");
    live.save(&destination).unwrap();
    fs::remove_dir_all(root).unwrap();
    let reopened = Session::open(&destination).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(render(&reopened), baseline);
    for ((asset, name), extension) in reopened
        .project
        .assets
        .iter()
        .zip(names)
        .zip(["mp3", "aac", "mp4"])
    {
        assert!(asset.source.path.ends_with(extension));
        assert_eq!(
            fs::read(reopened.source_path(asset).unwrap()).unwrap(),
            fs::read(fixtures.join(name)).unwrap()
        );
    }
}
