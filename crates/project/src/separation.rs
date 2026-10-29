//! Atomic worker-side separation and import through the shared Session path.
use crate::{Result, SeparationStemSet, Session, error};
use daw_core::{Edit, Id};
use daw_media::{AudioEncoder, WavCodec, WavEncoder};
pub use daw_separation::{Models as SeparationModels, Progress as SeparationProgress};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

/// Only owns a fresh, exclusively created temporary directory. Session/history
/// clones keep files alive independently of the separation job and save worker.
pub(super) struct TemporaryStems(PathBuf);
impl TemporaryStems {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("daw-temporary-stems-{}", Id::new_v4()));
        fs::create_dir(&path).map_err(error)?;
        Ok(Self(path))
    }
}
impl Drop for TemporaryStems {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Unpublished output owns its directory. Failure, cancellation, or a dropped
/// worker receiver removes only this job's files. Published files survive undo.
pub struct PreparedSeparation {
    session: Option<Session>,
    directory: Option<PathBuf>,
    pub warnings: Vec<String>,
    pub stems: SeparationStemSet,
}
impl Drop for PreparedSeparation {
    fn drop(&mut self) {
        if let Some(directory) = &self.directory {
            let _ = fs::remove_dir_all(directory);
        }
    }
}
impl PreparedSeparation {
    pub fn directory(&self) -> &Path {
        self.directory
            .as_deref()
            .expect("Unpublished stem directory")
    }
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
    let clean = &clean[..clean.floor_char_boundary(crate::MAX_FOLDER_NAME_BYTES)];
    let clean = clean.trim().trim_end_matches('.');
    let clean = if crate::project_folder_name(clean).is_ok() {
        clean
    } else {
        "Clip"
    };
    for index in 0..10_000 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!(" ({index})")
        };
        // The filesystem limit applies to the complete UTF-8 basename.
        let end = clean.floor_char_boundary(crate::MAX_FOLDER_NAME_BYTES - suffix.len());
        let path = parent.join(format!("{}{suffix}", &clean[..end]));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(error(e)),
        }
    }
    Err(error("No unused stem folder name available"))
}
impl Session {
    /// Merge saved generated-source locations without replacing newer edits or
    /// transport. Resolve paths against the written session before changing self.
    pub fn retarget_generated_sources(&mut self, written: &Session) -> Result<()> {
        if self.project.project_id != written.project.project_id {
            return Err(error("Saved sources belong to a different project"));
        }
        let sources = written
            .project
            .assets
            .iter()
            .filter(|asset| asset.source.kind == "generated")
            .map(|asset| {
                let mut source = asset.source.clone();
                source.path = written.source_path(asset)?.to_string_lossy().into_owned();
                source.path_kind = "absolute".into();
                Ok((asset.id, source))
            })
            .collect::<Result<std::collections::HashMap<_, _>>>()?;
        for asset in self
            .project
            .assets
            .iter_mut()
            .filter(|asset| asset.source.kind == "generated")
        {
            if let Some(source) = sources.get(&asset.id) {
                asset.source = source.clone();
            }
        }
        self.retain_temporary_stems();
        Ok(())
    }

    /// Release only storage no longer referenced by this session. Other worker
    /// and history snapshots retain their own shared owners until retargeted.
    pub(crate) fn retain_temporary_stems(&mut self) {
        if self.temporary_stems.is_empty() {
            return;
        }
        let sources: Vec<_> = self
            .project
            .assets
            .iter()
            .filter_map(|asset| self.source_path(asset).ok())
            .collect();
        self.temporary_stems.retain(|directory| {
            sources
                .iter()
                .any(|source| source.starts_with(&directory.0))
        });
    }

    pub(crate) fn temporary_stems_memory_bytes(&self) -> u64 {
        (self.temporary_stems.capacity() * std::mem::size_of::<std::sync::Arc<TemporaryStems>>())
            as u64
            + self
                .temporary_stems
                .iter()
                .map(|directory| directory.0.capacity() as u64)
                .sum::<u64>()
    }

