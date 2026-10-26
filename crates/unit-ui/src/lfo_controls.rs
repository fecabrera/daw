use crate::knobs::KnobSpec;
pub fn phase_spec(name: &'static str) -> KnobSpec {
    KnobSpec::new(name, -180.0..=180.0, 0.0, 1.0)
        .centered(0.0)
        .bipolar()
        .value_label(|v| format!("{v:.0}°"))
        .format(|v| format!("{v:.0}°"))
        .endpoint_labels("−180", "+180")
}
