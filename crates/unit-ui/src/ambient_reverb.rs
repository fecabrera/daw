use crate::{
    knobs::{self, Knob, ModulationMarker},
    layout as panels, meters, parameter_knobs, rows,
};
use daw_unit_core::{AmbientReverbSettings as Settings, ModulationParameter as P};
use egui::{Rect, Ui};
pub struct Bounds {
    pub power: Rect,
    pub sections: [Rect; 3],
    pub controls: [Rect; 9],
    pub meter: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            power: Rect::NOTHING,
            sections: [Rect::NOTHING; 3],
            controls: [Rect::NOTHING; 9],
            meter: Rect::NOTHING,
        }
    }
}
pub fn show(
    ui: &mut Ui,
    s: &mut Settings,
    compact: bool,
    height: f32,
    markers: &[Option<ModulationMarker>; 7],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut b = Bounds::default();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for section in 0..3 {
            let strip =
                panels::unit_strip(ui, height, egui::UiBuilder::new().id_salt(section), |ui| {
                    if !compact {
                        for row in 0..3 {
                            let mut control = |ui: &mut Ui, s: &mut Settings| match (section, row) {
                                (0, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.decay_seconds,
                                            parameter_knobs::destination(P::AmbientDecay),
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
                                            &mut s.bloom,
                                            parameter_knobs::destination(P::AmbientBloom),
                                        )
                                        .modulated(markers[1].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[1] = r.rect;
                                    r
                                }
                                (0, 2) => {
                                    let r = crate::reverb_controls::switch(
                                        ui,
                                        &mut s.hq,
                                        "hq",
                                        "Quality",
                                        ["ECO", "HQ"],
                                        true,
                                    );
                                    b.controls[7] = r.rect;
                                    r
                                }
                                (1, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.tone,
                                            parameter_knobs::destination(P::AmbientTone),
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
                                            &mut s.motion,
                                            parameter_knobs::destination(P::AmbientMotion),
                                        )
                                        .modulated(markers[3].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[3] = r.rect;
                                    r
                                }
                                (1, 2) => {
                                    let r = crate::reverb_controls::power(
                                        ui,
                                        &mut s.highpass_enabled,
                                        "High-pass",
                                        "Toggle High-pass",
                                    );
                                    b.controls[8] = r.rect;
                                    r
                                }
                                (2, 0) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.width,
                                            parameter_knobs::destination(P::ReverbWidth),
                                        )
                                        .modulated(markers[4].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[4] = r.rect;
                                    r
                                }
                                (2, 1) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.level_db,
                                            parameter_knobs::destination(P::Level),
                                        )
                                        .modulated(markers[5].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[5] = r.rect;
                                    r
                                }
                                (2, 2) => {
                                    let r = ui.add_enabled(
                                        true,
                                        Knob::new(
                                            &mut s.dry_wet,
                                            parameter_knobs::destination(P::DryWet),
                                        )
                                        .modulated(markers[6].as_ref(), &mut s.modulation_open),
                                    );
                                    b.controls[6] = r.rect;
                                    r
                                }
                                _ => unreachable!(),
                            };
                            if matches!((section, row), (0, 2) | (1, 2)) {
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
                        |ui| rows::unit_label(ui, ["Reverb", "Texture", "Out"][section]),
                    );
                })
                .response
                .rect;
            if section > 0 {
                panels::unit_divider(ui, strip.left(), strip.y_range().into());
            }
            if section == 2 && !compact {
                b.meter =
                    meters::effect_output_level(ui, strip, panels::PANEL_MARGIN, level, s.enabled);
            }
            b.sections[section] = strip;
        }
    });
    b
}
