//! A regular-file snapshot shared by decoder probe and validation readers.
use crate::{Error, Result};
use std::{
    fs::{File, Metadata},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex},
};
use symphonia::core::io::MediaSource;

const MIB: u64 = 1024 * 1024;
const ENCODED_BYTES: u64 = 256 * MIB;
const READ_BYTES: u64 = 512 * MIB;
const PCM_BYTES: u64 = 2048 * MIB;
const PACKETS: u64 = 100_000;
const SECONDS: u64 = 600;

struct State {
    file: File,
    remaining: Option<u64>,
    exhausted: bool,
    position: u64,
}

/// Each clone has an independent position and shares the same open handle
/// and cumulative read budget.
#[derive(Clone)]
pub(crate) struct Input {
    state: Arc<Mutex<State>>,
    position: u64,
    metadata: Arc<Metadata>,
}

impl Input {
    pub fn open(path: &Path, bounded: bool) -> io::Result<Self> {
        if bounded && !path.metadata()?.is_file() {
            return Err(io::Error::other("Comparison input must be a regular file"));
        }
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        if bounded && (!metadata.is_file() || metadata.len() > ENCODED_BYTES) {
            return Err(io::Error::other("Encoded audio exceeds comparison limits"));
        }
        Ok(Self {
            state: Arc::new(Mutex::new(State {
                file,
                remaining: bounded.then_some(READ_BYTES),
                exhausted: false,
                position: 0,
            })),
            position: 0,
            metadata: Arc::new(metadata),
        })
    }

    pub fn length(&self) -> u64 {
        self.metadata.len()
    }

    pub fn verify(&self) -> Result<()> {
        let state = self.state.lock().unwrap();
        if state.exhausted {
            return Err(Error("Comparison encoded read budget exceeded".into()));
        }
        if state.remaining.is_some() {
            let current = state.file.metadata().map_err(|e| Error(e.to_string()))?;
            if current.len() != self.metadata.len()
                || current.modified().ok() != self.metadata.modified().ok()
            {
                return Err(Error("Comparison input changed during preparation".into()));
            }
        }
        Ok(())
    }
}

impl Read for Input {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut state = self.state.lock().unwrap();
        let wanted = if state.remaining.is_some() {
            self.length()
                .saturating_sub(self.position)
                .min(out.len() as u64)
        } else {
            out.len() as u64
        };
        if wanted == 0 {
            return Ok(0);
        }
        let allowed = state
            .remaining
            .map_or(wanted, |remaining| remaining.min(wanted));
        if allowed == 0 {
            state.exhausted = true;
            return Err(io::Error::other("Comparison encoded read budget exceeded"));
        }
        if state.position != self.position {
            state.position = state.file.seek(SeekFrom::Start(self.position))?;
        }
        let read = state.file.read(&mut out[..allowed as usize])?;
        self.position += read as u64;
        state.position = self.position;
        if let Some(remaining) = &mut state.remaining {
            *remaining -= read as u64;
        }
        Ok(read)
    }
}

impl Seek for Input {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(n) => self.position.checked_add_signed(n),
            SeekFrom::End(n) => self.length().checked_add_signed(n),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid media seek"))?;
        self.position = position;
        Ok(position)
    }
}

impl MediaSource for Input {
    fn is_seekable(&self) -> bool {
        self.metadata.is_file()
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.length())
    }
}

#[derive(Default)]
pub(crate) struct Budget {
    pub enabled: bool,
    packets: u64,
    native: u64,
}

impl Budget {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub fn packet(&mut self) -> Result<()> {
        if self.enabled {
            self.packets += 1;
            if self.packets > PACKETS {
                return Err(Error("Comparison packet budget exceeded".into()));
            }
        }
        Ok(())
    }

    pub fn frames(&mut self, count: u64, native_rate: u32, prepared_rate: u32) -> Result<()> {
        if self.enabled {
            self.native = self
                .native
                .checked_add(count)
                .filter(|&n| n <= SECONDS * u64::from(native_rate))
                .ok_or_else(|| Error("Comparison native frame budget exceeded".into()))?;
            let prepared = crate::resampled_frames(self.native, native_rate, prepared_rate)?;
            if prepared > SECONDS * u64::from(prepared_rate)
                || self.native.saturating_add(prepared).saturating_mul(8) > PCM_BYTES
            {
                return Err(Error("Comparison prepared PCM budget exceeded".into()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_clones_share_budget_with_independent_positions() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-2.m4a");
        let mut first = Input::open(&path, true).unwrap();
        first.state.lock().unwrap().remaining = Some(6);
        let mut second = first.clone();
        let mut a = [0; 3];
        let mut b = [0; 3];
        first.read_exact(&mut a).unwrap();
        second.read_exact(&mut b).unwrap();
        assert_eq!(a, b);
        assert!(second.read_exact(&mut b).is_err());
        assert!(first.verify().is_err());
    }

    #[test]
    fn frame_and_packet_budgets_are_cumulative() {
        let mut budget = Budget::new(true);
        let rate = 48_000;
        budget.frames(u64::from(rate) * 599, rate, rate).unwrap();
        budget.frames(u64::from(rate), rate, rate).unwrap();
        assert!(budget.frames(1, rate, rate).is_err());

        for _ in 0..PACKETS {
            budget.packet().unwrap();
        }
        assert!(budget.packet().is_err());
        assert!(
            Budget::new(true)
                .frames(600 * 384_000, 384_000, 192_000)
                .is_err()
        );
    }
}
