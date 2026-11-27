//! Standalone numerical kernels over caller-owned PCM.
#![forbid(unsafe_code)]
pub mod loudness;
pub mod spectrum;

/// Initially supported processing rates. No project or device is required.
pub const SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
