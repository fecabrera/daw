use daw_core::{Edit, Id};
use daw_project::{Session, SessionHistory};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-audio-merge-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        let mut writer = hound::WavWriter::create(
            path.join("original.wav"),
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for frame in 0..6000 {
            let value = (frame as f32 * 0.07).sin() * 0.2;
            writer.write_sample(value).unwrap();
            writer.write_sample(-value).unwrap();
        }
        writer.finalize().unwrap();
        Self(path)
    }
    fn session(&self) -> (Session, Vec<Id>) {
        let mut s = Session::default();
        let a = s.import(&self.0.join("original.wav"), None, 1000).unwrap();
        let track = s.project.tracks[0].id;
        let b = s
            .import(&self.0.join("original.wav"), Some(track), 12_000)
            .unwrap();
        s.project.tracks[0].clips[0].gain_db = -3.0;
        s.project.tracks[0].clips[1].channels.inverted = true;
        (s, vec![a, b])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn render(s: &Session) -> Vec<[f32; 2]> {
    let mut out = vec![[0.; 2]; s.project.end() as usize];
    daw_engine::Renderer::new(s.plan()).render_offline(&mut out);
    out
}
fn close(a: &[[f32; 2]], b: &[[f32; 2]]) {
    assert_eq!(a.len(), b.len());
    assert!(
        a.iter()
            .zip(b)
            .all(|(a, b)| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6))
    );
}
#[test]
fn merge_save_open_save_as_collect_and_history_retain_leaf_sources() {
    let f = Fixture::new();
    let (base, ids) = f.session();
    let bytes = fs::read(f.0.join("original.wav")).unwrap();
    let before = render(&base);
    let (mut merged, id) = base.merged_audio_clips(&ids, &|| false).unwrap();
    close(&before, &render(&merged));
    assert_eq!(merged.audio.len(), base.audio.len());
    assert_eq!(merged.compound_views.len(), 1);
    let mut without_views = merged.clone();
    without_views.compound_views.clear();
    assert!(without_views.audio_preparation_needed(&without_views.project));
    without_views.prepare_stretches().unwrap();
    assert!(!without_views.audio_preparation_needed(&without_views.project));
    let mut history = SessionHistory::new(&base, false).unwrap();
    history.record(&merged).unwrap();
    merged.save(&f.0.join("project")).unwrap();
    let reopened = Session::open(&f.0.join("project")).unwrap();
    close(&before, &render(&reopened));
    assert_eq!(reopened.project.tracks[0].clips[0].id, id);
    let outcome = reopened
        .source_collection_snapshot(None, 1)
        .unwrap()
        .preflight(None, &|| false)
        .unwrap()
        .into_plan()
        .unwrap()
        .execute(&|_| true, &|| false, &mut |_| Ok(()))
        .unwrap();
    let mut collected = reopened.clone();
    outcome.apply(&mut collected, None, 1).unwrap();
    collected
        .save_snapshot(None)
        .unwrap()
        .save(&f.0.join("copy"))
        .unwrap();
    fs::remove_file(f.0.join("original.wav")).unwrap();
    let moved = Session::open(&f.0.join("copy")).unwrap();
    close(&before, &render(&moved));
    assert!(moved.warnings.is_empty(), "{:?}", moved.warnings);
    assert!(!bytes.is_empty());
    assert_eq!(history.undo(&merged).unwrap().project, base.project);
}
#[test]
fn extraction_normalize_transpose_rate_and_cancel_are_worker_transactions() {
    let f = Fixture::new();
    let (base, ids) = f.session();
    let original = base.project.clone();
    assert!(base.merged_audio_clips(&ids, &|| true).is_err());
    assert_eq!(base.project, original);
    let (merged, id) = base.merged_audio_clips(&ids, &|| false).unwrap();
    let clip = &merged.project.tracks[0].clips[0];
    let source = merged.render_clip_source(clip, &|| false).unwrap();
    assert_eq!(source.frame_count(), 17_000);
    assert!(source.samples[6000..11_000].iter().all(|v| *v == [0.; 2]));
    let edits = merged
        .normalize_clips(&[id], Default::default(), &Default::default())
        .unwrap();
    assert_eq!(edits.len(), 1);
    let shifted = merged
        .with_edit(Edit::SetClipTranspose {
            clip_id: id,
            transpose: 3,
        })
        .unwrap();
    assert!(
        shifted
            .audio_for_clip(&shifted.project.tracks[0].clips[0])
            .is_some()
    );
    assert_eq!(shifted.project.tracks[0].clips[0].length_frames, 17_000);
    for rate in daw_core::SAMPLE_RATES {
        let next = merged.with_audio_settings(rate, 512).unwrap();
        assert_eq!(next.project.sample_rate_hz, rate);
        assert!(render(&next).iter().flatten().all(|x| x.is_finite()));
    }
    assert_eq!(merged.project.tracks[0].clips[0].id, id);
}

