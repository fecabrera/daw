use daw_core::{Asset, Clip, Id, Project, Source};
use daw_engine::RenderPlan;
use daw_media::{AudioData, AudioDecoder, AudioEncoder, WavDecoder, WavEncoder};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;
fn error(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}

#[derive(Clone, Default)]
pub struct Session {
    pub project: Project,
    pub audio: HashMap<Id, AudioData>,
    pub folder: Option<PathBuf>,
    pub warnings: Vec<String>,
}
impl Session {
    pub fn plan(&self) -> RenderPlan {
        RenderPlan {
            project: self.project.clone(),
            audio: self.audio.clone(),
        }
    }
    pub fn open(folder: &Path) -> Result<Self> {
        let folder = absolute(folder)?;
        let content = fs::read(folder.join("project.json")).map_err(error)?;
        let project: Project = serde_json::from_slice(&content).map_err(error)?;
        project.validate().map_err(error)?;
        let mut session = Self {
            project,
            audio: HashMap::new(),
            folder: Some(folder),
            warnings: vec![],
        };
        for asset in &session.project.assets {
            let path = session.source_path(asset)?;
            match fs::metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
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
            let data = WavDecoder.decode(&path).map_err(error)?;
            if (data.samples.len() as u64) < asset.decoded_frame_count {
                session.warnings.push(format!("Source {} is shorter than saved metadata; unavailable ranges render as silence",path.display()));
            } else if data.metadata.sample_rate_hz != asset.source_metadata.sample_rate_hz
                || data.metadata.channels != asset.source_metadata.channels
                || data.metadata.bits_per_sample != asset.source_metadata.bits_per_sample
                || data.metadata.sample_format != asset.source_metadata.sample_format
                || data.samples.len() as u64 != asset.decoded_frame_count
            {
                session
                    .warnings
                    .push(format!("Source {} metadata changed", path.display()));
            }
            session.audio.insert(asset.id, data);
        }
        Ok(session)
    }
    pub fn source_path(&self, asset: &Asset) -> Result<PathBuf> {
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
        let data = WavDecoder.decode(&path).map_err(error)?;
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
        let t = next
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or_else(|| error("Select a track for import"))?;
        t.clips.push(Clip {
            id: clip_id,
            asset_id,
            name: name.clone(),
            start_frame: start,
            source_offset_frame: 0,
            length_frames: data.samples.len() as u64,
        });
        next.assets.push(Asset {
            id: asset_id,
            name,
            source: Source {
                kind: "external".into(),
                path: path.to_string_lossy().into_owned(),
                path_kind: "absolute".into(),
            },
            source_metadata: data.metadata.clone(),
            decoded_frame_count: data.samples.len() as u64,
        });
        next.validate().map_err(error)?;
        self.project = next;
        self.audio.insert(asset_id, data);
        Ok(clip_id)
    }
    pub fn save(&mut self, folder: &Path) -> Result<()> {
        let folder = absolute(folder)?;
        fs::create_dir_all(folder.join("assets")).map_err(error)?;
        let mut next = self.project.clone();
        for asset in &mut next.assets {
            let original = self
                .project
                .assets
                .iter()
                .find(|a| a.id == asset.id)
                .unwrap();
            let path = self.source_path(original)?;
            if let Some(relative) = relative_path(&folder, &path) {
                asset.source.path = relative.to_string_lossy().into_owned();
                asset.source.path_kind = "relative".into();
            } else {
                asset.source.path = path.to_string_lossy().into_owned();
                asset.source.path_kind = "absolute".into();
            }
        }
        next.validate().map_err(error)?;
        let bytes = serde_json::to_vec_pretty(&next).map_err(error)?;
        let target = folder.join("project.json");
        let temp = folder.join(format!(".project-{}.tmp", Id::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(error)?;
            file.write_all(&bytes).map_err(error)?;
            file.sync_all().map_err(error)?;
            drop(file);
            replace_file(&temp, &target)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.project = next;
        self.folder = Some(folder);
        Ok(())
    }
    pub fn export(&self, path: &Path, overwrite: bool) -> Result<Vec<String>> {
        self.project.validate().map_err(error)?;
        let end = self.project.end();
        if end == 0 {
            return Err(error("Cannot export a project with no clips"));
        }
        let path = absolute(path)?;
        for asset in &self.project.assets {
            let source = self.source_path(asset)?;
            if same_file(&path, &source) {
                return Err(error("Export cannot overwrite a source audio file"));
            }
        }
        if self
            .folder
            .as_ref()
            .is_some_and(|f| same_file(&path, &f.join("project.json")))
        {
            return Err(error("Export cannot overwrite project.json"));
        }
        if path.exists() && !overwrite {
            return Err(error(
                "Output already exists; confirm overwrite or use --overwrite",
            ));
        }
        let parent = path.parent().ok_or_else(|| error("Invalid output path"))?;
        let temp = parent.join(format!(".export-{}.tmp", Id::new_v4()));
        let result = (|| {
            let plan = self.plan();
            let mut encoder = WavEncoder::create(&temp).map_err(error)?;
            let mut block = vec![[0.0; 2]; 1024];
            let mut position = 0;
            while position < end {
                let count = (end - position).min(1024) as usize;
                plan.render(position, &mut block[..count]);
                encoder.write_frames(&block[..count]).map_err(error)?;
                position += count as u64;
            }
            let clipped = encoder.clipped;
            encoder.finish().map_err(error)?;
            if !overwrite && path.exists() {
                return Err(error("Output appeared during export; refusing overwrite"));
            }
            replace_file(&temp, &path)?;
            let mut warnings = self.warnings.clone();
            if clipped {
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
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
fn relative_path(base: &Path, path: &Path) -> Option<PathBuf> {
    let a = base.components().collect::<Vec<_>>();
    let b = path.components().collect::<Vec<_>>();
    if a.first() != b.first() {
        return None;
    }
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..a.len() {
        out.push("..");
    }
    for c in &b[common..] {
        out.push(c.as_os_str());
    }
    Some(out)
}
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}
fn replace_file(temp: &Path, target: &Path) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(temp, target).map_err(error)
    }
    #[cfg(target_os = "windows")]
    {
        let backup = target.with_extension(format!("backup-{}", Id::new_v4()));
        let exists = target.exists();
        if exists {
            fs::rename(target, &backup).map_err(error)?;
        }
        if let Err(e) = fs::rename(temp, target) {
            if exists {
                let _ = fs::rename(&backup, target);
            }
            return Err(error(e));
        }
        if exists {
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }
}
