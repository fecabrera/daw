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
fn processing_rate_and_buffer_survive_save_reopen_import_stretch_and_render() {
    let f = Fixture::new();
    let source = f.wav("high-rate.wav", 96_000, 2, 24, false);
    let source_bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 48_000).unwrap();
    let clip = &mut session.project.tracks[0].clips[0];
    clip.source_offset_frame = 240;
    clip.length_frames = 2400;
    clip.repeat = Some(ClipLoop {
        length_frames: 960,
        phase_frame: 240,
    });
    clip.stretch = daw_core::ClipStretch::new(2, 3);
    session.prepare_stretches().unwrap();
    let clip = session.project.tracks[0].clips[0].clone();
    let before = session.project.clone();
    let mut changed = session.with_audio_settings(96_000, 128).unwrap();
    let audio = &changed.audio[&clip.asset_id];
    assert_eq!(audio.sample_rate_hz, 96_000);
    assert_eq!(audio.samples.len(), 9600);
    assert_eq!(
        *audio.samples,
        *daw_media::decode_wav_at_rate(&source, 96_000)
            .unwrap()
            .samples
    );
    assert_eq!(
        changed.stretched_audio[&(clip.asset_id, clip.stretch.unwrap())]
            .samples
            .len(),
        14_400
    );
    let next_clip = &changed.project.tracks[0].clips[0];
    assert_eq!(next_clip.start_frame, clip.start_frame * 2);
    assert_eq!(next_clip.source_offset_frame, clip.source_offset_frame * 2);
    assert_eq!(next_clip.length_frames, clip.length_frames * 2);
    assert_eq!(
        next_clip.repeat.unwrap().phase_frame,
        clip.repeat.unwrap().phase_frame * 2
    );
    assert_eq!(next_clip.stretch, clip.stretch);
    assert_eq!(
        changed.project.assets[0].source_metadata.sample_rate_hz,
        96_000
    );
    changed.save(&f.0.join("project")).unwrap();
    let loaded = Session::open(&f.0.join("project")).unwrap();
    assert_eq!(loaded.project.sample_rate_hz, 96_000);
    assert_eq!(loaded.project.buffer_size_frames, 128);
    assert_eq!(loaded.audio[&clip.asset_id].sample_rate_hz, 96_000);
    assert_eq!(
        loaded.stretched_audio[&(clip.asset_id, clip.stretch.unwrap())].sample_rate_hz,
        96_000
    );
    let output = f.0.join("high-rate-mix.wav");
    loaded.export(&output, false).unwrap();
    let reader = hound::WavReader::open(&output).unwrap();
    assert_eq!(reader.spec().sample_rate, 96_000);
    assert_eq!(u64::from(reader.duration()), loaded.project.end());
    let mut imported = loaded.clone();
    imported
        .import(&source, None, imported.project.frames(2.0))
        .unwrap();
    assert_eq!(
        imported.project.assets.last().unwrap().decoded_frame_count,
        9600
    );
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(format!("{:?}", session.project), format!("{before:?}"));
}

#[test]
fn legacy_projects_default_buffer_and_failed_audio_changes_retain_session() {
    let f = Fixture::new();
    let mut json = serde_json::to_value(Project::default()).unwrap();
    json.as_object_mut().unwrap().remove("buffer_size_frames");
    fs::write(f.0.join("project.json"), serde_json::to_vec(&json).unwrap()).unwrap();
    let legacy = Session::open(&f.0).unwrap();
    assert_eq!(legacy.project.sample_rate_hz, 48_000);
    assert_eq!(legacy.project.buffer_size_frames, 512);
    let source = f.wav("source.wav", 48_000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let before = format!("{:?}", session.project);
    let asset = session.project.assets[0].id;
    let samples = session.audio[&asset].samples.clone();
    fs::write(&source, b"broken").unwrap();
    assert!(session.with_audio_settings(44_100, 256).is_err());
    assert_eq!(format!("{:?}", session.project), before);
    assert!(std::sync::Arc::ptr_eq(
        &samples,
        &session.audio[&asset].samples
    ));
    let buffer_only = session.with_audio_settings(48_000, 64).unwrap();
    assert_eq!(buffer_only.project.buffer_size_frames, 64);
    assert!(std::sync::Arc::ptr_eq(
        &samples,
        &buffer_only.audio[&asset].samples
    ));
    fs::remove_file(source).unwrap();
    let cached = session.with_audio_settings(44_100, 256).unwrap();
    assert_eq!(cached.audio[&asset].samples.len(), 4410);
    assert!(cached.warnings.last().unwrap().contains("cached audio"));
}

#[test]
fn resize_neighbor_trims_preserve_source_audio_and_survive_save_render_and_reopen() {
    use daw_core::{Clip, ClipEdge, Id};
    let fixture = Fixture::new();
    let source = fixture.wav("source.wav", 48000, 2, 16, false);
    let source_bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 400).unwrap();
    let primary = &mut session.project.tracks[0].clips[0];
    primary.length_frames = 600;
    let primary = primary.clone();
    let covered = Clip {
        id: Id::new_v4(),
        start_frame: 1050,
        length_frames: 200,
        source_offset_frame: 600,
        ..primary.clone()
    };
    let neighbor = Clip {
        id: Id::new_v4(),
        start_frame: 1400,
        length_frames: 600,
        source_offset_frame: 1200,
        repeat: Some(ClipLoop {
            length_frames: 100,
            phase_frame: 25,
        }),
        ..primary.clone()
    };
    session.project.tracks[0]
        .clips
        .extend([covered.clone(), neighbor.clone()]);
    session.project.validate().unwrap();
    let samples = session.audio[&primary.asset_id].samples.clone();
    session
        .project
        .edit(Edit::ResizeClips {
            clips: vec![primary.looped_by(ClipEdge::Right, 700)],
            edge: ClipEdge::Right,
        })
        .unwrap();
    assert_eq!(session.project.tracks[0].clips.len(), 2);
    let remaining = session.project.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.id == neighbor.id)
        .unwrap();
    assert_eq!(
        (remaining.start_frame, remaining.length_frames),
        (1700, 300)
    );
    for local in 0..300 {
        assert_eq!(
            remaining.source_frame(local),
            neighbor.source_frame(local + 300)
        );
    }
    assert!(std::sync::Arc::ptr_eq(
        &samples,
        &session.audio[&primary.asset_id].samples
    ));
    let mut before = vec![[0.0; 2]; session.project.end() as usize];
    session.plan().render(0, &mut before);
    session.save(&fixture.0.join("project")).unwrap();
    session
        .export(&fixture.0.join("before.wav"), false)
        .unwrap();
    let reopened = Session::open(&fixture.0.join("project")).unwrap();
    let mut after = vec![[0.0; 2]; before.len()];
    reopened.plan().render(0, &mut after);
    assert_eq!(before, after);
    let remaining = reopened.project.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.id == neighbor.id)
        .unwrap();
    assert_eq!(remaining.repeat.unwrap().phase_frame, 25);
    assert!(
        !reopened.project.tracks[0]
            .clips
            .iter()
            .any(|clip| clip.id == covered.id)
    );
    reopened
        .export(&fixture.0.join("after.wav"), false)
        .unwrap();
    assert_eq!(
        fs::read(fixture.0.join("before.wav")).unwrap(),
        fs::read(fixture.0.join("after.wav")).unwrap()
    );
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn named_project_save_and_save_as_create_child_folders_and_keep_sources() {
    let fixture = Fixture::new();
    let source = fixture.wav("source.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let id = session.project.project_id;
    let asset = session.project.assets[0].id;
    let samples = session.audio[&asset].samples.clone();
    session.save_new(&fixture.0, "  First mix  ").unwrap();
    let first = fixture.0.join("First mix");
    assert_eq!(session.folder, Some(first.clone()));
    assert_eq!(session.project.name, "First mix");
    assert!(first.join("assets").is_dir());
    assert!(!fixture.0.join("project.json").exists());
    let manifest = fs::read(first.join("project.json")).unwrap();
    session.export(&fixture.0.join("first.wav"), false).unwrap();
    let parent = fixture.0.join("another location");
    fs::create_dir(&parent).unwrap();
    session.save_new(&parent, "Second mix").unwrap();
    let second = parent.join("Second mix");
    assert_eq!(session.folder, Some(second.clone()));
    assert_eq!(session.project.name, "Second mix");
    assert_eq!(session.project.project_id, id);
    assert!(std::sync::Arc::ptr_eq(
        &samples,
        &session.audio[&asset].samples
    ));
    assert_eq!(fs::read(first.join("project.json")).unwrap(), manifest);
    let reopened = Session::open(&second).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.project.name, "Second mix");
    assert_eq!(reopened.project.project_id, id);
    assert_eq!(reopened.audio[&asset].samples, samples);
    assert_eq!(
        session.source_path(&session.project.assets[0]).unwrap(),
        source
    );
    reopened
        .export(&fixture.0.join("second.wav"), false)
        .unwrap();
    assert_eq!(
        fs::read(fixture.0.join("first.wav")).unwrap(),
        fs::read(fixture.0.join("second.wav")).unwrap()
    );
    session.save(&second).unwrap();
    assert!(!second.join("Second mix").exists());
    assert_eq!(Session::open(&first).unwrap().project.name, "First mix");
}

