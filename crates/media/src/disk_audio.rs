//! Immutable disk PCM with bounded, nonblocking read-ahead publication.
use crate::{AudioData, AudioReservation, Error, Result};
use daw_core::SourceMetadata;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

pub const STREAM_CHUNK_FRAMES: usize = 32_768;
const EMPTY: u64 = u64::MAX;
type Frame = [f32; 2];

struct PcmFile {
    file: File,
}
fn temporary_file() -> Result<PcmFile> {
    let path = std::env::temp_dir().join(format!("daw-pcm-{}", daw_core::Id::new_v4()));
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0400_0000 | 0x0000_0100);
    }
    let file = options
        .open(&path)
        .map_err(|e| Error(format!("Cannot create disk audio: {e}")))?;
    #[cfg(unix)]
    std::fs::remove_file(&path)
        .map_err(|e| Error(format!("Cannot unlink private disk audio: {e}")))?;
    Ok(PcmFile { file })
}

pub struct DiskAudio {
    file: PcmFile,
    frames: u64,
    hash: [u8; 32],
    preview: OnceLock<Arc<ReadAhead>>,
    error: Mutex<Option<String>>,
    failed: Arc<AtomicBool>,
}
impl DiskAudio {
    pub fn frame_count(&self) -> u64 {
        self.frames
    }
    pub fn content_hash(&self) -> &[u8; 32] {
        &self.hash
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
    /// Worker-only read. The file is private and immutable after publication.
    pub fn read(&self, start: u64, out: &mut [Frame], raw: &mut Vec<u8>) -> Result<usize> {
        let count = (self.frames.saturating_sub(start)).min(out.len() as u64) as usize;
        if count == 0 {
            raw.clear();
            return Ok(0);
        }
        let offset = start
            .checked_mul(8)
            .ok_or_else(|| Error("Disk audio offset overflow".into()))?;
        raw.resize(count * 8, 0);
        let mut done = 0;
        while done < raw.len() {
            #[cfg(unix)]
            let result = {
                use std::os::unix::fs::FileExt;
                self.file
                    .file
                    .read_at(&mut raw[done..], offset + done as u64)
            };
            #[cfg(windows)]
            let result = {
                use std::os::windows::fs::FileExt;
                self.file
                    .file
                    .seek_read(&mut raw[done..], offset + done as u64)
            };
            let read = result.map_err(|e| Error(format!("Cannot read disk audio: {e}")))?;
            if read == 0 {
                return Err(Error("Disk audio ended before its prepared length".into()));
            }
            done += read;
        }
        for (frame, bytes) in out.iter_mut().zip(raw.chunks_exact(8)) {
            *frame = [
                f32::from_le_bytes(bytes[..4].try_into().unwrap()),
                f32::from_le_bytes(bytes[4..].try_into().unwrap()),
            ];
        }
        Ok(count)
    }
    pub fn read_ahead(self: &Arc<Self>) -> Result<Arc<ReadAhead>> {
        ReadAhead::new(self, 8)
    }
    /// UI-only, nonblocking detail request; no file reads occur in painting.
    pub fn preview(self: &Arc<Self>, frame: u64) -> Option<Frame> {
        let cache = self.preview.get()?;
        let sample = cache.sample(frame);
        if sample.is_none() {
            cache.request(frame, None);
        }
        sample
    }
    fn record_error(&self, error: Error) {
        *self.error.lock().unwrap() = Some(error.to_string());
        self.failed.store(true, Ordering::Release);
    }
}

/// PCM payload uses atomic words: a failed generation check never creates a Rust
/// data race. Sequential consistency orders every payload read between its two
/// generation reads. Readers try once and never spin or wait for the writer.
struct Slot {
    generation: AtomicU64,
    start: AtomicU64,
    length: AtomicUsize,
    samples: Box<[AtomicU32]>,
}
impl Slot {
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            start: AtomicU64::new(EMPTY),
            length: AtomicUsize::new(0),
            samples: (0..STREAM_CHUNK_FRAMES * 2)
                .map(|_| AtomicU32::new(0))
                .collect(),
        }
    }
    fn sample(&self, frame: u64) -> Option<Frame> {
        let generation = self.generation.load(Ordering::SeqCst);
        if !generation.is_multiple_of(2) {
            return None;
        }
        let start = self.start.load(Ordering::SeqCst);
        let offset = frame.checked_sub(start)? as usize;
        if offset >= self.length.load(Ordering::SeqCst) {
            return None;
        }
        let value = std::array::from_fn(|ch| {
            f32::from_bits(self.samples[offset * 2 + ch].load(Ordering::SeqCst))
        });
        (generation == self.generation.load(Ordering::SeqCst)).then_some(value)
    }
    fn publish(&self, start: u64, frames: &[Frame]) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.start.store(start, Ordering::SeqCst);
        self.length.store(frames.len(), Ordering::SeqCst);
        for (sample, value) in self.samples.iter().zip(frames.iter().flatten()) {
            sample.store(value.to_bits(), Ordering::SeqCst);
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

pub struct ReadAhead {
    source: Weak<DiskAudio>,
    frames: u64,
    failed: Arc<AtomicBool>,
    slots: Vec<Slot>,
    wanted: AtomicU64,
    pinned: AtomicU64,
    _memory: AudioReservation,
}
impl ReadAhead {
    fn new(source: &Arc<DiskAudio>, slots: usize) -> Result<Arc<Self>> {
        let memory = AudioReservation::new((slots * STREAM_CHUNK_FRAMES) as u64, 0)?;
        let cache = Arc::new(Self {
            source: Arc::downgrade(source),
            frames: source.frames,
            failed: source.failed.clone(),
            slots: (0..slots).map(|_| Slot::new()).collect(),
            wanted: AtomicU64::new(0),
            pinned: AtomicU64::new(EMPTY),
            _memory: memory,
        });
        worker()?
            .send(Arc::downgrade(&cache))
            .map_err(|_| Error("Disk audio worker stopped".into()))?;
        Ok(cache)
    }
    pub fn request(&self, frame: u64, pinned: Option<u64>) {
        self.wanted.store(frame, Ordering::Relaxed);
        self.pinned
            .store(pinned.unwrap_or(EMPTY), Ordering::Relaxed);
    }
    pub fn sample(&self, frame: u64) -> Option<Frame> {
        self.slots.iter().find_map(|slot| slot.sample(frame))
    }
    pub fn ready(&self, frame: u64) -> bool {
        frame >= self.frames || self.sample(frame).is_some()
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
    fn fill(&self, frames: &mut [Frame], raw: &mut Vec<u8>) {
        let Some(source) = self.source.upgrade() else {
            return;
        };
        if source.failed() {
            return;
        }
        let align = |n: u64| n / STREAM_CHUNK_FRAMES as u64 * STREAM_CHUNK_FRAMES as u64;
        let wanted = align(self.wanted.load(Ordering::Relaxed));
        let pinned = self.pinned.load(Ordering::Relaxed);
        let targets = [
            wanted,
            wanted.saturating_add(STREAM_CHUNK_FRAMES as u64),
            wanted.saturating_add(2 * STREAM_CHUNK_FRAMES as u64),
            if pinned == EMPTY {
                EMPTY
            } else {
                align(pinned)
            },
        ];
        for target in targets {
            if target >= source.frames || self.ready(target) {
                continue;
            }
            let Some(slot) = self.slots.iter().find(|slot| {
                let start = slot.start.load(Ordering::Relaxed);
                start == EMPTY || !targets.contains(&start)
            }) else {
                continue;
            };
            match source.read(target, frames, raw) {
                Ok(count) => slot.publish(target, &frames[..count]),
                Err(error) => {
                    source.record_error(error);
                    return;
                }
            }
        }
    }
}
fn worker() -> Result<&'static std::sync::mpsc::Sender<Weak<ReadAhead>>> {
    static WORKER: OnceLock<std::result::Result<std::sync::mpsc::Sender<Weak<ReadAhead>>, String>> =
        OnceLock::new();
    WORKER
        .get_or_init(|| {
            let memory =
                AudioReservation::new(STREAM_CHUNK_FRAMES as u64, STREAM_CHUNK_FRAMES as u64 * 8)
                    .map_err(|error| error.to_string())?;
            let (sender, receiver) = std::sync::mpsc::channel::<Weak<ReadAhead>>();
            std::thread::Builder::new()
                .name("disk-audio-read-ahead".into())
                .spawn(move || {
                    let _memory = memory;
                    let mut clients = Vec::new();
                    let mut frames = vec![[0.; 2]; STREAM_CHUNK_FRAMES];
                    let mut raw = Vec::with_capacity(STREAM_CHUNK_FRAMES * 8);
                    loop {
                        let next = if clients.is_empty() {
                            receiver.recv().ok()
                        } else {
                            receiver.recv_timeout(Duration::from_millis(2)).ok()
                        };
                        if let Some(client) = next {
                            clients.push(client);
                        }
                        clients.extend(receiver.try_iter());
                        clients.retain(|client: &Weak<ReadAhead>| {
                            let Some(client) = client.upgrade() else {
                                return false;
                            };
                            client.fill(&mut frames, &mut raw);
                            true
                        });
                    }
                })
                .map_err(|e| format!("Cannot start disk audio worker: {e}"))?;
            Ok(sender)
        })
        .as_ref()
        .map_err(|e| Error(e.clone()))
}

/// Bounded worker reader. Clones share immutable blocks until the next worker read.
#[derive(Clone)]
pub struct FrameReader {
    audio: AudioData,
    block: Option<Arc<FrameBlock>>,
    live: Option<Arc<ReadAhead>>,
    error: Option<Arc<String>>,
}
struct FrameBlock {
    start: u64,
    frames: Vec<Frame>,
    _memory: AudioReservation,
}
impl FrameReader {
    pub fn new(audio: &AudioData) -> Self {
        Self {
            audio: audio.clone(),
            block: None,
            live: None,
            error: None,
        }
    }
    /// Owned descriptor storage. PCM and read-ahead use shared media admission.
    pub fn heap_bytes(&self) -> u64 {
        self.audio.metadata.sample_format.capacity() as u64
            + self
                .error
                .as_ref()
                .map_or(0, |error| error.capacity() as u64)
    }
    /// Worker/control only. Each renderer owns its own read-ahead requests.
    pub fn prepare_live(&mut self) -> Result<()> {
        if let Some(source) = &self.audio.disk {
            self.live = Some(source.read_ahead()?);
        }
        Ok(())
    }
    pub fn ready(&self, frame: u64, pinned: Option<u64>) -> bool {
        if self.audio.disk.is_none() || frame >= self.audio.frame_count() {
            return true;
        }
        let Some(cache) = &self.live else {
            return false;
        };
        cache.request(frame, pinned);
        !cache.failed() && cache.ready(frame)
    }
    /// Callback-safe. Call ready before advancing any renderer state.
    pub fn try_live_sample(&self, frame: u64) -> Option<Frame> {
        if frame >= self.audio.frame_count() {
            return Some([0.; 2]);
        }
        if self.audio.disk.is_some() {
            self.live.as_ref()?.sample(frame)
        } else {
            self.audio.samples.get(frame as usize).copied()
        }
    }
    pub fn live_sample(&self, frame: u64) -> Frame {
        if self.audio.disk.is_some() {
            self.live
                .as_ref()
                .and_then(|cache| cache.sample(frame))
                .unwrap_or_default()
        } else {
            self.audio
                .samples
                .get(frame as usize)
                .copied()
                .unwrap_or_default()
        }
    }
    /// Worker only. Cache blocks, including reservation ownership, survive renderer clones.
    pub fn error(&self) -> Option<String> {
        self.error
            .as_ref()
            .map(|error| error.as_ref().clone())
            .or_else(|| self.audio.disk.as_ref().and_then(|disk| disk.error()))
    }
    pub fn failed(&self) -> bool {
        self.audio.disk.as_ref().is_some_and(|disk| disk.failed())
    }
    pub fn sample(&mut self, frame: u64) -> Result<Frame> {
        let result = self.sample_inner(frame);
        if let Err(error) = &result {
            self.error = Some(Arc::new(error.to_string()));
        }
        result
    }
    fn sample_inner(&mut self, frame: u64) -> Result<Frame> {
        let Some(source) = &self.audio.disk else {
            return Ok(self
                .audio
                .samples
                .get(frame as usize)
                .copied()
                .unwrap_or_default());
        };
        if frame >= source.frames {
            return Ok([0.; 2]);
        }
        if let Some(block) = &self.block
            && let Some(index) = frame.checked_sub(block.start)
            && let Some(value) = block.frames.get(index as usize)
        {
            return Ok(*value);
        }
        let memory =
            AudioReservation::new(STREAM_CHUNK_FRAMES as u64, STREAM_CHUNK_FRAMES as u64 * 8)?;
        let start = frame / STREAM_CHUNK_FRAMES as u64 * STREAM_CHUNK_FRAMES as u64;
        let mut frames = vec![[0.; 2]; STREAM_CHUNK_FRAMES];
        let count = match source.read(start, &mut frames, &mut Vec::new()) {
            Ok(count) => count,
            Err(error) => {
                source.record_error(Error(error.to_string()));
                return Err(error);
            }
        };
        frames.truncate(count);
        let value = frames[(frame - start) as usize];
        self.block = Some(Arc::new(FrameBlock {
            start,
            frames,
            _memory: memory,
        }));
        Ok(value)
    }
}

pub struct DiskAudioWriter {
    file: PcmFile,
    bytes: Vec<u8>,
    frames: u64,
    hash: Sha256,
    peaks: Vec<[[f32; 2]; 2]>,
    peak: [[f32; 2]; 2],
    peak_frames: usize,
    expected_frames: Option<u64>,
    peak_memory: Vec<AudioReservation>,
    _memory: AudioReservation,
}
impl DiskAudioWriter {
    pub fn new(expected_frames: u64) -> Result<Self> {
        let peak_count = expected_frames.div_ceil(256);
        let scratch = peak_count
            .checked_mul(16)
            .and_then(|n| n.checked_add(8192 * 8))
            .ok_or_else(|| Error("Disk audio peaks exceed supported bounds".into()))?;
        let memory = AudioReservation::new(0, scratch)?;
        let mut peaks = Vec::new();
        peaks
            .try_reserve_exact(
                usize::try_from(peak_count)
                    .map_err(|_| Error("Disk audio peaks exceed supported bounds".into()))?,
            )
            .map_err(|e| Error(format!("Cannot prepare disk audio peaks: {e}")))?;
        Ok(Self {
            file: temporary_file()?,
            bytes: Vec::with_capacity(8192 * 8),
            frames: 0,
            hash: Sha256::new(),
            peaks,
            expected_frames: Some(expected_frames),
            peak_memory: Vec::new(),
            _memory: memory,
            peak: [[f32::INFINITY, f32::NEG_INFINITY]; 2],
            peak_frames: 0,
        })
    }
    /// Packet decoders have no trusted output length. Grow only bounded peak storage.
    pub(crate) fn incremental() -> Result<Self> {
        let mut writer = Self::new(0)?;
        writer.expected_frames = None;
        Ok(writer)
    }
    fn push_peak(&mut self) -> Result<()> {
        if self.peaks.len() == self.peaks.capacity() {
            let memory = AudioReservation::new(0, 4096 * 16)?;
            let _growth = AudioReservation::new(0, self.peaks.capacity() as u64 * 16)?;
            self.peaks
                .try_reserve_exact(4096)
                .map_err(|e| Error(format!("Cannot prepare disk audio peaks: {e}")))?;
            self.peak_memory.push(memory);
        }
        self.peaks.push(self.peak);
        Ok(())
    }
    pub fn write(&mut self, frames: &[Frame]) -> Result<()> {
        if self.frames.checked_add(frames.len() as u64).is_none() {
            return Err(Error("Disk audio length overflow".into()));
        }
        if self
            .expected_frames
            .is_some_and(|expected| frames.len() as u64 > expected.saturating_sub(self.frames))
        {
            return Err(Error("Disk audio exceeds its declared length".into()));
        }
        for block in frames.chunks(8192) {
            self.bytes.clear();
            for frame in block {
                if frame.iter().any(|s| !s.is_finite()) {
                    return Err(Error("Disk audio contains non-finite samples".into()));
                }
                for (ch, sample) in frame.iter().enumerate() {
                    self.bytes.extend_from_slice(&sample.to_le_bytes());
                    self.peak[ch][0] = self.peak[ch][0].min(*sample);
                    self.peak[ch][1] = self.peak[ch][1].max(*sample);
                }
                self.peak_frames += 1;
                if self.peak_frames == 256 {
                    self.push_peak()?;
                    self.peak = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
                    self.peak_frames = 0;
                }
            }
            self.hash.update(&self.bytes);
            (&self.file.file)
                .write_all(&self.bytes)
                .map_err(|e| Error(format!("Cannot write disk audio: {e}")))?;
            self.frames += block.len() as u64;
        }
        Ok(())
    }
    pub fn finish(mut self, sample_rate_hz: u32, metadata: SourceMetadata) -> Result<AudioData> {
        if self.frames == 0 {
            return Err(Error("Source contains no audio samples".into()));
        }
        if self
            .expected_frames
            .is_some_and(|expected| self.frames != expected)
        {
            return Err(Error("Disk audio ended before its declared length".into()));
        }
        if self.peak_frames != 0 {
            self.push_peak()?;
        }
        let disk = Arc::new(DiskAudio {
            file: self.file,
            frames: self.frames,
            hash: self.hash.finalize().into(),
            preview: OnceLock::new(),
            error: Mutex::new(None),
            failed: Arc::new(AtomicBool::new(false)),
        });
        let preview = ReadAhead::new(&disk, 4)?;
        let _ = disk.preview.set(preview);
        let audio = AudioData {
            sample_rate_hz,
            metadata,
            samples: Arc::new(Vec::new()),
            peaks: Arc::new(self.peaks),
            disk: Some(disk),
        };
        crate::register_audio(&audio);
        Ok(audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> SourceMetadata {
        SourceMetadata {
            sample_rate_hz: 48_000,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        }
    }
    #[test]
    fn private_pcm_preserves_frames_peaks_and_nonblocking_read_ahead() {
        let count = STREAM_CHUNK_FRAMES * 4 + 17;
        let frames: Vec<_> = (0..count)
            .map(|i| [(i as f32 * 0.01).sin(), (i as f32 * 0.013).cos()])
            .collect();
        let mut writer = DiskAudioWriter::new(count as u64).unwrap();
        for block in frames.chunks(719) {
            writer.write(block).unwrap();
        }
        let audio = writer.finish(48_000, metadata()).unwrap();
        assert!(audio.samples.is_empty());
        assert_eq!(audio.frame_count(), count as u64);
        assert_eq!(
            audio.peaks.as_slice(),
            crate::prepare_peaks(&frames, &|| false).unwrap()
        );
        let mut block = [[0.; 2]; 128];
        let mut raw = Vec::new();
        assert_eq!(
            audio
                .read_frames((count - 17) as u64, &mut block, &mut raw)
                .unwrap(),
            17
        );
        assert_eq!(&block[..17], &frames[count - 17..]);
        let cache = audio.disk.as_ref().unwrap().read_ahead().unwrap();
        let position = STREAM_CHUNK_FRAMES * 3 + 7;
        cache.request(position as u64, None);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !cache.ready(position as u64) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(cache.sample(position as u64), Some(frames[position]));
        cache.request(position as u64, Some(19));
        let end = std::time::Instant::now() + Duration::from_secs(5);
        while !cache.ready(position as u64) || !cache.ready(19) {
            assert!(std::time::Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(cache.sample(position as u64), Some(frames[position]));
        assert_eq!(cache.sample(19), Some(frames[19]));
        assert_eq!(
            audio.materialized(&|| false).unwrap().samples.as_slice(),
            frames
        );
        assert!(audio.materialized(&|| true).is_err());
        assert!(!cache.failed());
    }
    #[test]
    fn truncated_disk_storage_reports_failure_without_a_partial_read() {
        let count = STREAM_CHUNK_FRAMES as u64 * 5;
        let mut writer = DiskAudioWriter::new(count).unwrap();
        for _ in 0..5 {
            writer
                .write(&vec![[0.25, -0.25]; STREAM_CHUNK_FRAMES])
                .unwrap();
        }
        let audio = writer.finish(48_000, metadata()).unwrap();
        let disk = audio.disk.as_ref().unwrap();
        disk.file.file.set_len(8).unwrap();
        let mut reader = FrameReader::new(&audio);
        assert!(reader.sample(count - 1).is_err());
        assert!(disk.failed());
        assert!(disk.error().unwrap().contains("ended before"));
    }
    #[test]
    fn incremental_writer_preserves_pcm_without_trusting_a_declared_length() {
        let frames = vec![[0.125, -0.25]; 4096 * 256 + 17];
        let mut writer = DiskAudioWriter::incremental().unwrap();
        for block in frames.chunks(1152) {
            writer.write(block).unwrap();
        }
        let audio = writer.finish(44_100, metadata()).unwrap();
        assert_eq!(audio.frame_count(), frames.len() as u64);
        assert_eq!(
            audio.materialized(&|| false).unwrap().samples.as_slice(),
            frames
        );
        assert_eq!(
            audio.peaks.as_slice(),
            crate::prepare_peaks(&frames, &|| false).unwrap()
        );
    }

    #[test]
    fn disk_pcm_rejects_invalid_lengths_and_samples() {
        let mut writer = DiskAudioWriter::new(1).unwrap();
        assert!(writer.write(&[[f32::NAN, 0.]]).is_err());
        let mut writer = DiskAudioWriter::new(1).unwrap();
        assert!(writer.write(&[[0.; 2]; 2]).is_err());
        let writer = DiskAudioWriter::new(1).unwrap();
        assert!(writer.finish(48_000, metadata()).is_err());
    }
}
