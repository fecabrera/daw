//! Loudness and peak measurement over caller-owned stereo PCM.
//!
//! ```
//! use audio_analysis::loudness::{MeasureMode, OfflineAnalyzer};
//! let mut analyzer = OfflineAnalyzer::new(48_000, MeasureMode::SamplePeak)?;
//! analyzer.push([0.5, -0.25])?;
//! assert!((analyzer.finish()?.unwrap() + 6.0206).abs() < 0.001);
//! # Ok::<(), &'static str>(())
//! ```
mod offline;
pub use daw_analysis_kernels::loudness::*;
pub use offline::{MeasureMode, OfflineAnalyzer};
