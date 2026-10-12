use egui::{Color32, Frame, Rect, Response, Stroke, Ui, Vec2};

pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const HEADER_FONT_SIZE: f32 = 11.0;

/// Shared muted full-height separator for internally padded unit strips.
pub(super) fn unit_divider(ui: &Ui, x: f32, range: std::ops::RangeInclusive<f32>) {
    ui.painter().vline(
        x,
        range,
        Stroke::new(1.0, crate::theme::unit_separator(ui.ctx())),
    );
}

/// Shared framing for outlined information panels. Content and commands stay with each panel.
pub struct PanelStyle {
    pub outline: Option<Color32>,
    pub margin: i8,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            outline: None,
            margin: 10,
        }
    }
}

impl PanelStyle {
    /// Base panel for every source, effect, and modulator; callers supply names and controls.
    /// Collapse is temporary UI state keyed by the enclosing track/layer scope and unit name.
    pub fn unit<R>(
        &self,
        ui: &mut Ui,
        name: &str,
        height: f32,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        let id = ui.id().with(("unit-header", name));
        let collapsed = ui
            .ctx()
            .data(|data| data.get_temp::<bool>(id).unwrap_or(false));
        let accent = crate::theme::accent(ui.ctx());
        let mut header_rect = Rect::NOTHING;
        let frame = self.frame(ui.ctx(), false).inner_margin(0);
        let result = if collapsed {
            frame.fill(accent).show(ui, |ui| {
                let (strip, _) = ui.allocate_exact_size(
                    Vec2::new(HEADER_HEIGHT, height - 2.0),
                    egui::Sense::hover(),
                );
                header_rect = strip;
                paint_unit_title(ui, strip, name, accent, true);
                None
            })
        } else {
            frame.show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let spacing = ui.spacing().item_spacing.y;
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let (header, _) =
                        ui.allocate_exact_size(Vec2::new(0.0, HEADER_HEIGHT), egui::Sense::hover());
                    let body = Frame::new().inner_margin(self.margin).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = spacing;
                        content(ui)
                    });
                    header_rect = Rect::from_min_size(
                        header.min,
                        Vec2::new(body.response.rect.width(), HEADER_HEIGHT),
                    );
                    ui.painter().rect_filled(header_rect, 0.0, accent);
                    paint_unit_title(ui, header_rect, name, accent, false);
                    Some(body.inner)
                })
                .inner
            })
        };
        let header = ui.interact(header_rect, id, egui::Sense::click());
        let action = if collapsed { "Expand" } else { "Collapse" };
        header.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("{action} {name}"),
            )
        });
        if ui.is_enabled() && ui.input(|input| input.focused) && header.double_clicked() {
            ui.ctx().data_mut(|data| data.insert_temp(id, !collapsed));
            ui.ctx().request_repaint();
        }
        UnitResponse {
            inner: result.inner,
            response: result.response,
            header: header
                .on_hover_text(format!("Double-click to {} {name}", action.to_lowercase())),
        }
    }

    /// Track rows share their horizontal borders instead of stacking two strokes.
    pub fn track_frame(&self, ctx: &egui::Context) -> Frame {
        Frame::new()
            .fill(crate::theme::palette(ctx).panel)
            .inner_margin(self.margin + 1)
    }

    pub fn frame(&self, ctx: &egui::Context, selected: bool) -> Frame {
        Frame::new()
            .fill(crate::theme::palette(ctx).panel)
            .stroke(Stroke::new(
                1.0_f32,
                if selected {
                    crate::theme::accent(ctx)
                } else {
                    self.outline.unwrap_or(crate::theme::palette(ctx).border)
                },
            ))
            .inner_margin(self.margin)
    }
}

/// The header remains available for unit context menus in either presentation.
pub struct UnitResponse<R> {
    pub inner: Option<R>,
    pub response: Response,
    pub header: Response,
}

fn paint_unit_title(ui: &Ui, rect: Rect, name: &str, accent: Color32, vertical: bool) {
    let color = crate::theme::unit_header_text(accent);
    let galley = ui.painter().layout_no_wrap(
        name.to_owned(),
        egui::FontId::proportional(HEADER_FONT_SIZE),
        color,
    );
    let pos = rect.center() - galley.size() * 0.5;
    let text = egui::epaint::TextShape::new(pos, galley, color).with_angle_and_anchor(
        if vertical {
            -std::f32::consts::FRAC_PI_2
        } else {
            0.0
        },
        egui::Align2::CENTER_CENTER,
    );
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .add(text);
}
