//! Headerless multiband exciter controls shared by native hosts.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout as panels, menus, meters, parameter_knobs, rows,
};
use daw_unit_core::{ExciterBand, ExciterMode, ExciterSettings, ModulationParameter as P};
use egui::{Rect, Response, Ui, Vec2};
const SIZE: f32 = 42.0;
const GAP: f32 = 3.0;
pub struct Bounds {
    pub rect: Rect,
    pub ranges: Rect,
    pub crossovers: Vec<Rect>,
    pub bands: Vec<Rect>,
    pub modes: Vec<Rect>,
    pub knobs: Vec<[Rect; 3]>,
    pub power: Vec<Rect>,
    pub solo: Vec<Rect>,
    pub add: Rect,
    pub output: Rect,
    pub output_knobs: [Rect; 2],
    pub meter: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            rect: Rect::NOTHING,
            ranges: Rect::NOTHING,
            crossovers: Vec::new(),
            bands: Vec::new(),
            modes: Vec::new(),
            knobs: Vec::new(),
            power: Vec::new(),
            solo: Vec::new(),
            add: Rect::NOTHING,
            output: Rect::NOTHING,
            output_knobs: [Rect::NOTHING; 2],
            meter: Rect::NOTHING,
        }
    }
}
pub struct Controls<'a> {
    pub solos: &'a mut [bool],
    pub band_markers: &'a [[Option<ModulationMarker>; 3]],
    pub output_markers: &'a [Option<ModulationMarker>; 2],
    pub level: meters::OutputLevel<'a>,
}
fn footer_space(ui: &mut Ui, start: f32, height: f32) {
    ui.add_space((height - 22.0 - (ui.cursor().top() - start)).max(0.0));
}
fn remove_menu(response: &Response, remove: &mut bool, enabled: bool) {
    menus::ContextMenu::new(response).show(|ui| {
        if ui
            .add_enabled(enabled, egui::Button::new("Remove band"))
            .clicked()
        {
            *remove = true;
            ui.close();
        }
    });
}
fn ranges(ui: &mut Ui, s: &mut ExciterSettings, dimensions: (bool, f32), bounds: &mut Bounds) {
    let (compact, height) = dimensions;
    bounds.ranges =
        panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt("ranges"), |ui| {
            let start = ui.cursor().top();
            if !compact {
                egui::ScrollArea::vertical()
                    .id_salt("crossovers")
                    .max_height(height - 26.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = GAP;
                        for i in 0..s.bands.len() - 1 {
                            let min = if i == 0 {
                                20.0
                            } else {
                                s.bands[i - 1].upper_hz + 1.0
                            };
                            let max = s.bands[i + 1].upper_hz - 1.0;
                            let reset = [150.0, 2000.0, 6000.0]
                                .get(i)
                                .copied()
                                .unwrap_or((min * max).sqrt())
                                .clamp(min, max);
                            let spec = parameter_knobs::destination(P::EqBandFrequency)
                                .named("Crossover")
                                .reset_to(reset)
                                .display_scale(SIZE / knobs::CHOICE_SIZE);
                            let response = ui
                                .push_id(s.bands[i].id, |ui| {
                                    knobs::row(ui, &mut s.bands[i].upper_hz, SIZE, |ui, v| {
                                        ui.add(Knob::new(v, spec))
                                    })
                                })
                                .inner;
                            s.bands[i].upper_hz = s.bands[i].upper_hz.clamp(min, max);
                            bounds.crossovers.push(response.rect);
                            response.on_hover_text(format!(
                                "Boundary between bands {} and {}",
                                i + 1,
                                i + 2
                            ));
                        }
                    });
                footer_space(ui, start, height);
            }
            rows::centered(ui, if compact { height } else { 22.0 }, |ui| {
                rows::unit_label(ui, "Ranges");
            });
        })
        .response
        .rect;
}
struct BandControls<'a> {
    markers: &'a [Option<ModulationMarker>; 3],
    open: &'a mut bool,
    solo: &'a mut bool,
    removable: bool,
    tooltip: String,
}
struct BandBounds {
    section: Rect,
    mode: Rect,
    knobs: [Rect; 3],
    power: Rect,
    solo: Rect,
    remove: bool,
}
fn band(
    ui: &mut Ui,
    s: &mut ExciterBand,
    index: usize,
    dimensions: (bool, f32),
    c: BandControls<'_>,
) -> BandBounds {
    let (compact, height) = dimensions;
    let mut b = BandBounds {
        section: Rect::NOTHING,
        mode: Rect::NOTHING,
        knobs: [Rect::NOTHING; 3],
        power: Rect::NOTHING,
        solo: Rect::NOTHING,
        remove: false,
    };
    b.section = panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt(s.id), |ui| {
        let start = ui.cursor().top();
        ui.spacing_mut().item_spacing.y = GAP;
        if !compact {
            let mode = menus::Dropdown::from_id_salt("mode")
                .small_items()
                .selected_text(egui::RichText::new(s.mode.label()).size(panels::HEADER_FONT_SIZE))
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for value in ExciterMode::ALL {
                        ui.selectable_value(&mut s.mode, value, value.label());
                    }
                })
                .response
                .on_hover_text("Saturation mode");
            b.mode = mode.rect;
            for (row, target) in [0, 2, 1].into_iter().enumerate() {
                let mut spec = parameter_knobs::destination(P::SATURATOR[target])
                    .display_scale(SIZE / knobs::CHOICE_SIZE);
                if target == 2 {
                    spec = spec.named("Mix").reset_to(0.0);
                }
                let response = knobs::row(ui, s, SIZE, |ui, s| {
                    let value = match target {
                        0 => &mut s.drive_db,
                        1 => &mut s.level_db,
                        _ => &mut s.mix,
                    };
                    ui.add(Knob::new(value, spec).modulated(c.markers[target].as_ref(), c.open))
                });
                remove_menu(&response, &mut b.remove, c.removable);
                b.knobs[row] = response.rect;
            }
            footer_space(ui, start, height);
        }
        rows::centered(ui, if compact { height } else { 22.0 }, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let width = (ui.available_width() - 26.0).max(0.0);
            b.power = ui
                .allocate_ui_with_layout(
                    Vec2::new(width, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let response = rows::power_label(
                            ui,
                            &format!("B{}", index + 1),
                            &format!("Toggle exciter band {}", index + 1),
                            s.enabled,
                        );
                        if response.clicked() {
                            s.enabled = !s.enabled;
                        }
                        remove_menu(&response, &mut b.remove, c.removable);
                        response.on_hover_text(&c.tooltip).rect
                    },
                )
                .inner;
            let response = rows::square_toggle(ui, "S", *c.solo)
                .on_hover_text("Solo this band for listening. Export is unchanged.");
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    *c.solo,
                    format!("Solo exciter band {}", index + 1),
                )
            });
            if response.clicked() {
                *c.solo = !*c.solo;
            }
            b.solo = response.rect;
        });
    })
    .response
    .rect;
    b
}
fn output(
    ui: &mut Ui,
    s: &mut ExciterSettings,
    dimensions: (bool, f32),
    markers: &[Option<ModulationMarker>; 2],
    level: meters::OutputLevel<'_>,
    b: &mut Bounds,
) {
    let (compact, height) = dimensions;
    b.output = panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt("out"), |ui| {
        let start = ui.cursor().top();
        if !compact {
            for (i, p) in [P::Level, P::DryWet].into_iter().enumerate() {
                b.output_knobs[i] = knobs::row(ui, s, knobs::CHOICE_SIZE, |ui, s| {
                    let v = if i == 0 {
                        &mut s.level_db
                    } else {
                        &mut s.dry_wet
                    };
                    ui.add(
                        Knob::new(v, parameter_knobs::destination(p))
                            .modulated(markers[i].as_ref(), &mut s.modulation_open),
                    )
                })
                .rect;
            }
            footer_space(ui, start, height);
        }
        rows::centered(ui, if compact { height } else { 22.0 }, |ui| {
            rows::unit_label(ui, "Out");
        });
    })
    .response
    .rect;
    panels::unit_divider(ui, b.output.left(), b.output.y_range().into());
    if !compact {
        b.meter = meters::effect_output_level(ui, b.output, panels::PANEL_MARGIN, level, s.enabled);
    }
}
pub fn show(
    ui: &mut Ui,
    s: &mut ExciterSettings,
    dimensions: (bool, f32),
    c: Controls<'_>,
) -> Bounds {
    let mut b = Bounds::default();
    let mut remove = None;
    b.rect = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ranges(ui, s, dimensions, &mut b);
        for i in 0..s.bands.len() {
            let lower = if i == 0 { 0.0 } else { s.bands[i - 1].upper_hz };
            let upper = if i + 1 == s.bands.len() { "Nyquist".into() } else { knobs::format_frequency(s.bands[i].upper_hz) };
            let mut unused = false;
            let empty = std::array::from_fn(|_| None);
            let controls = BandControls {
                markers: c.band_markers.get(i).unwrap_or(&empty), open: &mut s.modulation_open,
                solo: c.solos.get_mut(i).unwrap_or(&mut unused), removable: s.bands.len() > 1,
                tooltip: format!("Band {}: {}–{}\nOff bypasses excitation and Level. Right-click a band control to remove it.", i + 1, knobs::format_frequency(lower), upper),
            };
            let band = band(ui, &mut s.bands[i], i, dimensions, controls);
            if band.remove { remove = Some(s.bands[i].id); }
            panels::unit_divider(ui, band.section.left(), band.section.y_range().into());
            b.bands.push(band.section); b.modes.push(band.mode); b.knobs.push(band.knobs);
            b.power.push(band.power); b.solo.push(band.solo);
        }
        let add = panels::unit_add_strip(ui, dimensions.1, "Add exciter band");
        b.add = add.inner.rect;
        panels::unit_divider(ui, add.response.rect.left(), add.response.rect.y_range().into());
        if add.inner.clicked() { s.add_band(); }
        output(ui, s, dimensions, c.output_markers, c.level, &mut b);
    }).response.rect;
    if let Some(id) = remove {
        s.remove_band(id);
    }
    b
}