#[test]
fn named_saves_reject_invalid_names_collisions_and_fail_without_changing_session() {
    use daw_project::project_folder_name;
    let fixture = Fixture::new();
    for name in [
        "",
        "  ",
        ".",
        "..",
        "../escape",
        "a/b",
        "a\\b",
        "a:b",
        "bad\nname",
        "a?b",
        "a*b",
        "a\"b",
        "a<b",
        "a>b",
        "a|b",
        "name.",
        "CON",
        "nul.wav",
        "COM1",
        "lpt9.txt",
    ] {
        assert!(project_folder_name(name).is_err(), "Accepted {name:?}");
        assert!(Session::default().save_new(&fixture.0, name).is_err());
    }
    assert!(project_folder_name(&"a".repeat(256)).is_err());
    assert_eq!(project_folder_name("  Música 01  ").unwrap(), "Música 01");
    assert_eq!(
        project_folder_name("Composition 1").unwrap(),
        "Composition 1"
    );
    assert!(fs::read_dir(&fixture.0).unwrap().next().is_none());
    let source = fixture.wav("source.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    session.save_new(&fixture.0, "Existing").unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    let folder = session.folder.clone();
    let manifest = fs::read(fixture.0.join("Existing/project.json")).unwrap();
    assert!(
        session
            .save_new(&fixture.0, "Existing")
            .unwrap_err()
            .to_string()
            .contains("already exists")
    );
    assert_eq!(
        fs::read(fixture.0.join("Existing/project.json")).unwrap(),
        manifest
    );
    fs::write(fixture.0.join("occupied"), b"leave this file").unwrap();
    assert!(session.save_new(&fixture.0, "occupied").is_err());
    assert_eq!(
        fs::read(fixture.0.join("occupied")).unwrap(),
        b"leave this file"
    );
    assert!(session.save_new(&fixture.0.join("absent"), "New").is_err());
    assert!(!fixture.0.join("absent").exists());
    assert!(session.save_new(&source, "New").is_err());
    assert_eq!(session.folder, folder);
    assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    let mut invalid_source = session.clone();
    invalid_source.folder = None;
    let before = serde_json::to_value(&invalid_source.project).unwrap();
    assert!(invalid_source.save_new(&fixture.0, "Failed save").is_err());
    assert!(!fixture.0.join("Failed save").exists());
    assert!(invalid_source.folder.is_none());
    assert_eq!(
        serde_json::to_value(&invalid_source.project).unwrap(),
        before
    );
}

#[test]
fn export_progress_tracks_encoded_frames_without_changing_audio() {
    use daw_media::{ExportFormat, ExportSettings};
    let fixture = Fixture::new();
    let source = fixture.wav("progress-source.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 2400).unwrap();
    for format in ExportFormat::ALL {
        let settings = ExportSettings {
            format,
            ..Default::default()
        };
        let output = fixture.0.join(format!("progress.{}", format.extension()));
        let mut events = Vec::new();
        let warnings = session
            .export_with_progress(&output, false, settings, |frames, total| {
                events.push((frames, total))
            })
            .unwrap();
        assert!(warnings.is_empty());
        let total = session.project.end();
        assert_eq!(events.first(), Some(&(0, total)));
        assert_eq!(events.last(), Some(&(total, total)));
        assert!(
            events
                .windows(2)
                .all(|pair| pair[1].0 > pair[0].0 && pair[1].0 - pair[0].0 <= 1024)
        );
        assert!(
            events
                .iter()
                .all(|&(frames, end)| frames <= total && end == total)
        );
        let plain = fixture.0.join(format!("plain.{}", format.extension()));
        session
            .export_with_settings(&plain, false, settings)
            .unwrap();
        assert_eq!(fs::read(output).unwrap(), fs::read(plain).unwrap());
    }
}

#[test]
fn export_formats_write_selected_codecs_and_mp3_preserves_gapless_length() {
    use daw_media::{ExportFormat, ExportSettings, Mp3Bitrate, WavCodec};
    use symphonia::core::{
        audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
        meta::MetadataOptions, probe::Hint,
    };
    let fixture = Fixture::new();
    let source = fixture.wav("formats.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 2400).unwrap();
    session.project.tracks[0].pan = -0.4;
    let clip = &mut session.project.tracks[0].clips[0];
    clip.length_frames = 48001;
    clip.repeat = Some(ClipLoop {
        length_frames: 4800,
        phase_frame: 0,
    });
    session.project.transport.r#loop = daw_core::Loop {
        enabled: true,
        start_frame: 2400,
        end_frame: 7200,
    };
    let before = serde_json::to_value(&session.project).unwrap();
    let mut rendered = vec![[0.0; 2]; session.project.end() as usize];
    session.plan().render(0, &mut rendered);
    for codec in WavCodec::ALL {
        let settings = ExportSettings {
            wav_codec: codec,
            ..Default::default()
        };
        let output = fixture.0.join(format!("{codec:?}.wav"));
        session
            .export_with_settings(&output, false, settings)
            .unwrap();
        let reader = hound::WavReader::open(&output).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48000);
        assert_eq!(reader.duration(), rendered.len() as u32);
        let (bits, sample_format) = match codec {
            WavCodec::Pcm16 => (16, hound::SampleFormat::Int),
            WavCodec::Pcm24 => (24, hound::SampleFormat::Int),
            WavCodec::Float32 => (32, hound::SampleFormat::Float),
        };
        assert_eq!(reader.spec().bits_per_sample, bits);
        assert_eq!(reader.spec().sample_format, sample_format);
        let decoded = daw_media::decode_wav(&output).unwrap();
        let tolerance = if codec == WavCodec::Pcm16 {
            2.0 / 32768.0
        } else if codec == WavCodec::Pcm24 {
            2.0 / 8_388_608.0
        } else {
            0.0
        };
        for (actual, expected) in decoded.samples.iter().zip(&rendered) {
            for ch in 0..2 {
                assert!((actual[ch] - expected[ch]).abs() <= tolerance);
            }
        }
        let again = fixture.0.join(format!("{codec:?}-again.wav"));
        session
            .export_with_settings(&again, false, settings)
            .unwrap();
        assert_eq!(fs::read(&output).unwrap(), fs::read(again).unwrap());
        if codec == WavCodec::Pcm24 {
            let default = fixture.0.join("default.wav");
            session.export(&default, false).unwrap();
            assert_eq!(fs::read(default).unwrap(), fs::read(output).unwrap());
        }
    }
    for bitrate in Mp3Bitrate::ALL {
        let output = fixture.0.join(format!("{}.mp3", bitrate.kbps()));
        let settings = ExportSettings {
            format: ExportFormat::Mp3,
            mp3_bitrate: bitrate,
            ..Default::default()
        };
        session
            .export_with_settings(&output, false, settings)
            .unwrap();
        let bytes = fs::read(&output).unwrap();
        assert_eq!(bytes[0], 0xff);
        assert_eq!(bytes[1] & 0xfe, 0xfa); // MPEG-1, Layer III.
        assert_eq!(bytes[2] & 0x0c, 0x04); // 48 kHz.
        let bitrate_table = [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
        ];
        assert_eq!(bitrate_table[(bytes[2] >> 4) as usize], bitrate.kbps());
        let mss = MediaSourceStream::new(
            Box::new(fs::File::open(&output).unwrap()),
            Default::default(),
        );
        let mut hint = Hint::new();
        hint.with_extension("mp3");
        let mut format = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .unwrap()
            .format;
        let track = format.default_track().unwrap();
        assert_eq!(track.codec_params.sample_rate, Some(48000));
        assert_eq!(track.codec_params.channels.unwrap().count(), 2);
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .unwrap();
        let mut samples = Vec::new();
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => panic!("{error}"),
            };
            let decoded = decoder.decode(&packet).unwrap();
            let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            buffer.copy_interleaved_ref(decoded);
            samples.extend_from_slice(buffer.samples());
        }
        assert_eq!(
            samples.len(),
            rendered.len() * 2,
            "{} kbps gapless duration",
            bitrate.kbps()
        );
        let mse: f64 = samples
            .iter()
            .zip(rendered.iter().flatten())
            .map(|(a, b)| f64::from(a - b).powi(2))
            .sum::<f64>()
            / samples.len() as f64;
        assert!(
            mse.sqrt() < 0.015,
            "{} kbps RMS error {}",
            bitrate.kbps(),
            mse.sqrt()
        );
        assert!(samples[..4000].iter().all(|sample| sample.abs() < 0.002));
        let again = fixture.0.join("repeat.mp3");
        session
            .export_with_settings(&again, true, settings)
            .unwrap();
        assert_eq!(bytes, fs::read(again).unwrap());
    }
    assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
}

#[test]
fn export_settings_keep_overwrite_protection_and_float_headroom() {
    use daw_media::{ExportFormat, ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("source.wav", 48000, 2, 16, false);
    let source_bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    session.project.master.gain_db = 24.0;
    let project_folder = fixture.0.join("project");
    session.save(&project_folder).unwrap();
    let manifest = fs::read(project_folder.join("project.json")).unwrap();
    for settings in [
        ExportSettings::default(),
        ExportSettings {
            wav_codec: WavCodec::Pcm16,
            ..Default::default()
        },
        ExportSettings {
            wav_codec: WavCodec::Float32,
            ..Default::default()
        },
        ExportSettings {
            format: ExportFormat::Mp3,
            ..Default::default()
        },
    ] {
        let output = fixture.0.join(format!(
            "{:?}.{}",
            settings.wav_codec,
            settings.format.extension()
        ));
        fs::write(&output, b"existing output").unwrap();
        assert!(
            session
                .export_with_settings(&output, false, settings)
                .is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), b"existing output");
        assert!(
            session
                .export_with_settings(&source, true, settings)
                .is_err()
        );
        assert!(
            session
                .export_with_settings(&project_folder.join("project.json"), true, settings)
                .is_err()
        );
        let warnings = session
            .export_with_settings(&output, true, settings)
            .unwrap();
        if settings.format == ExportFormat::Wav && settings.wav_codec == WavCodec::Float32 {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.contains("preserves headroom"))
            );
            assert!(
                hound::WavReader::open(&output)
                    .unwrap()
                    .samples::<f32>()
                    .any(|sample| sample.unwrap().abs() > 1.0)
            );
        } else {
            assert!(warnings.iter().any(|warning| warning.contains("clipped")));
        }
        assert!(
            Session::default()
                .export_with_settings(&output, true, settings)
                .is_err()
        );
        let completed = fs::read(&output).unwrap();
        let mut invalid_audio = session.clone();
        let audio = invalid_audio.audio.values_mut().next().unwrap();
        std::sync::Arc::make_mut(&mut audio.samples)[1000][0] = f32::NAN;
        assert!(
            invalid_audio
                .export_with_settings(&output, true, settings)
                .unwrap_err()
                .to_string()
                .contains("non-finite")
        );
        assert_eq!(fs::read(&output).unwrap(), completed);
    }
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(
        fs::read(project_folder.join("project.json")).unwrap(),
        manifest
    );
    assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".export-")
    }));
}

