//! Loudness and peak measurement over caller-owned stereo PCM.
//!
//! ```
//! use audio_analysis::loudness::{MeasureMode, OfflineAnalyzer};
//! let mut analyzer = OfflineAnalyzer::new(48_000, MeasureMode::SamplePeak)?;
//! analyzer.push([0.5, -0.25])?;
//! assert!((analyzer.finish()?.unwrap() + 6.0206).abs() < 0.001);
//! # Ok::<(), &'static str>(())
//! ```
mod distribution;
mod filter;
mod live;
mod offline;

pub use distribution::{Distribution, db, lufs};
pub use filter::{KWeighting, TruePeak};
pub use live::{IntegratedStatistics, Measurements, Observation, RollingLoudness};
pub use offline::{MeasureMode, OfflineAnalyzer};
