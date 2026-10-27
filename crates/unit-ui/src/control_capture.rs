//! Scoped UI response collection for format-neutral editor bindings.
//! This is used only while painting controls, never by audio processing.
use egui::{Rect, Response, Ui};
use std::sync::{Arc, Mutex};
const TAG: &str = "unit-control-responses";
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Response>>>);
// Drain even during unwinding: stored Responses own a Context and must not keep
// a Context -> UI stack tag -> Capture -> Response -> Context cycle alive.
struct ScopeGuard(Capture);
impl Drop for ScopeGuard {
    fn drop(&mut self) {
        self.0.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}
pub fn scope<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> (R, Vec<Response>) {
    let capture = Capture::default();
    let _guard = ScopeGuard(capture.clone());
    let result = ui
        .scope_builder(
            egui::UiBuilder::new()
                .ui_stack_info(egui::UiStackInfo::default().with_tag_value(TAG, capture.clone())),
            content,
        )
        .inner;
    let responses = std::mem::take(&mut *capture.0.lock().unwrap_or_else(|e| e.into_inner()));
    (result, responses)
}
pub(crate) fn record(ui: &Ui, response: &Response) {
    if let Some(capture) = ui
        .stack()
        .iter()
        .find_map(|s| s.tags().get_downcast::<Capture>(TAG))
    {
        capture
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(response.clone());
    }
}
pub fn at(responses: &[Response], rect: Rect) -> Option<Response> {
    responses.iter().find(|r| r.rect == rect).cloned()
}
