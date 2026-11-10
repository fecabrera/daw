//! One headerless body for the DAW and both standalone formats.
use crate::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout, meters, parameter_knobs, rows,
};
use daw_unit_core::{ModulationParameter as P, ShimmerReverbSettings as Settings};
use egui::{Rect, Response, Ui};
pub const STRIP_COUNT: usize = 6;
pub const BODY_WIDTH: f32 = 5.0 * layout::UNIT_STRIP_WIDTH + layout::unit_columns_width(2);
pub struct Bounds {
    pub power: Rect,
    pub sections: [Rect; STRIP_COUNT],
    /// Stable plugin parameter order, excluding global Bypass.
    pub controls: [Rect; 17],
    pub meter: Rect,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            power: Rect::NOTHING,
            sections: [Rect::NOTHING; STRIP_COUNT],
            controls: [Rect::NOTHING; 17],
            meter: Rect::NOTHING,
        }
    }
}
fn parameter(
    ui: &mut Ui,
    s: &mut Settings,
    index: usize,
    markers: &[Option<ModulationMarker>; 12],
) -> Response {
    let mut value = s.values()[index];
    let spec = parameter_knobs::destination(P::SHIMMER_REVERB[index]);
    let spec = if index == 11 {
        spec.reset_to(0.5)
    } else {
        spec
    };
    let r = ui.add(
        Knob::new(&mut value, spec).modulated(markers[index].as_ref(), &mut s.modulation_open),
    );
    if r.changed() {
        match index {
            0 => s.size = value,
            1 => s.predelay_ms = value,
            2 => s.vowel = value,
            3 => s.vowel_amount = value,
            4 => s.amount_1 = value,
            5 => s.amount_2 = value,
            6 => s.rate = value,
            7 => s.depth = value,
            8 => s.low_damp = value,
            9 => s.high_damp = value,
            10 => s.level_db = value,
            _ => s.dry_wet = value,
        }
    }
    r
}
fn interval(ui: &mut Ui, value: &mut i8, voice: usize) -> Response {
    let mut v = f32::from(*value);
    let r = ui.add(Knob::new(
        &mut v,
        KnobSpec::new(
            if voice == 0 { "Pitch 1" } else { "Pitch 2" },
            -15.0..=15.0,
            0.0,
            1.0,
        )
        .stepped(1.0)
        .centered(0.0)
        .bipolar()
        .value_label(|v| format!("{:+} st", daw_unit_core::shimmer_pitch(v as i8)))
        .format(|v| format!("{:+} semitones", daw_unit_core::shimmer_pitch(v as i8)))
        .endpoint_labels("−12", "+12"),
    ));
    if r.changed() {
        *value = v as i8;
    }
    r
}
fn note(ui: &mut Ui, s: &mut Settings) -> Response {
    rows::selector_row(ui, "Pre-delay", |ui| {
        let before = s.note;
        let mut r = crate::menus::Dropdown::from_id_salt("shimmer-note")
            .small_items()
            .truncate()
            .selected_text(
                egui::RichText::new(Settings::NOTE_LABELS[usize::from(s.note.min(15))])
                    .size(layout::HEADER_FONT_SIZE),
            )
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, label) in Settings::NOTE_LABELS.iter().enumerate() {
                    ui.selectable_value(&mut s.note, i as u8, *label);
                }
            })
            .response
            .on_hover_text(
                "Musical pre-delay. Maximum 8 seconds; slower tempos clamp at that limit.",
            );
        if before != s.note {
            r.mark_changed();
        }
        r.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::ComboBox,
                ui.is_enabled(),
                "Pre-delay note",
            )
        });
        crate::control_capture::record(ui, &r);
        r
    })
}
pub fn show(
    ui: &mut Ui,
    s: &mut Settings,
    compact: bool,
    height: f32,
    markers: &[Option<ModulationMarker>; 12],
    level: meters::OutputLevel<'_>,
    _tempo: f32,
) -> Bounds {
    let mut b = Bounds::default();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, f32::from(crate::theme::TOOLBAR_PADDING));
        for (section, label) in ["Reverb", "Vowel", "Shimmer", "Motion", "Damping", "Out"]
            .into_iter()
            .enumerate()
        {
            let width = if section == 2 {
                layout::unit_columns_width(2)
            } else {
                layout::UNIT_STRIP_WIDTH
            };
            let strip = layout::unit_strip_with_width(
                ui,
                height,
                width,
                egui::UiBuilder::new().id_salt(section),
                |ui| {
                    if !compact {
                        if section == 2 {
                            layout::unit_columns(ui, 2, |ui, column| {
                                for row in 0..3 {
                                    knobs::row(ui, s, knobs::CHOICE_SIZE, |ui, s| {
                                        let (slot, r) = match (column, row) {
                                            (0, 0) => (6, interval(ui, &mut s.pitch_1, 0)),
                                            (1, 0) => (8, interval(ui, &mut s.pitch_2, 1)),
                                            (0, 1) => (7, parameter(ui, s, 4, markers)),
                                            (1, 1) => (9, parameter(ui, s, 5, markers)),
                                            _ => {
                                                return ui.allocate_response(
                                                    egui::vec2(
                                                        ui.available_width(),
                                                        knobs::CHOICE_SIZE,
                                                    ),
                                                    egui::Sense::hover(),
                                                );
                                            }
                                        };
                                        b.controls[slot] = r.rect;
                                        r
                                    });
                                }
                            });
                        } else {
                            for row in 0..3 {
                                // Dropdown rows own the same 52-point allocation as knobs.
                                let mut control = |ui: &mut Ui, s: &mut Settings| {
                                    let (slot, r) = match (section, row) {
                                        (0, 0) => (0, parameter(ui, s, 0, markers)),
                                        (0, 1) if s.sync => (3, note(ui, s)),
                                        (0, 1) => (1, parameter(ui, s, 1, markers)),
                                        (0, 2) => (
                                            2,
                                            crate::reverb_controls::switch(
                                                ui,
                                                &mut s.sync,
                                                "shimmer-sync",
                                                "Mode",
                                                ["Time", "Sync"],
                                                true,
                                            ),
                                        ),
                                        (1, 0) => (4, parameter(ui, s, 2, markers)),
                                        (1, 1) => (5, parameter(ui, s, 3, markers)),
                                        (3, 0) => (10, parameter(ui, s, 6, markers)),
                                        (3, 1) => (11, parameter(ui, s, 7, markers)),
                                        (4, 0) => (12, parameter(ui, s, 8, markers)),
                                        (4, 1) => (13, parameter(ui, s, 9, markers)),
                                        (4, 2) => (
                                            14,
                                            crate::reverb_controls::switch(
                                                ui,
                                                &mut s.warm,
                                                "shimmer-warm",
                                                "Warm",
                                                ["Off", "On"],
                                                true,
                                            ),
                                        ),
                                        (5, 0) => (15, parameter(ui, s, 10, markers)),
                                        (5, 2) => (16, parameter(ui, s, 11, markers)),
                                        _ => {
                                            return ui.allocate_response(
                                                egui::vec2(
                                                    ui.available_width(),
                                                    knobs::CHOICE_SIZE,
                                                ),
                                                egui::Sense::hover(),
                                            );
                                        }
                                    };
                                    b.controls[slot] = r.rect;
                                    r
                                };
                                if matches!((section, row), (0, 2) | (4, 2))
                                    || ((section, row) == (0, 1) && s.sync)
                                {
                                    control(ui, s);
                                } else {
                                    knobs::row(ui, s, knobs::CHOICE_SIZE, control);
                                }
                            }
                        }
                    }
                    rows::centered(
                        ui,
                        if compact {
                            height
                        } else {
                            layout::COMPACT_BUTTON_SIZE
                        },
                        |ui| rows::unit_label(ui, label),
                    );
                },
            )
            .response
            .rect;

            if section == 5 && !compact {
                b.meter =
                    meters::effect_output_level(ui, strip, layout::PANEL_MARGIN, level, s.enabled);
            }
            b.sections[section] = strip;
        }
    });
    b
}
