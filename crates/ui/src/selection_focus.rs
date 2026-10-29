use egui::{Context, LayerId, Rect, Ui};

/// Selection follows its pane's focus, including controls inside a scrolled lane.
#[derive(Default)]
pub(super) struct SelectionFocus {
    bounds: Option<Rect>,
    layer: Option<LayerId>,
    horizontal_scroll: bool,
    vertical_scroll: bool,
}

impl SelectionFocus {
    pub fn set(&mut self, ui: &Ui, bounds: Rect, horizontal_scroll: bool) {
        self.bounds = Some(bounds);
        self.layer = Some(ui.layer_id());
        self.horizontal_scroll = horizontal_scroll;
        self.vertical_scroll = false;
    }

    pub fn set_vertical_scroll(&mut self, ui: &Ui, bounds: Rect) {
        self.set(ui, bounds, false);
        self.vertical_scroll = true;
    }

    pub fn owns(&self, ctx: &Context) -> bool {
        ctx.memory(|memory| memory.focused())
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|response| {
                self.layer == Some(response.layer_id)
                    && self.bounds.is_some_and(|bounds| {
                        let center = response.rect.center();
                        (self.horizontal_scroll || bounds.x_range().contains(center.x))
                            && (self.vertical_scroll || bounds.y_range().contains(center.y))
                    })
            })
    }

    pub fn lost(&self, ctx: &Context, available: bool, dragging: bool) -> bool {
        let (focused, outside_press) = ctx.input(|input| {
            (
                input.focused,
                input.pointer.any_pressed()
                    && input.pointer.press_origin().is_some_and(|origin| {
                        !self.bounds.is_some_and(|bounds| bounds.contains(origin))
                    }),
            )
        });
        !focused || !available || (!dragging && (outside_press || !self.owns(ctx)))
    }
}

/// Keep egui's implicit arrow navigation from moving focus before a list handles it.
pub(super) fn retain_list_navigation(ui: &Ui, id: egui::Id, horizontal: bool) {
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                vertical_arrows: true,
                horizontal_arrows: horizontal,
                escape: true,
                ..Default::default()
            },
        )
    });
}
