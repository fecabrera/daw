//! Format-neutral Multiband EQ plugin contract over the portable unit.
#[path = "../../drive-v2-plugin/src/exchange.rs"]
mod exchange;
mod mapping;
mod parameters;
mod processor;
mod state;
pub use exchange::StateExchange;
pub use parameters::{Parameter, ParameterError, ParameterInfo, Scale};
pub use processor::{ConfigurationError, Processor, SUPPORTED_RATES};
pub use state::{MAX_STATE_BYTES, PluginState, STATE_VERSION, StateError};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub vendor: &'static str,
    pub product: &'static str,
    pub version: &'static str,
    pub clap_id: &'static str,
    /// Canonical RFC 4122 byte order. VST3 platform TUID encoding belongs in its adapter.
    pub processor_id: [u8; 16],
    pub controller_id: [u8; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Gain,
    Percent,
    Shape,
    Frequency,
    Mode,
    Db,
    Integer,
    Selector,
    Scalar,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelControl {
    pub parameter: Parameter,
    pub control: Control,
    pub enabled_by: Option<Parameter>,
    pub diagram: Option<&'static str>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strip {
    pub footer: &'static str,
    pub power: Option<Parameter>,
    pub controls: &'static [PanelControl],
}
include!(concat!(env!("OUT_DIR"), "/manifest.rs"));

/// Standard current/held output telemetry; allocate and attach outside processing.
mod meter;
pub use meter::{METER_VALUES, OutputMeter, meter_values, receive_meter, reset_meter};

use daw_equalizer::MultibandEq as Dsp;
const MAGIC: &[u8; 8] = b"DAWMBEQ!";
/// Fixed portable DSP latency, including bypass.
pub const LATENCY_FRAMES: u32 = 0;

/// Host delay compensation at the selected processing rate.
pub fn latency_frames(_rate: u32) -> u32 {
    LATENCY_FRAMES
}

/// Conservative tail bound in samples; histories continue on quiet blocks.
pub fn tail_frames(rate: u32) -> u32 {
    rate.saturating_mul(4)
}

/// Format classification does not alter product identity.
pub const CLAP_FEATURE: &std::ffi::CStr = c"equalizer";
pub const VST3_CATEGORIES: &str = "Fx|EQ|Stereo";
pub const USES_TRANSPORT: bool = false;
