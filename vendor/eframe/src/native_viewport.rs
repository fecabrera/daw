//! Opt-in lifecycle status for a secondary native viewport.
use egui::{Context, Id, ViewportId};
use std::sync::{Arc, Weak};
pub use winit::window::Window;

/// Native window readiness for a watched secondary viewport.
#[derive(Clone)]
pub enum Status {
    /// The window has not presented yet.
    Opening,
    /// The backend successfully submitted a frame.
    Presented(Weak<Window>),
    /// A window-local startup or presentation error retired this viewport.
    Failed(String),
}

fn key(id: ViewportId) -> Id {
    Id::new(("eframe-native-viewport-status", id))
}

/// Watch a fresh secondary viewport ID before registering it.
pub fn watch(ctx: &Context, id: ViewportId) {
    if id != ViewportId::ROOT {
        ctx.data_mut(|data| data.insert_temp(key(id), Status::Opening));
    }
}

/// Read the status of a watched viewport.
pub fn status(ctx: &Context, id: ViewportId) -> Option<Status> {
    ctx.data(|data| data.get_temp(key(id)))
}

/// Remove the status after the application retires a viewport.
pub fn forget(ctx: &Context, id: ViewportId) {
    ctx.data_mut(|data| data.remove::<Status>(key(id)));
}

/// A presented dialog can move using its last submitted frame. Avoid rendering
/// for each move notification. Resize and scale changes still request frames.
pub(crate) fn compositor_handles_move(
    ctx: &Context,
    id: Option<ViewportId>,
    event: &winit::event::WindowEvent,
) -> bool {
    matches!(event, winit::event::WindowEvent::Moved(_))
        && id.is_some_and(|id| matches!(status(ctx, id), Some(Status::Presented(_))))
}

pub(crate) fn is_failed(ctx: &Context, id: ViewportId) -> bool {
    matches!(status(ctx, id), Some(Status::Failed(_)))
}

pub(crate) fn failed(ctx: &Context, id: ViewportId, error: &dyn std::fmt::Display) -> bool {
    if status(ctx, id).is_none() {
        return false;
    }
    ctx.data_mut(|data| data.insert_temp(key(id), Status::Failed(error.to_string())));
    ctx.request_repaint_of(ViewportId::ROOT);
    true
}

pub(crate) fn presented(ctx: &Context, id: ViewportId, window: &Arc<Window>) {
    if matches!(status(ctx, id), Some(Status::Opening)) {
        ctx.data_mut(|data| data.insert_temp(key(id), Status::Presented(Arc::downgrade(window))));
        ctx.request_repaint_of(ViewportId::ROOT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_presented_dialog_moves_use_the_cached_frame() {
        use winit::{
            dpi::{PhysicalPosition, PhysicalSize},
            event::WindowEvent,
        };
        let ctx = Context::default();
        let dialog = ViewportId::from_hash_of("dialog");
        let moved = WindowEvent::Moved(PhysicalPosition::new(100, 50));
        watch(&ctx, dialog);
        assert!(!compositor_handles_move(&ctx, Some(dialog), &moved));
        ctx.data_mut(|data| data.insert_temp(key(dialog), Status::Presented(Weak::new())));
        assert!(compositor_handles_move(&ctx, Some(dialog), &moved));
        assert!(!compositor_handles_move(
            &ctx,
            Some(ViewportId::ROOT),
            &moved
        ));
        assert!(!compositor_handles_move(
            &ctx,
            Some(dialog),
            &WindowEvent::Resized(PhysicalSize::new(420, 320))
        ));
        forget(&ctx, dialog);
        assert!(!compositor_handles_move(&ctx, Some(dialog), &moved));
    }

    #[test]
    fn child_failure_is_scoped_and_quarantined() {
        let ctx = Context::default();
        let first = ViewportId::from_hash_of(1);
        let second = ViewportId::from_hash_of(2);
        watch(&ctx, first);
        watch(&ctx, second);
        assert!(!failed(&ctx, ViewportId::ROOT, &"root failure"));
        assert!(failed(&ctx, first, &"child failure"));
        assert!(is_failed(&ctx, first));
        assert!(matches!(status(&ctx, second), Some(Status::Opening)));
        forget(&ctx, first);
        assert!(!is_failed(&ctx, first));
    }
}

/// Stages available to the desktop debug smoke test.
#[cfg(debug_assertions)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StartupFault {
    /// Fail before creating the platform window.
    Window,
    /// Fail on the first redraw before delivering Settings input.
    Redraw,
}

/// Queue a one-shot startup failure in a debug integration test.
#[cfg(debug_assertions)]
pub fn inject_fault(ctx: &Context, id: ViewportId, fault: StartupFault) {
    ctx.data_mut(|data| data.insert_temp(key(id).with("fault"), fault));
}

#[cfg(debug_assertions)]
pub(crate) fn take_fault(ctx: &Context, id: ViewportId, fault: StartupFault) -> bool {
    ctx.data_mut(|data| {
        let key = key(id).with("fault");
        let armed = data.get_temp::<StartupFault>(key) == Some(fault);
        if armed {
            data.remove::<StartupFault>(key);
        }
        armed
    })
}
