//! Transparent stereo loudness analysis. Prepare and retire workers outside audio.
mod filter;
mod offline;
pub use offline::{MeasureMode, OfflineAnalyzer};
mod statistics;
use daw_unit_core::{
    BandMeter, EffectUnit, LevelMonitor, LoudnessMeter, LoudnessReading, LoudnessSettings,
    PortKind, ProcessingContext, UnitDescriptor, UnitMeter,
};
use rtrb::{Producer, RingBuffer};
struct WorkerMeter(Arc<LoudnessMeter>);
impl daw_unit_core::MeterStorage for WorkerMeter {
    type Reading = (u64, LoudnessReading);
    const ZERO: Self::Reading = (
        0,
        LoudnessReading {
            values: [0.0; daw_unit_core::LOUDNESS_VALUES],
        },
    );
    fn store(&self, r: Self::Reading) {
        self.0.publish(r.0, r.1);
    }
    fn initialize(&self, _: Self::Reading) {}
    fn clear(&self) {
        self.0.reset();
    }
}
use filter::{Biquad, TruePeak};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy)]
struct Observation {
    epoch: u64,
    m: f64,
    s: f64,
    peak: f64,
    max_m: f64,
    max_s: f64,
    seconds: f64,
    ticks: u64,
    incomplete: bool,
}
const WORKER_STACK: usize = 256 * 1024;
struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    queue: Producer<Observation>,
    failed: bool,
}
impl Worker {
    fn new(meter: Arc<LoudnessMeter>) -> Self {
        let (queue, mut consumer) = RingBuffer::<Observation>::new(512);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let target = meter.clone();
        let started = std::thread::Builder::new()
            .name("loudness".into())
            .stack_size(WORKER_STACK)
            .spawn(move || {
                let meter = target;
                let mut stats = statistics::Statistics::new();
                let mut unit = UnitMeter::new(true);
                unit.attach(
                    Arc::new(WorkerMeter(meter.clone())),
                    (0, LoudnessReading::default()),
                );
                while !flag.load(Ordering::Acquire) {
                    let mut consumed = false;
                    while let Ok(o) = consumer.pop() {
                        if flag.load(Ordering::Acquire) {
                            break;
                        }
                        if o.epoch == meter.epoch() {
                            let r = stats.accept(o);
                            unit.publish((o.epoch, r));
                        }
                        consumed = true;
                    }
                    if !consumed {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                }
            });
        let failed = started.is_err();
        if failed {
            meter.mark_incomplete(meter.epoch());
        }
        Self {
            stop,
            thread: started.ok(),
            queue,
            failed,
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Hosts retire processors outside audio processing. Finish all worker
        // code and thread-local cleanup before they can unload the plugin library.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub struct Loudness {
    context: ProcessingContext,
    settings: LoudnessSettings,
    meter: Arc<LoudnessMeter>,
    worker: Option<Worker>,
    shelf: [Biquad; 2],
    highpass: [Biquad; 2],
    tp: TruePeak,
    powers: Box<[f64]>,
    index: usize,
    count: u64,
    sum_m: f64,
    sum_s: f64,
    epoch: u64,
    previous_frame: Option<u64>,
    pending_reset: bool,
    peak: f64,
    max_m: f64,
    max_s: f64,
    incomplete: bool,
    suspended: bool,
    output: LevelMonitor<3>,
}
impl Clone for Loudness {
    fn clone(&self) -> Self {
        Self::prepared(self.context, &self.settings, false)
    }
}
impl Loudness {
    pub fn new(context: ProcessingContext, s: &LoudnessSettings) -> Self {
        Self::prepared(context, s, true)
    }
    // Host snapshots detach publication. Cache checkpoints need no analysis thread.
    fn prepared(context: ProcessingContext, s: &LoudnessSettings, start_worker: bool) -> Self {
        let meter = Arc::new(LoudnessMeter::default());
        let rate = context.sample_rate_hz as f64;
        meter.set_enabled(s.enabled);
        Self {
            context,
            settings: s.clone(),
            worker: start_worker.then(|| Worker::new(meter.clone())),
            epoch: meter.epoch(),
            meter,
            shelf: [Biquad::shelf(rate); 2],
            highpass: [Biquad::highpass(rate); 2],
            tp: TruePeak::new(),
            powers: vec![0.0; context.sample_rate_hz as usize * 3].into_boxed_slice(),
            index: 0,
            count: 0,
            sum_m: 0.0,
            sum_s: 0.0,
            previous_frame: None,
            pending_reset: false,
            peak: 0.0,
            max_m: 0.0,
            max_s: 0.0,
            incomplete: false,
            suspended: false,
            output: LevelMonitor::new(context.sample_rate_hz, s.enabled),
        }
    }
    /// Control-thread preparation only, before callback publication.
    pub fn set_loudness_meter(&mut self, meter: Arc<LoudnessMeter>) {
        self.worker = Some(Worker::new(meter.clone()));
        self.meter = meter;
        self.meter.set_enabled(self.settings.enabled);
        self.pending_reset = true;
    }
    pub fn meter(&self) -> Arc<LoudnessMeter> {
        self.meter.clone()
    }
    pub fn set_output_peak_meter(&mut self, meter: Arc<BandMeter<3>>) {
        self.output.attach_peak(meter);
    }
    pub fn retarget(&mut self, s: &LoudnessSettings) {
        if self.settings.enabled != s.enabled
            || self.settings.hq != s.hq
            || self.settings.continuous != s.continuous
        {
            self.meter.set_enabled(s.enabled);
            self.meter.reset();
            self.pending_reset = true;
        }
        self.settings.enabled = s.enabled;
        self.settings.target = s.target;
        self.settings.hq = s.hq;
        self.settings.continuous = s.continuous;
        self.output.set_enabled(s.enabled);
    }
    /// Callback-safe handoff from the live analyzer to a worker-prepared renderer.
    pub fn inherit_analysis(&mut self, previous: &mut Self) {
        previous.retarget(&self.settings);
        if !self.settings.continuous {
            previous.reset();
        }
        std::mem::swap(self, previous);
    }
    pub fn suspend(&mut self, value: bool) {
        self.suspended = value;
    }
    fn clear_front(&mut self) {
        self.epoch = self.meter.epoch();
        self.count = 0;
        self.index = 0;
        self.sum_m = 0.0;
        self.sum_s = 0.0;
        self.peak = 0.0;
        self.max_m = 0.0;
        self.max_s = 0.0;
        self.incomplete = false;
        for f in self.shelf.iter_mut().chain(self.highpass.iter_mut()) {
            f.reset();
        }
        self.tp.reset();
        self.pending_reset = false;
    }
    pub fn process_at(&mut self, input: [f32; 2], frame: u64) -> [f32; 2] {
        if self.suspended || !self.settings.enabled || self.worker.is_none() {
            return input;
        }
        let discontinuity = self
            .previous_frame
            .is_some_and(|p| frame != p.saturating_add(1));
        self.previous_frame = Some(frame);
        if !self.settings.continuous && (discontinuity || self.pending_reset) {
            self.meter.reset();
            self.pending_reset = false;
        }
        if self.epoch != self.meter.epoch() {
            self.clear_front();
        }
        if self.worker.as_ref().is_some_and(|worker| worker.failed) {
            self.meter.mark_incomplete(self.epoch);
        }
        if self.meter.held() {
            return input;
        }
        let x = input.map(|x| {
            if x.is_finite() {
                x as f64
            } else {
                self.incomplete = true;
                0.0
            }
        });
        let mut power = 0.0;
        if self.incomplete {
            self.meter.mark_incomplete(self.epoch);
        }
        for (c, sample) in x.into_iter().enumerate() {
            let y = self.highpass[c].tick(self.shelf[c].tick(sample));
            power += y * y;
        }
        let rate = self.context.sample_rate_hz as usize;
        let m = rate * 2 / 5;
        let n = self.powers.len();
        if self.count >= m as u64 {
            self.sum_m -= self.powers[(self.index + n - m) % n];
        }
        if self.count >= n as u64 {
            self.sum_s -= self.powers[self.index];
        }
        self.powers[self.index] = power;
        self.index = (self.index + 1) % n;
        self.sum_m += power;
        self.sum_s += power;
        self.count += 1;
        self.peak = self.peak.max(self.tp.tick(x, self.settings.hq));
        self.output.sample_output(x.map(|v| v as f32));
        if self.count.is_multiple_of((rate / 100) as u64) {
            self.max_m = self.max_m.max(self.sum_m.max(0.0) / m as f64);
            self.max_s = self.max_s.max(self.sum_s.max(0.0) / n as f64);
        }
        if self.count.is_multiple_of((rate / 10) as u64) {
            let o = Observation {
                epoch: self.epoch,
                m: self.sum_m.max(0.0) / m as f64,
                s: self.sum_s.max(0.0) / n as f64,
                peak: self.peak,
                max_m: self.max_m,
                max_s: self.max_s,
                seconds: self.count as f64 / rate as f64,
                ticks: self.count / (rate / 10) as u64,
                incomplete: self.incomplete,
            };
            if self
                .worker
                .as_mut()
                .is_some_and(|worker| worker.queue.push(o).is_err())
            {
                self.incomplete = true;
                self.meter.mark_incomplete(self.epoch);
            }
            self.peak = 0.0;
        }
        input
    }
}
impl EffectUnit for Loudness {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::LOUDNESS_TYPE_ID,
            name: "Loudness Meter",
            is_source: false,
            inputs: &[PortKind::StereoAudio],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.previous_frame = None;
        if !self.settings.continuous {
            self.pending_reset = true;
        }
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        let frame = self.previous_frame.map_or(0, |p| p.saturating_add(1));
        self.process_at(input, frame)
    }
    fn heap_bytes(&self) -> u64 {
        self.powers.len() as u64 * 8
            + 512 * std::mem::size_of::<Observation>() as u64
            + 14001 * 32
            + WORKER_STACK as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn analyzer(rate: u32) -> Loudness {
        Loudness::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 4096,
            },
            &LoudnessSettings::default(),
        )
    }
    fn wait(m: &LoudnessMeter, seconds: f32) -> LoudnessReading {
        let start = std::time::Instant::now();
        loop {
            let r = m.reading();
            if r.values[12] >= seconds {
                return r;
            }
            assert!(
                start.elapsed().as_secs() < 5,
                "analysis did not reach {seconds}: {:?}",
                &r.values[..14]
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn stereo_sine_calibration_and_polarity_at_all_processing_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut unit = analyzer(rate);
            for f in 0..rate {
                let x =
                    (std::f64::consts::TAU * 1000.0 * f as f64 / rate as f64).sin() as f32 * 0.1;
                let input = [x, -x];
                let out = unit.process_sample(input);
                assert_eq!(out.map(f32::to_bits), input.map(f32::to_bits));
            }
            let r = wait(&unit.meter, 1.0);
            assert!((r.values[0] + 20.0).abs() < 0.08, "{rate}: {}", r.values[0]);
            assert!((r.values[2] + 20.0).abs() < 0.08, "{rate}: {}", r.values[2]);
            assert!((r.values[5] + 20.0).abs() < 0.03);
            assert_eq!(r.values[13], 0.0);
            assert_eq!(unit.latency_frames(), 0);
            assert_eq!(unit.tail_frames(), 0);
        }
    }
    #[test]
    fn windows_zero_pad_startup_and_reset_discards_prior_rings_and_worker_results() {
        let mut unit = analyzer(48000);
        for f in 0..144000 {
            let x = (std::f32::consts::TAU * 1000.0 * f as f32 / 48000.0).sin() * 0.1;
            unit.process_sample([x; 2]);
        }
        let full = wait(&unit.meter, 3.0);
        assert!((full.values[1] + 20.0).abs() < 0.08);
        unit.meter.reset();
        assert_eq!(unit.meter.reading(), LoudnessReading::default());
        for f in 0..4800 {
            let x = (std::f32::consts::TAU * 1000.0 * f as f32 / 48000.0).sin() * 0.1;
            unit.process_sample([x; 2]);
        }
        let start = wait(&unit.meter, 0.1);
        assert!((start.values[0] + 26.02).abs() < 0.1);
        assert!((start.values[1] + 34.77).abs() < 0.1);
        assert!(start.values[2].is_infinite());
        assert_eq!(start.values[12], 0.1);
        unit.meter.reset();
        for _ in 0..4800 {
            unit.process_sample([0.0; 2]);
        }
        let silence = wait(&unit.meter, 0.1);
        assert!(silence.values[0].is_infinite());
        assert!(silence.values[5].is_infinite());
    }
    #[test]
    fn hold_bypass_clock_and_suspension_preserve_audio_and_obey_epochs() {
        let mut unit = analyzer(48000);
        for _ in 0..4800 {
            unit.process_sample([0.1; 2]);
        }
        let before = wait(&unit.meter, 0.1);
        unit.meter.set_hold(true);
        for _ in 0..4800 {
            assert_eq!(unit.process_sample([0.2; 2]), [0.2; 2]);
        }
        assert_eq!(unit.meter.reading(), before);
        unit.meter.set_hold(false);
        unit.suspend(true);
        for _ in 0..4800 {
            unit.process_sample([0.3; 2]);
        }
        assert_eq!(unit.meter.reading(), before);
        unit.suspend(false);
        unit.process_at([0.0; 2], 100000);
        assert_eq!(unit.meter.reading(), Default::default());
        let mut settings = LoudnessSettings {
            enabled: false,
            ..Default::default()
        };
        unit.retarget(&settings);
        for _ in 0..4800 {
            assert_eq!(unit.process_sample([0.4; 2]), [0.4; 2]);
        }
        assert_eq!(unit.meter.reading(), Default::default());
        settings.enabled = true;
        settings.continuous = true;
        unit.retarget(&settings);
        for _ in 0..4800 {
            unit.process_sample([0.2; 2]);
        }
        wait(&unit.meter, 0.1);
        let epoch = unit.meter.epoch();
        unit.reset();
        unit.process_at([0.0; 2], 999999);
        assert_eq!(unit.meter.epoch(), epoch);
    }
    #[test]
    fn true_peak_reconstructs_between_samples_in_both_quality_modes() {
        for hq in [false, true] {
            let mut tp = TruePeak::new();
            let mut sample_peak = 0.0_f64;
            let mut true_peak = 0.0_f64;
            for f in 0..1024 {
                let x =
                    (std::f64::consts::TAU * 0.25 * f as f64 + std::f64::consts::FRAC_PI_4).sin();
                sample_peak = sample_peak.max(x.abs());
                true_peak = true_peak.max(tp.tick([x, -x], hq));
            }
            assert!(sample_peak < 0.708);
            assert!((true_peak - 1.0).abs() < 0.03, "{hq}: {true_peak}");
        }
    }
    #[test]
    fn invalid_observations_remain_visible_after_the_final_audio_frame() {
        let mut unit = analyzer(48000);
        let input = [f32::from_bits(0x7fc00001), -0.0];
        assert_eq!(
            unit.process_sample(input).map(f32::to_bits),
            input.map(f32::to_bits)
        );
        assert_eq!(unit.meter.reading().values[13], 1.0);
    }
}

#[cfg(test)]
mod retirement_tests {
    use super::*;

    #[test]
    fn retirement_and_meter_rebinding_release_worker_owners_before_returning() {
        for _ in 0..16 {
            let mut unit = Loudness::new(
                ProcessingContext {
                    sample_rate_hz: 48000,
                    max_block_frames: 512,
                },
                &LoudnessSettings::default(),
            );
            assert!(!unit.worker.as_ref().unwrap().failed);
            let original = unit.meter();
            let replacement = Arc::new(LoudnessMeter::default());
            unit.set_loudness_meter(replacement.clone());
            assert!(!unit.worker.as_ref().unwrap().failed);
            assert_eq!(
                Arc::strong_count(&original),
                1,
                "old worker must be retired"
            );
            drop(unit);
            assert_eq!(
                Arc::strong_count(&replacement),
                1,
                "worker must finish before the host can unload the plugin"
            );
        }
    }
}

#[cfg(test)]
mod overload_tests {
    use super::*;
    #[test]
    fn full_observation_queue_marks_incomplete_without_waiting_or_modifying_audio() {
        let mut unit = Loudness::new(
            ProcessingContext {
                sample_rate_hz: 48000,
                max_block_frames: 512,
            },
            &LoudnessSettings::default(),
        );
        let (queue, _consumer) = RingBuffer::new(1);
        unit.worker = Some(Worker {
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            queue,
            failed: false,
        });
        for _ in 0..9600 {
            assert_eq!(unit.process_sample([0.1, -0.2]), [0.1, -0.2]);
        }
        assert_eq!(unit.meter.reading().values[13], 1.0);
        unit.meter.reset();
        assert_eq!(unit.meter.reading().values[13], 0.0);
    }
}
