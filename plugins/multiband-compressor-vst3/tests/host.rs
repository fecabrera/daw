#![allow(
    non_snake_case,
    dead_code,
    unused_imports,
    clippy::undocumented_unsafe_blocks
)]
struct Drive(daw_multiband_compressor::MultibandCompressor);
impl Drive {
    fn new(c: daw_unit_core::ProcessingContext, s: &daw_drive_plugin::Settings) -> Self {
        let present: [_; 64] =
            std::array::from_fn(|i| s.bands.iter().any(|b| b.id == s.slots[i].id));
        Self(
            daw_multiband_compressor::MultibandCompressor::prepare_slots(
                c, &s.slots, &present, s.enabled, s.level_db, s.dry_wet,
            ),
        )
    }
    fn retarget(&mut self, s: &daw_drive_plugin::Settings) {
        for i in 0..64 {
            self.0.retarget_slot(
                i,
                &s.slots[i],
                s.bands.iter().any(|b| b.id == s.slots[i].id),
            );
        }
        self.0.retarget_output(s.enabled, s.level_db, s.dry_wet);
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
use daw_multiband_compressor_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;
