//! Worker-produced audition chunks. Every PCM owner leaves the callback through a queue.
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct AuditionStreamWriter {
    frames: Arc<AtomicUsize>,
    finished: Arc<AtomicBool>,
    incoming: Producer<Vec<[f32; 2]>>,
    retired: Consumer<Vec<[f32; 2]>>,
}
pub struct AuditionStream {
    frames: Arc<AtomicUsize>,
    finished: Arc<AtomicBool>,
    incoming: Consumer<Vec<[f32; 2]>>,
    retired: Producer<Vec<[f32; 2]>>,
    active: Option<Vec<[f32; 2]>>,
    index: usize,
    ended: bool,
}
pub fn audition_stream() -> (AuditionStreamWriter, AuditionStream) {
    let frames = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let (incoming, receive) = RingBuffer::new(4);
    let (retire, retired) = RingBuffer::new(8);
    (
        AuditionStreamWriter {
            frames: frames.clone(),
            finished: finished.clone(),
            incoming,
            retired,
        },
        AuditionStream {
            frames,
            finished,
            incoming: receive,
            retired: retire,
            active: None,
            index: 0,
            ended: false,
        },
    )
}
impl AuditionStreamWriter {
    pub fn chunk_frames(&self) -> usize {
        self.frames.load(Ordering::Acquire).max(1)
    }
    /// Wait only on the rendering worker, before producing the first device-sized batch.
    pub fn wait_for_start(&self, cancelled: &impl Fn() -> bool) -> Option<usize> {
        loop {
            if cancelled() || self.incoming.is_abandoned() {
                return None;
            }
            let frames = self.frames.load(Ordering::Acquire);
            if frames != 0 {
                return Some(frames);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    /// Empty audio marks completion. Waiting and retirement happen only on the worker.
    pub fn send(&mut self, mut samples: Vec<[f32; 2]>, cancelled: &impl Fn() -> bool) -> bool {
        loop {
            while self.retired.pop().is_ok() {}
            if cancelled() || self.incoming.is_abandoned() {
                return false;
            }
            match self.incoming.push(samples) {
                Ok(()) => return true,
                Err(rtrb::PushError::Full(buffer)) => samples = buffer,
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
impl AuditionStream {
    pub(super) fn start(&self, frames: usize) {
        self.frames.store(frames, Ordering::Release);
    }
    pub(super) fn next(&mut self) -> Option<[f32; 2]> {
        if self.ended {
            return None;
        }
        if self
            .active
            .as_ref()
            .is_none_or(|audio| self.index >= audio.len())
        {
            if self.active.is_some() {
                if self.retired.is_full() {
                    return Some([0.0; 2]);
                }
                let _ = self.retired.push(self.active.take().unwrap());
            }
            let finished = self.finished.load(Ordering::Acquire);
            match self.incoming.pop() {
                Ok(audio) => {
                    self.ended = audio.is_empty();
                    self.active = Some(audio);
                    self.index = 0;
                }
                Err(_) => {
                    if finished {
                        self.ended = true;
                        return None;
                    }
                    return Some([0.0; 2]);
                }
            }
        }
        if self.ended {
            return None;
        }
        let sample = self.active.as_ref().unwrap()[self.index];
        self.index += 1;
        Some(sample)
    }
}

impl Drop for AuditionStreamWriter {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::Release);
    }
}

pub(super) fn source_chunk_frames(
    project_rate: u32,
    device_rate: u32,
    device_frames: usize,
) -> Option<usize> {
    if project_rate == 0 || device_rate == 0 || device_frames == 0 {
        return None;
    }
    let frames =
        (device_frames as u128 * u128::from(project_rate)).div_ceil(u128::from(device_rate));
    if frames > daw_engine::NOTE_AUDITION_MAX_CHUNK_FRAMES as u128 {
        return None;
    }
    Some(frames as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_batches_cover_project_rate_conversion_without_unbounded_memory() {
        for project in [44100, 48000, 88200, 96000, 176400, 192000] {
            for device in [8000, 44100, 48000, 96000, 192000] {
                for block in [32, 512, 4096] {
                    let frames = source_chunk_frames(project, device, block).unwrap() as u64;
                    let target = block as u64 * u64::from(project);
                    assert!(frames * u64::from(device) >= target);
                    assert!((frames - 1) * u64::from(device) < target);
                }
            }
        }
        assert!(source_chunk_frames(48000, 0, 512).is_none());
        assert!(source_chunk_frames(u32::MAX, 1, usize::MAX).is_none());
        let (writer, stream) = audition_stream();
        stream.start(2048);
        assert_eq!(writer.wait_for_start(&|| false), Some(2048));
        drop(stream);
        assert_eq!(writer.wait_for_start(&|| false), None);
    }
}
