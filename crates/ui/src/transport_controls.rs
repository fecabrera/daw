use crate::{icons, theme};
use egui::{Response, Ui, Vec2};

pub(super) const PLAY_FILL: egui::Color32 = egui::Color32::from_rgb(51, 133, 85);
pub(super) const STOP_FILL: egui::Color32 = egui::Color32::from_rgb(197, 78, 88);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopAction {
    Stop,
    ReturnToOrigin,
    AtOrigin,
}
impl StopAction {
    pub fn from_transport(playing: bool, can_return_to_start: bool, playhead_frame: u64) -> Self {
        if playing || can_return_to_start {
            Self::Stop
        } else if playhead_frame > 0 {
            Self::ReturnToOrigin
        } else {
            Self::AtOrigin
        }
    }
}

pub(super) struct Buttons {
    pub play: Response,
    pub stop: Response,
    pub looping: Response,
}

pub(super) fn show(
    ui: &mut Ui,
    playing: bool,
    stop_action: StopAction,
    looping: bool,
    can_play: bool,
    can_loop: bool,
) -> Buttons {
    let size = Vec2::splat(icons::TOOLBAR_BUTTON_SIZE);
    let foreground = egui::Color32::WHITE;
    let play = ui
        .add_enabled_ui(can_play, |ui| {
            icons::borderless_toggle_with_colors(
                ui,
                icons::Lucide::Play,
                if playing { "Pause" } else { "Play" },
                playing,
                size,
                (PLAY_FILL, foreground),
            )
        })
        .inner;
    let (icon, label) = if stop_action == StopAction::Stop {
        (icons::Lucide::Square, "Stop")
    } else {
        (icons::Lucide::ChevronFirst, "Return to origin")
    };
    let stop = ui
        .add_enabled_ui(stop_action != StopAction::AtOrigin, |ui| {
            icons::borderless_toggle_with_colors(
                ui,
                icon,
                label,
                playing,
                size,
                (STOP_FILL, foreground),
            )
        })
        .inner;
    let looping = ui
        .add_enabled_ui(can_loop, |ui| {
            icons::borderless_toggle_with_colors(
                ui,
                icons::Lucide::Repeat,
                "Loop",
                looping,
                size,
                (theme::LOOP_COLOR, foreground),
            )
        })
        .inner;
    Buttons {
        play,
        stop,
        looping,
    }
}
