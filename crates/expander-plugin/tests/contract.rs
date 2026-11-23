#[path = "common/direct.rs"]
mod direct;
use daw_expander_plugin as plugin;
use direct::Direct;

const IDENTITY_FIXTURE: &str = include_str!("fixtures/identity.json");
const DEFAULT_FIXTURE: &[u8] = include_bytes!("fixtures/default-state.bin");

include!("../../filter-plugin/tests/common/contract.rs");
