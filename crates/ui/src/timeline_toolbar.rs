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
    ui.painter().hline(
        bounds.x_range(),
        bounds.top() + 0.5,
        Stroke::new(1.0, palette.border),
    );
    let padding = (WIDTH - BUTTON_SIZE) * 0.5;
    let top = bounds.min + Vec2::splat(padding);
    let step = BUTTON_SIZE + padding;
    let clip_settings_at = top
        + Vec2::new(
            0.0,
            if catalogs.components.is_some() {
                2.0 * step
            } else {
                step
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
        library: toggle_at(
            &mut rail,
            top,
            "library",
            icons::Lucide::Library,
            "Show effects catalog",
            catalogs.effects,
            enabled,
        ),
        components: catalogs.components.map(|selected| {
            toggle_at(
                &mut rail,
                top + Vec2::new(0.0, BUTTON_SIZE + padding),
                "instrument-components",
                icons::Lucide::Piano,
                "Show instrument components catalog",
                selected,
                enabled,
            )
        }),
        clip_settings: catalogs.clip_settings.map(|selected| {
            toggle_at(
                &mut rail,
                clip_settings_at,
                "clip-settings",
                icons::Lucide::FileHeadphone,
                "Show clip settings",
                selected,
                enabled,
            )
        }),
        chords: catalogs.chords.map(|selected| {
            toggle_at(
                &mut rail,
                top + Vec2::new(0.0, 2.0 * step),
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
