mod midi_input;
pub use midi_input::{MidiAction, MidiReceipt, MidiSender};
mod audition;
mod audition_stream;
mod effect_spectrum;
pub use audition_stream::{AuditionStream, AuditionStreamWriter, audition_stream};
mod cache_schedule;
mod devices;
#[cfg(target_os = "windows")]
mod wasapi;
use cpal::traits::{DeviceTrait, StreamTrait};
use daw_core::Id;
use daw_engine::{MidiOutputSaturatorEdit, ModulationTarget, RenderPlan, Renderer, TransportState};
pub use devices::{
    AudioSettings, Configuration, DeviceId, DeviceInfo, Devices, SampleFormat, discover_devices,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackSeek {
    Exact,
    #[default]
    #[serde(other)]
    Fast,
}
impl PlaybackSeek {
    pub const ALL: [Self; 2] = [Self::Fast, Self::Exact];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Exact => "Exact",
        }
    }
}
use rtrb::{Consumer, Producer, RingBuffer};
use rubato::Resampler;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

type ModulationMeter = Arc<ModulationReading>;

struct ModulationReading {
    values: [AtomicU32; daw_engine::MODULATION_SNAPSHOT_SIZE],
    // The transport can change before the next callback publishes its readings.
    playing: AtomicBool,
}
impl ModulationReading {
    fn read(&self, playing: bool, envelope: bool) -> Option<daw_engine::ModulationSnapshot> {
        let eligible = if envelope {
            self.playing.load(Ordering::Acquire) == playing
        } else {
            playing
        };
        if !eligible {
            return None;
        }
        let values =
            std::array::from_fn(|i| f32::from_bits(self.values[i].load(Ordering::Relaxed)));
        values[0].is_finite().then_some(values)
    }
}

mod effect_audition;
#[cfg(test)]
mod effect_update_tests;
mod effect_updates;
mod layer_meters;
mod meters;
mod metronome;
mod performance;
mod preparation;
#[cfg(test)]
mod preparation_tests;
mod source_meters;
pub use meters::StereoMeter;
pub use performance::{Performance, PerformanceSnapshot};

