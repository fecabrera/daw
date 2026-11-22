//! Worker-owned capture, sustain-resolved note lengths and durable whole-take publication.
use crate::{
    Result, Session, error,
    recovery::{self, RecoveryEntry, RecoveryStore},
};
use daw_core::{Id, MidiNote, MidiPattern, RecordedMidiTrack};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    time::{Duration, Instant},
};

const EVENT_LIMIT: usize = 1_000_000;
const JOURNAL_LIMIT: u64 = 256 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CaptureAction {
    Route {
        track: Id,
        enabled: bool,
    },
    Attack {
        token: Id,
        pitch: u8,
        velocity: u8,
    },
    ChannelAttack {
        token: Id,
        pitch: u8,
        velocity: u8,
        channel: u8,
    },
    Release {
        token: Id,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub frame: u64,
    pub action: CaptureAction,
}
#[derive(Clone, Serialize, Deserialize)]
struct Header {
    version: u32,
    id: Id,
    base: RecoveryEntry,
    targets: Vec<Id>,
}
struct OpenNote {
    start: u64,
    pitch: u8,
    velocity: u8,
}
pub struct MidiCapture {
    pub id: Id,
    pub base: Session,
    origin: Option<u64>,
    last: u64,
    routes: BTreeSet<Id>,
    open: BTreeMap<(Id, Id), OpenNote>,
    notes: BTreeMap<Id, Vec<(u64, u64, u8, u8)>>,
    count: usize,
    closed_count: usize,
}
impl MidiCapture {
    pub fn new(base: Session, id: Id, targets: &[Id]) -> Self {
        Self {
            id,
            base,
            origin: None,
            last: 0,
            routes: targets.iter().copied().collect(),
            open: BTreeMap::new(),
            notes: BTreeMap::new(),
            count: 0,
            closed_count: 0,
        }
    }
    pub fn start(&mut self, frame: u64) {
        self.origin = Some(frame);
        self.last = frame;
    }
    fn validate_event(&self, event: CaptureEvent) -> Result<()> {
        if let CaptureAction::Route {
            track,
            enabled: true,
        } = event.action
            && !self
                .base
                .project
                .tracks
                .iter()
                .any(|t| t.id == track && t.kind == daw_core::TrackKind::Midi)
        {
            return Err(error("MIDI recording target is unavailable"));
        }
        if let Some(origin) = self.origin {
            if event.frame < origin || event.frame < self.last {
                return Err(error("MIDI capture time moved backwards"));
            }
            if event.frame - origin > u64::from(self.base.project.sample_rate_hz) * 3600
                || self.count >= EVENT_LIMIT
            {
                return Err(error("MIDI take reached its recording limit"));
            }
            if let CaptureAction::Attack {
                pitch, velocity, ..
            }
            | CaptureAction::ChannelAttack {
                pitch, velocity, ..
            } = event.action
            {
                if pitch > 127 || !(1..=127).contains(&velocity) {
                    return Err(error("Invalid captured MIDI note"));
                }
                if self
                    .closed_count
                    .saturating_add(self.open.len())
                    .saturating_add(self.routes.len())
                    > EVENT_LIMIT
                {
                    return Err(error("MIDI take note limit reached"));
                }
            }
        }
        Ok(())
    }
    pub fn event(&mut self, event: CaptureEvent) -> Result<()> {
        self.validate_event(event)?;
        let Some(origin) = self.origin else {
            if let CaptureAction::Route { track, enabled } = event.action {
                if !self
                    .base
                    .project
                    .tracks
                    .iter()
                    .any(|t| t.id == track && t.kind == daw_core::TrackKind::Midi)
                {
                    return Err(error("MIDI recording target is unavailable"));
                }
                if enabled {
                    self.routes.insert(track);
                } else {
                    self.routes.remove(&track);
                }
            }
            return Ok(());
        };
        if event.frame < origin || event.frame < self.last {
            return Err(error("MIDI capture time moved backwards"));
        }
        if event.frame.saturating_sub(origin) > u64::from(self.base.project.sample_rate_hz) * 3600
            || self.count >= EVENT_LIMIT
        {
            return Err(error("MIDI take reached its recording limit"));
        }
        self.last = event.frame;
        self.count += 1;
        match event.action {
            CaptureAction::Route { track, enabled } => {
                if enabled {
                    if !self
                        .base
                        .project
                        .tracks
                        .iter()
                        .any(|t| t.id == track && t.kind == daw_core::TrackKind::Midi)
                    {
                        return Err(error("MIDI recording target is unavailable"));
                    }
                    self.routes.insert(track);
                } else {
                    self.routes.remove(&track);
                    let tokens: Vec<_> = self
                        .open
                        .keys()
                        .filter(|(owner, _)| *owner == track)
                        .copied()
                        .collect();
                    for token in tokens {
                        self.close(token, event.frame)?;
                    }
                }
            }
            CaptureAction::Attack {
                token,
                pitch,
                velocity,
            }
            | CaptureAction::ChannelAttack {
                token,
                pitch,
                velocity,
                ..
            } => {
                let channel = match event.action {
                    CaptureAction::ChannelAttack { channel, .. } if channel < 16 => Some(channel),
                    CaptureAction::ChannelAttack { .. } => {
                        return Err(error("Invalid captured MIDI channel"));
                    }
                    _ => None,
                };
                if pitch > 127 || !(1..=127).contains(&velocity) {
                    return Err(error("Invalid captured MIDI note"));
                }
                if self
                    .open
                    .len()
                    .checked_add(self.routes.len())
                    .is_none_or(|n| n > EVENT_LIMIT)
                {
                    return Err(error("MIDI capture ownership limit reached"));
                }
                for track in &self.base.project.tracks {
                    if !self.routes.contains(&track.id)
                        || !track.midi_options.accepts_channel(channel)
                    {
                        continue;
                    }
                    self.open.insert(
                        (track.id, token),
                        OpenNote {
                            start: event.frame,
                            pitch,
                            velocity,
                        },
                    );
                }
            }
            CaptureAction::Release { token } => {
                let tokens: Vec<_> = self
                    .open
                    .keys()
                    .filter(|(_, id)| *id == token)
                    .copied()
                    .collect();
                for token in tokens {
                    self.close(token, event.frame)?;
                }
            }
        }
        Ok(())
    }
    fn close(&mut self, key: (Id, Id), end: u64) -> Result<()> {
        if let Some(note) = self.open.remove(&key) {
            if self.closed_count >= EVENT_LIMIT {
                return Err(error("MIDI take note limit reached"));
            }
            self.closed_count += 1;
            self.notes.entry(key.0).or_default().push((
                note.start,
                end.max(note.start),
                note.pitch,
                note.velocity,
            ));
        }
        Ok(())
    }
    pub fn finish(mut self, end: u64) -> Result<Session> {
        let Some(origin) = self.origin else {
            return Ok(self.base);
        };
        let end = end.max(self.last);
        let tokens: Vec<_> = self.open.keys().copied().collect();
        for token in tokens {
            self.close(token, end)?;
        }
        let project = &self.base.project;
        let clock = project.musical_clock().map_err(|e| error(e.to_string()))?;
        let tick = |frame: u64| -> Result<u32> {
            let ticks = clock.interval_quarters(origin, frame).max(0.0) * 960.0;
            if !ticks.is_finite() || ticks > f64::from(u32::MAX - 1) {
                return Err(error("Recorded MIDI timing exceeds pattern bounds"));
            }
            Ok(ticks.round() as u32)
        };
        let mut tracks = Vec::new();
        for (track_id, notes) in &self.notes {
            let mut pattern = MidiPattern {
                length_ticks: tick(end)?.max(1),
                notes: Vec::new(),
            };
            for &(start, end, pitch, velocity) in notes {
                let start_tick = tick(start)?;
                let end_tick = tick(end)?.max(
                    start_tick
                        .checked_add(1)
                        .ok_or_else(|| error("MIDI note length overflow"))?,
                );
                pattern.length_ticks = pattern.length_ticks.max(end_tick);
                pattern.notes.push(MidiNote {
                    id: Id::new_v4(),
                    start_tick,
                    length_ticks: end_tick - start_tick,
                    pitch,
                    velocity,
                });
            }
            // The shared forward conversion must contain the requested exclusive end.
            while clock
                .frame_for_reference(
                    origin,
                    project.midi_frame(pattern.length_ticks),
                    project.tempo_bpm,
                )
                .map_err(error)?
                < end
            {
                pattern.length_ticks = pattern
                    .length_ticks
                    .checked_add(1)
                    .ok_or_else(|| error("MIDI take length overflow"))?;
            }
            pattern.notes.sort_by_key(|n| (n.start_tick, n.pitch, n.id));
            tracks.push(RecordedMidiTrack {
                track_id: *track_id,
                pattern,
            });
        }
        let mut next = self.base.clone();
        next.project = project.recorded_midi_take(origin, &tracks).map_err(error)?;
        Ok(next)
    }
}

pub struct TakeJournal {
    pub capture: MidiCapture,
    store: RecoveryStore,
    file: File,
    bytes: u64,
    synced: Instant,
    header: Header,
}
#[derive(Serialize, Deserialize)]
enum Record {
    Start(u64),
    Event(CaptureEvent),
    End(u64),
}
impl TakeJournal {
    pub fn begin(
        store: RecoveryStore,
        session: Session,
        targets: &[Id],
        revision: u64,
    ) -> Result<Self> {
        if targets.is_empty() {
            return Err(error("Select a MIDI track or enable R before recording"));
        }
        store.reserve_take(&session.project)?;
        let id = Id::new_v4();
        let base = store.checkpoint(
            session.save_snapshot(None)?,
            revision,
            session.folder.clone(),
            None,
        )?;
        let header = Header {
            version: 1,
            id,
            base,
            targets: targets.to_vec(),
        };
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(store.folder.join(format!("take-{id}.journal")))
            .map_err(error)?;
        file.sync_all().map_err(error)?;
        recovery::write_new(
            &store.folder.join("active-take"),
            &serde_json::to_vec(&header).map_err(error)?,
        )?;
        Ok(Self {
            capture: MidiCapture::new(session, id, targets),
            store,
            file,
            bytes: 0,
            synced: Instant::now(),
            header,
        })
    }
    fn append(&mut self, record: &Record) -> Result<()> {
        let bytes = serde_json::to_vec(record).map_err(error)?;
        let needed = bytes.len() as u64 + 36;
        if self.bytes.saturating_add(needed) > JOURNAL_LIMIT {
            return Err(error("MIDI journal reached 256 MiB; take interrupted"));
        }
        let written = self
            .file
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .and_then(|_| self.file.write_all(&Sha256::digest(&bytes)))
            .and_then(|_| self.file.write_all(&bytes));
        if let Err(failure) = written {
            use std::io::{Seek, SeekFrom};
            self.file.set_len(self.bytes).map_err(error)?;
            self.file.seek(SeekFrom::Start(self.bytes)).map_err(error)?;
            return Err(error(failure));
        }
        self.bytes += needed;
        if self.synced.elapsed() >= Duration::from_millis(250) {
            self.sync()?;
        }
        Ok(())
    }
    pub fn sync(&mut self) -> Result<()> {
        self.file.sync_all().map_err(error)?;
        self.synced = Instant::now();
        Ok(())
    }
    pub fn start(&mut self, frame: u64) -> Result<()> {
        self.append(&Record::Start(frame))?;
        self.capture.start(frame);
        Ok(())
    }
    pub fn event(&mut self, event: CaptureEvent) -> Result<()> {
        self.capture.validate_event(event)?;
        self.append(&Record::Event(event))?;
        self.capture.event(event)
    }
    pub fn finish(mut self, frame: u64, revision: u64) -> Result<(Session, RecoveryEntry)> {
        self.append(&Record::End(frame))?;
        self.sync()?;
        let folder = self.capture.base.folder.clone();
        let session = self.capture.finish(frame)?;
        let receipt = self.store.checkpoint_take(
            session.save_snapshot(None)?,
            revision,
            folder,
            self.header.id,
            self.header.base.generation.clone(),
        )?;
        // Keep the base/journal pinned until the UI has acknowledged the logical edit.
        Ok((session, receipt))
    }
    pub fn acknowledge(store: &RecoveryStore) -> Result<()> {
        let path = store.folder.join("active-take");
        if path.exists() {
            let header: Header =
                serde_json::from_slice(&recovery::read_bounded(&path)?).map_err(error)?;
            if !RecoveryStore::valid_entries(&store.folder)?
                .iter()
                .any(|e| e.applied_take == Some(header.id))
            {
                return Err(error(
                    "Keep the MIDI journal until its whole-take receipt is durable",
                ));
            }
            fs::remove_file(&path).map_err(error)?;
            recovery::sync_dir(&store.folder)?;
            let journal = store.folder.join(format!("take-{}.journal", header.id));
            if journal.exists() {
                fs::remove_file(journal).map_err(error)?;
            }
        }
        let pending = store.folder.join("recovered-pending");
        if pending.exists() {
            fs::remove_file(pending).map_err(error)?;
        }
        recovery::sync_dir(&store.folder)
    }
    /// Recover a contiguous checksummed prefix. Never apply only a subset of tracks.
    pub fn recover_pending(store: &RecoveryStore) -> Result<Option<RecoveryEntry>> {
        let path = store.folder.join("active-take");
        if !path.exists() {
            return Ok(None);
        }
        let header: Header =
            serde_json::from_slice(&recovery::read_bounded(&path)?).map_err(error)?;
        if store
            .folder
            .join(format!("discard-{}", header.base.project))
            .exists()
        {
            return Ok(None);
        }
        if header.version != 1 || header.base.generation.parent() != Some(store.folder.as_path()) {
            return Err(error("Invalid pending MIDI take owner"));
        }
        let existing = RecoveryStore::valid_entries(&store.folder)?;
        if let Some(entry) = existing
            .into_iter()
            .find(|e| e.applied_take == Some(header.id))
        {
            return Ok(Some(entry));
        }
        let base = Session::open_document(&header.base.generation)?.into_editor_session();
        let mut capture = MidiCapture::new(base, header.id, &header.targets);
        let journal_path = store.folder.join(format!("take-{}.journal", header.id));
        recovery::ordinary(&journal_path, false)?;
        let mut file = File::open(journal_path).map_err(error)?.take(JOURNAL_LIMIT);
        let mut end = 0;
        loop {
            let mut length = [0; 4];
            if file.read_exact(&mut length).is_err() {
                break;
            }
            let length = u32::from_le_bytes(length) as usize;
            if length > 4096 {
                break;
            }
            let mut hash = [0; 32];
            if file.read_exact(&mut hash).is_err() {
                break;
            }
            let mut bytes = vec![0; length];
            if file.read_exact(&mut bytes).is_err() || Sha256::digest(&bytes).as_slice() != hash {
                break;
            }
            let record: Record = serde_json::from_slice(&bytes).map_err(error)?;
            match record {
                Record::Start(frame) => {
                    capture.start(frame);
                    end = frame;
                }
                Record::Event(event) => {
                    end = event.frame;
                    capture.event(event)?;
                }
                Record::End(frame) => {
                    end = frame;
                }
            }
        }
        let session = capture.finish(end)?;
        store
            .checkpoint_take(
                session.save_snapshot(None)?,
                header.base.revision.saturating_add(1),
                header.base.original_folder.clone(),
                header.id,
                header.base.generation.clone(),
            )
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> (Session, [Id; 3]) {
        let mut s = Session::default();
        let a = s.project.add_midi_track().unwrap();
        let b = s.project.add_midi_track().unwrap();
        let c = s.project.add_midi_track().unwrap();
        (s, [a, b, c])
    }
    #[test]
    fn whole_take_overwrites_both_tracks_and_round_trips_history_and_save() {
        let (mut base, [a, b, _]) = session();
        for track in &mut base.project.tracks[..2] {
            track.instrument = Some(daw_core::OscillatorSettings::default().into());
        }
        base.project.add_midi_pattern(a, 0).unwrap();
        base.project.add_midi_pattern(b, 0).unwrap();
        let original = base.project.clone();
        let mut capture = MidiCapture::new(base.clone(), Id::new_v4(), &[a, b]);
        capture.start(6000);
        let token = Id::new_v4();
        capture
            .event(CaptureEvent {
                frame: 6000,
                action: CaptureAction::Attack {
                    token,
                    pitch: 60,
                    velocity: 100,
                },
            })
            .unwrap();
        capture
            .event(CaptureEvent {
                frame: 12000,
                action: CaptureAction::Release { token },
            })
            .unwrap();
        let result = capture.finish(12000).unwrap();
        assert_eq!(base.project, original);
        assert_eq!(result.project.tracks[0].clips.len(), 3);
        assert_eq!(result.project.tracks[1].clips.len(), 3);
        result.project.validate().unwrap();
        let mut history = crate::SessionHistory::new(&base, false).unwrap();
        history.record(&result).unwrap();
        let undone = history.undo(&result).unwrap();
        assert_eq!(undone.project, original);
        let redone = history.redo(&undone).unwrap();
        assert_eq!(redone.project, result.project);
        let folder = std::env::temp_dir().join(format!("daw-midi-save-{}", Id::new_v4()));
        result
            .save_snapshot(Some(&history))
            .unwrap()
            .save(&folder)
            .unwrap();
        let reopened = Session::open_document(&folder)
            .unwrap()
            .into_editor_session();
        assert_eq!(reopened.project, result.project);
        let recorded = folder.join("recorded.wav");
        let rendered = folder.join("reopened.wav");
        result.export(&recorded, false).unwrap();
        reopened.export(&rendered, false).unwrap();
        assert_eq!(fs::read(recorded).unwrap(), fs::read(rendered).unwrap());
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn dynamic_routes_close_removed_targets_and_never_replay_held_attacks_to_additions() {
        let (s, [a, b, c]) = session();
        let mut take = MidiCapture::new(s, Id::new_v4(), &[a, b]);
        take.start(100);
        let t = Id::new_v4();
        take.event(CaptureEvent {
            frame: 100,
            action: CaptureAction::Attack {
                token: t,
                pitch: 60,
                velocity: 95,
            },
        })
        .unwrap();
        take.event(CaptureEvent {
            frame: 200,
            action: CaptureAction::Route {
                track: b,
                enabled: false,
            },
        })
        .unwrap();
        take.event(CaptureEvent {
            frame: 250,
            action: CaptureAction::Route {
                track: c,
                enabled: true,
            },
        })
        .unwrap();
        take.event(CaptureEvent {
            frame: 400,
            action: CaptureAction::Release { token: t },
        })
        .unwrap();
        let result = take.finish(450).unwrap();
        for (index, length) in [(0, 12), (1, 4)] {
            let clip = &result.project.tracks[index].clips[0];
            assert_eq!(clip.start_frame, 100);
            let pattern = result
                .project
                .assets
                .iter()
                .find(|x| x.id == clip.asset_id)
                .unwrap()
                .midi_pattern
                .as_ref()
                .unwrap();
            assert_eq!(pattern.notes.len(), 1);
            assert_eq!(pattern.notes[0].length_ticks, length);
            assert_eq!(pattern.notes[0].velocity, 95);
        }
        assert!(result.project.tracks[2].clips.is_empty());
    }
    #[test]
    fn recovery_replays_a_durable_multitrack_prefix_once_and_discard_never_resurrects_it() {
        let root = std::env::temp_dir().join(format!("daw-midi-recovery-{}", Id::new_v4()));
        let (s, [a, b, _]) = session();
        let store = RecoveryStore::create(&root).unwrap();
        let mut take = TakeJournal::begin(store.clone(), s, &[a, b], 0).unwrap();
        take.start(0).unwrap();
        take.event(CaptureEvent {
            frame: 0,
            action: CaptureAction::Attack {
                token: Id::new_v4(),
                pitch: 64,
                velocity: 127,
            },
        })
        .unwrap();
        take.event(CaptureEvent {
            frame: 24000,
            action: CaptureAction::Route {
                track: a,
                enabled: true,
            },
        })
        .unwrap();
        take.sync().unwrap();
        take.file.write_all(&[32, 0]).unwrap(); // Torn next length leaves the accepted prefix intact.
        take.sync().unwrap();
        drop(take);
        drop(store);
        let entries = RecoveryStore::scan(&root).unwrap();
        assert_eq!(entries.len(), 1);
        let entry = entries[0].clone();
        let p = Session::open_document(&entry.generation)
            .unwrap()
            .into_editor_session()
            .project;
        assert_eq!(p.tracks[0].clips.len(), 1);
        assert_eq!(p.tracks[1].clips.len(), 1);
        assert_eq!(RecoveryStore::scan(&root).unwrap()[0].id, entry.id);
        let recovered = RecoveryStore::recover(&entry).unwrap();
        let before = recovered
            .recovered_take_before
            .as_ref()
            .expect("interrupted take retains its undo base");
        assert!(before.project.tracks.iter().all(|t| t.clips.is_empty()));
        assert_eq!(recovered.project.tracks[0].clips.len(), 1);
        assert_eq!(recovered.project.tracks[1].clips.len(), 1);
        drop(recovered);
        RecoveryStore::discard(&entry).unwrap();
        assert!(RecoveryStore::scan(&root).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
