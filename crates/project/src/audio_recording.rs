//! Staged recording sources and PCM watermarks for the shared take journal.
use crate::{Result, Session, error, recovery};
use daw_core::{Asset, Clip, ClipPlacement, Id, Source, SourceStorage, TrackKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

const BLOCK_FRAMES: usize = 1024;
const MAX_SPOOL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Destination {
    pub track: Id,
    pub route: usize,
    pub clip: Id,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    pub stereo: bool,
    pub asset: Id,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Layout {
    pub routes: Vec<Route>,
    pub destinations: Vec<Destination>,
}

impl Layout {
    pub fn new(base: &Session, stereo: &[bool], tracks: &[(Id, usize)]) -> Result<Self> {
        let layout = Self {
            routes: stereo
                .iter()
                .map(|&stereo| Route {
                    stereo,
                    asset: Id::new_v4(),
                })
                .collect(),
            destinations: tracks
                .iter()
                .map(|&(track, route)| Destination {
                    track,
                    route,
                    clip: Id::new_v4(),
                })
                .collect(),
        };
        layout.validate(base)?;
        Ok(layout)
    }

    pub fn validate(&self, base: &Session) -> Result<()> {
        let mut tracks = std::collections::HashSet::new();
        let mut assets = std::collections::HashSet::new();
        let mut clips = std::collections::HashSet::new();
        if self.routes.is_empty()
            || self.routes.len() > 16
            || self.destinations.is_empty()
            || self.destinations.len() > 16
            || self.routes.iter().any(|r| {
                !assets.insert(r.asset) || base.project.assets.iter().any(|a| a.id == r.asset)
            })
            || self.destinations.iter().any(|d| {
                !tracks.insert(d.track)
                    || !clips.insert(d.clip)
                    || d.route >= self.routes.len()
                    || !base
                        .project
                        .tracks
                        .iter()
                        .any(|t| t.id == d.track && t.kind == TrackKind::Audio)
                    || base
                        .project
                        .tracks
                        .iter()
                        .flat_map(|t| &t.clips)
                        .any(|clip| clip.id == d.clip)
            })
        {
            return Err(error("Invalid audio recording destinations"));
        }
        Ok(())
    }

    fn stride(&self) -> usize {
        // Keep an entire frame, including every route, in one durable prefix.
        self.routes.len() * 8
    }
}

pub(crate) fn spool_path(folder: &Path, take: Id) -> PathBuf {
    folder.join(format!("take-{take}.pcm"))
}

pub(crate) struct Spool {
    file: File,
    buffer: Vec<u8>,
    stride: usize,
    limit: u64,
    pub frames: u64,
    failed: bool,
}
impl Spool {
    pub fn open(path: &Path, layout: &Layout, rate: u32) -> Result<Self> {
        let stride = layout.stride();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(error)?;
        file.sync_all().map_err(error)?;
        Ok(Self {
            file,
            buffer: Vec::with_capacity(stride * BLOCK_FRAMES),
            stride,
            limit: (u64::from(rate) * 30 * 60).min(MAX_SPOOL_BYTES / stride as u64),
            frames: 0,
            failed: false,
        })
    }

    pub fn push(&mut self, samples: &[[f32; 2]]) -> Result<bool> {
        if self.failed {
            return Err(error("Audio recording spool is unavailable"));
        }
        if samples.len() * 8 != self.stride || samples.iter().flatten().any(|s| !s.is_finite()) {
            return Err(error("Invalid audio recording frame"));
        }
        if self.frames + (self.buffer.len() / self.stride) as u64 >= self.limit {
            return Err(error(
                "The audio take reached its storage or duration limit",
            ));
        }
        for sample in samples.iter().flatten() {
            self.buffer.extend(sample.to_le_bytes());
        }
        Ok(self.buffer.len() >= self.stride * BLOCK_FRAMES)
    }

    pub fn flush(&mut self) -> Result<Option<(u32, [u8; 32])>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        if let Err(failure) = self
            .file
            .write_all(&self.buffer)
            .and_then(|()| self.file.sync_data())
        {
            self.failed = true;
            self.buffer.clear();
            return Err(error(failure));
        }
        let frames = (self.buffer.len() / self.stride) as u32;
        let hash = Sha256::digest(&self.buffer).into();
        self.frames += u64::from(frames);
        self.buffer.clear();
        Ok(Some((frames, hash)))
    }
}

pub(crate) struct VerifiedSpool {
    data: BufReader<File>,
    stride: usize,
    bytes: Vec<u8>,
    pub frames: u64,
}
impl VerifiedSpool {
    pub fn open(path: &Path, layout: &Layout) -> Result<Self> {
        recovery::ordinary(path, false)?;
        Ok(Self {
            data: BufReader::new(File::open(path).map_err(error)?),
            stride: layout.stride(),
            bytes: Vec::new(),
            frames: 0,
        })
    }
    pub fn advances(&mut self, frames: u32, hash: [u8; 32]) -> bool {
        if frames == 0
            || frames as usize > BLOCK_FRAMES
            || (self.frames + u64::from(frames)) * self.stride as u64 > MAX_SPOOL_BYTES
        {
            return false;
        }
        self.bytes.resize(frames as usize * self.stride, 0);
        if self.data.read_exact(&mut self.bytes).is_err()
            || <[u8; 32]>::from(Sha256::digest(&self.bytes)) != hash
        {
            return false;
        }
        self.frames += u64::from(frames);
        true
    }
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Clone)]
pub(crate) struct StagedSource {
    relative: String,
    dir: Arc<Directory>,
}
impl StagedSource {
    fn path(&self) -> PathBuf {
        self.dir.0.join(&self.relative)
    }
}

