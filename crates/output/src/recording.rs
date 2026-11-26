//! Device capture and the playback timing bridge. Neither callback owns files.
use crate::{DeviceId, devices};
use cpal::traits::{DeviceTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const CHANNEL_LIMIT: usize = 16;
const FRAME_CAPACITY: usize = 65_536;
const HALF_FILTER: usize = 16;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputChannels {
    /// Zero-based physical channel.
    pub first: u16,
    pub stereo: bool,
}

#[derive(Clone, Debug)]
pub struct InputRequest {
    pub device: Option<DeviceId>,
    pub routes: Vec<InputChannels>,
}

#[derive(Clone, Copy)]
struct Anchor {
    frame: u64,
    time_ns: i64,
}

#[derive(Clone, Copy)]
pub(super) enum Timing {
    Start {
        ordinal: u64,
        frame: u64,
    },
    End {
        ordinal: u64,
    },
    Output {
        frame: u64,
        count: usize,
        time_ns: i64,
    },
}

pub struct ClockPort {
    pub(super) origin: Instant,
    reader: Mutex<Option<Consumer<Timing>>>,
    writer: Mutex<Option<Producer<Timing>>>,
    armed: AtomicBool,
    started: AtomicBool,
    fault: AtomicBool,
    device_rate: AtomicU32,
    delay: AtomicU32,
}

impl Default for ClockPort {
    fn default() -> Self {
        let (writer, reader) = RingBuffer::new(8192);
        Self {
            origin: Instant::now(),
            reader: Mutex::new(Some(reader)),
            writer: Mutex::new(Some(writer)),
            armed: AtomicBool::new(false),
            started: AtomicBool::new(false),
            fault: AtomicBool::new(false),
            device_rate: AtomicU32::new(0),
            delay: AtomicU32::new(0),
        }
    }
}

impl ClockPort {
    pub(super) fn reset_before_stream_retry(&self) {
        let (writer, reader) = RingBuffer::new(8192);
        *self.writer.lock().unwrap() = Some(writer);
        *self.reader.lock().unwrap() = Some(reader);
    }

    pub(super) fn writer(&self, rate: u32, delay: usize) -> Producer<Timing> {
        self.device_rate.store(rate, Ordering::Release);
        self.delay.store(delay as u32, Ordering::Release);
        self.writer.lock().unwrap().take().unwrap()
    }

    pub(super) fn armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }

    pub(super) fn fault(&self) {
        if self.armed() {
            self.fault.store(true, Ordering::Release);
        }
    }

    pub(super) fn send(&self, writer: &mut Producer<Timing>, timing: Timing) {
        if self.armed() {
            if matches!(timing, Timing::Start { .. }) {
                self.started.store(true, Ordering::Release);
            }
            if writer.push(timing).is_err() {
                self.fault();
            }
        }
    }

    fn subscribe(self: &Arc<Self>) -> Result<Subscription, String> {
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| "Recording clock unavailable")?;
        let mut reader = reader
            .take()
            .ok_or("A capture already owns the playback clock")?;
        while reader.pop().is_ok() {}
        self.fault.store(false, Ordering::Release);
        self.started.store(false, Ordering::Release);
        self.armed.store(true, Ordering::Release);
        Ok(Subscription {
            port: self.clone(),
            reader: Some(reader),
        })
    }
}

struct Subscription {
    port: Arc<ClockPort>,
    reader: Option<Consumer<Timing>>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.port.armed.store(false, Ordering::Release);
        if let Ok(mut slot) = self.port.reader.lock() {
            *slot = self.reader.take();
        }
    }
}

/// Express backend timestamps on a shared process clock without assuming that
/// two CPAL streams have the same StreamInstant epoch.
#[derive(Default)]
pub(super) struct TimestampBridge {
    native: Option<cpal::StreamInstant>,
    offset_ns: i64,
}

fn relative_ns(later: cpal::StreamInstant, earlier: cpal::StreamInstant) -> Option<i64> {
    if let Some(delta) = later.duration_since(&earlier) {
        i64::try_from(delta.as_nanos()).ok()
    } else {
        i64::try_from(earlier.duration_since(&later)?.as_nanos())
            .ok()
            .map(|n| -n)
    }
}