#[test]
fn huge_silent_gap_stays_sparse_and_oversized_linearization_fails_atomically() {
    let f = Fixture::new();
    let (mut base, ids) = f.session();
    base.project.tracks[0].clips[1].start_frame = 1_000_000_000_000;
    let (mut merged, id) = base.merged_audio_clips(&ids, &|| false).unwrap();
    assert_eq!(merged.audio.len(), base.audio.len());
    assert_eq!(
        merged
            .compound_views
            .values()
            .next()
            .unwrap()
            .track
            .clips
            .len(),
        2
    );
    let before = merged.project.clone();
    let audio_before = merged.audio.clone();
    assert!(
        merged
            .with_edit(Edit::SetClipTranspose {
                clip_id: id,
                transpose: 1
            })
            .is_err()
    );
    assert_eq!(merged.project, before);
    assert!(merged.prepare_stretches_cancellable(&|| true).is_err());
    assert_eq!(merged.audio.len(), audio_before.len());
    for (id, a) in audio_before {
        assert!(std::sync::Arc::ptr_eq(
            &a.samples,
            &merged.audio[&id].samples
        ));
    }
}

#[test]
fn sampler_uses_captured_edits_and_silence_without_track_processing() {
    let f = Fixture::new();
    let (base, ids) = f.session();
    let (mut merged, _) = base.merged_audio_clips(&ids, &|| false).unwrap();
    merged.project.tracks[0].gain_db = -30.0;
    let clip = merged.project.tracks[0].clips[0].clone();
    let source = merged.render_clip_source(&clip, &|| false).unwrap();
    let track = merged.project.add_midi_track().unwrap();
    let layer = &mut merged.project.tracks[1].instrument.as_mut().unwrap().layers[0];
    layer.clear_sources();
    layer.sampler = Some(Default::default());
    let owner = layer.id;
    let next = merged.with_sampler_clip(track, owner, &clip).unwrap();
    let sampler = next.project.tracks[1].instrument.as_ref().unwrap().layers[0]
        .sampler
        .as_ref()
        .unwrap();
    assert_eq!(sampler.input.as_ref().unwrap().asset_id, clip.asset_id);
    let prepared = next.sampler_audio[&sampler.id].root_audio();
    assert_eq!(prepared.frame_count(), source.frame_count());
    assert!(prepared.samples[7000..10000].iter().all(|s| *s == [0.; 2]));
    assert!(prepared.samples.iter().flatten().any(|v| v.abs() > 0.05));
}