    pub fn separate_clip(
        &self,
        clip_id: Id,
        models: &SeparationModels,
        progress: &SeparationProgress,
    ) -> Result<PreparedSeparation> {
        if !crate::SEPARATION_AVAILABLE {
            return Err(error(crate::SEPARATION_UNAVAILABLE_REASON));
        }
        self.separate_clip_with(clip_id, progress, |samples, directory| {
            daw_separation::separate(samples, models, directory, progress).map_err(error)
        })
    }
    fn separate_clip_with(
        &self,
        clip_id: Id,
        progress: &SeparationProgress,
        run: impl FnOnce(&[[f32; 2]], &Path) -> Result<daw_separation::Output>,
    ) -> Result<PreparedSeparation> {
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
        let temporary = self
            .folder
            .is_none()
            .then(TemporaryStems::new)
            .transpose()?
            .map(std::sync::Arc::new);
        let folder = self
            .folder
            .as_deref()
            .or_else(|| temporary.as_ref().map(|directory| directory.0.as_path()))
            .expect("Saved folder or owned temporary storage");
        let directory = reserve_directory(&folder.join("stems"), &clip.name)?;
        let mut prepared = PreparedSeparation {
            session: None,
            directory: Some(directory.clone()),
            warnings: Vec::new(),
            stems: SeparationStemSet::Four,
        };
        let output = run(&input, &directory)?;
        prepared.stems = output.stems;
        prepared.warnings = output.warnings;
        progress.check().map_err(error)?;
        let mut next = self.clone();
        if let Some(temporary) = &temporary {
            next.temporary_stems.push(temporary.clone());
        }
        let mut tracks = Vec::new();
        for &name in prepared.stems.names() {
            progress.check().map_err(error)?;
            let path = directory.join(format!("{name}.wav"));
            let data = prepare_stem(
                &path,
                input.len() as u64,
                clip.length_frames,
                self.project.sample_rate_hz,
                &|| progress.cancel.load(Ordering::Relaxed),
            )?;
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
            if self.folder.is_some() {
                asset.source.path = path
                    .strip_prefix(folder)
                    .map_err(error)?
                    .to_string_lossy()
                    .into_owned();
                asset.source.path_kind = "relative".into();
            }
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

/// Normalize only the known rate-rounding surplus. Both full decode/peak passes
/// use the same cancellation signal as inference and shared resampling.
fn prepare_stem(
    path: &Path,
    model_frames: u64,
    clip_frames: u64,
    sample_rate_hz: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<daw_media::AudioData> {
    let data = daw_media::decode_wav_cancellable(path, sample_rate_hz, cancelled).map_err(error)?;
    let expected =
        daw_media::resampled_frames(model_frames, daw_separation::MODEL_RATE, sample_rate_hz)
            .map_err(error)?;
    if data.metadata.channels != 2
        || data.metadata.sample_rate_hz != daw_separation::MODEL_RATE
        || data.samples.len() as u64 != expected
        || expected < clip_frames
    {
        return Err(error(
            "Stem duration or format does not match the model output",
        ));
    }
    let mut samples = (*data.samples).clone();
    samples.truncate(usize::try_from(clip_frames).map_err(error)?);
    let mut encoder =
        WavEncoder::at_sample_rate(path, WavCodec::Float32, sample_rate_hz).map_err(error)?;
    for block in samples.chunks(8192) {
        if cancelled() {
            return Err(error("Stem separation cancelled"));
        }
        encoder.write_frames(block).map_err(error)?;
    }
    encoder.finish().map_err(error)?;
    daw_media::decode_wav_cancellable(path, sample_rate_hz, cancelled).map_err(error)
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
    fn fake(samples: &[[f32; 2]], directory: &Path) -> Result<daw_separation::Output> {
        fake_stems(samples, directory, SeparationStemSet::Four)
    }
    fn fake_stems(
        samples: &[[f32; 2]],
        directory: &Path,
        stems: SeparationStemSet,
    ) -> Result<daw_separation::Output> {
        for stem in stems.names() {
            let mut encoder = WavEncoder::at_sample_rate(
                &directory.join(format!("{stem}.wav")),
                WavCodec::Float32,
                44_100,
            )
            .map_err(error)?;
            encoder.write_frames(samples).map_err(error)?;
            encoder.finish().map_err(error)?;
        }
        Ok(daw_separation::Output {
            stems,
            warnings: vec!["Test warning".into()],
        })
    }

    #[test]
    fn six_stem_import_is_atomic_and_survives_history_and_save() {
        for saved in [false, true] {
            let (fixture, mut session, clip) = Fixture::new();
            if !saved {
                session.folder = None;
            }
            let before = session.project.clone();
            let mut history = crate::SessionHistory::new(&session, false).unwrap();
            let prepared = session
                .separate_clip_with(clip, &SeparationProgress::default(), |input, path| {
                    fake_stems(input, path, SeparationStemSet::Six)
                })
                .unwrap();
            assert_eq!(session.project, before);
            assert_eq!(prepared.stems, SeparationStemSet::Six);
            let mut next = prepared.publish();
            history.record(&next).unwrap();
            assert_eq!(next.project.tracks.len(), 8);
            assert!(next.project.tracks[0].clips[0].muted);
            let tracks: Vec<_> = next
                .project
                .tracks
                .iter()
                .filter(|t| t.group_id.is_some())
                .collect();
            assert_eq!(
                tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
                SeparationStemSet::Six.names()
            );
            for track in tracks {
                let stem = &track.clips[0];
                assert_eq!(
                    (stem.start_frame, stem.length_frames, stem.gain_db),
                    (9600, 2400, -3.0)
                );
            }
            let undone = history.undo(&next).unwrap();
            assert_eq!(undone.project, before);
            next = history.redo(&undone).unwrap();
            let destination = fixture.0.join("six-stem-save");
            next.save(&destination).unwrap();
            let reopened = Session::open(&destination).unwrap();
            assert!(reopened.warnings.is_empty());
            assert_eq!(reopened.project.tracks.len(), 8);
            assert_eq!(
                reopened
                    .project
                    .assets
                    .iter()
                    .filter(|a| a.source.kind == "generated")
                    .count(),
                6
            );
            let failed =
                session.separate_clip_with(clip, &SeparationProgress::default(), |input, path| {
                    let mut output = fake(input, path)?;
                    output.stems = SeparationStemSet::Six;
                    Ok(output)
                });
            assert!(
                failed.is_err(),
                "incomplete detected six-stem output must not publish"
            );
            assert_eq!(session.project, before);
        }
    }

    #[test]
    fn stem_folder_collisions_keep_multibyte_names_within_the_byte_limit() {
        let (fixture, _, _) = Fixture::new();
        for name in [
            "Clip".into(),
            "界".repeat(84),
            "界".repeat(85),
            "🎵".repeat(100),
        ] {
            let parent = fixture.0.join(Id::new_v4().to_string());
            for index in 0..12 {
                let path = reserve_directory(&parent, &name).unwrap();
                let basename = path.file_name().unwrap().to_str().unwrap();
                assert!(basename.len() <= 255, "{basename}");
                assert!(basename.starts_with(name.chars().next().unwrap()));
                crate::project_folder_name(basename).unwrap();
                if index > 0 {
                    assert!(basename.ends_with(&format!(" ({index})")));
                }
                fs::write(path.join("keep.txt"), index.to_string()).unwrap();
            }
            assert_eq!(fs::read_dir(&parent).unwrap().count(), 12);
            for entry in fs::read_dir(&parent).unwrap() {
                assert!(entry.unwrap().path().join("keep.txt").is_file());
            }
        }
    }

    #[test]
    fn multibyte_stem_collisions_survive_repeated_separation_and_save_as() {
        let (fixture, mut session, clip) = Fixture::new();
        let name = "界".repeat(84);
        session.project.tracks[0].clips[0].name = name.clone();
        let first = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let first_directory = first.directory().to_owned();
        let _first = first.publish();
        let second = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let second_directory = second.directory().to_owned();
        assert_ne!(first_directory, second_directory);
        let mut next = second.publish();
        let destination = fixture.0.join("copy");
        let occupied = reserve_directory(
            &destination.join("stems"),
            second_directory.file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();
        fs::write(occupied.join("keep.txt"), b"existing folder").unwrap();
        next.save(&destination).unwrap();
        assert_eq!(
            fs::read(occupied.join("keep.txt")).unwrap(),
            b"existing folder"
        );
        assert!(first_directory.join("drums.wav").is_file());
        assert!(second_directory.join("drums.wav").is_file());
        let reopened = Session::open(&destination).unwrap();
        assert_eq!(reopened.project.tracks.len(), 6);
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        for asset in reopened
            .project
            .assets
            .iter()
            .filter(|a| a.source.kind == "generated")
        {
            let source = reopened.source_path(asset).unwrap();
            assert!(source.starts_with(destination.join("stems")));
            assert_ne!(source.parent().unwrap(), occupied);
            assert!(
                source
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .len()
                    <= 255
            );
            assert!(source.is_file());
        }
    }

    #[test]
    fn unsaved_stems_survive_undo_and_first_save_retargets_history_before_cleanup() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let temporary = directory.parent().unwrap().parent().unwrap().to_owned();
        let mut next = prepared.publish();
        assert!(next.folder.is_none());
        assert_eq!(next.project.tracks.len(), 6);
        assert!(next.project.tracks[0].clips[0].muted);
        history.record(&next).unwrap();
        let undone = history.undo(&next).unwrap();
        drop(next);
        assert!(directory.join("drums.wav").is_file());
        next = history.redo(&undone).unwrap();
        next.project.master.gain_db = -3.0;
        history.record(&next).unwrap();
        next = history.undo(&next).unwrap();
        assert!(history.can_redo());

        let saved = fixture.0.join("first-save");
        next.save(&saved).unwrap();
        // Retained history still needs the old files until its paths are retargeted.
        assert!(temporary.is_dir());
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(!temporary.exists());
        assert!(history.is_saved());
        assert!(history.can_redo());
        next = history.redo(&next).unwrap();
        assert_eq!(next.project.master.gain_db, -3.0);
        next = history.undo(&next).unwrap();
        let before_separation = history.undo(&next).unwrap();
        assert_eq!(before_separation.project.tracks.len(), 1);
        next = history.redo(&before_separation).unwrap();
        assert!(history.is_saved());
        for asset in next
            .project
            .assets
            .iter()
            .filter(|a| a.source.kind == "generated")
        {
            let path = next.source_path(asset).unwrap();
            assert!(path.starts_with(saved.join("stems")), "{path:?}");
            assert!(path.is_file());
        }
        let reopened = Session::open(&saved).unwrap();
        assert_eq!(reopened.project.tracks.len(), 6);
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        for track in reopened
            .project
            .tracks
            .iter()
            .filter(|t| t.group_id.is_some())
        {
            assert_eq!(
                reopened
                    .audio_for_clip(&track.clips[0])
                    .unwrap()
                    .samples
                    .len(),
                2400
            );
            assert_eq!(track.clips[0].start_frame, 9600);
        }
    }

    #[test]
    fn source_collection_snapshot_keeps_history_only_temporary_stems_alive() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let temporary = directory.parent().unwrap().parent().unwrap().to_owned();
        let published = prepared.publish();
        history.record(&published).unwrap();
        let mut live = history.undo(&published).unwrap();
        drop(published);
        live.save(&fixture.0.join("collected-plan")).unwrap();
        history.mark_saved(&live).unwrap();
        let snapshot = live.source_collection_snapshot(Some(&history), 10).unwrap();
        history.release_optional_memory();
        drop(history);
        drop(live);
        assert!(directory.join("drums.wav").is_file());
        let report = snapshot.preflight(None, &|| false).unwrap();
        assert!(report.is_ready());
        assert_eq!(report.summary().history_only_sources, 4);
        drop(report);
        drop(snapshot);
        assert!(!temporary.exists());
    }

    #[test]
    fn temporary_stems_are_shared_until_the_last_session_is_released() {
        let (_fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let temporary = directory.parent().unwrap().parent().unwrap().to_owned();
        let published = prepared.publish();
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        history.record(&published).unwrap();
        let worker = published.clone();
        drop(published);
        assert!(directory.join("drums.wav").is_file());
        drop(worker);
        let restored = history.undo(&session).unwrap();
        assert_eq!(restored.project.tracks.len(), 1);
        assert!(directory.join("drums.wav").is_file());
        history.release_optional_memory();
        assert!(!temporary.exists());
    }

    #[test]
    fn first_save_with_separation_undone_keeps_temporary_stems_for_redo() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let temporary = directory.parent().unwrap().parent().unwrap().to_owned();
        let mut next = prepared.publish();
        history.record(&next).unwrap();
        next = history.undo(&next).unwrap();
        let saved = fixture.0.join("undone-first-save");
        next.save(&saved).unwrap();
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(history.can_redo());
        assert!(directory.join("drums.wav").is_file());
        next = history.redo(&next).unwrap();
        next.save(&saved).unwrap();
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(!temporary.exists());
        assert_eq!(Session::open(&saved).unwrap().project.tracks.len(), 6);
    }

    #[test]
    fn regular_save_completion_retargets_temporary_stems_and_pending_history() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let temporary = directory.parent().unwrap().parent().unwrap().to_owned();
        let mut live = prepared.publish();
        history.record(&live).unwrap();
        live = history.undo(&live).unwrap();
        let destination = fixture.0.join("first-save");
        live.save(&destination).unwrap();
        history.mark_saved(&live).unwrap();
        history.record(&live).unwrap();
        live = history.redo(&live).unwrap();
        assert!(temporary.is_dir());

        let mut written = live.clone();
        written.save(&destination).unwrap();
        history.begin_group();
        live.project.tracks[0].name = "Newer grouped edit".into();
        live.project.transport.playhead_frame = 1234;
        history.record(&live).unwrap();
        // The actual ordinary-save completion merges only saved source locations.
        live.retarget_generated_sources(&written).unwrap();
        history.mark_saved(&written).unwrap();
        assert!(!temporary.exists());
        assert_eq!(live.project.tracks[0].name, "Newer grouped edit");
        assert_eq!(live.project.transport.playhead_frame, 1234);
        assert!(!history.is_saved());
        history.finish_group();
        let restored = history.undo(&live).unwrap();
        assert!(history.is_saved());
        let redone = history.redo(&restored).unwrap();
        assert_eq!(redone.project.tracks[0].name, "Newer grouped edit");
        assert!(!history.is_saved());
        for state in [&live, &restored, &redone] {
            for asset in &state.project.assets {
                if asset.source.kind == "generated" {
                    let path = state.source_path(asset).unwrap();
                    assert!(path.starts_with(destination.join("stems")));
                    assert!(path.is_file());
                } else {
                    assert_eq!(
                        state.source_path(asset).unwrap(),
                        session.source_path(asset).unwrap()
                    );
                }
            }
        }
        live.save(&destination).unwrap();
        assert_eq!(fs::read_dir(destination.join("stems")).unwrap().count(), 1);
        let reopened = Session::open(&destination).unwrap();
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
    }

    #[test]
    fn failed_first_save_retains_temporary_stems_and_the_unsaved_session() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let directory = prepared.directory().to_owned();
        let mut next = prepared.publish();
        let before = next.project.clone();
        let blocked = fixture.0.join("blocked-save");
        fs::create_dir_all(blocked.join("project.json")).unwrap();
        let sentinel = blocked.join("project.json/keep.txt");
        fs::write(&sentinel, b"unrelated contents").unwrap();
        assert!(next.save(&blocked).is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"unrelated contents");
        assert!(blocked.join("project.json").is_dir());
        assert!(fs::read_dir(&blocked).unwrap().all(|entry| {
            let name = entry.unwrap().file_name();
            !name.to_string_lossy().contains("backup-") && !name.to_string_lossy().ends_with(".tmp")
        }));
        assert!(next.folder.is_none());
        assert_eq!(next.project, before);
        assert!(directory.join("drums.wav").is_file());
        assert_eq!(fs::read_dir(blocked.join("stems")).unwrap().count(), 0);
        let saved = fixture.0.join("retry-save");
        next.save(&saved).unwrap();
        assert!(!directory.exists());
        assert_eq!(Session::open(&saved).unwrap().project.tracks.len(), 6);
    }

    #[test]
    fn unsaved_failures_cancellation_and_discard_clean_only_their_temporary_files() {
        let (fixture, mut session, clip) = Fixture::new();
        session.folder = None;
        let before = session.project.clone();
        for cancel in [false, true] {
            let progress = SeparationProgress::default();
            let mut temporary = PathBuf::new();
            let result = session.separate_clip_with(clip, &progress, |samples, path| {
                temporary = path.parent().unwrap().parent().unwrap().to_owned();
                fake(samples, path)?;
                if cancel {
                    progress.cancel.store(true, Ordering::Relaxed);
                    Ok(daw_separation::Output {
                        stems: SeparationStemSet::Four,
                        warnings: Vec::new(),
                    })
                } else {
                    Err(error("Inference failed"))
                }
            });
            assert!(result.is_err());
            assert!(!temporary.as_os_str().is_empty());
            assert!(!temporary.exists());
        }
        let prepared = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap();
        let temporary = prepared
            .directory()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        drop(prepared);
        assert!(!temporary.exists());
        assert_eq!(session.project, before);
        assert!(fixture.0.join("original.wav").is_file());
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
                .all(|a| Path::new(&a.source.path).starts_with("stems"))
        );
        assert_eq!(
            fs::read(fixture.0.join("original.wav")).unwrap(),
            original_bytes
        );
    }
    #[test]
    fn saving_undone_separation_keeps_save_as_paths_for_redo() {
        let (fixture, session, clip) = Fixture::new();
        let mut history = crate::SessionHistory::new(&session, false).unwrap();
        let mut next = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap()
            .publish();
        history.record(&next).unwrap();
        let copy = fixture.0.join("copy");
        next.save(&copy).unwrap();
        // Match UI Job::Saved: retarget history before recording saved metadata.
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(history.is_saved());
        while next.project.tracks.len() != 1 {
            next = history.undo(&next).unwrap();
        }
        assert!(!history.is_saved());
        next.save(&copy).unwrap();
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(history.is_saved());
        assert!(history.can_redo());

        // The copy must be independent of the first project's generated files.
        fs::remove_dir_all(fixture.0.join("project")).unwrap();
        while let Some(redone) = history.redo(&next) {
            next = redone;
            let assets: Vec<_> = next
                .project
                .assets
                .iter()
                .filter(|a| a.source.kind == "generated")
                .collect();
            assert_eq!(assets.len(), 4);
            for asset in assets {
                let path = next.source_path(asset).unwrap();
                assert!(path.starts_with(copy.join("stems")), "{path:?}");
                assert!(path.is_file());
            }
            assert!(!history.is_saved());
        }
        next.save(&copy).unwrap();
        history.mark_saved(&next).unwrap();
        history.record(&next).unwrap();
        assert!(history.is_saved());
        let reopened = Session::open(&copy).unwrap();
        assert_eq!(reopened.project.tracks.len(), 6);
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        assert!(reopened.project.tracks[0].clips[0].muted);
        for track in reopened
            .project
            .tracks
            .iter()
            .filter(|t| t.group_id.is_some())
        {
            assert_eq!(
                reopened
                    .audio_for_clip(&track.clips[0])
                    .unwrap()
                    .samples
                    .len(),
                2400
            );
        }
    }

