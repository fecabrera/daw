//! Bounded preview queue. Buffer ownership returns to the control thread.
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
struct Buffer {
    generation: u64,
    samples: Vec<[f32; 2]>,
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
        self.drain();
        self.stop();
        let generation = self.generation.load(Ordering::Acquire);
        self.playing.store(generation, Ordering::Release);
        let result = self
            .incoming
            .push(Box::new(Buffer {
                generation,
                samples,
            }))
            .map_err(|_| "Chord audition queue is full".to_owned());
        if result.is_err() {
            self.playing.store(0, Ordering::Release);
        }
        result
    }
}
impl Player {
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
                self.active = Some(buffer);
                self.index = 0;
                self.gain = self.step;
                break;
            }
        }
    }
    pub fn next(&mut self) -> [f32; 2] {
        let Some(buffer) = &self.active else {
            return [0.0; 2];
        };
        let current = buffer.generation == self.generation.load(Ordering::Acquire)
            && self.index < buffer.samples.len();
        self.gain = if current {
            (self.gain + self.step).min(1.0)
        } else {
            (self.gain - self.step).max(0.0)
        };
        // Hold the last sample only during cancellation's five-millisecond fade.
        let sample = buffer
            .samples
            .get(self.index.min(buffer.samples.len().saturating_sub(1)))
            .copied()
            .unwrap_or([0.0; 2]);
        self.index = self.index.saturating_add(1);
        [sample[0] * self.gain, sample[1] * self.gain]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preparation_tests::realtime;
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
