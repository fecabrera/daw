use crate::{
    knobs::{self, Knob},
    layout as panels, menus, meters, parameter_knobs, rows,
};
use daw_unit_core::{EqBandMode, ModulationParameter as P};
use egui::{Rect, Ui, Vec2};
pub const CONTROL_SIZE: f32 = 42.0;
const CONTROL_SCALE: f32 = CONTROL_SIZE / knobs::CHOICE_SIZE;
pub const CONTROL_GAP: f32 = 3.0;
pub fn band_controls(
    ui: &mut Ui,
    band: &mut daw_unit_core::MultibandEqBand,
    markers: &[Option<knobs::ModulationMarker>; 3],
    open: &mut bool,
    remove: &mut bool,
    solo: &mut bool,
    unit_enabled: bool,
) -> [Rect; 4] {
    std::array::from_fn(|row| {
        let response = if row == 0 {
            rows::centered(ui, CONTROL_SIZE, |ui| {
                let previous = band.mode;
                let mut response = menus::Dropdown::from_id_salt(("band-mode", band.id))
                    .small_items()
                    .selected_text(
                        egui::RichText::new(band.mode.label()).size(panels::HEADER_FONT_SIZE),
                    )
                    .width(ui.available_width())
                    .truncate()
                    .show_ui(ui, |ui| {
                        for mode in EqBandMode::ALL {
                            ui.selectable_value(&mut band.mode, mode, mode.label());
                        }
                    })
                    .response;
                if band.mode != previous {
                    response.mark_changed();
                }
                crate::control_capture::record(ui, &response);
                response.on_hover_text(format!("Band mode: {}", band.mode.label()))
            })
            .inner
        } else {
            knobs::row(ui, band, CONTROL_SIZE, |ui, band| {
                let i = match row {
                    1 => 1,
                    2 => 0,
                    _ => 2,
                };
                let enabled = i != 0 || band.mode.uses_gain();
                let value = match i {
                    0 => &mut band.gain_db,
                    1 => &mut band.frequency_hz,
                    _ => &mut band.q,
                };
                ui.add_enabled_ui(enabled, |ui| {
                    ui.add(
                        Knob::new(
                            value,
                            parameter_knobs::destination(P::EQ_BAND[i])
                                .display_scale(CONTROL_SCALE),
                        )
                        .modulated(markers[i].as_ref(), open),
                    )
                })
                .inner
            })
        };
        let mut menu_response = response.clone();
        if row == 0 {
            // The selector popup and its secondary-click actions need separate IDs.
            menu_response.id = response.id.with("band-actions");
        }
        menus::band_context_menu(
            &menu_response,
            &mut band.enabled,
            solo,
            remove,
            true,
            unit_enabled,
        );
        response.rect
    })
}
pub struct BandBounds {
    pub response: egui::Response,
    pub section: Rect,
    pub controls: [Rect; 4],
    pub power: Rect,
    pub solo: Rect,
    pub meter: Rect,
    pub remove: bool,
}
pub fn band(
    ui: &mut Ui,
    band: &mut daw_unit_core::MultibandEqBand,
    index: usize,
    dimensions: (bool, f32),
    markers: &[Option<knobs::ModulationMarker>; 3],
    open: &mut bool,
    audition: (meters::OutputLevel<'_>, &mut bool, bool),
) -> BandBounds {
    let (level, solo, unit_enabled) = audition;
    let (compact, height) = dimensions;
    let mut controls = [Rect::NOTHING; 4];
    let mut power = Rect::NOTHING;
    let mut solo_rect = Rect::NOTHING;
    let mut remove = false;
    let strip = ui.scope_builder(
        egui::UiBuilder::new().sense(egui::Sense::click_and_drag()),
        |ui| {
            panels::unit_strip(ui, height, egui::UiBuilder::new(), |ui| {
                let footer_top = ui.cursor().top() + height - crate::layout::COMPACT_BUTTON_SIZE;
                ui.spacing_mut().item_spacing.y = CONTROL_GAP;
                if !compact {
                    controls =
                        band_controls(ui, band, markers, open, &mut remove, solo, unit_enabled);
                    ui.add_space((footer_top - ui.cursor().top()).max(0.0));
                }
                rows::centered(
                    ui,
                    if compact {
                        height
                    } else {
                        crate::layout::COMPACT_BUTTON_SIZE
                    },
                    |ui| {
                        let width =
                            (ui.available_width() - crate::layout::COMPACT_BUTTON_SIZE - 4.0)
                                .max(0.0);
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let response = ui
                            .allocate_ui_with_layout(
                                Vec2::new(width, crate::layout::COMPACT_BUTTON_SIZE),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    rows::power_label(
                                        ui,
                                        &format!("B{}", index + 1),
                                        &format!("Toggle EQ band {}", index + 1),
                                        band.enabled,
                                    )
                                },
                            )
                            .inner
                            .on_hover_text("Off bypasses this band. Right-click for band actions.");
                        if response.clicked() {
                            band.enabled = !band.enabled;
                        }
                        power = response.rect;
                        menus::band_context_menu(
                            &response,
                            &mut band.enabled,
                            solo,
                            &mut remove,
                            true,
                            unit_enabled,
                        );
                        let response =
                            rows::band_solo(ui, &format!("Solo EQ band {}", index + 1), solo);
                        solo_rect = response.rect;
                        menus::band_context_menu(
                            &response,
                            &mut band.enabled,
                            solo,
                            &mut remove,
                            true,
                            unit_enabled,
                        );
                    },
                );
            })
        },
    );
    menus::band_context_menu(
        &strip.response,
        &mut band.enabled,
        solo,
        &mut remove,
        true,
        unit_enabled,
    );
    let mut b = BandBounds {
        section: strip.response.rect,
        response: strip.response,
        controls,
        power,
        solo: solo_rect,
        meter: Rect::NOTHING,
        remove,
    };
    if !compact {
        b.meter = meters::unit_monitor_rail(b.section, crate::layout::PANEL_MARGIN);
        let response = meters::band_output_level(
            ui,
            b.meter,
            level,
            unit_enabled,
            band.enabled,
            &format!("Band {} output", index + 1),
        );
        menus::band_context_menu(
            &response,
            &mut band.enabled,
            solo,
            &mut b.remove,
            true,
            unit_enabled,
        );
    }
    b
}
pub struct OutputBounds {
    pub section: Rect,
    pub control: Rect,
    pub meter: Rect,
}
pub fn output(
    ui: &mut Ui,
    gain: &mut f32,
    dimensions: (bool, f32),
    marker: Option<&knobs::ModulationMarker>,
    open: &mut bool,
    level: Option<meters::OutputLevel<'_>>,
    enabled: bool,
) -> OutputBounds {
    let (compact, height) = dimensions;
    let mut control = Rect::NOTHING;
    let section = panels::unit_strip(ui, height, egui::UiBuilder::new(), |ui| {
        if !compact {
            for row in 0..3 {
                let r = knobs::row(ui, gain, knobs::CHOICE_SIZE, |ui, v| {
                    if row == 0 {
                        ui.add(
                            Knob::new(v, parameter_knobs::destination(P::Level))
                                .modulated(marker, open),
                        )
                    } else {
                        ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover())
                    }
                });
                if row == 0 {
                    control = r.rect;
                }
            }
        }
        rows::centered(
            ui,
            if compact {
                height
            } else {
                crate::layout::COMPACT_BUTTON_SIZE
            },
            |ui| rows::unit_label(ui, "Out"),
        );
    })
    .response
    .rect;
    let meter = match (compact, level) {
        (false, Some(level)) => {
            meters::effect_output_level(ui, section, crate::layout::PANEL_MARGIN, level, enabled)
        }
        _ => Rect::NOTHING,
    };
    OutputBounds {
        section,
        control,
        meter,
    }
}