#[test]
fn track_reordering_preserves_data_audio_and_saved_order() {
    let fixture = Fixture::new();
    let source = fixture.wav("reorder.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    for index in 0..6 {
        session.import(&source, None, index * 4800).unwrap();
        let track = &mut session.project.tracks[index as usize];
        track.name = format!("Instrument {index}");
        track.gain_db = -(index as f32);
        track.pan = index as f32 / 10.0;
        track.muted = index == 4;
        track.soloed = index == 2;
        let clip = &mut track.clips[0];
        clip.name = format!("Take {index}");
        clip.source_offset_frame = 100;
        clip.length_frames = 3600;
        if index == 2 {
            clip.repeat = Some(ClipLoop {
                length_frames: 1200,
                phase_frame: 300,
            });
            clip.color = Some(RgbColor {
                r: 10,
                g: 90,
                b: 160,
            });
        }
    }
    let original = session.project.clone();
    let before_audio = fixture.0.join("before.wav");
    session.export(&before_audio, false).unwrap();
    let moved = original.tracks[0].id;
    session
        .project
        .edit(Edit::ReorderTrack {
            track_id: moved,
            index: 5,
        })
        .unwrap();
    let expected: Vec<_> = [1, 2, 3, 4, 5, 0]
        .map(|index| original.tracks[index].clone())
        .into();
    assert_eq!(
        serde_json::to_value(&session.project.tracks).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&session.project.assets).unwrap(),
        serde_json::to_value(&original.assets).unwrap()
    );
    let snapshot = serde_json::to_value(&session.project).unwrap();
    for invalid in [
        Edit::ReorderTrack {
            track_id: moved,
            index: 6,
        },
        Edit::ReorderTrack {
            track_id: daw_core::Id::new_v4(),
            index: 0,
        },
        Edit::Batch(vec![
            Edit::ReorderTrack {
                track_id: moved,
                index: 0,
            },
            Edit::ReorderTrack {
                track_id: moved,
                index: usize::MAX,
            },
        ]),
    ] {
        assert!(session.project.edit(invalid).is_err());
        assert_eq!(serde_json::to_value(&session.project).unwrap(), snapshot);
    }
    session
        .project
        .edit(Edit::ReorderTrack {
            track_id: moved,
            index: 5,
        })
        .unwrap();
    assert_eq!(serde_json::to_value(&session.project).unwrap(), snapshot);
    let project_folder = fixture.0.join("reordered-project");
    session.save(&project_folder).unwrap();
    let mut reopened = Session::open(&project_folder).unwrap();
    assert_eq!(
        serde_json::to_value(&reopened.project.tracks).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let after_audio = fixture.0.join("after.wav");
    reopened.export(&after_audio, false).unwrap();
    assert_eq!(
        fs::read(before_audio).unwrap(),
        fs::read(after_audio).unwrap()
    );
    // Move the last track back to the top, then move a middle track upward.
    reopened
        .project
        .edit(Edit::ReorderTrack {
            track_id: moved,
            index: 0,
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(&reopened.project.tracks).unwrap(),
        serde_json::to_value(&original.tracks).unwrap()
    );
    reopened
        .project
        .edit(Edit::ReorderTrack {
            track_id: original.tracks[4].id,
            index: 1,
        })
        .unwrap();
    assert_eq!(
        reopened
            .project
            .tracks
            .iter()
            .map(|track| track.id)
            .collect::<Vec<_>>(),
        [0, 4, 1, 2, 3, 5].map(|index| original.tracks[index].id)
    );
}

#[test]
fn batched_group_edits_validate_final_layout_and_persist() {
    let fixture = Fixture::new();
    let source = fixture.wav("group.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[0].id;
    session.project.tracks[0].clips[0].length_frames = 1200;
    let a = session.project.tracks[0].clips[0].clone();
    let b = daw_core::Clip {
        id: daw_core::Id::new_v4(),
        name: "B".into(),
        start_frame: 1200,
        source_offset_frame: 1200,
        ..a.clone()
    };
    session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: b.clone(),
        })
        .unwrap();
    let place = |clip: &daw_core::Clip, start| Edit::Place {
        clip_id: clip.id,
        track_id: track,
        start,
        offset: clip.source_offset_frame,
        length: clip.length_frames,
        repeat: clip.repeat,
    };
    // Moving A first temporarily overlaps B. Validate only after both moves finish.
    session
        .project
        .edit(Edit::Batch(vec![place(&a, 1200), place(&b, 2400)]))
        .unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    for invalid in [
        Edit::Batch(vec![place(&a, 0), place(&b, 0)]),
        Edit::Batch(vec![
            place(&a, 0),
            Edit::InsertClip {
                track_id: daw_core::Id::new_v4(),
                clip: b.clone(),
            },
        ]),
        Edit::Batch(vec![
            place(&a, 0),
            Edit::Batch(vec![Edit::Place {
                clip_id: b.id,
                track_id: track,
                start: 2400,
                offset: 4700,
                length: 1200,
                repeat: None,
            }]),
        ]),
    ] {
        assert!(session.project.edit(invalid).is_err());
        assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    }
    let second = session.project.add_track().unwrap();
    let copies: Vec<_> = session.project.tracks[0]
        .clips
        .iter()
        .map(|clip| daw_core::Clip {
            id: daw_core::Id::new_v4(),
            start_frame: clip.start_frame + 4800,
            color: Some(RgbColor {
                r: 20,
                g: 80,
                b: 160,
            }),
            ..clip.clone()
        })
        .collect();
    session
        .project
        .edit(Edit::Batch(
            copies
                .iter()
                .map(|clip| Edit::InsertClip {
                    track_id: second,
                    clip: clip.clone(),
                })
                .collect(),
        ))
        .unwrap();
    assert_eq!(session.project.assets.len(), 1);
    assert_eq!(session.audio.len(), 1);
    let folder = fixture.0.join("project");
    session.save(&folder).unwrap();
    let loaded = Session::open(&folder).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.project.tracks).unwrap(),
        serde_json::to_value(&session.project.tracks).unwrap()
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
fn tempo_edits_retain_clip_beats_and_source_metadata_across_save_and_render() {
    let f = Fixture::new();
    let source = f.wav("tempo.wav", 48000, 2, 16, false);
    let source_bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 48000).unwrap();
    session.import(&source, None, 72123).unwrap();
    for track in &mut session.project.tracks {
        let clip = &mut track.clips[0];
        clip.source_offset_frame = 600;
        clip.length_frames = 2400;
        clip.repeat = Some(ClipLoop {
            length_frames: 1200,
            phase_frame: 123,
        });
        clip.stretch = daw_core::ClipStretch::new(2, 3);
        clip.color = Some(DEFAULT_TRACK_COLOR);
    }
    session.prepare_stretches().unwrap();
    let prepared_count = session.stretched_audio.len();
    session.project.transport.playhead_frame = 1234;
    session.project.transport.r#loop = daw_core::Loop {
        enabled: true,
        start_frame: 100,
        end_frame: 4000,
    };
    let original = session.project.clone();
    let samples = session.audio[&original.assets[0].id].samples.clone();
    for bpm in [60.0, 123.5, 240.0] {
        session.project = original.clone();
        session.project.edit(Edit::SetTempo { bpm }).unwrap();
        for (track, before) in session.project.tracks.iter().zip(&original.tracks) {
            assert_eq!(track.id, before.id);
            let mut expected = before.clips[0].clone();
            expected.start_frame =
                (expected.start_frame as f64 * 120.0 / f64::from(bpm)).round() as u64;
            assert_eq!(
                serde_json::to_value(&track.clips[0]).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            let old_beats = before.clips[0].start_frame as f64 * 120.0 / (48000.0 * 60.0);
            let new_beats = track.clips[0].start_frame as f64 * f64::from(bpm) / (48000.0 * 60.0);
            assert!(
                (new_beats - old_beats).abs() <= f64::from(bpm) / (48000.0 * 60.0) * 0.5 + 1e-12
            );
        }
        let region = &session.project.transport.r#loop;
        assert_eq!(
            session.project.transport.playhead_frame,
            original.transport.playhead_frame
        );
        assert_eq!(region.enabled, original.transport.r#loop.enabled);
        assert_eq!(
            region.start_frame,
            (100.0 * 120.0 / f64::from(bpm)).ceil() as u64
        );
        assert_eq!(
            region.end_frame,
            (4000.0 * 120.0 / f64::from(bpm)).ceil() as u64
        );
        assert!(std::sync::Arc::ptr_eq(
            &samples,
            &session.audio[&original.assets[0].id].samples
        ));
        assert_eq!(session.stretched_audio.len(), prepared_count);
    }
    // Live playback uses the new positions, with the same prepared source samples.
    session.project.transport.r#loop.enabled = false;
    let plan = session.plan();
    let mut live = daw_engine::Renderer::new(plan.clone());
    live.seek(0);
    live.play();
    for frame in 0..26400 {
        assert_eq!(live.next_sample(), plan.sample_at(frame));
    }
    assert_eq!(plan.sample_at(47999), [0.0; 2]);
    let folder = f.0.join("project");
    session.save(&folder).unwrap();
    let loaded = Session::open(&folder).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.project).unwrap(),
        serde_json::to_value(&session.project).unwrap()
    );
    let before = f.0.join("before.wav");
    let after = f.0.join("after.wav");
    session.export(&before, false).unwrap();
    loaded.export(&after, false).unwrap();
    assert_eq!(fs::read(before).unwrap(), fs::read(after).unwrap());
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn tempo_edits_reject_overlap_invalid_values_and_timeline_overflow_atomically() {
    let f = Fixture::new();
    let source = f.wav("tempo.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 24000).unwrap();
    session.import(&source, None, 0).unwrap();
    let track = session.project.tracks[1].id;
    let mut second = session.project.tracks[1].clips[0].clone();
    second.id = daw_core::Id::new_v4();
    second.start_frame = second.length_frames;
    session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: second,
        })
        .unwrap();
    session.project.transport.r#loop = daw_core::Loop {
        enabled: true,
        start_frame: 24000,
        end_frame: 96000,
    };
    let original = serde_json::to_value(&session.project).unwrap();
    for bpm in [240.0, f32::MAX, 0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(session.project.edit(Edit::SetTempo { bpm }).is_err());
        assert_eq!(serde_json::to_value(&session.project).unwrap(), original);
    }
    session.project.edit(Edit::SetTempo { bpm: 120.0 }).unwrap();
    assert_eq!(serde_json::to_value(&session.project).unwrap(), original);
    // Adjacent exclusive endpoints are valid; speeding up creates an overlap.
    session.project.edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
    session.project.edit(Edit::SetTempo { bpm: 120.0 }).unwrap();
    assert_eq!(serde_json::to_value(&session.project).unwrap(), original);
    session.project.tracks[0].clips[0].start_frame = u64::MAX - 4800;
    session.project.validate().unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    for bpm in [60.0, f32::MIN_POSITIVE] {
        assert!(session.project.edit(Edit::SetTempo { bpm }).is_err());
        assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    }
    session.project.tracks[0].clips[0].start_frame = u64::MAX / 2 - 1024;
    session.project.validate().unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    assert!(session.project.edit(Edit::SetTempo { bpm: 60.0 }).is_err());
    assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
}

#[test]
fn tempo_changes_scale_enabled_and_disabled_selections_and_reject_invalid_ranges() {
    let f = Fixture::new();
    let source = f.wav("loop-tempo.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 24000).unwrap();
    let base = session.project.clone();
    for enabled in [false, true] {
        for bpm in [60.0, 123.5, 240.0] {
            session.project = base.clone();
            session.project.transport.playhead_frame = 12345;
            session.project.transport.r#loop = daw_core::Loop {
                enabled,
                start_frame: 24000,
                end_frame: 96000,
            };
            session.project.edit(Edit::SetTempo { bpm }).unwrap();
            let region = &session.project.transport.r#loop;
            assert_eq!(region.enabled, enabled);
            assert_eq!(
                region.start_frame,
                (24000.0 * 120.0 / f64::from(bpm)).ceil() as u64
            );
            assert_eq!(
                region.end_frame,
                (96000.0 * 120.0 / f64::from(bpm)).ceil() as u64
            );
            assert_eq!(session.project.transport.playhead_frame, 12345);
            for (frame, old_beats) in [(region.start_frame, 1.0), (region.end_frame, 4.0)] {
                let beats = frame as f64 * f64::from(bpm) / (48000.0 * 60.0);
                assert!(
                    beats >= old_beats && beats - old_beats < f64::from(bpm) / (48000.0 * 60.0)
                );
            }
        }
    }
    session.project = base.clone();
    session.project.edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
    assert_eq!(session.project.transport.r#loop.start_frame, 0);
    assert_eq!(session.project.transport.r#loop.end_frame, 0);
    for (start_frame, end_frame, bpm) in [(24000, u64::MAX, 60.0), (1, 2, f32::MAX)] {
        session.project = base.clone();
        session.project.transport.r#loop = daw_core::Loop {
            enabled: true,
            start_frame,
            end_frame,
        };
        session.project.validate().unwrap();
        let before = serde_json::to_value(&session.project).unwrap();
        assert!(session.project.edit(Edit::SetTempo { bpm }).is_err());
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
    value["sample_rate_hz"] = serde_json::json!(22050);
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

#[test]
fn stretched_clips_keep_sources_and_caches_through_trim_loop_split_copy_save_and_export() {
    let fixture = Fixture::new();
    let source = fixture.wav("source.wav", 48000, 2, 24, false);
    let original_file = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let asset = session.project.assets[0].id;
    let original_audio = session.audio[&asset].samples.clone();
    let original = session.project.tracks[0].clips[0].clone();
    let clip = &mut session.project.tracks[0].clips[0];
    clip.source_offset_frame = 1200;
    clip.length_frames = 2400;
    *clip = clip.stretched_to(0, 4800).unwrap();
    session.prepare_stretches().unwrap();
    let stretched = session.project.tracks[0].clips[0].clone();
    assert_eq!(stretched.source_offset_frame, 2400);
    let cached = session.audio_for_clip(&stretched).unwrap().samples.clone();
    assert_eq!(cached.len(), 9600);
    session.prepare_stretches().unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &cached,
        &session.audio_for_clip(&stretched).unwrap().samples
    ));
    assert!(std::sync::Arc::ptr_eq(
        &original_audio,
        &session.audio[&asset].samples
    ));
    assert_eq!(original.length_frames, 4800);
    // Restore the full source extent using stretched-frame coordinates.
    session.project.tracks[0].clips[0].length_frames = 9600 - stretched.source_offset_frame;
    session.project.validate().unwrap();
    session.project.tracks[0].clips[0] = stretched.clone();
    let clip = &mut session.project.tracks[0].clips[0];
    clip.repeat = Some(ClipLoop {
        length_frames: 4800,
        phase_frame: 300,
    });
    clip.length_frames = 7200;
    let id = clip.id;
    session
        .project
        .edit(Edit::Split {
            clip_id: id,
            at: 2400,
        })
        .unwrap();
    let second = session.project.add_track().unwrap();
    let copy = daw_core::Clip {
        id: daw_core::Id::new_v4(),
        ..session.project.tracks[0].clips[1].clone()
    };
    session
        .project
        .edit(Edit::InsertClip {
            track_id: second,
            clip: copy,
        })
        .unwrap();
    session.prepare_stretches().unwrap();
    assert_eq!(session.stretched_audio.len(), 1);
    let plan = session.plan();
    let mut live = daw_engine::Renderer::new(plan.clone());
    live.play();
    let block: Vec<_> = (0..7200).map(|_| live.next_sample()).collect();
    for frame in [1000, 3500, 6000] {
        let expected = plan.sample_at(frame as u64);
        for channel in 0..2 {
            assert!((block[frame][channel] - expected[channel]).abs() < 0.00001);
        }
    }
    let folder = fixture.0.join("project");
    session.save(&folder).unwrap();
    let loaded = Session::open(&folder).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.project.tracks).unwrap(),
        serde_json::to_value(&session.project.tracks).unwrap()
    );
    assert_eq!(loaded.stretched_audio.len(), 1);
    let before = fixture.0.join("before.wav");
    let after = fixture.0.join("after.wav");
    session.export(&before, false).unwrap();
    loaded.export(&after, false).unwrap();
    assert_eq!(fs::read(before).unwrap(), fs::read(after).unwrap());
    assert_eq!(fs::read(&source).unwrap(), original_file);
    // Old projects without the optional field retain normal source mapping.
    let mut json = serde_json::to_value(&loaded.project).unwrap();
    for track in json["tracks"].as_array_mut().unwrap() {
        for clip in track["clips"].as_array_mut().unwrap() {
            clip.as_object_mut().unwrap().remove("stretch");
        }
    }
    let old: Project = serde_json::from_value(json).unwrap();
    assert!(
        old.tracks
            .iter()
            .flat_map(|track| &track.clips)
            .all(|clip| clip.stretch.is_none())
    );
}

