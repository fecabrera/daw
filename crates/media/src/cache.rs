//! Weak decoded-source cache. Asset IDs remain separate from immutable PCM ownership.
use crate::{AudioData, Error, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::SystemTime,
};

#[derive(Clone, PartialEq, Eq)]
struct Revision {
    length: u64,
    modified: SystemTime,
}
struct Cached {
    revision: Revision,
    content: crate::SourceRevision,
    samples: Weak<Vec<[f32; 2]>>,
    disk: Option<Weak<crate::DiskAudio>>,
    peaks: Weak<Vec<[[f32; 2]; 2]>>,
    metadata: daw_core::SourceMetadata,
}
type Cache = HashMap<(PathBuf, u32), Cached>;
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn revision(path: &Path) -> Result<Revision> {
    let metadata =
        std::fs::metadata(path).map_err(|e| Error(format!("{}: {e}", path.display())))?;
    Ok(Revision {
        length: metadata.len(),
        modified: metadata.modified().map_err(|e| Error(e.to_string()))?,
    })
}
/// Worker-only lookup. Modification time and length invalidate ordinary source replacements.
/// No cache owner extends PCM lifetime, and a changed file during decode is never published.
pub fn decode_audio_cached(
    path: &Path,
    rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Audio preparation cancelled".into()));
    }
    let path =
        std::fs::canonicalize(path).map_err(|e| Error(format!("{}: {e}", path.display())))?;
    let before = revision(&path)?;
    let key = (path, rate);
    let receipt = crate::SourceRevision::read(&key.0, cancelled)?;
    {
        let mut cache = cache().lock().unwrap();
        cache.retain(|_, v| v.samples.strong_count() > 0 && v.peaks.strong_count() > 0);
        if let Some(cached) = cache
            .get(&key)
            .filter(|c| c.revision == before && c.content == receipt)
            && let Some((samples, peaks)) = cached.samples.upgrade().zip(cached.peaks.upgrade())
            && let Some(disk) = cached
                .disk
                .as_ref()
                .map_or(Some(None), |disk| disk.upgrade().map(Some))
        {
            crate::preparation_cache::record_live_hit();
            return Ok(AudioData {
                disk,
                sample_rate_hz: rate,
                samples,
                peaks,
                metadata: cached.metadata.clone(),
            });
        }
    }
    let disk_key = crate::preparation_cache::key(
        b"decoded-verified-v2",
        &[&receipt.sha256, &rate.to_le_bytes()],
    );
    let build = || {
        let snapshot = crate::source_revision::SourceSnapshot::read(&key.0, cancelled)?;
        if snapshot.revision != receipt {
            return Err(Error(
                "Source changed during audio preparation; try again".into(),
            ));
        }
        let audio = crate::decode_audio_cancellable(&snapshot.path, rate, cancelled)?;
        if before != revision(&key.0)? || receipt != crate::SourceRevision::read(&key.0, cancelled)?
        {
            return Err(Error(
                "Source changed during audio preparation; try again".into(),
            ));
        }
        Ok(audio)
    };
    // Large WAVs are already disk-backed. Do not load a legacy whole-buffer
    // preparation entry or persist an empty resident placeholder.
    let audio = if crate::stream_decode::large_wav(&key.0, rate) {
        build()?
    } else {
        crate::preparation_cache::cached_audio(disk_key, cancelled, build)?
    };
    if before != revision(&key.0)? || receipt != crate::SourceRevision::read(&key.0, cancelled)? {
        return Err(Error(
            "Source changed during audio preparation; try again".into(),
        ));
    }
    cache().lock().unwrap().insert(
        key,
        Cached {
            revision: before,
            content: receipt,
            samples: Arc::downgrade(&audio.samples),
            disk: audio.disk.as_ref().map(Arc::downgrade),
            peaks: Arc::downgrade(&audio.peaks),
            metadata: audio.metadata.clone(),
        },
    );
    Ok(audio)
}
