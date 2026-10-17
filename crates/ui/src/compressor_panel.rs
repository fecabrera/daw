use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{CompressorDetection, CompressorSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 3],
    pub sections: [Rect; 3],
}
fn row(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    column: usize,
    row: usize,
    reduction_db: f32,
    markers: &[Option<knobs::ModulationMarker>; 7],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        if (column, row) == (2, 2) {
            return meters::gain_reduction(ui, reduction_db);
        }
        if (column, row) == (0, 2) {
            let mut position = f32::from(s.detection == CompressorDetection::Rms);
            let response = ui.add(Knob::new(
                &mut position,
                KnobSpec::new("Detection mode", 0.0..=1.0, 0.0, 1.0)
                    .two_way_labels("Peak", "RMS")
                    .format(|v| {
                        if v == 0.0 {
                            "Peak".into()
                        } else {
                            "RMS".into()
                        }
                    }),
            ));
            if response.changed() {
                s.detection = if position == 0.0 {
                    CompressorDetection::Peak
                } else {
                    CompressorDetection::Rms
                };
            }
            return response;
        }
        let (index, value) = match (column, row) {
            (0, 0) => (0, &mut s.threshold_db),
            (0, 1) => (1, &mut s.ratio),
            (1, 0) => (2, &mut s.attack_ms),
            (1, 1) => (3, &mut s.release_ms),
            (1, 2) => (4, &mut s.knee_db),
            (2, 0) => (5, &mut s.level_db),
            (2, 1) => (6, &mut s.dry_wet),
            _ => unreachable!(),
        };
        let mut spec = parameter_knobs::destination(Parameter::COMPRESSOR[index]);
        if index == 5 {
            spec = spec.named("Compressor output level");
        }
        ui.add(Knob::new(value, spec).modulated(markers[index].as_ref(), &mut s.modulation_open))
    })
    .rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    compact: bool,
    height: f32,
    reduction_db: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, compact, height, reduction_db, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    compact: bool,
    height: f32,
    reduction_db: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [
        settings.threshold_db,
        settings.ratio,
        settings.attack_ms,
        settings.release_ms,
        settings.knee_db,
        settings.level_db,
        settings.dry_wet,
    ];
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::COMPRESSOR[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = settings.modulation_open;
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[Rect::NOTHING; 3]; 3],
        sections: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Compressor", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Comp", "Response", "Out"].into_iter().enumerate() {
                    let rect = ui
                        .push_id(column, |ui| {
                            egui::Frame::new()
                                .inner_margin(panel.margin)
                                .show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_width(
                                                UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin),
                                            );
                                            ui.set_min_height(crate::envelope_panel::body_height(
                                                compact,
                                            ));
                                            if !compact {
                                                for index in 0..3 {
                                                    bounds.controls[column][index] = row(
                                                        ui,
                                                        settings,
                                                        column,
                                                        index,
                                                        reduction_db,
                                                        &markers,
                                                    );
                                                }
                                            }
                                            rows::centered(
                                                ui,
                                                if compact {
                                                    crate::envelope_panel::body_height(true)
                                                } else {
                                                    icons::COMPACT_BUTTON_SIZE
                                                },
                                                |ui| {
                                                    rows::unit_label(ui, name);
                                                },
                                            );
                                        },
                                    );
                                })
                        })
                        .inner
                        .response
                        .rect;
                    bounds.sections[column] = rect;
                    if column > 0 {
                        panels::unit_divider(ui, rect.left(), rect.y_range().into());
                    }
                }
                if open {
                    let last = bounds.sections[2];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources, <CompressorSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS, <CompressorSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
                }
            });
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
