use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{LimiterMode, LimiterRouting, LimiterSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 2],
    pub sections: [Rect; 2],
    pub meter: Rect,
}
fn row(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
    column: usize,
    row: usize,
    markers: &[Option<knobs::ModulationMarker>; 4],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        let target = match (column, row) {
            (0, 0) => Some((0, &mut s.input_db)),
            (0, 1) => Some((1, &mut s.release_ms)),
            (0, 2) => Some((2, &mut s.lookahead_ms)),
            (1, 0) => Some((3, &mut s.ceiling_db)),
            _ => None,
        };
        if let Some((index, value)) = target {
            return ui.add(Knob::new(value, parameter_knobs::destination(Parameter::LIMITER[index]))
                .modulated(markers[index].as_ref(), &mut s.modulation_open));
        }
        match (column, row) {
            (1, 1) => {
                let mut value = f32::from(s.mode == LimiterMode::TruePeak);
                let response = ui.add(Knob::new(&mut value,
                    KnobSpec::new("Limiter mode", 0.0..=1.0, 0.0, 1.0)
                        .two_way_labels("Peak", "True")
                        .format(|v| if v == 0.0 { "Peak".into() } else { "True Peak".into() })));
                if response.changed() { s.mode = if value == 0.0 { LimiterMode::Peak } else { LimiterMode::TruePeak }; }
                response.on_hover_text("Peak: sample ceiling. True Peak: eight-times oversampled reconstruction guard.")
            }
            (1, 2) => {
                let mut value = f32::from(s.routing == LimiterRouting::MidSide);
                let response = ui.add(Knob::new(&mut value,
                    KnobSpec::new("Routing", 0.0..=1.0, 0.0, 1.0).two_way_labels("L/R", "M/S")
                        .format(|v| if v == 0.0 { "Linked left/right".into() } else { "Independent mid/side".into() })));
                if response.changed() { s.routing = if value == 0.0 { LimiterRouting::LeftRight } else { LimiterRouting::MidSide }; }
                response.on_hover_text("L/R: linked stereo limiting. M/S: independent Mid/Side limiting, then a linked output ceiling.")
            }
            _ => unreachable!(),
        }
    }).rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut LimiterSettings,
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
    settings: &mut LimiterSettings,
    compact: bool,
    height: f32,
    reduction_db: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let reduction_db = meters::unit_reading(settings.enabled, reduction_db);
    let bases = [
        settings.input_db,
        settings.release_ms,
        settings.lookahead_ms,
        settings.ceiling_db,
    ];
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::LIMITER[i],
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
        controls: [[Rect::NOTHING; 3]; 2],
        sections: [Rect::NOTHING; 2],
        meter: Rect::NOTHING,
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Limiter", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (column, name) in ["Limit", "Out"].into_iter().enumerate() {
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
                                                    bounds.controls[column][index] =
                                                        row(ui, settings, column, index, &markers);
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
                    if !compact && column == 0 {
                        let rail = meters::unit_monitor_rail(
                            rect.with_max_x(rect.left() + 2.0 * UNIT_STRIP_WIDTH),
                            panel.margin,
                        );
                        bounds.meter = rail;
                        meters::gain_reduction_rail(ui, rail, reduction_db);
                    }
                    if column > 0 {
                        panels::unit_divider(ui, rect.left(), rect.y_range().into());
                    }
                }
                if open {
                    let last = bounds.sections[1];
                    panels::unit_divider(ui, last.right(), last.y_range().into());
                    crate::modulation_panel::body(ui, &mut settings.modulations, sources,
                        <LimiterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS,
                        <LimiterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
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
