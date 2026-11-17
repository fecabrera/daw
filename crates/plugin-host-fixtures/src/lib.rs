//! Owned synthetic hosting fixtures. These are test binaries, not DAW products.
mod clap;
mod vst3;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU64, Ordering},
};
pub const EFFECT_CLAP_ID: &str = "org.daw.host-fixture.effect";
pub const INSTRUMENT_CLAP_ID: &str = "org.daw.host-fixture.instrument";
pub const EFFECT_VST3_ID: [u8; 16] = [
    0x48, 0x4f, 0x53, 0x54, 0x46, 0x49, 0x58, 0x54, 0x55, 0x52, 0x45, 0x45, 0x46, 0x58, 0, 1,
];
pub const INSTRUMENT_VST3_ID: [u8; 16] = [
    0x48, 0x4f, 0x53, 0x54, 0x46, 0x49, 0x58, 0x54, 0x55, 0x52, 0x45, 0x53, 0x59, 0x4e, 0, 1,
];
const LATENCY: usize = 3;
#[derive(Clone, Copy)]
struct Note {
    age: u64,
    id: i32,
    key: i16,
    channel: i16,
    velocity: f64,
    active: bool,
}
struct Audio {
    notes: [Note; 256],
    delay: [[f32; 2]; LATENCY],
    cursor: usize,
}
impl Audio {
    fn new() -> Self {
        Self {
            notes: [Note {
                age: 0,
                id: 0,
                key: 0,
                channel: 0,
                velocity: 0.0,
                active: false,
            }; 256],
            delay: [[0.0; 2]; LATENCY],
            cursor: 0,
        }
    }
    fn note(&mut self, on: bool, id: i32, key: i16, channel: i16, velocity: f64) {
        if on {
            if let Some(note) = self.notes.iter_mut().find(|n| !n.active) {
                *note = Note {
                    age: 0,
                    id,
                    key,
                    channel,
                    velocity,
                    active: true,
                };
            }
        } else {
            for note in &mut self.notes {
                if note.active
                    && (id >= 0 && note.id == id
                        || id < 0 && note.key == key && note.channel == channel)
                {
                    note.active = false;
                }
            }
        }
    }
    fn sample(&mut self, instrument: bool, gain: f64, main: [f32; 2], aux: [f32; 2]) -> [f32; 2] {
        let sum = self
            .notes
            .iter()
            .filter(|n| n.active)
            .map(|n| (f64::from(n.key) + 1.0) / 128.0 * n.velocity * (n.age as f64 / 4.0).min(1.0))
            .sum::<f64>() as f32;
        for note in self.notes.iter_mut().filter(|n| n.active) {
            note.age = note.age.saturating_add(1);
        }
        let input = std::array::from_fn(|ch| {
            ((if instrument {
                sum * if ch == 0 { 1.0 } else { 0.5 }
            } else {
                main[ch]
            }) + aux[ch])
                * gain as f32
        });
        let output = self.delay[self.cursor];
        self.delay[self.cursor] = input;
        self.cursor = (self.cursor + 1) % LATENCY;
        output
    }
}
struct Processor {
    instrument: bool,
    gain: AtomicU64,
    audio: UnsafeCell<Audio>,
}
impl Processor {
    fn new(instrument: bool) -> Self {
        Self {
            instrument,
            gain: AtomicU64::new(1.0f64.to_bits()),
            audio: UnsafeCell::new(Audio::new()),
        }
    }
    fn gain(&self) -> f64 {
        f64::from_bits(self.gain.load(Ordering::Acquire))
    }
    fn set_gain(&self, value: f64) {
        self.gain
            .store(value.clamp(0.0, 2.0).to_bits(), Ordering::Release);
    }
    fn state(&self) -> [u8; 16] {
        let mut state = *b"DHOSTFX1\0\0\0\0\0\0\0\0";
        state[8..].copy_from_slice(&self.gain().to_le_bytes());
        state
    }
    fn restore(&self, state: [u8; 16]) -> bool {
        if &state[..8] != b"DHOSTFX1" {
            return false;
        }
        let value = f64::from_le_bytes(state[8..].try_into().expect("fixed state"));
        if !value.is_finite() || !(0.0..=2.0).contains(&value) {
            return false;
        }
        self.set_gain(value);
        true
    }
}