    #[test]
    fn save_completion_retargets_grouped_history_without_losing_newer_edits() {
        let (fixture, session, clip) = Fixture::new();
        let mut next = session
            .separate_clip_with(clip, &SeparationProgress::default(), fake)
            .unwrap()
            .publish();
        let mut history = crate::SessionHistory::new(&next, false).unwrap();
        let mut saved = next.clone();
        saved.save(&fixture.0.join("copy")).unwrap();
        history.begin_group();
        next.project.name = "Newer grouped edit".into();
        history.record(&next).unwrap();
        history.mark_saved(&saved).unwrap();
        assert!(!history.is_saved());
        history.finish_group();

        let restored = history.undo(&saved).unwrap();
        assert!(history.is_saved());
        let redone = history.redo(&restored).unwrap();
        assert_eq!(redone.project.name, "Newer grouped edit");
        assert!(!history.is_saved());
        for state in [&restored, &redone] {
            let generated: Vec<_> = state
                .project
                .assets
                .iter()
                .filter(|a| a.source.kind == "generated")
                .collect();
            assert_eq!(generated.len(), 4);
            for asset in generated {
                assert!(
                    state
                        .source_path(asset)
                        .unwrap()
                        .starts_with(fixture.0.join("copy"))
                );
            }
        }
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
    fn all_project_rates_accept_exact_model_output_and_reject_truncated_stems() {
        for rate in daw_core::SAMPLE_RATES {
            let (_fixture, session, clip) = Fixture::new();
            let mut session = session.with_audio_settings(rate, 512).unwrap();
            for residue in [0, 1, 2, 3, 9] {
                let length = 2400 + residue;
                session.project.tracks[0].clips[0].length_frames = length;
                let next = session
                    .separate_clip_with(clip, &SeparationProgress::default(), fake)
                    .unwrap_or_else(|e| panic!("rate {rate}, length {length}: {e}"))
                    .publish();
                for track in next.project.tracks.iter().filter(|t| t.group_id.is_some()) {
                    let stem = &track.clips[0];
                    assert_eq!(stem.length_frames, length);
                    let audio = next.audio_for_clip(stem).unwrap();
                    assert_eq!(audio.samples.len() as u64, length);
                    assert!(
                        audio
                            .samples
                            .iter()
                            .all(|s| (s[0] + 2.0 * s[1]).abs() < 1e-5)
                    );
                }
            }
            let before = session.project.clone();
            assert!(
                session
                    .separate_clip_with(clip, &SeparationProgress::default(), |input, path| {
                        fake(&input[..input.len() - 1], path)
                    })
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("model output")
            );
            assert_eq!(session.project, before);
        }
    }

    #[test]
    fn both_post_inference_decodes_honor_cancellation_and_cleanup() {
        for second_decode in [false, true] {
            let (fixture, session, _) = Fixture::new();
            let before = session.project.clone();
            let directory = reserve_directory(&fixture.0.join("stems"), "cancel-test").unwrap();
            fake(&vec![[0.2, -0.1]; 2205], &directory).unwrap();
            let staged = PreparedSeparation {
                stems: SeparationStemSet::Four,
                session: None,
                directory: Some(directory.clone()),
                warnings: Vec::new(),
            };
            let path = directory.join("drums.wav");
            let polls = std::cell::Cell::new(0);
            let cancelled = || {
                // Detect the finalized project-rate file to cancel inside the
                // second decoder, rather than during normalization/encoding.
                let in_stage = !second_decode
                    || hound::WavReader::open(&path)
                        .is_ok_and(|r| r.spec().sample_rate == 48_000 && r.duration() == 2400);
                if in_stage {
                    polls.set(polls.get() + 1);
                }
                polls.get() >= 3
            };
            let failure = prepare_stem(&path, 2205, 2400, 48_000, &cancelled)
                .err()
                .unwrap();
            assert!(failure.to_string().contains("cancelled"), "{failure}");
            assert_eq!(polls.get(), 3);
            drop(staged);
            assert!(!directory.exists());
            assert_eq!(session.project, before);
        }
    }

    #[test]
    fn same_name_jobs_use_distinct_directories() {
        let (_fixture, session, clip) = Fixture::new();
        for saved in [false, true] {
            let mut session = session.clone();
            if !saved {
                session.folder = None;
            }
            let first = session
                .separate_clip_with(clip, &SeparationProgress::default(), fake)
                .unwrap();
            let second = session
                .separate_clip_with(clip, &SeparationProgress::default(), fake)
                .unwrap();
            assert_ne!(first.directory, second.directory);
        }
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
        let expected = match std::env::var("DAW_STEM_COUNT").as_deref() {
            Ok("6") => SeparationStemSet::Six,
            Ok("4") | Err(_) => SeparationStemSet::Four,
            other => panic!("Unsupported expected DAW_STEM_COUNT: {other:?}"),
        };
        let info = crate::validate_separation_model(
            crate::SeparationModelKind::Cpu,
            Path::new(&cpu),
            &SeparationProgress::default(),
        )
        .unwrap();
        assert_eq!(info.stems, expected);
        assert_eq!(info.file_size_bytes, fs::metadata(&cpu).unwrap().len());
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
        assert_eq!(result.stems, expected);
        eprintln!("Model warnings: {:?}", result.warnings);
        let mut session = result.publish();
        session.save(&folder).unwrap();
        let reopened = Session::open(&folder).unwrap();
        assert!(reopened.warnings.is_empty());
        assert_eq!(reopened.project.tracks.len(), expected.names().len() + 2);
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
