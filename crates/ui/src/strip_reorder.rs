//! DAW identity adapters over the shared strip gesture lifecycle.
use daw_core::{Id, Modulation, MultibandEqBand};
pub(super) use daw_unit_ui::strip_reorder::{
    active, begin_frame, dragging, end_frame, owner, scroll,
};
use egui::{Rect, Response, Ui};
pub(super) fn arm_items<T: Clone + Send + Sync + 'static>(
    ui: &Ui,
    owner: egui::Id,
    response: &Response,
    unit: Id,
    original: &[T],
) {
    daw_unit_ui::strip_reorder::arm(ui, owner, response, egui::Id::new(unit), original);
}
pub(super) fn finish_items<T: Clone + PartialEq + Send + Sync + 'static>(
    ui: &Ui,
    owner: egui::Id,
    items: &mut Vec<T>,
    slots: &[(Id, Rect)],
    bounds: Rect,
) {
    #[cfg(test)]
    for (id, rect) in slots {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new((*id, "route-drag-bounds")), *rect));
    }
    let slots: Vec<_> = slots
        .iter()
        .map(|(id, r)| (egui::Id::new(id), *r))
        .collect();
    daw_unit_ui::strip_reorder::finish(ui, owner, items, &slots, bounds);
}
pub(super) use arm_items as arm;
pub(super) use arm_items as arm_bands;
pub(super) fn finish(
    ui: &Ui,
    owner: egui::Id,
    items: &mut Vec<Modulation>,
    slots: &[(Id, Rect)],
    bounds: Rect,
) {
    finish_items(ui, owner, items, slots, bounds);
}
pub(super) fn finish_bands(
    ui: &Ui,
    owner: egui::Id,
    items: &mut Vec<MultibandEqBand>,
    slots: &[(Id, Rect)],
    bounds: Rect,
) {
    finish_items(ui, owner, items, slots, bounds);
}
#[cfg(test)]
pub(super) fn bounds(ctx: &egui::Context, id: Id) -> Rect {
    ctx.data(|d| d.get_temp(egui::Id::new((id, "route-drag-bounds"))))
        .unwrap()
}
