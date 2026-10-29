use crate::{icons, theme, toolbars};
use egui::{Rect, Response, Stroke, Ui, Vec2};

pub(super) const WIDTH: f32 = toolbars::HEIGHT;

pub(super) fn bounds(lane: Rect) -> Rect {
    Rect::from_min_max(
        egui::pos2((lane.right() - WIDTH).max(lane.left()), lane.top()),
        lane.max,
    )
}

/// Shared lane rail for Effects and each Instrument layer, outside horizontal scrolling.
pub(super) fn show(
    ui: &mut Ui,
    bounds: Rect,
    compact: &mut bool,
    enabled: bool,
    artwork: [icons::Lucide; 2],
    names: [&str; 2],
) -> Response {
    let colors = theme::palette(ui.ctx());
    ui.painter().rect_filled(bounds, 0.0, colors.panel);
    // Pinned Output shares the preceding scroll region's bottom pixel.
    // Its row owner paints that separator after the lane content.
    let shared_top = ui
        .stack()
        .iter()
        .any(|node| node.tags().contains(crate::panels::SHARED_TOP_BORDER_TAG));
    if !shared_top {
        ui.painter().hline(
            bounds.x_range(),
            bounds.top() + 0.5,
            Stroke::new(1.0, colors.border),
        );
    }
    ui.painter().vline(
        bounds.left() + 0.5,
        bounds.y_range(),
        Stroke::new(1.0, colors.border),
    );
    let inset = (WIDTH - icons::TOOLBAR_BUTTON_SIZE) * 0.5;
    let mut rail = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("lane-toolbar")
            .max_rect(bounds.shrink(inset)),
    );
    rail.set_clip_rect(bounds.intersect(ui.clip_rect()));
    compact_button(&mut rail, compact, enabled, artwork, names)
}

/// Toggle the saved lane presentation on release.
fn compact_button(
    ui: &mut Ui,
    compact: &mut bool,
    enabled: bool,
    artwork: [icons::Lucide; 2],
    names: [&str; 2],
) -> Response {
    let index = usize::from(!*compact);
    let icon = artwork[index];
    let name = names[index];
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            icons::borderless_toggle(
                ui,
                icon,
                name,
                false,
                Vec2::splat(icons::TOOLBAR_BUTTON_SIZE),
            )
        })
        .inner;
    if (response.is_pointer_button_down_on() || response.clicked())
        && ui.rect_contains_pointer(response.rect)
    {
        response.request_focus();
    }
    if response.clicked() && ui.rect_contains_pointer(response.rect) {
        *compact = !*compact;
    }
    response
}
