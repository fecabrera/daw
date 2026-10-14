use crate::{icons, theme, toolbars};
use egui::{Rect, Response, Stroke, Ui, Vec2};

pub(super) const WIDTH: f32 = toolbars::HEIGHT;

pub(super) fn bounds(lane: Rect) -> Rect {
    Rect::from_min_max(
        egui::pos2((lane.right() - WIDTH).max(lane.left()), lane.top()),
        lane.max,
    )
}

pub(super) fn show(ui: &mut Ui, bounds: Rect, compact: &mut bool, enabled: bool) -> Response {
    let colors = theme::palette(ui.ctx());
    ui.painter().rect_filled(bounds, 0.0, colors.panel);
    ui.painter().hline(
        bounds.x_range(),
        bounds.top() + 0.5,
        Stroke::new(1.0, colors.border),
    );
    ui.painter().vline(
        bounds.left() + 0.5,
        bounds.y_range(),
        Stroke::new(1.0, colors.border),
    );
    let inset = (WIDTH - icons::TOOLBAR_BUTTON_SIZE) * 0.5;
    let mut rail = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("effects-lane-toolbar")
            .max_rect(bounds.shrink(inset)),
    );
    rail.set_clip_rect(bounds.intersect(ui.clip_rect()));
    compact_button(&mut rail, compact, enabled)
}

/// Toggle the saved chain presentation on release.
fn compact_button(ui: &mut Ui, compact: &mut bool, enabled: bool) -> Response {
    let icon = if *compact {
        icons::Lucide::ArrowUpFromLine
    } else {
        icons::Lucide::ArrowDownFromLine
    };
    let name = if *compact {
        "Expand effects"
    } else {
        "Collapse effects"
    };
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
