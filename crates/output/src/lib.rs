use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use daw_core::Id;
use daw_engine::{RenderPlan, Renderer};
use rtrb::{Consumer, Producer, RingBuffer};
use rubato::Resampler;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

pub struct Status {
    pub playhead: AtomicU64,
    pub playing: AtomicBool,
    pub failed: AtomicBool,
    pub clipped: AtomicBool,
    pub peaks: [AtomicU32; 2],
}
impl Default for Status {
    fn default() -> Self {
        Self {
            playhead: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            clipped: AtomicBool::new(false),
            peaks: [AtomicU32::new(0), AtomicU32::new(0)],
        }
    }
}
#[derive(Default)]
pub struct TrackMeter {
    pub peaks: [AtomicU32; 2],
    pub clipped: [AtomicBool; 2],
}
impl TrackMeter {
    fn publish(&self, peaks: [f32; 2]) {
        for (ch, peak) in peaks.into_iter().enumerate() {
            self.peaks[ch].store(peak.to_bits(), Ordering::Relaxed);
            if peak > 1.0 {
                self.clipped[ch].store(true, Ordering::Relaxed);
            }
        }
    }
}
// Prepared on the application thread and retired there. The callback only reads
// the existing meter handles and writes atomics; it does not allocate or lock.
struct PreparedRenderer {
    renderer: Renderer,
    meters: Vec<Arc<TrackMeter>>,
}
impl PreparedRenderer {
    fn new(plan: RenderPlan, meters: &HashMap<Id, Arc<TrackMeter>>) -> Self {
        let handles = plan
            .project
            .tracks
            .iter()
            .map(|t| meters[&t.id].clone())
            .collect();
        Self {
            renderer: Renderer::new(plan),
            meters: handles,
        }
    }
    fn publish_track_peaks(&self) {
        for (meter, peaks) in self.meters.iter().zip(self.renderer.track_peaks()) {
            meter.publish(peaks);
        }
    }
}
impl std::ops::Deref for PreparedRenderer {
    type Target = Renderer;
    fn deref(&self) -> &Renderer {
        &self.renderer
    }
}
impl std::ops::DerefMut for PreparedRenderer {
    fn deref_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
}
fn track_meters(
    plan: &RenderPlan,
    previous: &HashMap<Id, Arc<TrackMeter>>,
) -> HashMap<Id, Arc<TrackMeter>> {
    plan.project
        .tracks
        .iter()
        .map(|t| (t.id, previous.get(&t.id).cloned().unwrap_or_default()))
        .collect()
}
enum Command {
    Plan(Box<PreparedRenderer>),
    Play,
    Pause,
    Stop,
    Seek(u64),
}
type BuiltStream = (
    cpal::Stream,
    Producer<Command>,
    Consumer<Box<PreparedRenderer>>,
);
pub struct AudioOutput {
    _stream: cpal::Stream,
    commands: Producer<Command>,
    retired: Consumer<Box<PreparedRenderer>>,
    pub status: Arc<Status>,
    pub description: String,
    track_meters: HashMap<Id, Arc<TrackMeter>>,
}
impl AudioOutput {
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default audio output device")?;
        let default = device.default_output_config().map_err(|e| e.to_string())?;
        let config = device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .find(|c| {
                c.channels() == default.channels()
                    && c.sample_format() == default.sample_format()
                    && c.min_sample_rate().0 <= 48_000
                    && c.max_sample_rate().0 >= 48_000
            })
            .map(|c| c.with_sample_rate(cpal::SampleRate(48_000)))
            .unwrap_or(default);
        let format = config.sample_format();
        let rate = config.sample_rate().0;
        let mut stream_config: cpal::StreamConfig = config.clone().into();
        if let cpal::SupportedBufferSize::Range { min, max } = config.buffer_size() {
            stream_config.buffer_size = cpal::BufferSize::Fixed(512.clamp(*min, *max));
        }
        let status = Arc::new(Status::default());
        let meters = track_meters(&plan, &HashMap::new());
        // Retried stream construction happens before playback; each attempt receives fresh queues.
        let build = |cfg: &cpal::StreamConfig| -> Result<BuiltStream, String> {
            let (commands, consumer) = RingBuffer::new(32);
            let (retire, retired) = RingBuffer::new(64);
            let state = Callback::new(
                plan.clone(),
                &meters,
                consumer,
                retire,
                status.clone(),
                rate,
            )?;
            macro_rules! stream {
                ($t:ty) => {
                    build_stream::<$t>(&device, cfg, state, status.clone())
                };
            }
            let stream = match format {
                cpal::SampleFormat::F32 => stream!(f32),
                cpal::SampleFormat::F64 => stream!(f64),
                cpal::SampleFormat::I8 => stream!(i8),
                cpal::SampleFormat::I16 => stream!(i16),
                cpal::SampleFormat::I32 => stream!(i32),
                cpal::SampleFormat::I64 => stream!(i64),
                cpal::SampleFormat::U8 => stream!(u8),
                cpal::SampleFormat::U16 => stream!(u16),
                cpal::SampleFormat::U32 => stream!(u32),
                cpal::SampleFormat::U64 => stream!(u64),
                _ => return Err("Unsupported output device sample format".into()),
            }?;
            Ok((stream, commands, retired))
        };
        let (stream, commands, retired) = match build(&stream_config) {
            Ok(s) => s,
            Err(first) => {
                if stream_config.buffer_size == cpal::BufferSize::Default {
                    return Err(first);
                }
                stream_config.buffer_size = cpal::BufferSize::Default;
                build(&stream_config)?
            }
        };
        stream.play().map_err(|e| e.to_string())?;
        let description = format!(
            "{} · {} Hz · {:?}",
            device.name().unwrap_or_else(|_| "Default output".into()),
            rate,
            stream_config.buffer_size
        );
        Ok(Self {
            _stream: stream,
            commands,
            retired,
            status,
            description,
            track_meters: meters,
        })
    }
    fn send(&mut self, command: Command) -> Result<(), String> {
        self.drain();
        self.commands
            .push(command)
            .map_err(|_| "Audio command queue is full; try again".into())
    }
    pub fn sync(&mut self, plan: RenderPlan) -> Result<(), String> {
        let meters = track_meters(&plan, &self.track_meters);
        self.send(Command::Plan(Box::new(PreparedRenderer::new(
            plan, &meters,
        ))))?;
        self.track_meters = meters;
        Ok(())
    }
    pub fn track_meter(&self, id: Id) -> Option<&TrackMeter> {
        self.track_meters.get(&id).map(AsRef::as_ref)
    }
    pub fn play(&mut self) -> Result<(), String> {
        self.send(Command::Play)
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.send(Command::Pause)
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.send(Command::Stop)
    }
    pub fn seek(&mut self, frame: u64) -> Result<(), String> {
        self.send(Command::Seek(frame))
    }
    pub fn drain(&mut self) {
        while let Ok(old) = self.retired.pop() {
            drop(old);
        }
    }
}
struct Callback {
    renderer: Box<PreparedRenderer>,
    commands: Consumer<Command>,
    retired: Producer<Box<PreparedRenderer>>,
    pending: Option<Box<PreparedRenderer>>,
    status: Arc<Status>,
    converter: Option<rubato::FftFixedIn<f32>>,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    output_count: usize,
    index: usize,
    rendered_input: bool,
}
impl Callback {
    fn new(
        plan: RenderPlan,
        meters: &HashMap<Id, Arc<TrackMeter>>,
        commands: Consumer<Command>,
        retired: Producer<Box<PreparedRenderer>>,
        status: Arc<Status>,
        rate: u32,
    ) -> Result<Self, String> {
        let converter = if rate != 48_000 {
            Some(
                rubato::FftFixedIn::new(48_000, rate as usize, 512, 2, 2)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let input = converter
            .as_ref()
            .map(|c| c.input_buffer_allocate(true))
            .unwrap_or_default();
        let output = converter
            .as_ref()
            .map(|c| c.output_buffer_allocate(true))
            .unwrap_or_default();
        Ok(Self {
            renderer: Box::new(PreparedRenderer::new(plan, meters)),
            commands,
            retired,
            pending: None,
            status,
            converter,
            input,
            output,
            output_count: 0,
            index: 0,
            rendered_input: false,
        })
    }
    fn update(&mut self) {
        for _ in 0..8 {
            let command = if let Some(pending) = self.pending.take() {
                Command::Plan(pending)
            } else {
                match self.commands.pop() {
                    Ok(c) => c,
                    Err(_) => break,
                }
            };
            match command {
                Command::Plan(mut next) => {
                    if self.retired.is_full() {
                        self.pending = Some(next);
                        break;
                    }
                    next.inherit(&self.renderer);
                    let old = std::mem::replace(&mut self.renderer, next);
                    // A single producer owns this queue; the capacity check guarantees success.
                    let _ = self.retired.push(old);
                }
                Command::Play => {
                    self.renderer.play();
                    self.discard_buffered_output();
                }
                Command::Pause => {
                    self.renderer.pause();
                    self.discard_buffered_output();
                }
                Command::Stop => {
                    self.renderer.stop();
                    self.discard_buffered_output();
                }
                Command::Seek(frame) => {
                    self.renderer.seek(frame);
                    self.discard_buffered_output();
                }
            }
        }
    }
    fn discard_buffered_output(&mut self) {
        // Discard cached samples from before the command. Keep filter history so
        // the renderer's transport fade remains continuous through resampling.
        self.index = self.output_count;
    }
    fn next(&mut self) -> [f32; 2] {
        if let Some(converter) = &mut self.converter {
            while self.index >= self.output_count {
                self.rendered_input = true;
                for i in 0..converter.input_frames_next() {
                    let s = self.renderer.next_sample();
                    self.input[0][i] = s[0];
                    self.input[1][i] = s[1];
                }
                match converter.process_into_buffer(&self.input, &mut self.output, None) {
                    Ok((_, count)) => {
                        self.output_count = count;
                        self.index = 0;
                    }
                    Err(_) => {
                        self.status.failed.store(true, Ordering::Relaxed);
                        self.renderer.playing = false;
                        return [0.0; 2];
                    }
                }
            }
            let s = [self.output[0][self.index], self.output[1][self.index]];
            self.index += 1;
            s
        } else {
            self.renderer.next_sample()
        }
    }
}
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut callback: Callback,
    status: Arc<Status>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                callback.update();
                callback.renderer.reset_track_peaks();
                callback.rendered_input = false;
                let mut peaks = [0.0_f32; 2];
                for frame in data.chunks_mut(channels) {
                    let sample = callback.next();
                    for ch in 0..2 {
                        peaks[ch] = peaks[ch].max(sample[ch].abs());
                    }
                    for (ch, value) in frame.iter_mut().enumerate() {
                        let s = if channels == 1 {
                            (sample[0] + sample[1]) * 0.5
                        } else if ch < 2 {
                            sample[ch]
                        } else {
                            0.0
                        };
                        *value = T::from_sample(s.clamp(-1.0, 1.0));
                    }
                }
                // Keep the last input-block level when this buffer consumes only
                // cached converter output. Publish silence after new silent input.
                if callback.converter.is_none() || callback.rendered_input {
                    callback.renderer.publish_track_peaks();
                }
                for (ch, peak) in peaks.iter().enumerate() {
                    callback.status.peaks[ch].store(peak.to_bits(), Ordering::Relaxed);
                }
                if peaks.iter().any(|p| *p > 1.0) {
                    callback.status.clipped.store(true, Ordering::Relaxed);
                }
                callback
                    .status
                    .playhead
                    .store(callback.renderer.playhead, Ordering::Relaxed);
                callback
                    .status
                    .playing
                    .store(callback.renderer.playing, Ordering::Relaxed);
            },
            move |_| {
                status.failed.store(true, Ordering::Relaxed);
                status.playing.store(false, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn transport_plan() -> RenderPlan {
        use daw_core::{Asset, Clip, Project, Source, SourceMetadata};
        let mut project = Project::default();
        project.add_track().unwrap();
        project.transport.playhead_frame = 4800;
        let id = Id::new_v4();
        let metadata = SourceMetadata {
            sample_rate_hz: 48_000,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        };
        project.assets.push(Asset {
            id,
            name: "Test".into(),
            source: Source {
                kind: "external".into(),
                path: "/test.wav".into(),
                path_kind: "absolute".into(),
            },
            source_metadata: metadata.clone(),
            decoded_frame_count: 48_000,
        });
        project.tracks[0].clips.push(Clip {
            stretch: None,
            id: Id::new_v4(),
            asset_id: id,
            name: "Test".into(),
            color: None,
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 48_000,
            repeat: None,
        });
        RenderPlan {
            stretched_audio: HashMap::new(),
            project,
            audio: HashMap::from([(
                id,
                daw_media::AudioData {
                    samples: Arc::new(vec![[0.25, -0.25]; 48_000]),
                    metadata,
                    peaks: Arc::new(vec![]),
                },
            )]),
        }
    }

    #[test]
    fn callback_pauses_resumes_and_returns_to_start_with_resampling() {
        for rate in [44_100, 48_000, 96_000] {
            let mut plan = transport_plan();
            let meters = track_meters(&plan, &HashMap::new());
            let (mut producer, consumer) = RingBuffer::new(32);
            let (retire, mut retired) = RingBuffer::new(64);
            let status = Arc::new(Status::default());
            let mut callback = Callback::new(
                plan.clone(),
                &meters,
                consumer,
                retire,
                status.clone(),
                rate,
            )
            .unwrap();
            producer.push(Command::Play).unwrap();
            callback.update();
            for _ in 0..5000 {
                callback.next();
            }
            producer.push(Command::Pause).unwrap();
            callback.update();
            assert!(!callback.renderer.playing);
            let paused_at = callback.renderer.playhead;
            assert!(paused_at > 4800);
            assert_eq!(callback.index, callback.output_count);
            for _ in 0..2000 {
                callback.next();
            }
            assert_eq!(callback.renderer.playhead, paused_at);
            assert_eq!(callback.next(), [0.0; 2]);

            plan.project.transport.r#loop = daw_core::Loop {
                enabled: true,
                start_frame: 8800,
                end_frame: 11_000,
            };
            producer.push(Command::Play).unwrap();
            producer
                .push(Command::Plan(Box::new(PreparedRenderer::new(
                    plan, &meters,
                ))))
                .unwrap();
            producer.push(Command::Seek(9600)).unwrap();
            callback.update();
            assert!(callback.renderer.playing);
            assert!(retired.pop().is_ok());
            for _ in 0..2000 {
                callback.next();
            }
            producer.push(Command::Stop).unwrap();
            callback.update();
            assert!(!callback.renderer.playing);
            assert_eq!(callback.renderer.playhead, 4800);
            assert_eq!(callback.index, callback.output_count);
            for _ in 0..3000 {
                callback.next();
            }
            assert_eq!(callback.renderer.playhead, 4800);
            assert_eq!(callback.next(), [0.0; 2]);
            assert!(!status.failed.load(Ordering::Relaxed));
        }
    }

    #[test]
    fn playback_and_meters_include_tracks_beyond_four() {
        let mut plan = transport_plan();
        let clip = plan.project.tracks[0].clips[0].clone();
        for _ in 0..7 {
            plan.project.add_track().unwrap();
            plan.project
                .tracks
                .last_mut()
                .unwrap()
                .clips
                .push(daw_core::Clip {
                    id: Id::new_v4(),
                    ..clip.clone()
                });
        }
        for track in &mut plan.project.tracks {
            track.gain_db = -12.0;
        }
        plan.project.validate().unwrap();
        let expected = plan.sample_at(5800);
        let meters = track_meters(&plan, &HashMap::new());
        let mut prepared = PreparedRenderer::new(plan.clone(), &meters);
        prepared.play();
        for _ in 0..1000 {
            prepared.next_sample();
        }
        let actual = prepared.next_sample();
        for ch in 0..2 {
            assert!((actual[ch] - expected[ch]).abs() < 1e-6);
        }
        prepared.publish_track_peaks();
        assert_eq!(meters.len(), 8);
        for meter in meters.values() {
            for peak in &meter.peaks {
                assert!(
                    (f32::from_bits(peak.load(Ordering::Relaxed)) - expected[0].abs() / 8.0).abs()
                        < 1e-6
                );
            }
        }

        // A live plan update must also prepare meters for newly added tracks.
        plan.project.add_track().unwrap();
        plan.project
            .tracks
            .last_mut()
            .unwrap()
            .clips
            .push(daw_core::Clip {
                id: Id::new_v4(),
                ..clip
            });
        let updated_meters = track_meters(&plan, &meters);
        let mut updated = PreparedRenderer::new(plan.clone(), &updated_meters);
        updated.inherit(&prepared);
        for _ in 0..1000 {
            updated.next_sample();
        }
        let expected = plan.sample_at(updated.playhead);
        let actual = updated.next_sample();
        for ch in 0..2 {
            assert!((actual[ch] - expected[ch]).abs() < 1e-6);
        }
        updated.publish_track_peaks();
        assert_eq!(updated_meters.len(), 9);
        let last = plan.project.tracks.last().unwrap().id;
        assert_eq!(
            f32::from_bits(updated_meters[&last].peaks[0].load(Ordering::Relaxed)),
            0.25
        );
    }
    #[test]
    fn track_meter_latches_each_channel_and_follows_track_identity() {
        let mut plan = RenderPlan {
            stretched_audio: HashMap::new(),
            project: daw_core::Project::default(),
            audio: HashMap::new(),
        };
        plan.project.add_track().unwrap();
        plan.project.add_track().unwrap();
        let first = plan.project.tracks[0].id;
        let second = plan.project.tracks[1].id;
        let meters = track_meters(&plan, &HashMap::new());
        meters[&first].publish([1.2, 0.5]);
        meters[&first].publish([0.0; 2]);
        assert!(meters[&first].clipped[0].load(Ordering::Relaxed));
        assert!(!meters[&first].clipped[1].load(Ordering::Relaxed));
        assert!(!meters[&second].clipped[0].load(Ordering::Relaxed));
        meters[&first].clipped[0].store(false, Ordering::Relaxed);
        meters[&first].publish([0.5, 1.1]);
        assert!(!meters[&first].clipped[0].load(Ordering::Relaxed));
        assert!(meters[&first].clipped[1].load(Ordering::Relaxed));

        plan.project.tracks.swap(0, 1);
        let updated = track_meters(&plan, &meters);
        assert!(Arc::ptr_eq(&updated[&first], &meters[&first]));
        plan.project.tracks.retain(|t| t.id == second);
        let updated = track_meters(&plan, &updated);
        assert!(!updated.contains_key(&first));
        assert!(Arc::ptr_eq(&updated[&second], &meters[&second]));
    }
    #[test]
    fn callback_resamples_without_device_and_retires_plans() {
        for rate in [44_100, 48_000, 96_000] {
            let plan = RenderPlan {
                stretched_audio: HashMap::new(),
                project: daw_core::Project::default(),
                audio: HashMap::new(),
            };
            let (mut producer, consumer) = RingBuffer::new(32);
            let (retire, mut retired) = RingBuffer::new(64);
            let status = Arc::new(Status::default());
            let mut callback = Callback::new(
                plan.clone(),
                &HashMap::new(),
                consumer,
                retire,
                status.clone(),
                rate,
            )
            .unwrap();
            producer.push(Command::Play).unwrap();
            callback.update();
            assert!(!callback.renderer.playing);
            for _ in 0..10_000 {
                assert_eq!(callback.next(), [0.0; 2]);
            }
            assert!(!status.failed.load(Ordering::Relaxed));
            producer.push(Command::Seek(123)).unwrap();
            producer
                .push(Command::Plan(Box::new(PreparedRenderer::new(
                    plan,
                    &HashMap::new(),
                ))))
                .unwrap();
            callback.update();
            assert_eq!(callback.renderer.playhead, 123);
            assert!(retired.pop().is_ok());
        }
    }
}
