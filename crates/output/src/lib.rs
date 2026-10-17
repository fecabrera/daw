use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use daw_core::Id;
use daw_engine::{ModulationTarget, RenderPlan, Renderer};
use rtrb::{Consumer, Producer, RingBuffer};
use rubato::Resampler;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

mod meters;
pub use meters::StereoMeter;

#[derive(Default)]
pub struct Status {
    pub playhead: AtomicU64,
    pub playing: AtomicBool,
    pub can_return_to_playback_start: AtomicBool,
    pub failed: AtomicBool,
    pub meter: StereoMeter,
}
impl Status {
    fn publish_transport(&self, renderer: &Renderer) {
        self.playhead.store(renderer.playhead, Ordering::Relaxed);
        self.playing.store(renderer.playing, Ordering::Relaxed);
        self.can_return_to_playback_start
            .store(renderer.can_return_to_playback_start(), Ordering::Relaxed);
    }
}
// Prepared on the application thread and retired there. The callback only reads
// the existing meter handles and writes atomics; it does not allocate or lock.
struct PreparedRenderer {
    renderer: Renderer,
    meters: Vec<Arc<StereoMeter>>,
    modulation: Vec<(ModulationTarget, Arc<[AtomicU32; 5]>)>,
}
impl PreparedRenderer {
    fn new(plan: RenderPlan, meters: &HashMap<Id, Arc<StereoMeter>>) -> Self {
        let handles = plan
            .project
            .tracks
            .iter()
            .map(|t| meters[&t.id].clone())
            .collect();
        Self {
            renderer: Renderer::new(plan),
            meters: handles,
            modulation: Vec::new(),
        }
    }
    fn bind_modulation(&mut self, meters: &HashMap<ModulationTarget, Arc<[AtomicU32; 5]>>) {
        self.modulation = meters
            .iter()
            .map(|(key, meter)| (*key, meter.clone()))
            .collect();
    }
    fn publish_track_peaks(&self) {
        for (key, meter) in &self.modulation {
            let values = self
                .renderer
                .modulation_values(*key)
                .unwrap_or([f32::NAN; 5]);
            for (cell, value) in meter.iter().zip(values) {
                cell.store(value.to_bits(), Ordering::Relaxed);
            }
        }
        for (meter, peaks) in self.meters.iter().zip(self.renderer.track_peaks()) {
            meter.publish_peaks(peaks);
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
    previous: &HashMap<Id, Arc<StereoMeter>>,
) -> HashMap<Id, Arc<StereoMeter>> {
    plan.project
        .tracks
        .iter()
        .map(|t| (t.id, previous.get(&t.id).cloned().unwrap_or_default()))
        .collect()
}
fn effect_meters(
    plan: &RenderPlan,
    previous: &HashMap<Id, Arc<AtomicU32>>,
) -> HashMap<Id, Arc<AtomicU32>> {
    plan.project
        .all_effects()
        .filter(|effect| {
            matches!(
                effect.unit,
                daw_core::EffectSettings::Compressor(_) | daw_core::EffectSettings::Limiter(_)
            )
        })
        .map(|effect| {
            (
                effect.id,
                previous.get(&effect.id).cloned().unwrap_or_default(),
            )
        })
        .collect()
}
fn bind_effect_meters(renderer: &mut Renderer, meters: &HashMap<Id, Arc<AtomicU32>>) {
    for (id, meter) in meters {
        renderer.set_effect_meter(*id, meter.clone());
    }
}
fn band_meters(
    plan: &RenderPlan,
    previous: &HashMap<Id, Arc<[AtomicU32; 3]>>,
) -> HashMap<Id, Arc<[AtomicU32; 3]>> {
    plan.project
        .all_effects()
        .filter(|effect| {
            matches!(
                effect.unit,
                daw_core::EffectSettings::Ott(_) | daw_core::EffectSettings::Eq3(_)
            )
        })
        .map(|effect| {
            (
                effect.id,
                previous.get(&effect.id).cloned().unwrap_or_default(),
            )
        })
        .collect()
}
fn bind_band_meters(renderer: &mut Renderer, meters: &HashMap<Id, Arc<[AtomicU32; 3]>>) {
    for (id, meter) in meters {
        renderer.set_band_meter(*id, meter.clone());
    }
}
fn modulation_meters(
    plan: &RenderPlan,
    previous: &HashMap<ModulationTarget, Arc<[AtomicU32; 5]>>,
) -> HashMap<ModulationTarget, Arc<[AtomicU32; 5]>> {
    let effects = plan
        .project
        .all_effects()
        .filter(|e| {
            matches!(
                e.unit,
                daw_core::EffectSettings::AutoFilter(_) | daw_core::EffectSettings::Drive(_)
            )
        })
        .map(|e| ModulationTarget::Effect(e.id));
    let amplitudes = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |instrument| {
            instrument
                .layers
                .iter()
                .filter(|l| l.amplitude.present)
                .map(move |layer| ModulationTarget::Amplitude {
                    track: track.id,
                    layer: layer.id,
                })
        })
    });
    let oscillators = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.layers.iter().flat_map(move |l| {
                l.oscillators
                    .iter()
                    .map(move |s| ModulationTarget::Oscillator {
                        track: track.id,
                        layer: l.id,
                        oscillator: s.id,
                    })
            })
        })
    });
    let lfos = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.lfo_destinations()
                .filter(|(_, l)| l.present)
                .map(move |(id, _)| ModulationTarget::Lfo {
                    track: track.id,
                    layer: id.layer,
                    unit: id.unit,
                })
        })
    });
    let macros = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.macro_modules()
                .filter(|(_, m)| m.present)
                .flat_map(move |(layer, m)| {
                    m.controls
                        .iter()
                        .filter(|c| c.present)
                        .map(move |c| ModulationTarget::Macro {
                            track: track.id,
                            layer,
                            unit: c.id,
                        })
                })
        })
    });
    effects
        .chain(macros)
        .chain(lfos)
        .chain(amplitudes)
        .chain(oscillators)
        .map(|key| {
            let handle = previous.get(&key).cloned().unwrap_or_else(|| {
                Arc::new(std::array::from_fn(|_| AtomicU32::new(f32::NAN.to_bits())))
            });
            (key, handle)
        })
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
    project_rate_hz: u32,
    buffer_size_frames: u32,
    _stream: cpal::Stream,
    commands: Producer<Command>,
    retired: Consumer<Box<PreparedRenderer>>,
    pub status: Arc<Status>,
    pub description: String,
    track_meters: HashMap<Id, Arc<StereoMeter>>,
    effect_meters: HashMap<Id, Arc<AtomicU32>>,
    band_meters: HashMap<Id, Arc<[AtomicU32; 3]>>,
    modulation_meters: HashMap<ModulationTarget, Arc<[AtomicU32; 5]>>,
}
impl AudioOutput {
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        let project_rate_hz = plan.project.sample_rate_hz;
        let buffer_size_frames = plan.project.buffer_size_frames;
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
                    && c.min_sample_rate().0 <= project_rate_hz
                    && c.max_sample_rate().0 >= project_rate_hz
            })
            .map(|c| c.with_sample_rate(cpal::SampleRate(project_rate_hz)))
            .unwrap_or(default);
        let format = config.sample_format();
        let rate = config.sample_rate().0;
        let mut stream_config: cpal::StreamConfig = config.clone().into();
        stream_config.buffer_size = cpal::BufferSize::Fixed(match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => buffer_size_frames.clamp(*min, *max),
            cpal::SupportedBufferSize::Unknown => buffer_size_frames,
        });
        let status = Arc::new(Status::default());
        let meters = track_meters(&plan, &HashMap::new());
        let reductions = effect_meters(&plan, &HashMap::new());
        let changes = band_meters(&plan, &HashMap::new());
        let modulation = modulation_meters(&plan, &HashMap::new());
        // Retried stream construction happens before playback; each attempt receives fresh queues.
        let build = |cfg: &cpal::StreamConfig| -> Result<BuiltStream, String> {
            let (commands, consumer) = RingBuffer::new(32);
            let (retire, retired) = RingBuffer::new(64);
            let mut state = Callback::new(
                plan.clone(),
                &meters,
                consumer,
                retire,
                status.clone(),
                rate,
            )?;
            bind_effect_meters(&mut state.renderer, &reductions);
            bind_band_meters(&mut state.renderer, &changes);
            state.renderer.bind_modulation(&modulation);
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
            "{} · {} Hz · {:?} (requested {} samples)",
            device.name().unwrap_or_else(|_| "Default output".into()),
            rate,
            stream_config.buffer_size,
            buffer_size_frames
        );
        Ok(Self {
            project_rate_hz,
            buffer_size_frames,
            _stream: stream,
            commands,
            retired,
            status,
            description,
            track_meters: meters,
            effect_meters: reductions,
            band_meters: changes,
            modulation_meters: modulation,
        })
    }
    fn send(&mut self, command: Command) -> Result<(), String> {
        self.drain();
        self.commands
            .push(command)
            .map_err(|_| "Audio command queue is full; try again".into())
    }
    pub fn sync(&mut self, plan: RenderPlan) -> Result<(), String> {
        if plan.project.sample_rate_hz != self.project_rate_hz
            || plan.project.buffer_size_frames != self.buffer_size_frames
        {
            return Err("Recreate output after changing project audio settings".into());
        }
        let meters = track_meters(&plan, &self.track_meters);
        let reductions = effect_meters(&plan, &self.effect_meters);
        let changes = band_meters(&plan, &self.band_meters);
        let modulation = modulation_meters(&plan, &self.modulation_meters);
        let mut prepared = PreparedRenderer::new(plan, &meters);
        bind_effect_meters(&mut prepared, &reductions);
        bind_band_meters(&mut prepared, &changes);
        prepared.bind_modulation(&modulation);
        self.send(Command::Plan(Box::new(prepared)))?;
        self.track_meters = meters;
        self.effect_meters = reductions;
        self.band_meters = changes;
        self.modulation_meters = modulation;
        Ok(())
    }
    pub fn track_meter(&self, id: Id) -> Option<&StereoMeter> {
        self.track_meters.get(&id).map(AsRef::as_ref)
    }
    /// Display-only block snapshots. No synthetic animation when playback is stopped.
    pub fn modulation_values(&self, key: ModulationTarget) -> Option<[f32; 5]> {
        if !self.status.playing.load(Ordering::Relaxed) {
            return None;
        }
        let meter = self.modulation_meters.get(&key)?;
        let values = std::array::from_fn(|i| f32::from_bits(meter[i].load(Ordering::Relaxed)));
        values[0].is_finite().then_some(values)
    }
    pub fn gain_reduction_db(&self, id: Id) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.effect_meters
            .get(&id)
            .map_or(0.0, |meter| f32::from_bits(meter.load(Ordering::Relaxed)))
    }
    pub fn ott_gain_change_db(&self, id: Id) -> [f32; 3] {
        self.band_monitor_values(id)
    }
    /// Linear Low/Mid/High output peaks. Stopped or absent outputs show silence.
    pub fn eq_band_levels(&self, id: Id) -> [f32; 3] {
        self.band_monitor_values(id)
    }
    fn band_monitor_values(&self, id: Id) -> [f32; 3] {
        if !self.status.playing.load(Ordering::Relaxed) {
            return [0.0; 3];
        }
        self.band_meters.get(&id).map_or([0.0; 3], |meter| {
            std::array::from_fn(|i| f32::from_bits(meter[i].load(Ordering::Relaxed)))
        })
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
        meters: &HashMap<Id, Arc<StereoMeter>>,
        commands: Consumer<Command>,
        retired: Producer<Box<PreparedRenderer>>,
        status: Arc<Status>,
        rate: u32,
    ) -> Result<Self, String> {
        let project_rate_hz = plan.project.sample_rate_hz;
        let buffer_size_frames = plan.project.buffer_size_frames as usize;
        let converter = if rate != project_rate_hz {
            Some(
                rubato::FftFixedIn::new(
                    project_rate_hz as usize,
                    rate as usize,
                    buffer_size_frames,
                    2,
                    2,
                )
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
                    next.inherit(&mut self.renderer);
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
                callback.status.meter.publish_peaks(peaks);
                callback.status.publish_transport(&callback.renderer);
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
    pub(super) fn transport_plan() -> RenderPlan {
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
            midi_pattern: None,
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
                    sample_rate_hz: daw_core::SAMPLE_RATE,
                    samples: Arc::new(vec![[0.25, -0.25]; 48_000]),
                    metadata,
                    peaks: Arc::new(vec![]),
                },
            )]),
        }
    }

    #[test]
    fn variable_project_rates_and_buffers_render_at_device_rate() {
        for project_rate in daw_core::SAMPLE_RATES {
            for buffer in daw_core::BUFFER_SIZES {
                let mut plan = transport_plan();
                plan.project = plan
                    .project
                    .with_audio_settings(project_rate, buffer)
                    .unwrap();
                let id = plan.project.assets[0].id;
                plan.audio
                    .insert(id, plan.audio[&id].at_sample_rate(project_rate).unwrap());
                let meters = track_meters(&plan, &HashMap::new());
                let (mut producer, consumer) = RingBuffer::new(32);
                let (retire, _retired) = RingBuffer::new(64);
                let status = Arc::new(Status::default());
                let start = plan.project.transport.playhead_frame;
                let mut callback =
                    Callback::new(plan, &meters, consumer, retire, status.clone(), 48_000).unwrap();
                producer.push(Command::Play).unwrap();
                callback.update();
                let mut audible = false;
                for _ in 0..4_800 {
                    let sample = callback.next();
                    assert!(sample.iter().all(|value| value.is_finite()));
                    audible |= sample[0].abs() > 0.1;
                }
                assert!(audible);
                let consumed = callback.renderer.playhead - start;
                let expected = u64::from(project_rate) / 10;
                let quantum = callback
                    .converter
                    .as_ref()
                    .map_or(1, |converter| converter.input_frames_next())
                    as u64;
                assert!(
                    // FFT conversion can prefetch two input blocks.
                    consumed.abs_diff(expected) <= quantum * 2,
                    "rate {project_rate}, buffer {buffer}: {consumed}"
                );
                assert!(!status.failed.load(Ordering::Relaxed));
            }
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
            status.publish_transport(&callback.renderer);
            assert!(!status.can_return_to_playback_start.load(Ordering::Relaxed));
            producer.push(Command::Play).unwrap();
            callback.update();
            for _ in 0..5000 {
                callback.next();
            }
            producer.push(Command::Pause).unwrap();
            callback.update();
            assert!(!callback.renderer.playing);
            status.publish_transport(&callback.renderer);
            assert!(!status.playing.load(Ordering::Relaxed));
            assert!(status.can_return_to_playback_start.load(Ordering::Relaxed));
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
            status.publish_transport(&callback.renderer);
            assert!(status.can_return_to_playback_start.load(Ordering::Relaxed));
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
            status.publish_transport(&callback.renderer);
            assert!(!status.can_return_to_playback_start.load(Ordering::Relaxed));
            assert_eq!(status.playhead.load(Ordering::Relaxed), 4800);
            producer.push(Command::Seek(0)).unwrap();
            callback.update();
            status.publish_transport(&callback.renderer);
            assert_eq!(status.playhead.load(Ordering::Relaxed), 0);
            assert!(!status.can_return_to_playback_start.load(Ordering::Relaxed));
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
        updated.inherit(&mut prepared);
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
    fn track_and_master_peak_hold_retain_maximum_until_reset() {
        let track = StereoMeter::default();
        let master = Status::default();
        for peaks in [
            [0.0; 2],
            [0.5, 0.25],
            [0.125, 0.25],
            [0.5, 2.0],
            [0.75, 0.5],
            [0.0; 2],
        ] {
            track.publish_peaks(peaks);
            master.meter.publish_peaks(peaks);
        }
        for held in [track.held_peak(), master.meter.held_peak()] {
            assert_eq!(held, 2.0);
        }
        assert!(
            track
                .peaks
                .iter()
                .chain(&master.meter.peaks)
                .all(|peak| peak.load(Ordering::Relaxed) == 0)
        );
        track.reset();
        assert_eq!(track.held_peak(), 0.0);
        assert_eq!(master.meter.held_peak(), 2.0);
        assert!(!track.is_clipped());
        assert!(master.meter.is_clipped());
        track.publish_peaks([0.125, 0.25]);
        master.meter.publish_peaks([0.125, 0.25]);
        assert_eq!(track.held_peak(), 0.25);
        assert_eq!(master.meter.held_peak(), 2.0);
        master.meter.reset();
        assert_eq!(track.held_peak(), 0.25);
        master.meter.publish_peaks([0.5, 0.125]);
        assert_eq!(master.meter.held_peak(), 0.5);
        assert!(!master.meter.is_clipped());
        assert!(
            master
                .meter
                .peaks
                .iter()
                .zip([0.5_f32, 0.125])
                .all(|(peak, expected)| peak.load(Ordering::Relaxed) == expected.to_bits())
        );
    }

    #[test]
    fn track_meter_latches_stereo_and_follows_track_identity() {
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
        meters[&first].publish_peaks([1.2, 0.5]);
        meters[&first].publish_peaks([0.0; 2]);
        assert!(meters[&first].is_clipped());
        assert!(!meters[&second].is_clipped());
        meters[&first].reset();
        assert!(!meters[&first].is_clipped());
        meters[&first].publish_peaks([0.5, 1.1]);
        assert!(meters[&first].is_clipped());

        plan.project.tracks.swap(0, 1);
        let updated = track_meters(&plan, &meters);
        assert!(Arc::ptr_eq(&updated[&first], &meters[&first]));
        assert_eq!(updated[&first].held_peak(), 1.1);
        assert_eq!(updated[&second].held_peak(), 0.0);
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

#[cfg(test)]
mod compressor_meter_tests {
    use super::*;

    #[test]
    fn effect_meter_handles_follow_ids_and_report_callback_compression() {
        for master in [false, true] {
            let mut plan = crate::tests::transport_plan();
            let mut effect = daw_core::EffectInstance::compressor();
            let daw_core::EffectSettings::Compressor(settings) = &mut effect.unit else {
                unreachable!()
            };
            settings.threshold_db = -36.0;
            settings.attack_ms = 0.1;
            let id = effect.id;
            effect_test_chain(&mut plan, master).push(effect.clone());
            let meters = effect_meters(&plan, &HashMap::new());
            let (producer, consumer) = RingBuffer::new(32);
            let (retire, _retired) = RingBuffer::new(64);
            let status = Arc::new(Status::default());
            let tracks = track_meters(&plan, &HashMap::new());
            let mut callback =
                Callback::new(plan.clone(), &tracks, consumer, retire, status, 48_000).unwrap();
            bind_effect_meters(&mut callback.renderer, &meters);
            let mut producer = producer;
            producer
                .push(Command::Play)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            for _ in 0..2000 {
                callback.next();
            }
            assert!(f32::from_bits(meters[&id].load(Ordering::Relaxed)) > 10.0);
            let mut other = effect;
            other.id = Id::new_v4();
            let second = other.id;
            effect_test_chain(&mut plan, master).insert(0, other);
            let updated = effect_meters(&plan, &meters);
            assert!(Arc::ptr_eq(&updated[&id], &meters[&id]));
            assert!(!Arc::ptr_eq(&updated[&second], &meters[&id]));
            effect_test_chain(&mut plan, master).retain(|effect| effect.id != id);
            let removed = effect_meters(&plan, &updated);
            assert!(!removed.contains_key(&id));
            assert!(Arc::ptr_eq(&removed[&second], &updated[&second]));
            producer
                .push(Command::Stop)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            assert_eq!(meters[&id].load(Ordering::Relaxed), 0);
        }
    }
}

#[cfg(test)]
mod band_meter_tests {
    use super::*;

    #[test]
    fn band_meter_handles_follow_ids_and_report_callback_levels_or_compression() {
        for (master, eq) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut plan = crate::tests::transport_plan();
            let mut effect = if eq {
                daw_core::EffectInstance::equalizer()
            } else {
                daw_core::EffectInstance::ott()
            };
            if let daw_core::EffectSettings::Ott(settings) = &mut effect.unit {
                settings.bands[0].attack_ms = 0.1;
            }
            let id = effect.id;
            effect_test_chain(&mut plan, master).push(effect.clone());
            let meters = band_meters(&plan, &HashMap::new());
            let (producer, consumer) = RingBuffer::new(32);
            let (retire, _retired) = RingBuffer::new(64);
            let status = Arc::new(Status::default());
            let tracks = track_meters(&plan, &HashMap::new());
            let mut callback =
                Callback::new(plan.clone(), &tracks, consumer, retire, status, 48_000).unwrap();
            bind_band_meters(&mut callback.renderer, &meters);
            let mut producer = producer;
            producer
                .push(Command::Play)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            for _ in 0..2000 {
                callback.next();
            }
            assert!(
                meters[&id]
                    .iter()
                    .any(|m| f32::from_bits(m.load(Ordering::Relaxed)).abs()
                        > if eq { 0.01 } else { 1.0 })
            );
            let mut other = effect;
            other.id = Id::new_v4();
            let second = other.id;
            effect_test_chain(&mut plan, master).insert(0, other);
            let updated = band_meters(&plan, &meters);
            assert!(Arc::ptr_eq(&updated[&id], &meters[&id]));
            assert!(!Arc::ptr_eq(&updated[&second], &meters[&id]));
            effect_test_chain(&mut plan, master).retain(|effect| effect.id != id);
            let removed = band_meters(&plan, &updated);
            assert!(!removed.contains_key(&id));
            assert!(Arc::ptr_eq(&removed[&second], &updated[&second]));
            producer
                .push(Command::Stop)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            assert!(
                meters[&id]
                    .iter()
                    .all(|m| f32::from_bits(m.load(Ordering::Relaxed)) == 0.0)
            );
        }
    }
}

#[cfg(test)]
fn effect_test_chain(plan: &mut RenderPlan, master: bool) -> &mut Vec<daw_core::EffectInstance> {
    if master {
        &mut plan.project.master.effects
    } else {
        &mut plan.project.tracks[0].effects
    }
}

#[cfg(test)]
mod modulation_meter_tests {
    use super::*;
    #[test]
    fn processed_parameter_snapshots_publish_without_advancing_dsp_and_follow_unit_identity() {
        let mut plan = crate::tests::transport_plan();
        let effect = daw_core::EffectInstance::auto_filter();
        let key = ModulationTarget::Effect(effect.id);
        plan.project.tracks[0].effects.push(effect);
        let drive = daw_core::EffectInstance::drive();
        let drive_key = ModulationTarget::Effect(drive.id);
        plan.project.tracks[0].effects.push(drive);
        let track = plan.project.add_midi_track().unwrap();
        let layer = &mut plan.project.tracks[1].instrument.as_mut().unwrap().layers[0];
        layer.amplitude.present = true;
        let amplitude = ModulationTarget::Amplitude {
            track,
            layer: layer.id,
        };
        layer.oscillators.push(Default::default());
        let oscillator = ModulationTarget::Oscillator {
            track,
            layer: layer.id,
            oscillator: layer.oscillators[0].id,
        };
        let handles = modulation_meters(&plan, &HashMap::new());
        assert_eq!(handles.len(), 4);
        assert!(f32::from_bits(handles[&key][0].load(Ordering::Relaxed)).is_nan());
        let tracks = track_meters(&plan, &HashMap::new());
        let mut renderer = PreparedRenderer::new(plan.clone(), &tracks);
        renderer.bind_modulation(&handles);
        renderer.play();
        for _ in 0..512 {
            renderer.next_sample();
        }
        let expected = renderer.modulation_values(key).unwrap();
        let frame = renderer.playhead;
        renderer.publish_track_peaks();
        renderer.publish_track_peaks();
        assert_eq!(renderer.playhead, frame);
        assert_eq!(expected, renderer.modulation_values(key).unwrap());
        assert_eq!(
            expected,
            std::array::from_fn(|i| f32::from_bits(handles[&key][i].load(Ordering::Relaxed)))
        );
        assert_eq!(
            f32::from_bits(handles[&amplitude][0].load(Ordering::Relaxed)),
            daw_core::SILENT_GAIN_DB
        );
        assert_eq!(
            renderer.modulation_values(drive_key).unwrap(),
            std::array::from_fn(|i| f32::from_bits(handles[&drive_key][i].load(Ordering::Relaxed)))
        );
        assert!(f32::from_bits(handles[&oscillator][0].load(Ordering::Relaxed)).is_nan());
        let next = modulation_meters(&plan, &handles);
        assert!(Arc::ptr_eq(&next[&oscillator], &handles[&oscillator]));
        assert!(Arc::ptr_eq(&next[&key], &handles[&key]));
        plan.project.tracks.reverse();
        assert!(Arc::ptr_eq(
            &modulation_meters(&plan, &handles)[&amplitude],
            &handles[&amplitude]
        ));
        plan.project.tracks[1].effects.clear();
        let removed = modulation_meters(&plan, &handles);
        assert!(!removed.contains_key(&key));
        assert!(removed.contains_key(&amplitude));
        assert!(Arc::ptr_eq(&removed[&oscillator], &handles[&oscillator]));
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0]
            .oscillators
            .clear();
        assert!(!modulation_meters(&plan, &handles).contains_key(&oscillator));
    }
}
