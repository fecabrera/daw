mod source_tempo;
pub use source_tempo::{TempoCatalog, TempoSource};
mod audio_recording;
mod bounce;
mod bounce_tail;
mod generated_media;
pub mod midi_input;
pub mod midi_recording;
pub mod recovery;
mod stem_export;
pub use bounce::{BounceError, BounceProgress, PreparedBounce};
mod normalize;
pub use normalize::{NormalizeMode, NormalizeOptions, NormalizeProgress};
mod background_audio;
mod effect_warnings;
pub use background_audio::BackgroundAudio;
mod audition;
pub use audition::NoteAudition;
mod separation;
pub use daw_separation::{
    AVAILABLE as SEPARATION_AVAILABLE, ModelInfo as SeparationModelInfo,
    ModelKind as SeparationModelKind, StemSet as SeparationStemSet,
    UNAVAILABLE_REASON as SEPARATION_UNAVAILABLE_REASON,
    validate_model as validate_separation_model,
};
pub use separation::{PreparedSeparation, SeparationModels, SeparationProgress};
mod inspection;
pub use inspection::inspect_project;
mod history;
mod hosted;
pub use history::{HistoryMemory, SessionHistory};
mod audio_merge;
mod clip_audio;
mod compound_audio;
mod export_destination;
mod file_publish;
mod relative_path;
use relative_path::relative_path;
mod sampler;
pub mod source_collection;
mod source_save;
mod source_storage;
mod write_guard;
use daw_core::{Asset, Clip, Id, Project, Source};
use daw_engine::RenderPlan;
use daw_media::{AudioData, AudioEncoder, ExportEncoder, ExportFormat, ExportSettings, WavCodec};
pub use source_save::{SaveSnapshot, SavedSession};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};
pub use write_guard::{ManifestRevision, ProjectWriteGuard};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;
fn error(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}

const MAX_FOLDER_NAME_BYTES: usize = 255;
const EXPORT_TAIL_WARNING_WINDOW_MS: u64 = 50;
const EXPORT_TAIL_WARNING_PEAK: f32 = 0.001; // -60 dBFS

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportRegion {
    pub start_frame: u64,
    pub end_frame: u64,
    pub include_tail: bool,
}

/// A portable folder name, shared by the naming dialog and project creation.
pub fn project_folder_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(error("Enter a project name"));
    }
    if name.len() > MAX_FOLDER_NAME_BYTES {
        return Err(error("Project name is too long"));
    }
    if name.ends_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err(error(
            "Use a folder name without / \\ : * ? \" < > | or a trailing period",
        ));
    }
    let base = name
        .split('.')
        .next()
        .unwrap()
        .trim_end()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str())
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
    {
        return Err(error("This folder name is reserved; choose another name"));
    }
    Ok(name)
}

#[derive(Clone, Default)]
pub struct Session {
    pub plugin_host: Option<std::sync::Arc<daw_plugin_host::HostClient>>,
    pub plugin_states: daw_plugin_host::StateBank,
    pub project: Project,
    pub audio: HashMap<Id, AudioData>,
    pub sampler_audio: HashMap<Id, daw_media::PreparedSampler>,
    /// Transient exact pitches requested by live input; never serialized.
    pub live_midi_pitches: HashMap<Id, Vec<u8>>,
    pub stretched_audio: daw_media::ClipAudioCache,
    /// Worker-compiled sparse waveform clocks. No bins are allocated for gaps.
    pub compound_views: HashMap<Id, std::sync::Arc<daw_core::CompiledAudioTrack>>,
    tempo_catalog: Option<std::sync::Arc<TempoCatalog>>,
    pub folder: Option<PathBuf>,
    pub warnings: Vec<String>,
    // A verified missing source may render silence; an undecoded source must wait.
    missing_audio: HashSet<Id>,
    // Clones and history share ownership until generated sources are saved or released.
    temporary_stems: Vec<std::sync::Arc<separation::TemporaryStems>>,
    recorded_sources: HashMap<Id, audio_recording::StagedSource>,
    recovery_owner: Option<std::sync::Arc<recovery::RecoveryStore>>,
    /// Only an interrupted, not-yet-published recovered take restores one history step.
    pub recovered_take_before: Option<Box<Session>>,
}
/// A validated document without prepared media. Do not construct a renderer from its editor state.
pub struct ProjectDocument {
    session: Session,
    stretch_upgrade_notice: Option<String>,
}
impl ProjectDocument {
    /// Allow the desktop to present the upgrade separately after audio preparation.
    pub fn take_stretch_upgrade_notice(&mut self) -> Option<String> {
        self.stretch_upgrade_notice.take()
    }