#[test]
fn stretch_overwrite_preserves_neighbor_audio_and_survives_save_render_and_reopen() {
    use daw_core::{Clip, ClipEdge, ClipStretch, Id};
    for edge in [ClipEdge::Left, ClipEdge::Right] {
        let fixture = Fixture::new();
        let source = fixture.wav("source.wav", 48000, 2, 16, false);
        let source_bytes = fs::read(&source).unwrap();
        let mut session = Session::default();
        session
            .import(
                &source,
                None,
                if edge == ClipEdge::Right { 400 } else { 1400 },
            )
            .unwrap();
        let primary = &mut session.project.tracks[0].clips[0];
        primary.length_frames = 600;
        let primary = primary.clone();
        let covered = Clip {
            id: Id::new_v4(),
            start_frame: 1050,
            length_frames: 200,
            source_offset_frame: 600,
            ..primary.clone()
        };
        let neighbor = Clip {
            id: Id::new_v4(),
            start_frame: if edge == ClipEdge::Right { 1400 } else { 400 },
            length_frames: 600,
            source_offset_frame: 1200,
            repeat: Some(ClipLoop {
                length_frames: 100,
                phase_frame: 25,
            }),
            stretch: ClipStretch::new(1, 2),
            ..primary.clone()
        };
        session.project.tracks[0]
            .clips
            .extend([covered.clone(), neighbor.clone()]);
        session.prepare_stretches().unwrap();
        let originals = session.audio[&primary.asset_id].samples.clone();
        let cached_neighbor = session.audio_for_clip(&neighbor).unwrap().samples.clone();
        let candidate = primary
            .stretched_to(if edge == ClipEdge::Right { 400 } else { 700 }, 1300)
            .unwrap();
        let preview = session
            .project
            .stretched_clips(std::slice::from_ref(&candidate), edge)
            .unwrap();
        session
            .project
            .edit(Edit::StretchClips {
                clips: vec![candidate],
                edge,
            })
            .unwrap();
        assert_eq!(format!("{:?}", session.project), format!("{preview:?}"));
        session.prepare_stretches().unwrap();
        assert_eq!(session.project.tracks[0].clips.len(), 2);
        let retained = session.project.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == neighbor.id)
            .unwrap();
        assert_eq!(retained.length_frames, 300);
        assert_eq!(retained.stretch, neighbor.stretch);
        for local in 0..300 {
            assert_eq!(
                retained.source_frame(local),
                neighbor.source_frame(local + retained.start_frame - neighbor.start_frame)
            );
        }
        assert!(std::sync::Arc::ptr_eq(
            &cached_neighbor,
            &session.audio_for_clip(retained).unwrap().samples
        ));
        assert!(std::sync::Arc::ptr_eq(
            &originals,
            &session.audio[&primary.asset_id].samples
        ));
        let mut before = vec![[0.0; 2]; session.project.end() as usize];
        session.plan().render(0, &mut before);
        session.save(&fixture.0.join("project")).unwrap();
        session
            .export(&fixture.0.join("before.wav"), false)
            .unwrap();
        let reopened = Session::open(&fixture.0.join("project")).unwrap();
        let mut after = vec![[0.0; 2]; before.len()];
        reopened.plan().render(0, &mut after);
        assert_eq!(before, after);
        assert!(
            !reopened.project.tracks[0]
                .clips
                .iter()
                .any(|clip| clip.id == covered.id)
        );
        reopened
            .export(&fixture.0.join("after.wav"), false)
            .unwrap();
        assert_eq!(
            fs::read(fixture.0.join("before.wav")).unwrap(),
            fs::read(fixture.0.join("after.wav")).unwrap()
        );
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
    }
}

#[test]
fn move_and_drop_middle_splits_preserve_audio_sources_caches_and_saved_results() {
    use daw_core::{Clip, ClipPlacement, ClipStretch, Id};
    for dropped in [false, true] {
        let fixture = Fixture::new();
        let source = fixture.wav("source.wav", 48000, 2, 16, false);
        let source_bytes = fs::read(&source).unwrap();
        let mut session = Session::default();
        session.import(&source, None, 0).unwrap();
        let neighbor = &mut session.project.tracks[0].clips[0];
        neighbor.length_frames = 12000;
        neighbor.source_offset_frame = 240;
        neighbor.repeat = Some(ClipLoop {
            length_frames: 600,
            phase_frame: 125,
        });
        neighbor.stretch = ClipStretch::new(1, 2);
        let neighbor = neighbor.clone();
        session.prepare_stretches().unwrap();
        let samples = session.audio[&neighbor.asset_id].samples.clone();
        let prepared = session.audio_for_clip(&neighbor).unwrap().samples.clone();
        let incoming_id = if dropped {
            let data = daw_media::decode_wav(&source).unwrap();
            session
                .import_decoded_overwrite(&source, data, Some(session.project.tracks[0].id), 3000)
                .unwrap()
        } else {
            let track = session.project.add_track().unwrap();
            let moving = Clip {
                id: Id::new_v4(),
                name: "Moved".into(),
                start_frame: 1000,
                source_offset_frame: 600,
                length_frames: 4800,
                repeat: None,
                ..neighbor.clone()
            };
            session
                .project
                .edit(Edit::InsertClip {
                    track_id: track,
                    clip: moving.clone(),
                })
                .unwrap();
            session
                .project
                .edit(Edit::OverwriteClips(vec![ClipPlacement {
                    track_id: session.project.tracks[0].id,
                    clip: Clip {
                        start_frame: 3000,
                        ..moving.clone()
                    },
                }]))
                .unwrap();
            assert!(session.project.tracks[1].clips.is_empty());
            moving.id
        };
        let pieces: Vec<_> = session.project.tracks[0]
            .clips
            .iter()
            .filter(|clip| clip.id != incoming_id)
            .collect();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].id, neighbor.id);
        assert_ne!(pieces[1].id, neighbor.id);
        assert_eq!((pieces[0].start_frame, pieces[0].end()), (0, 3000));
        assert_eq!((pieces[1].start_frame, pieces[1].end()), (7800, 12000));
        for piece in pieces {
            for local in 0..piece.length_frames {
                assert_eq!(
                    piece.source_frame(local),
                    neighbor.source_frame(local + piece.start_frame)
                );
            }
            assert!(std::sync::Arc::ptr_eq(
                &prepared,
                &session.audio_for_clip(piece).unwrap().samples
            ));
        }
        assert!(std::sync::Arc::ptr_eq(
            &samples,
            &session.audio[&neighbor.asset_id].samples
        ));
        let mut before = vec![[0.0; 2]; session.project.end() as usize];
        session.plan().render(0, &mut before);
        session.save(&fixture.0.join("project")).unwrap();
        session
            .export(&fixture.0.join("before.wav"), false)
            .unwrap();
        let reopened = Session::open(&fixture.0.join("project")).unwrap();
        let mut after = vec![[0.0; 2]; before.len()];
        reopened.plan().render(0, &mut after);
        assert_eq!(before, after);
        assert_eq!(
            reopened.project.tracks[0]
                .clips
                .iter()
                .map(|clip| clip.id)
                .collect::<Vec<_>>(),
            session.project.tracks[0]
                .clips
                .iter()
                .map(|clip| clip.id)
                .collect::<Vec<_>>()
        );
        reopened
            .export(&fixture.0.join("after.wav"), false)
            .unwrap();
        assert_eq!(
            fs::read(fixture.0.join("before.wav")).unwrap(),
            fs::read(fixture.0.join("after.wav")).unwrap()
        );
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
    }
}

#[test]
fn invalid_overwrite_import_retains_assets_clips_and_decoded_audio() {
    let fixture = Fixture::new();
    let source = fixture.wav("source.wav", 48000, 1, 16, false);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let before = format!("{:?}", session.project);
    let audio_count = session.audio.len();
    let data = daw_media::decode_wav(&source).unwrap();
    assert!(
        session
            .import_decoded_overwrite(&source, data.clone(), Some(daw_core::Id::new_v4()), 20)
            .is_err()
    );
    assert!(
        session
            .import_decoded_overwrite(&source, data, None, u64::MAX - 10)
            .is_err()
    );
    assert_eq!(format!("{:?}", session.project), before);
    assert_eq!(session.audio.len(), audio_count);
}

#[test]
fn mp3_export_converts_high_processing_rates_with_correct_duration() {
    use daw_media::{ExportFormat, ExportSettings};
    use symphonia::core::{
        codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
        meta::MetadataOptions, probe::Hint,
    };
    let fixture = Fixture::new();
    let source = fixture.wav("rates.wav", 96_000, 2, 16, false);
    let mut original = Session::default();
    original.import(&source, None, 0).unwrap();
    for rate in daw_core::SAMPLE_RATES {
        let session = original.with_audio_settings(rate, 64).unwrap();
        let output = fixture.0.join(format!("{rate}.mp3"));
        session
            .export_with_settings(
                &output,
                false,
                ExportSettings {
                    format: ExportFormat::Mp3,
                    ..Default::default()
                },
            )
            .unwrap();
        let mss = MediaSourceStream::new(
            Box::new(fs::File::open(output).unwrap()),
            Default::default(),
        );
        let mut hint = Hint::new();
        hint.with_extension("mp3");
        let mut format = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .unwrap()
            .format;
        let track = format.default_track().unwrap();
        let output_rate = rate.min(48_000);
        assert_eq!(track.codec_params.sample_rate, Some(output_rate));
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .unwrap();
        let mut frames = 0;
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => panic!("{error}"),
            };
            frames += decoder.decode(&packet).unwrap().frames();
        }
        assert_eq!(frames, output_rate as usize / 10, "processing rate {rate}");
    }
}

