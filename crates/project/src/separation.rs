//! Atomic worker-side separation and import through the shared Session path.
use crate::{Result, Session, error};
use daw_core::{Edit, Id};
use daw_media::{AudioEncoder, WavCodec, WavEncoder};
pub use daw_separation::{Models as SeparationModels, Progress as SeparationProgress};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

/// Unpublished output owns its directory. Failure, cancellation, or a dropped
/// worker receiver removes only this job's files. Published files survive undo.
pub struct PreparedSeparation {
    session: Option<Session>,
    directory: Option<PathBuf>,
    pub warnings: Vec<String>,
}
impl Drop for PreparedSeparation {
    fn drop(&mut self) {
        if let Some(directory) = &self.directory {
            let _ = fs::remove_dir_all(directory);
        }
    }
}
impl PreparedSeparation {
    pub fn publish(mut self) -> Session {
        self.directory = None;
        self.session
            .take()
            .expect("Prepared separation has one session")
    }
}
fn reserve_directory(parent: &Path, name: &str) -> Result<PathBuf> {
    fs::create_dir_all(parent).map_err(error)?;
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(100)
        .collect();
    let clean = clean.trim().trim_end_matches('.');
    let clean = if crate::project_folder_name(clean).is_ok() {
        clean
    } else {
        "Clip"
    };
    for index in 0..10_000 {
        let path = parent.join(if index == 0 {
            clean.to_owned()
        } else {
            format!("{clean} ({index})")
        });
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(error(e)),
        }
    }
    Err(error("No unused stem folder name available"))
}
impl Session {
    pub fn separate_clip(
        &self,
        clip_id: Id,
        models: &SeparationModels,
        progress: &SeparationProgress,
    ) -> Result<PreparedSeparation> {
        self.separate_clip_with(clip_id, progress, |samples, directory| {
            daw_separation::separate(samples, models, directory, progress).map_err(error)
        })
    }
    fn separate_clip_with(
        &self,
        clip_id: Id,
        progress: &SeparationProgress,
        run: impl FnOnce(&[[f32; 2]], &Path) -> Result<Vec<String>>,
    ) -> Result<PreparedSeparation> {
        let folder = self
            .folder
            .as_ref()
            .ok_or_else(|| error("Save the project before separating a clip"))?;
        progress.check().map_err(error)?;
        let clip = self
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == clip_id)
            .ok_or_else(|| error("Clip not found"))?
            .clone();
        let audio = self
            .audio_for_clip(&clip)
            .ok_or_else(|| error("Prepared audio is missing; separation requires an audio clip"))?;
        let mut input = Vec::new();
        input
            .try_reserve_exact(usize::try_from(clip.length_frames).map_err(error)?)
            .map_err(error)?;
        for local in 0..clip.length_frames {
            if local % 8192 == 0 {
                progress.check().map_err(error)?;
            }
            let source = clip.source_frame(local);
            let fade = daw_engine::clip_repeat_fade(&clip, source, self.project.sample_rate_hz);
            input.push(
                audio
                    .samples
                    .get(source as usize)
                    .ok_or_else(|| error("Clip source range is unavailable"))?
                    .map(|v| v * fade),
            );
        }
        let input = daw_media::resample_cancellable(
            &input,
            self.project.sample_rate_hz,
            daw_separation::MODEL_RATE,
            &|| progress.cancel.load(Ordering::Relaxed),
        )
        .map_err(error)?;
        let directory = reserve_directory(&folder.join("stems"), &clip.name)?;
        let mut prepared = PreparedSeparation {
            session: None,
            directory: Some(directory.clone()),
            warnings: Vec::new(),
        };
        prepared.warnings = run(&input, &directory)?;
        progress.check().map_err(error)?;
        let mut next = self.clone();
        let mut tracks = Vec::new();
        for name in daw_separation::STEMS {
            progress.check().map_err(error)?;
            let path = directory.join(format!("{name}.wav"));
            let data =
                daw_media::decode_wav_at_rate(&path, self.project.sample_rate_hz).map_err(error)?;
            if data.metadata.channels != 2 || data.samples.iter().flatten().any(|v| !v.is_finite())
            {
                return Err(error("Invalid separated stereo audio"));
            }
            // Resampling can differ by one rounding frame. Preserve the exact
            // arranged duration, without accepting a truncated inference result.
            if data.samples.len().abs_diff(clip.length_frames as usize) > 2 {
                return Err(error("Stem duration does not match the clip"));
            }
            let mut samples = (*data.samples).clone();
            samples.resize(clip.length_frames as usize, [0.0; 2]);
            let mut encoder =
                WavEncoder::at_sample_rate(&path, WavCodec::Float32, self.project.sample_rate_hz)
                    .map_err(error)?;
            for block in samples.chunks(8192) {
                progress.check().map_err(error)?;
                encoder.write_frames(block).map_err(error)?;
            }
            encoder.finish().map_err(error)?;
            let data =
                daw_media::decode_wav_at_rate(&path, self.project.sample_rate_hz).map_err(error)?;
            let id = next.import_decoded(&path, data, None, clip.start_frame)?;
            let track = next
                .project
                .tracks
                .iter_mut()
                .find(|t| t.clips.iter().any(|c| c.id == id))
                .unwrap();
            track.name = name.to_owned();
            let stem = track.clips.iter_mut().find(|c| c.id == id).unwrap();
            stem.name = name.to_owned();
            stem.gain_db = clip.gain_db;
            stem.color = clip.color;
            tracks.push(track.id);
            let asset = next
                .project
                .assets
                .iter_mut()
                .find(|a| a.id == stem.asset_id)
                .unwrap();
            asset.source.kind = "generated".into();
            asset.source.path = path
                .strip_prefix(folder)
                .map_err(error)?
                .to_string_lossy()
                .into_owned();
            asset.source.path_kind = "relative".into();
        }
        next.project
            .edit(Edit::Batch(vec![
                Edit::GroupTracks {
                    track_ids: tracks,
                    group_id: Id::new_v4(),
                    name: format!("{} stems", clip.name),
                },
                Edit::SetClipMuted {
                    clip_id,
                    muted: true,
                },
            ]))
            .map_err(error)?;
        progress.check().map_err(error)?;
        prepared.session = Some(next);
        Ok(prepared)
    }
}

