//! Format-neutral Drive v2 parameters, state and prepared DSP.
mod exchange;
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
pub type OutputMeter = daw_unit_core::BandMeter<3>;

/// Portable DSP latency, including bypass.
pub const LATENCY_FRAMES: u32 = 0;

/// Host delay compensation at the selected processing rate.
pub fn latency_frames(_rate: u32) -> u32 {
    LATENCY_FRAMES
}

pub const METER_VALUES: usize = 2;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    [
        f64::from(m.readings()[0]),
        f64::from(m.boost_peaks()[0].peak()),
    ]
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    use daw_unit_core::MeterStorage;
    m.clear();
    m.store([v[1], 0.0, 0.0]);
    m.initialize([v[0], 0.0, 0.0]);
}
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    if mask & 1 != 0 {
        m.boost_peaks()[0].reset_peak();
    }
}

/// Conservative tail bound in samples; histories continue on quiet blocks.
pub fn tail_frames(rate: u32) -> u32 {
    rate.saturating_mul(1)
}

/// Format classification does not alter product identity.
pub const CLAP_FEATURE: &std::ffi::CStr = c"distortion";
pub const VST3_CATEGORIES: &str = "Fx|Distortion|Stereo";
pub const USES_TRANSPORT: bool = false;