#[test]
fn midi_patterns_instruments_and_mixed_audio_round_trip_and_export_without_source_paths() {
    use daw_core::{MidiNote, MidiPattern, TrackKind, Waveform};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    session.project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
    session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .oscillators[0]
        .waveform = Waveform::Triangle;
    session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .oscillators
        .extend([
            daw_core::OscillatorSettings {
                waveform: Waveform::Square,
                transpose: -12,
                gain_db: -18.0,
                muted: true,
                ..Default::default()
            },
            daw_core::OscillatorSettings {
                waveform: Waveform::Saw,
                transpose: 12,
                gain_db: -24.0,
                ..Default::default()
            },
        ]);
    // Persist independent curve choices and retained amounts, including Linear's amount.
    let envelope = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .envelopes[0]
        .settings;
    envelope.attack.time_ms = 37.0;
    envelope.attack.shape = daw_core::CurveShape::Logarithmic;
    envelope.attack.amount = 0.7;
    envelope.decay.time_ms = 83.0;
    envelope.decay.shape = daw_core::CurveShape::SCurve;
    envelope.decay.amount = 0.3;
    envelope.sustain = 0.62;
    envelope.release.time_ms = 310.0;
    envelope.release.shape = daw_core::CurveShape::Linear;
    envelope.release.amount = 0.9;
    let mut second_layer = daw_core::InstrumentLayerSettings::default();
    second_layer.oscillators.push(daw_core::OscillatorSettings {
        waveform: daw_core::Waveform::Sine,
        transpose: -12,
        gain_db: -18.0,
        ..Default::default()
    });
    second_layer.install_amplitude_envelope();
    second_layer.envelopes[0].settings.attack.time_ms = 25.0;
    second_layer.envelopes[0].settings.release.time_ms = 700.0;
    session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers
        .push(second_layer);
    session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers
        .push(daw_core::InstrumentLayerSettings::default());
    let layers = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers;
    layers[0].name = "Lead 🎹".into();
    layers[1].name = "Bass".into();
    layers[2].name = "Unused".into();
    layers[0].gain_db = -3.0;
    layers[0].pan = -0.5;
    layers[1].gain_db = -6.0;
    layers[1].pan = 0.65;
    layers[0].soloed = true;
    layers[1].soloed = true;
    layers[2].muted = true;
    layers[0].envelopes[0].enabled = false;
    layers[1].oscillator_enabled = false;
    layers[0].collapsed = true;
    layers[0].oscillator_collapsed = true;
    layers[1].envelopes[0].collapsed = true;
    layers.swap(0, 1);
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: daw_core::Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 69,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    // Round-trip and export a chord through the same persisted source format.
    let mut chord = session
        .project
        .assets
        .last()
        .unwrap()
        .midi_pattern
        .clone()
        .unwrap();
    chord.notes.push(MidiNote {
        id: daw_core::Id::new_v4(),
        pitch: 72,
        ..chord.notes[0].clone()
    });
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: chord,
        })
        .unwrap();
    let source = fixture.wav("source.wav", 48_000, 2, 16, false);
    session.import(&source, None, 0).unwrap();
    let folder = fixture.0.join("project");
    session.save(&folder).unwrap();
    let restored = Session::open(&folder).unwrap();
    assert!(restored.warnings.is_empty());
    assert_eq!(restored.project.tracks[0].kind, TrackKind::Midi);
    assert_eq!(
        restored.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    assert_eq!(
        restored.project.assets[2].midi_pattern,
        session.project.assets[2].midi_pattern
    );
    let settings = ExportSettings {
        wav_codec: WavCodec::Float32,
        ..Default::default()
    };
    let first = fixture.0.join("first.wav");
    let second = fixture.0.join("second.wav");
    session
        .export_with_settings(&first, false, settings)
        .unwrap();
    restored
        .export_with_settings(&second, false, settings)
        .unwrap();
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let samples = hound::WavReader::open(&first)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(samples[12_000..24_000].iter().any(|x| x.abs() > 0.02));
    let mut save_as = restored;
    save_as.save(&fixture.0.join("another")).unwrap();
    assert!(
        Session::open(&fixture.0.join("another"))
            .unwrap()
            .warnings
            .is_empty()
    );
    assert_eq!(
        Session::open(&fixture.0.join("another"))
            .unwrap()
            .project
            .tracks[0]
            .instrument,
        save_as.project.tracks[0].instrument
    );
    let changed = save_as.with_audio_settings(44_100, 128).unwrap();
    assert_eq!(changed.project.tracks[0].clips[0].length_frames, 22_050);
    changed
        .export_with_settings(&fixture.0.join("resampled.wav"), false, settings)
        .unwrap();
    let mp3 = ExportSettings {
        format: daw_media::ExportFormat::Mp3,
        ..settings
    };
    changed
        .export_with_settings(&fixture.0.join("pattern.mp3"), false, mp3)
        .unwrap();
}

