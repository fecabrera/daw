use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{CompressorDetection, CompressorSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[Rect; 3]; 3],
    pub sections: [Rect; 3],
}
fn db(value: f32) -> String {
    format!("{value:.1} dB")
}
fn time(value: f32) -> String {
    if value < 10.0 {
        format!("{value:.1} ms")
    } else {
        format!("{value:.0} ms")
    }
}
fn timing(name: &'static str, min: f32, max: f32, reset: f32) -> KnobSpec {
    let spec = KnobSpec::new(name, min..=max, reset, 0.1);
    let spec = if min < 1.0 {
        spec.scale(
            |v| (v / 0.1).ln() / 2000_f32.ln(),
            |p| 0.1 * 2000_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
    } else {
        spec.scale(
            |v| (v / 10.0).ln() / 200_f32.ln(),
            |p| 10.0 * 200_f32.powf(p),
            |v, d| v * 2_f32.powf(d / 24.0),
        )
    };
    spec.value_label(time).format(time).endpoint_labels(
        if min < 1.0 { "0.1" } else { "10" },
        if max == 200.0 { "200" } else { "2k" },
    )
}
fn row(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    column: usize,
    row: usize,
    reduction_db: f32,
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        match (column, row) {
            (0, 0) => ui.add(Knob::new(
                &mut s.threshold_db,
                KnobSpec::new("Threshold", -60.0..=0.0, -18.0, 0.1)
                    .value_label(db)
                    .format(|v| format!("{v:.1} dBFS"))
                    .endpoint_labels("−60", "0"),
            )),
            (0, 1) => ui.add(Knob::new(
                &mut s.ratio,
                KnobSpec::new("Ratio", 1.0..=20.0, 4.0, 0.1)
                    .value_label(|v| format!("{v:.1}:1"))
                    .format(|v| format!("{v:.1}:1"))
                    .endpoint_labels("1:1", "20:1"),
            )),
            (0, 2) => {
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
                response
            }
            (1, 0) => ui.add(Knob::new(
                &mut s.attack_ms,
                timing("Attack", 0.1, 200.0, 10.0),
            )),
            (1, 1) => ui.add(Knob::new(
                &mut s.release_ms,
                timing("Release", 10.0, 2000.0, 100.0),
            )),
            (1, 2) => ui.add(Knob::new(
                &mut s.knee_db,
                KnobSpec::new("Knee", 0.0..=24.0, 6.0, 0.1)
                    .value_label(db)
                    .format(db)
                    .endpoint_labels("0", "24"),
            )),
            (2, 0) => {
                knobs::gain_with_markers_named(ui, &mut s.level_db, "Compressor output level")
            }
            (2, 1) => ui.add(Knob::new(
                &mut s.dry_wet,
                KnobSpec::new("Dry/wet", 0.0..=1.0, 1.0, 0.01)
                    .value_label(|v| format!("{:.0}%", v * 100.0))
                    .format(|v| format!("{:.0}%", v * 100.0))
                    .endpoint_labels("Dry", "Wet"),
            )),
            (2, 2) => meters::gain_reduction(ui, reduction_db),
            _ => unreachable!(),
        }
    })
    .rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut CompressorSettings,
    compact: bool,
    height: f32,
    reduction_db: f32,
) -> (Bounds, egui::Response) {
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
            });
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
