//! Native editor bindings for PlateReverb.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::PlateReverbSettings,
    model: &Model,
) -> body::Bounds {
    body::render(
        ui,
        settings,
        daw_unit_ui::meters::OutputLevel::new(
            model.output_meter.readings()[0],
            Some(&model.output_meter.boost_peaks()[0]),
        )
        .with_reset_request(&model.output_reset),
        daw_unit_ui::meters::GainReduction::new(
            model.output_meter.reduction.reading(),
            Some(&model.output_meter.reduction),
        )
        .for_unit(settings.enabled),
        daw_unit_ui::meters::BandGainChange::new(
            model.output_meter.bands.readings(),
            Some(&model.output_meter.bands),
        )
        .for_unit(settings.enabled),
    )
}
fn paint_extra(
    _: &mut egui::Ui,
    bounds: &body::Bounds,
    model: &Model,
    _: &daw_unit_core::PlateReverbSettings,
) {
    for (mask, response) in &bounds.readouts {
        if response.clicked() {
            model
                .meter_reset
                .fetch_or(*mask, std::sync::atomic::Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "../../filter-editor/src/editor_tests.rs"]
mod tests;
#[cfg(test)]
fn paint_reference(ui: &mut egui::Ui, s: &mut daw_unit_core::PlateReverbSettings, model: &Model) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    daw_unit_ui::plate_reverb::show(
        ui,
        s,
        false,
        daw_unit_ui::layout::unit_control_height(),
        &std::array::from_fn::<_, 8, _>(|_| None),
        level,
    );
}
#[cfg(test)]
#[path = "parity_tests.rs"]
mod builtin_parity;
#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    s: &mut daw_unit_core::PlateReverbSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = body::render(ui, s, 0.0.into(), 0.0.into(), [0.0; 3].into());
    b.controls
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .filter(|r| r.enabled())
                .map(|r| (daw_drive_plugin::Parameter::ALL[i + 1], r.rect))
        })
        .collect()
}
#[cfg(test)]
fn assert_builtin_defaults() {
    assert_eq!(
        daw_drive_plugin::PluginState::default().to_settings(),
        daw_unit_core::PlateReverbSettings::default()
    );
}