#[test]
fn transformed_compound_keeps_context_handles_and_publishes_only_complete_buffers() {
    let f = Fixture::new();
    let (mut base, _) = f.session();
    let original = base.project.tracks[0].clips[0].clone();
    base.project.tracks[0].clips.clear();
    for index in 0..4 {
        let mut clip = original.clone();
        clip.id = Id::new_v4();
        clip.start_frame = index * 1000;
        clip.source_offset_frame = 2000;
        clip.length_frames = 1000;
        base.project.tracks[0].clips.push(clip);
    }
    base.project.tracks[0].clips[1].fades.fade_in = daw_core::ClipFade {
        length_frames: 200,
        crossfade: true,
        ..Default::default()
    };
    base.project.tracks[0].clips[2].fades.fade_out = daw_core::ClipFade {
        length_frames: 200,
        crossfade: true,
        ..Default::default()
    };
    let ids: Vec<_> = base.project.tracks[0].clips[1..3]
        .iter()
        .map(|c| c.id)
        .collect();
    let (merged, id) = base.merged_audio_clips(&ids, &|| false).unwrap();
    let requested = merged
        .project
        .edited(Edit::SetClipTranspose {
            clip_id: id,
            transpose: 3,
        })
        .unwrap();
    assert!(merged.audio_preparation_needed(&requested));
    let shifted = merged
        .with_edit(Edit::SetClipTranspose {
            clip_id: id,
            transpose: 3,
        })
        .unwrap();
    let clip = shifted.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap();
    let key = daw_media::ClipAudioKey::for_clip(clip).unwrap();
    let prepared = &shifted.stretched_audio[&key];
    let context = prepared.context.as_ref().unwrap();
    assert_eq!(context.start_frame, 499);
    assert_eq!(prepared.audio.frame_count(), 2000);
    assert_eq!(context.audio.frame_count(), 2998);
    assert_eq!(shifted.project.clip_fade_limit(id, true, true), 499);
    let mut renderer = daw_engine::Renderer::new(shifted.plan());
    let mut output = vec![[0.; 2]; 4500];
    renderer.render_offline(&mut output);
    assert!(renderer.source_error().is_none());
    assert!(output.iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn verified_missing_leaves_stay_silent_but_short_source_extraction_fails() {
    let f = Fixture::new();
    let (base, ids) = f.session();
    let (mut merged, _) = base.merged_audio_clips(&ids, &|| false).unwrap();
    let folder = f.0.join("saved");
    merged.save(&folder).unwrap();
    fs::remove_file(f.0.join("original.wav")).unwrap();
    let missing = Session::open(&folder).unwrap();
    assert!(!missing.warnings.is_empty());
    assert!(render(&missing).iter().all(|v| *v == [0.; 2]));
    let mut writer = hound::WavWriter::create(
        f.0.join("original.wav"),
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for _ in 0..2000 {
        writer.write_sample(0.1f32).unwrap();
        writer.write_sample(-0.1f32).unwrap();
    }
    writer.finalize().unwrap();
    let shortened = Session::open(&folder).unwrap();
    assert!(!shortened.warnings.is_empty());
    assert!(
        shortened
            .render_clip_source(&shortened.project.tracks[0].clips[0], &|| false)
            .is_err()
    );
}

#[test]
fn context_rounding_keeps_the_exact_stretched_body_length() {
    let f = Fixture::new();
    for engine in [
        daw_core::StretchEngine::INTEGRATED,
        daw_core::StretchEngine::RUBBER_BAND,
    ] {
        let (mut base, ids) = f.session();
        base.project.stretch_engine = engine;
        base.project.tracks[0].clips.truncate(1);
        let clip = &mut base.project.tracks[0].clips[0];
        clip.start_frame = 0;
        clip.length_frames = 4095;
        clip.source_offset_frame = 1905;
        let (mut merged, _) = base.merged_audio_clips(&ids[..1], &|| false).unwrap();
        let clip = &mut merged.project.tracks[0].clips[0];
        clip.stretch = daw_core::ClipStretch::new(2, 1);
        clip.length_frames = 2048;
        merged.project.validate().unwrap();
        merged.prepare_stretches().unwrap();
        let audio = merged
            .audio_for_clip(&merged.project.tracks[0].clips[0])
            .unwrap();
        assert_eq!(audio.frame_count(), 2048);
        assert!(audio.samples.iter().flatten().all(|v| v.is_finite()));
    }
}

#[test]
fn cropped_compounds_keep_the_complete_source_clock_and_new_edge_smoothing() {
    let f = Fixture::new();
    let (base, ids) = f.session();
    let (merged, id) = base.merged_audio_clips(&ids, &|| false).unwrap();
    let original = &merged.project.tracks[0].clips[0];
    let source = merged.render_clip_source(original, &|| false).unwrap();
    let candidate = original
        .trimmed_by(daw_core::ClipEdge::Left, 5000, source.frame_count())
        .unwrap();
    let cropped = merged
        .with_edit(Edit::ResizeClips {
            clips: vec![candidate],
            edge: daw_core::ClipEdge::Left,
        })
        .unwrap();
    let clip = cropped.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap();
    assert!(
        cropped
            .stretched_audio
            .contains_key(&daw_media::ClipAudioKey::unity(clip.asset_id))
    );
    let actual = render(&cropped);
    for local in 0..clip.length_frames {
        let expected = source.samples[(clip.source_offset_frame + local) as usize]
            .map(|v| v * clip.fades.gain(local, clip.length_frames, 48000));
        assert!(
            actual[(clip.start_frame + local) as usize]
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
    }
}
