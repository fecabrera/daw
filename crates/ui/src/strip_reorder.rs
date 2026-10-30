//! Temporary unit-strip gesture state. Models change only after a valid release.
use crate::drag;
use daw_core::{Id, Modulation, MultibandEqBand};
use egui::{Context, Pos2, Rect, Response, Ui};

#[derive(Clone)]
struct State {
    host: egui::Id,
    owner: egui::Id,
    unit: Id,
    original: Original,
    origin: Pos2,
    focus: egui::Id,
    active: bool,
    frame: u64,
}
#[derive(Clone)]
enum Original {
    Routes(Vec<Modulation>),
    EqBands(Vec<MultibandEqBand>),
}
fn key() -> egui::Id {
    egui::Id::new("modulation-strip-drag")
}
fn host_key() -> egui::Id {
    key().with("host")
}
fn state(ctx: &Context) -> Option<State> {
    ctx.data(|d| d.get_temp(key()))
}
fn clear(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<State>(key()));
}
pub(super) fn dragging(ctx: &Context) -> bool {
    state(ctx).is_some()
}
pub(super) fn begin_frame(ui: &Ui, host: egui::Id, allowed: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(host_key(), host));
    if let Some(s) = state(ui.ctx())
        && (!allowed || drag::cancelled(ui) || s.host != host)
    {
        if ui.input(|i| i.focused && i.key_pressed(egui::Key::Escape)) {
            drag::restore_focus(ui.ctx(), s.focus);
        }
        clear(ui.ctx());
    }
}
pub(super) fn end_frame(ctx: &Context) {
    if state(ctx).is_some_and(|s| s.frame != ctx.cumulative_frame_nr()) {
        clear(ctx);
    }
}
pub(super) fn owner(ui: &Ui) -> egui::Id {
    ui.make_persistent_id("modulation-strip-order")
}
pub(super) fn active(ui: &Ui, owner: egui::Id) -> bool {
    let Some(mut s) = state(ui.ctx()).filter(|s| s.owner == owner) else {
        return false;
    };
    if drag::cancelled(ui) {
        clear(ui.ctx());
        return false;
    }
    s.active |= drag::crossed_threshold(ui, s.origin);
    s.frame = ui.ctx().cumulative_frame_nr();
    let active = s.active;
    ui.ctx().data_mut(|d| d.insert_temp(key(), s));
    active
}
pub(super) fn arm(
    ui: &Ui,
    owner: egui::Id,
    response: &Response,
    unit: Id,
    original: &[Modulation],
) {
    arm_items(
        ui,
        owner,
        response,
        unit,
        || Original::Routes(original.to_vec()),
        "Drag strip space to reorder modulation connections.",
    );
}
pub(super) fn arm_bands(
    ui: &Ui,
    owner: egui::Id,
    response: &Response,
    unit: Id,
    original: &[MultibandEqBand],
) {
    arm_items(
        ui,
        owner,
        response,
        unit,
        || Original::EqBands(original.to_vec()),
        "Drag strip space to reorder EQ bands.",
    );
}
fn arm_items(
    ui: &Ui,
    owner: egui::Id,
    response: &Response,
    unit: Id,
    original: impl FnOnce() -> Original,
    tooltip: &str,
) {
    response
        .clone()
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text(tooltip);
    if dragging(ui.ctx()) {
        return;
    }
    let Some(origin) = drag::press_origin(ui, response) else {
        return;
    };
    response.request_focus();
    let host = ui
        .ctx()
        .data(|d| d.get_temp(host_key()))
        .unwrap_or(egui::Id::NULL);
    let frame = ui.ctx().cumulative_frame_nr();
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            key(),
            State {
                host,
                owner,
                unit,
                original: original(),
                origin,
                focus: response.id,
                active: false,
                frame,
            },
        )
    });
}
pub(super) fn finish(
    ui: &Ui,
    owner: egui::Id,
    routes: &mut Vec<Modulation>,
    slots: &[(Id, Rect)],
    bounds: Rect,
) {
    #[cfg(test)]
    for (id, rect) in slots {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new((*id, "route-drag-bounds")), *rect));
    }
    let unchanged = state(ui.ctx()).is_some_and(
        |s| matches!(s.original, Original::Routes(ref original) if routes == original),
    );
    if let Some((old, index)) = finish_move(ui, owner, unchanged, slots, bounds) {
        let route = routes.remove(old);
        routes.insert(index, route);
    }
}
pub(super) fn finish_bands(
    ui: &Ui,
    owner: egui::Id,
    bands: &mut Vec<MultibandEqBand>,
    slots: &[(Id, Rect)],
    bounds: Rect,
) {
    let unchanged = state(ui.ctx()).is_some_and(
        |s| matches!(s.original, Original::EqBands(ref original) if bands == original),
    );
    if let Some((old, index)) = finish_move(ui, owner, unchanged, slots, bounds) {
        let band = bands.remove(old);
        bands.insert(index, band);
    }
}
fn finish_move(
    ui: &Ui,
    owner: egui::Id,
    unchanged: bool,
    slots: &[(Id, Rect)],
    bounds: Rect,
) -> Option<(usize, usize)> {
    let mut s = state(ui.ctx()).filter(|s| s.owner == owner)?;
    if drag::cancelled(ui) || !unchanged {
        clear(ui.ctx());
        return None;
    }
    s.active |= drag::crossed_threshold(ui, s.origin);
    s.frame = ui.ctx().cumulative_frame_nr();
    let target = if s.active {
        ui.input(|i| i.pointer.latest_pos())
            .filter(|p| bounds.contains(*p))
            .and_then(|p| {
                let old = slots.iter().position(|(id, _)| *id == s.unit)?;
                let slot = drag::insertion_slot(p, slots.iter().map(|(_, r)| *r));
                let index = slot - usize::from(old < slot);
                (index != old).then_some((slot, index, old))
            })
    } else {
        None
    };
    if s.active {
        if let Some((slot, _, _)) = target {
            drag::insertion_line(
                ui,
                &slots.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
                slot,
                bounds,
            );
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NotAllowed
        });
        ui.ctx().request_repaint();
    }
    if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
        clear(ui.ctx());
        drag::restore_focus(ui.ctx(), s.focus);
        return target.map(|(_, index, old)| (old, index));
    } else if !ui.input(|i| i.pointer.primary_down()) {
        clear(ui.ctx());
    } else {
        ui.ctx().data_mut(|d| d.insert_temp(key(), s));
    }
    None
}
pub(super) fn scroll(ui: &Ui) {
    if state(ui.ctx()).is_some_and(|s| s.active) {
        drag::scroll_near_edges(ui, drag::Axis::Horizontal, 360.0);
    }
}

#[cfg(test)]
pub(super) fn bounds(ctx: &Context, route: Id) -> Rect {
    ctx.data(|d| d.get_temp(egui::Id::new((route, "route-drag-bounds"))))
        .unwrap()
}
