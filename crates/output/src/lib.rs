use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use daw_core::Id;
use daw_engine::{ModulationTarget, RenderPlan, Renderer, TransportState};
use rtrb::{Consumer, Producer, RingBuffer};
use rubato::Resampler;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

type ModulationMeter = Arc<[AtomicU32; daw_engine::MODULATION_SNAPSHOT_SIZE]>;

mod meters;
mod performance;
mod preparation;
#[cfg(test)]
mod preparation_tests;
pub use meters::StereoMeter;
pub use performance::{Performance, PerformanceSnapshot};

#[derive(Default)]
pub struct Status {
    pub performance: Performance,
    transport_sequence: AtomicU64,
    // Monotonic project samples rendered, independent of seeks and loop wraps.
    rendered_frames: AtomicU64,
    paused: AtomicBool,
    playback_start: AtomicU64,
    requested_revision: AtomicU64,
    applied_revision: AtomicU64,
    pub playhead: AtomicU64,
    pub playing: AtomicBool,
    pub can_return_to_playback_start: AtomicBool,
    pub failed: AtomicBool,
    pub meter: StereoMeter,
}
impl Status {
    fn publish_transport(&self, renderer: &Renderer) {
        self.transport_sequence.fetch_add(1, Ordering::AcqRel);
        let state = renderer.transport_state();
        self.paused.store(state.paused, Ordering::Relaxed);
        self.playback_start
            .store(state.playback_start.unwrap_or(0), Ordering::Relaxed);
        self.playhead.store(renderer.playhead, Ordering::Relaxed);
        self.playing.store(renderer.playing, Ordering::Relaxed);
        self.can_return_to_playback_start
            .store(renderer.can_return_to_playback_start(), Ordering::Relaxed);
        self.transport_sequence.fetch_add(1, Ordering::Release);
    }
    fn transport(&self) -> TransportState {
        loop {
            let sequence = self.transport_sequence.load(Ordering::Acquire);
            if !sequence.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            let state = TransportState {
                frame: self.playhead.load(Ordering::Relaxed),
                playing: self.playing.load(Ordering::Relaxed),
                paused: self.paused.load(Ordering::Relaxed),
                playback_start: self
                    .can_return_to_playback_start
                    .load(Ordering::Relaxed)
                    .then(|| self.playback_start.load(Ordering::Relaxed)),
            };
            if self.transport_sequence.load(Ordering::Acquire) == sequence {
                return state;
            }
        }
    }
}
// Prepared on the application thread and retired there. The callback only reads
// the existing meter handles and writes atomics; it does not allocate or lock.
struct PreparedRenderer {
    _memory: performance::RendererMemoryLease,
    renderer: Renderer,
    revision: u64,
    prepared_transport: bool,
    reprepare: Option<preparation::Anchor>,
    activate_at: Option<u64>,
    preparation_lead: u64,
    ready_at: Option<std::time::Instant>,
    requested_at: Option<std::time::Instant>,
    meters: Vec<Arc<StereoMeter>>,
    modulation: Vec<(ModulationTarget, ModulationMeter)>,
}
impl PreparedRenderer {
    #[cfg(test)]
    fn new(plan: RenderPlan, meters: &HashMap<Id, Arc<StereoMeter>>) -> Self {
        Self::try_new(plan, meters).expect("test renderer fits output budget")
    }
    fn try_new(plan: RenderPlan, meters: &HashMap<Id, Arc<StereoMeter>>) -> Result<Self, String> {
        let handles = plan
            .project
            .tracks
            .iter()
            .map(|t| meters[&t.id].clone())
            .collect();
        let renderer = Renderer::new(plan);
        let memory =
            performance::RendererMemoryLease::try_new(renderer.memory_usage().owned_bytes())?;
        Ok(Self {
            _memory: memory,
            renderer,
            revision: 0,
            prepared_transport: false,
            reprepare: None,
            activate_at: None,
            preparation_lead: 0,
            ready_at: None,
            requested_at: None,
            meters: handles,
            modulation: Vec::new(),
        })
    }
    fn bind_modulation(&mut self, meters: &HashMap<ModulationTarget, ModulationMeter>) {
        self.modulation = meters
            .iter()
            .map(|(key, meter)| (*key, meter.clone()))
            .collect();
    }
    fn publish_track_peaks(&self) {
        for (key, meter) in &self.modulation {
            let values = self
                .renderer
                .modulation_snapshot(*key)
                .unwrap_or([f32::NAN; daw_engine::MODULATION_SNAPSHOT_SIZE]);
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
    previous: &HashMap<ModulationTarget, ModulationMeter>,
) -> HashMap<ModulationTarget, ModulationMeter> {
    let effects = plan
        .project
        .all_effects()
        .filter(|e| {
            matches!(
                e.unit,
                daw_core::EffectSettings::AutoFilter(_)
                    | daw_core::EffectSettings::Drive(_)
                    | daw_core::EffectSettings::Eq3(_)
                    | daw_core::EffectSettings::Compressor(_)
                    | daw_core::EffectSettings::Limiter(_)
                    | daw_core::EffectSettings::DirectionMixer(_)
                    | daw_core::EffectSettings::StereoSpread(_)
                    | daw_core::EffectSettings::Saturator(_)
                    | daw_core::EffectSettings::Bitcrusher(_)
                    | daw_core::EffectSettings::Ott(_)
            )
        })
        .map(|e| ModulationTarget::Effect(e.id));
    let samplers = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.layers
                .iter()
                .filter(|l| l.sampler.is_some())
                .map(move |l| ModulationTarget::Sampler {
                    track: track.id,
                    layer: l.id,
                })
        })
    });
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
    let envelopes = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.envelope_destinations()
                .filter(|(_, e)| e.present)
                .map(move |(id, _)| ModulationTarget::Envelope {
                    track: track.id,
                    layer: id.layer,
                    unit: id.unit,
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
        .chain(envelopes)
        .chain(amplitudes)
        .chain(samplers)
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
    PauseAt(u64),
    // Stop the active audio now without marking the pending prepared plan as applied.
    PausePending(u64),
    ResumeAt(u64),
    #[cfg(test)]
    Play,
    #[cfg(test)]
    Pause,
    #[cfg(test)]
    Stop,
    #[cfg(test)]
    Seek(u64),
}
type BuiltStream = (
    cpal::Stream,
    Producer<Command>,
    Consumer<Box<PreparedRenderer>>,
);
pub struct AudioOutput {
    plan: Arc<RenderPlan>,
    desired_transport: Option<(u64, TransportState)>,
    project_rate_hz: u32,
    buffer_size_frames: u32,
    _stream: cpal::Stream,
    preparation: preparation::Worker,
    prepare_revision: u64,
    prepared: Option<preparation::Ready>,
    commands: Producer<Command>,
    retired: Consumer<Box<PreparedRenderer>>,
    pub status: Arc<Status>,
    pub description: String,
    track_meters: HashMap<Id, Arc<StereoMeter>>,
    effect_meters: HashMap<Id, Arc<AtomicU32>>,
    band_meters: HashMap<Id, Arc<[AtomicU32; 3]>>,
    modulation_meters: HashMap<ModulationTarget, ModulationMeter>,
}
/// Device-independent, worker-owned seek preparation. It can start after project
/// load and transfer to device output without discarding completed checkpoints.
pub struct AudioPreparation {
    worker: preparation::Worker,
    status: Arc<Status>,
}
impl AudioPreparation {
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        let status = Arc::new(Status::default());
        let worker = preparation::Worker::new(status.clone())?;
        worker.prewarm(Arc::new(plan));
        Ok(Self { worker, status })
    }
    pub fn performance(&self) -> PerformanceSnapshot {
        self.status.performance.snapshot()
    }
    pub fn update_plan(&self, plan: RenderPlan) {
        self.worker.prewarm(Arc::new(plan));
    }
}
impl AudioOutput {
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        Self::with_preparation(plan, None)
    }
    pub fn with_preparation(
        plan: RenderPlan,
        preparation: Option<AudioPreparation>,
    ) -> Result<Self, String> {
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
        let preparation = match preparation {
            Some(p) => p,
            None => AudioPreparation::new(plan.clone())?,
        };
        let status = preparation.status.clone();
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
        status
            .playhead
            .store(plan.project.transport.playhead_frame, Ordering::Relaxed);
        Ok(Self {
            plan: Arc::new(plan),
            desired_transport: None,
            project_rate_hz,
            buffer_size_frames,
            _stream: stream,
            preparation: preparation.worker,
            prepare_revision: 0,
            prepared: None,
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
    fn bindings(&self) -> preparation::Bindings {
        preparation::Bindings {
            tracks: self.track_meters.clone(),
            effects: self.effect_meters.clone(),
            bands: self.band_meters.clone(),
            modulation: self.modulation_meters.clone(),
        }
    }
    pub fn take_preparation_error(&mut self) -> Option<String> {
        let error = self.preparation.take_error()?;
        self.desired_transport = None;
        self.prepare_revision = self.status.applied_revision.load(Ordering::Acquire);
        self.status
            .requested_revision
            .store(self.prepare_revision, Ordering::Release);
        Some(error)
    }
    /// Latest requested transport until activation; useful for pending Play/Pause/Seek UI.
    pub fn transport_intent(&self) -> TransportState {
        self.current_transport()
    }
    /// Keep the displayed playhead on the audible transport until a playing seek activates.
    pub fn displayed_frame(&self) -> u64 {
        displayed_frame(self.current_transport(), self.status.transport())
    }
    fn current_transport(&self) -> TransportState {
        self.desired_transport
            .filter(|(revision, _)| {
                *revision > self.status.applied_revision.load(Ordering::Acquire)
            })
            .map_or_else(|| self.status.transport(), |(_, state)| state)
    }
    fn prepare(&mut self, state: Option<TransportState>) {
        self.prepare_revision = self
            .preparation
            .submit(self.plan.clone(), self.bindings(), state);
        self.status
            .requested_revision
            .store(self.prepare_revision, Ordering::Release);
        self.desired_transport = state.map(|state| (self.prepare_revision, state));
        self.prepared = None;
    }
    pub fn sync(&mut self, plan: RenderPlan) -> Result<(), String> {
        if plan.project.sample_rate_hz != self.project_rate_hz
            || plan.project.buffer_size_frames != self.buffer_size_frames
        {
            return Err("Recreate output after changing project audio settings".into());
        }
        self.drain();
        let transport = self
            .desired_transport
            .filter(|(r, _)| *r > self.status.applied_revision.load(Ordering::Acquire))
            .map(|(_, s)| s);
        self.plan = Arc::new(plan);
        self.prepare(transport);
        Ok(())
    }
    /// Register once after the UI creates output; worker completion wakes idle painting.
    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.preparation.set_waker(wake);
    }
    pub fn track_meter(&self, id: Id) -> Option<&StereoMeter> {
        self.track_meters.get(&id).map(AsRef::as_ref)
    }
    /// Display-only block snapshots. No synthetic animation when playback is stopped.
    pub fn modulation_values(&self, key: ModulationTarget) -> Option<[f32; 5]> {
        self.modulation_snapshot(key)
            .map(|v| std::array::from_fn(|i| v[i]))
    }
    pub fn modulation_snapshot(
        &self,
        key: ModulationTarget,
    ) -> Option<daw_engine::ModulationSnapshot> {
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
        self.drain();
        let mut state = self.current_transport();
        if state.playing {
            return Ok(());
        }
        let resume = state.paused && !self.preparing();
        if !state.paused {
            state.playback_start = Some(state.frame);
        }
        state.playing = true;
        state.paused = false;
        if resume {
            self.immediate(state, true)
        } else {
            self.prepare(Some(state));
            Ok(())
        }
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        state.paused |= state.playing;
        state.playing = false;
        if self.preparing() {
            if self.commands.is_full() {
                return Err("Audio command queue is full; try again".into());
            }
            self.prepare(Some(state));
            self.send(Command::PausePending(self.prepare_revision))
        } else {
            self.immediate(state, false)
        }
    }
    fn immediate(&mut self, state: TransportState, resume: bool) -> Result<(), String> {
        if self.commands.is_full() {
            return Err("Audio command queue is full; try again".into());
        }
        let revision = self.preparation.cancel();
        self.prepare_revision = revision;
        self.status
            .requested_revision
            .store(revision, Ordering::Release);
        self.desired_transport = Some((revision, state));
        self.send(if resume {
            Command::ResumeAt(revision)
        } else {
            Command::PauseAt(revision)
        })
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        state.frame = state.playback_start.take().unwrap_or(state.frame);
        state.playing = false;
        state.paused = false;
        self.prepare(Some(state));
        Ok(())
    }
    pub fn seek(&mut self, frame: u64) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        state.frame = frame;
        self.prepare(Some(state));
        Ok(())
    }
    pub fn preparing(&self) -> bool {
        self.prepare_revision != self.status.applied_revision.load(Ordering::Acquire)
    }
    pub fn drain(&mut self) {
        while let Ok(mut old) = self.retired.pop() {
            if old.revision == self.prepare_revision
                && let Some(transport) = old.reprepare.take()
            {
                self.preparation.reprepare(
                    preparation::Ready {
                        revision: old.revision,
                        renderer: old,
                        bindings: self.bindings(),
                    },
                    transport,
                );
            } else {
                drop(old);
            }
        }
        if let Some(ready) = self.preparation.take()
            && ready.revision == self.prepare_revision
        {
            self.prepared = Some(ready);
        }
        if let Some(mut ready) = self.prepared.take() {
            match self.commands.push(Command::Plan(ready.renderer)) {
                Ok(()) => {
                    self.track_meters = ready.bindings.tracks;
                    self.effect_meters = ready.bindings.effects;
                    self.band_meters = ready.bindings.bands;
                    self.modulation_meters = ready.bindings.modulation;
                }
                Err(rtrb::PushError::Full(Command::Plan(renderer))) => {
                    ready.renderer = renderer;
                    self.prepared = Some(ready);
                }
                Err(_) => unreachable!("only a prepared plan was submitted"),
            }
        }
    }
}
struct Callback {
    renderer: Box<PreparedRenderer>,
    commands: Consumer<Command>,
    retired: Producer<Box<PreparedRenderer>>,
    pending: Option<Box<PreparedRenderer>>,
    scheduled: Option<Box<PreparedRenderer>>,
    rendered_frames: u64,
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
            renderer: Box::new(PreparedRenderer::try_new(plan, meters)?),
            commands,
            retired,
            pending: None,
            scheduled: None,
            rendered_frames: 0,
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
        let started = std::time::Instant::now();
        self.activate_scheduled();
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
                    if next.revision != self.status.requested_revision.load(Ordering::Acquire) {
                        let _ = self.retired.push(next);
                        continue;
                    }
                    if let Some(scheduled) = self.scheduled.take() {
                        let _ = self.retired.push(scheduled);
                        if self.retired.is_full() {
                            self.pending = Some(next);
                            break;
                        }
                    }
                    if next.activate_at.is_some() {
                        self.scheduled = Some(next);
                        self.activate_scheduled();
                        continue;
                    }
                    if next.prepared_transport {
                        next.activate_transport(&self.renderer);
                        self.discard_buffered_output();
                    } else if next.can_inherit_without_preroll(&self.renderer) {
                        next.inherit(&mut self.renderer);
                    } else {
                        // Rebuild at this exact clock, then catch up on the worker.
                        // The current renderer keeps playing throughout preparation.
                        next.reprepare = Some(self.preparation_anchor());
                        let _ = self.retired.push(next);
                        continue;
                    }
                    self.install(next);
                }
                Command::PausePending(revision) => {
                    if revision == self.status.requested_revision.load(Ordering::Acquire) {
                        self.renderer.pause();
                        self.discard_buffered_output();
                        self.status.publish_transport(&self.renderer);
                    }
                }
                Command::PauseAt(revision) | Command::ResumeAt(revision) => {
                    if revision == self.status.requested_revision.load(Ordering::Acquire) {
                        if matches!(command, Command::ResumeAt(_)) {
                            self.renderer.resume();
                        } else {
                            self.renderer.pause();
                        }
                        self.discard_buffered_output();
                        self.status.publish_transport(&self.renderer);
                        self.status
                            .applied_revision
                            .store(revision, Ordering::Release);
                    }
                }
                #[cfg(test)]
                Command::Play => {
                    self.renderer.play();
                    self.discard_buffered_output();
                }
                #[cfg(test)]
                Command::Pause => {
                    self.renderer.pause();
                    self.discard_buffered_output();
                }
                #[cfg(test)]
                Command::Stop => {
                    self.renderer.stop();
                    self.discard_buffered_output();
                }
                #[cfg(test)]
                Command::Seek(frame) => {
                    self.renderer.seek(frame);
                    self.discard_buffered_output();
                }
            }
        }
        self.status
            .performance
            .commands(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
    }
    fn preparation_anchor(&self) -> preparation::Anchor {
        preparation::Anchor {
            transport: self.renderer.transport_state(),
            clock: self.rendered_frames,
        }
    }
    fn install(&mut self, next: Box<PreparedRenderer>) {
        if let Some(requested_at) = next.requested_at {
            self.status
                .performance
                .activation(requested_at.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        }
        if let Some(ready_at) = next.ready_at {
            self.status
                .performance
                .prepared_wait(ready_at.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        }
        let revision = next.revision;
        let old = std::mem::replace(&mut self.renderer, next);
        self.status.publish_transport(&self.renderer);
        self.status
            .applied_revision
            .store(revision, Ordering::Release);
        // The single producer checks capacity before transferring either owner.
        let _ = self.retired.push(old);
    }
    fn activate_scheduled(&mut self) {
        let Some(next) = &self.scheduled else { return };
        let stale = next.revision != self.status.requested_revision.load(Ordering::Acquire);
        let at = next
            .activate_at
            .expect("scheduled renderer has a sample clock");
        if (!stale && at > self.rendered_frames) || self.retired.is_full() {
            return;
        }
        let mut next = self.scheduled.take().unwrap();
        if stale {
            let _ = self.retired.push(next);
        } else if at < self.rendered_frames {
            // UI/queue delays can miss a deadline. Retry with more delivery headroom;
            // never rewind the active transport or replay DSP on this thread.
            next.preparation_lead = next
                .preparation_lead
                .saturating_mul(2)
                .min(u64::from(next.plan.project.sample_rate_hz));
            next.reprepare = Some(self.preparation_anchor());
            let _ = self.retired.push(next);
        } else {
            next.activate_transport(&self.renderer);
            self.install(next);
        }
    }
    fn next_project_sample(&mut self) -> [f32; 2] {
        self.activate_scheduled();
        let sample = self.renderer.next_sample();
        self.rendered_frames += 1;
        sample
    }
    fn discard_buffered_output(&mut self) {
        // Discard cached samples from before the command. Keep filter history so
        // the renderer's transport fade remains continuous through resampling.
        self.index = self.output_count;
    }
    fn next(&mut self) -> [f32; 2] {
        if self.converter.is_some() {
            while self.index >= self.output_count {
                self.rendered_input = true;
                let count = self.converter.as_ref().unwrap().input_frames_next();
                for i in 0..count {
                    let s = self.next_project_sample();
                    self.input[0][i] = s[0];
                    self.input[1][i] = s[1];
                }
                match self.converter.as_mut().unwrap().process_into_buffer(
                    &self.input,
                    &mut self.output,
                    None,
                ) {
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
            self.next_project_sample()
        }
    }
}
fn displayed_frame(intent: TransportState, active: TransportState) -> u64 {
    if intent.playing && active.playing {
        active.frame
    } else {
        intent.frame
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
    let device_rate = config.sample_rate.0;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let started = std::time::Instant::now();
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
                callback
                    .status
                    .rendered_frames
                    .store(callback.rendered_frames, Ordering::Release);
                callback.status.publish_transport(&callback.renderer);
                callback.status.performance.callback(
                    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                    data.len() / channels,
                    device_rate,
                );
            },
            move |_| {
                status.performance.device_failure();
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
            sampler_audio: Default::default(),
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
            sampler_audio: Default::default(),
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
                sampler_audio: Default::default(),
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
    fn all_effect_meters_clear_on_bypass_and_resume_after_compatible_updates() {
        for master in [false, true] {
            for make in [
                daw_core::EffectInstance::equalizer,
                daw_core::EffectInstance::compressor,
                daw_core::EffectInstance::limiter,
                daw_core::EffectInstance::ott,
            ] {
                let mut plan = crate::tests::transport_plan();
                let mut effect = make();
                match &mut effect.unit {
                    daw_core::EffectSettings::Compressor(s) => {
                        s.threshold_db = -36.0;
                        s.attack_ms = 0.1;
                    }
                    daw_core::EffectSettings::Limiter(s) => s.input_db = 24.0,
                    _ => {}
                }
                effect_test_chain(&mut plan, master).push(effect);
                let scalar = effect_meters(&plan, &HashMap::new());
                let bands = band_meters(&plan, &HashMap::new());
                let cells: Vec<_> = scalar
                    .values()
                    .map(AsRef::as_ref)
                    .chain(bands.values().flat_map(|m| m.iter()))
                    .collect();
                let tracks = track_meters(&plan, &HashMap::new());
                let mut renderer = PreparedRenderer::new(plan.clone(), &tracks);
                bind_effect_meters(&mut renderer, &scalar);
                bind_band_meters(&mut renderer, &bands);
                renderer.play();
                for _ in 0..2000 {
                    renderer.next_sample();
                }
                assert!(cells.iter().any(|cell| cell.load(Ordering::Relaxed) != 0));
                for enabled in [false, true, false, true] {
                    match &mut effect_test_chain(&mut plan, master)[0].unit {
                        daw_core::EffectSettings::Eq3(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Compressor(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Limiter(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Ott(s) => s.enabled = enabled,
                        _ => unreachable!(),
                    }
                    let mut next = PreparedRenderer::new(plan.clone(), &tracks);
                    bind_effect_meters(&mut next, &scalar);
                    bind_band_meters(&mut next, &bands);
                    next.inherit(&mut renderer);
                    renderer = next;
                    if !enabled {
                        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
                        // No writes while bypassed, even as audio and detector history advance.
                        for cell in &cells {
                            cell.store(123, Ordering::Relaxed);
                        }
                    }
                    for _ in 0..2000 {
                        renderer.next_sample();
                    }
                    if enabled {
                        assert!(cells.iter().any(|cell| cell.load(Ordering::Relaxed) != 0));
                    } else {
                        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 123));
                        // Restore a valid zero reading before preparing the next renderer.
                        for cell in &cells {
                            cell.store(0, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }

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
        let mut eq = daw_core::EffectInstance::equalizer();
        let eq_key = ModulationTarget::Effect(eq.id);
        if let daw_core::EffectSettings::Eq3(s) = &mut eq.unit {
            s.bands[2].q = 2.4;
        }
        plan.project.tracks[0].effects.push(eq);
        let mut compressor = daw_core::EffectInstance::compressor();
        let compressor_key = ModulationTarget::Effect(compressor.id);
        if let daw_core::EffectSettings::Compressor(s) = &mut compressor.unit {
            s.dry_wet = 0.4;
        }
        plan.project.tracks[0].effects.push(compressor);
        let mut ott = daw_core::EffectInstance::ott();
        let ott_key = ModulationTarget::Effect(ott.id);
        if let daw_core::EffectSettings::Ott(s) = &mut ott.unit {
            s.bands[2].output_db = 3.0;
        }
        plan.project.tracks[0].effects.push(ott);
        let mut limiter = daw_core::EffectInstance::limiter();
        let limiter_key = ModulationTarget::Effect(limiter.id);
        if let daw_core::EffectSettings::Limiter(s) = &mut limiter.unit {
            s.lookahead_ms = 2.3;
        }
        plan.project.tracks[0].effects.push(limiter);
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
        let sample_layer = daw_core::InstrumentLayerSettings {
            sampler: Some(Default::default()),
            ..Default::default()
        };
        let sampler = ModulationTarget::Sampler {
            track,
            layer: sample_layer.id,
        };
        plan.project.tracks[1]
            .instrument
            .as_mut()
            .unwrap()
            .layers
            .push(sample_layer);
        let mut direction = daw_core::EffectInstance::direction_mixer();
        let direction_key = ModulationTarget::Effect(direction.id);
        if let daw_core::EffectSettings::DirectionMixer(s) = &mut direction.unit {
            s.direction_degrees = 45.0;
            s.spread = 0.5;
        }
        plan.project.tracks[0].effects.push(direction);
        let mut spread = daw_core::EffectInstance::stereo_spread();
        let spread_key = ModulationTarget::Effect(spread.id);
        if let daw_core::EffectSettings::StereoSpread(s) = &mut spread.unit {
            s.lower_intensity = 0.2;
            s.upper_intensity = 0.8;
            s.lower_frequency_hz = 400.0;
            s.upper_frequency_hz = 10000.0;
        }
        plan.project.tracks[0].effects.push(spread);
        let mut crusher = daw_core::EffectInstance::bitcrusher();
        let crusher_key = ModulationTarget::Effect(crusher.id);
        if let daw_core::EffectSettings::Bitcrusher(s) = &mut crusher.unit {
            s.drive_db = 6.0;
            s.resolution_bits = 8;
            s.downsampling = 7;
            s.level_db = -6.0;
            s.dry_wet = 0.75;
        }
        plan.project.tracks[0].effects.push(crusher);
        let handles = modulation_meters(&plan, &HashMap::new());
        assert_eq!(handles.len(), 12);
        assert!(f32::from_bits(handles[&key][0].load(Ordering::Relaxed)).is_nan());
        let tracks = track_meters(&plan, &HashMap::new());
        let mut renderer = PreparedRenderer::new(plan.clone(), &tracks);
        renderer.bind_modulation(&handles);
        renderer.play();
        for _ in 0..512 {
            renderer.next_sample();
        }
        assert_eq!(
            &renderer.modulation_snapshot(direction_key).unwrap()[..2],
            &[45.0, 0.5]
        );
        let crusher = renderer.modulation_snapshot(crusher_key).unwrap();
        for (actual, expected) in crusher[..5].iter().zip([6.0, 8.0, 7.0, -6.0, 0.75]) {
            assert!((actual - expected).abs() < 1e-5);
        }
        assert!(crusher[5..].iter().all(|v| v.is_nan()));
        let spread = renderer.modulation_snapshot(spread_key).unwrap();
        for (actual, expected) in spread[..4].iter().zip([0.2, 0.8, 400.0, 10000.0]) {
            assert!((actual - expected).abs() < 0.02);
        }
        assert!(spread[4..].iter().all(|v| v.is_nan()));
        let expected = renderer.modulation_snapshot(key).unwrap();
        let frame = renderer.playhead;
        renderer.publish_track_peaks();
        renderer.publish_track_peaks();
        assert_eq!(renderer.playhead, frame);
        assert_eq!(
            expected.map(f32::to_bits),
            renderer.modulation_snapshot(key).unwrap().map(f32::to_bits)
        );
        assert_eq!(
            expected.map(f32::to_bits),
            std::array::from_fn(|i| handles[&key][i].load(Ordering::Relaxed))
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
        let eq = renderer.modulation_snapshot(eq_key).unwrap();
        assert!(eq[..7].iter().all(|v| v.is_finite()));
        assert!(eq[7..].iter().all(|v| v.is_nan()));
        assert_eq!(eq[6], 2.4);
        assert_eq!(
            eq.map(f32::to_bits),
            std::array::from_fn(|i| handles[&eq_key][i].load(Ordering::Relaxed))
        );
        let compressor = renderer.modulation_snapshot(compressor_key).unwrap();
        assert!(compressor[..7].iter().all(|v| v.is_finite()));
        assert!(compressor[7..].iter().all(|v| v.is_nan()));
        assert_eq!(compressor[6], 0.4);
        assert_eq!(
            compressor.map(f32::to_bits),
            std::array::from_fn(|i| handles[&compressor_key][i].load(Ordering::Relaxed))
        );
        let reading = renderer.modulation_snapshot(sampler).unwrap();
        assert!(reading.iter().all(|v| v.is_nan()));
        assert_eq!(
            reading.map(f32::to_bits),
            std::array::from_fn(|i| handles[&sampler][i].load(Ordering::Relaxed))
        );
        let limiter = renderer.modulation_snapshot(limiter_key).unwrap();
        assert!(limiter[..4].iter().all(|v| v.is_finite()));
        assert!(limiter[4..].iter().all(|v| v.is_nan()));
        assert_eq!(limiter[2], 2.3);
        assert_eq!(
            limiter.map(f32::to_bits),
            std::array::from_fn(|i| handles[&limiter_key][i].load(Ordering::Relaxed))
        );
        let ott = renderer.modulation_snapshot(ott_key).unwrap();
        assert!(ott.iter().all(|v| v.is_finite()));
        assert_eq!(ott[14], 3.0);
        assert_eq!(
            ott.map(f32::to_bits),
            std::array::from_fn(|i| handles[&ott_key][i].load(Ordering::Relaxed))
        );
        let next = modulation_meters(&plan, &handles);
        assert!(Arc::ptr_eq(&next[&ott_key], &handles[&ott_key]));
        assert!(Arc::ptr_eq(&next[&sampler], &handles[&sampler]));
        assert!(Arc::ptr_eq(&next[&limiter_key], &handles[&limiter_key]));
        assert!(Arc::ptr_eq(
            &next[&compressor_key],
            &handles[&compressor_key]
        ));
        assert!(Arc::ptr_eq(&next[&eq_key], &handles[&eq_key]));
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
        assert!(!removed.contains_key(&eq_key));
        assert!(!removed.contains_key(&compressor_key));
        assert!(!removed.contains_key(&limiter_key));
        assert!(removed.contains_key(&amplitude));
        assert!(Arc::ptr_eq(&removed[&oscillator], &handles[&oscillator]));
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0]
            .oscillators
            .clear();
        assert!(!modulation_meters(&plan, &handles).contains_key(&oscillator));
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1].sampler = None;
        assert!(!modulation_meters(&plan, &handles).contains_key(&sampler));
    }
}
