//! Headerless multiband compressor controls shared by native hosts.
use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout as panels, menus, meters, parameter_knobs, rows,
};
use daw_unit_core::{
    CompressorDetection, ModulationParameter as P, MultibandCompressorBand,
    MultibandCompressorSettings,
};
use egui::{Rect, Response, Ui, Vec2};
const SIZE: f32 = 42.0;
const GAP: f32 = 3.0;
pub struct Bounds {
    pub rect: Rect,
    pub ranges: Rect,
    pub crossovers: Vec<Rect>,
    pub bands: Vec<Rect>,
    pub band_responses: Vec<Response>,
    pub modes: Vec<Rect>,
    pub reductions: Vec<Rect>,
    pub knobs: Vec<[Rect; 6]>,
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
            band_responses: Vec::new(),
            modes: Vec::new(),
            reductions: Vec::new(),
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
    pub band_markers: &'a [[Option<ModulationMarker>; 6]],
    pub output_markers: &'a [Option<ModulationMarker>; 2],
    pub level: meters::OutputLevel<'a>,
    pub levels: &'a [meters::OutputLevel<'a>],
    pub reductions: &'a [meters::GainReduction<'a>],
    pub add_enabled: bool,
}
fn footer_space(ui: &mut Ui, start: f32, height: f32) {
    ui.add_space((height - 18.0 - (ui.cursor().top() - start)).max(0.0));
}

