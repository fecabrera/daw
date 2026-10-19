//! Resource policies for the same exact transport cache. No profile changes DSP.
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheProfile {
    Light,
    Responsive,
    #[default]
    #[serde(other)]
    Balanced,
}

#[derive(Clone, Copy, Debug)]
pub struct CachePolicy {
    pub checkpoint_bytes: u64,
    pub max_checkpoints: usize,
    pub minimum_interval: Duration,
    pub edit_quiet: Duration,
    pub work_slice: Duration,
    pub idle_wait: Duration,
    pub warm_while_playing: bool,
}
impl CacheProfile {
    pub const ALL: [Self; 3] = [Self::Light, Self::Balanced, Self::Responsive];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Balanced => "Balanced",
            Self::Responsive => "Responsive",
        }
    }
    pub const fn policy(self) -> CachePolicy {
        let (mib, max_checkpoints, interval, quiet, work, wait, warm_while_playing) = match self {
            Self::Light => (32, 32, 1000, 750, 2, 40, false),
            Self::Balanced => (128, 64, 250, 250, 5, 20, true),
            Self::Responsive => (256, 128, 125, 100, 10, 10, true),
        };
        CachePolicy {
            checkpoint_bytes: mib * 1024 * 1024,
            max_checkpoints,
            minimum_interval: Duration::from_millis(interval),
            edit_quiet: Duration::from_millis(quiet),
            work_slice: Duration::from_millis(work),
            idle_wait: Duration::from_millis(wait),
            warm_while_playing,
        }
    }
}
