//! Bounded preview queue. Buffer ownership returns to the control thread.
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
struct Buffer {
    generation: u64,
    samples: Vec<[f32; 2]>,
    stream: Option<super::AuditionStream>,
}
pub(super) struct Sender {
    playing: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    incoming: Producer<Box<Buffer>>,
    retired: Consumer<Box<Buffer>>,
}
pub(super) struct Player {
    playing: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    incoming: Consumer<Box<Buffer>>,
    retired: Producer<Box<Buffer>>,
    active: Option<Box<Buffer>>,
    chunk_frames: usize,
    index: usize,
    gain: f32,
    step: f32,
}
pub(super) fn channel(rate: u32) -> (Sender, Player) {
    let generation = Arc::new(AtomicU64::new(0));
    let playing = Arc::new(AtomicU64::new(0));
    let (incoming, receive) = RingBuffer::new(2);
    let (retire, retired) = RingBuffer::new(8);
    (
        Sender {
            playing: playing.clone(),
            generation: generation.clone(),
            incoming,
            retired,
        },
        Player {
            playing,
            generation,
            incoming: receive,
            retired: retire,
            active: None,
            chunk_frames: (rate as usize / 100).max(1),
            index: 0,
            gain: 0.0,
            step: 1.0 / (rate / 200).max(1) as f32,
        },
    )
}
impl Sender {
    pub fn drain(&mut self) {
        while self.retired.pop().is_ok() {}
    }
    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Acquire) != 0
    }
    pub fn stop(&self) {
        self.playing.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }
    pub fn play(&mut self, samples: Vec<[f32; 2]>) -> Result<(), String> {
        self.enqueue(samples, None)
    }
    pub fn play_stream(&mut self, stream: super::AuditionStream) -> Result<(), String> {
        self.enqueue(Vec::new(), Some(stream))
    }
    fn enqueue(
        &mut self,
        samples: Vec<[f32; 2]>,
        stream: Option<super::AuditionStream>,
    ) -> Result<(), String> {
        self.drain();
        self.stop();
        let generation = self.generation.load(Ordering::Acquire);
        self.playing.store(generation, Ordering::Release);
        let result = self
            .incoming
            .push(Box::new(Buffer {
                generation,
                samples,
                stream,
            }))
            .map_err(|_| "Chord audition queue is full".to_owned());
        if result.is_err() {
            self.playing.store(0, Ordering::Release);
        }
        result
    }
}
impl Player {
    pub fn set_stream_chunk_frames(&mut self, frames: usize) {
        self.chunk_frames = frames;
        if let Some(stream) = self
            .active
            .as_ref()
            .and_then(|buffer| buffer.stream.as_ref())
        {
            stream.start(frames);
        }
    }
    pub fn update(&mut self) {
        // Never pop owners unless their retirement slot is available.
        if self.active.is_some() && self.gain == 0.0 && !self.retired.is_full() {
            let buffer = self.active.take().unwrap();
            let _ = self.playing.compare_exchange(
                buffer.generation,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            let _ = self.retired.push(buffer);
        }
        if self.active.is_some() {
            return;
        }
        for _ in 0..2 {
            if self.retired.is_full() {
                break;
            }
            let Ok(buffer) = self.incoming.pop() else {
                break;
            };
            if buffer.generation != self.generation.load(Ordering::Acquire) {
                let _ = self.retired.push(buffer);
            } else {
                if let Some(stream) = &buffer.stream {
                    stream.start(self.chunk_frames);
                }
                self.active = Some(buffer);
                self.index = 0;
                self.gain = self.step;
                break;
            }
        }
    }
    pub fn next(&mut self) -> [f32; 2] {
        let Some(buffer) = &mut self.active else {
            return [0.0; 2];
        };
        let stream_sample = buffer.stream.as_mut().and_then(|stream| stream.next());
        let current = buffer.generation == self.generation.load(Ordering::Acquire)
            && if buffer.stream.is_some() {
                stream_sample.is_some()
            } else {
                self.index < buffer.samples.len()
            };
        self.gain = if current {
            (self.gain + self.step).min(1.0)
        } else {
            (self.gain - self.step).max(0.0)
        };
        // Hold the last sample only during cancellation's five-millisecond fade.
        let sample = stream_sample.unwrap_or_else(|| {
            buffer
                .samples
                .get(self.index.min(buffer.samples.len().saturating_sub(1)))
                .copied()
                .unwrap_or([0.0; 2])
        });
        self.index = self.index.saturating_add(1);
        [sample[0] * self.gain, sample[1] * self.gain]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preparation_tests::realtime;
    #[test]
    fn streaming_chunks_keep_gain_and_retire_without_callback_allocation() {
        let (mut sender, mut player) = channel(48_000);
        let (mut writer, stream) = crate::audition_stream();
        for _ in 0..4 {
            assert!(writer.send(vec![[0.5, -0.25]; 256], &|| false));
        }
        sender.play_stream(stream).unwrap();
        realtime(|| {
            player.update();
            for _ in 0..500 {
                player.next();
            }
            // Crossing chunk boundaries must not restart the gain ramp.
            for _ in 0..500 {
                assert_eq!(player.next(), [0.5, -0.25]);
            }
        });
        assert!(writer.send(vec![[0.5, -0.25]; 256], &|| false));
        drop(writer);
        realtime(|| {
            for _ in 0..600 {
                player.next();
            }
            player.update();
            assert_eq!(player.next(), [0.0; 2]);
        });
        assert!(!sender.playing());
        sender.drain();
        let (mut writer, stream) = crate::audition_stream();
        for _ in 0..4 {
            assert!(writer.send(vec![[0.5; 2]; 256], &|| false));
        }
        sender.play_stream(stream).unwrap();
        realtime(|| {
            player.update();
            for _ in 0..500 {
                player.next();
            }
        });
        sender.stop();
        realtime(|| {
            for _ in 0..500 {
                player.next();
            }
            player.update();
            assert_eq!(player.next(), [0.0; 2]);
        });
        sender.drain();
        drop(writer);
        let (mut writer, stream) = crate::audition_stream();
        drop(stream);
        assert!(!writer.send(vec![[0.5; 2]; 32], &|| false));
    }

    #[test]
    fn replacement_cancellation_and_retirement_never_allocate_on_callback() {
        let (mut sender, mut player) = channel(48_000);
        sender.play(vec![[0.5, -0.25]; 4800]).unwrap();
        realtime(|| {
            player.update();
            for _ in 0..500 {
                player.next();
            }
            assert_eq!(player.next(), [0.5, -0.25]);
        });
        sender.play(vec![[-0.25, 0.5]; 4800]).unwrap();
        realtime(|| {
            player.update();
            for _ in 0..500 {
                player.next();
            }
            assert_eq!(player.next(), [0.0, -0.0]);
            player.update();
            for _ in 0..500 {
                player.next();
            }
            assert_eq!(player.next(), [-0.25, 0.5]);
        });
        sender.stop();
        assert!(!sender.playing());
        realtime(|| {
            for _ in 0..500 {
                player.next();
            }
            player.update();
            assert_eq!(player.next(), [0.0; 2]);
        });
        sender.drain();
        assert!(!sender.playing());
        assert!(player.active.is_none());
    }
    #[test]
    fn stale_previews_queue_full_and_natural_end_are_bounded() {
        let (mut sender, mut player) = channel(44_100);
        sender.play(vec![[1.0; 2]; 1000]).unwrap();
        sender.play(vec![[0.5; 2]; 1000]).unwrap();
        assert!(sender.play(vec![[0.25; 2]; 1000]).is_err());
        realtime(|| {
            player.update();
            assert!(player.active.is_none());
        });
        sender.play(vec![[0.25; 2]; 1000]).unwrap();
        realtime(|| {
            player.update();
            for _ in 0..1500 {
                player.next();
            }
            player.update();
            assert_eq!(player.next(), [0.0; 2]);
            assert!(player.active.is_none());
        });
        sender.drain();
        assert!(!sender.playing());
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    use crate::{Callback, Status, preparation_tests::realtime, tests::transport_plan};
    #[test]
    fn stream_starts_with_the_actual_device_and_converter_batch_without_callback_allocation() {
        for project_rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for device_rate in [44100, 48000, 192000] {
                let mut plan = transport_plan();
                plan.project.sample_rate_hz = project_rate;
                let meters = crate::track_meters(&plan, &Default::default());
                let (_commands, incoming) = RingBuffer::new(32);
                let (retire, _retired) = RingBuffer::new(64);
                let mut callback = Callback::new(
                    plan,
                    &meters,
                    incoming,
                    retire,
                    Arc::new(Status::default()),
                    device_rate,
                )
                .unwrap();
                let mut sender = callback.audition_sender.take().unwrap();
                let (writer, stream) = crate::audition_stream();
                sender.play_stream(stream).unwrap();
                let frames = callback.audition_chunk_frames(32, device_rate);
                assert!(frames >= callback.input.first().map_or(0, Vec::len));
                assert!(frames as u64 * u64::from(device_rate) >= 32 * u64::from(project_rate));
                realtime(|| {
                    callback.audition.set_stream_chunk_frames(frames);
                    callback.update();
                });
                assert_eq!(writer.wait_for_start(&|| false), Some(frames));
                let larger = callback.audition_chunk_frames(4096, device_rate);
                realtime(|| callback.audition.set_stream_chunk_frames(larger));
                assert_eq!(writer.chunk_frames(), larger);
                sender.stop();
            }
        }
    }
    #[test]
    fn preview_mixes_through_project_clock_without_moving_stopped_transport() {
        let plan = transport_plan();
        let meters = crate::track_meters(&plan, &Default::default());
        let (_commands, incoming) = RingBuffer::new(32);
        let (retire, _retired) = RingBuffer::new(64);
        let mut callback = Callback::new(
            plan,
            &meters,
            incoming,
            retire,
            Arc::new(Status::default()),
            48_000,
        )
        .unwrap();
        let mut sender = callback.audition_sender.take().unwrap();
        sender.play(vec![[0.25, -0.5]; 4800]).unwrap();
        let frame = callback.renderer.playhead;
        realtime(|| {
            callback.update();
            for _ in 0..500 {
                callback.next();
            }
            assert_eq!(callback.next(), [0.25, -0.5]);
            assert_eq!(callback.renderer.playhead, frame);
            assert!(!callback.renderer.playing);
        });
        sender.stop();
        realtime(|| {
            for _ in 0..500 {
                callback.next();
            }
            callback.update();
            assert_eq!(callback.next(), [0.0; 2]);
        });
        sender.drain();
    }
}