/// Copy only project-owned generated audio on Save As. External recordings stay
/// referenced. Each destination is reserved without replacing existing files.
pub(crate) struct GeneratedCopies(pub Vec<PathBuf>);
impl Drop for GeneratedCopies {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}
pub(crate) fn copy_generated(
    session: &Session,
    next: &mut daw_core::Project,
    folder: &Path,
) -> Result<GeneratedCopies> {
    let mut cleanup = GeneratedCopies(Vec::new());
    let mut directories: std::collections::HashMap<PathBuf, PathBuf> =
        std::collections::HashMap::new();
    for asset in next
        .assets
        .iter_mut()
        .filter(|a| a.source.kind == "generated")
    {
        let original = session
            .project
            .assets
            .iter()
            .find(|a| a.id == asset.id)
            .unwrap();
        let source = session.source_path(original)?;
        if source.starts_with(folder.join("stems")) {
            continue;
        }

        let parent = source
            .parent()
            .ok_or_else(|| error("Stem folder is missing"))?;
        let target_dir = if let Some(path) = directories.get(parent) {
            path.clone()
        } else {
            let path = reserve_directory(
                &folder.join("stems"),
                &parent.file_name().unwrap_or_default().to_string_lossy(),
            )?;
            cleanup.0.push(path.clone());
            directories.insert(parent.to_path_buf(), path.clone());
            path
        };
        let target = target_dir.join(
            source
                .file_name()
                .ok_or_else(|| error("Stem file name is missing"))?,
        );
        fs::copy(&source, &target).map_err(error)?;
        asset.source.path = target.to_string_lossy().into_owned();
        asset.source.path_kind = "absolute".into();
    }
    Ok(cleanup)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> (Self, Session, Id) {
            let root = std::env::temp_dir().join(format!("daw-stems-{}", Id::new_v4()));
            fs::create_dir(&root).unwrap();
            let source = root.join("original.wav");
            let mut encoder =
                WavEncoder::at_sample_rate(&source, WavCodec::Float32, 48_000).unwrap();
            encoder.write_frames(&vec![[0.2, -0.1]; 4800]).unwrap();
            encoder.finish().unwrap();
            let mut session = Session::default();
            session.save(&root.join("project")).unwrap();
            let clip = session.import(&source, None, 9600).unwrap();
            let c = &mut session.project.tracks[0].clips[0];
            c.length_frames = 2400;
            c.source_offset_frame = 120;
            c.repeat = Some(daw_core::ClipLoop {
                length_frames: 960,
                phase_frame: 240,
            });
            c.gain_db = -3.0;
            session.project.validate().unwrap();
            (Self(root), session, clip)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fake(samples: &[[f32; 2]], directory: &Path) -> Result<Vec<String>> {
        for stem in daw_separation::STEMS {
            let mut encoder = WavEncoder::at_sample_rate(
                &directory.join(format!("{stem}.wav")),
                WavCodec::Float32,
                44_100,
            )
            .map_err(error)?;
            encoder.write_frames(samples).map_err(error)?;
            encoder.finish().map_err(error)?;
        }
        Ok(vec!["Test warning".into()])
    }
    #[test]
    fn separation_preserves_original_aligns_group_and_reopens_after_save_as() {
        let (fixture, session, clip) = Fixture::new();
        let original_bytes = fs::read(fixture.0.join("original.wav")).unwrap();
        let original = session.project.clone();
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), |samples, path| {
                assert_eq!(samples.len(), 2205);
                assert!(samples.iter().any(|s| (s[0] - 0.2).abs() < 0.005));
                assert!(samples.iter().any(|s| s[0].abs() < 0.005));
                assert!(samples.iter().all(|s| (s[0] + 2.0 * s[1]).abs() < 1e-5));
                fake(samples, path)
            })
            .unwrap();
        assert_eq!(session.project, original);
        assert_eq!(prepared.warnings, vec!["Test warning"]);
        let mut next = prepared.publish();
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        history.record(&next).unwrap();
        assert_eq!(next.project.tracks.len(), 6);
        let retained = &next.project.tracks[0].clips[0];
        assert!(retained.muted);
        let mut expected = original.tracks[0].clips[0].clone();
        expected.muted = true;
        assert_eq!(*retained, expected);
        let group = next
            .project
            .tracks
            .iter()
            .find(|t| t.kind == daw_core::TrackKind::Group)
            .unwrap()
            .id;
        for track in next
            .project
            .tracks
            .iter()
            .filter(|t| t.group_id == Some(group))
        {
            let stem = &track.clips[0];
            assert_eq!(
                (stem.start_frame, stem.length_frames, stem.gain_db),
                (9600, 2400, -3.0)
            );
            assert!(!stem.muted);
            assert_eq!(stem.transpose, 0);
            assert!(stem.stretch.is_none());
        }
        next.project.validate().unwrap();
        next.save(&fixture.0.join("copy")).unwrap();
        history.mark_saved(&next).unwrap();
        let restored = history.undo(&next).unwrap();
        assert!(!restored.project.tracks[0].clips[0].muted);
        assert_eq!(restored.project.tracks.len(), 1);
        let mut redone = history.redo(&restored).unwrap();
        assert!(redone.project.tracks[0].clips[0].muted);
        assert!(
            redone
                .project
                .assets
                .iter()
                .filter(|a| a.source.kind == "generated")
                .all(|a| redone
                    .source_path(a)
                    .unwrap()
                    .starts_with(fixture.0.join("copy")))
        );
        redone.save(&fixture.0.join("copy")).unwrap();
        fs::remove_dir_all(fixture.0.join("project")).unwrap();
        let reopened = Session::open(&fixture.0.join("copy")).unwrap();
        assert_eq!(reopened.project.tracks.len(), 6);
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        assert!(
            reopened
                .project
                .assets
                .iter()
                .filter(|a| a.source.kind == "generated")
                .all(|a| a.source.path.starts_with("stems/"))
        );
        assert_eq!(
            fs::read(fixture.0.join("original.wav")).unwrap(),
            original_bytes
        );
    }
    #[test]
    fn failed_cancelled_and_discarded_jobs_leave_no_stems_or_edits() {
        let (_fixture, session, clip) = Fixture::new();
        let before = session.project.clone();
        let progress = SeparationProgress::default();
        assert!(
            session
                .separate_clip_with(clip, &progress, |samples, path| {
                    fake(samples, path)?;
                    Err(error("Inference failed"))
                })
                .is_err()
        );
        let stems = session.folder.as_ref().unwrap().join("stems");
        assert_eq!(fs::read_dir(&stems).unwrap().count(), 0);
        let prepared = session.separate_clip_with(clip, &progress, fake).unwrap();
        drop(prepared);
        assert_eq!(fs::read_dir(&stems).unwrap().count(), 0);
        assert!(
            session
                .separate_clip_with(clip, &progress, |samples, path| {
                    let result = fake(samples, path);
                    progress.cancel.store(true, Ordering::Relaxed);
                    result
                })
                .is_err()
        );
        assert_eq!(fs::read_dir(&stems).unwrap().count(), 0);
        assert_eq!(session.project, before);
    }
    #[test]
    fn same_name_jobs_use_distinct_directories() {
        let (_fixture, session, clip) = Fixture::new();
        let first = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let second = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        assert_ne!(first.directory, second.directory);
    }
}

