//! Pointer gesture primitives shared by catalog and installed-unit drags.
use egui::{Context, Pos2, Response, Ui, Vec2};

pub(super) fn cancelled(ui: &Ui) -> bool {
    !ui.is_enabled() || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
}

/// Arm on press, including a motion and release delivered together next frame.
pub(super) fn press_origin(ui: &Ui, response: &Response) -> Option<Pos2> {
    if cancelled(ui)
        || !response.is_pointer_button_down_on()
        || !ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
    {
        return None;
    }
    ui.input(|input| input.pointer.press_origin())
        .filter(|origin| response.rect.contains(*origin))
}

pub(super) fn crossed_threshold(ui: &Ui, origin: Pos2) -> bool {
    let threshold = ui
        .ctx()
        .options(|options| options.input_options.max_click_dist);
    ui.input(|input| input.pointer.latest_pos())
        .is_some_and(|pointer| pointer.distance(origin) > threshold)
}

pub(super) fn restore_focus(ctx: &Context, focus: egui::Id) {
    ctx.memory_mut(|memory| memory.request_focus(focus));
}

pub(super) enum Axis {
    Horizontal,
    Vertical,
}

/// Scroll the containing viewport while the pointer is held near an edge.
pub(super) fn scroll_near_edges(ui: &Ui, axis: Axis, speed: f32) {
    scroll_drag(ui, axis, speed, ui.is_enabled(), ui.clip_rect());
}

/// The host can keep a validated drag scrolling while its parameter controls are disabled.
pub(super) fn scroll_drag(ui: &Ui, axis: Axis, speed: f32, allowed: bool, bounds: egui::Rect) {
    if !allowed
        || ui.input(|input| {
            !input.focused || input.key_pressed(egui::Key::Escape) || !input.pointer.primary_down()
        })
    {
        return;
    }
    let viewport = ui.clip_rect().intersect(bounds);
    let Some(pointer) = ui
        .input(|input| input.pointer.latest_pos())
        .filter(|p| viewport.contains(*p))
    else {
        return;
    };
    let (position, start, end) = match axis {
        Axis::Horizontal => (pointer.x, viewport.left(), viewport.right()),
        Axis::Vertical => (pointer.y, viewport.top(), viewport.bottom()),
    };
    let edge = 24.0_f32.min((end - start) * 0.5);
    let direction = if position < start + edge {
        1.0
    } else if position > end - edge {
        -1.0
    } else {
        return;
    };
    let delta = direction * speed * ui.input(|input| input.stable_dt.min(0.05));
    ui.scroll_with_delta_animation(
        match axis {
            Axis::Horizontal => Vec2::new(delta, 0.0),
            Axis::Vertical => Vec2::new(0.0, delta),
        },
        egui::style::ScrollAnimation::none(),
    );
    ui.ctx().request_repaint();
}

/// Shared horizontal insertion policy for effects, oscillator sources and routes.
pub(super) fn insertion_slot(pointer: Pos2, slots: impl IntoIterator<Item = egui::Rect>) -> usize {
    slots
        .into_iter()
        .take_while(|r| pointer.x >= r.center().x)
        .count()
}
pub(super) fn insertion_line(ui: &Ui, slots: &[egui::Rect], slot: usize, viewport: egui::Rect) {
    if viewport.width() < 2.0 || viewport.height() <= 0.0 {
        return;
    }
    let x = slots
        .get(slot)
        .map_or_else(
            || slots.last().map_or(viewport.left(), |r| r.right()),
            |r| r.left(),
        )
        .clamp(viewport.left() + 1.0, viewport.right() - 1.0);
    ui.painter_at(viewport).vline(
        x,
        viewport.y_range(),
        egui::Stroke::new(2.0, crate::theme::accent(ui.ctx())),
    );
}
