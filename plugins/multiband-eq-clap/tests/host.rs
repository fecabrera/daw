#![allow(
    non_snake_case,
    dead_code,
    unused_imports,
    clippy::undocumented_unsafe_blocks
)]
struct Drive(daw_equalizer::MultibandEq);
impl Drive {
    fn new(
        context: daw_unit_core::ProcessingContext,
        settings: &daw_unit_core::MultibandEqSettings,
    ) -> Self {
        let mut dsp = daw_equalizer::MultibandEq::new(context, settings);
        dsp.prepare_fixed_band_slots();
        Self(dsp)
    }
    fn retarget(&mut self, s: &daw_unit_core::MultibandEqSettings) {
        self.0.retarget(s);
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        daw_unit_core::EffectUnit::process_sample(&mut self.0, input)
    }
    fn reset(&mut self) {
        daw_unit_core::EffectUnit::reset(&mut self.0);
    }
    fn latency_frames(&self) -> u32 {
        0
    }
}
use daw_multiband_eq_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;
