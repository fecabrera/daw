use crate::{
    icons,
    knobs::{self, Knob},
    layout as panels,
    layout::UNIT_STRIP_WIDTH,
    meters, parameter_knobs, rows, theme,
};
use daw_unit_core::{EqSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub struct Bounds {
    pub sections: [Rect; 3],
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 3],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}
fn controls(
    ui: &mut Ui,
    band: &mut daw_unit_core::EqBandSettings,
    index: usize,
    markers: &[Option<knobs::ModulationMarker>; 7],
    open: &mut bool,
) -> [Rect; 3] {
    let first = [0, 3, 4][index];
    let mut bounds = [Rect::NOTHING; 3];
    for (row, rect) in bounds.iter_mut().enumerate() {
        if index == 1 && row > 0 {
            // Mid retains the shared row/footer geometry without legacy frequency/Q controls.
            knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| {
                ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover())
            });
            continue;
        }
        let value = match row {
            0 => &mut band.gain_db,
            1 => &mut band.frequency_hz,
            _ => &mut band.q,
        };
        let target = first + row;
        *rect = knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, v| {
            ui.add(
                Knob::new(
                    v,
                    parameter_knobs::destination(Parameter::EQUALIZER[target]),
                )
                .modulated(markers[target].as_ref(), open),
            )
        })
        .rect;
    }
    bounds
}
pub fn show(
    ui: &mut Ui,
    settings: &mut EqSettings,
    compact: bool,
    body_height: f32,
    levels: [f32; 3],
    markers: &[Option<knobs::ModulationMarker>; 7],
) -> Bounds {
    let mut bounds = Bounds {
        sections: [Rect::NOTHING; 3],
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 3],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    let margin = crate::layout::PANEL_MARGIN;

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));

        for (index, name) in ["Low", "Mid", "High"].into_iter().enumerate() {
            let strip = ui.push_id(index, |ui| {
                egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(margin));
                        ui.set_min_height(body_height);
                        let band = &mut settings.bands[index];
                        if !compact { bounds.knobs[index] = controls(ui, band, index, markers, &mut settings.modulation_open); }
                        rows::centered(ui, if compact { body_height } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                            let response = rows::power_label(ui, name, &format!("Toggle {name} band cut"), band.enabled);
                            let response = response.on_hover_text(if index == 1 {
                                "Mid shelf uses Low/High frequency boundaries and Q slopes. Off cuts this middle range; saved gain is retained.".into()
                            } else {
                                format!("{name}: off cuts this range using frequency and Q; saved gain is retained.")
                            });
                            bounds.bands[index] = response.rect;
                            if response.clicked() { band.enabled = !band.enabled; }
                        });
                    });
                })
            }).inner.response.rect;
            bounds.sections[index] = strip;
            if !compact {
                let rail = meters::unit_monitor_rail(strip, margin);
                bounds.meters[index] = rail;
                meters::band_level_rail(ui, rail, name, levels[index], settings.bands[index].enabled);
            }
            if index > 0 { panels::unit_divider(ui, strip.left(), strip.y_range().into()); }
        }

    });

    bounds
}
