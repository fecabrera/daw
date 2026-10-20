//! Shared track/layer mixing controls and numeric commit rules.
use crate::{Inputs, faders, knobs, numeric_input, rows, toolbars};
use egui::{Rect, Response, Ui};

pub(super) fn mute_solo(
    ui: &mut Ui,
    muted: &mut bool,
    soloed: &mut bool,
    tooltips: [&str; 2],
) -> [Response; 2] {
    std::array::from_fn(|index| {
        let (label, name, selected) = if index == 0 {
            ("M", "Mute", &mut *muted)
        } else {
            ("S", "Solo", &mut *soloed)
        };
        let response = toolbars::square_toggle(ui, label, *selected).on_hover_text(tooltips[index]);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), *selected, name)
        });
        if response.clicked() {
            *selected = !*selected;
        }
        response
    })
}

pub(super) struct GainPanResponse {
    pub bounds: [Rect; 4],
    pub error: Option<&'static str>,
}

/// Knobs apply immediately. Numeric drafts commit on Enter or focus loss; Escape cancels.
pub(super) fn gain_pan(
    ui: &mut Ui,
    gain: &mut f32,
    pan: &mut f32,
    inputs: &mut Inputs,
) -> GainPanResponse {
    let mut error = None;
    let bounds = rows::centered(ui, knobs::SIZE, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let gain_knob = knobs::gain(ui, gain);
        if gain_knob.changed() {
            inputs.gain = faders::format_gain(*gain);
        }
        let gain_input = numeric_input(ui, &mut inputs.gain, "Gain (dB)");
        if cancel_input(ui, &gain_input) {
            inputs.gain = faders::format_gain(*gain);
            gain_input.surrender_focus();
        } else if commit_input(ui, &gain_input) {
            match faders::parse_gain(&inputs.gain) {
                Some(value) => *gain = value,
                None => {
                    inputs.gain = faders::format_gain(*gain);
                    error = Some("Gain must be a finite number or −∞");
                }
            }
        }
        let pan_knob = knobs::pan(ui, pan);
        if pan_knob.changed() {
            inputs.pan = format!("{pan:.2}");
        }
        let pan_input = numeric_input(ui, &mut inputs.pan, "Pan (-1 left, 0 center, +1 right)");
        if cancel_input(ui, &pan_input) {
            inputs.pan = format!("{pan:.2}");
            pan_input.surrender_focus();
        } else if commit_input(ui, &pan_input) {
            match inputs.pan.parse::<f32>() {
                Ok(value) if value.is_finite() && (-1.0..=1.0).contains(&value) => *pan = value,
                _ => {
                    inputs.pan = format!("{pan:.2}");
                    error = Some("Pan must be between -1 and 1");
                }
            }
        }
        [
            gain_knob.rect,
            gain_input.rect,
            pan_knob.rect,
            pan_input.rect,
        ]
    })
    .inner;
    GainPanResponse { bounds, error }
}

pub(super) fn cancel_input(ui: &Ui, response: &Response) -> bool {
    !ui.is_enabled()
        || !ui.input(|input| input.focused)
        || ((response.has_focus() || response.lost_focus())
            && ui.input(|input| input.key_pressed(egui::Key::Escape)))
}
pub(super) fn commit_input(ui: &Ui, response: &Response) -> bool {
    response.lost_focus()
        || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
}
