//! Worker-side admission for decoded/prepared PCM and known temporary buffers.
//! Weak ownership includes audio retained by old renderers without keeping it alive.
use crate::{AudioData, Error, Result};
use std::sync::{Mutex, OnceLock, Weak};

/// Shared preparation budget. This is a PCM/known-scratch budget, not a process RSS limit.
pub const AUDIO_MEMORY_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;
type Samples = Weak<Vec<[f32; 2]>>;
type Peaks = Weak<Vec<[[f32; 2]; 2]>>;
#[derive(Default)]
struct State {
    buffers: Vec<(Samples, Peaks)>,
    reserved: u64,
}
impl State {
    fn used(&mut self) -> u64 {
        self.buffers
            .retain(|(s, p)| s.strong_count() != 0 || p.strong_count() != 0);
        let mut samples = std::collections::HashSet::new();
        let mut peaks = std::collections::HashSet::new();
        let mut bytes = self.reserved;
        for (s, p) in &self.buffers {
            if samples.insert(s.as_ptr())
                && let Some(s) = s.upgrade()
            {
                bytes = bytes.saturating_add(s.capacity() as u64 * 8);
            }
            if peaks.insert(p.as_ptr())
                && let Some(p) = p.upgrade()
            {
                bytes = bytes.saturating_add(p.capacity() as u64 * 16);
            }
        }
        bytes
    }
}
fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
pub fn register_audio(audio: &AudioData) {
    let mut state = state().lock().unwrap();
    if !state.buffers.iter().any(|(s, p)| {
        s.as_ptr() == std::sync::Arc::as_ptr(&audio.samples)
            && p.as_ptr() == std::sync::Arc::as_ptr(&audio.peaks)
    }) {
        state.buffers.push((
            std::sync::Arc::downgrade(&audio.samples),
            std::sync::Arc::downgrade(&audio.peaks),
        ));
    }
}
pub struct AudioReservation(u64);
impl AudioReservation {
    /// Reserve output PCM/peaks plus caller-owned temporary bytes before allocation.
    pub fn new(frames: u64, scratch_bytes: u64) -> Result<Self> {
        let bytes = frames
            .checked_mul(8)
            .and_then(|n| {
                frames
                    .div_ceil(256)
                    .checked_mul(16)
                    .and_then(|p| n.checked_add(p))
            })
            .and_then(|n| n.checked_add(scratch_bytes))
            .ok_or_else(|| Error("Audio preparation exceeds supported memory bounds".into()))?;
        // Disposable cache ownership must yield before required audio is rejected.
        // Release it before acquiring the registry lock: dropping buffers updates ownership.
        if bytes > AUDIO_MEMORY_BUDGET_BYTES.saturating_sub(audio_memory_bytes()) {
            crate::release_preparation_cache_memory();
        }
        let mut state = state().lock().unwrap();
        if bytes > AUDIO_MEMORY_BUDGET_BYTES.saturating_sub(state.used()) {
            return Err(Error("Audio preparation exceeds the 2 GiB shared audio memory budget; shorten the source or close unused projects".into()));
        }
        state.reserved += bytes;
        Ok(Self(bytes))
    }
}
impl Drop for AudioReservation {
    fn drop(&mut self) {
        state().lock().unwrap().reserved -= self.0;
    }
}

/// Deduplicate shared PCM/peak owners across plans, history and sampler variants.
/// Construct and traverse on a control/worker thread only.
#[derive(Default)]
pub struct AudioMemoryUsage {
    samples: std::collections::HashSet<usize>,
    peaks: std::collections::HashSet<usize>,
    pub bytes: u64,
}
impl AudioMemoryUsage {
    pub fn samples(&mut self, samples: &std::sync::Arc<Vec<[f32; 2]>>) {
        if self
            .samples
            .insert(std::sync::Arc::as_ptr(samples) as usize)
        {
            self.bytes = self.bytes.saturating_add(samples.capacity() as u64 * 8);
        }
    }
    pub fn audio(&mut self, audio: &AudioData) {
        self.samples(&audio.samples);
        if self
            .peaks
            .insert(std::sync::Arc::as_ptr(&audio.peaks) as usize)
        {
            self.bytes = self
                .bytes
                .saturating_add(audio.peaks.capacity() as u64 * 16);
        }
    }
}
/// PCM and scratch currently admitted, including old owners. Not a process RSS reading.
pub fn audio_memory_bytes() -> u64 {
    state().lock().unwrap().used()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_rejects_oversized_requests_before_allocating_and_counts_shared_owners_once() {
        assert!(AudioReservation::new(u64::MAX, 0).is_err());
        assert!(AudioReservation::new(AUDIO_MEMORY_BUDGET_BYTES / 8, 0).is_err());
        let samples = std::sync::Arc::new(vec![[0.0; 2]; 8]);
        let peaks = std::sync::Arc::new(vec![[[0.0; 2]; 2]; 1]);
        let pair = (
            std::sync::Arc::downgrade(&samples),
            std::sync::Arc::downgrade(&peaks),
        );
        let mut state = State {
            buffers: vec![pair.clone(), pair],
            reserved: 32,
        };
        assert_eq!(state.used(), 32 + 8 * 8 + 16);
        let retired_renderer = samples.clone();
        drop((samples, peaks));
        assert_eq!(state.used(), 32 + 8 * 8);
        drop(retired_renderer);
        assert_eq!(state.used(), 32);
    }
}