impl TimestampBridge {
    pub(super) fn timestamp(
        &mut self,
        process_epoch: Instant,
        observed: Instant,
        callback: cpal::StreamInstant,
        boundary: cpal::StreamInstant,
        calibrating: bool,
    ) -> Option<i64> {
        let elapsed =
            i64::try_from(observed.checked_duration_since(process_epoch)?.as_nanos()).ok()?;
        if self.native.is_none() {
            self.native = Some(callback);
            self.offset_ns = elapsed;
        }
        let delta = relative_ns(callback, self.native?)?;
        let offset = elapsed.checked_sub(delta)?;
        if calibrating {
            self.offset_ns = self.offset_ns.min(offset);
        }
        // A stalled or discontinuous clock must not silently move the take.
        if (offset - self.offset_ns).abs() > 250_000_000 {
            return None;
        }
        self.offset_ns
            .checked_add(relative_ns(boundary, self.native?)?)
    }
}

#[derive(Clone, Copy)]
struct InputFrame([f32; CHANNEL_LIMIT]);

pub struct Capture {
    _input_owner: InputOwner,
    playback: Subscription,
    input: Consumer<InputFrame>,
    timestamps: Consumer<Anchor>,
    failed: Arc<AtomicBool>,
    pub peaks: Vec<Arc<AtomicU32>>,
    input_rate: u32,
    input_channels: Vec<u16>,
    pub channels: Vec<InputChannels>,
    alignment: Alignment,
    last_input: Instant,
}

fn config(
    device: &cpal::Device,
    required_channels: u16,
    required_rate: u32,
) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device.default_input_config().map_err(|e| e.to_string())?;
    device
        .supported_input_configs()
        .map_err(|e| e.to_string())?
        .filter(|c| {
            c.channels() >= required_channels
                && devices::sample_format_from_cpal(c.sample_format()).is_some()
        })
        .map(|c| {
            let r = required_rate.clamp(c.min_sample_rate().0, c.max_sample_rate().0);
            c.with_sample_rate(cpal::SampleRate(r))
        })
        .min_by_key(|c| {
            (
                c.sample_rate().0.abs_diff(required_rate),
                c.channels().abs_diff(default.channels()),
                c.sample_format() != default.sample_format(),
            )
        })
        .ok_or_else(|| "The input device does not support the selected channels".into())
}

impl Capture {
    pub fn open(
        clock: &Arc<ClockPort>,
        project_rate: u32,
        request: InputRequest,
    ) -> Result<Self, String> {
        let mut selected = Vec::new();
        if request.routes.is_empty() || request.routes.len() > 16 {
            return Err("Audio recording supports 1–16 armed audio tracks per take".into());
        }
        for route in &request.routes {
            selected.push(route.first);
            if route.stereo {
                selected.push(
                    route
                        .first
                        .checked_add(1)
                        .ok_or("Invalid stereo input channel pair")?,
                );
            }
        }
        selected.sort_unstable();
        selected.dedup();
        if selected.len() > CHANNEL_LIMIT {
            return Err("Audio recording supports at most 16 distinct input channels".into());
        }
        let (frames_tx, input) = RingBuffer::new(FRAME_CAPACITY);
        let (stamps_tx, timestamps) = RingBuffer::new(8192);
        let failed = Arc::new(AtomicBool::new(false));
        let peaks: Vec<_> = request
            .routes
            .iter()
            .map(|_| Arc::new(AtomicU32::new(0)))
            .collect();
        let route_indexes: Vec<_> = request
            .routes
            .iter()
            .map(|route| {
                let first = selected.binary_search(&route.first).unwrap();
                (first, first + usize::from(route.stereo))
            })
            .collect();
        let writer = InputWriter {
            frames: frames_tx,
            stamps: stamps_tx,
            selected: selected.clone(),
            failed: failed.clone(),
            peaks: peaks.clone(),
            route_indexes,
            clock: clock.clone(),
        };
        let (input_owner, input_rate) = InputOwner::open(writer, request.device, project_rate)?;
        let playback = clock.subscribe()?;
        Ok(Self {
            _input_owner: input_owner,
            playback,
            input,
            timestamps,
            failed,
            peaks,
            input_rate,
            input_channels: selected,
            channels: request.routes,
            alignment: Alignment::new(
                project_rate,
                clock.device_rate.load(Ordering::Acquire),
                clock.delay.load(Ordering::Acquire),
            ),
            last_input: Instant::now(),
        })
    }

