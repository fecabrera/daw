use egui::{Color32, Frame, Rect, Response, Stroke, Ui, Vec2};

pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const HEADER_FONT_SIZE: f32 = 11.0;
pub(super) const UNIT_POWER_SIZE: f32 = 10.0;
/// A neighboring panel supplies this unit's left divider.
pub(super) const SHARED_LEFT_BORDER_TAG: &str = "shared-unit-left-border";
/// The adjoining lane or status panel owns this unit's bottom separator.
pub(super) const SHARED_BOTTOM_BORDER_TAG: &str = "shared-unit-bottom-border";
/// The pinned row owns the boundary above its components.
pub(super) const SHARED_TOP_BORDER_TAG: &str = "shared-unit-top-border";

/// Three shared knob rows and a footer, before the unit frame and header.
pub(super) fn unit_control_height() -> f32 {
    3.0 * crate::knobs::CHOICE_SIZE
        + 3.0 * f32::from(crate::theme::TOOLBAR_PADDING)
        + crate::icons::COMPACT_BUTTON_SIZE
}

pub(super) fn unit_height(compact: bool) -> f32 {
    if compact {
        crate::ROW_HEIGHT
    } else {
        unit_control_height() + 2.0 * f32::from(PanelStyle::default().margin) + HEADER_HEIGHT + 2.0
    }
}

/// Shared muted full-height separator for internally padded unit strips.
pub(super) fn unit_divider(ui: &Ui, x: f32, range: std::ops::RangeInclusive<f32>) {
    ui.painter().vline(
        x,
        range,
        Stroke::new(1.0, crate::theme::unit_separator(ui.ctx())),
    );
}

/// Base control column shared by Tracks, Layers, and the Effects catalog. Row content stays with each view.
/// Paint the border last so selected rows, lanes, and scrolling cannot cover it.
pub(super) struct ControlPanel {
    rect: Rect,
}

impl ControlPanel {
    pub(super) const WIDTH: f32 = 250.0;

    pub(super) fn begin(ui: &Ui, bounds: Rect) -> Self {
        let rect = Rect::from_min_max(
            bounds.min,
            egui::pos2(bounds.left() + Self::WIDTH, bounds.bottom()),
        );
        ui.painter()
            .rect_filled(rect, 0.0, crate::theme::palette(ui.ctx()).panel);
        Self { rect }
    }

    pub(super) fn header<R>(
        &self,
        ui: &mut Ui,
        title: &str,
        actions: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        ui.allocate_ui_with_layout(
            Vec2::new(self.rect.width(), crate::toolbars::HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_width(self.rect.width());
                crate::toolbars::Toolbar::row(ui, |ui| {
                    ui.label(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions)
                        .inner
                })
            },
        )
        .inner
    }

    /// Catalog entries share padding, small text, and the view controls' selected fill.
    pub(super) fn catalog_item(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        indented: bool,
    ) -> Response {
        self.catalog_row(ui, name, selected, indented, None)
    }

    pub(super) fn catalog_folder(&self, ui: &mut Ui, name: &str, open: bool) -> Response {
        self.catalog_row(ui, name, false, false, Some(open))
            .on_hover_text(format!(
                "{} {name}",
                if open { "Collapse" } else { "Expand" }
            ))
    }

