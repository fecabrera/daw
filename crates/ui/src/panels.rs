use egui::{Color32, Frame, Rect, Response, Stroke, Ui, Vec2};

pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const HEADER_FONT_SIZE: f32 = 11.0;
pub(super) const UNIT_POWER_SIZE: f32 = 10.0;

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
        enabled: &mut bool,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        let id = ui.id().with(("unit-header", name));
        let collapsed = ui
            .ctx()
            .data(|data| data.get_temp::<bool>(id).unwrap_or(false));
        let mut header_rect = Rect::NOTHING;
        let frame = self.frame(ui.ctx(), false).inner_margin(0);
        let result = if collapsed {
            frame.show(ui, |ui| {
                let (strip, _) = ui.allocate_exact_size(
                    Vec2::new(HEADER_HEIGHT, height - 2.0),
                    egui::Sense::hover(),
                );
                // The full strip remains the expansion/context-menu target.
                header_rect = strip;
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
                    Some(body.inner)
                })
                .inner
            })
        };
        let header = ui.interact(header_rect, id, egui::Sense::click());
        let power_rect = Rect::from_min_size(
            header_rect.min + Vec2::splat(1.0),
            Vec2::splat(HEADER_HEIGHT - 2.0),
        );
        let power = ui.interact(power_rect, id.with("power"), egui::Sense::click());
        power.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                *enabled,
                format!("{name} enabled"),
            )
        });
        if ui.is_enabled() && ui.input(|input| input.focused) && power.clicked() {
            *enabled = !*enabled;
        }
        let fill = crate::theme::unit_header_fill(ui.ctx(), *enabled);
        let cap = Rect::from_min_size(
            header_rect.min,
            Vec2::new(header_rect.width(), HEADER_HEIGHT),
        );
        ui.painter().rect_filled(cap, 0.0, fill);
        let header_text = if *enabled {
            crate::theme::unit_header_text(fill)
        } else {
            crate::theme::palette(ui.ctx()).secondary
        };
        if collapsed {
            let body = Rect::from_min_max(
                egui::pos2(header_rect.left(), cap.bottom()),
                header_rect.max,
            );
            let text = if *enabled {
                crate::theme::palette(ui.ctx()).text
            } else {
                header_text
            };
            paint_unit_title(ui, body, name, text, true);
        } else {
            paint_unit_title(ui, cap, name, header_text, false);
        }
        let color = if *enabled {
            crate::theme::accent(ui.ctx())
        } else {
            header_text
        };
        let color = if ui.is_enabled() {
            color
        } else {
            color.gamma_multiply(0.5)
        };
        if power.hovered() || power.has_focus() {
            ui.painter().circle_filled(
                power_rect.center(),
                UNIT_POWER_SIZE * 0.5 + 3.0,
                color.gamma_multiply(0.15),
            );
        }
        ui.painter().circle(
            power_rect.center(),
            (UNIT_POWER_SIZE - 1.0) * 0.5,
            if *enabled {
                color
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, color),
        );
        let action = if collapsed { "Expand" } else { "Collapse" };
        header.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("{action} {name}"),
            )
        });
        if ui.is_enabled()
            && ui.input(|input| input.focused)
            && header.double_clicked()
            && !header
                .interact_pointer_pos()
                .is_some_and(|pos| power_rect.contains(pos))
        {
            ui.ctx().data_mut(|data| data.insert_temp(id, !collapsed));
            ui.ctx().request_repaint();
        }
        UnitResponse {
            inner: result.inner,
            response: result.response,
            header: header
                .on_hover_text(format!("Double-click to {} {name}", action.to_lowercase())),
            power: power.on_hover_text(format!(
                "{} {name}",
                if *enabled { "Turn off" } else { "Turn on" }
            )),
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
    pub power: Response,
}

fn paint_unit_title(ui: &Ui, rect: Rect, name: &str, color: Color32, vertical: bool) {
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