#[derive(Default)]
pub struct Status {
    midi: midi_input::Port,
    pub performance: Performance,
    metronome_enabled: AtomicBool,
    transport_sequence: AtomicU64,
    // Monotonic project samples rendered, independent of seeks and loop wraps.
    rendered_frames: AtomicU64,
    paused: AtomicBool,
    playback_start: AtomicU64,
    requested_revision: AtomicU64,
    applied_revision: AtomicU64,
    processing_revision: AtomicU64,
    processing_publication: AtomicU64,
    processing_bytes: AtomicU64,
    pub playhead: AtomicU64,
    pub playing: AtomicBool,
    pub can_return_to_playback_start: AtomicBool,
    pub failed: AtomicBool,
    pub source_failed: AtomicBool,
    pub buffering: AtomicBool,
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
// Prepared and retired outside the callback. The callback only reads
// the existing meter handles and writes atomics; it does not allocate or lock.
struct PreparedRenderer {
    _memory: performance::RendererMemoryLease,
    renderer: Renderer,
    revision: u64,
    publication: u64,
    prepared_transport: bool,
    live_effect: Option<LiveEffect>,
    reprepare: Option<preparation::Anchor>,
    allow_checkpoint_reuse: bool,
    activate_at: Option<u64>,
    preparation_lead: u64,
    ready_at: Option<std::time::Instant>,
    requested_at: Option<std::time::Instant>,
    meters: Vec<Arc<StereoMeter>>,
    layer_meters: layer_meters::Layers,
    modulation: Vec<(ModulationTarget, ModulationMeter)>,
}
#[derive(Clone, Copy)]
struct LiveEffect {
    base_publication: u64,
    kind: LiveEffectKind,
}
#[derive(Clone, Copy)]
enum LiveEffectKind {
    Hosted(Id),
    Builtin(Id),
    Additions,
    Remove(Option<Id>),
    Move,
    UnverifiedRemoval,
}
impl PreparedRenderer {
    fn needs_native_boundary(&self) -> bool {
        self.live_effect.is_some_and(|effect| {
            matches!(
                effect.kind,
                LiveEffectKind::Remove(_) | LiveEffectKind::Move
            )
        })
    }
    #[cfg(test)]
    fn new(plan: RenderPlan, meters: &HashMap<Id, Arc<StereoMeter>>) -> Self {
        Self::try_new(plan, meters).expect("test renderer fits output budget")
    }
    fn try_new(plan: RenderPlan, meters: &HashMap<Id, Arc<StereoMeter>>) -> Result<Self, String> {
        Self::try_new_with_insert(plan, meters, None)
    }
    fn try_new_with_insert(
        plan: RenderPlan,
        meters: &HashMap<Id, Arc<StereoMeter>>,
        insert: Option<LiveEffect>,
    ) -> Result<Self, String> {
        let handles = plan
            .project
            .tracks
            .iter()
            .map(|t| {
                meters
                    .get(&t.id)
                    .cloned()
                    .ok_or_else(|| format!("Missing output meter binding for track {}", t.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut renderer = match insert.map(|insert| insert.kind) {
            Some(LiveEffectKind::Builtin(effect)) => {
                Renderer::try_new_for_builtin_insert(plan, effect)
            }
            Some(LiveEffectKind::Hosted(unit)) => Renderer::try_new_for_hosted_insert(plan, unit),
            Some(LiveEffectKind::Additions) => Renderer::try_new_for_effect_additions(plan),
            _ => Renderer::try_new(plan),
        }
        .map_err(|e| e.to_string())?;
        renderer.prepare_live_midi();
        renderer.prepare_streaming()?;
        let memory =
            performance::RendererMemoryLease::try_new(renderer.memory_usage().owned_bytes())?;
        Ok(Self {
            _memory: memory,
            renderer,
            revision: 0,
            publication: 0,
            prepared_transport: false,
            live_effect: insert,
            reprepare: None,
            allow_checkpoint_reuse: false,
            activate_at: None,
            preparation_lead: 0,
            ready_at: None,
            requested_at: None,
            meters: handles,
            layer_meters: Default::default(),
            modulation: Vec::new(),
        })
    }
    fn bind_modulation(&mut self, meters: &HashMap<ModulationTarget, ModulationMeter>) {
        self.modulation = meters
            .iter()
            .map(|(key, meter)| (*key, meter.clone()))
            .collect();
    }
    fn publish_track_peaks(&self, release: f32) {
        for (key, peaks) in self.renderer.layer_peaks() {
            if let Some(meter) = self.layer_meters.get(&key) {
                meter.publish_block_peaks(peaks, release);
            }
        }
        for (key, meter) in &self.modulation {
            let reading =
                if !self.renderer.playing && matches!(key, ModulationTarget::Envelope { .. }) {
                    self.renderer.live_envelope_snapshot(*key)
                } else {
                    self.renderer.modulation_snapshot(*key)
                };
            let values = reading.unwrap_or([f32::NAN; daw_engine::MODULATION_SNAPSHOT_SIZE]);
            for (cell, value) in meter.values.iter().zip(values) {
                cell.store(value.to_bits(), Ordering::Relaxed);
            }
            meter
                .playing
                .store(self.renderer.playing, Ordering::Release);
        }
        for (meter, peaks) in self.meters.iter().zip(self.renderer.track_peaks()) {
            meter.publish_block_peaks(peaks, release);
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
    previous: &HashMap<Id, Arc<daw_unit_core::PeakMeter>>,
) -> HashMap<Id, Arc<daw_unit_core::PeakMeter>> {
    plan.project
        .all_effects()
        .filter(|effect| {
            matches!(
                effect.unit,
                daw_core::EffectSettings::HarmonicLift(_)
                    | daw_core::EffectSettings::Compressor(_)
                    | daw_core::EffectSettings::Limiter(_)
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
fn bind_effect_meters(
    renderer: &mut Renderer,
    meters: &HashMap<Id, Arc<daw_unit_core::PeakMeter>>,
) {
    for (id, meter) in meters {
        renderer.set_effect_peak_meter(*id, meter.clone());
    }
}
// Dynamics have separate output and reduction/band readings. Keep their handles independent.
fn loudness_meters(
    plan: &RenderPlan,
    previous: &HashMap<Id, Arc<daw_unit_core::LoudnessMeter>>,
) -> HashMap<Id, Arc<daw_unit_core::LoudnessMeter>> {
    plan.project
        .all_effects()
        .filter(|e| matches!(e.unit, daw_core::EffectSettings::Loudness(_)))
        .map(|e| (e.id, previous.get(&e.id).cloned().unwrap_or_default()))
        .collect()
}
fn bind_loudness_meters(
    renderer: &mut Renderer,
    meters: &HashMap<Id, Arc<daw_unit_core::LoudnessMeter>>,
) {
    for (id, meter) in meters {
        renderer.set_loudness_meter(*id, meter.clone());
    }
}
fn output_meters(
    plan: &RenderPlan,
    previous: &HashMap<Id, Arc<daw_unit_core::BandMeter<3>>>,
) -> HashMap<Id, Arc<daw_unit_core::BandMeter<3>>> {
    plan.project
        .all_effects()
        .filter(|e| {
            matches!(
                e.unit,
                daw_core::EffectSettings::Compressor(_)
                    | daw_core::EffectSettings::Limiter(_)
                    | daw_core::EffectSettings::Ott(_)
                    | daw_core::EffectSettings::Loudness(_)
                    | daw_core::EffectSettings::Eq3(_)
                    | daw_core::EffectSettings::DirectionMixer(_)
                    | daw_core::EffectSettings::StereoSpread(_)
            )
        })
        .map(|e| (e.id, previous.get(&e.id).cloned().unwrap_or_default()))
        .collect()
}
fn bind_output_meters(
    renderer: &mut Renderer,
    meters: &HashMap<Id, Arc<daw_unit_core::BandMeter<3>>>,
) {
    for (id, meter) in meters {
        renderer.set_effect_output_peak_meter(*id, meter.clone());
    }
}
fn band_meters(
    plan: &RenderPlan,
    previous: &HashMap<ModulationTarget, Arc<daw_unit_core::BandMeter<3>>>,
) -> HashMap<ModulationTarget, Arc<daw_unit_core::BandMeter<3>>> {
    plan.project
        .all_effects()
        .flat_map(|effect| {
            let group = matches!(
                effect.unit,
                daw_core::EffectSettings::Compressor(_)
                    | daw_core::EffectSettings::Ott(_)
                    | daw_core::EffectSettings::Eq3(_)
                    | daw_core::EffectSettings::PlateReverb(_)
                    | daw_core::EffectSettings::AmbientReverb(_)
                    | daw_core::EffectSettings::ShimmerReverb(_)
                    | daw_core::EffectSettings::Chorus(_)
                    | daw_core::EffectSettings::Drive(_)
                    | daw_core::EffectSettings::DriveV2(_)
                    | daw_core::EffectSettings::Bitcrusher(_)
                    | daw_core::EffectSettings::Saturator(_)
                    | daw_core::EffectSettings::Filter(_)
                    | daw_core::EffectSettings::AutoFilter(_)
                    | daw_core::EffectSettings::MultibandEq(_)
                    | daw_core::EffectSettings::HarmonicLift(_)
                    | daw_core::EffectSettings::MultibandCompressor(_)
                    | daw_core::EffectSettings::Exciter(_)
            )
            .then_some(ModulationTarget::Effect(effect.id));
            let individual = matches!(
                effect.unit,
                daw_core::EffectSettings::MultibandEq(_)
                    | daw_core::EffectSettings::MultibandCompressor(_)
                    | daw_core::EffectSettings::HarmonicLift(_)
            );
            group
                .into_iter()
                .map(|key| {
                    (
                        key,
                        matches!(effect.unit, daw_core::EffectSettings::Compressor(_)),
                    )
                })
                .chain(
                    effect
                        .unit
                        .band_ids()
                        .filter(move |_| individual)
                        .map(move |band| {
                            (
                                ModulationTarget::EffectBand {
                                    effect: effect.id,
                                    band,
                                },
                                matches!(
                                    effect.unit,
                                    daw_core::EffectSettings::MultibandCompressor(_)
                                ),
                            )
                        }),
                )
        })
        .map(|(key, compressor)| {
            let meter = previous
                .get(&key)
                .filter(|meter| !compressor || meter.history().is_some() || meter.curve().is_some())
                .cloned()
                .unwrap_or_else(|| {
                    if compressor && matches!(key, ModulationTarget::EffectBand { .. }) {
                        Arc::new(daw_unit_core::BandMeter::with_curve_history())
                    } else if compressor {
                        Arc::new(daw_unit_core::BandMeter::with_history())
                    } else {
                        Arc::default()
                    }
                });
            (key, meter)
        })
        .collect()
}
fn bind_band_meters(
    renderer: &mut Renderer,
    meters: &HashMap<ModulationTarget, Arc<daw_unit_core::BandMeter<3>>>,
) {
    for (id, meter) in meters {
        renderer.set_target_band_peak_meter(*id, meter.clone());
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
                    | daw_core::EffectSettings::DriveV2(_)
                    | daw_core::EffectSettings::Eq3(_)
                    | daw_core::EffectSettings::MultibandEq(_)
                    | daw_core::EffectSettings::HarmonicLift(_)
                    | daw_core::EffectSettings::MultibandCompressor(_)
                    | daw_core::EffectSettings::Exciter(_)
                    | daw_core::EffectSettings::Compressor(_)
                    | daw_core::EffectSettings::Limiter(_)
                    | daw_core::EffectSettings::DirectionMixer(_)
                    | daw_core::EffectSettings::StereoSpread(_)
                    | daw_core::EffectSettings::Saturator(_)
                    | daw_core::EffectSettings::Bitcrusher(_)
                    | daw_core::EffectSettings::PlateReverb(_)
                    | daw_core::EffectSettings::AmbientReverb(_)
                    | daw_core::EffectSettings::ShimmerReverb(_)
                    | daw_core::EffectSettings::Chorus(_)
                    | daw_core::EffectSettings::Ott(_)
            )
        })
        .map(|e| ModulationTarget::Effect(e.id));
    let bands = plan.project.all_effects().flat_map(|e| {
        e.unit
            .band_ids()
            .map(move |band| ModulationTarget::EffectBand { effect: e.id, band })
    });
    let noises = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.layers
                .iter()
                .filter(|l| l.noise.is_some())
                .map(move |l| ModulationTarget::Noise {
                    track: track.id,
                    layer: l.id,
                })
        })
    });
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
    let oscillator_levels = plan.project.tracks.iter().flat_map(|track| {
        track.instrument.iter().flat_map(move |i| {
            i.layers
                .iter()
                .filter(|l| !l.oscillators.is_empty())
                .map(move |l| ModulationTarget::OscillatorLevel {
                    track: track.id,
                    layer: l.id,
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
    let sidechains = plan
        .project
        .audio_sidechain_routes()
        .into_iter()
        .map(|(_, r)| ModulationTarget::Sidechain { route: r.id });
    let hosted = plan.project.hosted_units().into_iter().flat_map(|(_, s)| {
        s.parameters
            .iter()
            .filter(|p| p.definition.accepts_modulation())
            .map(move |p| ModulationTarget::Hosted {
                unit: s.id,
                parameter: p.target,
            })
    });
    effects
        .chain(hosted)
        .chain(bands)
        .chain(sidechains)
        .chain(macros)
        .chain(lfos)
        .chain(envelopes)
        .chain(amplitudes)
        .chain(samplers)
        .chain(noises)
        .chain(oscillators)
        .chain(oscillator_levels)
        .map(|key| {
            let handle = previous.get(&key).cloned().unwrap_or_else(|| {
                Arc::new(ModulationReading {
                    values: std::array::from_fn(|_| AtomicU32::new(f32::NAN.to_bits())),
                    playing: AtomicBool::new(false),
                })
            });
            (key, handle)
        })
        .collect()
}
enum Command {
    Effects(Box<effect_updates::Prepared>),
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
    audition::Sender,
    Consumer<Box<effect_updates::Prepared>>,
);
struct PublishedPlan {
    revision: u64,
    publication: u64,
    plan: Arc<RenderPlan>,
    bindings: preparation::Bindings,
}
struct SaturatorFlight {
    effect: Id,
    submitted: Arc<RenderPlan>,
    controls_only: bool,
}
pub struct AudioOutput {
    audition: audition::Sender,
    plan: Arc<RenderPlan>,
    saturator_flight: Option<SaturatorFlight>,
    desired_transport: Option<(u64, preparation::TransportRequest)>,
    resume_needs_preparation: bool,
    project_rate_hz: u32,
    buffer_size_frames: u32,
    // None is used only by device-independent control/callback tests.
    _stream: Option<cpal::Stream>,
    preparation: preparation::Worker,
    prepare_revision: u64,
    preparation_failed: bool,
    prepared: Option<preparation::Ready>,
    commands: Producer<Command>,
    retired: Consumer<Box<PreparedRenderer>>,
    effects: effect_updates::Worker,
    controls: effect_updates::Worker,
    retired_effects: Consumer<Box<effect_updates::Prepared>>,
    pending_effects: Option<effect_updates::Ready>,
    pending_controls: Option<effect_updates::Ready>,
    effect_error: Option<String>,
    active_plan: Option<Arc<RenderPlan>>,
    active_revision: u64,
    active_publication: u64,
    next_publication: u64,
    published_plans: std::collections::VecDeque<PublishedPlan>,
    pub status: Arc<Status>,
    pub description: String,
    bindings: preparation::Bindings,
    spectrum_cache: std::cell::RefCell<HashMap<Id, daw_unit_core::SpectrumCache>>,
}
/// Device-independent, worker-owned seek preparation. It can start after project
/// load and transfer to device output without discarding completed checkpoints.
pub struct AudioPreparation {
    worker: preparation::Worker,
    status: Arc<Status>,
}
impl AudioPreparation {
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        Self::with_profile(plan, daw_engine::CacheProfile::default())
    }
    pub fn with_profile(
        plan: RenderPlan,
        profile: daw_engine::CacheProfile,
    ) -> Result<Self, String> {
        let status = Arc::new(Status::default());
        let worker = preparation::Worker::new(status.clone())?;
        worker.configure_cache(profile, false);
        worker.prewarm(Arc::new(plan));
        Ok(Self { worker, status })
    }
    pub fn configure_cache(&self, profile: daw_engine::CacheProfile, editing: bool) {
        self.worker.configure_cache(profile, editing);
    }
    pub fn performance(&self) -> PerformanceSnapshot {
        self.status.performance.snapshot()
    }
    pub fn update_plan(&self, plan: RenderPlan) {
        self.worker.prewarm(Arc::new(plan));
    }
}
impl AudioOutput {
    /// Play prepared project-rate stereo audio independently of transport.
    pub fn audition(&mut self, rate: u32, samples: Vec<[f32; 2]>) -> Result<(), String> {
        if rate != self.project_rate_hz {
            return Err("Audition sample rate differs from output".into());
        }
        self.audition.play(samples)
    }
    pub fn audition_stream(&mut self, rate: u32, stream: AuditionStream) -> Result<(), String> {
        if rate != self.project_rate_hz {
            return Err("Audition sample rate differs from output".into());
        }
        self.audition.play_stream(stream)
    }
    pub fn auditioning(&self) -> bool {
        self.audition.playing()
    }
    pub fn stop_audition(&self) {
        self.audition.stop();
    }
    pub fn new(plan: RenderPlan) -> Result<Self, String> {
        Self::with_preparation(plan, None)
    }
    pub fn with_preparation(
        plan: RenderPlan,
        preparation: Option<AudioPreparation>,
    ) -> Result<Self, String> {
        Self::with_settings(plan, preparation, &AudioSettings::default())
    }
    pub fn with_settings(
        plan: RenderPlan,
        preparation: Option<AudioPreparation>,
        settings: &AudioSettings,
    ) -> Result<Self, String> {
        let project_rate_hz = plan.project.sample_rate_hz;
        let buffer_size_frames = plan.project.buffer_size_frames;
        let device = devices::output_device(settings.output_device.as_ref())?;
        let default = device.default_output_config().map_err(|e| e.to_string())?;
        let config = devices::choose_config(
            device
                .supported_output_configs()
                .map_err(|e| e.to_string())?,
            default,
            plan.project.audio_quality(),
        )?;
        let format = config.sample_format();
        let rate = config.sample_rate().0;
        let mut stream_config: cpal::StreamConfig = config.clone().into();
        let requested_buffer = buffer_size_frames;
        stream_config.buffer_size = cpal::BufferSize::Fixed(match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => requested_buffer.clamp(*min, *max),
            cpal::SupportedBufferSize::Unknown => requested_buffer,
        });
        let preparation = match preparation {
            Some(p) => p,
            None => AudioPreparation::new(plan.clone())?,
        };
        let status = preparation.status.clone();
        let bindings = preparation::Bindings::prepare(&plan, &Default::default());
        // Retried stream construction happens before playback; each attempt receives fresh queues.
        let build = |cfg: &cpal::StreamConfig| -> Result<BuiltStream, String> {
            let (commands, consumer) = RingBuffer::new(32);
            let (retire, retired) = RingBuffer::new(64);
            let mut state = Callback::initial(&plan, consumer, retire, status.clone(), rate)?;
            let audition = state.audition_sender.take().unwrap();
            let effect_retired = state.effect_receiver.take().unwrap();
            bindings.attach(&mut state.renderer);
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
                cpal::SampleFormat::I24 => stream!(cpal::I24),
                cpal::SampleFormat::I32 => stream!(i32),
                cpal::SampleFormat::I64 => stream!(i64),
                cpal::SampleFormat::U8 => stream!(u8),
                cpal::SampleFormat::U16 => stream!(u16),
                cpal::SampleFormat::U32 => stream!(u32),
                cpal::SampleFormat::U64 => stream!(u64),
                _ => return Err("Unsupported output device sample format".into()),
            }?;
            Ok((stream, commands, retired, audition, effect_retired))
        };
        let (stream, commands, retired, audition, retired_effects) = match build(&stream_config) {
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
            "{} · {} Hz · {} · {:?} (requested {} samples)",
            device.name().unwrap_or_else(|_| "Default output".into()),
            rate,
            devices::sample_format_from_cpal(format).map_or("Unknown format", SampleFormat::label),
            stream_config.buffer_size,
            requested_buffer
        );
        status
            .playhead
            .store(plan.project.transport.playhead_frame, Ordering::Relaxed);
        let initial_transport = TransportState {
            frame: plan.project.transport.playhead_frame,
            playing: false,
            paused: false,
            playback_start: None,
        };
        let mut output = Self {
            plan: Arc::new(plan),
            saturator_flight: None,
            desired_transport: None,
            resume_needs_preparation: false,
            project_rate_hz,
            buffer_size_frames,
            _stream: Some(stream),
            audition,
            preparation: preparation.worker,
            prepare_revision: 0,
            preparation_failed: false,
            prepared: None,
            commands,
            retired,
            effects: effect_updates::Worker::new()?,
            controls: effect_updates::Worker::controls()?,
            retired_effects,
            pending_effects: None,
            pending_controls: None,
            effect_error: None,
            active_plan: None,
            active_revision: 0,
            active_publication: 0,
            next_publication: status.processing_publication.load(Ordering::Acquire),
            published_plans: Default::default(),
            status,
            description,
            bindings,
            spectrum_cache: Default::default(),
        };
        output.prepare(Some(initial_transport));
        Ok(output)
    }
    fn send(&mut self, command: Command) -> Result<(), String> {
        self.drain();
        self.commands
            .push(command)
            .map_err(|_| "Audio command queue is full; try again".into())
    }
    /// Live monitor state; does not change project data or offline rendering.
    pub fn set_metronome(&self, enabled: bool) {
        self.status
            .metronome_enabled
            .store(enabled, Ordering::Relaxed);
    }

    /// UI state only; changes never enter the project, history or export renderer.
    pub fn set_exciter_solos(&self, selected: &std::collections::HashSet<(Id, Id)>) {
        for (key, flag) in &self.bindings.exciter_solos {
            if let ModulationTarget::EffectBand { effect, band } = *key {
                flag.store(selected.contains(&(effect, band)), Ordering::Relaxed);
            }
        }
    }
    /// Coalesced live input: no command allocation, queue growth, preparation or voice restart.
    pub fn set_live_midi_pitch(
        &self,
        track: Id,
        clip: Id,
        note: Id,
        pitch: u8,
        active: bool,
    ) -> bool {
        self.bindings
            .live_pitches
            .set(track, clip, note, pitch, active)
    }
    /// Roll back a released preview against the complete playback schedule.
    /// Newer active drags remain in control of their mailboxes.
    pub fn restore_released_midi_pitch(&self, track: Id, clip: Id, note: Id) {
        self.bindings
            .live_pitches
            .restore_released(track, clip, note);
    }
    pub fn configure_cache(&self, profile: daw_engine::CacheProfile, editing: bool) {
        self.preparation.configure_cache(profile, editing);
    }
    /// Control-thread access to worker-latched disk failures.
    pub fn source_error(&self) -> Option<String> {
        self.plan
            .project
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .filter(|clip| !clip.muted)
            .filter_map(|clip| self.plan.audio_for_clip(clip))
            .filter_map(|audio| audio.disk.as_ref())
            .find_map(|disk| disk.error())
    }
    pub fn take_preparation_error(&mut self) -> Option<String> {
        let error = self
            .effect_error
            .take()
            .or_else(|| self.preparation.take_error())?;
        let deferred = self
            .saturator_flight
            .take()
            .filter(|flight| !flight.submitted.same_processing_plan(&self.plan));
        if self
            .desired_transport
            .is_some_and(|(_, request)| !request.state.playing)
        {
            // Retain Stop/Pause intent and its immediate callback command even
            // when reconstruction fails before the callback consumes it. Play
            // must prepare this requested position rather than resume old DSP.
            self.preparation_failed = true;
            self.prepared = None;
            return Some(error);
        }
        self.desired_transport = None;
        self.prepare_revision = self.status.applied_revision.load(Ordering::Acquire);
        self.status
            .requested_revision
            .store(self.prepare_revision, Ordering::Release);
        if let Some(flight) = deferred {
            self.start_saturator_flight(flight.effect, flight.controls_only);
        }
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
    fn pending_transport(&self) -> Option<preparation::TransportRequest> {
        self.desired_transport
            .filter(|(revision, _)| {
                *revision > self.status.applied_revision.load(Ordering::Acquire)
            })
            .map(|(_, request)| request)
    }
    fn current_transport(&self) -> TransportState {
        self.pending_transport()
            .map_or_else(|| self.status.transport(), |request| request.state)
    }
    fn prepare(&mut self, state: Option<TransportState>) {
        self.saturator_flight = None;
        self.prepare_dispatched(state.map(Into::into));
    }
    fn prepare_dispatched(&mut self, request: Option<preparation::TransportRequest>) {
        self.prepare_plan(request, None);
    }
    fn prepare_plan(
        &mut self,
        request: Option<preparation::TransportRequest>,
        live_effect: Option<preparation::LiveEffectBase>,
    ) {
        let defer_controls = live_effect.as_ref().is_some_and(|effect| {
            matches!(
                effect.edit,
                preparation::LiveEffectRequest::Remove(_) | preparation::LiveEffectRequest::Move(_)
            )
        });
        if request.is_some_and(|request| !request.state.playing) {
            self.resume_needs_preparation = true;
        }
        self.preparation_failed = false;
        self.prepare_revision = if let Some(insert) = live_effect {
            self.preparation.submit_live_effect(
                self.dispatched_plan().clone(),
                self.bindings.clone(),
                self.append_predecessor(),
                insert,
            )
        } else if let Some(predecessor) = self.append_predecessor()
            && request.is_none()
        {
            self.preparation.submit_append(
                self.dispatched_plan().clone(),
                self.bindings.clone(),
                predecessor,
            )
        } else {
            self.preparation.submit_transport(
                self.dispatched_plan().clone(),
                self.bindings.clone(),
                request,
            )
        };
        self.status
            .requested_revision
            .store(self.prepare_revision, Ordering::Release);
        self.desired_transport = request.map(|request| (self.prepare_revision, request));
        self.prepared = None;
        self.effects.cancel(self.prepare_revision);
        self.controls.cancel(self.prepare_revision);
        self.pending_controls = None;
        self.pending_effects = None;
        self.effect_error = None;
        if !defer_controls {
            self.submit_control_update(true);
        }
    }
    pub fn sync(&mut self, plan: RenderPlan) -> Result<(), String> {
        self.sync_with_mode(plan, PlaybackSeek::default())
    }
    pub fn sync_with_mode(&mut self, plan: RenderPlan, mode: PlaybackSeek) -> Result<(), String> {
        if plan.project.sample_rate_hz != self.project_rate_hz
            || plan.project.buffer_size_frames != self.buffer_size_frames
            || plan.project.output_sample_format != self.plan.project.output_sample_format
        {
            return Err("Recreate output after changing project audio settings".into());
        }
        self.drain();
        let transport = self.pending_transport();
        if let Some(flight) = &self.saturator_flight
            && (self.plan.midi_output_saturator_edit(&plan)
                == Some(MidiOutputSaturatorEdit::Drive(flight.effect))
                || (self.plan.external.offline == plan.external.offline
                    && self.plan.external.publish == plan.external.publish
                    && self.plan.same_processing_plan(&plan)))
        {
            self.plan = Arc::new(plan);
            return Ok(());
        }
        let edit = if transport.is_none() && !self.preparing() {
            self.active_plan
                .as_ref()
                .and_then(|active| active.midi_output_saturator_edit(&plan))
        } else {
            None
        };
        let live = mode == PlaybackSeek::Fast
            && transport.is_none()
            && self.status.playing.load(Ordering::Acquire)
            && self.current_plan_published();
        let live_edit = live
            .then(|| {
                use preparation::LiveEffectRequest::*;
                let before = self.plan.project.all_effects().count();
                let after = plan.project.all_effects().count();
                if after == before {
                    return self
                        .plan
                        .needs_live_effect_move(&plan)
                        .then(|| Move(self.plan.clone()));
                }
                let old_native = self.plan.project.hosted_units().len();
                let new_native = plan.project.hosted_units().len();
                let old_delayed = self.plan.latency_effect_count();
                let new_delayed = plan.latency_effect_count();
                if after == before + 1
                    && (new_native == old_native + 1
                        || (new_native == 0 && new_delayed == old_delayed + 1))
                {
                    Some(Effect(self.plan.clone()))
                } else if after > before && (new_native != 0 || new_delayed > old_delayed) {
                    Some(Additions(self.plan.clone()))
                } else if before == after + 1
                    && (old_native == new_native + 1 || old_delayed == new_delayed + 1)
                {
                    Some(Remove(self.plan.clone()))
                } else {
                    None
                }
            })
            .flatten();
        let live_effect = live_edit.map(|edit| preparation::LiveEffectBase {
            publication: self.active_publication,
            edit,
        });
        self.plan = Arc::new(plan);
        self.saturator_flight = None;
        match edit {
            Some(MidiOutputSaturatorEdit::Append(effect)) => {
                let insert = live.then_some(preparation::LiveEffectBase {
                    publication: self.active_publication,
                    edit: preparation::LiveEffectRequest::Builtin(effect),
                });
                self.start_saturator_insertion(effect, insert);
            }
            Some(MidiOutputSaturatorEdit::Drive(effect)) => {
                self.start_saturator_flight(effect, true);
            }
            None if live_effect.is_some() => self.prepare_plan(None, live_effect),
            None if transport.is_some() || self.active_plan.is_none() => {
                self.prepare_dispatched(transport)
            }
            None => self.prepare_effects(),
        }
        Ok(())
    }
    /// Publish a baked arrangement through exact worker reconstruction. Inheriting
    /// source effect tails would add history already present in the rendered clip.
    pub fn sync_reconstructed(&mut self, plan: RenderPlan) -> Result<(), String> {
        if plan.project.sample_rate_hz != self.project_rate_hz
            || plan.project.buffer_size_frames != self.buffer_size_frames
            || plan.project.output_sample_format != self.plan.project.output_sample_format
        {
            return Err("Recreate output after changing project audio settings".into());
        }
        self.drain();
        let state = self.current_transport();
        self.plan = Arc::new(plan);
        self.prepare(Some(state));
        Ok(())
    }
    fn prepare_effects(&mut self) {
        let Some(previous) = self.active_plan.clone() else {
            self.prepare_dispatched(None);
            return;
        };
        self.prepare_revision = self
            .preparation
            .update_effect_cache(self.dispatched_plan().clone(), self.append_predecessor());
        self.status
            .requested_revision
            .store(self.prepare_revision, Ordering::Release);
        self.preparation_failed = false;
        self.effect_error = None;
        self.prepared = None;
        self.pending_effects = None;
        self.pending_controls = None;
        self.submit_control_update(false);
        if self.saturator_controls_only() {
            self.effects.cancel(self.prepare_revision);
            return;
        }
        self.effects.submit(effect_updates::Request {
            revision: self.prepare_revision,
            base_revision: self.active_revision,
            base_publication: self.active_publication,
            previous,
            desired: self.dispatched_plan().clone(),
            bindings: self.bindings.clone(),
            partial_only: false,
        });
    }
    fn dispatched_plan(&self) -> &Arc<RenderPlan> {
        self.saturator_flight
            .as_ref()
            .map_or(&self.plan, |flight| &flight.submitted)
    }
    fn append_predecessor(&self) -> Option<Arc<RenderPlan>> {
        self.saturator_flight
            .as_ref()
            .filter(|flight| !flight.controls_only)?;
        self.active_plan.clone()
    }
    fn saturator_controls_only(&self) -> bool {
        self.saturator_flight
            .as_ref()
            .is_some_and(|flight| flight.controls_only)
    }
    fn start_saturator_flight(&mut self, effect: Id, controls_only: bool) {
        self.saturator_flight = Some(SaturatorFlight {
            effect,
            submitted: self.plan.clone(),
            controls_only,
        });
        self.prepare_effects();
    }
    fn start_saturator_insertion(
        &mut self,
        effect: Id,
        insert: Option<preparation::LiveEffectBase>,
    ) {
        if let Some(insert) = insert {
            self.saturator_flight = Some(SaturatorFlight {
                effect,
                submitted: self.plan.clone(),
                controls_only: false,
            });
            self.prepare_plan(None, Some(insert));
        } else {
            self.start_saturator_flight(effect, false);
        }
    }
    fn finish_saturator_flight(&mut self) {
        if self.active_revision != self.prepare_revision
            || self.applied_revision() != self.prepare_revision
        {
            return;
        }
        let Some(flight) = self.saturator_flight.take() else {
            return;
        };
        if Arc::ptr_eq(&flight.submitted, &self.plan) {
            return;
        }
        if flight.submitted.same_processing_plan(&self.plan) {
            self.active_plan = Some(self.plan.clone());
        } else {
            self.start_saturator_flight(flight.effect, true);
        }
    }
    fn submit_control_update(&mut self, partial_only: bool) {
        self.pending_controls = None;
        if let Some(previous) = self.active_plan.clone() {
            self.controls.submit(effect_updates::Request {
                revision: self.prepare_revision,
                base_revision: self.active_revision,
                base_publication: self.active_publication,
                previous,
                desired: self.dispatched_plan().clone(),
                bindings: self.bindings.clone(),
                partial_only,
            });
        }
    }
    /// Register once after the UI creates output; worker completion wakes idle painting.
    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        let wake = Arc::new(wake);
        let effects_wake = wake.clone();
        self.preparation.set_waker(move || wake());
        let controls_wake = effects_wake.clone();
        self.effects.set_waker(move || effects_wake());
        self.controls.set_waker(move || controls_wake());
    }
    pub fn track_meter(&self, id: Id) -> Option<&StereoMeter> {
        self.bindings.tracks.get(&id).map(AsRef::as_ref)
    }
    /// Post-fader instrument bus monitor, scoped by track and layer/Output identity.
    pub fn layer_meter(&self, track: Id, layer: Id) -> Option<&StereoMeter> {
        self.bindings.layers.get(&(track, layer)).map(AsRef::as_ref)
    }
    /// Display-only block snapshots. No synthetic animation when playback is stopped.
    pub fn effect_spectrum(&self, effect: Id) -> Option<daw_unit_core::SpectrumReading> {
        let meter = self.bindings.spectra.get(&effect)?;
        let mut caches = self.spectrum_cache.borrow_mut();
        let cache = caches.entry(effect).or_default();
        if !self.status.playing.load(Ordering::Relaxed) {
            cache.clear();
            return Some(daw_unit_core::SpectrumReading {
                sample_rate_hz: self.plan.project.sample_rate_hz,
                ..Default::default()
            });
        }
        Some(cache.read(meter).unwrap_or(daw_unit_core::SpectrumReading {
            sample_rate_hz: self.plan.project.sample_rate_hz,
            ..Default::default()
        }))
    }
    pub fn modulation_values(&self, key: ModulationTarget) -> Option<[f32; 5]> {
        self.modulation_snapshot(key)
            .map(|v| std::array::from_fn(|i| v[i]))
    }
    pub fn modulation_snapshot(
        &self,
        key: ModulationTarget,
    ) -> Option<daw_engine::ModulationSnapshot> {
        let playing = self.status.playing.load(Ordering::Relaxed);
        let is_envelope = matches!(key, ModulationTarget::Envelope { .. });
        self.bindings
            .modulation
            .get(&key)?
            .read(playing, is_envelope)
    }
    pub fn gain_reduction_db(&self, id: Id) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.bindings
            .effects
            .get(&id)
            .map_or(0.0, |meter| meter.reading())
    }
    /// Held readings remain available while paused/stopped and while panels are hidden.
    pub fn gain_reduction_meter(&self, id: Id) -> Option<&daw_unit_core::PeakMeter> {
        self.bindings.effects.get(&id).map(AsRef::as_ref)
    }
    /// Detector input envelope for the compressor transfer-curve marker.
    /// Audio-clocked sweep remains available while paused or the panel is hidden.
    pub fn compressor_history(&self, id: Id) -> Option<daw_unit_core::LevelHistoryReading> {
        self.bindings
            .bands
            .get(&ModulationTarget::Effect(id))?
            .history()?
            .reading()
    }
    pub fn compressor_detector_level(&self, id: Id) -> f32 {
        self.band_monitor_values(id)[0]
    }
    pub fn ott_gain_change_db(&self, id: Id) -> [f32; 3] {
        self.band_monitor_values(id)
    }
    /// Independent OTT band holds, also available while paused/stopped.
    pub fn ott_gain_change_meter(&self, id: Id) -> Option<&daw_unit_core::BandMeter<3>> {
        self.bindings
            .bands
            .get(&ModulationTarget::Effect(id))
            .map(AsRef::as_ref)
    }
    /// Held linear output sample peak, retained while paused/stopped and resettable by the UI.
    pub fn source_output_level(&self, target: daw_engine::SourceOutputTarget) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.bindings
            .sources
            .get(&target)
            .map_or(0.0, |m| m.readings()[0])
    }
    pub fn source_output_peak(
        &self,
        target: daw_engine::SourceOutputTarget,
    ) -> Option<&daw_unit_core::PeakHold> {
        self.source_output_meter(target)
            .map(|m| &m.boost_peaks()[0])
    }
    pub fn source_output_meter(
        &self,
        target: daw_engine::SourceOutputTarget,
    ) -> Option<&daw_unit_core::BandMeter<3>> {
        self.bindings.sources.get(&target).map(AsRef::as_ref)
    }
    pub fn source_output_levels(&self, target: daw_engine::SourceOutputTarget) -> [f32; 2] {
        if !self.status.playing.load(Ordering::Relaxed) {
            return [0.0; 2];
        }
        Self::stereo_output(self.source_output_meter(target))
    }
    fn stereo_output(meter: Option<&daw_unit_core::BandMeter<3>>) -> [f32; 2] {
        meter.map_or([0.0; 2], |m| {
            let [_, left, right] = m.readings();
            [left, right]
        })
    }
    pub fn effect_output_meter(&self, id: Id) -> Option<&daw_unit_core::BandMeter<3>> {
        self.bindings
            .outputs
            .get(&id)
            .or_else(|| self.bindings.bands.get(&ModulationTarget::Effect(id)))
            .map(AsRef::as_ref)
    }
    pub fn effect_output_levels(&self, id: Id) -> [f32; 2] {
        if !self.status.playing.load(Ordering::Relaxed) {
            return [0.0; 2];
        }
        Self::stereo_output(self.effect_output_meter(id))
    }
    /// Held effect output peak, retained while paused/stopped and resettable by the UI.
    pub fn loudness_meter(&self, id: Id) -> Option<&daw_unit_core::LoudnessMeter> {
        self.bindings.loudness.get(&id).map(AsRef::as_ref)
    }
    pub fn effect_output_peak(&self, id: Id) -> Option<&daw_unit_core::PeakHold> {
        self.effect_output_meter(id).map(|m| &m.boost_peaks()[0])
    }
    /// Current linear output level; stopped or absent outputs show silence.
    pub fn effect_output_level(&self, id: Id) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.bindings
            .outputs
            .get(&id)
            .map_or_else(|| self.band_monitor_values(id)[0], |m| m.readings()[0])
    }
    /// Linear Low/Mid/High output peaks. Stopped or absent outputs show silence.
    pub fn eq_band_levels(&self, id: Id) -> [f32; 3] {
        self.band_monitor_values(id)
    }
    pub fn eq_individual_band_level(&self, effect: Id, band: Id) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.bindings
            .bands
            .get(&ModulationTarget::EffectBand { effect, band })
            .map_or(0.0, |m| m.readings()[0])
    }
    pub fn compressor_band_curve(
        &self,
        effect: Id,
        band: Id,
    ) -> (f32, Option<daw_unit_core::LevelHistoryReading>) {
        let curve = self
            .bindings
            .bands
            .get(&ModulationTarget::EffectBand { effect, band })
            .and_then(|m| m.curve());
        let level = if self.status.playing.load(Ordering::Relaxed) {
            curve.map_or(0.0, |m| m.readings()[0])
        } else {
            0.0
        };
        (
            level,
            curve.and_then(|m| m.history()).and_then(|h| h.reading()),
        )
    }
    pub fn individual_band_level_peak(
        &self,
        effect: Id,
        band: Id,
    ) -> Option<&daw_unit_core::PeakHold> {
        self.bindings
            .bands
            .get(&ModulationTarget::EffectBand { effect, band })
            .map(|m| &m.boost_peaks()[0])
    }
    pub fn individual_band_peak(&self, effect: Id, band: Id) -> Option<&daw_unit_core::PeakHold> {
        self.bindings
            .bands
            .get(&ModulationTarget::EffectBand { effect, band })
            .and_then(|m| m.curve())
            .and_then(|m| m.curve_reduction())
            .map(|m| m.hold())
    }
    pub fn multiband_compressor_reduction(&self, effect: Id, band: Id) -> f32 {
        if !self.status.playing.load(Ordering::Relaxed) {
            return 0.0;
        }
        self.bindings
            .bands
            .get(&ModulationTarget::EffectBand { effect, band })
            .and_then(|m| m.curve())
            .and_then(|m| m.curve_reduction())
            .map_or(0.0, |m| m.reading())
    }
    fn band_monitor_values(&self, id: Id) -> [f32; 3] {
        if !self.status.playing.load(Ordering::Relaxed) {
            return [0.0; 3];
        }
        self.bindings
            .bands
            .get(&ModulationTarget::Effect(id))
            .map_or([0.0; 3], |meter| meter.readings())
    }
    pub fn play(&mut self) -> Result<(), String> {
        self.play_with_mode(PlaybackSeek::default())
    }
    pub fn play_with_mode(&mut self, mode: PlaybackSeek) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        if state.playing {
            return Ok(());
        }
        let resume = state.paused
            && !self.resume_needs_preparation
            && self.prepare_revision == self.applied_revision()
            && !self.preparing();
        if !state.paused {
            state.playback_start = Some(state.frame);
        }
        state.playing = true;
        state.paused = false;
        if resume {
            self.immediate(state, true)
        } else {
            self.prepare_playback_position(state, mode);
            Ok(())
        }
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        if state.playing && !self.preparing() {
            // An ordinary pause retains active playback history. A prepared seek
            // clears it for silent monitoring and needs preparation on Play.
            self.resume_needs_preparation = false;
        }
        state.paused |= state.playing;
        state.playing = false;
        if self.preparing() || self.prepare_revision != self.applied_revision() {
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
        self.saturator_flight = None;
        self.preparation_failed = false;
        self.prepare_revision = revision;
        self.status
            .requested_revision
            .store(revision, Ordering::Release);
        self.desired_transport = Some((revision, state.into()));
        self.send(if resume {
            Command::ResumeAt(revision)
        } else {
            Command::PauseAt(revision)
        })
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.drain();
        if self.commands.is_full() {
            return Err("Audio command queue is full; try again".into());
        }
        let mut state = self.current_transport();
        state.frame = state.playback_start.take().unwrap_or(state.frame);
        state.playing = false;
        state.paused = false;
        self.prepare(Some(state));
        // No intervening drain: reserve the checked slot for immediate silence,
        // before a fast worker can enqueue its completed replacement.
        self.commands
            .push(Command::PausePending(self.prepare_revision))
            .map_err(|_| "Audio command queue is full; try again".into())
    }
    pub fn seek(&mut self, frame: u64) -> Result<(), String> {
        self.seek_with_mode(frame, PlaybackSeek::default())
    }
    pub fn seek_with_mode(&mut self, frame: u64, mode: PlaybackSeek) -> Result<(), String> {
        self.drain();
        let mut state = self.current_transport();
        state.frame = frame;
        self.prepare_playback_position(state, mode);
        Ok(())
    }
    fn prepare_playback_position(&mut self, state: TransportState, mode: PlaybackSeek) {
        self.saturator_flight = None;
        self.prepare_dispatched(Some(preparation::TransportRequest {
            state,
            fast: state.playing && mode == PlaybackSeek::Fast,
        }));
    }
    /// Publication receipt for hosts that stage an atomic model transaction.
    pub fn current_plan_published(&self) -> bool {
        self.active_plan
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(p, &self.plan))
            && self.active_publication == self.status.processing_publication.load(Ordering::Acquire)
            && self.prepare_revision == self.status.applied_revision.load(Ordering::Acquire)
    }
    pub fn requested_revision(&self) -> u64 {
        self.prepare_revision
    }
    pub fn applied_revision(&self) -> u64 {
        self.status.applied_revision.load(Ordering::Acquire)
    }
    pub fn preparing(&self) -> bool {
        !self.preparation_failed
            && (self.prepare_revision != self.applied_revision()
                || !Arc::ptr_eq(self.dispatched_plan(), &self.plan))
    }
    fn publish_bindings(&mut self, ready: preparation::Bindings) {
        self.bindings = ready;
        self.spectrum_cache
            .borrow_mut()
            .retain(|id, _| self.bindings.spectra.contains_key(id));
    }
    fn acknowledge_processing(&mut self) {
        let publication = self.status.processing_publication.load(Ordering::Acquire);
        if publication != self.active_publication
            && let Some(i) = self
                .published_plans
                .iter()
                .position(|p| p.publication == publication)
        {
            let record = self.published_plans.remove(i).unwrap();
            self.active_revision = record.revision;
            self.active_publication = publication;
            self.active_plan = Some(record.plan);
            self.publish_bindings(record.bindings);
        }
    }
    fn forget_publication(&mut self, publication: u64) {
        self.published_plans
            .retain(|p| p.publication != publication);
    }
    fn publication_id(&mut self) -> u64 {
        self.next_publication += 1;
        self.next_publication
    }
    pub fn drain(&mut self) {
        self.audition.drain();
        self.acknowledge_processing();
        let mut retry = false;
        let mut retry_controls = false;
        while let Ok(old) = self.retired_effects.pop() {
            // A callback may have applied a candidate after the first status read.
            // Unique publication IDs keep a rejected sibling from erasing its ACK record.
            self.acknowledge_processing();
            if !old.applied {
                self.forget_publication(old.publication);
                retry |= old.complete && old.revision == self.prepare_revision;
                retry_controls |= !old.complete && old.revision == self.prepare_revision;
            } else if old.publication != self.active_publication {
                self.forget_publication(old.publication);
            }
        }
        if retry && self.status.applied_revision.load(Ordering::Acquire) != self.prepare_revision {
            self.status.performance.effect_rebase();
            self.prepare_effects();
        } else if retry_controls
            && self.status.applied_revision.load(Ordering::Acquire) != self.prepare_revision
        {
            self.submit_control_update(true);
        }
        if let Some(ready) = self.controls.take()
            && ready.revision == self.prepare_revision
        {
            self.pending_controls = Some(ready);
        }
        if let Some(mut ready) = self.pending_controls.take() {
            if self.saturator_controls_only()
                && !matches!(&ready.prepared, Ok(Some(p)) if p.complete)
            {
                match ready.prepared {
                    Err(error) => {
                        self.effect_error = Some(error);
                        self.preparation_failed = true;
                    }
                    Ok(_) => {
                        // Keep the frozen input if the compatible worker cannot cover it.
                        self.saturator_flight.as_mut().unwrap().controls_only = false;
                        self.prepare_effects();
                    }
                }
            } else if let Ok(Some(mut prepared)) = ready.prepared {
                let publication = self.publication_id();
                prepared.publication = publication;
                let effective = if prepared.complete {
                    ready.revision
                } else {
                    u64::MAX - ready.revision
                };
                match self.commands.push(Command::Effects(prepared)) {
                    Ok(()) => {
                        self.published_plans.push_back(PublishedPlan {
                            revision: effective,
                            publication,
                            plan: ready.desired,
                            bindings: ready.bindings,
                        });
                    }
                    Err(rtrb::PushError::Full(Command::Effects(prepared))) => {
                        ready.prepared = Ok(Some(prepared));
                        self.pending_controls = Some(ready);
                    }
                    Err(_) => unreachable!("only controls are submitted here"),
                }
            }
        }
        if let Some(ready) = self.effects.take()
            && ready.revision == self.prepare_revision
            && self.status.applied_revision.load(Ordering::Acquire) != ready.revision
        {
            self.pending_effects = Some(ready);
        }
        if let Some(mut ready) = self.pending_effects.take() {
            match ready.prepared {
                Ok(Some(mut prepared)) => {
                    let publication = self.publication_id();
                    prepared.publication = publication;
                    match self.commands.push(Command::Effects(prepared)) {
                        Ok(()) => self.published_plans.push_back(PublishedPlan {
                            revision: ready.revision,
                            publication,
                            plan: ready.desired,
                            bindings: ready.bindings,
                        }),
                        Err(rtrb::PushError::Full(Command::Effects(prepared))) => {
                            ready.prepared = Ok(Some(prepared));
                            self.pending_effects = Some(ready);
                        }
                        Err(_) => unreachable!("only effect updates are submitted here"),
                    }
                }
                Ok(None) => {
                    if let Some(reason) = ready.fallback {
                        self.status.performance.effect_fallback(reason);
                    }
                    let transport = self.pending_transport();
                    self.prepare_dispatched(transport);
                }
                Err(error) => {
                    self.effect_error = Some(error);
                    self.preparation_failed = true;
                }
            }
        }
        while let Ok(mut old) = self.retired.pop() {
            self.acknowledge_processing();
            self.forget_publication(old.publication);
            if old.revision == self.prepare_revision
                && let Some(transport) = old.reprepare.take()
            {
                if old.live_effect.is_some_and(|effect| {
                    matches!(
                        effect.kind,
                        LiveEffectKind::Remove(_) | LiveEffectKind::UnverifiedRemoval
                    )
                }) {
                    self.submit_control_update(true);
                }
                self.preparation.reprepare(
                    preparation::Ready {
                        revision: old.revision,
                        renderer: old,
                        bindings: self.bindings.clone(),
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
            let publication = self.publication_id();
            ready.renderer.publication = publication;
            match self.commands.push(Command::Plan(ready.renderer)) {
                Ok(()) => {
                    self.published_plans.push_back(PublishedPlan {
                        revision: ready.revision,
                        publication,
                        plan: self.dispatched_plan().clone(),
                        bindings: ready.bindings,
                    });
                }
                Err(rtrb::PushError::Full(Command::Plan(renderer))) => {
                    ready.renderer = renderer;
                    self.prepared = Some(ready);
                }
                Err(_) => unreachable!("only a prepared plan was submitted"),
            }
        }
        self.finish_saturator_flight();
    }
}
struct Callback {
    midi: midi_input::Receiver,
    project_rate_hz: u32,
    metronome: metronome::Metronome,
    audition: audition::Player,
    audition_sender: Option<audition::Sender>,
    renderer: Box<PreparedRenderer>,
    initial_renderer: bool,
    commands: Consumer<Command>,
    retired: Producer<Box<PreparedRenderer>>,
    pending: Option<Box<PreparedRenderer>>,
    pending_effect: Option<Box<effect_updates::Prepared>>,
    effect_retired: Producer<Box<effect_updates::Prepared>>,
    effect_receiver: Option<Consumer<Box<effect_updates::Prepared>>>,
    scheduled: Option<Box<PreparedRenderer>>,
    rendered_frames: u64,
    status: Arc<Status>,
    converter: Option<rubato::FftFixedIn<f32>>,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    output_count: usize,
    index: usize,
}
impl Callback {
    fn audition_chunk_frames(&self, device_frames: usize, device_rate: u32) -> usize {
        audition_stream::source_chunk_frames(self.project_rate_hz, device_rate, device_frames)
            .unwrap_or(1)
            .max(self.input.first().map_or(0, Vec::len))
            .max((self.project_rate_hz as usize / 200).max(1))
    }
    /// The only device-startup constructor: never instantiate logical track DSP here.
    fn initial(
        plan: &RenderPlan,
        commands: Consumer<Command>,
        retired: Producer<Box<PreparedRenderer>>,
        status: Arc<Status>,
        rate: u32,
    ) -> Result<Self, String> {
        let mut callback = Self::new(
            preparation::initial_plan(plan),
            &HashMap::new(),
            commands,
            retired,
            status,
            rate,
        )?;
        callback.initial_renderer = true;
        Ok(callback)
    }

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
        let (sender, audition) = audition::channel(project_rate_hz);
        let (effect_retired, effect_receiver) = RingBuffer::new(64);
        let midi = midi_input::Receiver::new(&status);
        Ok(Self {
            project_rate_hz,
            midi,
            metronome: metronome::Metronome::new(project_rate_hz),
            audition,
            audition_sender: Some(sender),
            renderer: Box::new(PreparedRenderer::try_new(plan, meters)?),
            initial_renderer: false,
            commands,
            retired,
            pending: None,
            pending_effect: None,
            effect_retired,
            effect_receiver: Some(effect_receiver),
            scheduled: None,
            rendered_frames: 0,
            status,
            converter,
            input,
            output,
            output_count: 0,
            index: 0,
        })
    }
    fn update(&mut self) {
        self.metronome
            .set_enabled(self.status.metronome_enabled.load(Ordering::Relaxed));
        self.audition.update();
        let started = std::time::Instant::now();
        self.activate_scheduled();
        for _ in 0..8 {
            let command = if let Some(pending) = self.pending_effect.take() {
                Command::Effects(pending)
            } else if let Some(pending) = self.pending.take() {
                Command::Plan(pending)
            } else {
                match self.commands.pop() {
                    Ok(c) => c,
                    Err(_) => break,
                }
            };
            match command {
                Command::Effects(mut next) => {
                    if self.effect_retired.is_full() {
                        self.pending_effect = Some(next);
                        break;
                    }
                    if next.revision == self.status.requested_revision.load(Ordering::Acquire)
                        && next.base_revision == self.renderer.revision
                        && next.base_publication == self.renderer.publication
                    {
                        self.renderer.apply_effect_update(&mut next.update);
                        std::mem::swap(&mut self.renderer.modulation, &mut next.modulation);
                        let bytes = self.renderer.owned_memory_bytes();
                        self.renderer._memory.redistribute(&mut next.memory, bytes);
                        let effective = if next.complete {
                            next.revision
                        } else {
                            u64::MAX - next.revision
                        };
                        self.renderer.revision = effective;
                        self.renderer.publication = next.publication;
                        self.status.processing_bytes.store(bytes, Ordering::Release);
                        self.status
                            .processing_revision
                            .store(effective, Ordering::Release);
                        self.status
                            .processing_publication
                            .store(next.publication, Ordering::Release);
                        if next.complete {
                            self.status
                                .applied_revision
                                .store(next.revision, Ordering::Release);
                        }
                        self.status.performance.activation(
                            next.requested_at
                                .elapsed()
                                .as_nanos()
                                .min(u128::from(u64::MAX)) as u64,
                        );
                        self.status
                            .performance
                            .effect_applied(next.update.prepared_processors());
                        next.applied = true;
                    }
                    let _ = self.effect_retired.push(next);
                }
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
                    if next.needs_native_boundary()
                        && next
                            .live_effect
                            .is_some_and(|edit| edit.base_publication == self.renderer.publication)
                        && self.renderer.playing
                    {
                        let pending = self.renderer.pending_hosted_frames();
                        if pending != 0 {
                            next.activate_at =
                                Some(self.rendered_frames.saturating_add(pending as u64));
                        }
                    }
                    if next.activate_at.is_some() {
                        self.scheduled = Some(next);
                        self.activate_scheduled();
                        continue;
                    }
                    if next.prepared_transport {
                        next.activate_transport(&self.renderer);
                        next.inherit_analysis(&mut self.renderer);
                        // The initial stopped renderer contributes only silence. Its
                        // converted buffer can contain active audition audio, which
                        // must survive publication of a stopped project renderer.
                        if !self.initial_renderer || next.playing || self.renderer.playing {
                            self.discard_buffered_output();
                        }
                    } else if !self.inherit_plan(&mut next) {
                        // Rebuild at this exact clock, then catch up on the worker.
                        // The current renderer keeps playing throughout preparation.
                        self.reprepare_plan(next);
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
    fn inherit_plan(&mut self, next: &mut PreparedRenderer) -> bool {
        if let Some(insert) = next.live_effect
            && insert.base_publication == self.renderer.publication
            && match insert.kind {
                LiveEffectKind::Hosted(unit) => {
                    next.try_insert_hosted_effect(&mut self.renderer, unit)
                }
                LiveEffectKind::Builtin(_) => next.try_insert_builtin_effect(&mut self.renderer),
                LiveEffectKind::Additions => next.try_insert_effect_group(&mut self.renderer),
                LiveEffectKind::Remove(unit) => {
                    next.try_remove_effect_live(&mut self.renderer, unit)
                }
                LiveEffectKind::Move => next.try_move_effects_live(&mut self.renderer),
                LiveEffectKind::UnverifiedRemoval => false,
            }
        {
            return true;
        }
        if next.can_inherit_without_preroll(&self.renderer) {
            next.inherit(&mut self.renderer);
            return true;
        }
        false
    }
    fn preparation_anchor(&self) -> preparation::Anchor {
        preparation::Anchor {
            transport: self.renderer.transport_state(),
            clock: self.rendered_frames,
        }
    }
    fn reprepare_plan(&mut self, mut next: Box<PreparedRenderer>) {
        next.allow_checkpoint_reuse &= self.renderer.allows_checkpoint_reuse();
        next.reprepare = Some(self.preparation_anchor());
        let _ = self.retired.push(next);
    }
    fn install(&mut self, mut next: Box<PreparedRenderer>) {
        if next.prepared_transport {
            next.inherit_live_midi(&mut self.renderer);
        }
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
        self.initial_renderer = false;
        self.status
            .processing_bytes
            .store(self.renderer.owned_memory_bytes(), Ordering::Release);
        self.status
            .processing_revision
            .store(revision, Ordering::Release);
        self.status
            .processing_publication
            .store(self.renderer.publication, Ordering::Release);
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
        } else if next.needs_native_boundary() {
            if self.inherit_plan(&mut next) {
                self.install(next);
            } else {
                self.reprepare_plan(next);
            }
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
            next.inherit_analysis(&mut self.renderer);
            self.install(next);
        }
    }
    fn next_project_sample(&mut self) -> [f32; 2] {
        self.activate_scheduled();
        let count_in = self
            .midi
            .sample(&mut self.renderer, self.rendered_frames, &self.status);
        let sample = self.renderer.next_sample();
        self.status
            .buffering
            .store(self.renderer.buffering(), Ordering::Relaxed);
        if self.renderer.buffering() && self.renderer.source_failed() {
            self.renderer.playing = false;
            self.status.source_failed.store(true, Ordering::Relaxed);
        }
        let frame = (self.renderer.playing && !self.renderer.buffering())
            .then(|| self.renderer.playhead.saturating_sub(1));
        self.metronome
            .set_signature(self.renderer.clock.signature_at(self.renderer.playhead));
        self.metronome.set_enabled(
            count_in.is_some() || self.status.metronome_enabled.load(Ordering::Relaxed),
        );
        let click = if let Some(pre_roll) = count_in {
            self.metronome.next(
                Some(pre_roll),
                f64::from(self.renderer.clock.tempo_at(self.renderer.playhead)),
            )
        } else {
            self.metronome.next_mapped(frame, &self.renderer.clock)
        };
        let audition = self.audition.next();
        // Worker activation deadlines use the project sample clock. A disk
        // hold must not move that clock ahead of the transport/DSP state.
        if !self.renderer.buffering() {
            self.rendered_frames += 1;
        }
        [
            sample[0] + audition[0] + click,
            sample[1] + audition[1] + click,
        ]
    }
    fn discard_buffered_output(&mut self) {
        // Discard cached samples from before the command. Keep filter history so
        // the renderer's transport fade remains continuous through resampling.
        self.index = self.output_count;
    }
    fn next(&mut self) -> [f32; 2] {
        if self.converter.is_some() {
            while self.index >= self.output_count {
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
                // Cover both device demand and the converter's full input batch.
                let frames = callback.audition_chunk_frames(data.len() / channels, device_rate);
                callback.audition.set_stream_chunk_frames(frames);
                callback.update();
                callback.renderer.reset_track_peaks();
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
                let release = meters::release_for_frames(data.len() / channels, device_rate);
                // Cached converter output still advances display time. No new
                // input peaks means release, rather than a jump to silence.
                callback.renderer.publish_track_peaks(release);
                callback.status.meter.publish_block_peaks(peaks, release);
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
            rendered_audio: None,
            compound_audio: None,
            beat_pattern: None,
            id,
            name: "Test".into(),
            source: Source {
                kind: "external".into(),
                path: "/test.wav".into(),
                storage: None,
                path_kind: "absolute".into(),
            },
            source_metadata: metadata.clone(),
            decoded_frame_count: 48_000,
        });
        project.tracks[0].clips.push(Clip {
            channels: Default::default(),
            fades: Default::default(),
            muted: false,
            stretch: None,
            id: Id::new_v4(),
            asset_id: id,
            name: "Test".into(),
            color: None,
            start_frame: 0,
            source_offset_frame: 0,
            length_frames: 48_000,
            repeat: None,
            gain_db: 0.0,
            transpose: 0,
        });
        RenderPlan {
            external: Default::default(),
            beat_tracks: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: HashMap::new(),
            project,
            audio: HashMap::from([(
                id,
                daw_media::AudioData {
                    disk: None,
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
        prepared.publish_track_peaks(0.0);
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
        updated.publish_track_peaks(0.0);
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
            external: Default::default(),
            beat_tracks: Default::default(),
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
                external: Default::default(),
                beat_tracks: Default::default(),
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
            assert!(callback.renderer.playing);
            for _ in 0..10_000 {
                assert_eq!(callback.next(), [0.0; 2]);
            }
            assert!(callback.renderer.playing);
            assert!(callback.renderer.playhead > 0);
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
mod gain_reduction_meter_tests {
    use super::*;

    fn effect(limiter: bool) -> daw_core::EffectInstance {
        let mut effect = if limiter {
            daw_core::EffectInstance::limiter()
        } else {
            daw_core::EffectInstance::compressor()
        };
        match &mut effect.unit {
            daw_core::EffectSettings::Compressor(settings) => {
                settings.threshold_db = -48.0;
                settings.attack_ms = 0.1;
            }
            daw_core::EffectSettings::Limiter(settings) => {
                settings.input_db = 24.0;
                settings.ceiling_db = -24.0;
            }
            _ => unreachable!(),
        }
        effect
    }

    #[test]
    fn layer_and_main_effects_capture_peaks_through_shared_bindings() {
        for kind in 0..3 {
            let mut plan = crate::tests::transport_plan();
            let track = plan.project.add_midi_track().unwrap();
            let clip = plan.project.add_midi_pattern(track, 0).unwrap();
            plan.project
                .edit(daw_core::Edit::ReplaceMidiPattern {
                    clip_id: clip,
                    pattern: daw_core::MidiPattern {
                        length_ticks: 3840,
                        notes: vec![daw_core::MidiNote {
                            id: Id::new_v4(),
                            start_tick: 0,
                            length_ticks: 1920,
                            pitch: 60,
                            velocity: 100,
                        }],
                    },
                })
                .unwrap();
            let track = plan
                .project
                .tracks
                .iter_mut()
                .find(|entry| entry.id == track)
                .unwrap();
            track.instrument = Some(daw_core::OscillatorSettings::default().into());
            let instrument = track.instrument.as_mut().unwrap();
            let mut effect = if kind == 2 {
                daw_core::EffectInstance::ott()
            } else {
                effect(kind == 1)
            };
            if let daw_core::EffectSettings::Ott(settings) = &mut effect.unit {
                for band in &mut settings.bands {
                    band.input_db = 24.0;
                    band.attack_ms = 0.1;
                }
            }
            let layer = effect.id;
            instrument.layers[0].effects.push(effect.clone());
            effect.id = Id::new_v4();
            let main = effect.id;
            instrument.main.effects.push(effect);
            let meters = effect_meters(&plan, &HashMap::new());
            let outputs = output_meters(&plan, &HashMap::new());
            let bands = band_meters(&plan, &HashMap::new());
            let mut renderer = Renderer::new(plan.clone());
            bind_effect_meters(&mut renderer, &meters);
            bind_band_meters(&mut renderer, &bands);
            bind_output_meters(&mut renderer, &outputs);
            renderer.play();
            crate::preparation_tests::realtime(|| {
                for _ in 0..4000 {
                    renderer.next_sample();
                }
            });
            let peak = |id| {
                if kind == 2 {
                    bands[&ModulationTarget::Effect(id)]
                        .reduction_peaks()
                        .iter()
                        .map(|p| p.peak())
                        .sum::<f32>()
                } else {
                    meters[&id].peak()
                }
            };
            for id in [layer, main] {
                assert!(peak(id) > 0.0);
                assert!(outputs[&id].readings()[0] > 0.0);
                assert!(outputs[&id].boost_peaks()[0].peak() > 0.0);
                assert!(Arc::ptr_eq(
                    &outputs[&id],
                    &output_meters(&plan, &outputs)[&id]
                ));
            }
            if kind == 0 {
                for id in [layer, main] {
                    let key = ModulationTarget::Effect(id);
                    let reading = bands[&key].readings();
                    let history = bands[&key].history().unwrap().reading().unwrap();
                    assert!(history.count > 0);
                    assert!(history.bins[..history.count].iter().any(|bin| bin[0] > 0.0));
                    assert!(reading[0] > 0.0);
                    assert!(Arc::ptr_eq(&band_meters(&plan, &bands)[&key], &bands[&key]));
                    let mut prepared = Renderer::new(plan.clone());
                    bind_band_meters(&mut prepared, &bands);
                    assert_eq!(bands[&key].readings(), reading);
                    assert_eq!(bands[&key].history().unwrap().reading().unwrap(), history);
                }
            }
            if kind == 2 {
                for peak in bands[&ModulationTarget::Effect(layer)].reduction_peaks() {
                    peak.reset_peak();
                }
            } else {
                meters[&layer].reset_peak();
            }
            assert_eq!(peak(layer), 0.0);
            assert!(outputs[&layer].boost_peaks()[0].peak() > 0.0);
            outputs[&main].boost_peaks()[0].reset_peak();
            assert!(peak(main) > 0.0);
            assert_eq!(outputs[&main].boost_peaks()[0].peak(), 0.0);
            assert!(if kind == 2 {
                Arc::ptr_eq(
                    &band_meters(&plan, &bands)[&ModulationTarget::Effect(main)],
                    &bands[&ModulationTarget::Effect(main)],
                )
            } else {
                Arc::ptr_eq(&effect_meters(&plan, &meters)[&main], &meters[&main])
            });
        }
    }

    #[test]
    fn effect_meter_handles_follow_ids_and_report_callback_compression() {
        for (limiter, master) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut plan = crate::tests::transport_plan();
            let effect = effect(limiter);
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
            let curve = band_meters(&plan, &HashMap::new());
            bind_band_meters(&mut callback.renderer, &curve);
            let mut producer = producer;
            producer
                .push(Command::Play)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            crate::preparation_tests::realtime(|| {
                for _ in 0..2000 {
                    callback.next();
                }
            });
            if !limiter {
                assert!(curve[&ModulationTarget::Effect(id)].readings()[0] > 0.0);
            }
            assert!(meters[&id].reading() > 10.0);
            assert!(meters[&id].peak() >= meters[&id].reading());
            let current = meters[&id].reading();
            meters[&id].reset_peak();
            assert_eq!(meters[&id].peak(), 0.0);
            assert_eq!(meters[&id].reading(), current);
            crate::preparation_tests::realtime(|| {
                callback.next();
            });
            assert!(meters[&id].peak() > 0.0);
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
            assert!(!callback.renderer.playing);
            // Limiter priming can retain a current reading; AudioOutput masks
            // it while stopped, independently of the held peak.
            if !limiter {
                assert_eq!(meters[&id].reading(), 0.0);
            }
            assert!(meters[&id].peak() > 0.0);
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
                daw_core::EffectInstance::chorus,
                daw_core::EffectInstance::drive,
                daw_core::EffectInstance::drive_v2,
                daw_core::EffectInstance::bitcrusher,
                daw_core::EffectInstance::saturator,
                daw_core::EffectInstance::exciter,
                daw_core::EffectInstance::filter,
                daw_core::EffectInstance::auto_filter,
                daw_core::EffectInstance::multiband_equalizer,
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
                let outputs = output_meters(&plan, &HashMap::new());
                let bands = band_meters(&plan, &HashMap::new());
                let cells: Vec<_> = scalar
                    .values()
                    .map(|meter| meter.current().as_ref())
                    .chain(
                        bands
                            .values()
                            .chain(outputs.values())
                            .flat_map(|m| m.current().iter()),
                    )
                    .collect();
                let tracks = track_meters(&plan, &HashMap::new());
                let mut renderer = PreparedRenderer::new(plan.clone(), &tracks);
                bind_effect_meters(&mut renderer, &scalar);
                bind_band_meters(&mut renderer, &bands);
                bind_output_meters(&mut renderer, &outputs);
                renderer.play();
                for _ in 0..2000 {
                    renderer.next_sample();
                }
                assert!(cells.iter().any(|cell| cell.load(Ordering::Relaxed) != 0));
                for enabled in [false, true, false, true] {
                    match &mut effect_test_chain(&mut plan, master)[0].unit {
                        daw_core::EffectSettings::MultibandEq(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Eq3(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Chorus(s) => s.enabled = enabled,
                        daw_core::EffectSettings::AmbientReverb(s) => s.enabled = enabled,
                        daw_core::EffectSettings::ShimmerReverb(s) => s.enabled = enabled,
                        daw_core::EffectSettings::PlateReverb(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Drive(s) => s.enabled = enabled,
                        daw_core::EffectSettings::DriveV2(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Bitcrusher(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Saturator(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Exciter(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Filter(s) => s.enabled = enabled,
                        daw_core::EffectSettings::AutoFilter(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Compressor(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Limiter(s) => s.enabled = enabled,
                        daw_core::EffectSettings::Ott(s) => s.enabled = enabled,
                        _ => unreachable!(),
                    }
                    let mut next = PreparedRenderer::new(plan.clone(), &tracks);
                    bind_effect_meters(&mut next, &scalar);
                    bind_band_meters(&mut next, &bands);
                    bind_output_meters(&mut next, &outputs);
                    next.inherit(&mut renderer);
                    renderer = next;
                    if !enabled {
                        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
                        assert!(scalar.values().all(|meter| meter.peak() == 0.0));
                        assert!(
                            bands
                                .values()
                                .chain(outputs.values())
                                .flat_map(|meter| meter
                                    .reduction_peaks()
                                    .iter()
                                    .chain(meter.boost_peaks()))
                                .all(|p| p.peak() == 0.0)
                        );
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
                        assert!(scalar.values().all(|meter| meter.peak() == 0.0));
                        assert!(
                            bands
                                .values()
                                .chain(outputs.values())
                                .flat_map(|meter| meter
                                    .reduction_peaks()
                                    .iter()
                                    .chain(meter.boost_peaks()))
                                .all(|p| p.peak() == 0.0)
                        );
                        // Restore a valid zero reading before preparing the next renderer.
                        for cell in &cells {
                            cell.store(0, Ordering::Relaxed);
                        }
                    }
                }
                let retained = output_meters(&plan, &outputs);
                for (id, meter) in &outputs {
                    assert!(Arc::ptr_eq(meter, &retained[id]));
                }
                effect_test_chain(&mut plan, master).clear();
                assert!(output_meters(&plan, &outputs).is_empty());
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
            crate::preparation_tests::realtime(|| {
                for _ in 0..2000 {
                    callback.next();
                }
            });
            assert!(
                meters[&ModulationTarget::Effect(id)]
                    .current()
                    .iter()
                    .any(|m| f32::from_bits(m.load(Ordering::Relaxed)).abs()
                        > if eq { 0.01 } else { 1.0 })
            );
            if !eq {
                let peak = &meters[&ModulationTarget::Effect(id)].reduction_peaks()[0];
                assert!(peak.peak() > 0.0);
                let current = meters[&ModulationTarget::Effect(id)].readings();
                let other_peaks = meters[&ModulationTarget::Effect(id)]
                    .reduction_peaks()
                    .each_ref()
                    .map(|p| p.peak());
                peak.reset_peak();
                assert_eq!(peak.peak(), 0.0);
                assert_eq!(meters[&ModulationTarget::Effect(id)].readings(), current);
                for (i, expected) in other_peaks.into_iter().enumerate().skip(1) {
                    assert_eq!(
                        meters[&ModulationTarget::Effect(id)].reduction_peaks()[i].peak(),
                        expected
                    );
                }
                crate::preparation_tests::realtime(|| {
                    callback.next();
                });
                assert!(peak.peak() > 0.0);
                let boosts = meters[&ModulationTarget::Effect(id)].boost_peaks();
                let held = boosts.each_ref().map(|p| p.peak());
                let band = held
                    .iter()
                    .position(|value| *value > 0.0)
                    .expect("callback captures boost");
                let reductions = meters[&ModulationTarget::Effect(id)]
                    .reduction_peaks()
                    .each_ref()
                    .map(|p| p.peak());
                let current = meters[&ModulationTarget::Effect(id)].readings();
                boosts[band].reset_peak();
                assert_eq!(boosts[band].peak(), 0.0);
                assert_eq!(meters[&ModulationTarget::Effect(id)].readings(), current);
                assert_eq!(
                    meters[&ModulationTarget::Effect(id)]
                        .reduction_peaks()
                        .each_ref()
                        .map(|p| p.peak()),
                    reductions
                );
                for (i, expected) in held.into_iter().enumerate() {
                    if i != band {
                        assert_eq!(boosts[i].peak(), expected);
                    }
                }
            }
            let mut other = effect;
            other.id = Id::new_v4();
            let second = other.id;
            effect_test_chain(&mut plan, master).insert(0, other);
            let updated = band_meters(&plan, &meters);
            assert!(Arc::ptr_eq(
                &updated[&ModulationTarget::Effect(id)],
                &meters[&ModulationTarget::Effect(id)]
            ));
            assert!(!Arc::ptr_eq(
                &updated[&ModulationTarget::Effect(second)],
                &meters[&ModulationTarget::Effect(id)]
            ));
            effect_test_chain(&mut plan, master).retain(|effect| effect.id != id);
            let removed = band_meters(&plan, &updated);
            assert!(!removed.contains_key(&ModulationTarget::Effect(id)));
            assert!(Arc::ptr_eq(
                &removed[&ModulationTarget::Effect(second)],
                &updated[&ModulationTarget::Effect(second)]
            ));
            let held_boosts = meters[&ModulationTarget::Effect(id)]
                .boost_peaks()
                .each_ref()
                .map(|p| p.peak());
            producer
                .push(Command::Stop)
                .unwrap_or_else(|_| panic!("queue"));
            callback.update();
            assert!(
                meters[&ModulationTarget::Effect(id)]
                    .current()
                    .iter()
                    .all(|m| f32::from_bits(m.load(Ordering::Relaxed)) == 0.0)
            );
            if !eq {
                assert!(meters[&ModulationTarget::Effect(id)].reduction_peaks()[0].peak() > 0.0);
            }
            assert_eq!(
                meters[&ModulationTarget::Effect(id)]
                    .boost_peaks()
                    .each_ref()
                    .map(|p| p.peak()),
                held_boosts
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
    fn live_envelope_meter_publishes_release_and_final_idle_without_another_event() {
        use daw_core::{EnvelopeMode, MidiNote, MidiPattern, OscillatorSettings};
        for (mode, pause) in [
            (EnvelopeMode::PerNote, false),
            (EnvelopeMode::Retrigger, false),
            (EnvelopeMode::Legato, true),
        ] {
            let mut plan = crate::tests::transport_plan();
            plan.project.transport.playhead_frame = 0;
            plan.project.tracks[0].clips.clear();
            let track = plan.project.add_midi_track().unwrap();
            let mut instrument = daw_core::InstrumentSettings::from(OscillatorSettings::default());
            let layer = &mut instrument.layers[0];
            let envelope = &mut layer.envelopes[0];
            envelope.mode = mode;
            envelope.settings.release.time_ms = 40.0;
            let key = ModulationTarget::Envelope {
                track,
                layer: layer.id,
                unit: envelope.id,
            };
            plan.project.tracks[1].instrument = Some(instrument);
            if pause {
                let clip_id = plan.project.add_midi_pattern(track, 0).unwrap();
                plan.project
                    .edit(daw_core::Edit::ReplaceMidiPattern {
                        clip_id,
                        pattern: MidiPattern {
                            length_ticks: 960,
                            notes: vec![MidiNote {
                                id: Id::from_u128(10),
                                start_tick: 0,
                                length_ticks: 192,
                                pitch: 60,
                                velocity: 127,
                            }],
                        },
                    })
                    .unwrap();
            }
            let handles = modulation_meters(&plan, &HashMap::new());
            let meter = &handles[&key];
            let mut renderer =
                PreparedRenderer::new(plan.clone(), &track_meters(&plan, &Default::default()));
            renderer.bind_modulation(&handles);
            renderer.prepare_live_midi();
            renderer.live_midi_route(track, true);
            let stage = daw_engine::ENVELOPE_PLOT_POSITION.start;
            if pause {
                renderer.play();
                for _ in 0..5001 {
                    renderer.next_sample();
                }
                renderer.publish_track_peaks(0.0);
                assert!(meter.read(true, true).unwrap()[stage] >= 3.0);
                renderer.pause();
                assert!(meter.read(false, true).is_none());
                renderer.next_sample();
                renderer.publish_track_peaks(0.0);
                assert!(meter.read(false, true).is_none());
            }
            let token = Id::from_u128(11);
            assert!(renderer.live_midi_note(token, Some((69, 127))));
            for _ in 0..513 {
                renderer.next_sample();
            }
            renderer.live_midi_note(token, None);
            for _ in 0..127 {
                renderer.next_sample();
            }
            renderer.publish_track_peaks(0.0);
            assert!((3.0..4.0).contains(&meter.read(false, true).unwrap()[stage]));
            assert!(meter.read(false, false).is_none());
            for _ in 0..2049 {
                renderer.next_sample();
            }
            renderer.publish_track_peaks(0.0);
            let final_reading = meter.read(false, true);
            assert!(
                final_reading.is_none(),
                "{mode:?} final reading: {final_reading:?}"
            );
            assert!(f32::from_bits(meter.values[stage].load(Ordering::Relaxed)).is_nan());
        }
    }

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
        let oscillator_level = ModulationTarget::OscillatorLevel {
            track,
            layer: layer.id,
        };
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
        assert_eq!(handles.len(), 13);
        assert!(handles.contains_key(&oscillator_level));
        assert!(f32::from_bits(handles[&key].values[0].load(Ordering::Relaxed)).is_nan());
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
        renderer.publish_track_peaks(0.0);
        renderer.publish_track_peaks(0.0);
        assert_eq!(renderer.playhead, frame);
        assert_eq!(
            expected.map(f32::to_bits),
            renderer.modulation_snapshot(key).unwrap().map(f32::to_bits)
        );
        assert_eq!(
            expected.map(f32::to_bits),
            std::array::from_fn(|i| handles[&key].values[i].load(Ordering::Relaxed))
        );
        assert_eq!(
            f32::from_bits(handles[&amplitude].values[0].load(Ordering::Relaxed)),
            daw_core::SILENT_GAIN_DB
        );
        assert_eq!(
            renderer.modulation_values(drive_key).unwrap(),
            std::array::from_fn(|i| f32::from_bits(
                handles[&drive_key].values[i].load(Ordering::Relaxed)
            ))
        );
        assert!(f32::from_bits(handles[&oscillator].values[0].load(Ordering::Relaxed)).is_nan());
        assert!(
            f32::from_bits(handles[&oscillator_level].values[0].load(Ordering::Relaxed)).is_nan()
        );
        let retained = modulation_meters(&plan, &handles);
        assert!(Arc::ptr_eq(
            &retained[&oscillator_level],
            &handles[&oscillator_level]
        ));
        let eq = renderer.modulation_snapshot(eq_key).unwrap();
        assert!(eq[..7].iter().all(|v| v.is_finite()));
        assert!(eq[7..].iter().all(|v| v.is_nan()));
        assert_eq!(eq[6], 2.4);
        assert_eq!(
            eq.map(f32::to_bits),
            std::array::from_fn(|i| handles[&eq_key].values[i].load(Ordering::Relaxed))
        );
        let compressor = renderer.modulation_snapshot(compressor_key).unwrap();
        assert!(compressor[..8].iter().all(|v| v.is_finite()));
        assert!(compressor[8..].iter().all(|v| v.is_nan()));
        assert_eq!(compressor[6], 0.4);
        assert_eq!(
            compressor.map(f32::to_bits),
            std::array::from_fn(|i| handles[&compressor_key].values[i].load(Ordering::Relaxed))
        );
        let reading = renderer.modulation_snapshot(sampler).unwrap();
        assert!(reading.iter().all(|v| v.is_nan()));
        assert_eq!(
            reading.map(f32::to_bits),
            std::array::from_fn(|i| handles[&sampler].values[i].load(Ordering::Relaxed))
        );
        let limiter = renderer.modulation_snapshot(limiter_key).unwrap();
        assert!(limiter[..4].iter().all(|v| v.is_finite()));
        assert!(limiter[4..].iter().all(|v| v.is_nan()));
        assert_eq!(limiter[2], 2.3);
        assert_eq!(
            limiter.map(f32::to_bits),
            std::array::from_fn(|i| handles[&limiter_key].values[i].load(Ordering::Relaxed))
        );
        let ott = renderer.modulation_snapshot(ott_key).unwrap();
        let parameters = daw_core::ModulationParameter::OTT.len();
        assert!(ott[..parameters].iter().all(|v| v.is_finite()));
        assert!(ott[parameters..].iter().all(|v| v.is_nan()));
        assert_eq!(ott[14], 3.0);
        assert_eq!(
            ott.map(f32::to_bits),
            std::array::from_fn(|i| handles[&ott_key].values[i].load(Ordering::Relaxed))
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

#[cfg(test)]
mod multiband_meter_tests {
    use super::*;
    #[test]
    fn harmonic_branch_handles_bind_and_retain_scoped_identity() {
        let mut plan = crate::tests::transport_plan();
        let mut first = daw_core::EffectInstance::harmonic_lift();
        let daw_core::EffectSettings::HarmonicLift(s) = &mut first.unit else {
            unreachable!()
        };
        for band in &mut s.bands {
            band.enabled = true;
        }
        let band = s.bands[0].id;
        let mut second = first.clone();
        second.id = Id::new_v4();
        let first_key = ModulationTarget::EffectBand {
            effect: first.id,
            band,
        };
        let second_key = ModulationTarget::EffectBand {
            effect: second.id,
            band,
        };
        plan.project.tracks[0].effects.push(first);
        plan.project.master.effects.push(second);
        let handles = band_meters(&plan, &Default::default());
        assert_eq!(handles.len(), 14);
        assert!(!Arc::ptr_eq(&handles[&first_key], &handles[&second_key]));
        let mut renderer = Renderer::new(plan.clone());
        bind_band_meters(&mut renderer, &handles);
        renderer.play();
        for _ in 0..2048 {
            renderer.next_sample();
        }
        assert!(handles[&first_key].boost_peaks()[0].peak() > 0.);
        assert!(handles[&second_key].boost_peaks()[0].peak() > 0.);
        let retained = band_meters(&plan, &handles);
        assert!(Arc::ptr_eq(&handles[&first_key], &retained[&first_key]));
        plan.project.tracks[0].effects.clear();
        let removed = band_meters(&plan, &retained);
        assert!(!removed.contains_key(&first_key));
        assert!(Arc::ptr_eq(&handles[&second_key], &removed[&second_key]));
    }

    #[test]
    fn individual_band_handles_are_scoped_retained_pruned_and_bound_through_the_shared_path() {
        let mut plan = crate::tests::transport_plan();
        let mut first = daw_core::EffectInstance::multiband_equalizer();
        let daw_core::EffectSettings::MultibandEq(s) = &mut first.unit else {
            unreachable!()
        };
        s.bands.extend((0..40).map(|_| Default::default()));
        let band = s.bands[0].id;
        let other_band = s.bands[40].id;
        let mut second = first.clone();
        second.id = Id::new_v4();
        // Even manifests with equal local band IDs must keep different effect meters independent.
        let first_key = ModulationTarget::EffectBand {
            effect: first.id,
            band,
        };
        let last_key = ModulationTarget::EffectBand {
            effect: first.id,
            band: other_band,
        };
        let second_key = ModulationTarget::EffectBand {
            effect: second.id,
            band,
        };
        plan.project.tracks[0].effects.push(first);
        plan.project.master.effects.push(second);
        let meters = band_meters(&plan, &HashMap::new());
        assert_eq!(meters.len(), 84);
        for effect in [&first_key, &second_key] {
            let ModulationTarget::EffectBand { effect, .. } = effect else {
                unreachable!()
            };
            assert!(meters.contains_key(&ModulationTarget::Effect(*effect)));
        }
        assert!(!Arc::ptr_eq(&meters[&first_key], &meters[&second_key]));
        let mut renderer = Renderer::new(plan.clone());
        bind_band_meters(&mut renderer, &meters);
        renderer.play();
        for _ in 0..1000 {
            renderer.next_sample();
        }
        assert!(meters[&first_key].readings()[0] > 0.0);
        assert!(meters[&last_key].readings()[0] > 0.0);
        let retained = band_meters(&plan, &meters);
        assert!(Arc::ptr_eq(&retained[&first_key], &meters[&first_key]));
        let daw_core::EffectSettings::MultibandEq(s) = &mut plan.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        s.remove_band(band);
        let pruned = band_meters(&plan, &retained);
        assert!(!pruned.contains_key(&first_key));
        assert!(pruned.contains_key(&second_key));
        assert!(pruned.contains_key(&last_key));
    }
}

#[cfg(test)]
mod loudness_tests {
    use super::*;
    fn wait(m: &daw_unit_core::LoudnessMeter, seconds: f32) -> daw_unit_core::LoudnessReading {
        let start = std::time::Instant::now();
        loop {
            let r = m.reading();
            if r.values[12] >= seconds {
                return r;
            }
            assert!(start.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn loudness_handles_follow_ids_and_callback_inheritance_preserves_measurements() {
        for master in [false, true] {
            let mut plan = crate::tests::transport_plan();
            let effect = daw_core::EffectInstance::loudness();
            let id = effect.id;
            effect_test_chain(&mut plan, master).push(effect);
            let handles = loudness_meters(&plan, &HashMap::new());
            let meter = &handles[&id];
            let mut old = Renderer::new(plan.clone());
            bind_loudness_meters(&mut old, &handles);
            old.play();
            crate::preparation_tests::realtime(|| {
                for _ in 0..4800 {
                    old.next_sample();
                }
            });
            let before = wait(meter, 0.1);
            let epoch = meter.epoch();
            if let daw_core::EffectSettings::Loudness(s) =
                &mut effect_test_chain(&mut plan, master)[0].unit
            {
                s.target = 2;
            }
            let next_handles = loudness_meters(&plan, &handles);
            assert!(Arc::ptr_eq(meter, &next_handles[&id]));
            let mut next = Renderer::new(plan.clone());
            bind_loudness_meters(&mut next, &next_handles);
            crate::preparation_tests::realtime(|| next.inherit(&mut old));
            assert_eq!(meter.epoch(), epoch);
            assert_eq!(meter.reading(), before);
            crate::preparation_tests::realtime(|| {
                next.pause();
                for _ in 0..512 {
                    next.next_sample();
                }
            });
            assert_eq!(meter.epoch(), epoch);
            assert_eq!(meter.reading(), before);
            crate::preparation_tests::realtime(|| {
                next.stop();
                for _ in 0..512 {
                    next.next_sample();
                }
            });
            assert_eq!(meter.reading(), before);
            crate::preparation_tests::realtime(|| {
                next.play();
                next.next_sample();
            });
            assert_ne!(meter.epoch(), epoch);
            assert_eq!(meter.reading(), Default::default());
            effect_test_chain(&mut plan, master).clear();
            assert!(loudness_meters(&plan, &handles).is_empty());
        }
    }
    #[test]
    fn seek_replay_latency_priming_and_offline_range_skips_do_not_enter_analysis() {
        let mut plan = crate::tests::transport_plan();
        plan.project
            .master
            .effects
            .push(daw_core::EffectInstance::plate_reverb());
        plan.project
            .master
            .effects
            .push(daw_core::EffectInstance::limiter());
        let meter_effect = daw_core::EffectInstance::loudness();
        let id = meter_effect.id;
        plan.project.master.effects.push(meter_effect);
        let mut renderer = Renderer::new(plan);
        let meter = Arc::new(daw_unit_core::LoudnessMeter::default());
        renderer.set_loudness_meter(id, meter.clone());
        renderer.prepare_transport(TransportState {
            frame: 24000,
            playing: true,
            paused: false,
            playback_start: Some(0),
        });
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_eq!(meter.reading(), Default::default());
        for _ in 0..4800 {
            renderer.next_sample();
        }
        wait(&meter, 0.1);
        assert_eq!(meter.reading().values[12], 0.1);
        meter.reset();
        renderer.render_offline_at(40000, &mut [[0.0; 2]; 4800]);
        wait(&meter, 0.1);
        assert_eq!(meter.reading().values[12], 0.1);
    }
}

#[cfg(test)]
mod loudness_prepared_transport_tests {
    use super::*;
    #[test]
    fn prepared_seek_handoff_preserves_continuous_analysis_and_restarts_transport_analysis() {
        for continuous in [false, true] {
            let mut plan = crate::tests::transport_plan();
            let mut effect = daw_core::EffectInstance::loudness();
            let id = effect.id;
            if let daw_core::EffectSettings::Loudness(s) = &mut effect.unit {
                s.continuous = continuous;
            }
            plan.project.master.effects.push(effect);
            let handles = loudness_meters(&plan, &HashMap::new());
            let meter = &handles[&id];
            let mut old = Renderer::new(plan.clone());
            bind_loudness_meters(&mut old, &handles);
            old.play();
            for _ in 0..4800 {
                old.next_sample();
            }
            let start = std::time::Instant::now();
            while meter.reading().values[12] < 0.1 {
                assert!(start.elapsed().as_secs() < 5);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let epoch = meter.epoch();
            let before = meter.reading();
            let mut next = Renderer::new(plan);
            next.prepare_transport(TransportState {
                frame: 24000,
                playing: true,
                paused: false,
                playback_start: Some(0),
            });
            bind_loudness_meters(&mut next, &handles);
            crate::preparation_tests::realtime(|| {
                next.activate_transport(&old);
                next.inherit_analysis(&mut old);
            });
            assert_eq!(meter.reading(), before);
            crate::preparation_tests::realtime(|| {
                next.next_sample();
            });
            if continuous {
                assert_eq!(meter.epoch(), epoch);
                assert_eq!(meter.reading(), before);
            } else {
                assert_ne!(meter.epoch(), epoch);
                assert_eq!(meter.reading(), Default::default());
            }
        }
    }
}

#[cfg(test)]
mod metronome_tests;
