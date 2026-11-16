//! Bounded physical-key ownership and sustain reduction, shared by input and capture.
use daw_core::Id;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiInputEvent {
    Attack { token: Id, pitch: u8, velocity: u8 },
    Release { token: Id },
    Panic,
}
#[derive(Clone, Copy)]
struct Key {
    token: Id,
    channel: u8,
    pitch: u8,
    down: bool,
    sounding: bool,
}
pub struct MidiPerformance {
    keys: [Option<Key>; 256],
    pedal: [bool; 16],
    next: u128,
}
impl Default for MidiPerformance {
    fn default() -> Self {
        Self {
            keys: [None; 256],
            pedal: [false; 16],
            next: 0,
        }
    }
}
impl MidiPerformance {
    /// Clear sound ownership but retain physical off tombstones until their keys rise.
    pub fn silence(&mut self) {
        self.pedal.fill(false);
        for slot in &mut self.keys {
            if let Some(key) = slot {
                if key.down {
                    key.sounding = false;
                } else {
                    *slot = None;
                }
            }
        }
    }
    pub fn disconnect(&mut self) {
        self.keys.fill(None);
        self.pedal.fill(false);
    }
    pub fn process(
        &mut self,
        bytes: [u8; 3],
        filter: Option<u8>,
        mut emit: impl FnMut(MidiInputEvent) -> Result<(), String>,
    ) -> Result<(), String> {
        let [status, pitch, value] = bytes;
        let channel = status & 15;
        if pitch > 127 || value > 127 || filter.is_some_and(|c| c != channel) {
            return Ok(());
        }
        match status & 0xf0 {
            0x90 if value > 0 => {
                let slot = self
                    .keys
                    .iter_mut()
                    .find(|k| k.is_none())
                    .ok_or("MIDI physical-note limit reached; take interrupted")?;
                self.next = self.next.checked_add(1).ok_or("MIDI token overflow")?;
                let token = Id::from_u128(self.next);
                *slot = Some(Key {
                    token,
                    channel,
                    pitch,
                    down: true,
                    sounding: true,
                });
                emit(MidiInputEvent::Attack {
                    token,
                    pitch,
                    velocity: value,
                })?;
            }
            0x80 | 0x90 => {
                if let Some(index) = self
                    .keys
                    .iter()
                    .enumerate()
                    .filter_map(|(i, k)| {
                        k.filter(|k| k.channel == channel && k.pitch == pitch && k.down)
                            .map(|k| (i, k.token))
                    })
                    .min_by_key(|(_, id)| *id)
                    .map(|(i, _)| i)
                {
                    let key = self.keys[index].as_mut().unwrap();
                    key.down = false;
                    if !self.pedal[usize::from(channel)] || !key.sounding {
                        let key = self.keys[index].take().unwrap();
                        if key.sounding {
                            emit(MidiInputEvent::Release { token: key.token })?;
                        }
                    }
                }
            }
            0xb0 if pitch == 64 => {
                self.pedal[usize::from(channel)] = value >= 64;
                if value < 64 {
                    for slot in &mut self.keys {
                        if slot.is_some_and(|k| k.channel == channel && !k.down) {
                            let key = slot.take().unwrap();
                            if key.sounding {
                                emit(MidiInputEvent::Release { token: key.token })?;
                            }
                        }
                    }
                }
            }
            0xb0 if matches!(pitch, 120 | 123) => {
                self.silence();
                emit(MidiInputEvent::Panic)?;
            }
            _ => {}
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn feed(p: &mut MidiPerformance, bytes: [u8; 3]) -> Vec<MidiInputEvent> {
        let mut events = Vec::new();
        p.process(bytes, None, |e| {
            events.push(e);
            Ok(())
        })
        .unwrap();
        events
    }
    #[test]
    fn repeated_keys_pair_fifo_and_sustain_keeps_independent_tokens() {
        let mut p = MidiPerformance::default();
        let first = feed(&mut p, [0x90, 60, 70]);
        let second = feed(&mut p, [0x90, 60, 100]);
        let MidiInputEvent::Attack { token: a, .. } = first[0] else {
            panic!()
        };
        let MidiInputEvent::Attack { token: b, .. } = second[0] else {
            panic!()
        };
        feed(&mut p, [0xb0, 64, 127]);
        assert!(feed(&mut p, [0x80, 60, 0]).is_empty());
        assert!(feed(&mut p, [0x90, 60, 0]).is_empty());
        assert_eq!(
            feed(&mut p, [0xb0, 64, 0]),
            vec![
                MidiInputEvent::Release { token: a },
                MidiInputEvent::Release { token: b }
            ]
        );
    }
    #[test]
    fn old_physical_off_after_panic_cannot_release_a_new_press() {
        let mut p = MidiPerformance::default();
        feed(&mut p, [0x90, 60, 70]);
        p.silence();
        let attack = feed(&mut p, [0x90, 60, 100]);
        assert!(feed(&mut p, [0x80, 60, 0]).is_empty());
        let MidiInputEvent::Attack { token, .. } = attack[0] else {
            panic!()
        };
        assert_eq!(
            feed(&mut p, [0x80, 60, 0]),
            vec![MidiInputEvent::Release { token }]
        );
    }
    #[test]
    fn filter_and_capacity_are_explicit() {
        let mut p = MidiPerformance::default();
        let mut called = false;
        p.process([0x91, 64, 127], Some(0), |_| {
            called = true;
            Ok(())
        })
        .unwrap();
        assert!(!called);
        for _ in 0..256 {
            feed(&mut p, [0x90, 60, 127]);
        }
        assert!(p.process([0x90, 60, 127], None, |_| Ok(())).is_err());
    }
}