    /// The editor stores this model while BackgroundAudio builds a complete session.
    /// Callers must guard all playback/preview paths until that completion arrives.
    pub fn into_editor_session(mut self) -> Session {
        if let Some(notice) = self.stretch_upgrade_notice.take() {
            self.session.warnings.insert(0, notice);
        }
        self.session
    }
}

impl Session {
    pub fn recovered_working_copy(&self) -> bool {
        self.recovery_owner.as_ref().is_some_and(|owner| {
            self.folder.as_ref().is_some_and(|folder| {
                folder.parent() == Some(owner.folder.as_path())
                    && folder
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("restored-"))
            })
        })
    }
    /// Prepare an atomic project-rate change on a worker. Source files stay untouched.
    pub fn with_audio_settings(
        &self,
        sample_rate_hz: u32,
        buffer_size_frames: u32,
    ) -> Result<Self> {
        self.with_processing_settings(
            sample_rate_hz,
            buffer_size_frames,
            self.project.stretch_engine,
        )
    }

    /// Prepare rate and engine changes together before publishing any project data.
    pub fn with_processing_settings(
        &self,
        sample_rate_hz: u32,
        buffer_size_frames: u32,
        engine: daw_core::StretchEngine,
    ) -> Result<Self> {
        self.with_processing_settings_cancellable(
            sample_rate_hz,
            buffer_size_frames,
            engine,
            &|| false,
        )
    }
    pub fn with_processing_settings_cancellable(
        &self,
        sample_rate_hz: u32,
        buffer_size_frames: u32,
        engine: daw_core::StretchEngine,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        if cancelled() {
            return Err(error("Audio preparation cancelled"));
        }
        engine.validate().map_err(error)?;
        let mut project = self
            .project
            .with_audio_settings(sample_rate_hz, buffer_size_frames)
            .map_err(error)?;
        project.stretch_engine = engine;
        let mut next = self.clone();
        next.project = project;
        if sample_rate_hz == self.project.sample_rate_hz {
            if engine != self.project.stretch_engine {
                next.stretched_audio.clear();
                next.prepare_stretches_cancellable(cancelled)?;
            }
            return Ok(next);
        }
        for asset in &self.project.assets {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            if !asset.is_file_audio() {
                continue;
            }
            let Some(cached) = self.audio.get(&asset.id) else {
                continue;
            };
            let path = self.source_path(asset)?;
            let audio = if path.exists() {
                daw_media::decode_audio_cached(&path, sample_rate_hz, cancelled).map_err(error)?
            } else {
                next.warnings.push(format!(
                    "{}: source unavailable; converted cached audio",
                    asset.name
                ));
                cached.at_sample_rate(sample_rate_hz).map_err(error)?
            };
            next.audio.insert(asset.id, audio);
        }
        next.stretched_audio.clear();
        next.prepare_stretches_cancellable(cancelled)?;
        Ok(next)
    }

    pub fn plan(&self) -> RenderPlan {
        RenderPlan {
            external: daw_engine::ExternalPreparation {
                host: self.plugin_host.clone(),
                states: self.plugin_states.clone(),
                offline: false,
                publish: true,
            },
            beat_tracks: Default::default(),
            project: self.project.clone(),
            audio: self.audio.clone(),
            sampler_audio: self.sampler_audio.clone(),
            stretched_audio: self.stretched_audio.clone(),
        }
    }
    pub fn open(folder: &Path) -> Result<Self> {
        Self::open_document(folder)?
            .into_editor_session()
            .prepare_owned_audio(&|| false, &|_, _| {}, &|_| {})
    }
    pub fn open_document(folder: &Path) -> Result<ProjectDocument> {
        let folder = absolute(folder)?;
        let (project, previous_stretch_revision) = inspection::load_manifest(&folder)?;
        Self::document_from_manifest(folder, project, previous_stretch_revision)
    }

    fn document_from_manifest(
        folder: PathBuf,
        project: Project,
        previous_stretch_revision: Option<u32>,
    ) -> Result<ProjectDocument> {
        let mut session = Self {
            plugin_host: None,
            plugin_states: Default::default(),
            project,
            audio: HashMap::new(),
            sampler_audio: Default::default(),
            live_midi_pitches: Default::default(),
            stretched_audio: HashMap::new(),
            compound_views: HashMap::new(),
            tempo_catalog: None,
            folder: Some(folder),
            temporary_stems: Vec::new(),
            recorded_sources: HashMap::new(),
            recovery_owner: None,
            recovered_take_before: None,
            missing_audio: HashSet::new(),
            warnings: Vec::new(),
        };
        session
            .warnings
            .extend(effect_warnings::missing(&session.project));
        for (_, settings) in session.project.hosted_units() {
            if let Some(reference) = &settings.state {
                session
                    .plugin_states
                    .load(session.folder.as_deref().expect("opened folder"), reference)
                    .map_err(error)?;
            }
        }
        let stretch_upgrade_notice = previous_stretch_revision.map(|previous| {
            format!(
                "Integrated time stretch upgraded from revision {previous} to {}. Prepared audio was rebuilt. The sound can change. Save to retain the new revision.",
                daw_core::StretchEngine::INTEGRATED.revision
            )
        });
        Ok(ProjectDocument {
            session,
            stretch_upgrade_notice,
        })
    }
    /// Worker-only completion of a document or edited model. Existing immutable buffers are reused.
    pub fn prepare_project_audio(
        &mut self,
        cancelled: &(impl Fn() -> bool + Sync),
        progress: &impl Fn(u64, u64),
    ) -> Result<()> {
        *self = self
            .clone()
            .prepare_owned_audio(cancelled, progress, &|_| {})?;
        Ok(())
    }
    /// Prepare a private candidate. On failure, dropping it discards every
    /// partial change. Borrowed callers must keep their original until success.
    fn prepare_owned_audio(
        mut self,
        cancelled: &(impl Fn() -> bool + Sync),
        progress: &impl Fn(u64, u64),
        decoded: &impl Fn(&HashMap<Id, AudioData>),
    ) -> Result<Self> {
        let session = &mut self;
        let total = session
            .project
            .assets
            .iter()
            .filter(|asset| asset.is_file_audio())
            .count() as u64
            + 1;
        let mut done = 0;
        progress(done, total);
        for asset in &session.project.assets {
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            if !asset.is_file_audio() {
                continue;
            }
            if session.audio.contains_key(&asset.id) {
                session.missing_audio.remove(&asset.id);
                continue;
            }
            done += 1;
            progress(done, total);
            let path = session.source_path(asset)?;
            match fs::metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    session.missing_audio.insert(asset.id);
                    let clips = session
                        .project
                        .tracks
                        .iter()
                        .flat_map(|t| &t.clips)
                        .filter(|c| c.asset_id == asset.id)
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    session.warnings.push(format!(
                        "Missing source {} (clips: {clips}); rendered as silence",
                        path.display()
                    ));
                    continue;
                }
                Err(e) => return Err(error(format!("{}: {e}", path.display()))),
                _ => {}
            }
            let data =
                daw_media::decode_audio_cached(&path, session.project.sample_rate_hz, cancelled)
                    .map_err(error)?;
            if (data.frame_count()) < asset.decoded_frame_count {
                session.warnings.push(format!("Source {} is shorter than saved metadata; unavailable ranges render as silence",path.display()));
            } else if data.metadata.sample_rate_hz != asset.source_metadata.sample_rate_hz
                || data.metadata.channels != asset.source_metadata.channels
                || data.metadata.bits_per_sample != asset.source_metadata.bits_per_sample
                || data.metadata.sample_format != asset.source_metadata.sample_format
                || data.frame_count() != asset.decoded_frame_count
            {
                session
                    .warnings
                    .push(format!("Source {} metadata changed", path.display()));
            }
            session.audio.insert(asset.id, data);
            session.missing_audio.remove(&asset.id);
            if cancelled() {
                return Err(error("Audio preparation cancelled"));
            }
            decoded(&session.audio);
        }
        if cancelled() {
            return Err(error("Audio preparation cancelled"));
        }
        session.prepare_stretches_in_place(cancelled, |candidate| {
            candidate.prepare_sampler_audio_parallel(cancelled)
        })?;
        progress(total, total);
        Ok(self)
    }
    pub fn source_path(&self, asset: &Asset) -> Result<PathBuf> {
        asset.source.validate_storage().map_err(error)?;
        if asset.source.is_included_recording() {
            if let Some(path) = self.staged_recording_path(asset) {
                recovery::ordinary(&path, false)?;
                return Ok(path);
            }
            return source_storage::included_path(
                self.folder
                    .as_deref()
                    .ok_or_else(|| error("Included source requires project folder"))?,
                &asset.source.path,
            );
        }
        self.source_path_unchecked(asset)
    }
    /// Pure path resolution for captured/validated owners; no filesystem access.
    pub(crate) fn source_path_unchecked(&self, asset: &Asset) -> Result<PathBuf> {
        asset.source.validate_storage().map_err(error)?;
        if !asset.is_file_audio() {
            return Err(error("Embedded sources have no file path"));
        }
        if let Some(path) = self.staged_recording_path(asset) {
            return Ok(path);
        }
        let p = PathBuf::from(&asset.source.path);
        if asset.source.path_kind == "absolute" {
            if !p.is_absolute() {
                return Err(error(
                    "Absolute source path is not absolute on this platform",
                ));
            }
            Ok(p)
        } else {
            if p.is_absolute() {
                return Err(error("Relative source path must not be absolute"));
            }
            Ok(normalize(
                &self
                    .folder
                    .as_ref()
                    .ok_or_else(|| error("Relative source requires project folder"))?
                    .join(p),
            ))
        }
    }
    pub fn import(&mut self, path: &Path, track: Option<Id>, start: u64) -> Result<Id> {
        let path = absolute(path)?;
        let data = daw_media::decode_audio_cached(&path, self.project.sample_rate_hz, &|| false)
            .map_err(error)?;
        self.import_decoded(&path, data, track, start)
    }
    /// Import audio prepared by the file-drag preview worker without decoding it again.
    pub fn import_decoded(
        &mut self,
        path: &Path,
        data: AudioData,
        track: Option<Id>,
        start: u64,
    ) -> Result<Id> {
        self.import_decoded_mode(path, data, track, start, false)
    }

    /// File drops take priority over existing clips, retaining uncovered source ranges.
    pub fn import_decoded_overwrite(
        &mut self,
        path: &Path,
        data: AudioData,
        track: Option<Id>,
        start: u64,
    ) -> Result<Id> {
        self.import_decoded_mode(path, data, track, start, true)
    }

    fn import_decoded_mode(
        &mut self,
        path: &Path,
        data: AudioData,
        track: Option<Id>,
        start: u64,
        overwrite: bool,
    ) -> Result<Id> {
        if let Some(id) = track
            && !self
                .project
                .tracks
                .iter()
                .any(|t| t.id == id && t.kind == daw_core::TrackKind::Audio)
        {
            return Err(Error("Audio imports require an audio track".into()));
        }
        let data = data
            .at_sample_rate(self.project.sample_rate_hz)
            .map_err(error)?;
        let path = absolute(path)?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let asset_id = Id::new_v4();
        let clip_id = Id::new_v4();
        let mut next = self.project.clone();
        let track_id = match track {
            Some(id) => id,
            None => next.add_track().map_err(error)?,
        };
        let clip = Clip {
            channels: Default::default(),
            fades: Default::default(),
            reversed: false,
            muted: false,
            stretch: None,
            id: clip_id,
            asset_id,
            name: name.clone(),
            color: None,
            start_frame: start,
            source_offset_frame: 0,
            length_frames: data.frame_count(),
            repeat: None,
            gain_db: 0.0,
            transpose: 0,
        };
        next.assets.push(Asset {
            midi_pattern: None,
            rendered_audio: None,
            compound_audio: None,
            beat_pattern: None,
            id: asset_id,
            name,
            source: Source {
                kind: "external".into(),
                path: path.to_string_lossy().into_owned(),
                storage: None,
                path_kind: "absolute".into(),
            },
            source_metadata: data.metadata.clone(),
            decoded_frame_count: data.frame_count(),
        });
        if overwrite {
            next.edit(daw_core::Edit::OverwriteClips(vec![
                daw_core::ClipPlacement { track_id, clip },
            ]))
            .map_err(error)?;
        } else {
            next.edit(daw_core::Edit::InsertClip { track_id, clip })
                .map_err(error)?;
        }

        self.project = next;
        self.audio.insert(asset_id, data);
        Ok(clip_id)
    }
    /// Creates a named project inside an existing parent without replacing another folder.
    pub fn save_new(&mut self, parent: &Path, name: &str) -> Result<()> {
        let name = project_folder_name(name)?;
        let folder = absolute(parent)?.join(name);
        let mut next = self.clone();
        next.project.name = name.to_owned();
        next.project.validate().map_err(error)?;
        fs::create_dir(&folder).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                error("A folder with this project name already exists; choose another name or location")
            } else {
                error(e)
            }
        })?;
        if let Err(e) = next.save(&folder) {
            // Only remove empty directories; retain any unrelated files that appeared meanwhile.
            let _ = fs::remove_dir(folder.join("assets"));
            let _ = fs::remove_dir(folder.join("stems"));
            let _ = fs::remove_dir(&folder);
            return Err(e);
        }
        *self = next;
        Ok(())
    }
    pub fn save(&mut self, folder: &Path) -> Result<()> {
        let saved = self.save_snapshot(None)?.save(folder)?;
        *self = saved.session;
        Ok(())
    }
    pub fn export(&self, path: &Path, overwrite: bool) -> Result<Vec<String>> {
        self.export_with_settings(path, overwrite, ExportSettings::default())
    }
    pub fn export_with_settings(
        &self,
        path: &Path,
        overwrite: bool,
        settings: ExportSettings,
    ) -> Result<Vec<String>> {
        self.export_with_progress(path, overwrite, settings, |_, _| {})
    }
    /// Reports encoded timeline frames; successful return also confirms file finalization.
    pub fn export_with_progress(
        &self,
        path: &Path,
        overwrite: bool,
        settings: ExportSettings,
        progress: impl FnMut(u64, u64),
    ) -> Result<Vec<String>> {
        self.export_interval(path, overwrite, settings, None, progress)
    }

    /// Render an exact slice of the offline mix, including processing history before the start.
    pub fn export_range(
        &self,
        path: &Path,
        settings: ExportSettings,
        range: std::ops::Range<u64>,
    ) -> Result<Vec<String>> {
        let arranged_end = self.project.end();
        if arranged_end != 0 && (range.start >= range.end || range.end > arranged_end) {
            return Err(error(
                "Export range must be inside the arrangement and have positive length",
            ));
        }
        self.export_region_with_progress(
            path,
            false,
            settings,
            ExportRegion {
                start_frame: range.start,
                end_frame: range.end,
                include_tail: false,
            },
            |_, _| {},
        )
    }

    /// Render a desktop export interval. The end may follow the last clip.
    pub fn export_region_with_progress(
        &self,
        path: &Path,
        overwrite: bool,
        settings: ExportSettings,
        region: ExportRegion,
        progress: impl FnMut(u64, u64),
    ) -> Result<Vec<String>> {
        self.export_interval(path, overwrite, settings, Some(region), progress)
    }

    fn export_interval(
        &self,
        path: &Path,
        overwrite: bool,
        settings: ExportSettings,
        region: Option<ExportRegion>,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Vec<String>> {
        self.project.validate().map_err(error)?;
        let arranged_end = self.project.end();
        if arranged_end == 0 {
            return Err(error("Cannot export a project with no clips"));
        }
        let region = region.unwrap_or(ExportRegion {
            start_frame: 0,
            end_frame: arranged_end,
            include_tail: true,
        });
        if region.start_frame >= region.end_frame {
            return Err(error("Export range must have positive length"));
        }
        let requested = absolute(path)?;
        let path = file_publish::canonical_entry(&requested).map_err(error)?;
        self.check_export_destination(&path, overwrite)?;
        let parent = path.parent().ok_or_else(|| error("Invalid output path"))?;
        let temp = parent.join(format!(".export-{}.tmp", Id::new_v4()));
        let result = (|| {
            let mut snapshot = self.clone();
            if region.include_tail && region.end_frame < arranged_end {
                let tracks: Vec<_> = snapshot
                    .project
                    .tracks
                    .iter()
                    .map(|track| track.id)
                    .collect();
                snapshot.project = snapshot
                    .project
                    .without_clip_range(&tracks, region.end_frame, arranged_end)
                    .map_err(error)?;
            }
            snapshot.prepare_stretches_in_place(&|| false, |candidate| {
                candidate.prepare_sampler_audio(&|| false)
            })?;
            let mut plan = snapshot.plan();
            plan.external.offline = true;
            plan.external.publish = false;
            let mut renderer = daw_engine::Renderer::try_new(plan).map_err(error)?;
            renderer.try_seek(0).map_err(error)?;
            let start = region.start_frame;
            let requested_tail = if region.include_tail {
                renderer.offline_tail_frames()
            } else {
                0
            };
            let tail_cap = daw_engine::offline_tail_cap(self.project.sample_rate_hz);
            let end = region
                .end_frame
                .checked_add(requested_tail.min(tail_cap))
                .ok_or_else(|| error("Export duration exceeds the supported frame range"))?;
            let cutoff_check_start = (requested_tail > tail_cap).then(|| {
                end - u64::from(self.project.sample_rate_hz) * EXPORT_TAIL_WARNING_WINDOW_MS / 1000
            });
            let mut audible_cutoff = false;
            progress(0, end - start);
            let mut encoder =
                ExportEncoder::at_sample_rate(&temp, settings, self.project.sample_rate_hz)
                    .map_err(error)?;
            let block_size = self.project.buffer_size_frames as usize;
            let mut block = vec![[0.0; 2]; block_size];
            let mut position = 0;
            while position < end {
                let count = (end - position).min(block_size as u64) as usize;
                renderer.render_offline(&mut block[..count]);
                if let Some(unit) = renderer.hosted_fault() {
                    return Err(error(format!(
                        "Plugin processing failed in unit {unit}; export was not published"
                    )));
                }
                if let Some(message) = renderer.source_error() {
                    return Err(error(message));
                }
                if !audible_cutoff
                    && let Some(start) = cutoff_check_start
                    && position + count as u64 > start
                {
                    let offset = start.saturating_sub(position) as usize;
                    audible_cutoff = block[offset..count]
                        .iter()
                        .flat_map(|&frame| settings.channels.mix_frame(frame))
                        .any(|sample| sample.abs() >= EXPORT_TAIL_WARNING_PEAK);
                }
                let skip = start.saturating_sub(position).min(count as u64) as usize;
                if skip < count {
                    encoder.write_frames(&block[skip..count]).map_err(error)?;
                }
                position += count as u64;
                progress(position.saturating_sub(start), end - start);
            }
            let clipped = encoder.finish_with_clipping().map_err(error)?;
            if file_publish::canonical_entry(&requested).map_err(error)? != path {
                return Err(error("Output folder changed during export"));
            }
            self.check_export_destination(&path, overwrite)?;
            if overwrite {
                replace_file(&temp, &path)?;
            } else {
                file_publish::create(&temp, &path).map_err(error)?;
            }
            let mut warnings = self.warnings.clone();
            if audible_cutoff {
                warnings.push("Audio tail limited to 10 seconds after the export end; the remaining tail was truncated".into());
            }
            if clipped
                && !(settings.format == ExportFormat::Wav
                    && settings.wav_codec == WavCodec::Float32)
            {
                warnings
                    .push("Master output clipped; lower master gain to avoid distortion".into());
            }
            Ok(warnings)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
pub fn absolute(path: &Path) -> Result<PathBuf> {
    Ok(normalize(&if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().map_err(error)?.join(path)
    }))
}
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            // Parent components must be resolved by the filesystem after any
            // preceding symlink, not cancelled against lexical components.
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}
fn replace_file(temp: &Path, target: &Path) -> Result<()> {
    file_publish::replace(temp, target).map_err(error)
}