    /// Used on the worker before asking the transport to start. No device calls
    /// or sleeps are made on the UI or playback callback.
    pub fn prepare(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while self.alignment.input_times.len() < 8 || self.alignment.output_times.len() < 3 {
            self.drain();
            if self.faulted() || Instant::now() >= deadline {
                return Err("The selected audio input and output could not establish a recording clock. Check device access and routing.".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    pub fn faulted(&self) -> bool {
        self.failed.load(Ordering::Acquire)
            || self.playback.port.fault.load(Ordering::Acquire)
            || self.alignment.fault
            || self.last_input.elapsed() > STARTUP_TIMEOUT
    }

    pub fn origin(&self) -> Option<u64> {
        self.alignment.start.map(|(_, frame)| frame)
    }

    pub fn delivered(&self) -> u64 {
        self.alignment.written
    }

    pub fn ended(&self) -> bool {
        self.alignment
            .end
            .zip(self.alignment.start)
            .is_some_and(|(end, (start, _))| self.alignment.written >= end.saturating_sub(start))
    }

    pub fn poll(
        &mut self,
        write: impl FnMut(&[[f32; 2]]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.poll_through(u64::MAX, write)
    }

    pub fn poll_through(
        &mut self,
        exclusive_frame: u64,
        mut write: impl FnMut(&[[f32; 2]]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.drain();
        let mut routes = [[0.0; 2]; 16];
        let mut count = 0;
        while count < 8192
            && self.origin().is_some_and(|origin| {
                origin
                    .checked_add(self.delivered())
                    .is_some_and(|next| next < exclusive_frame)
            })
        {
            let Some(samples) = self.alignment.next(self.input_rate) else {
                break;
            };
            for (i, route) in self.channels.iter().enumerate() {
                let first = self.input_channels.binary_search(&route.first).unwrap();
                routes[i] = [samples[first], samples[first + usize::from(route.stereo)]];
            }
            write(&routes[..self.channels.len()])?;
            self.alignment.written += 1;
            count += 1;
        }
        Ok(())
    }

    fn drain(&mut self) {
        let a = &mut self.alignment;
        while let Ok(event) = self.playback.reader.as_mut().unwrap().pop() {
            match event {
                Timing::Start { ordinal, frame } => a.start = Some((ordinal, frame)),
                Timing::End { ordinal } => a.end = Some(ordinal),
                Timing::Output {
                    frame,
                    count,
                    time_ns,
                } => {
                    if let Some(previous) = a.output_next
                        && frame != previous
                        && a.start.is_some_and(|(start, _)| {
                            frame as f64 > a.output_at(start)
                                && a.end.is_none_or(|end| (previous as f64) < a.output_at(end))
                        })
                    {
                        a.fault = true;
                        break;
                    }
                    a.output_next = frame.checked_add(count as u64);
                    a.output_times.push_back(Anchor { frame, time_ns });
                }
            }
        }
        while let Ok(anchor) = self.timestamps.pop() {
            a.input_times.push_back(anchor);
            self.last_input = Instant::now();
        }
        for _ in 0..FRAME_CAPACITY {
            let Ok(frame) = self.input.pop() else { break };
            a.input.push_back(frame);
        }
        // Retain startup/count-in history only long enough for the first
        // hardware anchor and the interpolation kernel that straddles it.
        if a.start.is_none() {
            while a.output_times.len() > 32 {
                a.output_times.pop_front();
            }
            while a.input_times.len() > 32 {
                a.input_times.pop_front();
            }
            while a.input.len() > (self.input_rate as usize / 2).min(FRAME_CAPACITY) {
                a.input.pop_front();
                a.input_base += 1;
            }
        }
        if a.input.len() > FRAME_CAPACITY * 2
            || a.output_times.len() > 8192
            || a.input_times.len() > 8192
        {
            a.fault = true;
        }
    }
}

struct InputWriter {
    frames: Producer<InputFrame>,
    stamps: Producer<Anchor>,
    selected: Vec<u16>,
    failed: Arc<AtomicBool>,
    peaks: Vec<Arc<AtomicU32>>,
    route_indexes: Vec<(usize, usize)>,
    clock: Arc<ClockPort>,
}
struct InputOwner {
    stop: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl InputOwner {
    fn open(
        writer: InputWriter,
        device: Option<DeviceId>,
        project_rate: u32,
    ) -> Result<(Self, u32), String> {
        let (stop, close) = mpsc::channel();
        let (ready, result) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("audio-input".into())
            .spawn(move || match writer.open(device.as_ref(), project_rate) {
                Ok((stream, rate)) => {
                    if ready.send(Ok(rate)).is_ok() {
                        let _ = close.recv();
                    }
                    drop(stream);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
            .map_err(|e| e.to_string())?;
        // Device APIs can wait on the native microphone permission prompt.
        // If that wait outlives preflight, the disconnected receiver makes
        // the owner drop any stream the OS returns later.
        let rate = result
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "Timed out opening the audio input. Check microphone permission.")??;
        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            rate,
        ))
    }
}
impl Drop for InputOwner {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl InputWriter {
    fn open(
        self,
        selection: Option<&DeviceId>,
        project_rate: u32,
    ) -> Result<(cpal::Stream, u32), String> {
        let device = devices::input_device(selection)?;
        let required = self
            .selected
            .last()
            .copied()
            .unwrap()
            .checked_add(1)
            .ok_or("Invalid input channel")?;
        let selected = config(&device, required, project_rate)?;
        let format = selected.sample_format();
        let rate = selected.sample_rate().0;
        let config = cpal::StreamConfig {
            buffer_size: cpal::BufferSize::Default,
            ..selected.into()
        };
        macro_rules! stream {
            ($t:ty) => {
                self.build::<$t>(&device, &config)
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
            _ => return Err("Unsupported audio input sample format".into()),
        }?;
        stream
            .play()
            .map_err(|e| format!("Cannot start the selected audio input: {e}"))?;
        Ok((stream, rate))
    }

    fn build<T>(
        mut self,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let device_channels = usize::from(config.channels);
        let failed = self.failed.clone();
        let mut position = 0u64;
        let mut bridge = TimestampBridge::default();
        device
            .build_input_stream(
                config,
                move |data: &[T], info| {
                    if self.failed.load(Ordering::Relaxed) || data.is_empty() {
                        return;
                    }
                    let stamp = info.timestamp();
                    let Some(time_ns) = bridge.timestamp(
                        self.clock.origin,
                        Instant::now(),
                        stamp.callback,
                        stamp.capture,
                        !self.clock.started.load(Ordering::Acquire),
                    ) else {
                        self.failed.store(true, Ordering::Release);
                        return;
                    };
                    if !data.len().is_multiple_of(device_channels)
                        || self
                            .stamps
                            .push(Anchor {
                                frame: position,
                                time_ns,
                            })
                            .is_err()
                    {
                        self.failed.store(true, Ordering::Release);
                        return;
                    }
                    let mut peak = [0.0f32; CHANNEL_LIMIT];
                    for frame in data.chunks_exact(device_channels) {
                        let mut samples = [0.0; CHANNEL_LIMIT];
                        for (i, &channel) in self.selected.iter().enumerate() {
                            let sample =
                                <f32 as cpal::Sample>::from_sample(frame[usize::from(channel)]);
                            if !sample.is_finite() {
                                self.failed.store(true, Ordering::Release);
                                return;
                            }
                            peak[i] = peak[i].max(sample.abs());
                            samples[i] = sample;
                        }
                        if self.frames.push(InputFrame(samples)).is_err() {
                            self.failed.store(true, Ordering::Release);
                            return;
                        }
                        position += 1;
                    }
                    for (value, &(left, right)) in self.peaks.iter().zip(&self.route_indexes) {
                        value.store(peak[left].max(peak[right]).to_bits(), Ordering::Relaxed);
                    }
                },
                move |_| {
                    failed.store(true, Ordering::Release);
                },
                None,
            )
            .map_err(|e| format!("Cannot open the selected audio input: {e}"))
    }
}

struct Alignment {
    project_rate: u32,
    device_rate: u32,
    delay: u32,
    start: Option<(u64, u64)>,
    end: Option<u64>,
    written: u64,
    output_next: Option<u64>,
    output_times: VecDeque<Anchor>,
    input_times: VecDeque<Anchor>,
    input: VecDeque<InputFrame>,
    input_base: u64,
    fault: bool,
}

impl Alignment {
    fn new(project_rate: u32, device_rate: u32, delay: u32) -> Self {
        Self {
            project_rate,
            device_rate,
            delay,
            start: None,
            end: None,
            written: 0,
            output_next: None,
            output_times: VecDeque::new(),
            input_times: VecDeque::new(),
            input: VecDeque::new(),
            input_base: 0,
            fault: false,
        }
    }

    fn output_at(&self, ordinal: u64) -> f64 {
        ordinal as f64 * f64::from(self.device_rate) / f64::from(self.project_rate)
            + f64::from(self.delay)
    }

    fn next(&mut self, input_rate: u32) -> Option<[f32; CHANNEL_LIMIT]> {
        let (origin, _) = self.start?;
        let ordinal = origin.checked_add(self.written)?;
        if self.fault || self.end.is_some_and(|end| ordinal >= end) {
            return None;
        }
        let output = self.output_at(ordinal);
        let time = interpolate(&mut self.output_times, output, false, &mut self.fault)?;
        let input = interpolate(&mut self.input_times, time, true, &mut self.fault)?;
        let middle = input.floor() as u64;
        let first = middle.saturating_sub(HALF_FILTER as u64 - 1);
        if first < self.input_base {
            self.fault = true;
            return None;
        }
        if middle.saturating_add(HALF_FILTER as u64) >= self.input_base + self.input.len() as u64 {
            return None;
        }
        while self.input_base < first {
            self.input.pop_front();
            self.input_base += 1;
        }
        let scale = (f64::from(self.project_rate) / f64::from(input_rate)).min(1.0);
        let mut result = [0.0f64; CHANNEL_LIMIT];
        let mut total = 0.0;
        for i in first..=middle + HALF_FILTER as u64 {
            let delta = input - i as f64;
            let phase = std::f64::consts::PI * delta * scale;
            let sinc = if phase.abs() < 1e-9 {
                1.0
            } else {
                phase.sin() / phase
            };
            let window = 0.5 + 0.5 * (std::f64::consts::PI * delta / HALF_FILTER as f64).cos();
            let coefficient = scale * sinc * window;
            total += coefficient;
            for (sum, value) in result
                .iter_mut()
                .zip(self.input[(i - self.input_base) as usize].0)
            {
                *sum += f64::from(value) * coefficient;
            }
        }
        Some(result.map(|value| (value / total) as f32))
    }
}

fn interpolate(
    anchors: &mut VecDeque<Anchor>,
    x: f64,
    inverse: bool,
    fault: &mut bool,
) -> Option<f64> {
    let coordinate = |a: Anchor| {
        if inverse {
            a.time_ns as f64
        } else {
            a.frame as f64
        }
    };
    let value = |a: Anchor| {
        if inverse {
            a.frame as f64
        } else {
            a.time_ns as f64
        }
    };
    loop {
        let a = *anchors.front()?;
        let b = *anchors.get(1)?;
        let l = coordinate(a);
        let r = coordinate(b);
        if r <= l || value(b) <= value(a) || x < l {
            *fault = true;
            return None;
        }
        if x <= r {
            return Some(value(a) + (value(b) - value(a)) * (x - l) / (r - l));
        }
        anchors.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rubato::{FftFixedIn, Resampler};

    fn looped_signal(project_rate: u32, output_rate: u32, source: &[f32]) -> (Vec<f32>, u32) {
        if project_rate == output_rate {
            return (source.to_vec(), 0);
        }
        let mut converter =
            FftFixedIn::<f32>::new(project_rate as usize, output_rate as usize, 512, 2, 1).unwrap();
        let delay = converter.output_delay() as u32;
        let mut rendered = Vec::new();
        for chunk in source.chunks_exact(512) {
            rendered.extend(converter.process(&[chunk], None).unwrap().remove(0));
        }
        (rendered, delay)
    }

    #[test]
    fn simulated_loopback_matches_project_impulses_across_device_rates_and_drift() {
        const DEVICE_RATE: u32 = 48_000;
        const INPUT_RATE: u32 = 44_100;
        const BASE_NS: f64 = 1_000_000_000.0;
        for rate in daw_core::SAMPLE_RATES {
            for drift in [-170.0, 240.0] {
                let origin = u64::from(rate / 40 + 31);
                let length = u64::from(rate / 10 + 17);
                let a = origin + u64::from(rate / 97);
                let b = origin + length - u64::from(rate / 173);
                let mut original = vec![0.0; (rate as usize / 5).next_multiple_of(512)];
                original[a as usize] = 1.0;
                original[b as usize] = -0.7;
                let (dac, delay) = looped_signal(rate, DEVICE_RATE, &original);
                let mut alignment = Alignment::new(rate, DEVICE_RATE, delay);
                alignment.start = Some((origin, 21_001));
                alignment.end = Some(origin + length);
                // DAC and ADC use unrelated block starts and an independent ADC oscillator.
                for frame in (0..dac.len()).step_by(127) {
                    alignment.output_times.push_back(Anchor {
                        frame: frame as u64,
                        time_ns: (BASE_NS + frame as f64 * 1e9 / f64::from(DEVICE_RATE)).round()
                            as i64,
                    });
                }
                let measured_input_rate = f64::from(INPUT_RATE) * (1.0 + drift * 1e-6);
                for frame in 0..(INPUT_RATE as usize / 5) {
                    let elapsed = (frame as f64 + 0.37) / measured_input_rate;
                    if frame.is_multiple_of(113) {
                        alignment.input_times.push_back(Anchor {
                            frame: frame as u64,
                            time_ns: (BASE_NS + elapsed * 1e9).round() as i64,
                        });
                    }
                    let axis = elapsed * f64::from(DEVICE_RATE);
                    let first = axis.floor() as usize;
                    let u = (axis - first as f64) as f32;
                    let sample = dac.get(first).copied().unwrap_or(0.0) * (1.0 - u)
                        + dac.get(first + 1).copied().unwrap_or(0.0) * u;
                    let mut routed = [0.0; CHANNEL_LIMIT];
                    routed[0] = sample;
                    alignment.input.push_back(InputFrame(routed));
                }
                let mut observed = Vec::new();
                while let Some(samples) = alignment.next(INPUT_RATE) {
                    observed.push(samples[0]);
                    alignment.written += 1;
                }
                assert!(!alignment.fault, "clock fault for {rate} Hz");
                assert_eq!(
                    observed.len() as u64,
                    length,
                    "capture interval for {rate} Hz"
                );
                let positive = observed
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0 as u64;
                let negative = observed
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0 as u64;
                assert!(
                    positive.abs_diff(a - origin) <= 4,
                    "{rate}: positive pulse expected {}, got {positive}",
                    a - origin
                );
                assert!(
                    negative.abs_diff(b - origin) <= 4,
                    "{rate}: negative pulse expected {}, got {negative}",
                    b - origin
                );
            }
        }
    }

    #[test]
    fn rejects_reversed_native_clock_without_extrapolating_past_the_last_anchor() {
        let mut anchors = VecDeque::from([
            Anchor {
                frame: 0,
                time_ns: 100,
            },
            Anchor {
                frame: 12,
                time_ns: 90,
            },
        ]);
        let mut fault = false;
        assert!(interpolate(&mut anchors, 11.0, false, &mut fault).is_none());
        assert!(fault);
        let mut valid = VecDeque::from([
            Anchor {
                frame: 0,
                time_ns: 100,
            },
            Anchor {
                frame: 12,
                time_ns: 200,
            },
        ]);
        let mut fault = false;
        assert!(interpolate(&mut valid, 13.0, false, &mut fault).is_none());
        assert!(!fault);
        valid.push_back(Anchor {
            frame: 24,
            time_ns: 300,
        });
        assert!(
            (interpolate(&mut valid, 13.0, false, &mut fault).unwrap() - 208.3333333333333).abs()
                < 1e-9
        );
    }

    #[test]
    fn simulated_loopback_tracks_drift_at_thirty_minutes() {
        let rate = 48_000;
        let origin = 8192;
        let offset = 30 * 60 * u64::from(rate);
        let presentation = origin + offset;
        let reference_ns = presentation as f64 * 1e9 / f64::from(rate);
        for ppm in [-240.0, 240.0] {
            let actual = f64::from(rate) * (1.0 + ppm * 1e-6);
            let sample_center = reference_ns * actual / 1e9 + 0.39;
            let center = sample_center.floor() as u64;
            let mut alignment = Alignment::new(rate, rate, 0);
            alignment.start = Some((origin, 300));
            alignment.end = Some(presentation + 1);
            alignment.written = offset;
            alignment.input_base = center - 64;
            for frame in [presentation - 70, presentation + 53] {
                alignment.output_times.push_back(Anchor {
                    frame,
                    time_ns: (frame as f64 * 1e9 / f64::from(rate)).round() as i64,
                });
            }
            for frame in center - 64..=center + 64 {
                if frame == center - 64 || frame == center + 64 {
                    alignment.input_times.push_back(Anchor {
                        frame,
                        time_ns: ((frame as f64 - 0.39) * 1e9 / actual).round() as i64,
                    });
                }
                let x = (frame as f64 - sample_center) / 3.0;
                let mut values = [0.0; CHANNEL_LIMIT];
                values[0] = (-x * x).exp() as f32;
                alignment.input.push_back(InputFrame(values));
            }
            assert!(alignment.next(rate).unwrap()[0] > 0.99);
            alignment.written += 1;
            assert!(alignment.next(rate).is_none(), "Stop is exclusive");
            assert!(!alignment.fault);
        }
    }

    #[test]
    #[ignore = "requires a wired pair or a dedicated virtual loopback device"]
    fn native_wired_loopback() {
        use crate::{AudioOutput, AudioSettings, MidiAction, discover_devices};
        let named = |variable: &str, devices: &[crate::DeviceInfo]| -> DeviceId {
            let name = std::env::var(variable).unwrap_or_else(|_| {
                panic!("{variable} must name an explicitly wired loopback endpoint")
            });
            let matches: Vec<_> = devices.iter().filter(|d| d.id.name == name).collect();
            assert_eq!(
                matches.len(),
                1,
                "loopback device must be unambiguous: {name}"
            );
            matches[0].id.clone()
        };
        let devices = discover_devices().unwrap();
        let input = named("DAW_LOOPBACK_INPUT", &devices.inputs);
        let output = named("DAW_LOOPBACK_OUTPUT", &devices.outputs);
        let rate = std::env::var("DAW_LOOPBACK_RATE").map_or(48_000, |s| s.parse::<u32>().unwrap());
        assert!(daw_core::SAMPLE_RATES.contains(&rate));
        let mut plan = crate::tests::transport_plan();
        plan.project.sample_rate_hz = rate;
        let asset = &mut plan.project.assets[0];
        asset.source_metadata.sample_rate_hz = rate;
        asset.decoded_frame_count = u64::from(rate);
        plan.project.tracks[0].clips[0].length_frames = u64::from(rate);
        let origin = rate / 10;
        let pulse = rate / 5;
        plan.project.transport.playhead_frame = u64::from(origin);
        let data = plan.audio.get_mut(&asset.id).unwrap();
        let mut samples = vec![[0.0; 2]; rate as usize];
        samples[(origin + pulse) as usize] = [0.125; 2];
        data.sample_rate_hz = rate;
        data.metadata.sample_rate_hz = rate;
        data.samples = Arc::new(samples);
        let mut playback = AudioOutput::with_settings(
            plan,
            None,
            &AudioSettings {
                output_device: Some(output),
                input_device: Some(input.clone()),
            },
        )
        .unwrap();
        let ready = Instant::now() + Duration::from_secs(8);
        let mut sender = loop {
            playback.drain();
            if let Some(sender) = playback.take_midi_sender() {
                break sender;
            }
            assert!(Instant::now() < ready, "playback did not prepare");
            std::thread::sleep(Duration::from_millis(2));
        };
        let mut capture = Capture::open(
            &sender.recording_clock(),
            rate,
            InputRequest {
                device: Some(input),
                routes: vec![InputChannels::default()],
            },
        )
        .unwrap();
        capture.prepare().unwrap();
        sender
            .send(MidiAction::Record { count_in_frames: 0 })
            .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        let mut peak = (0, 0.0f32);
        let mut count = 0;
        let mut stop = false;
        while !capture.ended() && Instant::now() < end {
            playback.drain();
            while sender.receipt().is_some() {}
            capture
                .poll(|samples| {
                    let sample = samples[0][0].abs();
                    if sample > peak.1 {
                        peak = (count, sample);
                    }
                    count += 1;
                    Ok(())
                })
                .unwrap();
            assert!(
                !capture.faulted(),
                "loopback capture reported a clock or input fault"
            );
            if !stop && count >= rate / 2 {
                sender.send(MidiAction::Stop).unwrap();
                stop = true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            capture.ended(),
            "loopback did not reach the exclusive Stop boundary"
        );
        assert!(peak.1 >= 0.001, "no loopback signal detected");
        let signed = i64::from(peak.0) - i64::from(pulse);
        println!(
            "{}; project {rate} Hz, captured impulse {signed} frames ({:.3} ms) from reference",
            playback.description,
            signed as f64 * 1000.0 / f64::from(rate)
        );
        assert!(
            signed.abs() <= i64::from(rate * 3 / 1000),
            "loopback exceeds the proposed 3 ms alignment budget"
        );
    }
}
