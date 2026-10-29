use crate::{icons, theme, toolbars};
use egui::{Rect, Stroke, Ui, Vec2};

pub(super) const WIDTH: f32 = toolbars::HEIGHT;
pub(super) const BUTTON_SIZE: f32 = icons::TOOLBAR_BUTTON_SIZE;

pub(super) struct Buttons {
    pub library: egui::Response,
    pub components: Option<egui::Response>,
    pub clip_settings: Option<egui::Response>,
    pub lane: egui::Response,
    pub chords: Option<egui::Response>,
}

pub(super) struct Catalogs {
    pub effects: bool,
    pub components: Option<bool>,
    pub clip_settings: Option<bool>,
    pub chords: Option<bool>,
}

pub(super) fn show(
    ui: &mut Ui,
    bounds: Rect,
    catalogs: Catalogs,
    lane_visible: bool,
    enabled: bool,
) -> Buttons {
    let palette = theme::palette(ui.ctx());
    ui.painter().rect_filled(bounds, 0.0, palette.panel);
    ui.painter().vline(
        bounds.right() - 0.5,
        bounds.y_range(),
        Stroke::new(1.0, palette.border),
    );
    let padding = (WIDTH - BUTTON_SIZE) * 0.5;
    let top = bounds.min + Vec2::splat(padding);
    let step = BUTTON_SIZE + padding;
    let effects_at = top
        + Vec2::new(
            0.0,
            if catalogs.clip_settings.is_some() || catalogs.components.is_some() {
                step
            } else {
                0.0
            },
        );
    let bottom = egui::pos2(top.x, bounds.bottom() - padding - BUTTON_SIZE);
    let mut rail = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("timeline-toolbar")
            .max_rect(bounds),
    );
    rail.set_clip_rect(bounds.intersect(ui.clip_rect()));
    Buttons {
        components: catalogs.components.map(|selected| {
            toggle_at(
                &mut rail,
                top,
                "instrument-components",
                icons::Lucide::KeyboardMusic,
                "Show instrument components catalog",
                selected,
                enabled,
            )
        }),
        clip_settings: catalogs.clip_settings.map(|selected| {
            toggle_at(
                &mut rail,
                top,
                "clip-settings",
                icons::Lucide::FileHeadphone,
                "Show clip settings",
                selected,
                enabled,
            )
        }),
        library: toggle_at(
            &mut rail,
            effects_at,
            "library",
            icons::Lucide::Library,
            "Show effects catalog",
            catalogs.effects,
            enabled,
        ),
        chords: catalogs.chords.map(|selected| {
            toggle_at(
                &mut rail,
                top + Vec2::new(
                    0.0,
                    if catalogs.clip_settings.is_some() || catalogs.components.is_some() {
                        2.0 * step
                    } else {
                        step
                    },
                ),
                "chords",
                icons::Lucide::Piano,
                "Show chords catalog",
                selected,
                enabled,
            )
        }),
        lane: toggle_at(
            &mut rail,
            bottom,
            "effects-lane",
            icons::Lucide::GalleryHorizontal,
            "Show effects lane",
            lane_visible,
            enabled,
        ),
    }
}

fn toggle_at(
    ui: &mut Ui,
    position: egui::Pos2,
    id: &str,
    icon: icons::Lucide,
    name: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let mut button = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("timeline-toolbar", id))
            .max_rect(Rect::from_min_size(position, Vec2::splat(BUTTON_SIZE)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    button.set_clip_rect(ui.clip_rect());
    button
        .add_enabled_ui(enabled, |ui| {
            icons::borderless_toggle(ui, icon, name, selected, Vec2::splat(BUTTON_SIZE))
        })
        .inner
}
