use crate::{icons, theme};
use egui::{Response, Ui, Vec2};

pub(super) const PLAY_FILL: egui::Color32 = egui::Color32::from_rgb(51, 133, 85);
pub(super) const STOP_FILL: egui::Color32 = egui::Color32::from_rgb(197, 78, 88);

pub(super) struct Buttons {
    pub play: Response,
    pub stop: Response,
    pub looping: Response,
}

pub(super) fn show(
    ui: &mut Ui,
    playing: bool,
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
    let stop = icons::borderless_toggle_with_colors(
        ui,
        icons::Lucide::Square,
        "Stop",
        playing,
        size,
        (STOP_FILL, foreground),
    );
    stop.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Stop")
    });
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