#[test]
fn silent_fader_gain_survives_save_reopen_playback_and_float_export() {
    use daw_core::SILENT_GAIN_DB;
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("silence-source.wav", 48_000, 2, 16, false);
    let bytes = fs::read(&source).unwrap();
    let mut original = Session::default();
    original.import(&source, None, 0).unwrap();
    assert_ne!(original.plan().sample_at(1000), [0.0; 2]);
    for master in [false, true] {
        let mut session = original.clone();
        if master {
            session.project.master.gain_db = SILENT_GAIN_DB;
        } else {
            session.project.tracks[0].gain_db = SILENT_GAIN_DB;
        }
        session.project.validate().unwrap();
        let folder = fixture.0.join(if master {
            "master-silent"
        } else {
            "track-silent"
        });
        session.save(&folder).unwrap();
        let loaded = Session::open(&folder).unwrap();
        assert_eq!(
            loaded.project.master.gain_db,
            session.project.master.gain_db
        );
        assert_eq!(
            loaded.project.tracks[0].gain_db,
            session.project.tracks[0].gain_db
        );
        assert!(!loaded.project.tracks[0].muted && !loaded.project.tracks[0].soloed);
        let plan = loaded.plan();
        assert_eq!(plan.sample_at(1000), [0.0; 2]);
        let mut renderer = daw_engine::Renderer::new(plan);
        renderer.play();
        for _ in 0..loaded.project.end() {
            assert_eq!(renderer.next_sample(), [0.0; 2]);
        }
        let output = fixture
            .0
            .join(if master { "master.wav" } else { "track.wav" });
        loaded
            .export_with_settings(
                &output,
                false,
                ExportSettings {
                    wav_codec: WavCodec::Float32,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut reader = hound::WavReader::open(output).unwrap();
        assert!(reader.samples::<f32>().all(|sample| sample.unwrap() == 0.0));
    }
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn silent_oscillator_survives_save_reopen_playback_and_float_export() {
    use daw_core::{MidiNote, MidiPattern, SILENT_GAIN_DB};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: daw_core::Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 69,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    let assets = serde_json::to_value(&session.project.assets).unwrap();
    let mut instrument: daw_core::InstrumentSettings =
        daw_core::OscillatorSettings::default().into();
    instrument.layers[0].oscillators[0].gain_db = SILENT_GAIN_DB;
    session
        .project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    let folder = fixture.0.join("silent-oscillator");
    session.save(&folder).unwrap();
    let mut loaded = Session::open(&folder).unwrap();
    assert!(loaded.warnings.is_empty());
    assert_eq!(
        loaded.project.tracks[0].instrument.as_ref().unwrap().layers[0].oscillators[0].gain_db,
        SILENT_GAIN_DB
    );
    assert_eq!(
        serde_json::to_value(&loaded.project.assets).unwrap(),
        assets
    );
    let mut renderer = daw_engine::Renderer::new(loaded.plan());
    renderer.play();
    for _ in 0..loaded.project.end() {
        assert_eq!(renderer.next_sample(), [0.0; 2]);
    }
    let output = fixture.0.join("silent-oscillator.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        hound::WavReader::open(output)
            .unwrap()
            .samples::<f32>()
            .all(|sample| sample.unwrap() == 0.0)
    );
    let mut instrument = loaded.project.tracks[0].instrument.clone().unwrap();
    instrument.layers[0].oscillators[0].gain_db = 0.0;
    loaded
        .project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    let mut renderer = daw_engine::Renderer::new(loaded.plan());
    renderer.play();
    assert!((0..loaded.project.end()).any(|_| {
        renderer
            .next_sample()
            .iter()
            .any(|sample| sample.abs() > 0.02)
    }));
    assert_eq!(
        serde_json::to_value(&loaded.project.assets).unwrap(),
        assets
    );
}

#[test]
fn eq_save_reopen_save_as_rate_change_and_float_export_use_the_same_settings_and_audio() {
    use daw_core::{EffectInstance, EffectSettings};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("eq-source.wav", 48000, 2, 32, true);
    let bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut eq = EffectInstance::equalizer();
    let EffectSettings::Eq3(settings) = &mut eq.unit else {
        unreachable!()
    };
    settings.collapsed = true;
    settings.bands[1].frequency_hz = 440.0;
    settings.bands[1].gain_db = 12.0;
    settings.bands[0].enabled = false;
    session
        .project
        .edit(Edit::SetEffects {
            track_id: session.project.tracks[0].id,
            effects: vec![eq.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("eq-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![eq.clone()]);
    assert!(loaded.project.tracks[0].effects_compact);
    let second = fixture.0.join("eq-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![eq.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.tracks[0].effects, vec![eq]);
    assert!(changed.project.tracks[0].effects_compact);
    let output = fixture.0.join("equalized.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded.clone();
    dry.project.tracks[0].effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(rendered[1000][0].abs() > dry_audio[1000][0].abs() * 2.0);
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn drive_save_reopen_save_as_rate_change_and_float_export_preserve_the_effect() {
    use daw_core::{DriveColorShape, EffectInstance, EffectSettings};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("drive-source.wav", 48000, 2, 32, true);
    let bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut drive = EffectInstance::drive();
    let EffectSettings::Drive(settings) = &mut drive.unit else {
        unreachable!()
    };
    settings.collapsed = true;
    settings.color_shape = DriveColorShape::MidHumpBassCut;
    settings.color_frequency_hz = 1250.0;
    settings.color_strength = 0.7;
    settings.gain = 0.9;
    settings.tone = 0.3;
    settings.volume_db = -9.0;
    session
        .project
        .edit(Edit::SetEffects {
            track_id: session.project.tracks[0].id,
            effects: vec![drive.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("drive-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![drive.clone()]);
    assert!(loaded.project.tracks[0].effects_compact);
    let second = fixture.0.join("drive-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![drive.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.tracks[0].effects, vec![drive]);
    let output = fixture.0.join("driven.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded.clone();
    dry.project.tracks[0].effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(
        rendered
            .iter()
            .zip(dry_audio)
            .any(|(wet, dry)| (wet[0] - dry[0]).abs() > 0.01)
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn auto_filter_save_reopen_rate_change_and_float_export_preserve_the_effect() {
    use daw_core::{EffectInstance, EffectSettings, LfoRateMode, LfoWave, NoteRate};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("auto-filter-source.wav", 48000, 2, 32, true);
    let bytes = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut drive = EffectInstance::auto_filter();
    let EffectSettings::AutoFilter(settings) = &mut drive.unit else {
        unreachable!()
    };
    settings.collapsed = true;
    settings.wave = LfoWave::SawDown;
    settings.rate_mode = LfoRateMode::Note;
    settings.note_rate = NoteRate(14);
    settings.strength_octaves = 2.0;
    settings.stereo_phase_degrees = -90.0;
    settings.sync_phase_degrees = 120.0;
    settings.frequency_hz = 300.0;
    settings.q = 2.0;
    settings.level_db = -6.0;
    settings.dry_wet = 0.8;
    session
        .project
        .edit(Edit::SetEffects {
            track_id: session.project.tracks[0].id,
            effects: vec![drive.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("auto-filter-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![drive.clone()]);
    assert!(loaded.project.tracks[0].effects_compact);
    let second = fixture.0.join("auto-filter-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![drive.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.tracks[0].effects, vec![drive]);
    let output = fixture.0.join("filtered.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded.clone();
    dry.project.tracks[0].effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(
        rendered
            .iter()
            .zip(dry_audio)
            .any(|(wet, dry)| (wet[0] - dry[0]).abs() > 0.01)
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn compressor_save_open_save_as_rate_change_and_export_keep_settings_and_audio() {
    use daw_core::{CompressorDetection, EffectInstance, EffectSettings};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("compressor-source.wav", 48000, 2, 32, true);
    let original = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut effect = EffectInstance::compressor();
    let EffectSettings::Compressor(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.threshold_db = -36.0;
    settings.detection = CompressorDetection::Rms;
    settings.attack_ms = 0.1;
    settings.knee_db = 12.0;
    settings.level_db = 3.0;
    settings.dry_wet = 0.75;
    settings.collapsed = true;
    session
        .project
        .edit(Edit::SetEffects {
            track_id: session.project.tracks[0].id,
            effects: vec![effect.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("compressor-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![effect.clone()]);
    assert!(loaded.project.tracks[0].effects_compact);
    let second = fixture.0.join("compressor-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![effect.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.tracks[0].effects, vec![effect]);
    let output = fixture.0.join("compressed.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded;
    dry.project.tracks[0].effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(
        rendered
            .iter()
            .zip(dry_audio)
            .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
    );
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn ott_save_open_save_as_rate_change_and_export_keep_settings_and_audio() {
    use daw_core::{CompressorDetection, EffectInstance, EffectSettings};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("ott-source.wav", 48000, 2, 32, true);
    let original = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut effect = EffectInstance::ott();
    let EffectSettings::Ott(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.low_frequency_hz = 150.0;
    settings.high_frequency_hz = 3500.0;
    settings.bands[1].depth = 0.6;
    settings.detection = CompressorDetection::Rms;
    settings.bands[0].attack_ms = 0.1;
    settings.bands[2].output_db = -3.0;
    settings.level_db = 3.0;
    settings.dry_wet = 0.75;
    settings.collapsed = true;
    session
        .project
        .edit(Edit::SetEffects {
            track_id: session.project.tracks[0].id,
            effects: vec![effect.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("ott-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![effect.clone()]);
    assert!(loaded.project.tracks[0].effects_compact);
    let second = fixture.0.join("ott-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.tracks[0].effects, vec![effect.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.tracks[0].effects, vec![effect]);
    let output = fixture.0.join("compressed.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded;
    dry.project.tracks[0].effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(
        rendered
            .iter()
            .zip(dry_audio)
            .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
    );
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn master_effects_save_open_rate_change_and_export_use_the_master_bus() {
    use daw_core::{CompressorDetection, EffectInstance, EffectSettings};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("compressor-source.wav", 48000, 2, 32, true);
    let original = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut effect = EffectInstance::compressor();
    let EffectSettings::Compressor(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.threshold_db = -36.0;
    settings.detection = CompressorDetection::Rms;
    settings.attack_ms = 0.1;
    settings.knee_db = 12.0;
    settings.level_db = 3.0;
    settings.dry_wet = 0.75;
    settings.collapsed = true;
    session
        .project
        .edit(Edit::SetMasterEffects {
            effects: vec![effect.clone()],
            compact: true,
        })
        .unwrap();
    let first = fixture.0.join("compressor-first");
    session.save(&first).unwrap();
    let mut loaded = Session::open(&first).unwrap();
    assert_eq!(loaded.project.master.effects, vec![effect.clone()]);
    assert!(loaded.project.master.effects_compact);
    let second = fixture.0.join("compressor-second");
    loaded.save(&second).unwrap();
    let loaded = Session::open(&second).unwrap();
    assert_eq!(loaded.project.master.effects, vec![effect.clone()]);
    let changed = loaded.with_audio_settings(96000, 128).unwrap();
    assert_eq!(changed.project.master.effects, vec![effect]);
    let output = fixture.0.join("compressed.wav");
    loaded
        .export_with_settings(
            &output,
            false,
            ExportSettings {
                wav_codec: WavCodec::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let exported: Vec<f32> = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
    daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
    assert_eq!(
        exported,
        rendered.iter().flatten().copied().collect::<Vec<_>>()
    );
    let mut dry = loaded;
    dry.project.master.effects.clear();
    let mut dry_audio = vec![[0.0; 2]; rendered.len()];
    daw_engine::Renderer::new(dry.plan()).render_offline(&mut dry_audio);
    assert!(
        rendered
            .iter()
            .zip(&dry_audio)
            .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
    );
    assert_eq!(fs::read(source).unwrap(), original);
    let missing = EffectInstance {
        id: daw_core::Id::new_v4(),
        unit: EffectSettings::Missing {
            type_id: "future.master".into(),
            state: serde_json::json!({"control": 7}),
        },
    };
    dry.project.master.effects.push(missing.clone());
    let future = fixture.0.join("future-master");
    dry.save(&future).unwrap();
    let restored = Session::open(&future).unwrap();
    assert_eq!(restored.project.master.effects, [missing]);
    assert!(
        restored
            .warnings
            .iter()
            .any(|warning| warning.contains("future.master on Master"))
    );
    let mut bypassed = vec![[0.0; 2]; dry_audio.len()];
    daw_engine::Renderer::new(restored.plan()).render_offline(&mut bypassed);
    assert_eq!(bypassed, dry_audio);
}

#[test]
fn limiter_save_open_rate_change_and_export_keep_the_ceiling_and_exact_frame_count() {
    use daw_core::{EffectInstance, EffectSettings, LimiterMode, LimiterRouting};
    use daw_media::{ExportSettings, WavCodec};
    let fixture = Fixture::new();
    let source = fixture.wav("limiter.wav", 48000, 2, 32, true);
    let original = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let mut effect = EffectInstance::limiter();
    let EffectSettings::Limiter(s) = &mut effect.unit else {
        unreachable!()
    };
    s.input_db = 24.0;
    s.ceiling_db = -6.0;
    s.lookahead_ms = 10.0;
    s.routing = LimiterRouting::MidSide;
    s.mode = LimiterMode::TruePeak;
    s.collapsed = true;
    session
        .project
        .edit(Edit::SetMasterEffects {
            effects: vec![effect.clone()],
            compact: false,
        })
        .unwrap();
    session.save(&fixture.0.join("limiter-first")).unwrap();
    let mut loaded = Session::open(&fixture.0.join("limiter-first")).unwrap();
    loaded.save(&fixture.0.join("limiter-second")).unwrap();
    let loaded = Session::open(&fixture.0.join("limiter-second")).unwrap();
    assert_eq!(loaded.project.master.effects, vec![effect.clone()]);
    for loaded in [
        loaded.clone(),
        loaded.with_audio_settings(96000, 128).unwrap(),
    ] {
        assert_eq!(loaded.project.master.effects, vec![effect.clone()]);
        let output = fixture
            .0
            .join(format!("limited-{}.wav", loaded.project.sample_rate_hz));
        loaded
            .export_with_settings(
                &output,
                false,
                ExportSettings {
                    wav_codec: WavCodec::Float32,
                    ..Default::default()
                },
            )
            .unwrap();
        let exported: Vec<f32> = hound::WavReader::open(output)
            .unwrap()
            .samples::<f32>()
            .map(Result::unwrap)
            .collect();
        assert_eq!(exported.len(), loaded.project.end() as usize * 2);
        assert!(
            exported
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10_f32.powf(-6.0 / 20.0))
        );
        let mut rendered = vec![[0.0; 2]; loaded.project.end() as usize];
        daw_engine::Renderer::new(loaded.plan()).render_offline(&mut rendered);
        assert_eq!(
            exported,
            rendered.iter().flatten().copied().collect::<Vec<_>>()
        );
    }
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn stretch_engine_switch_is_atomic_saved_and_cache_scoped() {
    use daw_core::StretchEngine;
    use std::sync::Arc;
    let fixture = Fixture::new();
    let source = fixture.wav("engine.wav", 48000, 2, 32, true);
    let original = fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let clip = &mut session.project.tracks[0].clips[0];
    clip.stretch = daw_core::ClipStretch::new(2, 3);
    session.prepare_stretches().unwrap();
    let clip = session.project.tracks[0].clips[0].clone();
    let key = (clip.asset_id, clip.stretch.unwrap());
    let mut integrated = session
        .with_processing_settings(48000, 512, StretchEngine::INTEGRATED)
        .unwrap();
    assert_eq!(session.project.stretch_engine, StretchEngine::RUBBER_BAND);
    assert_eq!(integrated.project.stretch_engine, StretchEngine::INTEGRATED);
    assert_eq!(
        format!("{:?}", session.project.tracks),
        format!("{:?}", integrated.project.tracks)
    );
    assert!(Arc::ptr_eq(
        &session.audio[&clip.asset_id].samples,
        &integrated.audio[&clip.asset_id].samples
    ));
    assert!(!Arc::ptr_eq(
        &session.stretched_audio[&key].samples,
        &integrated.stretched_audio[&key].samples
    ));
    let prepared = integrated.stretched_audio[&key].samples.clone();
    integrated.prepare_stretches().unwrap();
    assert!(Arc::ptr_eq(
        &prepared,
        &integrated.stretched_audio[&key].samples
    ));
    let mut first = vec![[0.0; 2]; 4800];
    integrated.plan().render(0, &mut first);
    integrated.save(&fixture.0.join("integrated")).unwrap();
    let reopened = Session::open(&fixture.0.join("integrated")).unwrap();
    assert_eq!(reopened.project.stretch_engine, StretchEngine::INTEGRATED);
    let mut rendered = vec![[0.0; 2]; 4800];
    reopened.plan().render(0, &mut rendered);
    assert_eq!(first, rendered);
    let back = integrated
        .with_processing_settings(48000, 512, StretchEngine::RUBBER_BAND)
        .unwrap();
    assert_eq!(
        *session.stretched_audio[&key].samples,
        *back.stretched_audio[&key].samples
    );
    // A changed source allocation invalidates even an otherwise identical cache key.
    integrated.audio.get_mut(&clip.asset_id).unwrap().samples = Arc::new(vec![[0.0; 2]; 4800]);
    assert!(integrated.audio_for_clip(&clip).is_none());
    assert!(integrated.plan().audio_for_clip(&clip).is_none());
    integrated.prepare_stretches().unwrap();
    assert!(
        integrated
            .audio_for_clip(&clip)
            .unwrap()
            .samples
            .iter()
            .all(|s| *s == [0.0; 2])
    );
    let before = format!("{:?}", session.project);
    assert!(
        session
            .with_processing_settings(
                48000,
                512,
                StretchEngine {
                    revision: 3,
                    ..StretchEngine::INTEGRATED
                }
            )
            .is_err()
    );
    assert_eq!(format!("{:?}", session.project), before);
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn cancelled_engine_preparation_retains_all_original_buffers() {
    use std::cell::Cell;
    let fixture = Fixture::new();
    let source = fixture.wav("cancel.wav", 48000, 2, 32, true);
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    session.project.tracks[0].clips[0].stretch = daw_core::ClipStretch::new(2, 3);
    session.prepare_stretches().unwrap();
    let clip = session.project.tracks[0].clips[0].clone();
    let key = (clip.asset_id, clip.stretch.unwrap());
    let buffer = session.stretched_audio[&key].samples.clone();
    let calls = Cell::new(0);
    let result = session.with_processing_settings_cancellable(
        48000,
        512,
        daw_core::StretchEngine::INTEGRATED,
        &|| {
            calls.set(calls.get() + 1);
            calls.get() > 6
        },
    );
    assert!(result.is_err());
    assert_eq!(
        session.project.stretch_engine,
        daw_core::StretchEngine::RUBBER_BAND
    );
    assert!(std::sync::Arc::ptr_eq(
        &buffer,
        &session.stretched_audio[&key].samples
    ));
}

#[test]
fn modulation_visibility_survives_save_reopen_save_as_and_legacy_defaults() {
    use daw_core::{
        AmplitudeSettings, AutoFilterSettings, EffectInstance, EffectSettings,
        InstrumentLayerSettings,
    };
    let fixture = Fixture::new();
    let mut session = Session::default();
    session.project.add_midi_track().unwrap();
    let layers = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers;
    layers[0].amplitude.present = true;
    layers[0].amplitude.modulation_open = true;
    layers[0].amplitude.collapsed = true;
    layers.push(InstrumentLayerSettings::default());
    layers[1].amplitude.present = true;
    let mut open = EffectInstance::auto_filter();
    let EffectSettings::AutoFilter(settings) = &mut open.unit else {
        unreachable!()
    };
    settings.modulation_open = true;
    settings.collapsed = true;
    session.project.tracks[0].effects = vec![open.clone(), EffectInstance::auto_filter()];
    open.id = daw_core::Id::new_v4();
    session.project.master.effects.push(open);
    let first = fixture.0.join("modulation-first");
    session.save(&first).unwrap();
    let mut reopened = Session::open(&first).unwrap();
    let second = fixture.0.join("modulation-copy");
    reopened.save(&second).unwrap();
    let copied = Session::open(&second).unwrap();
    for loaded in [&reopened, &copied] {
        assert_eq!(
            loaded.project.tracks[0].instrument,
            session.project.tracks[0].instrument
        );
        assert_eq!(
            loaded.project.tracks[0].effects,
            session.project.tracks[0].effects
        );
        assert_eq!(
            loaded.project.master.effects,
            session.project.master.effects
        );
    }
    // Older unit payloads omit the presentation field and must start closed.
    let mut amplitude = serde_json::to_value(AmplitudeSettings::default()).unwrap();
    amplitude.as_object_mut().unwrap().remove("modulation_open");
    assert!(
        !serde_json::from_value::<AmplitudeSettings>(amplitude.clone())
            .unwrap()
            .modulation_open
    );
    amplitude["modulation_open"] = serde_json::Value::String("open".into());
    assert!(serde_json::from_value::<AmplitudeSettings>(amplitude).is_err());
    let mut filter = serde_json::to_value(AutoFilterSettings::default()).unwrap();
    filter.as_object_mut().unwrap().remove("modulation_open");
    assert!(
        !serde_json::from_value::<AutoFilterSettings>(filter.clone())
            .unwrap()
            .modulation_open
    );
    filter["modulation_open"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AutoFilterSettings>(filter).is_err());
}

#[test]
fn drive_modulation_round_trips_save_as_and_defaults_legacy_settings() {
    use daw_core::{
        EffectInstance, EffectSettings, EnvelopeMode, Modulation, ModulationParameter,
        ModulationSourceId,
    };
    let fixture = Fixture::new();
    let mut session = Session::default();
    session.project.add_midi_track().unwrap();
    let layer = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.envelopes[0].present = true;
    layer.envelopes[0].mode = EnvelopeMode::Retrigger;
    let source = ModulationSourceId {
        layer: layer.id,
        unit: layer.envelopes[0].id,
    };
    let mut effect = EffectInstance::drive();
    let EffectSettings::Drive(settings) = &mut effect.unit else {
        panic!()
    };
    settings.modulation_open = true;
    settings.modulations = ModulationParameter::DRIVE
        .into_iter()
        .map(|p| {
            let mut r = Modulation::new(p);
            r.source = Some(source);
            r.amount = -p.limit();
            r
        })
        .collect();
    session.project.tracks[0].effects.push(effect);
    session.save(&fixture.0.join("drive-modulation")).unwrap();
    let mut reopened = Session::open(&fixture.0.join("drive-modulation")).unwrap();
    reopened
        .save(&fixture.0.join("drive-modulation-copy"))
        .unwrap();
    let copied = Session::open(&fixture.0.join("drive-modulation-copy")).unwrap();
    assert_eq!(
        copied.project.tracks[0].effects,
        session.project.tracks[0].effects
    );
    let duplicate = daw_core::Id::new_v4();
    reopened
        .project
        .edit(daw_core::Edit::DuplicateTrack {
            track_id: reopened.project.tracks[0].id,
            new_track_id: duplicate,
        })
        .unwrap();
    assert_ne!(
        reopened.project.tracks[0].effects[0].id,
        reopened.project.tracks[1].effects[0].id
    );
    assert_eq!(
        reopened.project.tracks[0].effects[0].unit,
        reopened.project.tracks[1].effects[0].unit
    );
    reopened.project.validate().unwrap();
    let mut legacy = serde_json::to_value(daw_core::DriveSettings::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("modulation_open");
    legacy.as_object_mut().unwrap().remove("modulations");
    let loaded: daw_core::DriveSettings = serde_json::from_value(legacy.clone()).unwrap();
    assert!(!loaded.modulation_open && loaded.modulations.is_empty() && loaded.valid());
    legacy["modulation_open"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<daw_core::DriveSettings>(legacy).is_err());
}

#[test]
fn lfo_instances_connections_and_presentation_survive_save_as_duplication_and_rate_changes() {
    use daw_core::{
        EffectInstance, EffectSettings, LfoInstance, LfoPolarity, LfoRestart, Modulation,
        ModulationParameter, ModulationSourceId,
    };
    let f = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let instrument = session.project.tracks[0].instrument.as_mut().unwrap();
    let layer = &mut instrument.layers[0];
    let mut lfo = LfoInstance {
        collapsed: true,
        enabled: false,
        ..Default::default()
    };
    lfo.settings.phase_degrees = -45.0;
    lfo.settings.polarity = LfoPolarity::Bipolar;
    lfo.settings.restart = LfoRestart::Legato;
    lfo.settings.smooth_ms = 17.0;
    lfo.settings.depth = 0.35;
    lfo.modulation_open = true;
    layer.envelopes[0].present = true;
    layer.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
    for parameter in [ModulationParameter::LfoDepth, ModulationParameter::LfoRate] {
        lfo.modulations.push(Modulation {
            source: Some(ModulationSourceId {
                layer: layer.id,
                unit: layer.envelopes[0].id,
            }),
            amount: 0.5,
            ..Modulation::new(parameter)
        });
    }

    let source = ModulationSourceId {
        layer: layer.id,
        unit: lfo.id,
    };
    layer.lfos.push(lfo);
    layer.amplitude.present = true;
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(source);
    route.amount = 0.7;
    layer.amplitude.modulations.push(route);
    let mut effect = EffectInstance::auto_filter();
    let EffectSettings::AutoFilter(settings) = &mut effect.unit else {
        panic!()
    };
    let mut route = Modulation::new(ModulationParameter::Frequency);
    route.source = Some(source);
    route.amount = -2.0;
    settings.modulations.push(route);
    session.project.tracks[0].effects.push(effect);
    session.save(&f.0.join("lfo")).unwrap();
    let mut opened = Session::open(&f.0.join("lfo")).unwrap();
    opened.save(&f.0.join("lfo-copy")).unwrap();
    let mut copy = Session::open(&f.0.join("lfo-copy")).unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    assert_eq!(
        copy.project.tracks[0].effects,
        session.project.tracks[0].effects
    );
    copy.project
        .edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: daw_core::Id::new_v4(),
        })
        .unwrap();
    copy.project.validate().unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        copy.project.tracks[1].instrument
    );
    let changed = copy.with_audio_settings(96000, 256).unwrap();
    assert_eq!(
        changed.project.tracks[0].instrument,
        copy.project.tracks[0].instrument
    );
}

#[test]
fn instrument_main_bus_and_shared_source_connections_survive_save_as_and_duplication() {
    use daw_core::{
        EnvelopeInstance, EnvelopeMode, LfoInstance, Modulation, ModulationParameter,
        ModulationSourceId,
    };
    let fixture = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let instrument = session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.gain_db = -8.0;
    instrument.main.pan = 0.3;
    instrument.main.collapsed = false;
    instrument.main.envelopes.push(EnvelopeInstance {
        mode: EnvelopeMode::Retrigger,
        ..Default::default()
    });
    instrument.main.lfos.push(LfoInstance::default());
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.envelopes[0].id,
    });
    route.amount = 0.5;
    instrument.layers[0].amplitude.present = true;
    instrument.layers[0].amplitude.modulations.push(route);
    session.save(&fixture.0.join("main")).unwrap();
    let mut opened = Session::open(&fixture.0.join("main")).unwrap();
    assert_eq!(
        opened.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    opened.save(&fixture.0.join("main-copy")).unwrap();
    let mut copy = Session::open(&fixture.0.join("main-copy")).unwrap();
    copy.project
        .edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: daw_core::Id::new_v4(),
        })
        .unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        copy.project.tracks[1].instrument
    );
    copy.project.validate().unwrap();
    let changed = copy.with_audio_settings(96000, 256).unwrap();
    assert_eq!(
        changed.project.tracks[0].instrument,
        copy.project.tracks[0].instrument
    );
}

#[test]
fn oscillator_pitch_ids_routes_and_open_controls_survive_save_as_duplicate_and_rate_changes() {
    use daw_core::{
        LfoInstance, Modulation, ModulationParameter, ModulationSourceId, OscillatorSettings,
    };
    let f = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let i = session.project.tracks[0].instrument.as_mut().unwrap();
    i.main.lfos.push(LfoInstance::default());
    let source = ModulationSourceId {
        layer: i.main.id,
        unit: i.main.lfos[0].id,
    };
    let l = &mut i.layers[0];
    l.oscillator_modulation_open = true;
    for amount in [0.25, -12.5] {
        let s = OscillatorSettings::default();
        let mut r = Modulation::new(ModulationParameter::Pitch);
        r.source = Some(source);
        r.target = Some(s.id);
        r.amount = amount;
        l.oscillators.push(s);
        l.oscillator_modulations.push(r);
    }
    session.save(&f.0.join("pitch")).unwrap();
    let mut opened = Session::open(&f.0.join("pitch")).unwrap();
    opened.save(&f.0.join("pitch-copy")).unwrap();
    let mut copy = Session::open(&f.0.join("pitch-copy")).unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    copy.project
        .edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: daw_core::Id::new_v4(),
        })
        .unwrap();
    copy.project.validate().unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        copy.project.tracks[1].instrument
    );
    let changed = copy.with_audio_settings(96000, 256).unwrap();
    assert_eq!(
        changed.project.tracks[0].instrument,
        copy.project.tracks[0].instrument
    );
}

#[test]
fn transferred_components_keep_connections_and_audio_through_save_as_and_reopen() {
    use daw_core::{
        InstrumentComponent, InstrumentLayerSettings, LfoInstance, Modulation, ModulationParameter,
        ModulationSourceId, OscillatorSettings,
    };
    let f = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let clip = session.project.add_midi_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: daw_core::MidiPattern {
                length_ticks: 960,
                notes: vec![daw_core::MidiNote {
                    id: daw_core::Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 57,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    session.project.tracks[0].clips[0].length_frames = 24_000;
    let i = session.project.tracks[0].instrument.as_mut().unwrap();
    i.layers.push(InstrumentLayerSettings::default());
    i.layers[0].lfos.push(LfoInstance::default());
    let owners = [i.layers[0].id, i.layers[1].id, i.main.id];
    let lfo = i.layers[0].lfos[0].id;
    let osc = OscillatorSettings::default();
    let mut route = Modulation::new(ModulationParameter::Pitch);
    route.source = Some(ModulationSourceId {
        layer: owners[0],
        unit: lfo,
    });
    route.target = Some(osc.id);
    route.amount = 7.0;
    let oscillator_id = osc.id;
    let second = OscillatorSettings {
        gain_db: -18.0,
        ..Default::default()
    };
    let mut second_route = route;
    second_route.id = daw_core::Id::new_v4();
    second_route.target = Some(second.id);
    second_route.amount = -3.0;
    i.layers[0].oscillators.extend([osc, second]);
    i.layers[0]
        .oscillator_modulations
        .extend([route, second_route]);
    let mut original = vec![[0.0; 2]; 24_000];
    daw_engine::Renderer::new(session.plan()).render_offline(&mut original);
    assert!(original.iter().any(|s| s[0].abs() > 0.01));
    for (source, destination, component) in [
        (owners[0], owners[1], InstrumentComponent::Oscillators),
        (owners[0], owners[2], InstrumentComponent::Lfo(lfo)),
    ] {
        session
            .project
            .edit(Edit::TransferInstrumentComponent {
                track_id: track,
                source,
                destination,
                destination_index: None,
                component,
                duplicate: false,
            })
            .unwrap();
    }
    session
        .project
        .edit(Edit::TransferInstrumentComponent {
            track_id: track,
            source: owners[1],
            destination: owners[1],
            destination_index: Some(1),
            component: InstrumentComponent::Oscillator(oscillator_id),
            duplicate: false,
        })
        .unwrap();
    let mut instrument = session.project.tracks[0].instrument.clone().unwrap();
    instrument.layers[1].oscillator_modulations.reverse();
    session
        .project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    let mut moved = vec![[0.0; 2]; 24_000];
    daw_engine::Renderer::new(session.plan()).render_offline(&mut moved);
    assert_eq!(moved, original);
    session.save(&f.0.join("transferred")).unwrap();
    let mut opened = Session::open(&f.0.join("transferred")).unwrap();
    opened.save(&f.0.join("transferred-copy")).unwrap();
    let copy = Session::open(&f.0.join("transferred-copy")).unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    let mut restored = vec![[0.0; 2]; 24_000];
    daw_engine::Renderer::new(copy.plan()).render_offline(&mut restored);
    assert_eq!(restored, moved);
    let settings = daw_media::ExportSettings {
        wav_codec: daw_media::WavCodec::Float32,
        ..Default::default()
    };
    let output = f.0.join("transferred.wav");
    copy.export_with_settings(&output, false, settings).unwrap();
    let samples = hound::WavReader::open(output)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        samples,
        moved
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect::<Vec<_>>()
    );
}

#[test]
fn macro_strips_routing_and_presentation_survive_save_as_duplication_and_rate_changes() {
    use daw_core::*;
    let f = Fixture::new();
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let i = session.project.tracks[0].instrument.as_mut().unwrap();
    let module = &mut i.main.macro_module;
    module.add();
    module.add();
    module.enabled = false;
    module.collapsed = true;
    module.modulation_open = true;
    module.controls[0].settings.value = 0.63;
    module.controls[0].name = Some("Brightness".into());
    module.controls[1].name = Some("Movement".into());
    module.controls[1].enabled = false;
    let source = ModulationSourceId {
        layer: i.main.id,
        unit: module.controls[0].id,
    };
    let lfo = LfoInstance::default();
    module.modulations.push(Modulation {
        target: Some(module.controls[1].id),
        amount: -0.6,
        source: Some(ModulationSourceId {
            layer: i.layers[0].id,
            unit: lfo.id,
        }),
        ..Modulation::new(ModulationParameter::MacroValue)
    });
    i.layers[0].lfos.push(lfo);
    i.layers[0].amplitude.modulations.push(Modulation {
        source: Some(source),
        amount: 1.0,
        ..Modulation::new(ModulationParameter::Amplitude)
    });
    i.layers[0].amplitude.present = true;
    assert_eq!(
        i.layers[0].move_component(InstrumentComponent::Amplitude, 0),
        Some(true)
    );
    i.main.lfos.push(LfoInstance::default());
    assert_eq!(
        i.main.move_component(InstrumentComponent::Macros, 0),
        Some(true)
    );
    let owner = i.main.id;
    let units = [
        InstrumentComponent::Macros,
        InstrumentComponent::Lfo(i.main.lfos[0].id),
    ];
    let group = session
        .project
        .component_group_transfer(track, owner, owner, &units, 0, true)
        .unwrap();
    session.project.edit(group.edit).unwrap();
    session.save(&f.0.join("macro")).unwrap();
    let mut reopened = Session::open(&f.0.join("macro")).unwrap();
    reopened.save(&f.0.join("macro-copy")).unwrap();
    let mut copy = Session::open(&f.0.join("macro-copy")).unwrap();
    assert_eq!(
        copy.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    copy.project
        .edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: Id::new_v4(),
        })
        .unwrap();
    let changed = copy.with_audio_settings(96_000, 256).unwrap();
    assert_eq!(
        changed.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    assert_eq!(
        changed.project.tracks[1].instrument,
        session.project.tracks[0].instrument
    );
    changed.project.validate().unwrap();
}

#[test]
fn effect_modulation_survives_save_as_duplication_and_processing_rate_changes() {
    use daw_core::{
        EffectInstance, EffectSettings, LfoInstance, Modulation, ModulationParameter,
        ModulationSourceId,
    };
    for kind in 0..7 {
        let fixture = Fixture::new();
        let mut session = Session::default();
        let track = session.project.add_midi_track().unwrap();
        let instrument = session.project.tracks[0].instrument.as_mut().unwrap();
        let lfo = LfoInstance::default();
        let source = ModulationSourceId {
            layer: instrument.main.id,
            unit: lfo.id,
        };
        instrument.main.lfos.push(lfo);
        let mut effect = [
            EffectInstance::equalizer,
            EffectInstance::compressor,
            EffectInstance::limiter,
            EffectInstance::ott,
            EffectInstance::direction_mixer,
            EffectInstance::stereo_spread,
            EffectInstance::bitcrusher,
        ][kind]();
        let (open, collapsed, routes) = match &mut effect.unit {
            EffectSettings::Eq3(s) => {
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::Compressor(s) => {
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::Limiter(s) => {
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::Bitcrusher(s) => {
                s.drive_db = 12.0;
                s.resolution_bits = 7;
                s.downsampling = 9;
                s.level_db = -6.0;
                s.dry_wet = 0.7;
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::StereoSpread(s) => {
                s.lower_intensity = 0.2;
                s.upper_intensity = 0.8;
                s.lower_frequency_hz = 500.0;
                s.upper_frequency_hz = 12000.0;
                s.order = 11;
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::DirectionMixer(s) => {
                s.input = daw_core::DirectionMixerInput::MidSide;
                s.direction_degrees = 30.0;
                s.spread = 0.6;
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            EffectSettings::Ott(s) => {
                (&mut s.modulation_open, &mut s.collapsed, &mut s.modulations)
            }
            _ => unreachable!(),
        };
        *open = true;
        *collapsed = true;
        *routes = [
            &ModulationParameter::EQUALIZER[..],
            &ModulationParameter::COMPRESSOR[..],
            &ModulationParameter::LIMITER[..],
            &ModulationParameter::OTT[..],
            &ModulationParameter::DIRECTION_MIXER[..],
            &ModulationParameter::STEREO_SPREAD[..],
            &ModulationParameter::BITCRUSHER[..],
        ][kind]
            .iter()
            .copied()
            .map(|parameter| Modulation {
                source: Some(source),
                amount: -parameter.limit() * 0.5,
                ..Modulation::new(parameter)
            })
            .collect();
        session.project.tracks[0].effects.push(effect.clone());
        session.save(&fixture.0.join("effect-modulation")).unwrap();
        let mut reopened = Session::open(&fixture.0.join("effect-modulation")).unwrap();
        reopened
            .save(&fixture.0.join("effect-modulation-copy"))
            .unwrap();
        let mut copied = Session::open(&fixture.0.join("effect-modulation-copy")).unwrap();
        assert_eq!(copied.project.tracks[0].effects, vec![effect.clone()]);
        copied
            .project
            .edit(daw_core::Edit::DuplicateTrack {
                track_id: track,
                new_track_id: daw_core::Id::new_v4(),
            })
            .unwrap();
        copied.project.validate().unwrap();
        assert_eq!(copied.project.tracks[1].effects[0].unit, effect.unit);
        assert_ne!(copied.project.tracks[1].effects[0].id, effect.id);
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let changed = copied.with_audio_settings(rate, 256).unwrap();
            changed.project.validate().unwrap();
            for index in 0..2 {
                assert_eq!(
                    changed.project.tracks[index].effects,
                    copied.project.tracks[index].effects
                );
            }
        }
    }
}

#[test]
fn sampler_files_regions_durations_and_root_metadata_survive_storage_and_rate_changes() {
    use daw_core::{SamplerMode, SamplerSettings, SamplerTrigger};
    let fixture = Fixture::new();
    let path = fixture.wav("sampler.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    let layer = &mut session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.sampler = Some(SamplerSettings::default());
    let owner = layer.id;
    session = session.with_sampler_file(track, owner, &path).unwrap();
    let id = session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .sampler
        .as_ref()
        .unwrap()
        .id;
    assert_eq!(session.audio[&id].samples.len(), 4800);
    assert!(session.project.tracks[0].clips.is_empty());
    let before = serde_json::to_value(&session.project).unwrap();
    assert!(
        session
            .with_sampler_file(track, owner, &fixture.0.join("missing.wav"))
            .is_err()
    );
    assert_eq!(serde_json::to_value(&session.project).unwrap(), before);
    let mut instrument = session.project.tracks[0].instrument.clone().unwrap();
    instrument.main.lfos.push(daw_core::LfoInstance::default());
    let mut route = daw_core::Modulation::new(daw_core::ModulationParameter::SamplerGain);
    route.source = Some(daw_core::ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.lfos[0].id,
    });
    route.amount = -12.0;
    let s = instrument.layers[0].sampler.as_mut().unwrap();
    s.start_frame = 1200;
    s.end_frame = 3600;
    s.mode = SamplerMode::Time;
    s.time_ms = 100.0;
    s.root_note = 61;
    s.trigger = SamplerTrigger::Gate;
    s.collapsed = true;
    s.modulation_open = true;
    s.modulations.push(route);
    session = session
        .with_edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    assert_eq!(session.audio[&id].samples.len(), 4800);
    session.save(&fixture.0.join("sampler-project")).unwrap();
    let mut reopened = Session::open(&fixture.0.join("sampler-project")).unwrap();
    reopened.save(&fixture.0.join("sampler-copy")).unwrap();
    let copied = Session::open(&fixture.0.join("sampler-copy")).unwrap();
    assert_eq!(
        copied.project.tracks[0].instrument,
        session.project.tracks[0].instrument
    );
    for rate in daw_core::SAMPLE_RATES {
        let changed = copied.with_audio_settings(rate, 256).unwrap();
        let s = changed.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .sampler
            .as_ref()
            .unwrap();
        assert_eq!(s.root_note, 61);
        assert_eq!(s.start_frame, (f64::from(rate) / 40.0).round() as u64);
        assert_eq!(changed.audio[&id].samples.len(), rate as usize / 10);
        assert_eq!(s.trigger, SamplerTrigger::Gate);
        assert!(s.modulation_open);
        assert_eq!(s.modulations, vec![route]);
    }
    let mut instrument = copied.project.tracks[0].instrument.clone().unwrap();
    let s = instrument.layers[0].sampler.as_mut().unwrap();
    s.mode = SamplerMode::Bars;
    s.start_frame = 0;
    s.end_frame = 48000;
    s.input.as_mut().unwrap().length_frames = 48000;
    s.input.as_mut().unwrap().repeat = Some(daw_core::ClipLoop {
        length_frames: 4800,
        phase_frame: 0,
    });
    s.bars = 1;
    let bars = copied
        .with_edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument),
        })
        .unwrap();
    let slower = bars.with_edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
    assert_eq!(
        slower.audio[&id].samples.len(),
        bars.audio[&id].samples.len() * 2
    );
    let mut invalid = bars.project.tracks[0].instrument.clone().unwrap();
    invalid.layers[0].sampler.as_mut().unwrap().bars = 100;
    assert!(
        bars.with_edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(invalid)
        })
        .is_err()
    );
    fs::remove_file(&path).unwrap();
    let missing = Session::open(&fixture.0.join("sampler-copy")).unwrap();
    assert!(!missing.audio.contains_key(&id));
    assert!(!missing.warnings.is_empty());
}

#[test]
fn sampler_clip_assignment_reuses_trim_repeat_and_stretch_mapping_without_changing_arrangement() {
    let fixture = Fixture::new();
    let path = fixture.wav("region.wav", 48000, 2, 16, false);
    let mut session = Session::default();
    let audio_track = session.project.add_track().unwrap();
    let clip_id = session.import(&path, Some(audio_track), 0).unwrap();
    let track = session.project.add_midi_track().unwrap();
    let layer = &mut session.project.tracks[1]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.sampler = Some(Default::default());
    let owner = layer.id;
    let mut clip = session.project.tracks[0].clips[0].clone();
    clip.source_offset_frame = 480;
    clip.length_frames = 6000;
    clip.repeat = Some(ClipLoop {
        length_frames: 2400,
        phase_frame: 300,
    });
    clip.stretch = daw_core::ClipStretch::new(1, 2);
    session.prepare_stretches().unwrap();
    // Assigning a stretched dropped clip prepares its shared source cache as part of the job.
    let assigned = session.with_sampler_clip(track, owner, &clip).unwrap();
    let s = assigned.project.tracks[1]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .sampler
        .as_ref()
        .unwrap();
    let source = assigned.audio_for_clip(&clip).unwrap();
    for local in [0, 1000, 2300, 4900, 5999] {
        assert_eq!(
            assigned.audio[&s.id].samples[local],
            source.samples[clip.source_frame(local as u64) as usize]
        );
    }
    assert_eq!(assigned.project.tracks[0].clips[0].id, clip_id);
    assert_eq!(assigned.project.tracks[0].clips[0].length_frames, 4800);
    assert_eq!(assigned.project.assets.len(), session.project.assets.len());
}
