//! Worker-only owned track jobs. The callback never dispatches or waits here.
use super::{
    Renderer, effects::EffectChain, latency::Delay, midi::MidiTrackRuntime, schedule::AudioTrack,
    track_processing::TrackProcessor,
};
use daw_core::{Clip, TrackKind};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(super) const GRAIN: u64 = 2048;
const STACK: usize = 512 * 1024;
const SCRATCH_LIMIT: u64 = 64 * 1024 * 1024;

/// Auto selects an eligible worker width for substantial multi-track history.
/// Serial is a reference path. Parallel requests two workers even for cheap tracks;
/// budget admission or thread-start failure still falls back to serial processing.
/// ParallelWithWidth qualifies a two- to four-worker prefix while live catch-up
/// remains two workers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreparationMode {
    #[default]
    Auto,
    Serial,
    Parallel,
    ParallelWithWidth(u8),
}
#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationStats {
    pub parallel_blocks: u64,
    pub serial_blocks: u64,
    pub parallel_bytes: u64,
    pub workers: usize,
}
struct TrackWork {
    index: usize,
    group: bool,
    audio: AudioTrack,
    midi: Option<MidiTrackRuntime>,
    effects: EffectChain,
    delay: Delay,
    gains: [f32; 4],
    output: Vec<super::latency::Sample>,
    raw: Vec<super::latency::Sample>,
    elapsed: u64,
}
struct Batch {
    items: Vec<TrackWork>,
    clips: Arc<Vec<Vec<Clip>>>,
    from: u64,
    frames: usize,
    tempo: f32,
    region: Option<(u64, u64)>,
    cancel_block: u64,
    live_tile: bool,
    complete: bool,
    failed: bool,
}
impl Batch {
    fn process(&mut self, cancel: &AtomicBool) -> bool {
        for work in &mut self.items {
            let started = Instant::now();
            work.output.resize(self.frames, ([0.0; 2], false));
            work.raw.resize(self.frames, ([0.0; 2], false));
            if work.group {
                continue;
            }
            let mut processor = TrackProcessor {
                monitor_only: false,
                paused: false,
                sidechains: &[],
                sidechain_sources: None,
                audio: &mut work.audio,
                midi: &mut work.midi,
                effects: &mut work.effects,
                delay: &mut work.delay,
                clips: &self.clips[work.index],
            };
            for (offset, output) in work.output.iter_mut().enumerate() {
                let frame = self.from + offset as u64;
                if (offset == 0 || frame.is_multiple_of(self.cancel_block))
                    && cancel.load(Ordering::Relaxed)
                {
                    return false;
                }
                if self.live_tile
                    && !processor
                        .audio
                        .ready(processor.clips, frame, self.region.map(|r| r.0))
                {
                    self.failed = true;
                    return false;
                }
                let raw = processor.sample_unaligned(
                    frame,
                    self.tempo,
                    self.region,
                    true,
                    self.live_tile,
                );
                work.raw[offset] = raw;
                let (sample, mono) = processor.delay.tick(raw);
                let base = if mono { 0 } else { 2 };
                *output = if self.live_tile {
                    (sample, mono)
                } else {
                    (
                        std::array::from_fn(|ch| sample[ch] * work.gains[base + ch]),
                        mono,
                    )
                };
            }
            work.elapsed = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        }
        true
    }
}
pub(super) struct ParallelTracks {
    senders: Vec<mpsc::SyncSender<Batch>>,
    done: mpsc::Receiver<(usize, Batch)>,
    threads: Vec<JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    batches: Vec<Option<Batch>>,
    slots: Vec<Option<TrackWork>>,
    buffers: Vec<Vec<super::latency::Sample>>,
    raw_buffers: Vec<Vec<super::latency::Sample>>,
    times: Vec<u64>,
    order: Vec<usize>,
    groups: Vec<usize>,
    bytes: u64,
    block: usize,
}
impl ParallelTracks {
    fn estimate(renderer: &Renderer, workers: usize) -> u64 {
        let tracks = &renderer.plan.project.tracks;
        let block = u64::from(renderer.plan.project.buffer_size_frames).max(GRAIN);
        let metadata = tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| std::mem::size_of::<Clip>() as u64 + c.name.len() as u64)
            .sum::<u64>();
        (workers * STACK + 64 * 1024) as u64
            + metadata
            + tracks.len() as u64
                * (block * 32
                    + ((workers + 1)
                        * (std::mem::size_of::<TrackWork>()
                            + std::mem::size_of::<Option<TrackWork>>()))
                        as u64
                    + 128)
    }
    pub fn new(renderer: &Renderer, available: u64, workers: usize) -> Option<Self> {
        let project = &renderer.plan.project;
        // Captured Return inputs still need the shared serial routing path.
        if renderer.sidechains.active()
            || (project.tracks.iter().any(|t| t.kind == TrackKind::Return)
                && project.assets.iter().any(|a| a.rendered_audio.is_some()))
        {
            return None;
        }
        let bytes = Self::estimate(renderer, workers);
        if bytes > available.min(SCRATCH_LIMIT) {
            return None;
        }
        let count = renderer.plan.project.tracks.len();
        let clips = Arc::new(
            renderer
                .plan
                .project
                .tracks
                .iter()
                .map(|t| t.clips.clone())
                .collect(),
        );
        let block = u64::from(renderer.plan.project.buffer_size_frames).max(GRAIN) as usize;
        let (done_tx, done) = mpsc::sync_channel(workers);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut pool = Self {
            senders: Vec::with_capacity(workers),
            done,
            threads: Vec::with_capacity(workers),
            cancel: cancel.clone(),
            batches: (0..workers)
                .map(|_| {
                    Some(Batch {
                        items: Vec::with_capacity(count),
                        clips: Arc::clone(&clips),
                        from: 0,
                        frames: 0,
                        tempo: 120.0,
                        region: None,
                        cancel_block: 512,
                        live_tile: false,
                        complete: false,
                        failed: false,
                    })
                })
                .collect(),
            slots: (0..count).map(|_| None).collect(),
            buffers: (0..count).map(|_| Vec::with_capacity(block)).collect(),
            raw_buffers: (0..count).map(|_| Vec::with_capacity(block)).collect(),
            times: vec![1; count],
            order: (0..count).collect(),
            groups: vec![0; count],
            bytes,
            block,
        };
        for worker in 0..workers {
            let (sender, receiver) = mpsc::sync_channel::<Batch>(1);
            let finished = done_tx.clone();
            let cancel = cancel.clone();
            let thread = std::thread::Builder::new()
                .name(format!("cache-track-{worker}"))
                .stack_size(STACK)
                .spawn(move || {
                    while let Ok(mut batch) = receiver.recv() {
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            batch.process(&cancel)
                        })) {
                            Ok(complete) => batch.complete = complete,
                            Err(_) => batch.failed = true,
                        }
                        if finished.send((worker, batch)).is_err() {
                            break;
                        }
                    }
                })
                .ok()?;
            pool.senders.push(sender);
            pool.threads.push(thread);
        }
        Some(pool)
    }
    #[cfg(test)]
    pub fn disconnect_worker(&mut self) {
        // Drop the receiver immediately; the next send returns its owned job.
        self.senders[0] = mpsc::sync_channel(1).0;
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn block(&self) -> u64 {
        self.block as u64
    }
    pub fn workers(&self) -> usize {
        self.threads.len()
    }
    /// A partial job is never a checkpoint. Restore ownership before returning;
    /// callers discard a cancelled cursor and restart from a complete checkpoint.
    pub fn process(
        &mut self,
        renderer: &mut Renderer,
        range: std::ops::Range<u64>,
        live: bool,
        live_tile: bool,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<bool, ()> {
        let from = range.start;
        let to = range.end;
        let frames = (to - from) as usize;
        if frames > self.block {
            return Err(());
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.order
            .sort_unstable_by_key(|&i| std::cmp::Reverse(self.times[i]));
        let count = if live_tile { 2 } else { self.workers() };
        let mut loads = [0_u64; 4];
        let loads = &mut loads[..count];
        for &index in &self.order {
            let group = (0..count).min_by_key(|&worker| loads[worker]).unwrap();
            self.groups[index] = group;
            loads[group] = loads[group].saturating_add(self.times[index].max(1));
        }
        let region = renderer.loop_region(live);
        for batch in self.batches.iter_mut().flatten() {
            batch.from = from;
            batch.frames = frames;
            batch.tempo = renderer.plan.project.tempo_bpm;
            batch.region = region;
            batch.cancel_block = u64::from(renderer.plan.project.buffer_size_frames.max(1));
            batch.live_tile = live_tile;
            batch.complete = false;
            batch.failed = false;
        }
        for (index, (((audio, midi), effects), delay)) in renderer
            .audio_tracks
            .drain(..)
            .zip(renderer.midi.drain(..))
            .zip(renderer.effects.drain(..))
            .zip(renderer.alignment.tracks.drain(..))
            .enumerate()
        {
            self.batches[self.groups[index]]
                .as_mut()
                .ok_or(())?
                .items
                .push(TrackWork {
                    index,
                    group: super::send_routing::is_bus(renderer.plan.project.tracks[index].kind),
                    audio,
                    midi,
                    effects,
                    delay,
                    gains: renderer.gains[index].target,
                    output: std::mem::take(&mut self.buffers[index]),
                    raw: std::mem::take(&mut self.raw_buffers[index]),
                    elapsed: 0,
                });
        }
        for worker in 0..count {
            self.senders[worker]
                .send(self.batches[worker].take().ok_or(())?)
                .map_err(|_| ())?;
        }
        for _ in 0..count {
            loop {
                match self.done.recv_timeout(Duration::from_millis(1)) {
                    Ok((worker, batch)) => {
                        self.batches[worker] = Some(batch);
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if cancelled() {
                            self.cancel.store(true, Ordering::Relaxed);
                        }
                        if self.threads.iter().any(JoinHandle::is_finished) {
                            return Err(());
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Err(()),
                }
            }
        }
        let mut complete = self.batches[..count].iter().flatten().all(|b| b.complete);
        let failed = self.batches[..count].iter().flatten().any(|b| b.failed);
        for batch in self.batches[..count].iter_mut().flatten() {
            for item in batch.items.drain(..) {
                let index = item.index;
                self.times[index] = item.elapsed.max(1);
                self.slots[index] = Some(item);
            }
        }
        // Restore all processors before processing the dependent bus stages.
        for slot in &mut self.slots {
            let item = slot.take().ok_or(())?;
            renderer.audio_tracks.push(item.audio);
            renderer.midi.push(item.midi);
            renderer.effects.push(item.effects);
            renderer.alignment.tracks.push(item.delay);
            self.buffers[item.index] = item.output;
            self.raw_buffers[item.index] = item.raw;
        }
        if complete && !failed {
            for offset in 0..frames {
                let frame = from + offset as u64;
                if frame.is_multiple_of(u64::from(renderer.plan.project.buffer_size_frames.max(1)))
                    && cancelled()
                {
                    complete = false;
                    break;
                }
                if live_tile {
                    renderer.advance_live_ramp(true);
                }
                for (index, buffer) in self.buffers.iter().enumerate() {
                    let (sample, mono) = buffer[offset];
                    if live_tile
                        && super::send_routing::is_bus(renderer.plan.project.tracks[index].kind)
                    {
                        continue;
                    }
                    renderer.buses.output[index] = if live_tile {
                        renderer.gains[index].apply(sample, mono, true, renderer.ramp)
                    } else {
                        sample
                    };
                    renderer.buses.raw[index] = self.raw_buffers[index][offset];
                }
                let mix = renderer.sample_buses(frame, region, live_tile, live_tile);
                let mix = renderer.sample_master(mix, frame, region, live_tile);
                if live_tile {
                    renderer.alignment.previous_feed = Some(frame);
                    renderer.latent_held_mix = mix;
                    renderer.playhead += 1;
                    renderer.finish_live_mix(mix, false);
                }
            }
        }
        if failed {
            Err(())
        } else {
            Ok(complete && !cancelled())
        }
    }
}
impl Drop for ParallelTracks {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.senders.clear();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