fn ranges(
    ui: &mut Ui,
    s: &mut MultibandCompressorSettings,
    dimensions: (bool, f32),
    bounds: &mut Bounds,
) {
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
                        for i in 0..s.bands.len().saturating_sub(1) {
                            let min = if i == 0 {
                                20.0
                            } else {
                                s.bands[i - 1].upper_hz + 1.0
                            };
                            let max = s.bands[i + 1].upper_hz - 1.0;
                            let reset = [200.0, 2000.0]
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
            rows::centered(ui, if compact { height } else { 18.0 }, |ui| {
                rows::unit_label(ui, "Split");
            });
        })
        .response
        .rect;
}
struct BandControls<'a> {
    markers: &'a [Option<ModulationMarker>; 6],
    open: &'a mut bool,
    solo: &'a mut bool,
    removable: bool,
    tooltip: String,
    reduction: meters::GainReduction<'a>,
    level: meters::OutputLevel<'a>,
    enabled: bool,
}
struct BandBounds {
    response: Option<Response>,
    section: Rect,
    mode: Rect,
    reduction: Rect,
    knobs: [Rect; 6],
    power: Rect,
    solo: Rect,
    remove: bool,
}
fn band(
    ui: &mut Ui,
    s: &mut MultibandCompressorBand,
    index: usize,
    dimensions: (bool, f32),
    c: BandControls<'_>,
) -> BandBounds {
    let (compact, height) = dimensions;
    let mut b = BandBounds {
        response: None,
        section: Rect::NOTHING,
        mode: Rect::NOTHING,
        reduction: Rect::NOTHING,
        knobs: [Rect::NOTHING; 6],
        power: Rect::NOTHING,
        solo: Rect::NOTHING,
        remove: false,
    };
    let strip = ui.scope_builder(
        egui::UiBuilder::new().sense(egui::Sense::click_and_drag()),
        |ui| {
            panels::unit_strip_with_width(
                ui,
                height,
                panels::unit_columns_width(2),
                egui::UiBuilder::new()
                    .id_salt(s.id)
                    .sense(egui::Sense::click_and_drag()),
                |ui| {
                    let start = ui.cursor().top();
                    ui.spacing_mut().item_spacing.y = GAP;
                    if !compact {
                        panels::unit_columns(ui, 2, |ui, column| {
                            for row in 0..4 {
                                let response = knobs::row(ui, s, SIZE, |ui, s| {
                                    if row == 3 && column == 1 {
                                        return meters::gain_reduction_sized(ui, c.reduction, SIZE);
                                    }
                                    if row == 3 {
                                        let mut detection =
                                            f32::from(s.detection == CompressorDetection::Rms);
                                        let r = ui.add(Knob::new(
                                            &mut detection,
                                            knobs::KnobSpec::new("Detection", 0.0..=1.0, 0.0, 1.0)
                                                .two_way_labels("Peak", "RMS")
                                                .display_scale(SIZE / knobs::CHOICE_SIZE)
                                                .format(|v| {
                                                    if v == 0.0 {
                                                        "Peak".into()
                                                    } else {
                                                        "RMS".into()
                                                    }
                                                }),
                                        ));
                                        if r.changed() {
                                            s.detection = if detection == 0.0 {
                                                CompressorDetection::Peak
                                            } else {
                                                CompressorDetection::Rms
                                            };
                                        }
                                        return r;
                                    }
                                    let target = match (row, column) {
                                        (0, 0) => 0,
                                        (0, 1) => 1,
                                        (1, 0) => 2,
                                        (1, 1) => 3,
                                        (2, 0) => 4,
                                        _ => 5,
                                    };
                                    let value = match target {
                                        0 => &mut s.threshold_db,
                                        1 => &mut s.ratio,
                                        2 => &mut s.attack_ms,
                                        3 => &mut s.release_ms,
                                        4 => &mut s.knee_db,
                                        _ => &mut s.level_db,
                                    };
                                    let mut spec =
                                        parameter_knobs::destination(P::COMPRESSOR[target])
                                            .display_scale(SIZE / knobs::CHOICE_SIZE);
                                    if target == 5 {
                                        spec = spec.named("Makeup");
                                    }
                                    ui.add(
                                        Knob::new(value, spec)
                                            .modulated(c.markers[target].as_ref(), c.open),
                                    )
                                });
                                if row < 3 {
                                    let target = match (row, column) {
                                        (0, 0) => 0,
                                        (0, 1) => 1,
                                        (1, 0) => 2,
                                        (1, 1) => 3,
                                        (2, 0) => 4,
                                        _ => 5,
                                    };
                                    b.knobs[target] = response.rect;
                                } else if column == 0 {
                                    b.mode = response.rect;
                                } else {
                                    b.reduction = response.rect;
                                }
                                menus::band_context_menu(
                                    &response,
                                    &mut s.enabled,
                                    c.solo,
                                    &mut b.remove,
                                    c.removable,
                                    c.enabled,
                                );
                            }
                        });
                        footer_space(ui, start, height);
                    }
                    rows::centered(ui, if compact { height } else { 18.0 }, |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let width = (ui.available_width() - 26.0).max(0.0);
                        b.power = ui
                            .allocate_ui_with_layout(
                                Vec2::new(width, 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let response = rows::power_label(
                                        ui,
                                        &format!("Band {}", index + 1),
                                        &format!("Toggle compressor band {}", index + 1),
                                        s.enabled,
                                    );
                                    if response.clicked() {
                                        s.enabled = !s.enabled;
                                    }
                                    menus::band_context_menu(
                                        &response,
                                        &mut s.enabled,
                                        c.solo,
                                        &mut b.remove,
                                        c.removable,
                                        c.enabled,
                                    );
                                    response.on_hover_text(&c.tooltip).rect
                                },
                            )
                            .inner;
                        let response = rows::band_solo(
                            ui,
                            &format!("Solo compressor band {}", index + 1),
                            c.solo,
                        );
                        menus::band_context_menu(
                            &response,
                            &mut s.enabled,
                            c.solo,
                            &mut b.remove,
                            c.removable,
                            c.enabled,
                        );
                        b.solo = response.rect;
                    });
                },
            )
        },
    );
    b.section = strip.response.rect;
    if !compact {
        let response = meters::band_output_level(
            ui,
            meters::unit_monitor_rail(b.section, panels::PANEL_MARGIN),
            c.level,
            c.enabled,
            s.enabled,
            &format!("Band {} output", index + 1),
        );
        menus::band_context_menu(
            &response,
            &mut s.enabled,
            c.solo,
            &mut b.remove,
            c.removable,
            c.enabled,
        );
    }
    menus::band_context_menu(
        &strip.response,
        &mut s.enabled,
        c.solo,
        &mut b.remove,
        c.removable,
        c.enabled,
    );
    b.response = Some(strip.response);
    b
}
fn output(
    ui: &mut Ui,
    s: &mut MultibandCompressorSettings,
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
        rows::centered(ui, if compact { height } else { 18.0 }, |ui| {
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
    s: &mut MultibandCompressorSettings,
    dimensions: (bool, f32),
    c: Controls<'_>,
) -> Bounds {
    let mut b = Bounds::default();
    let mut remove = None;
    b.rect = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ranges(ui, s, dimensions, &mut b);
        let order:Vec<_>=if s.presentation.is_empty() {(0..s.bands.len()).collect()}else{s.presentation.iter().filter_map(|id|s.bands.iter().position(|b|b.id==*id)).collect()};
        for i in order {
            let lower = if i == 0 { 0.0 } else { s.bands[i - 1].upper_hz };
            let upper = if i + 1 == s.bands.len() { "Nyquist".into() } else { knobs::format_frequency(s.bands[i].upper_hz) };
            let mut unused = false;
            let empty = std::array::from_fn(|_| None);
            let controls = BandControls {
                markers: c.band_markers.get(i).unwrap_or(&empty), open: &mut s.modulation_open,
                solo: c.solos.get_mut(i).unwrap_or(&mut unused), removable: s.bands.len() > 1,
                reduction:c.reductions.get(i).copied().unwrap_or(0.0.into()),
                level:c.levels.get(i).copied().unwrap_or(0.0.into()),enabled:s.enabled,
                tooltip: format!("Band {}: {}–{}\nOff bypasses compression and Makeup. Right-click for band power, Solo and removal.", i + 1, knobs::format_frequency(lower), upper),
            };
            let band = band(ui, &mut s.bands[i], i, dimensions, controls);
            if band.remove { remove = Some(s.bands[i].id); }
            panels::unit_divider(ui, band.section.left(), band.section.y_range().into());
            b.band_responses.push(band.response.unwrap());
            b.bands.push(band.section); b.modes.push(band.mode); b.reductions.push(band.reduction); b.knobs.push(band.knobs);
            b.power.push(band.power); b.solo.push(band.solo);
        }
        let add = ui.add_enabled_ui(c.add_enabled,|ui|panels::unit_add_strip(ui, dimensions.1, "Add compressor band")).inner;
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
