//! Disposable worker-owned exact PCM cache. Cache failures always rebuild from source.
use crate::{AudioData, AudioReservation, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

const MEMORY_BYTES: u64 = 128 * 1024 * 1024;
const DISK_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"DAWPCM01";
pub(crate) type Key = [u8; 32];

/// Includes dependency/build compatibility. Bump the domain when processing options change.
pub(crate) fn key(domain: &[u8], parts: &[&[u8]]) -> Key {
    let mut hash = Sha256::new();
    hash.update(b"daw-prepared-v1");
    hash.update(env!("DAW_MEDIA_BUILD").as_bytes());
    hash.update(include_bytes!("../../../Cargo.lock"));
    hash.update(std::env::consts::OS.as_bytes());
    hash.update(std::env::consts::ARCH.as_bytes());
    hash.update([u8::from(cfg!(debug_assertions))]);
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparationCacheStats {
    pub memory_hits: u64,
    pub disk_hits: u64,
    pub builds: u64,
}
#[derive(Default)]
struct Counters {
    memory_hits: AtomicU64,
    disk_hits: AtomicU64,
    builds: AtomicU64,
}

struct Entry {
    key: Key,
    audio: AudioData,
    bytes: u64,
}
#[derive(Default)]
struct State {
    entries: VecDeque<Entry>,
    bytes: u64,
    // Weak locks coordinate misses without extending audio or request lifetimes.
    flights: HashMap<Key, Weak<Mutex<()>>>,
}
/// All methods run on control workers. The callback never owns this cache.
#[derive(Default)]
pub struct PreparationCache {
    root: Option<PathBuf>,
    state: Mutex<State>,
    counters: Counters,
}
impl PreparationCache {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            state: Mutex::default(),
            counters: Counters::default(),
        }
    }
    fn path(&self, key: Key) -> Option<PathBuf> {
        self.root.as_ref().map(|root| {
            root.join(key.iter().map(|b| format!("{b:02x}")).collect::<String>())
                .with_extension("pcm")
        })
    }
    fn remember(&self, key: Key, audio: &AudioData) {
        let bytes = audio.samples.capacity() as u64 * 8 + audio.peaks.capacity() as u64 * 16;
        if bytes > MEMORY_BYTES {
            return;
        }
        crate::register_audio(audio);
        let mut state = self.state.lock().unwrap();
        while state.bytes + bytes > MEMORY_BYTES
            || crate::audio_memory_bytes() > crate::AUDIO_MEMORY_BUDGET_BYTES - MEMORY_BYTES
        {
            let Some(old) = state.entries.pop_front() else {
                break;
            };
            state.bytes -= old.bytes;
        }
        state.entries.push_back(Entry {
            key,
            audio: audio.clone(),
            bytes,
        });
        state.bytes += bytes;
    }
    pub(crate) fn get_or_prepare(
        &self,
        key: Key,
        cancelled: &impl Fn() -> bool,
        build: impl FnOnce() -> Result<AudioData>,
    ) -> Result<AudioData> {
        let flight = {
            let mut state = self.state.lock().unwrap();
            state.flights.retain(|_, lock| lock.strong_count() > 0);
            let lock = state
                .flights
                .get(&key)
                .and_then(Weak::upgrade)
                .unwrap_or_else(|| Arc::new(Mutex::new(())));
            state.flights.insert(key, Arc::downgrade(&lock));
            lock
        };
        let _guard = loop {
            check(cancelled)?;
            match flight.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
            }
        };
        {
            let mut state = self.state.lock().unwrap();
            if let Some(index) = state.entries.iter().position(|entry| entry.key == key) {
                let entry = state.entries.remove(index).unwrap();
                let audio = entry.audio.clone();
                state.entries.push_back(entry);
                self.counters.memory_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(audio);
            }
        }
        if let Some(path) = self.path(key)
            && let Ok(audio) = read(&path, key, cancelled)
        {
            check(cancelled)?;
            self.remember(key, &audio);
            self.counters.disk_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(audio);
        }
        check(cancelled)?;
        self.counters.builds.fetch_add(1, Ordering::Relaxed);
        let audio = build()?;
        check(cancelled)?;
        if let Some(path) = self.path(key) {
            // A cache is optional: permission, quota, or concurrent cleanup failures
            // cannot turn successful audio preparation into a project failure.
            let _ = write(&path, key, &audio, cancelled);
        }
        check(cancelled)?;
        self.remember(key, &audio);
        Ok(audio)
    }
    pub fn stats(&self) -> PreparationCacheStats {
        PreparationCacheStats {
            memory_hits: self.counters.memory_hits.load(Ordering::Relaxed),
            disk_hits: self.counters.disk_hits.load(Ordering::Relaxed),
            builds: self.counters.builds.load(Ordering::Relaxed),
        }
    }
    pub fn release_memory(&self) {
        let mut state = self.state.lock().unwrap();
        state.entries.clear();
        state.bytes = 0;
    }
}
static CACHE: OnceLock<PreparationCache> = OnceLock::new();
/// Call at application startup. None provides bounded memory caching without disk storage.
pub fn configure_preparation_cache(root: Option<PathBuf>) {
    let _ = CACHE.set(PreparationCache::new(root));
}
pub(crate) fn configured() -> bool {
    CACHE.get().is_some()
}
pub(crate) fn cached_audio(
    key: Key,
    cancelled: &impl Fn() -> bool,
    build: impl FnOnce() -> Result<AudioData>,
) -> Result<AudioData> {
    match CACHE.get() {
        Some(cache) => cache.get_or_prepare(key, cancelled, build),
        None => build(),
    }
}
pub fn release_preparation_cache_memory() {
    if let Some(cache) = CACHE.get() {
        cache.release_memory();
    }
}

