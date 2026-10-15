use crate::{icons, theme, toolbars};
use egui::{Rect, Stroke, Ui, Vec2};

pub(super) const WIDTH: f32 = toolbars::HEIGHT;
pub(super) const BUTTON_SIZE: f32 = icons::TOOLBAR_BUTTON_SIZE;

pub(super) struct Buttons {
    pub library: egui::Response,
    pub components: Option<egui::Response>,
    pub lane: egui::Response,
}

pub(super) fn show(
    ui: &mut Ui,
    bounds: Rect,
    catalog_visible: bool,
    components_visible: Option<bool>,
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
            catalog_visible,
            enabled,
        ),
        components: components_visible.map(|selected| {
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
