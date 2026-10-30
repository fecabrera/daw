use crate::{
    knobs::{self, Knob, KnobSpec},
    layout as panels, meters, parameter_knobs, rows,
};
use daw_unit_core::{EqBandMode, ModulationParameter as P};
use egui::{Rect, Ui, Vec2};
pub const CONTROL_SIZE: f32 = 42.0;
const CONTROL_SCALE: f32 = CONTROL_SIZE / knobs::CHOICE_SIZE;
pub const CONTROL_GAP: f32 = 3.0;
fn remove_menu(response: &egui::Response, remove: &mut bool) {
    crate::menus::ContextMenu::new(response).show(|ui| {
        if ui.button("Remove band").clicked() {
            *remove = true;
            ui.close();
        }
    });
}
pub fn diagram(value: f32) -> Vec<Vec2> {
    let mode = EqBandMode::ALL[value.round() as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let peak = (-((x - 0.5) * 5.0).powi(2)).exp();
            let y = match mode {
                EqBandMode::Bell => 0.5 + 0.5 * peak,
                EqBandMode::LowShelf => 0.25 + 0.5 * low,
                EqBandMode::HighShelf => 0.75 - 0.5 * low,
                EqBandMode::LowPass => low,
                EqBandMode::HighPass => 1.0 - low,
                EqBandMode::Notch => 1.0 - peak,
            };
            Vec2::new(x - 0.5, 0.5 - y)
        })
        .collect()
}
pub fn band_controls(
    ui: &mut Ui,
    band: &mut daw_unit_core::MultibandEqBand,
    markers: &[Option<knobs::ModulationMarker>; 3],
    open: &mut bool,
    remove: &mut bool,
) -> [Rect; 4] {
    std::array::from_fn(|row| {
        let response = knobs::row(ui, band, CONTROL_SIZE, |ui, band| {
            if row == 0 {
                return crate::lfo_controls::choice(
                    ui,
                    &mut band.mode,
                    &EqBandMode::ALL,
                    KnobSpec::new("Band mode", 0.0..=5.0, 0.0, 1.0)
                        .stepped(1.0)
                        .cyclic()
                        .diagrams(diagram)
                        .display_scale(CONTROL_SCALE)
                        .format(|v| EqBandMode::ALL[v.round() as usize].label().into()),
                );
            }
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
                        parameter_knobs::destination(P::EQ_BAND[i]).display_scale(CONTROL_SCALE),
                    )
                    .modulated(markers[i].as_ref(), open),
                )
            })
            .inner
        });
        remove_menu(&response, remove);
        response.rect
    })
}
pub struct BandBounds {
    pub response: egui::Response,
    pub section: Rect,
    pub controls: [Rect; 4],
    pub power: Rect,
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
    level: f32,
) -> BandBounds {
    let (compact, height) = dimensions;
    let mut controls = [Rect::NOTHING; 4];
    let mut power = Rect::NOTHING;
    let mut remove = false;
    let strip = ui.scope_builder(
        egui::UiBuilder::new().sense(egui::Sense::click_and_drag()),
        |ui| {
            panels::unit_strip(ui, height, egui::UiBuilder::new(), |ui| {
                ui.spacing_mut().item_spacing.y = CONTROL_GAP;
                if !compact {
                    controls = band_controls(ui, band, markers, open, &mut remove);
                }
                rows::centered(
                    ui,
                    if compact {
                        height
                    } else {
                        crate::layout::COMPACT_BUTTON_SIZE
                    },
                    |ui| {
                        let response = rows::power_label(
                            ui,
                            &format!("Band {}", index + 1),
                            &format!("Toggle EQ band {}", index + 1),
                            band.enabled,
                        )
                        .on_hover_text("Off bypasses this band. Right-click to remove it.");
                        if response.clicked() {
                            band.enabled = !band.enabled;
                        }
                        power = response.rect;
                        remove_menu(&response, &mut remove);
                    },
                );
            })
        },
    );
    let mut b = BandBounds {
        section: strip.response.rect,
        response: strip.response,
        controls,
        power,
        meter: Rect::NOTHING,
        remove,
    };
    if !compact {
        b.meter = meters::unit_monitor_rail(b.section, crate::layout::PANEL_MARGIN);
        meters::band_level_rail(
            ui,
            b.meter,
            &format!("Band {}", index + 1),
            level,
            band.enabled,
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
    pub add_enabled: bool,
    pub markers: &'a [[Option<knobs::ModulationMarker>; 3]],
    pub levels: &'a [f32],
    pub output_marker: Option<&'a knobs::ModulationMarker>,
    pub output_level: Option<meters::OutputLevel<'a>>,
}
pub struct Bounds {
    pub sections: Vec<Rect>,
    pub bands: Vec<Rect>,
    pub band_responses: Vec<egui::Response>,
    pub knobs: Vec<[Rect; 4]>,
    pub power: Vec<Rect>,
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
        sections: vec![],
        bands: vec![],
        band_responses: vec![],
        knobs: vec![],
        power: vec![],
        meters: vec![],
        add: None,
        output: Rect::NOTHING,
        output_control: Rect::NOTHING,
        output_meter: Rect::NOTHING,
        remove: None,
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for (i, band) in settings.bands.iter_mut().enumerate() {
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
                        meters::unit_reading(
                            settings.enabled,
                            [controls.levels.get(i).copied().unwrap_or(0.0)],
                        )[0],
                    )
                })
                .inner;
            if b.remove {
                bounds.remove = Some(i);
            }
            if i > 0 {
                panels::unit_divider(ui, b.section.left(), b.section.y_range().into());
            }
            bounds.sections.push(b.section);
            bounds.bands.push(b.section);
            bounds.band_responses.push(b.response);
            bounds.knobs.push(b.controls);
            bounds.power.push(b.power);
            if !dimensions.0 {
                bounds.meters.push(b.meter);
            }
        }
        let add = ui
            .add_enabled_ui(controls.add_enabled, |ui| {
                panels::unit_add_strip(ui, dimensions.1, "Add EQ band")
            })
            .inner;
        if !settings.bands.is_empty() {
            panels::unit_divider(
                ui,
                add.response.rect.left(),
                add.response.rect.y_range().into(),
            );
        }
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
        panels::unit_divider(ui, out.section.left(), out.section.y_range().into());
        bounds.sections.push(out.section);
        bounds.output = out.section;
        bounds.output_control = out.control;
        bounds.output_meter = out.meter;
    });
    bounds
}
