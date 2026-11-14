//! Portable unit controls and presentation. No project, device, or host dependencies.
pub mod ambient_reverb;
mod compressor_graph;
pub mod drive;
pub mod drive_v2;
pub mod eq_graph;
mod eq_spectrum;
pub mod fonts;
pub mod gain;
mod icons;
pub mod knobs;
pub mod layout;
pub mod lfo_controls;
pub mod menus;
pub mod meters;
pub mod parameter_knobs;
pub mod plate_reverb;
mod reverb_controls;
pub mod rows;
pub mod shimmer_reverb;
pub mod spectrum_graph;
pub mod theme;

pub mod bitcrusher;
pub mod exciter;
pub mod saturator;

pub mod compressor;
pub mod compressor_curve;
pub mod control_capture;
pub mod harmonic_lift;

pub mod chorus;
pub mod direction_mixer;
pub mod limiter;
pub mod ott;
pub mod stereo_spread;

pub mod auto_filter;

pub mod equalizer;

pub mod multiband_compressor;
pub mod multiband_eq;

pub mod drag;
pub mod strip_reorder;

pub mod loudness;

pub mod tooltips;

mod compressor_history;
