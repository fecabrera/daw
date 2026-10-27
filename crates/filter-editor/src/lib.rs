//! Native editor bindings for Filter.
mod body;
#[path = "../../drive-v2-editor/src/editor.rs"]
mod editor;
pub use editor::*;

fn show_body(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::FilterSettings,
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
    _: &daw_unit_core::FilterSettings,
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
fn paint_reference(ui: &mut egui::Ui, settings: &mut daw_unit_core::FilterSettings, model: &Model) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let reduction = daw_unit_ui::meters::GainReduction::new(
        model.output_meter.reduction.reading(),
        Some(&model.output_meter.reduction),
    )
    .for_unit(settings.enabled);
    let gains = daw_unit_ui::meters::BandGainChange::new(
        model.output_meter.bands.readings(),
        Some(&model.output_meter.bands),
    )
    .for_unit(settings.enabled);
    let levels = model.output_meter.bands.readings();
    let _ = (reduction, gains, levels);
    use daw_unit_ui::layout::unit_control_height;
    let _ = {
        let mut view = daw_unit_core::AutoFilterSettings {
            enabled: settings.enabled,
            shape: settings.shape,
            frequency_hz: settings.frequency_hz,
            q: settings.q,
            level_db: settings.level_db,
            dry_wet: settings.dry_wet,
            lfo_enabled: false,
            ..Default::default()
        };
        let b = daw_unit_ui::auto_filter::show(
            ui,
            &mut view,
            (false, unit_control_height()),
            &std::array::from_fn(|_| None),
            false,
            level,
        );
        settings.shape = view.shape;
        settings.frequency_hz = view.frequency_hz;
        settings.q = view.q;
        settings.level_db = view.level_db;
        settings.dry_wet = view.dry_wet;
        b
    };
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::FilterSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = body::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn(|_| None),
        0.0.into(),
    );
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
    let native = daw_drive_plugin::PluginState::default().to_settings();
    let builtin: daw_unit_core::FilterSettings = Default::default();
    assert_eq!(native, builtin);
}