#[cfg(test)]
mod model_smoke {
    use super::*;
    /// Local opt-in check; weights and private media never enter test fixtures.
    #[test]
    #[ignore = "requires local HTDemucs model paths and test WAV"]
    fn local_models_separate_and_import() {
        let cpu = std::env::var_os("DAW_STEM_CPU_MODEL").expect("DAW_STEM_CPU_MODEL");
        let coreml = std::env::var_os("DAW_STEM_COREML_MODEL").map(PathBuf::from);
        let input = std::env::var_os("DAW_STEM_INPUT").expect("DAW_STEM_INPUT");
        let folder = std::env::temp_dir().join(format!("daw-model-smoke-{}", Id::new_v4()));
        let mut session = Session::default();
        session.save(&folder).unwrap();
        let clip = session.import(Path::new(&input), None, 9600).unwrap();
        let length = session.project.tracks[0].clips[0].length_frames;
        let result = session
            .separate_clip(
                clip,
                &SeparationModels {
                    cpu: Some(cpu.into()),
                    coreml,
                },
                &SeparationProgress::default(),
            )
            .unwrap();
        eprintln!("Model warnings: {:?}", result.warnings);
        let mut session = result.publish();
        session.save(&folder).unwrap();
        let reopened = Session::open(&folder).unwrap();
        assert!(reopened.warnings.is_empty());
        assert_eq!(reopened.project.tracks.len(), 6);
        assert!(reopened.project.tracks[0].clips[0].muted);
        for track in reopened
            .project
            .tracks
            .iter()
            .filter(|t| t.group_id.is_some())
        {
            assert_eq!(
                (track.clips[0].start_frame, track.clips[0].length_frames),
                (9600, length)
            );
            assert!(
                reopened
                    .audio_for_clip(&track.clips[0])
                    .unwrap()
                    .samples
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite())
            );
        }
        fs::remove_dir_all(folder).unwrap();
    }
}
