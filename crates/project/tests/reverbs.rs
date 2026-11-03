use daw_core::{Edit, EffectInstance};
use daw_media::{ExportSettings, WavCodec};
use daw_project::Session;
use std::fs;

#[test]
fn reverbs_save_reopen_and_export_the_same_capped_tail_on_track_and_master() {
    let root = std::env::temp_dir().join(format!("daw-reverbs-{}", daw_core::Id::new_v4()));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let source = root.join("impulse.wav");
    let mut wav = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for i in 0..4096 {
        wav.write_sample(if i == 512 { 0.2_f32 } else { 0.0 })
            .unwrap();
        wav.write_sample(0.0_f32).unwrap();
    }
    wav.finalize().unwrap();
    for (index, master, effect) in [
        (0, false, EffectInstance::plate_reverb()),
        (1, true, EffectInstance::ambient_reverb()),
    ] {
        let mut session = Session::default();
        session.import(&source, None, 0).unwrap();
        if master {
            session
                .project
                .edit(Edit::SetMasterEffects {
                    effects: vec![effect.clone()],
                    compact: false,
                })
                .unwrap();
        } else {
            let track = session.project.tracks[0].id;
            session
                .project
                .edit(Edit::SetEffects {
                    track_id: track,
                    effects: vec![effect.clone()],
                    compact: false,
                })
                .unwrap();
        }
        let folder = root.join(format!("project-{index}"));
        session.save(&folder).unwrap();
        let loaded = Session::open(&folder).unwrap();
        assert!(loaded.project.all_effects().any(|e| *e == effect));
        let end = loaded.project.end();
        let output = root.join(format!("tail-{index}.wav"));
        let warnings = loaded
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
            warnings
                .iter()
                .any(|w| w.contains("tail") && w.contains("truncated"))
        );
        let mut wav = hound::WavReader::open(output).unwrap();
        assert_eq!(u64::from(wav.len()) / 2, end + 480000);
        let frames: Vec<_> = wav
            .samples::<f32>()
            .take((end as usize + 8192) * 2)
            .map(Result::unwrap)
            .collect();
        assert!(frames[end as usize * 2..].iter().any(|x| x.abs() > 1e-8));
        let mut renderer = daw_engine::Renderer::new(loaded.plan());
        let mut direct = vec![[0.0; 2]; frames.len() / 2];
        renderer.render_offline(&mut direct);
        assert!(direct.iter().flatten().zip(frames).all(|(a, b)| *a == b));
    }
}
