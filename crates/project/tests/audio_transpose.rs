use daw_core::{ClipLoop, ClipStretch, Edit, Id, StretchEngine};
use daw_engine::{Renderer, TransportState};
use daw_project::{Session, SessionHistory};
use std::{fs, path::PathBuf, sync::Arc};

struct Fixture(PathBuf);
impl Fixture {
    fn new(channels: u16) -> Self {
        let path = std::env::temp_dir().join(format!("daw-clip-pitch-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        let mut wav = hound::WavWriter::create(
            path.join("tone.wav"),
            hound::WavSpec {
                channels,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for frame in 0..24000 {
            let value = (frame as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.2;
            wav.write_sample(value).unwrap();
            if channels == 2 {
                wav.write_sample(-value).unwrap();
            }
        }
        wav.finalize().unwrap();
        Self(path)
    }
    fn session(&self, engine: StretchEngine) -> Session {
        let mut session = Session::default();
        session.project.stretch_engine = engine;
        session.import(&self.0.join("tone.wav"), None, 0).unwrap();
        session
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn transpose(session: &Session, value: i8) -> Session {
    session
        .with_edit(Edit::SetClipTranspose {
            clip_id: session.project.tracks[0].clips[0].id,
            transpose: value,
        })
        .unwrap()
}
fn frequency(samples: &[[f32; 2]], rate: u32) -> f64 {
    let middle = &samples[samples.len() / 4..samples.len() * 3 / 4];
    middle
        .windows(2)
        .filter(|pair| pair[0][0] <= 0.0 && pair[1][0] > 0.0)
        .count() as f64
        * f64::from(rate)
        / middle.len() as f64
}

#[test]
fn audio_clip_transpose_uses_both_engines_preserves_duration_stereo_and_render_paths() {
    for channels in [1, 2] {
        let fixture = Fixture::new(channels);
        let original_file = fs::read(fixture.0.join("tone.wav")).unwrap();
        for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
            for stretch in [None, ClipStretch::new(3, 2)] {
                let mut base = fixture.session(engine);
                let clip = &mut base.project.tracks[0].clips[0];
                clip.stretch = stretch;
                clip.length_frames = stretch.map_or(24000, |s| s.scale(24000).unwrap());
                base.prepare_stretches().unwrap();
                for pitch in [-12, 12] {
                    let changed = transpose(&base, pitch);
                    let clip = &changed.project.tracks[0].clips[0];
                    assert_eq!(
                        clip.length_frames,
                        base.project.tracks[0].clips[0].length_frames
                    );
                    assert_eq!(clip.source_offset_frame, 0);
                    assert_eq!(clip.stretch, stretch);
                    let audio = changed.audio_for_clip(clip).unwrap();
                    assert_eq!(audio.samples.len() as u64, clip.length_frames);
                    let expected = 440.0 * 2.0_f64.powf(f64::from(pitch) / 12.0);
                    assert!(
                        (frequency(&audio.samples, 48000) - expected).abs() < 6.0,
                        "{engine:?}, {pitch}, {stretch:?}"
                    );
                    if channels == 2 {
                        assert!(audio.samples.iter().all(|s| (s[0] + s[1]).abs() < 0.0001));
                    }
                    let plan = changed.plan();
                    assert!(Arc::ptr_eq(
                        &audio.samples,
                        &plan.audio_for_clip(clip).unwrap().samples
                    ));
                    assert!(Arc::ptr_eq(
                        &audio.peaks,
                        &plan.audio_for_clip(clip).unwrap().peaks
                    ));
                    let mut direct = vec![[0.0; 2]; clip.length_frames as usize];
                    plan.render(0, &mut direct);
                    let mut offline = vec![[0.0; 2]; direct.len()];
                    Renderer::new(plan.clone()).render_offline(&mut offline);
                    assert_eq!(direct, offline);
                    let mut live = Renderer::new(plan);
                    live.prepare_transport(TransportState {
                        frame: 3000,
                        playing: true,
                        ..Default::default()
                    });
                    for (frame, expected) in direct.iter().enumerate().take(5000).skip(3000) {
                        let sample = live.next_sample();
                        if frame >= 3300 {
                            assert_eq!(sample, *expected);
                        }
                    }
                    assert!(Arc::ptr_eq(
                        &base.audio[&clip.asset_id].samples,
                        &changed.audio[&clip.asset_id].samples
                    ));
                }
            }
        }
        assert_eq!(fs::read(fixture.0.join("tone.wav")).unwrap(), original_file);
    }
}

#[test]
fn audio_clip_pitch_variants_share_matching_requests_and_invalidate_source_engine_and_rate() {
    let fixture = Fixture::new(2);
    let mut base = fixture.session(StretchEngine::RUBBER_BAND);
    let first = base.project.tracks[0].clips[0].clone();
    for (index, pitch) in [12, -12, 12].into_iter().enumerate() {
        let mut clip = first.clone();
        clip.id = Id::new_v4();
        clip.start_frame = (index as u64 + 1) * first.length_frames;
        clip.transpose = pitch;
        base.project.tracks[0].clips.push(clip);
    }
    assert!(base.audio_preparation_needed(&base.project));
    base.prepare_stretches().unwrap();
    assert_eq!(base.stretched_audio.len(), 2);
    let clips = base.project.tracks[0].clips.clone();
    assert!(Arc::ptr_eq(
        &base.audio_for_clip(&clips[1]).unwrap().samples,
        &base.audio_for_clip(&clips[3]).unwrap().samples
    ));
    assert!(!Arc::ptr_eq(
        &base.audio_for_clip(&clips[1]).unwrap().samples,
        &base.audio_for_clip(&clips[2]).unwrap().samples
    ));
    let original = base.audio_for_clip(&clips[1]).unwrap().samples.clone();
    base.prepare_stretches().unwrap();
    assert!(Arc::ptr_eq(
        &original,
        &base.audio_for_clip(&clips[1]).unwrap().samples
    ));
    assert!(!base.audio_preparation_needed(&base.project));
    let mut changed = base
        .with_processing_settings(48000, 512, StretchEngine::INTEGRATED)
        .unwrap();
    assert!(!Arc::ptr_eq(
        &original,
        &changed.audio_for_clip(&clips[1]).unwrap().samples
    ));
    let back = changed
        .with_processing_settings(48000, 512, StretchEngine::RUBBER_BAND)
        .unwrap();
    assert_eq!(*original, *back.audio_for_clip(&clips[1]).unwrap().samples);
    let rate = changed.with_audio_settings(96000, 128).unwrap();
    let clip = &rate.project.tracks[0].clips[1];
    assert_eq!(rate.audio_for_clip(clip).unwrap().sample_rate_hz, 96000);
    assert!((frequency(&rate.audio_for_clip(clip).unwrap().samples, 96000) - 880.0).abs() < 6.0);
    changed.audio.get_mut(&first.asset_id).unwrap().samples = Arc::new(vec![[0.0; 2]; 24000]);
    assert!(changed.audio_for_clip(&clips[1]).is_none());
    assert!(changed.plan().audio_for_clip(&clips[1]).is_none());
    changed.prepare_stretches().unwrap();
    assert!(
        changed
            .audio_for_clip(&clips[1])
            .unwrap()
            .samples
            .iter()
            .all(|s| *s == [0.0; 2])
    );
    let key = daw_media::ClipAudioKey::for_clip(&clips[1]).unwrap();
    changed.stretched_audio.remove(&key);
    assert!(changed.audio_for_clip(&clips[1]).is_none());
}

#[test]
fn audio_clip_pitch_roundtrips_export_history_and_retains_trim_repeat_coordinates() {
    let fixture = Fixture::new(2);
    for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
        let mut base = fixture.session(engine);
        let clip = &mut base.project.tracks[0].clips[0];
        clip.source_offset_frame = 123;
        clip.length_frames = 18000;
        clip.repeat = Some(ClipLoop {
            length_frames: 8000,
            phase_frame: 579,
        });
        clip.gain_db = -3.0;
        let mut history = SessionHistory::new(&base, true).unwrap();
        let mut changed = transpose(&base, 12);
        let lowered = transpose(&changed, -12);
        assert!(
            (frequency(
                &lowered
                    .audio_for_clip(&lowered.project.tracks[0].clips[0])
                    .unwrap()
                    .samples,
                48000
            ) - 220.0)
                .abs()
                < 6.0
        );
        history.record(&changed).unwrap();
        let clip = changed.project.tracks[0].clips[0].clone();
        assert_eq!(
            (clip.source_offset_frame, clip.repeat, clip.length_frames),
            (123, base.project.tracks[0].clips[0].repeat, 18000)
        );
        let mut rendered = vec![[0.0; 2]; 18000];
        changed.plan().render(0, &mut rendered);
        let undone = history.undo(&changed).unwrap();
        assert_eq!(undone.project.tracks[0].clips[0].transpose, 0);
        let redone = history.redo(&undone).unwrap();
        assert!(Arc::ptr_eq(
            &changed.audio_for_clip(&clip).unwrap().samples,
            &redone.audio_for_clip(&clip).unwrap().samples
        ));
        let folder = fixture.0.join(format!("{:?}", engine.id));
        changed.save(&folder).unwrap();
        let loaded = Session::open(&folder).unwrap();
        assert_eq!(loaded.project.tracks[0].clips[0].transpose, 12);
        assert_eq!(loaded.project.stretch_engine, engine);
        let mut reopened = vec![[0.0; 2]; 18000];
        loaded.plan().render(0, &mut reopened);
        assert_eq!(rendered, reopened);
        let export = folder.join("render.wav");
        loaded
            .export_with_settings(
                &export,
                false,
                daw_media::ExportSettings {
                    wav_codec: daw_media::WavCodec::Float32,
                    ..Default::default()
                },
            )
            .unwrap();
        let file: Vec<f32> = hound::WavReader::open(&export)
            .unwrap()
            .samples::<f32>()
            .map(Result::unwrap)
            .collect();
        assert_eq!(file, rendered.iter().flatten().copied().collect::<Vec<_>>());
        let reset = transpose(&loaded, 0);
        assert!(reset.stretched_audio.is_empty());
        assert!(Arc::ptr_eq(
            &reset
                .audio_for_clip(&reset.project.tracks[0].clips[0])
                .unwrap()
                .samples,
            &reset.audio[&clip.asset_id].samples
        ));
    }
}

#[test]
fn audio_clip_pitch_failure_and_cancellation_never_publish_partial_variants() {
    let fixture = Fixture::new(2);
    for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
        let base = transpose(&fixture.session(engine), 12);
        let clip = base.project.tracks[0].clips[0].clone();
        let old = base.audio_for_clip(&clip).unwrap().samples.clone();
        let calls = std::cell::Cell::new(0);
        assert!(
            base.with_edit_cancellable(
                Edit::SetClipTranspose {
                    clip_id: clip.id,
                    transpose: -12
                },
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() > 6
                }
            )
            .is_err()
        );
        assert_eq!(base.project.tracks[0].clips[0].transpose, 12);
        assert!(Arc::ptr_eq(
            &old,
            &base.audio_for_clip(&clip).unwrap().samples
        ));
        let mut invalid = base.clone();
        invalid.audio.get_mut(&clip.asset_id).unwrap().samples =
            Arc::new(vec![[f32::NAN; 2]; 24000]);
        assert!(
            invalid
                .with_edit(Edit::SetClipTranspose {
                    clip_id: clip.id,
                    transpose: 24
                })
                .is_err()
        );
        assert_eq!(invalid.project.tracks[0].clips[0].transpose, 12);
        assert_eq!(invalid.stretched_audio.len(), 1);
        assert!(Arc::ptr_eq(
            &old,
            &invalid.stretched_audio.values().next().unwrap().samples
        ));
        for pitch in [-48, 48] {
            let end = transpose(&base, pitch);
            let audio = end.audio_for_clip(&end.project.tracks[0].clips[0]).unwrap();
            assert_eq!(audio.samples.len(), 24000);
            assert!(audio.samples.iter().flatten().all(|s| s.is_finite()));
        }
    }
}
