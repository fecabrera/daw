//! Bounded live MIDI boundary. Only construction/control code takes the channel mutex.
use super::{Ordering, Status};
use daw_core::Id;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64},
};
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub enum MidiAction {
    Route { track: Id, enabled: bool },
    Attack { token: Id, pitch: u8, velocity: u8 },
    Release { token: Id },
    Record { count_in_frames: u64 },
    Seal,
    Stop,
    Pause,
    Panic,
}
#[derive(Clone, Copy, Debug)]
pub struct MidiReceipt {
    pub action: MidiAction,
    pub frame: u64,
    pub clock: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy)]
struct Packet {
    action: MidiAction,
    at: u64,
    sequence: u64,
    epoch: u64,
    capture_frame: Option<u64>,
}
struct Channels {
    tx: Option<Producer<Packet>>,
    rx: Option<Consumer<Packet>>,
    receipts_tx: Option<Producer<MidiReceipt>>,
    receipts_rx: Option<Consumer<MidiReceipt>>,
}
pub(super) struct Port {
    channels: Mutex<Channels>,
    origin: Instant,
    stamp: AtomicU64,
    clock: AtomicU64,
    frame: AtomicU64,
    playing: AtomicBool,
    sequence: AtomicU64,
    epoch: Arc<AtomicU64>,
    pub fault: AtomicBool,
}
impl Default for Port {
    fn default() -> Self {
        let (tx, rx) = RingBuffer::new(8192);
        let (receipts_tx, receipts_rx) = RingBuffer::new(32768);
        Self {
            channels: Mutex::new(Channels {
                tx: Some(tx),
                rx: Some(rx),
                receipts_tx: Some(receipts_tx),
                receipts_rx: Some(receipts_rx),
            }),
            origin: Instant::now(),
            stamp: AtomicU64::new(0),
            clock: AtomicU64::new(0),
            frame: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            epoch: Arc::new(AtomicU64::new(0)),
            fault: AtomicBool::new(false),
        }
    }
}
pub struct MidiSender {
    tx: Producer<Packet>,
    receipts: Consumer<MidiReceipt>,
    status: Arc<Status>,
    rate: u32,
    last_at: u64,
    sequence: u64,
    last_capture_frame: Option<u64>,
    recording_offset_frames: i64,
}
impl MidiSender {
    /// Shift captured timestamps only; monitoring remains immediate. Positive values delay notes.
    pub fn set_recording_offset_ms(&mut self, milliseconds: i16) {
        self.recording_offset_frames =
            i64::from(milliseconds.clamp(-200, 200)) * i64::from(self.rate) / 1000;
    }
    pub fn send(&mut self, action: MidiAction) -> Result<(), String> {
        self.send_at(action, Instant::now())
    }
    pub fn send_at(&mut self, action: MidiAction, arrival: Instant) -> Result<(), String> {
        let port = &self.status.midi;
        if port.fault.load(Ordering::Acquire) {
            return Err("MIDI input stopped after queue pressure; reconnect the input.".into());
        }
        let mut anchor = None;
        for _ in 0..8 {
            let before = port.sequence.load(Ordering::Acquire);
            let pair = (
                port.clock.load(Ordering::Relaxed),
                port.stamp.load(Ordering::Relaxed),
                port.frame.load(Ordering::Relaxed),
                port.playing.load(Ordering::Relaxed),
            );
            if before.is_multiple_of(2) && before == port.sequence.load(Ordering::Acquire) {
                anchor = Some(pair);
                break;
            }
        }
        let Some((clock, stamp, frame, playing)) = anchor else {
            self.fail();
            return Err("MIDI output clock is unavailable; take interrupted".into());
        };
        let elapsed = arrival
            .saturating_duration_since(port.origin)
            .as_micros()
            .min(u128::from(u64::MAX)) as u64;
        let delta = (i128::from(elapsed) - i128::from(stamp)).min(20_000);
        let frames = delta * i128::from(self.rate) / 1_000_000;
        let mapped_clock = (i128::from(clock) + frames).clamp(0, i128::from(u64::MAX)) as u64;
        let at = mapped_clock.max(self.last_at);
        if matches!(
            action,
            MidiAction::Record { .. } | MidiAction::Stop | MidiAction::Pause | MidiAction::Panic
        ) {
            self.last_capture_frame = None;
        }
        let capture_frame = if playing
            && matches!(
                action,
                MidiAction::Attack { .. } | MidiAction::Release { .. } | MidiAction::Route { .. }
            ) {
            Some(
                ((i128::from(frame) + frames + i128::from(self.recording_offset_frames))
                    .clamp(0, i128::from(u64::MAX)) as u64)
                    .max(self.last_capture_frame.unwrap_or(0)),
            )
        } else {
            None
        };
        if capture_frame.is_some() {
            self.last_capture_frame = capture_frame;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let packet = Packet {
            action,
            at,
            sequence: self.sequence,
            epoch: port.epoch.load(Ordering::Acquire),
            capture_frame,
        };
        if self.tx.push(packet).is_err() {
            self.fail();
            return Err("MIDI event queue overflow; notes released and take interrupted.".into());
        }
        self.last_at = at;
        Ok(())
    }
    pub fn reset_input(&mut self) {
        self.status.midi.fault.store(false, Ordering::Release);
        self.panic();
        self.last_at = self.status.midi.clock.load(Ordering::Acquire);
        self.last_capture_frame = None;
    }
    pub fn last_frame(&self) -> u64 {
        self.status
            .midi
            .frame
            .load(Ordering::Acquire)
            .max(self.status.playhead.load(Ordering::Acquire))
    }
    pub fn emergency_handle(&self) -> Arc<AtomicU64> {
        self.status.midi.epoch.clone()
    }
    pub fn receipt(&mut self) -> Option<MidiReceipt> {
        self.receipts.pop().ok()
    }
    pub fn failed(&self) -> bool {
        self.status.midi.fault.load(Ordering::Acquire) || self.status.failed.load(Ordering::Acquire)
    }
    pub fn fail(&self) {
        self.status.midi.fault.store(true, Ordering::Release);
        self.status.midi.epoch.fetch_add(1, Ordering::AcqRel);
    }
    pub fn panic(&self) {
        self.status.midi.epoch.fetch_add(1, Ordering::AcqRel);
    }
}
pub(super) struct Receiver {
    rx: Consumer<Packet>,
    receipts: Producer<MidiReceipt>,
    pending: Option<Packet>,
    epoch: u64,
    count_in: Option<(u64, u64, Packet)>,
    capture_origin: u64,
}
impl Receiver {
    pub fn new(status: &Status) -> Self {
        let mut channels = status.midi.channels.lock().unwrap();
        Self {
            rx: channels.rx.take().unwrap(),
            receipts: channels.receipts_tx.take().unwrap(),
            pending: None,
            epoch: 0,
            count_in: None,
            capture_origin: 0,
        }
    }
    fn receipt(
        &mut self,
        packet: Packet,
        renderer: &daw_engine::Renderer,
        clock: u64,
        status: &Status,
    ) {
        if matches!(packet.action, MidiAction::Record { .. }) {
            self.capture_origin = renderer.playhead;
        }
        if self
            .receipts
            .push(MidiReceipt {
                action: packet.action,
                frame: packet
                    .capture_frame
                    .map_or(renderer.playhead, |frame| frame.max(self.capture_origin)),
                clock,
                sequence: packet.sequence,
            })
            .is_err()
        {
            status.midi.fault.store(true, Ordering::Release);
            status.midi.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
    pub fn sample(
        &mut self,
        renderer: &mut daw_engine::Renderer,
        clock: u64,
        status: &Status,
    ) -> Option<u64> {
        let epoch = status.midi.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            renderer.panic_live_midi();
            renderer.recording = false;
            self.count_in = None;
            self.epoch = epoch;
            self.receipt(
                Packet {
                    action: MidiAction::Panic,
                    at: clock,
                    sequence: 0,
                    epoch,
                    capture_frame: None,
                },
                renderer,
                clock,
                status,
            );
        }
        if let Some((_, end, packet)) = self.count_in
            && clock >= end
        {
            self.count_in = None;
            renderer.panic_live_midi();
            if renderer.has_live_midi_routes() {
                renderer.recording = true;
                renderer.play();
                self.receipt(packet, renderer, clock, status);
            } else {
                self.receipt(
                    Packet {
                        action: MidiAction::Seal,
                        ..packet
                    },
                    renderer,
                    clock,
                    status,
                );
            }
        }
        for _ in 0..64 {
            let Some(packet) = self.pending.take().or_else(|| self.rx.pop().ok()) else {
                break;
            };
            // Panic invalidates physical note/capture events, not the selected
            // routing state. Focus can change while startup routes are queued.
            if packet.epoch != self.epoch && !matches!(packet.action, MidiAction::Route { .. }) {
                continue;
            }
            if packet.at > clock {
                self.pending = Some(packet);
                break;
            }
            match packet.action {
                MidiAction::Route { track, enabled } => renderer.live_midi_route(track, enabled),
                MidiAction::Attack {
                    token,
                    pitch,
                    velocity,
                } => {
                    renderer.live_midi_note(token, Some((pitch, velocity)));
                }
                MidiAction::Release { token } => {
                    renderer.live_midi_note(token, None);
                }
                MidiAction::Record { count_in_frames }
                    if count_in_frames > 0 && !renderer.playing =>
                {
                    self.count_in = Some((clock, clock.saturating_add(count_in_frames), packet));
                    continue;
                }
                MidiAction::Record { .. } if !renderer.has_live_midi_routes() => {
                    self.receipt(
                        Packet {
                            action: MidiAction::Seal,
                            ..packet
                        },
                        renderer,
                        clock,
                        status,
                    );
                    continue;
                }
                MidiAction::Record { .. } => {
                    renderer.panic_live_midi();
                    renderer.recording = true;
                    renderer.play();
                }
                MidiAction::Seal | MidiAction::Stop | MidiAction::Pause | MidiAction::Panic => {
                    self.count_in = None;
                    renderer.recording = false;
                    renderer.panic_live_midi();
                }
            }
            self.receipt(packet, renderer, clock, status);
            match packet.action {
                MidiAction::Stop => renderer.stop(),
                MidiAction::Pause => renderer.pause(),
                _ => {}
            }
        }
        status.midi.sequence.fetch_add(1, Ordering::AcqRel);
        status.midi.clock.store(clock, Ordering::Relaxed);
        status
            .midi
            .frame
            .store(renderer.playhead, Ordering::Relaxed);
        status
            .midi
            .playing
            .store(renderer.playing, Ordering::Relaxed);
        status.midi.stamp.store(
            status
                .midi
                .origin
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
        status.midi.sequence.fetch_add(1, Ordering::Release);
        self.count_in
            .map(|(start, _, _)| clock.saturating_sub(start))
    }
}
impl super::AudioOutput {
    /// Transfer the sole producer to the non-UI input broker.
    pub fn take_midi_sender(&self) -> Option<MidiSender> {
        // Startup uses an empty renderer. Route commands sent to it have no track
        // to enable and cannot be inherited by the first complete project plan.
        if !self.current_plan_published() {
            return None;
        }
        let mut channels = self.status.midi.channels.lock().ok()?;
        Some(MidiSender {
            tx: channels.tx.take()?,
            receipts: channels.receipts_rx.take()?,
            status: self.status.clone(),
            rate: self.project_rate_hz,
            last_at: 0,
            sequence: 0,
            last_capture_frame: None,
            recording_offset_frames: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Callback, StereoMeter};
    use std::collections::HashMap;
    fn fixture() -> (Callback, MidiSender, Vec<Id>) {
        let mut project = daw_core::Project::default();
        let mut tracks = Vec::new();
        for _ in 0..2 {
            let id = project.add_midi_track().unwrap();
            project.tracks.last_mut().unwrap().instrument =
                Some(daw_core::OscillatorSettings::default().into());
            tracks.push(id);
        }
        let plan = daw_engine::RenderPlan {
            external: Default::default(),
            project,
            beat_tracks: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let meters = tracks
            .iter()
            .map(|id| (*id, Arc::new(StereoMeter::default())))
            .collect::<HashMap<_, _>>();
        let (_, commands) = RingBuffer::new(8);
        let (retired, _) = RingBuffer::new(8);
        let status = Arc::new(Status::default());
        let callback =
            Callback::new(plan, &meters, commands, retired, status.clone(), 48000).unwrap();
        let mut channels = status.midi.channels.lock().unwrap();
        let tx = channels.tx.take().unwrap();
        let receipts = channels.receipts_rx.take().unwrap();
        drop(channels);
        let sender = MidiSender {
            tx,
            receipts,
            status,
            rate: 48000,
            last_at: 0,
            sequence: 0,
            last_capture_frame: None,
            recording_offset_frames: 0,
        };
        (callback, sender, tracks)
    }
    #[test]
    fn loaded_project_attaches_midi_only_after_instruments_are_published() {
        let (mut output, mut callback) = crate::preparation_tests::controlled_output(48000);
        let mut plan = (*output.plan).clone();
        let track = plan.project.add_midi_track().unwrap();
        plan.project.tracks.last_mut().unwrap().instrument =
            Some(daw_core::OscillatorSettings::default().into());
        let origin = plan.project.transport.playhead_frame;
        output.plan = Arc::new(plan.clone());
        *callback.renderer =
            crate::PreparedRenderer::new(crate::preparation::initial_plan(&plan), &HashMap::new());
        assert!(
            !output.preparing(),
            "revision zero alone does not establish startup readiness"
        );
        assert!(!output.current_plan_published());
        assert!(
            output.take_midi_sender().is_none(),
            "keep the producer until real instrument routes exist"
        );
        let meters = plan
            .project
            .tracks
            .iter()
            .map(|t| (t.id, Arc::new(StereoMeter::default())))
            .collect();
        let mut next = Box::new(crate::PreparedRenderer::new(plan, &meters));
        next.publication = 1;
        crate::preparation_tests::realtime(|| callback.install(next));
        output.active_plan = Some(output.plan.clone());
        output.active_publication = 1;
        assert!(output.current_plan_published());
        let mut sender = output
            .take_midi_sender()
            .expect("attach without cycling the device");
        assert!(output.take_midi_sender().is_none());
        sender
            .send(MidiAction::Route {
                track,
                enabled: true,
            })
            .unwrap();
        sender
            .send(MidiAction::Attack {
                token: Id::new_v4(),
                pitch: 69,
                velocity: 110,
            })
            .unwrap();
        assert!(pump(&mut callback, 2000) > 0.01);
        assert_eq!(callback.renderer.playhead, origin);
    }
    fn pump(callback: &mut Callback, count: usize) -> f32 {
        let mut peak = 0.0_f32;
        crate::preparation_tests::realtime(|| {
            for _ in 0..count {
                let sample = callback.next_project_sample();
                peak = peak.max(sample[0].abs()).max(sample[1].abs());
            }
        });
        peak
    }
    #[test]
    fn stopped_multitrack_monitoring_and_removed_route_preserve_the_other_voice_without_heap_work()
    {
        let (mut callback, mut sender, tracks) = fixture();
        for track in &tracks {
            sender
                .send(MidiAction::Route {
                    track: *track,
                    enabled: true,
                })
                .unwrap();
        }
        sender
            .send(MidiAction::Attack {
                token: Id::from_u128(7),
                pitch: 69,
                velocity: 110,
            })
            .unwrap();
        assert!(pump(&mut callback, 2000) > 0.01);
        assert_eq!(callback.renderer.playhead, 0);
        sender
            .send(MidiAction::Route {
                track: tracks[0],
                enabled: false,
            })
            .unwrap();
        pump(&mut callback, 2000);
        callback.renderer.reset_track_peaks();
        assert!(pump(&mut callback, 500) > 0.01);
        let peaks = callback.renderer.track_peaks().collect::<Vec<_>>();
        assert_eq!(peaks[0], [0.0; 2]);
        assert!(peaks[1][0] > 0.01);
        sender.panic();
        pump(&mut callback, 2000);
        assert_eq!(pump(&mut callback, 500), 0.0);
    }
    #[test]
    fn count_in_holds_the_playhead_and_records_beyond_an_empty_arrangement() {
        let (mut callback, mut sender, tracks) = fixture();
        sender
            .send(MidiAction::Route {
                track: tracks[0],
                enabled: true,
            })
            .unwrap();
        sender
            .send(MidiAction::Record {
                count_in_frames: 480,
            })
            .unwrap();
        let mut started = None;
        for _ in 0..2000 {
            callback.next_project_sample();
            while let Some(receipt) = sender.receipt() {
                if matches!(receipt.action, MidiAction::Record { .. }) {
                    started = Some(receipt);
                }
            }
        }
        let receipt = started.expect("capture start receipt");
        assert_eq!(receipt.frame, 0);
        assert!(receipt.clock >= 480);
        assert!(callback.renderer.playing);
        assert!(callback.renderer.recording);
        assert!(callback.renderer.playhead > 0);
        sender.send(MidiAction::Seal).unwrap();
        pump(&mut callback, 2000);
        assert!(!callback.renderer.recording);
    }
    #[test]
    fn stopped_and_paused_auto_filter_seeks_do_not_audition_cached_notes_with_live_input_enabled() {
        let mut project = daw_core::Project::default();
        let track = project.add_midi_track().unwrap();
        project.tracks[0].instrument = Some(daw_core::OscillatorSettings::default().into());
        project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::auto_filter());
        let clip = project.add_midi_pattern(track, 0).unwrap();
        project
            .edit(daw_core::Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: daw_core::MidiPattern {
                    length_ticks: 3840,
                    notes: vec![daw_core::MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 3840,
                        pitch: 60,
                        velocity: 110,
                    }],
                },
            })
            .unwrap();
        // A buffered master stage also exercises latency priming at the seek frame.
        project
            .master
            .effects
            .push(daw_core::EffectInstance::limiter());
        let plan = daw_engine::RenderPlan {
            external: Default::default(),
            project,
            beat_tracks: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let status = Arc::new(crate::Status::default());
        let bindings = crate::preparation::Bindings::prepare(&plan, &Default::default());
        let worker = crate::preparation::Worker::new(status).unwrap();
        for paused in [false, true] {
            for frame in [6000, 12000] {
                worker.submit(
                    plan.clone(),
                    bindings.clone(),
                    Some(daw_engine::TransportState {
                        frame,
                        playing: false,
                        paused,
                        playback_start: paused.then_some(0),
                    }),
                );
                let mut prepared = crate::preparation_tests::ready(&worker).renderer;
                prepared.live_midi_route(track, true);
                let mut peak = 0.0_f32;
                crate::preparation_tests::realtime(|| {
                    for _ in 0..2000 {
                        for sample in prepared.next_sample() {
                            peak = peak.max(sample.abs());
                        }
                    }
                });
                assert_eq!(
                    peak, 0.0,
                    "seek must not play cached notes: paused={paused}, frame={frame}"
                );
                prepared.live_midi_note(Id::new_v4(), Some((69, 110)));
                crate::preparation_tests::realtime(|| {
                    for _ in 0..2000 {
                        for sample in prepared.next_sample() {
                            peak = peak.max(sample.abs());
                        }
                    }
                });
                assert!(
                    peak > 0.01,
                    "live keyboard input must still sound after seeking"
                );
                assert_eq!(prepared.playhead, frame);
            }
        }
    }
    #[test]
    fn restored_transport_has_live_voice_capacity_before_publication() {
        let (callback, _, tracks) = fixture();
        let plan = callback.renderer.plan.clone();
        let status = Arc::new(crate::Status::default());
        let bindings = crate::preparation::Bindings::prepare(&plan, &Default::default());
        let worker = crate::preparation::Worker::new(status).unwrap();
        for frame in [0, 4800] {
            worker.submit(
                plan.clone(),
                bindings.clone(),
                Some(daw_engine::TransportState {
                    frame,
                    playing: false,
                    paused: false,
                    playback_start: None,
                }),
            );
            let mut prepared = crate::preparation_tests::ready(&worker).renderer;
            assert!(prepared.prepared_transport);
            for track in &tracks {
                prepared.live_midi_route(*track, true);
            }
            assert!(
                prepared.live_midi_note(Id::new_v4(), Some((69, 110))),
                "transport-cache restoration must retain live voice capacity"
            );
            let mut peak = 0.0_f32;
            crate::preparation_tests::realtime(|| {
                for _ in 0..2000 {
                    for sample in prepared.next_sample() {
                        peak = peak.max(sample.abs());
                    }
                }
            });
            assert!(peak > 0.01);
            assert_eq!(prepared.playhead, frame);
        }
    }
    #[test]
    fn focus_panic_preserves_queued_routes_but_discards_old_note_attacks() {
        let (mut callback, mut sender, tracks) = fixture();
        sender
            .send(MidiAction::Route {
                track: tracks[0],
                enabled: true,
            })
            .unwrap();
        sender
            .send(MidiAction::Attack {
                token: Id::from_u128(40),
                pitch: 60,
                velocity: 127,
            })
            .unwrap();
        sender.panic();
        assert_eq!(
            pump(&mut callback, 2000),
            0.0,
            "pre-focus-loss attacks must not sound later"
        );
        sender
            .send(MidiAction::Attack {
                token: Id::from_u128(41),
                pitch: 60,
                velocity: 127,
            })
            .unwrap();
        assert!(
            pump(&mut callback, 2000) > 0.01,
            "new input must use the restored route without reconnecting"
        );
        sender
            .send(MidiAction::Route {
                track: tracks[0],
                enabled: false,
            })
            .unwrap();
        sender.panic();
        pump(&mut callback, 2000);
        sender
            .send(MidiAction::Attack {
                token: Id::from_u128(42),
                pitch: 60,
                velocity: 127,
            })
            .unwrap();
        assert_eq!(
            pump(&mut callback, 2000),
            0.0,
            "a queued route removal must also survive panic"
        );
    }
    #[test]
    fn full_ordinary_queue_cannot_block_emergency_release() {
        let (mut callback, mut sender, tracks) = fixture();
        sender
            .send(MidiAction::Route {
                track: tracks[0],
                enabled: true,
            })
            .unwrap();
        sender
            .send(MidiAction::Attack {
                token: Id::from_u128(9),
                pitch: 60,
                velocity: 127,
            })
            .unwrap();
        assert!(pump(&mut callback, 2000) > 0.01);
        let mut failed = false;
        for _ in 0..9000 {
            if sender
                .send(MidiAction::Attack {
                    token: Id::from_u128(10),
                    pitch: 60,
                    velocity: 127,
                })
                .is_err()
            {
                failed = true;
                break;
            }
        }
        assert!(failed);
        pump(&mut callback, 2000);
        assert_eq!(pump(&mut callback, 500), 0.0);
    }
}
