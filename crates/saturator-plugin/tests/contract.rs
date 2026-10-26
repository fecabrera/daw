use daw_saturator::Saturator as Dsp;
use daw_saturator_plugin::{
    IDENTITY, LATENCY_FRAMES, MAX_STATE_BYTES, OutputMeter, PANEL, Parameter as P, PluginState,
    Processor, SUPPORTED_RATES, StateExchange,
};
use daw_unit_core::SaturatorSettings as Settings;
const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &str = include_str!("fixtures/default.hex");
include!("common/contract.rs");
