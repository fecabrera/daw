use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout as panels, meters, parameter_knobs, rows,
};
use daw_unit_core::{ModulationParameter as P, PlateReverbSettings as Settings};
use egui::{Rect, Ui};
pub const STRIP_COUNT: usize = 5;
pub struct Bounds {
    pub power: Rect,
    pub sections: [Rect; STRIP_COUNT],
    pub controls: [Rect; 12],
    pub meter: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            power: Rect::NOTHING,
            sections: [Rect::NOTHING; STRIP_COUNT],
            controls: [Rect::NOTHING; 12],
            meter: Rect::NOTHING,
        }
    }
}
pub fn show(
    ui: &mut Ui,
    s: &mut Settings,
    compact: bool,
    height: f32,
    markers: &[Option<ModulationMarker>; 8],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut b = Bounds::default();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for section in 0..STRIP_COUNT {
            let strip =
                panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt(section), |ui| {
                    if !compact {
                        for row in 0..3 {
                            let mut control = |ui: &mut Ui, s: &mut Settings| match (section, row) {
                                (0, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.predelay_ms,
                                            parameter_knobs::destination(P::PlatePreDelay),
                                        )
                                        .modulated(markers[0].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[0] = r.rect;
                                    r
                                }
                                (0, 1) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.decay_seconds,
                                            parameter_knobs::destination(P::PlateDecay),
                                        )
                                        .modulated(markers[1].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[1] = r.rect;
                                    r
                                }
                                (3, 1) => {
                                    let r = crate::reverb_controls::switch(
                                        ui,
                                        &mut s.gated,
                                        "gated",
                                        "Tail",
                                        ["Normal", "Gated"],
                                        true,
                                    );
                                    b.controls[8] = r.rect;
                                    r
                                }
                                (1, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.highpass_hz,
                                            parameter_knobs::destination(P::PlateHighPass),
                                        )
                                        .modulated(markers[2].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[2] = r.rect;
                                    r
                                }
                                (1, 1) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.lowpass_hz,
                                            parameter_knobs::destination(P::PlateLowPass),
                                        )
                                        .modulated(markers[3].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[3] = r.rect;
                                    r
                                }
                                (3, 2) => {
                                    let r = crate::reverb_controls::switch(
                                        ui,
                                        &mut s.dark,
                                        "dark",
                                        "Color",
                                        ["Bright", "Dark"],
                                        true,
                                    );
                                    b.controls[9] = r.rect;
                                    r
                                }
                                (2, 0) => {
                                    let r = ui.add_enabled(
                                        s.drive_enabled,
                                        Knob::new(
                                            &mut s.drive_db,
                                            parameter_knobs::destination(P::PlateDrive),
                                        )
                                        .modulated(markers[4].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[4] = r.rect;
                                    r
                                }
                                (1, 2) | (2, 1) | (2, 2) | (4, 2) => ui.allocate_response(
                                    egui::vec2(ui.available_width(), knobs::CHOICE_SIZE),
                                    egui::Sense::hover(),
                                ),
                                (3, 0) => {
                                    let r = crate::reverb_controls::switch(
                                        ui,
                                        &mut s.hq,
                                        "hq",
                                        "Quality",
                                        ["ECO", "HQ"],
                                        !s.gated,
                                    );
                                    b.controls[11] = r.rect;
                                    r
                                }
                                (0, 2) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.width,
                                            parameter_knobs::destination(P::ReverbWidth),
                                        )
                                        .modulated(markers[5].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[5] = r.rect;
                                    r
                                }
                                (4, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.level_db,
                                            parameter_knobs::destination(P::Level),
                                        )
                                        .modulated(markers[6].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[6] = r.rect;
                                    r
                                }
                                (4, 1) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.dry_wet,
                                            parameter_knobs::destination(P::DryWet),
                                        )
                                        .modulated(markers[7].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[7] = r.rect;
                                    r
                                }
                                _ => unreachable!(),
                            };
                            if section == 3
                                || matches!((section, row), (1, 2) | (2, 1) | (2, 2) | (4, 2))
                            {
                                control(ui, s);
                            } else {
                                knobs::row(ui, s, knobs::CHOICE_SIZE, control);
                            }
                        }
                    }
                    rows::centered(
                        ui,
                        if compact {
                            height
                        } else {
                            panels::COMPACT_BUTTON_SIZE
                        },
                        |ui| {
                            if section == 2 {
                                let mut response =
                                    rows::power_label(ui, "Drive", "Toggle Drive", s.drive_enabled);
                                b.controls[10] = response.rect;
                                if response.clicked() {
                                    s.drive_enabled = !s.drive_enabled;
                                    response.mark_changed();
                                }
                                response
                            } else {
                                rows::unit_label(
                                    ui,
                                    ["Reverb", "Filters", "", "Options", "Out"][section],
                                )
                            }
                        },
                    );
                })
                .response
                .rect;
            if section > 0 {
                panels::unit_divider(ui, strip.left(), strip.y_range().into());
            }
            if section == 4 && !compact {
                b.meter =
                    meters::effect_output_level(ui, strip, panels::PANEL_MARGIN, level, s.enabled);
            }
            b.sections[section] = strip;
        }
    });
    b
}
