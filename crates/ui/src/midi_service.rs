//! Device/broker/capture worker. No input or recording loop runs in egui painting.
use crate::midi_writer::{Request as WriteRequest, Status as WriteStatus, Writer};
use daw_core::Id;
use daw_output::{MidiAction, MidiSender};
use daw_project::{SaveSnapshot, Session, recovery::RecoveryEntry};
use rtrb::{Producer, RingBuffer};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct MidiDevice {
    pub id: String,
    pub name: String,
}
/// Native implementations live in desktop. Backend calls run only on the broker.
pub trait MidiBackend: Send {
    fn devices(&mut self) -> Result<Vec<MidiDevice>, String>;
    fn connect(&mut self, device: &str, ingress: MidiIngress) -> Result<(), String>;
    fn disconnect(&mut self);
}
#[derive(Clone, Copy)]
struct RawMidi {
    bytes: [u8; 3],
    at: Instant,
    sequence: u64,
    connection: u64,
}
pub struct MidiIngress {
    tx: Producer<RawMidi>,
    sequence: Arc<AtomicU64>,
    connection: u64,
    overflow: Arc<AtomicBool>,
    emergency: Option<Arc<AtomicU64>>,
}
impl MidiIngress {
    /// Native callback entry. Ignore unsupported/system messages; never allocate or wait.
    pub fn receive(&mut self, bytes: &[u8]) {
        if bytes.len() != 3
            || !matches!(bytes[0] & 0xf0, 0x80 | 0x90 | 0xb0)
            || bytes[1] > 127
            || bytes[2] > 127
        {
            return;
        }
        if self
            .tx
            .push(RawMidi {
                bytes: [bytes[0], bytes[1], bytes[2]],
                at: Instant::now(),
                sequence: self.sequence.fetch_add(1, Ordering::AcqRel),
                connection: self.connection,
            })
            .is_err()
        {
            self.overflow.store(true, Ordering::Release);
            if let Some(epoch) = &self.emergency {
                epoch.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}
pub(super) enum Command {
    Connect(Option<String>, Option<u8>, i16),
    Output(MidiSender),
    Routes(Vec<Id>),
    Record(Session, u64, u64),
    Boundary(MidiAction),
    Snapshot(SaveSnapshot, u64, Option<PathBuf>),
    Acknowledge,
    Retry,
    Recover(RecoveryEntry),
    OpenSaved(RecoveryEntry),
    Discard(RecoveryEntry),
    LeaveProject(Id),
    Shutdown,
}
pub(super) enum Message {
    StorageReady,
    TakeRetained(bool),
    Devices(Vec<MidiDevice>),
    Connected(bool),
    Error(String),
    Recording(bool),
    Take { base: String, session: Session },
    Pitch(u8, Vec<Id>),
    Recoveries(Vec<RecoveryEntry>),
    Recovered(Session),
}
#[derive(Clone, Default)]
pub(super) struct Inbox(Arc<Mutex<VecDeque<Message>>>);
impl Inbox {
    pub(super) fn send(&self, message: Message) -> Result<(), ()> {
        let mut queue = self.0.lock().map_err(|_| ())?;
        // Coalesce transient status. Critical whole-take outcomes stay queued until polled.
        queue.retain(|old| {
            !matches!(
                (old, &message),
                (Message::Devices(_), Message::Devices(_))
                    | (Message::Connected(_), Message::Connected(_))
                    | (Message::Recording(_), Message::Recording(_))
            )
        });
        if queue.len() >= 256 {
            if let Some(index) = queue.iter().position(|m| {
                matches!(
                    m,
                    Message::Pitch(..) | Message::Devices(..) | Message::Error(..)
                )
            }) {
                queue.remove(index);
            } else {
                return Err(());
            }
        }
        queue.push_back(message);
        Ok(())
    }
    pub fn try_recv(&self) -> Result<Message, mpsc::TryRecvError> {
        self.0
            .lock()
            .map_err(|_| mpsc::TryRecvError::Disconnected)?
            .pop_front()
            .ok_or(mpsc::TryRecvError::Empty)
    }
}
struct Control {
    command: Command,
    at: Instant,
    sequence: u64,
}
enum Work {
    Control(Box<Control>),
    Raw(RawMidi),
}
impl Work {
    fn sequence(&self) -> u64 {
        match self {
            Self::Control(c) => c.sequence,
            Self::Raw(r) => r.sequence,
        }
    }
}
pub(super) struct Service {
    tx: mpsc::SyncSender<Control>,
    sequence: Arc<AtomicU64>,
    pub rx: Inbox,
    emergency: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Service {
    pub fn new(mut backend: Box<dyn MidiBackend>, root: PathBuf) -> Self {
        let (tx, commands) = mpsc::sync_channel(32);
        let sequence = Arc::new(AtomicU64::new(0));
        let input_sequence = sequence.clone();
        let messages = Inbox::default();
        let rx = messages.clone();
        let emergency = Arc::new(AtomicBool::new(false));
        let panic = emergency.clone();
        let worker = std::thread::spawn(move || {
            let result = run(
                &mut *backend,
                root,
                commands,
                &messages,
                panic,
                input_sequence,
            );
            backend.disconnect();
            if let Err(error) = result {
                let _ = messages.send(Message::Error(error));
            }
        });
        Self {
            tx,
            sequence,
            rx,
            emergency,
            worker: Some(worker),
        }
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.tx
            .try_send(Control {
                command,
                at: Instant::now(),
                sequence: self.sequence.fetch_add(1, Ordering::AcqRel),
            })
            .map_err(|_| "MIDI control queue is full".into())
    }
    pub fn panic(&self) {
        self.emergency.store(true, Ordering::Release);
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.panic();
        let _ = self.tx.send(Control {
            command: Command::Shutdown,
            at: Instant::now(),
            sequence: self.sequence.fetch_add(1, Ordering::AcqRel),
        });
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn run(
    backend: &mut dyn MidiBackend,
    root: PathBuf,
    commands: mpsc::Receiver<Control>,
    messages: &Inbox,
    panic: Arc<AtomicBool>,
    input_sequence: Arc<AtomicU64>,
) -> Result<(), String> {
    let mut writer = Writer::new(root, messages.clone());
    let mut storage_ready = false;
    let mut take_busy = false;
    let mut record_sent = false;
    let mut capture_started = false;
    let mut cancelled = false;
    let overflow = Arc::new(AtomicBool::new(false));
    let (mut ingress, mut raw) = RingBuffer::new(8192);
    let mut output: Option<MidiSender> = None;
    let mut routes = Vec::new();
    let mut selected_device: Option<String> = None;
    let mut channel = None;
    let mut recording_offset_ms = 0;
    let mut enumeration_error: Option<String> = None;
    let mut connection = 0_u64;
    let mut performance = daw_project::midi_input::MidiPerformance::default();
    let mut last_frame = 0;
    let mut fault_handled = false;
    let mut enumerated = Instant::now() - Duration::from_secs(1);
    let mut work = Vec::with_capacity(8224);
    loop {
        while let Some(status) = writer.poll() {
            match status {
                WriteStatus::StorageReady => storage_ready = true,
                WriteStatus::Fault => {
                    if let Some(o) = &output {
                        o.panic();
                    }
                }
                WriteStatus::Idle => {
                    take_busy = false;
                    record_sent = false;
                    capture_started = false;
                    cancelled = false;
                }
                WriteStatus::BeginReady(count_in_frames) => {
                    if !cancelled && !routes.is_empty() {
                        if let Some(o) = &mut output {
                            match o.send(MidiAction::Record { count_in_frames }) {
                                Ok(()) => record_sent = true,
                                Err(error) => {
                                    writer.interrupt(last_frame);
                                    let _ = messages.send(Message::Error(error));
                                }
                            }
                        }
                    } else {
                        writer.interrupt(last_frame);
                    }
                }
            }
        }
        let fault = overflow.swap(false, Ordering::AcqRel)
            || output.as_ref().is_some_and(MidiSender::failed);
        let emergency = panic.swap(false, Ordering::AcqRel) || (fault && !fault_handled);
        if fault {
            fault_handled = true;
        }
        if emergency {
            if let Some(o) = &output {
                o.panic();
            }
            performance.silence();
            while raw.pop().is_ok() {}
            if take_busy {
                cancelled = true;
                if fault || !record_sent {
                    writer.interrupt(
                        output
                            .as_ref()
                            .map_or(last_frame, |o| o.last_frame().max(last_frame)),
                    );
                }
            }
            if fault && selected_device.is_some() {
                backend.disconnect();
                selected_device = None;
                let _ = messages.send(Message::Connected(false));
            }
        }
        for _ in 0..32 {
            match commands.try_recv() {
                Ok(c) => work.push(Work::Control(Box::new(c))),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(_) => return Ok(()),
            }
        }
        for _ in 0..8192 {
            match raw.pop() {
                Ok(packet) => work.push(Work::Raw(packet)),
                Err(_) => break,
            }
        }
        work.sort_unstable_by_key(Work::sequence);
        for event in work.drain(..) {
            let (command, control_at) = match event {
                Work::Control(control) => {
                    let Control { command, at, .. } = *control;
                    (command, at)
                }
                Work::Raw(packet) => {
                    if packet.connection != connection {
                        continue;
                    }
                    if let Some(o) = &mut output {
                        let result = performance.process(packet.bytes, channel, |event| {
                            use daw_project::midi_input::MidiInputEvent;
                            let action = match event {
                                MidiInputEvent::Attack {
                                    token,
                                    pitch,
                                    velocity,
                                } => {
                                    let _ = messages.send(Message::Pitch(pitch, routes.clone()));
                                    MidiAction::Attack {
                                        token,
                                        pitch,
                                        velocity,
                                    }
                                }
                                MidiInputEvent::Release { token } => MidiAction::Release { token },
                                MidiInputEvent::Panic => MidiAction::Panic,
                            };
                            o.send_at(action, packet.at)
                        });
                        if let Err(error) = result {
                            panic.store(true, Ordering::Release);
                            let _ = messages.send(Message::Error(error));
                        }
                    }
                    continue;
                }
            };
            let result: Result<(), String> = (|| {
                match command {
                    Command::LeaveProject(id) => writer.request(WriteRequest::LeaveProject(id))?,
                    Command::Shutdown => return Err("__shutdown__".into()),
                    Command::Connect(device, filter, offset) => {
                        if take_busy {
                            return Err("Finish the MIDI take before changing the input".into());
                        }
                        backend.disconnect();
                        selected_device = None;
                        let _ = messages.send(Message::Connected(false));
                        connection = connection.wrapping_add(1);
                        fault_handled = false;
                        if let Some(o) = &mut output {
                            o.reset_input();
                            o.send(MidiAction::Panic)?;
                            for track in &routes {
                                o.send(MidiAction::Route {
                                    track: *track,
                                    enabled: false,
                                })?;
                            }
                        }
                        performance.disconnect();
                        let (tx, rx) = RingBuffer::new(8192);
                        ingress = tx;
                        raw = rx;
                        channel = filter;
                        recording_offset_ms = offset;
                        if let Some(sender) = &mut output {
                            sender.set_recording_offset_ms(offset);
                        }
                        if let Some(id) = &device {
                            backend.connect(
                                id,
                                MidiIngress {
                                    tx: std::mem::replace(&mut ingress, RingBuffer::new(1).0),
                                    sequence: input_sequence.clone(),
                                    connection,
                                    overflow: overflow.clone(),
                                    emergency: output.as_ref().map(MidiSender::emergency_handle),
                                },
                            )?;
                        }
                        selected_device = device;
                        if let Some(o) = &mut output {
                            for track in &routes {
                                o.send(MidiAction::Route {
                                    track: *track,
                                    enabled: selected_device.is_some(),
                                })?;
                            }
                        }
                        let _ = messages.send(Message::Connected(selected_device.is_some()));
                    }
                    Command::Output(mut sender) => {
                        sender.set_recording_offset_ms(recording_offset_ms);
                        if let Some(previous) = &output {
                            previous.panic();
                        }
                        if let Some(device) = selected_device.as_ref() {
                            backend.disconnect();
                            connection = connection.wrapping_add(1);
                            performance.disconnect();
                            let (tx, rx) = RingBuffer::new(8192);
                            raw = rx;
                            let connected = backend.connect(
                                device,
                                MidiIngress {
                                    tx,
                                    sequence: input_sequence.clone(),
                                    connection,
                                    overflow: overflow.clone(),
                                    emergency: Some(sender.emergency_handle()),
                                },
                            );
                            if let Err(error) = connected {
                                selected_device = None;
                                let _ = messages.send(Message::Connected(false));
                                let _ = messages.send(Message::Error(error));
                            }
                        }
                        for track in &routes {
                            sender.send(MidiAction::Route {
                                track: *track,
                                enabled: selected_device.is_some(),
                            })?;
                        }
                        output = Some(sender);
                        fault_handled = false;
                    }
                    Command::Routes(next) => {
                        if let Some(o) = output.as_mut().filter(|_| selected_device.is_some()) {
                            for track in routes.iter().filter(|id| !next.contains(id)) {
                                o.send_at(
                                    MidiAction::Route {
                                        track: *track,
                                        enabled: false,
                                    },
                                    control_at,
                                )?;
                            }
                            for track in next.iter().filter(|id| !routes.contains(id)) {
                                o.send_at(
                                    MidiAction::Route {
                                        track: *track,
                                        enabled: true,
                                    },
                                    control_at,
                                )?;
                            }
                        }
                        routes = next;
                    }
                    Command::Record(session, rev, count_in_frames) => {
                        if take_busy {
                            if let Some(o) = &mut output {
                                o.send(MidiAction::Seal)?;
                            }
                            return Ok(());
                        }
                        if routes.is_empty() || output.is_none() || !storage_ready {
                            return Err("Select a MIDI track and wait for recovery storage before recording".into());
                        }
                        writer.request(WriteRequest::Begin {
                            session,
                            targets: routes.clone(),
                            revision: rev,
                            count_in: count_in_frames,
                        })?;
                        take_busy = true;
                        record_sent = false;
                        capture_started = false;
                        cancelled = false;
                        let _ = messages.send(Message::Recording(true));
                    }
                    Command::Boundary(action) => {
                        if take_busy && !record_sent {
                            cancelled = true;
                            writer.interrupt(last_frame);
                        }
                        if let Some(o) = &mut output {
                            o.send(action)?;
                        }
                    }
                    Command::Snapshot(snapshot, rev, folder) => {
                        writer.request(WriteRequest::Snapshot(snapshot, rev, folder))?
                    }
                    Command::Acknowledge => writer.request(WriteRequest::Acknowledge)?,
                    Command::Retry => writer.request(WriteRequest::Retry)?,
                    Command::Recover(entry) => writer.request(WriteRequest::Recover(entry))?,
                    Command::OpenSaved(entry) => writer.request(WriteRequest::OpenSaved(entry))?,
                    Command::Discard(entry) => writer.request(WriteRequest::Discard(entry))?,
                }
                Ok(())
            })();
            if let Err(error) = result {
                if error == "__shutdown__" {
                    if let Some(o) = &output {
                        o.panic();
                    }
                    return Ok(());
                }
                let _ = messages.send(Message::Error(error));
                if !take_busy {
                    let _ = messages.send(Message::Recording(false));
                }
            }
        }
        if enumerated.elapsed() >= Duration::from_millis(500) {
            match backend.devices() {
                Ok(devices) => {
                    enumeration_error = None;
                    if selected_device
                        .as_ref()
                        .is_some_and(|id| !devices.iter().any(|d| &d.id == id))
                    {
                        panic.store(true, Ordering::Release);
                        backend.disconnect();
                        if let Some(o) = &mut output {
                            for track in &routes {
                                let _ = o.send(MidiAction::Route {
                                    track: *track,
                                    enabled: false,
                                });
                            }
                        }
                        selected_device = None;
                        connection = connection.wrapping_add(1);
                        let _ = messages.send(Message::Connected(false));
                        let _=messages.send(Message::Error("MIDI input disconnected; held notes released and recording interrupted.".into()));
                    }
                    let _ = messages.send(Message::Devices(devices));
                }
                Err(error) => {
                    if selected_device.is_some() && enumeration_error.as_ref() != Some(&error) {
                        panic.store(true, Ordering::Release);
                        let _ = messages.send(Message::Error(error.clone()));
                    }
                    enumeration_error = Some(error);
                }
            }
            enumerated = Instant::now();
        }
        if let Some(o) = &mut output {
            while let Some(receipt) = o.receipt() {
                last_frame = receipt.frame;
                match receipt.action {
                    MidiAction::Record { .. } => {
                        performance.silence();
                        capture_started = true;
                    }
                    MidiAction::Seal | MidiAction::Stop | MidiAction::Pause | MidiAction::Panic => {
                        performance.silence();
                    }
                    _ => {}
                }
                if take_busy
                    && (capture_started
                        || matches!(
                            receipt.action,
                            MidiAction::Route { .. }
                                | MidiAction::Seal
                                | MidiAction::Stop
                                | MidiAction::Pause
                                | MidiAction::Panic
                        ))
                    && let Err(error) = writer.packet(receipt)
                {
                    o.panic();
                    let _ = messages.send(Message::Error(error));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
