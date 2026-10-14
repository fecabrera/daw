use egui::{Context, LayerId, Rect, Ui};

/// Selection follows its pane's focus, including controls inside a scrolled lane.
#[derive(Default)]
pub(super) struct SelectionFocus {
    bounds: Option<Rect>,
    layer: Option<LayerId>,
    horizontal_scroll: bool,
}

impl SelectionFocus {
    pub fn set(&mut self, ui: &Ui, bounds: Rect, horizontal_scroll: bool) {
        self.bounds = Some(bounds);
        self.layer = Some(ui.layer_id());
        self.horizontal_scroll = horizontal_scroll;
    }

    pub fn owns(&self, ctx: &Context) -> bool {
        ctx.memory(|memory| memory.focused())
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|response| {
                self.layer == Some(response.layer_id)
                    && self.bounds.is_some_and(|bounds| {
                        let center = response.rect.center();
                        if self.horizontal_scroll {
                            bounds.y_range().contains(center.y)
                        } else {
                            bounds.contains(center)
                        }
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
