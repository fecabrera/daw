use daw_core::{ClipLoop, DEFAULT_TRACK_COLOR, Edit, Project, RgbColor, default_track_color};
use daw_project::Session;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("daw-test-{}", daw_core::Id::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn wav(&self, name: &str, rate: u32, channels: u16, bits: u16, float: bool) -> PathBuf {
        let path = self.0.join(name);
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: bits,
                sample_format: if float {
                    hound::SampleFormat::Float
                } else {
                    hound::SampleFormat::Int
                },
            },
        )
        .unwrap();
        for i in 0..rate / 10 {
            for _ in 0..channels {
                let s = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.25;
                if float {
                    writer.write_sample(s).unwrap();
                } else if bits == 16 {
                    writer.write_sample((s * 32767.0) as i16).unwrap();
                } else {
                    writer.write_sample((s * 8388607.0) as i32).unwrap();
                }
            }
        }
        writer.finalize().unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn inserted_clip_snapshots_share_assets_persist_and_reject_invalid_placement() {
    let fixture = Fixture::new();
    let source = fixture.wav("copy.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[0].id;
    let original = session.project.tracks[0].clips[0].clone();
    let samples = session.audio[&original.asset_id].samples.clone();
    let copy = daw_core::Clip {
        id: daw_core::Id::new_v4(),
        start_frame: 6000,
        source_offset_frame: 1000,
        length_frames: 7500,
        repeat: Some(ClipLoop {
            length_frames: 2000,
            phase_frame: 700,
        }),
        color: Some(RgbColor {
            r: 100,
            g: 50,
            b: 150,
        }),
        ..original.clone()
    };
    session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: copy.clone(),
        })
        .unwrap();
    assert_eq!(session.project.assets.len(), 1);
    assert_eq!(session.audio.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &samples,
        &session.audio[&copy.asset_id].samples
    ));
    let before = serde_json::to_value(&session.project).unwrap();
    for failure in 0..7 {
        let mut invalid = copy.clone();
        invalid.id = daw_core::Id::new_v4();
        let mut destination = track;
        match failure {
            0 => invalid.id = copy.id,
            1 => invalid.asset_id = daw_core::Id::new_v4(),
            2 => invalid.start_frame = 100,
            3 => invalid.source_offset_frame = 4800,
            4 => invalid.start_frame = u64::MAX,
            5 => destination = daw_core::Id::new_v4(),
            _ => {
                invalid.repeat = Some(ClipLoop {
                    length_frames: 0,
                    phase_frame: 0,
                })
            }
        }
        assert!(
            session
                .project
                .edit(Edit::InsertClip {
                    track_id: destination,
                    clip: invalid
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    }
    let folder = fixture.0.join("project");
    session.save(&folder).unwrap();
    let loaded = Session::open(&folder).unwrap();
    let pasted = &loaded.project.tracks[0].clips[1];
    assert_eq!(
        serde_json::to_value(pasted).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    session
        .export(&fixture.0.join("before.wav"), false)
        .unwrap();
    loaded.export(&fixture.0.join("after.wav"), false).unwrap();
    assert_eq!(
        fs::read(fixture.0.join("before.wav")).unwrap(),
        fs::read(fixture.0.join("after.wav")).unwrap()
    );
}

#[test]
fn supported_encodings_and_resampling() {
    let f = Fixture::new();
    for channels in [1, 2] {
        for (bits, float) in [(16, false), (24, false), (32, true)] {
            let path = f.wav(
                &format!("{channels}-{bits}.wav"),
                44100,
                channels,
                bits,
                float,
            );
            let mut s = Session::default();
            s.import(&path, None, 0).unwrap();
            assert_eq!(s.project.assets[0].decoded_frame_count, 4800);
            assert_eq!(s.audio.values().next().unwrap().metadata.channels, channels);
        }
    }
    let unsupported = f.wav("three-channel.wav", 48000, 3, 16, false);
    assert!(Session::default().import(&unsupported, None, 0).is_err());
}

#[test]
fn save_reopen_export_repeatability_missing_and_shortened_sources() {
    let f = Fixture::new();
    let source = f.wav("source.wav", 48000, 2, 24, false);
    let mut s = Session::default();
    s.import(&source, None, 480).unwrap();
    let folder = f.0.join("project");
    s.save(&folder).unwrap();
    assert_eq!(s.project.assets[0].source.path_kind, "relative");
    let loaded = Session::open(&folder).unwrap();
    let one = f.0.join("one.wav");
    let two = f.0.join("two.wav");
    s.export(&one, false).unwrap();
    loaded.export(&two, false).unwrap();
    assert_eq!(fs::read(&one).unwrap(), fs::read(&two).unwrap());
    assert!(s.export(&one, false).is_err());
    assert!(s.export(&source, true).is_err());
    assert_eq!(hound::WavReader::open(&one).unwrap().duration(), 5280);
    // A valid source that becomes shorter preserves the saved arrangement.
    let mut writer = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..200 {
        writer.write_sample(0i32).unwrap();
    }
    writer.finalize().unwrap();
    let shorter = Session::open(&folder).unwrap();
    assert!(!shorter.warnings.is_empty());
    assert_eq!(shorter.project.end(), 5280);
    assert_eq!(shorter.plan().sample_at(5000), [0.0, 0.0]);
    fs::remove_file(source).unwrap();
    let mut missing = Session::open(&folder).unwrap();
    assert_eq!(missing.audio.len(), 0);
    assert!(!missing.warnings.is_empty());
    assert_eq!(missing.project.end(), 5280);
    missing.export(&f.0.join("missing.wav"), false).unwrap();
    missing.save(&folder).unwrap();
    assert_eq!(Session::open(&folder).unwrap().project.end(), 5280);
}

#[test]
fn edit_rejection_is_transactional_and_split_preserves_ranges() {
    let f = Fixture::new();
    let source = f.wav("source.wav", 48000, 1, 16, false);
    let mut s = Session::default();
    let first = s.import(&source, None, 0).unwrap();
    let track = s.project.tracks[0].id;
    s.import(&source, Some(track), 4800).unwrap();
    let original = serde_json::to_string(&s.project).unwrap();
    assert!(
        s.project
            .edit(Edit::Place {
                clip_id: first,
                track_id: track,
                start: 1,
                offset: 0,
                length: 4800,
                repeat: None
            })
            .is_err()
    );
    assert_eq!(original, serde_json::to_string(&s.project).unwrap());
    s.project
        .edit(Edit::Split {
            clip_id: first,
            at: 2400,
        })
        .unwrap();
    s.project.validate().unwrap();
    assert_eq!(s.project.tracks[0].clips.len(), 3);
    let right = s.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.start_frame == 2400)
        .unwrap();
    assert_eq!(right.source_offset_frame, 2400);
}

#[test]
fn display_colors_persist_and_survive_clip_edits_without_changing_audio() {
    let f = Fixture::new();
    let source = f.wav("colors.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    let clip = session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[0].id;
    let track_color = RgbColor {
        r: 42,
        g: 31,
        b: 60,
    };
    let clip_color = Some(RgbColor {
        r: 150,
        g: 70,
        b: 30,
    });
    let audio = session.plan().sample_at(1234);
    session
        .project
        .edit(Edit::SetTrackColor {
            track_id: track,
            color: track_color,
        })
        .unwrap();
    session
        .project
        .edit(Edit::SetClipColor {
            clip_id: clip,
            color: clip_color,
        })
        .unwrap();
    assert_eq!(session.plan().sample_at(1234), audio);
    let before = serde_json::to_value(&session.project).unwrap();
    for command in [
        Edit::SetTrackColor {
            track_id: daw_core::Id::new_v4(),
            color: DEFAULT_TRACK_COLOR,
        },
        Edit::SetClipColor {
            clip_id: daw_core::Id::new_v4(),
            color: None,
        },
    ] {
        assert!(session.project.edit(command).is_err());
        assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    }
    session.save(&f.0.join("project")).unwrap();
    let mut loaded = Session::open(&f.0.join("project")).unwrap();
    assert_eq!(loaded.project.tracks[0].color, track_color);
    assert_eq!(loaded.project.tracks[0].clips[0].color, clip_color);
    let destination = loaded.project.add_track().unwrap();
    loaded
        .project
        .edit(Edit::Place {
            clip_id: clip,
            track_id: destination,
            start: 100,
            offset: 100,
            length: 4600,
            repeat: None,
        })
        .unwrap();
    loaded
        .project
        .edit(Edit::Split {
            clip_id: clip,
            at: 2400,
        })
        .unwrap();
    assert_eq!(loaded.project.tracks[0].color, track_color);
    assert_eq!(loaded.project.tracks[1].color, default_track_color(1));
    assert!(
        loaded.project.tracks[1]
            .clips
            .iter()
            .all(|c| c.color == clip_color)
    );
    for id in loaded.project.tracks[1]
        .clips
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>()
    {
        loaded
            .project
            .edit(Edit::SetClipColor {
                clip_id: id,
                color: None,
            })
            .unwrap();
    }
    loaded
        .project
        .edit(Edit::SetTrackColor {
            track_id: track,
            color: DEFAULT_TRACK_COLOR,
        })
        .unwrap();
    loaded.save(&f.0.join("project")).unwrap();
    let reset = Session::open(&f.0.join("project")).unwrap();
    let manifest = serde_json::to_value(&reset.project).unwrap();
    assert!(
        reset
            .project
            .tracks
            .iter()
            .enumerate()
            .all(|(index, t)| t.color == default_track_color(index))
    );
    assert!(
        reset.project.tracks[1]
            .clips
            .iter()
            .all(|c| c.color.is_none())
    );
    assert_eq!(
        manifest["tracks"][0]["color"],
        serde_json::json!({"r":255,"g":112,"b":67})
    );
    assert!(manifest["tracks"][1]["clips"][0].get("color").is_none());
}

#[test]
fn older_manifests_use_default_colors_and_invalid_rgb_is_rejected() {
    let f = Fixture::new();
    let source = f.wav("default-colors.wav", 48000, 1, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    session.save(&f.0.join("project")).unwrap();
    let manifest = f.0.join("project/project.json");
    let mut original: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(
        original["tracks"][0]["color"],
        serde_json::json!({"r":255,"g":112,"b":67})
    );
    original["tracks"][0]
        .as_object_mut()
        .unwrap()
        .remove("color");
    fs::write(&manifest, serde_json::to_vec(&original).unwrap()).unwrap();
    assert!(original["tracks"][0].get("color").is_none());
    assert!(original["tracks"][0]["clips"][0].get("color").is_none());
    let loaded = Session::open(&f.0.join("project")).unwrap();
    assert_eq!(loaded.project.tracks[0].color, DEFAULT_TRACK_COLOR);
    assert_eq!(loaded.project.tracks[0].clips[0].color, None);
    for path in ["/tracks/0/color", "/tracks/0/clips/0/color"] {
        for bad in [
            serde_json::json!(-1),
            serde_json::json!(256),
            serde_json::json!(12.5),
        ] {
            let mut value = original.clone();
            let parent = path.rsplit_once('/').unwrap().0;
            value.pointer_mut(parent).unwrap()["color"] =
                serde_json::json!({"r":bad,"g":30,"b":60});
            fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(Session::open(&f.0.join("project")).is_err());
        }
    }
    original["tracks"][0]["clips"][0]["color"] = serde_json::Value::Null;
    fs::write(&manifest, serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(
        Session::open(&f.0.join("project")).unwrap().project.tracks[0].clips[0].color,
        None
    );
    original["tracks"][0]["color"] = serde_json::Value::Null;
    fs::write(&manifest, serde_json::to_vec(&original).unwrap()).unwrap();
    assert!(Session::open(&f.0.join("project")).is_err());
}

#[test]
fn missing_track_colors_get_the_palette_order_and_saved_colors_are_preserved() {
    let f = Fixture::new();
    let mut session = Session::default();
    for _ in 0..18 {
        session.project.add_track().unwrap();
    }
    let custom = RgbColor {
        r: 12,
        g: 34,
        b: 56,
    };
    session.project.tracks[3].color = custom;
    session.project.tracks[6].color = daw_core::LEGACY_CLIP_COLOR;
    session.save(&f.0).unwrap();
    let saved = Session::open(&f.0).unwrap();
    for (index, track) in saved.project.tracks.iter().enumerate() {
        assert_eq!(track.color, session.project.tracks[index].color);
    }
    let manifest = f.0.join("project.json");
    let mut value = serde_json::to_value(&saved.project).unwrap();
    for (index, track) in value["tracks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        if index != 3 && index != 6 {
            track.as_object_mut().unwrap().remove("color");
        }
    }
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut loaded = Session::open(&f.0).unwrap();
    for (index, track) in loaded.project.tracks.iter().enumerate() {
        assert_eq!(
            track.color,
            match index {
                3 => custom,
                6 => daw_core::LEGACY_CLIP_COLOR,
                _ => default_track_color(index),
            }
        );
    }
    loaded.save(&f.0).unwrap();
    assert_eq!(
        serde_json::to_value(&Session::open(&f.0).unwrap().project).unwrap(),
        serde_json::to_value(&loaded.project).unwrap()
    );
    loaded.project.add_track().unwrap();
    assert_eq!(loaded.project.tracks[18].color, default_track_color(18));
}

#[test]
fn clip_repeats_save_reopen_move_split_and_export_without_copying_audio() {
    let f = Fixture::new();
    let source = f.wav("repeat.wav", 48000, 2, 24, false);
    let mut session = Session::default();
    let id = session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[0].id;
    let asset = session.project.tracks[0].clips[0].asset_id;
    let samples = session.audio[&asset].samples.clone();
    let repeat = ClipLoop {
        length_frames: 2000,
        phase_frame: 700,
    };
    session
        .project
        .edit(Edit::Place {
            clip_id: id,
            track_id: track,
            start: 100,
            offset: 1000,
            length: 7500,
            repeat: Some(repeat),
        })
        .unwrap();
    let original = session.project.tracks[0].clips[0].clone();
    session.save(&f.0.join("project")).unwrap();
    let mut loaded = Session::open(&f.0.join("project")).unwrap();
    let clip = &loaded.project.tracks[0].clips[0];
    assert_eq!(clip.repeat, Some(repeat));
    assert_eq!(clip.length_frames, 7500);
    assert_eq!(clip.source_frame(2000), 1700);
    session.export(&f.0.join("one.wav"), false).unwrap();
    loaded.export(&f.0.join("two.wav"), false).unwrap();
    assert_eq!(
        fs::read(f.0.join("one.wav")).unwrap(),
        fs::read(f.0.join("two.wav")).unwrap()
    );
    assert_eq!(
        hound::WavReader::open(f.0.join("one.wav"))
            .unwrap()
            .duration(),
        7600
    );
    let destination = loaded.project.add_track().unwrap();
    loaded
        .project
        .edit(Edit::Place {
            clip_id: id,
            track_id: destination,
            start: 100,
            offset: 1000,
            length: 7500,
            repeat: Some(repeat),
        })
        .unwrap();
    loaded
        .project
        .edit(Edit::Split {
            clip_id: id,
            at: 2600,
        })
        .unwrap();
    let right = loaded.project.tracks[1]
        .clips
        .iter()
        .find(|c| c.start_frame == 2600)
        .unwrap();
    assert_eq!(right.source_offset_frame, 1000);
    assert_eq!(right.repeat.unwrap().phase_frame, 1200);
    for local in 0..right.length_frames {
        assert_eq!(
            right.source_frame(local),
            original.source_frame(local + 2500)
        );
    }
    assert_eq!(loaded.project.assets.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &session.audio[&asset].samples,
        &samples
    ));
    let older =
        serde_json::to_value(&Session::open(&f.0.join("project")).unwrap().project).unwrap();
    let mut older = older;
    older["tracks"][0]["clips"][0]
        .as_object_mut()
        .unwrap()
        .remove("repeat");
    older["tracks"][0]["clips"][0]["length_frames"] = serde_json::json!(2000);
    let older: Project = serde_json::from_value(older).unwrap();
    older.validate().unwrap();
    assert_eq!(older.tracks[0].clips[0].repeat, None);
}

#[test]
fn invalid_clip_repeats_and_overlaps_are_rejected_transactionally() {
    let f = Fixture::new();
    let source = f.wav("repeat-bounds.wav", 48000, 1, 16, false);
    let mut session = Session::default();
    let id = session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[0].id;
    session.import(&source, Some(track), 6000).unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    for (start, offset, length, repeat) in [
        (
            0,
            0,
            5500,
            ClipLoop {
                length_frames: 0,
                phase_frame: 0,
            },
        ),
        (
            0,
            0,
            5500,
            ClipLoop {
                length_frames: 1000,
                phase_frame: 1000,
            },
        ),
        (
            0,
            4000,
            5500,
            ClipLoop {
                length_frames: 1000,
                phase_frame: 0,
            },
        ),
        (
            0,
            u64::MAX,
            5500,
            ClipLoop {
                length_frames: 1000,
                phase_frame: 0,
            },
        ),
        (
            u64::MAX - 10,
            0,
            100,
            ClipLoop {
                length_frames: 1000,
                phase_frame: 0,
            },
        ),
        (
            0,
            0,
            6500,
            ClipLoop {
                length_frames: 1000,
                phase_frame: 0,
            },
        ),
    ] {
        assert!(
            session
                .project
                .edit(Edit::Place {
                    clip_id: id,
                    track_id: track,
                    start,
                    offset,
                    length,
                    repeat: Some(repeat)
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    }
}

#[test]
fn tempo_round_trips_and_older_projects_use_the_default() {
    let f = Fixture::new();
    let mut session = Session::default();
    session.project.tempo_bpm = 97.25;
    session.save(&f.0).unwrap();
    assert_eq!(Session::open(&f.0).unwrap().project.tempo_bpm, 97.25);
    let manifest = f.0.join("project.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("tempo_bpm");
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(Session::open(&f.0).unwrap().project.tempo_bpm, 120.0);
    value["tempo_bpm"] = serde_json::json!(0.0);
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(Session::open(&f.0).is_err());
}

#[test]
fn schema_version_is_ignored_and_invalid_structure_is_rejected() {
    let f = Fixture::new();
    let mut value = serde_json::to_value(Project::default()).unwrap();
    value["schema_version"] = serde_json::json!({"any":"value"});
    fs::write(
        f.0.join("project.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let s = Session::open(&f.0).unwrap();
    assert!(s.export(&f.0.join("empty.wav"), false).is_err());
    value["sample_rate_hz"] = serde_json::json!(44100);
    fs::write(
        f.0.join("project.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    assert!(Session::open(&f.0).is_err());
}

#[test]
fn eight_tracks_save_reopen_export_and_solo_overrides_mute() {
    let f = Fixture::new();
    let source = f.wav("source.wav", 48000, 2, 24, false);
    let mut s = Session::default();
    for _ in 0..8 {
        s.import(&source, None, 0).unwrap();
    }
    let folder = f.0.join("project");
    s.save(&folder).unwrap();
    let reopened = Session::open(&folder).unwrap();
    assert_eq!(reopened.project.tracks.len(), 8);
    let original_export = f.0.join("original.wav");
    let reopened_export = f.0.join("reopened.wav");
    s.export(&original_export, false).unwrap();
    reopened.export(&reopened_export, false).unwrap();
    assert_eq!(
        fs::read(original_export).unwrap(),
        fs::read(reopened_export).unwrap()
    );
    let all = s.plan().sample_at(1200);
    s.project.tracks[0].muted = true;
    s.project.tracks[0].soloed = true;
    let solo = s.plan().sample_at(1200);
    assert!((all[0] - solo[0] * 8.0).abs() < 1e-6);
    s.project.tracks[0].soloed = false;
    let seven = s.plan().sample_at(1200);
    assert!((seven[0] - solo[0] * 7.0).abs() < 1e-6);
}

#[test]
fn save_as_keeps_sources_and_failed_save_preserves_manifest() {
    let f = Fixture::new();
    let source = f.wav("source.wav", 48000, 2, 16, false);
    let mut s = Session::default();
    s.import(&source, None, 0).unwrap();
    let original_folder = f.0.join("original");
    s.save(&original_folder).unwrap();
    let original = fs::read(original_folder.join("project.json")).unwrap();
    let destination = f.0.join("nested/other");
    s.save(&destination).unwrap();
    assert_eq!(s.source_path(&s.project.assets[0]).unwrap(), source);
    assert_eq!(Session::open(&destination).unwrap().audio.len(), 1);
    // An invalid live state fails before replacing the existing manifest.
    let saved = fs::read(destination.join("project.json")).unwrap();
    s.project.tracks[0].pan = 2.0;
    assert!(s.save(&destination).is_err());
    assert_eq!(fs::read(destination.join("project.json")).unwrap(), saved);
    assert_eq!(
        fs::read(original_folder.join("project.json")).unwrap(),
        original
    );
    s.project.tracks[0].pan = 0.0;
    let blocked = f.0.join("not-a-folder");
    fs::write(&blocked, b"existing file").unwrap();
    assert!(s.save(&blocked).is_err());
    assert_eq!(s.folder.as_ref().unwrap(), &destination);
    assert_eq!(fs::read(blocked).unwrap(), b"existing file");
}

#[test]
fn damaged_and_nonfinite_sources_fail_without_partial_import() {
    let f = Fixture::new();
    let truncated = f.wav("truncated.wav", 48000, 2, 16, false);
    let len = fs::metadata(&truncated).unwrap().len();
    fs::OpenOptions::new()
        .write(true)
        .open(&truncated)
        .unwrap()
        .set_len(len - 100)
        .unwrap();
    let mut s = Session::default();
    assert!(s.import(&truncated, None, 0).is_err());
    assert!(s.project.tracks.is_empty() && s.audio.is_empty());
    let path = f.0.join("nan.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    writer.write_sample(f32::NAN).unwrap();
    writer.finalize().unwrap();
    assert!(s.import(&path, None, 0).is_err());
    assert!(s.project.tracks.is_empty() && s.audio.is_empty());
}

#[test]
fn export_preserves_float_headroom_then_warns_and_saturates() {
    let f = Fixture::new();
    let source = f.wav("source.wav", 48000, 2, 24, false);
    let mut s = Session::default();
    s.import(&source, None, 0).unwrap();
    s.project.master.gain_db = 24.0;
    assert!(s.plan().sample_at(1300)[0].abs() > 1.0);
    let output = f.0.join("clipped.wav");
    let warnings = s.export(&output, false).unwrap();
    assert!(warnings.iter().any(|w| w.contains("clipped")));
    let samples = hound::WavReader::open(output)
        .unwrap()
        .samples::<i32>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(*samples.iter().max().unwrap(), 8_388_607);
    assert_eq!(*samples.iter().min().unwrap(), -8_388_608);
}