pub fn preparation_cache_stats() -> PreparationCacheStats {
    CACHE
        .get()
        .map_or_else(PreparationCacheStats::default, PreparationCache::stats)
}
pub(crate) fn record_live_hit() {
    if let Some(cache) = CACHE.get() {
        cache.counters.memory_hits.fetch_add(1, Ordering::Relaxed);
    }
}

/// Shared admission for foreground transforms across editor, export and audition jobs.
/// Waiting is cancellable and occurs only on workers, never on note-on or the callback.
pub(crate) fn transform_slot<T>(
    cancelled: &impl Fn() -> bool,
    build: impl FnOnce() -> Result<T>,
) -> Result<T> {
    static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    struct Slot;
    impl Drop for Slot {
        fn drop(&mut self) {
            ACTIVE.fetch_sub(1, Ordering::Release);
        }
    }
    loop {
        check(cancelled)?;
        let active = ACTIVE.load(Ordering::Acquire);
        if active < 2
            && ACTIVE
                .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let _slot = Slot;
    build()
}

fn check(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err(Error("Audio preparation cancelled".into()))
    } else {
        Ok(())
    }
}
fn io(error: impl std::fmt::Display) -> Error {
    Error(error.to_string())
}
fn read(path: &Path, key: Key, cancelled: &impl Fn() -> bool) -> Result<AudioData> {
    let mut reader = BufReader::new(File::open(path).map_err(io)?);
    let length = reader.get_ref().metadata().map_err(io)?.len();
    let mut header = [0_u8; 72];
    reader.read_exact(&mut header).map_err(io)?;
    if &header[..8] != MAGIC || header[8..40] != key {
        return Err(io("Incompatible audio cache"));
    }
    let u32_at = |n| u32::from_le_bytes(header[n..n + 4].try_into().unwrap());
    let rate = u32_at(40);
    let frames = u64::from_le_bytes(header[44..52].try_into().unwrap());
    let source_rate = u32_at(52);
    let channels = u32_at(56);
    let bits = u32_at(60);
    let text_len = u32_at(64) as usize;
    if !daw_core::SAMPLE_RATES.contains(&rate)
        || source_rate == 0
        || !(1..=2).contains(&channels)
        || text_len > 64
        || header[68..] != [0; 4]
    {
        return Err(io("Invalid audio cache dimensions"));
    }
    let peaks = frames.div_ceil(256);
    let expected = frames
        .checked_mul(8)
        .and_then(|n| peaks.checked_mul(16).and_then(|p| n.checked_add(p)))
        .and_then(|n| n.checked_add(104 + text_len as u64))
        .ok_or_else(|| io("Audio cache size overflow"))?;
    if expected != length {
        return Err(io("Invalid audio cache size"));
    }
    let _reservation = AudioReservation::new(frames, 0)?;
    let mut format = vec![0; text_len];
    reader.read_exact(&mut format).map_err(io)?;
    let metadata = daw_core::SourceMetadata {
        sample_rate_hz: source_rate,
        channels: channels as u16,
        bits_per_sample: u16::try_from(bits).map_err(io)?,
        sample_format: String::from_utf8(format.clone()).map_err(io)?,
    };
    let mut hash = Sha256::new();
    hash.update(header);
    hash.update(&format);
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(usize::try_from(frames).map_err(io)?)
        .map_err(io)?;
    let mut waveform = Vec::new();
    waveform
        .try_reserve_exact(usize::try_from(peaks).map_err(io)?)
        .map_err(io)?;
    let mut buffer = [0; 32768];
    let mut remaining = frames;
    while remaining > 0 {
        check(cancelled)?;
        let count = remaining.min(4096) as usize;
        let bytes = &mut buffer[..count * 8];
        reader.read_exact(bytes).map_err(io)?;
        hash.update(&*bytes);
        for bytes in bytes.chunks_exact(8) {
            let sample = [
                f32::from_le_bytes(bytes[..4].try_into().unwrap()),
                f32::from_le_bytes(bytes[4..].try_into().unwrap()),
            ];
            if sample.iter().any(|v| !v.is_finite()) {
                return Err(io("Invalid audio cache samples"));
            }
            samples.push(sample);
        }
        remaining -= count as u64;
    }
    let mut remaining = peaks;
    while remaining > 0 {
        check(cancelled)?;
        let count = remaining.min(2048) as usize;
        let bytes = &mut buffer[..count * 16];
        reader.read_exact(bytes).map_err(io)?;
        hash.update(&*bytes);
        for bytes in bytes.chunks_exact(16) {
            let mut peak = [[0.0; 2]; 2];
            for (value, bytes) in peak.iter_mut().flatten().zip(bytes.chunks_exact(4)) {
                *value = f32::from_le_bytes(bytes.try_into().unwrap());
                if !value.is_finite() {
                    return Err(io("Invalid cached peaks"));
                }
            }
            waveform.push(peak);
        }
        remaining -= count as u64;
    }
    let mut digest = [0; 32];
    reader.read_exact(&mut digest).map_err(io)?;
    let actual: [u8; 32] = hash.finalize().into();
    if actual != digest {
        return Err(io("Audio cache checksum mismatch"));
    }
    check(cancelled)?;
    let audio = AudioData {
        sample_rate_hz: rate,
        samples: Arc::new(samples),
        peaks: Arc::new(waveform),
        metadata,
    };
    crate::register_audio(&audio);
    Ok(audio)
}
fn write(path: &Path, key: Key, audio: &AudioData, cancelled: &impl Fn() -> bool) -> Result<()> {
    check(cancelled)?;
    let root = path.parent().ok_or_else(|| io("Missing cache directory"))?;
    fs::create_dir_all(root).map_err(io)?;
    let mut temporary = None;
    for attempt in 0..128 {
        let candidate = root.join(format!(
            ".daw-pcm-{}-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io)?
                .as_nanos(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io(error)),
        }
    }
    let (temporary, file) = temporary.ok_or_else(|| io("Cache temporary collision"))?;
    let result = (|| {
        let mut writer = BufWriter::new(file);
        let mut header = [0; 72];
        header[..8].copy_from_slice(MAGIC);
        header[8..40].copy_from_slice(&key);
        header[40..44].copy_from_slice(&audio.sample_rate_hz.to_le_bytes());
        header[44..52].copy_from_slice(&(audio.samples.len() as u64).to_le_bytes());
        header[52..56].copy_from_slice(&audio.metadata.sample_rate_hz.to_le_bytes());
        header[56..60].copy_from_slice(&u32::from(audio.metadata.channels).to_le_bytes());
        header[60..64].copy_from_slice(&u32::from(audio.metadata.bits_per_sample).to_le_bytes());
        header[64..68].copy_from_slice(&(audio.metadata.sample_format.len() as u32).to_le_bytes());
        let mut hash = Sha256::new();
        let mut put = |bytes: &[u8]| -> Result<()> {
            hash.update(bytes);
            writer.write_all(bytes).map_err(io)
        };
        put(&header)?;
        put(audio.metadata.sample_format.as_bytes())?;
        let mut buffer = [0; 32768];
        for chunk in audio.samples.chunks(4096) {
            check(cancelled)?;
            for (value, bytes) in chunk.iter().flatten().zip(buffer.chunks_exact_mut(4)) {
                bytes.copy_from_slice(&value.to_le_bytes());
            }
            put(&buffer[..chunk.len() * 8])?;
        }
        for chunk in audio.peaks.chunks(2048) {
            check(cancelled)?;
            for (value, bytes) in chunk
                .iter()
                .flatten()
                .flatten()
                .zip(buffer.chunks_exact_mut(4))
            {
                bytes.copy_from_slice(&value.to_le_bytes());
            }
            put(&buffer[..chunk.len() * 16])?;
        }
        writer.write_all(&hash.finalize()).map_err(io)?;
        writer.flush().map_err(io)?;
        // Close the temporary before publication so Windows never replaces an open writer.
        drop(writer);
        check(cancelled)?;
        match fs::rename(&temporary, path) {
            Ok(()) => {}
            Err(_) if path.is_file() => {
                // Windows cannot replace an open destination. It is disposable;
                // a concurrent reader keeps its old handle or rebuilds on a miss.
                if fs::remove_file(path).is_ok() {
                    fs::rename(&temporary, path).map_err(io)?;
                }
            }
            Err(error) => return Err(io(error)),
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    if result.is_ok() {
        prune(root);
    }
    result
}
fn prune(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let metadata = entry.metadata().ok()?;
            if name.starts_with(".daw-pcm-")
                && name.ends_with(".tmp")
                && metadata.modified().ok()?.elapsed().ok()?.as_secs() > 86400
            {
                let _ = fs::remove_file(&path);
                return None;
            }
            if path.extension()?.to_str()? != "pcm"
                || path.file_stem()?.to_str()?.len() != 64
                || !path
                    .file_stem()?
                    .to_str()?
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            if !metadata.is_file() {
                return None;
            }
            Some((metadata.modified().ok()?, metadata.len(), entry.path()))
        })
        .collect();
    let mut total = files
        .iter()
        .fold(0_u64, |total, (_, bytes, _)| total.saturating_add(*bytes));
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, bytes, path) in files {
        if total <= DISK_BYTES {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
}

/// PCM identity is path independent and never used on the UI or callback thread.
pub(crate) fn audio_key(audio: &AudioData, cancelled: &impl Fn() -> bool) -> Result<Key> {
    type Fingerprint = (Weak<Vec<[f32; 2]>>, u32, daw_core::SourceMetadata, Key);
    static FINGERPRINTS: OnceLock<Mutex<Vec<Fingerprint>>> = OnceLock::new();
    let fingerprints = FINGERPRINTS.get_or_init(Default::default);
    check(cancelled)?;
    {
        let mut values = fingerprints.lock().unwrap();
        values.retain(|(owner, _, _, _)| owner.strong_count() > 0);
        if let Some((_, _, _, key)) = values.iter().find(|(owner, rate, metadata, _)| {
            owner.as_ptr() == Arc::as_ptr(&audio.samples)
                && *rate == audio.sample_rate_hz
                && *metadata == audio.metadata
        }) {
            return Ok(*key);
        }
    }
    let mut hash = Sha256::new();
    hash.update(audio.sample_rate_hz.to_le_bytes());
    hash.update(audio.metadata.sample_rate_hz.to_le_bytes());
    hash.update(audio.metadata.channels.to_le_bytes());
    hash.update(audio.metadata.bits_per_sample.to_le_bytes());
    hash.update(audio.metadata.sample_format.as_bytes());
    let mut buffer = [0; 32768];
    for chunk in audio.samples.chunks(4096) {
        check(cancelled)?;
        for (value, bytes) in chunk.iter().flatten().zip(buffer.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&value.to_le_bytes());
        }
        hash.update(&buffer[..chunk.len() * 8]);
    }
    let key = hash.finalize().into();
    let mut values = fingerprints.lock().unwrap();
    if values.len() >= 256 {
        values.remove(0);
    }
    values.push((
        Arc::downgrade(&audio.samples),
        audio.sample_rate_hz,
        audio.metadata.clone(),
        key,
    ));
    Ok(key)
}

/// Platform cache location selected by application startup, never a project source folder.
/// Empty DAW_AUDIO_CACHE disables disk caching; an explicit path supports headless deployments.
pub fn preparation_cache_directory() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("DAW_AUDIO_CACHE") {
        return (!value.is_empty()).then(|| PathBuf::from(value));
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));
    base.filter(|path| path.is_absolute())
        .map(|base| base.join("daw/prepared-audio-v1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "daw-pcm-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn audio() -> AudioData {
        let source = AudioData {
            sample_rate_hz: 48000,
            samples: Arc::new(vec![]),
            peaks: Arc::new(vec![]),
            metadata: daw_core::SourceMetadata {
                sample_rate_hz: 44100,
                channels: 1,
                bits_per_sample: 24,
                sample_format: "pcm_int".into(),
            },
        };
        crate::prepared_audio(
            &source,
            (0..1025)
                .map(|index| [index as f32 * 0.001, -0.25])
                .collect(),
        )
    }
    #[test]
    fn disk_round_trip_is_exact_after_all_memory_owners_are_released() {
        let dir = Directory::new();
        let cache = PreparationCache::new(Some(dir.0.clone()));
        let key = key(b"test", &[b"one"]);
        let original = audio();
        let first = cache
            .get_or_prepare(key, &|| false, || Ok(original.clone()))
            .unwrap();
        assert!(Arc::ptr_eq(&first.samples, &original.samples));
        let hit = cache
            .get_or_prepare(key, &|| false, || panic!("memory hit must not build"))
            .unwrap();
        assert!(Arc::ptr_eq(&hit.samples, &first.samples));
        cache.release_memory();
        drop(hit);
        drop(first);
        drop(cache);
        let reopened = PreparationCache::new(Some(dir.0.clone()));
        let disk = reopened
            .get_or_prepare(key, &|| false, || panic!("disk hit must not build"))
            .unwrap();
        assert_eq!(disk.metadata, original.metadata);
        assert_eq!(disk.samples, original.samples);
        assert_eq!(disk.peaks, original.peaks);
        assert!(!Arc::ptr_eq(&disk.samples, &original.samples));
    }
    #[test]
    fn corrupt_truncated_and_incompatible_entries_rebuild_without_partial_audio() {
        let dir = Directory::new();
        let cache = PreparationCache::new(Some(dir.0.clone()));
        let key = key(b"test", &[b"bad"]);
        let path = cache.path(key).unwrap();
        for kind in 0..3 {
            write(&path, key, &audio(), &|| false).unwrap();
            let mut bytes = fs::read(&path).unwrap();
            match kind {
                0 => bytes.truncate(80),
                1 => bytes[80] ^= 1,
                _ => bytes[8] ^= 1,
            }
            fs::write(&path, bytes).unwrap();
            let count = AtomicUsize::new(0);
            let prepared = cache
                .get_or_prepare(key, &|| false, || {
                    count.fetch_add(1, Ordering::Relaxed);
                    Ok(audio())
                })
                .unwrap();
            assert_eq!(count.load(Ordering::Relaxed), 1);
            assert_eq!(prepared.samples, audio().samples);
            cache.release_memory();
        }
    }
    #[test]
    fn cache_write_failure_and_cancellation_do_not_publish_partial_files() {
        let dir = Directory::new();
        let root = dir.0.join("file");
        fs::write(&root, b"occupied").unwrap();
        let cache = PreparationCache::new(Some(root));
        let key = key(b"test", &[]);
        assert_eq!(
            cache
                .get_or_prepare(key, &|| false, || Ok(audio()))
                .unwrap()
                .samples,
            audio().samples
        );
        let called = AtomicUsize::new(0);
        assert!(
            cache
                .get_or_prepare(key, &|| true, || {
                    called.fetch_add(1, Ordering::Relaxed);
                    Ok(audio())
                })
                .is_err()
        );
        assert_eq!(called.load(Ordering::Relaxed), 0);
        let path = dir.0.join("cancel.pcm");
        assert!(write(&path, key, &audio(), &|| true).is_err());
        assert!(!path.exists());
    }
    #[test]
    fn concurrent_identical_requests_compute_once_and_share_pcm() {
        let cache = PreparationCache::default();
        let calls = AtomicUsize::new(0);
        let key = key(b"concurrent", &[]);
        std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                cache
                    .get_or_prepare(key, &|| false, || {
                        calls.fetch_add(1, Ordering::Relaxed);
                        Ok(audio())
                    })
                    .unwrap()
            });
            let second = scope.spawn(|| {
                cache
                    .get_or_prepare(key, &|| false, || {
                        calls.fetch_add(1, Ordering::Relaxed);
                        Ok(audio())
                    })
                    .unwrap()
            });
            assert!(Arc::ptr_eq(
                &first.join().unwrap().samples,
                &second.join().unwrap().samples
            ));
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn keys_distinguish_content_rate_and_processing_settings() {
        let first = audio();
        let content = audio_key(&first, &|| false).unwrap();
        let mut second = first.clone();
        second.sample_rate_hz = 96000;
        assert_ne!(content, audio_key(&second, &|| false).unwrap());
        second = first.clone();
        Arc::make_mut(&mut second.samples)[0][0] = 0.125;
        assert_ne!(content, audio_key(&second, &|| false).unwrap());
        assert_ne!(
            key(b"transform", &[&content, b"pitch1"]),
            key(b"transform", &[&content, b"pitch2"])
        );
    }
}