    fn catalog_row(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        indented: bool,
        folder: Option<bool>,
    ) -> Response {
        let padding = f32::from(crate::theme::TOOLBAR_PADDING);
        let palette = crate::theme::palette(ui.ctx());
        let galley = ui.painter().layout_no_wrap(
            name.into(),
            egui::TextStyle::Small.resolve(ui.style()),
            palette.text,
        );
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(self.rect.width() - 1.0, galley.size().y + 2.0 * padding),
            if folder.is_some() {
                egui::Sense::click()
            } else {
                egui::Sense::click_and_drag()
            },
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                if folder.is_some() {
                    egui::WidgetType::CollapsingHeader
                } else {
                    egui::WidgetType::Button
                },
                ui.is_enabled(),
                folder.unwrap_or(selected),
                name,
            )
        });
        let color = if selected || response.hovered() || response.has_focus() {
            palette.text
        } else {
            palette.secondary
        };
        if selected {
            ui.painter().rect_filled(
                rect,
                ui.visuals().widgets.inactive.corner_radius,
                crate::theme::toggle_colors(ui.ctx(), true).0,
            );
        }
        let icon_size = crate::icons::TOOLBAR_ICON_SIZE;
        let indent = if indented {
            2.0 * (icon_size + padding)
        } else {
            0.0
        };
        let text_offset = folder.map_or(indent, |_| 2.0 * (icon_size + padding));
        if let Some(open) = folder {
            let icons = [
                if open {
                    crate::icons::Lucide::ChevronDown
                } else {
                    crate::icons::Lucide::ChevronRight
                },
                if open {
                    crate::icons::Lucide::FolderOpen
                } else {
                    crate::icons::Lucide::Folder
                },
            ];
            for (index, icon) in icons.into_iter().enumerate() {
                icon.size(icon_size).color(color).image().paint_at(
                    ui,
                    Rect::from_center_size(
                        egui::pos2(
                            rect.left()
                                + padding
                                + icon_size * 0.5
                                + index as f32 * (icon_size + padding),
                            rect.center().y,
                        ),
                        Vec2::splat(icon_size),
                    ),
                );
            }
        }
        ui.painter().galley_with_override_text_color(
            rect.min + Vec2::new(padding + text_offset, padding),
            galley,
            color,
        );
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// One row boundary can span both the control column and its workspace lane.
    pub(super) fn row_dividers(ui: &Ui, row: Rect, top: bool, bottom: bool) {
        Self::paint_row_dividers(ui.painter(), row, top, bottom);
    }

    /// Reuse row geometry when a clipped preview mask restores the lane background.
    pub(super) fn paint_row_dividers(painter: &egui::Painter, row: Rect, top: bool, bottom: bool) {
        let stroke = Stroke::new(1.0, crate::theme::palette(painter.ctx()).border);
        if top {
            painter.hline(row.x_range(), row.top() + 0.5, stroke);
        }
        if bottom {
            painter.hline(row.x_range(), row.bottom() - 0.5, stroke);
        }
    }

    /// A fixed row shares its top separator with the preceding scroll region.
    pub(super) fn pinned_row_divider(ui: &Ui, row: Rect) {
        // Share the preceding row's bottom pixel rather than drawing the next pixel too.
        Self::paint_row_dividers(
            ui.painter(),
            row.translate(Vec2::new(0.0, -1.0)),
            true,
            false,
        );
    }

    /// Paint after neutral dividers, without an inset or a replacement edge at the clip limit.
    pub(super) fn selected_row(ui: &Ui, row: Rect, clip: Rect) {
        paint_selection_border(
            &ui.painter().with_clip_rect(clip.intersect(ui.clip_rect())),
            row,
        );
    }

    pub(super) fn finish(self, ui: &Ui) {
        ui.painter().vline(
            self.rect.right() - 0.5,
            self.rect.y_range(),
            Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
        );
    }
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
    /// Callers supply compact title mode and project-owned enable/collapse states.
    pub fn unit<R>(
        &self,
        ui: &mut Ui,
        title: (&str, bool),
        height: f32,
        enabled: &mut bool,
        collapsed: &mut bool,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        self.unit_shell(ui, title, height, (enabled, collapsed), true, content)
    }

    /// Units pinned above a lane separator share that separator instead of adding a bottom edge.
    pub fn unit_in_lane<R>(
        &self,
        ui: &mut Ui,
        title: (&str, bool),
        height: f32,
        enabled: &mut bool,
        collapsed: &mut bool,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        self.unit_shell(ui, title, height, (enabled, collapsed), false, content)
    }

    fn unit_shell<R>(
        &self,
        ui: &mut Ui,
        title: (&str, bool),
        height: f32,
        states: (&mut bool, &mut bool),
        bottom_border: bool,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        let bottom_border = bottom_border
            && !ui
                .stack()
                .iter()
                .any(|node| node.tags().contains(SHARED_BOTTOM_BORDER_TAG));
        let top_border = !ui
            .stack()
            .iter()
            .any(|node| node.tags().contains(SHARED_TOP_BORDER_TAG));
        let (enabled, collapsed) = states;
        let (name, compact_title) = title;
        let id = ui.id().with(("unit-header", name));
        let is_collapsed = *collapsed;
        let mut header_rect = Rect::NOTHING;
        let mut frame = self.frame(ui.ctx(), false).inner_margin(0);
        let outline = frame.stroke;
        if !bottom_border || !top_border {
            frame.stroke.color = Color32::TRANSPARENT;
        }
        let result = if is_collapsed {
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
        let reserved = if is_collapsed {
            0.0
        } else {
            f32::from(
                ui.stack()
                    .iter()
                    .any(|n| n.tags().contains("unit-note-selector")),
            ) * 90.0
                + f32::from(
                    ui.stack()
                        .iter()
                        .any(|n| n.tags().contains("unit-modulation-button")),
                ) * 26.0
        };
        let mut interaction_rect = header_rect;
        interaction_rect.max.x -= reserved;
        let mut header = ui.interact(interaction_rect, id, egui::Sense::click_and_drag());
        header.rect = header_rect;
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
        let mut cap_fill = cap;
        if !top_border {
            // The frame still reserves its border geometry. Cover that omitted edge
            // without shifting the header controls or changing the shared unit height.
            cap_fill.min.y = result.response.rect.top();
        }
        ui.painter().rect_filled(cap_fill, 0.0, fill);
        let header_text = if *enabled {
            crate::theme::unit_header_text(fill)
        } else {
            crate::theme::palette(ui.ctx()).secondary
        };
        if is_collapsed {
            let body = Rect::from_min_max(
                egui::pos2(header_rect.left(), cap.bottom()),
                header_rect.max,
            );
            let text = if *enabled {
                crate::theme::palette(ui.ctx()).text
            } else {
                header_text
            };
            let strip_title = if compact_title {
                format!("{}...", name.chars().take(3).collect::<String>())
            } else {
                name.to_owned()
            };
            paint_unit_title(ui, body, &strip_title, text, true);
        } else {
            let title_rect = if reserved >= 90.0 && cap.width() < 260.0 {
                Rect::from_min_max(
                    egui::pos2(power_rect.right() + 7.0, cap.top()),
                    egui::pos2(cap.right() - reserved, cap.bottom()),
                )
            } else {
                cap
            };
            paint_unit_title(ui, title_rect, name, header_text, false);
        }
        let color = if *enabled {
            crate::theme::accent(ui.ctx())
        } else {
            header_text
        };
        crate::icons::paint_dot_toggle(ui, &power, UNIT_POWER_SIZE, *enabled, color);
        let action = if is_collapsed { "Expand" } else { "Collapse" };
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
            *collapsed = !is_collapsed;
            ui.ctx().request_repaint();
        }
        if !bottom_border || !top_border {
            let rect = result.response.rect.shrink(0.5);
            if top_border {
                ui.painter().hline(rect.x_range(), rect.top(), outline);
            }
            if bottom_border {
                ui.painter().hline(rect.x_range(), rect.bottom(), outline);
            }
            if !ui
                .stack()
                .iter()
                .any(|node| node.tags().contains(SHARED_LEFT_BORDER_TAG))
            {
                ui.painter().vline(rect.left(), rect.y_range(), outline);
            }
            ui.painter().vline(rect.right(), rect.y_range(), outline);
        }
        UnitResponse {
            inner: result.inner,
            response: result.response,
            header: header.on_hover_text(if bottom_border {
                format!("Double-click to {} {name}", action.to_lowercase())
            } else {
                format!(
                    "Drag to move {name}; double-click to {}",
                    action.to_lowercase()
                )
            }),
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

/// Shared selected outline for track rows and effect units.
pub(super) fn selection_border(ui: &Ui, rect: Rect) {
    paint_selection_border(ui.painter(), rect);
}
fn paint_selection_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        2.0,
        crate::theme::selection_outline(painter.ctx()),
        egui::StrokeKind::Inside,
    );
}