pub struct Controls<'a> {
    pub graph: crate::eq_graph::Input<'a>,
    pub solos: &'a mut [bool],
    pub add_enabled: bool,
    pub markers: &'a [[Option<knobs::ModulationMarker>; 3]],
    pub levels: &'a [meters::OutputLevel<'a>],
    pub output_marker: Option<&'a knobs::ModulationMarker>,
    pub output_level: Option<meters::OutputLevel<'a>>,
}
pub struct Bounds {
    pub graph: crate::eq_graph::Bounds,
    pub sections: Vec<Rect>,
    pub bands: Vec<Rect>,
    pub band_responses: Vec<egui::Response>,
    pub knobs: Vec<[Rect; 4]>,
    pub power: Vec<Rect>,
    pub solo: Vec<Rect>,
    pub meters: Vec<Rect>,
    pub add: Option<egui::Response>,
    pub output: Rect,
    pub output_control: Rect,
    pub output_meter: Rect,
    pub remove: Option<usize>,
}
/// Canonical band bank body. The owning host applies add/remove requests after painting.
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::MultibandEqSettings,
    dimensions: (bool, f32),
    controls: Controls<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        graph: Default::default(),
        sections: vec![],
        bands: vec![],
        band_responses: vec![],
        knobs: vec![],
        power: vec![],
        solo: vec![],
        meters: vec![],
        add: None,
        output: Rect::NOTHING,
        output_control: Rect::NOTHING,
        output_meter: Rect::NOTHING,
        remove: None,
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        bounds.graph =
            crate::eq_graph::show(ui, settings, dimensions.1, !dimensions.0, controls.graph);
        for (i, band) in settings.bands.iter_mut().enumerate() {
            let mut unused = false;
            let empty = std::array::from_fn(|_| None);
            let b = ui
                .push_id(band.id, |ui| {
                    self::band(
                        ui,
                        band,
                        i,
                        dimensions,
                        controls.markers.get(i).unwrap_or(&empty),
                        &mut settings.modulation_open,
                        (
                            controls.levels.get(i).copied().unwrap_or(0.0.into()),
                            controls.solos.get_mut(i).unwrap_or(&mut unused),
                            settings.enabled,
                        ),
                    )
                })
                .inner;
            if b.remove {
                bounds.remove = Some(i);
            }

            bounds.sections.push(b.section);
            bounds.bands.push(b.section);
            bounds.band_responses.push(b.response);
            bounds.knobs.push(b.controls);
            bounds.power.push(b.power);
            bounds.solo.push(b.solo);
            if !dimensions.0 {
                bounds.meters.push(b.meter);
            }
        }
        let add = ui
            .add_enabled_ui(controls.add_enabled, |ui| {
                panels::unit_add_strip(ui, dimensions.1, "Add EQ band")
            })
            .inner;

        bounds.sections.push(add.response.rect);
        bounds.add = Some(add.inner);
        let out = output(
            ui,
            &mut settings.output_gain_db,
            dimensions,
            controls.output_marker,
            &mut settings.modulation_open,
            controls.output_level,
            settings.enabled,
        );

        bounds.sections.push(out.section);
        bounds.output = out.section;
        bounds.output_control = out.control;
        bounds.output_meter = out.meter;
    });
    bounds
}
