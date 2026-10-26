//! Format-neutral Bitcrusher plugin contract over the portable unit.
#[path = "../../drive-v2-plugin/src/exchange.rs"]
mod exchange;
mod mapping;
mod parameters;
#[path = "../../saturator-plugin/src/processor.rs"]
mod processor;
#[path = "../../saturator-plugin/src/state.rs"]
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

use daw_bitcrusher::Bitcrusher as Dsp;
const MAGIC: &[u8; 8] = b"DAWBCR1!";
/// Fixed portable DSP latency, including bypass.
pub const LATENCY_FRAMES: u32 = 0;