fn directory() -> Result<Arc<Directory>> {
    let root = std::env::temp_dir().canonicalize().map_err(error)?;
    let dir = root.join(format!("daw-recording-{}", Id::new_v4()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(error)?;
    }
    #[cfg(not(unix))]
    fs::create_dir(&dir).map_err(error)?;
    let dir = Arc::new(Directory(dir));
    fs::create_dir_all(dir.0.join("assets/sources")).map_err(error)?;
    Ok(dir)
}

impl Session {
    pub(crate) fn staged_recording_path(&self, asset: &Asset) -> Option<PathBuf> {
        self.recorded_sources
            .get(&asset.id)
            .filter(|source| {
                asset.source.is_included_recording() && source.relative == asset.source.path
            })
            .map(StagedSource::path)
    }

    pub(crate) fn retain_recorded_sources(&mut self) {
        self.recorded_sources.retain(|&id, source| {
            self.project.assets.iter().any(|a| {
                a.id == id && a.source.is_included_recording() && a.source.path == source.relative
            })
        });
    }

    pub fn adopt_recorded_sources(&mut self, candidate: &Session) {
        for (&id, owner) in &candidate.recorded_sources {
            self.recorded_sources.insert(id, owner.clone());
            if let Some(data) = candidate.audio.get(&id) {
                self.audio.insert(id, data.clone());
                self.missing_audio.remove(&id);
            }
        }
    }

    /// A retry restores IDs and prepared media from its durable receipt while
    /// leaving the currently opened project's other media paths alone.
    pub fn load_recovered_recordings(
        &mut self,
        recovered: &Session,
        previous: &Session,
    ) -> Result<()> {
        let recordings: Vec<_> = recovered
            .project
            .assets
            .iter()
            .filter(|a| {
                a.source.is_included_recording()
                    && !previous.project.assets.iter().any(|old| old.id == a.id)
            })
            .cloned()
            .collect();
        if recordings.is_empty() {
            return Ok(());
        }
        let dir = directory()?;
        for asset in recordings {
            let owner = StagedSource {
                relative: asset.source.path.clone(),
                dir: dir.clone(),
            };
            let source = recovered.source_path(&asset)?;
            fs::copy(source, owner.path()).map_err(error)?;
            let data = daw_media::decode_audio_bounded(
                &owner.path(),
                recovered.project.sample_rate_hz,
                &|| false,
            )
            .map_err(error)?;
            self.recorded_sources.insert(asset.id, owner);
            self.audio.insert(asset.id, data);
        }
        Ok(())
    }

    pub(crate) fn stage_recorded_audio(
        &mut self,
        path: &Path,
        layout: &Layout,
        origin: u64,
        frames: u64,
    ) -> Result<(Vec<Asset>, Vec<ClipPlacement>)> {
        if frames == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        layout.validate(self)?;
        origin
            .checked_add(frames)
            .ok_or_else(|| error("Audio take exceeds timeline bounds"))?;
        recovery::ordinary(path, false)?;
        let dir = directory()?;
        let mut owners = Vec::new();
        let mut outputs = Vec::new();
        for route in &layout.routes {
            let relative = format!("assets/sources/{}.wav", route.asset);
            let owner = StagedSource {
                relative,
                dir: dir.clone(),
            };
            let spec = hound::WavSpec {
                channels: if route.stereo { 2 } else { 1 },
                sample_rate: self.project.sample_rate_hz,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            outputs.push(hound::WavWriter::create(owner.path(), spec).map_err(error)?);
            owners.push(owner);
        }
        let mut reader = BufReader::new(File::open(path).map_err(error)?);
        let mut bytes = vec![0; layout.stride()];
        for _ in 0..frames {
            reader.read_exact(&mut bytes).map_err(error)?;
            for ((writer, route), stereo) in outputs
                .iter_mut()
                .zip(&layout.routes)
                .zip(bytes.chunks_exact(8))
            {
                for sample in stereo
                    .chunks_exact(4)
                    .take(if route.stereo { 2 } else { 1 })
                {
                    writer
                        .write_sample(f32::from_le_bytes(sample.try_into().unwrap()))
                        .map_err(error)?;
                }
            }
        }
        for writer in outputs {
            writer.finalize().map_err(error)?;
        }
        let mut assets = Vec::new();
        for (route, owner) in layout.routes.iter().zip(owners) {
            let data = daw_media::decode_audio_bounded(
                &owner.path(),
                self.project.sample_rate_hz,
                &|| false,
            )
            .map_err(error)?;
            if data.frame_count() != frames {
                return Err(error("Recorded WAV has an unexpected length"));
            }
            assets.push(Asset {
                id: route.asset,
                name: "Audio take".into(),
                source: Source {
                    kind: "external".into(),
                    storage: Some(SourceStorage::Project),
                    path_kind: "relative".into(),
                    path: owner.relative.clone(),
                },
                source_metadata: data.metadata.clone(),
                decoded_frame_count: frames,
                midi_pattern: None,
                rendered_audio: None,
                compound_audio: None,
                beat_pattern: None,
            });
            self.recorded_sources.insert(route.asset, owner);
            self.audio.insert(route.asset, data);
        }
        let placements = layout
            .destinations
            .iter()
            .map(|dest| ClipPlacement {
                track_id: dest.track,
                clip: Clip {
                    id: dest.clip,
                    asset_id: layout.routes[dest.route].asset,
                    name: "Audio take".into(),
                    start_frame: origin,
                    source_offset_frame: 0,
                    length_frames: frames,
                    channels: Default::default(),
                    fades: Default::default(),
                    reversed: false,
                    muted: false,
                    stretch: None,
                    color: None,
                    repeat: None,
                    gain_db: 0.0,
                    transpose: 0,
                },
            })
            .collect();
        Ok((assets, placements))
    }
}
