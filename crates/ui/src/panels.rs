use egui::{Color32, Frame, Rect, Response, Stroke, Ui, Vec2};

pub(super) use daw_unit_ui::layout::UNIT_STRIP_WIDTH;
pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) use daw_unit_ui::layout::HEADER_FONT_SIZE;
pub(super) const UNIT_POWER_SIZE: f32 = 10.0;
/// A neighboring panel supplies this unit's left divider.
pub(super) const SHARED_LEFT_BORDER_TAG: &str = "shared-unit-left-border";
/// The adjoining lane or status panel owns this unit's bottom separator.
pub(super) const SHARED_BOTTOM_BORDER_TAG: &str = "shared-unit-bottom-border";
/// The pinned row owns the boundary above its components.
pub(super) const SHARED_TOP_BORDER_TAG: &str = "shared-unit-top-border";

/// Three shared knob rows and a footer, before the unit frame and header.
pub(super) use daw_unit_ui::layout::unit_control_height;

pub(super) fn unit_height(compact: bool) -> f32 {
    if compact {
        crate::ROW_HEIGHT
    } else {
        unit_control_height() + 2.0 * f32::from(PanelStyle::default().margin) + HEADER_HEIGHT + 2.0
    }
}

/// Standard internally padded control strip, shared by repeated sources and EQ bands.
pub(super) use daw_unit_ui::layout::unit_strip;

/// One shared gap between full-size control columns and at either outer edge.
#[cfg(test)]
pub(super) use daw_unit_ui::layout::unit_column_gap;

#[cfg(test)]
pub(super) use daw_unit_ui::layout::unit_columns_width;

/// Full-height add strip: one shared circular button plus standard panel padding.
pub(super) fn unit_add_strip(
    ui: &mut Ui,
    height: f32,
    name: &str,
) -> egui::InnerResponse<Response> {
    let margin = f32::from(PanelStyle::default().margin);
    ui.scope(|ui| {
        crate::icons::circular_strip_button(
            ui,
            crate::icons::Lucide::Plus,
            name,
            Vec2::new(crate::icons::COMPACT_BUTTON_SIZE, height) + Vec2::splat(2.0 * margin),
            crate::icons::COMPACT_BUTTON_SIZE,
        )
    })
}

/// Shared muted full-height separator for internally padded unit strips.
pub(super) use daw_unit_ui::layout::unit_divider;

/// Base control column shared by Tracks, Layers, and the Effects catalog. Row content stays with each view.
/// Paint the border last so selected rows, lanes, and scrolling cannot cover it.
pub(super) struct ControlPanel {
    rect: Rect,
}

enum CatalogIcon {
    None,
    Folder(bool),
    Track(crate::icons::Lucide),
    Group(bool),
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

    /// Catalog entries share dropdown spacing, small text, and the view controls' selected fill.
    pub(super) fn catalog_item(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        indented: bool,
    ) -> Response {
        self.catalog_row(
            ui,
            name,
            selected,
            usize::from(indented) * 2,
            CatalogIcon::None,
        )
    }

    pub(super) fn catalog_folder(&self, ui: &mut Ui, name: &str, open: bool) -> Response {
        self.catalog_row(ui, name, false, 0, CatalogIcon::Folder(open))
            .on_hover_text(format!(
                "{} {name}",
                if open { "Collapse" } else { "Expand" }
            ))
    }

    /// Project navigation shares catalog painting while retaining dynamic IDs/names.
    pub(super) fn project_track(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        depth: usize,
        leading: (crate::icons::Lucide, Option<bool>),
    ) -> Response {
        self.catalog_row(
            ui,
            name,
            selected,
            depth,
            leading
                .1
                .map_or(CatalogIcon::Track(leading.0), CatalogIcon::Group),
        )
    }

    /// Catalog lists share dropdown margins, item padding and row gaps.
    pub(super) fn catalog_body<R>(&self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> R {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = crate::menus::ITEM_SPACING;
            Frame::new()
                .inner_margin(crate::menus::OUTER_MARGIN)
                .show(ui, |ui| {
                    ui.set_width(
                        (self.rect.width() - 1.0 - 2.0 * f32::from(crate::menus::OUTER_MARGIN))
                            .max(0.0),
                    );
                    content(ui)
                })
                .inner
        })
        .inner
    }

    /// Inset form contents from the panel edges. Form rows retain their own height;
    /// catalog lists use `catalog_body` and the shared menu spacing instead.
    pub(super) fn padded_body<R>(
        &self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        ui.scope(|ui| {
            let padding = crate::theme::TOOLBAR_PADDING;
            ui.set_width((self.rect.width() - 1.0).max(0.0));
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            Frame::new()
                .inner_margin(egui::Margin::symmetric(padding, 0))
                .show(ui, |ui| {
                    ui.set_width((self.rect.width() - 1.0 - 2.0 * f32::from(padding)).max(0.0));
                    ui.spacing_mut().item_spacing.x = f32::from(padding);
                    content(ui)
                })
                .inner
        })
    }

    pub(super) fn content_row_height(ui: &Ui) -> f32 {
        // Use pixel-rounded galley height so form fields retain stable baselines
        // at fractional display scales.
        ui.painter()
            .layout_no_wrap(
                "Ag".into(),
                egui::TextStyle::Small.resolve(ui.style()),
                crate::theme::palette(ui.ctx()).text,
            )
            .size()
            .y
            + 2.0 * f32::from(crate::theme::TOOLBAR_PADDING)
    }

    /// Form controls share one row height and centered text baseline.
    pub(super) fn field_row<R>(
        &self,
        ui: &mut Ui,
        label_width: f32,
        label: impl FnOnce(&mut Ui),
        control: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        crate::rows::field_with_height(
            ui,
            label_width,
            Self::content_row_height(ui),
            label,
            control,
        )
    }

    fn catalog_row(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        depth: usize,
        leading: CatalogIcon,
    ) -> Response {
        let folder = match leading {
            CatalogIcon::Folder(open) | CatalogIcon::Group(open) => Some(open),
            _ => None,
        };
        let padding = f32::from(crate::menus::ITEM_PADDING);
        let palette = crate::theme::palette(ui.ctx());
        let galley = ui.painter().layout_no_wrap(
            name.into(),
            egui::TextStyle::Small.resolve(ui.style()),
            palette.text,
        );
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                galley.size().y.max(crate::icons::TOOLBAR_ICON_SIZE) + 2.0 * padding,
            ),
            if !matches!(leading, CatalogIcon::None) {
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
        let step = icon_size + padding;
        let indent = depth as f32 * step;
        let icon_rect = |index: usize| {
            Rect::from_center_size(
                egui::pos2(
                    rect.left() + padding + indent + icon_size * 0.5 + index as f32 * step,
                    rect.center().y,
                ),
                Vec2::splat(icon_size),
            )
        };
        let slots = match leading {
            CatalogIcon::None => 0.,
            CatalogIcon::Track(icon) => {
                icon.size(icon_size)
                    .color(color)
                    .image()
                    .paint_at(ui, icon_rect(0));
                1.
            }
            CatalogIcon::Group(open) => {
                crate::icons::paint_disclosure(ui, icon_rect(0), open, color);
                1.
            }
            CatalogIcon::Folder(open) => {
                crate::icons::paint_disclosure(ui, icon_rect(0), open, color);
                let icon = if open {
                    crate::icons::Lucide::FolderOpen
                } else {
                    crate::icons::Lucide::Folder
                };
                icon.size(icon_size)
                    .color(color)
                    .image()
                    .paint_at(ui, icon_rect(1));
                2.
            }
        };
        let text_offset = indent + slots * step;
        ui.painter().galley_with_override_text_color(
            egui::pos2(
                rect.left() + padding + text_offset,
                rect.center().y - galley.size().y * 0.5,
            ),
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

    /// Nested rows share their right edge with the control column. Only their
    /// inset left edge needs a separate divider, including a group's child area.
    pub(super) fn row_left_divider(ui: &Ui, row: Rect, clip: Rect) {
        ui.painter()
            .with_clip_rect(clip.intersect(ui.clip_rect()))
            .vline(
                row.left() + 0.5,
                row.y_range(),
                Stroke::new(1.0, crate::theme::palette(ui.ctx()).border),
            );
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
        self.finish_edge(ui, self.rect.right() - 0.5);
    }

    pub(super) fn finish_left(self, ui: &Ui) {
        self.finish_edge(ui, self.rect.left() + 0.5);
    }

    fn finish_edge(&self, ui: &Ui, x: f32) {
        ui.painter().vline(
            x,
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
            margin: daw_unit_ui::layout::PANEL_MARGIN,
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
        let left_border = !ui
            .stack()
            .iter()
            .any(|node| node.tags().contains(SHARED_LEFT_BORDER_TAG));
        let (enabled, collapsed) = states;
        let (name, compact_title) = title;
        let id = ui.id().with(("unit-header", name));
        let is_collapsed = *collapsed;
        let mut header_rect = Rect::NOTHING;
        let mut frame = self.frame(ui.ctx(), false).inner_margin(0);
        let outline = frame.stroke;
        if !bottom_border || !top_border || !left_border {
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
        if !bottom_border || !top_border || !left_border {
            let rect = result.response.rect.shrink(0.5);
            if top_border {
                ui.painter().hline(rect.x_range(), rect.top(), outline);
            }
            if bottom_border {
                ui.painter().hline(rect.x_range(), rect.bottom(), outline);
            }
            if left_border {
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

#[cfg(test)]
mod catalog_style_tests {
    use super::*;
    #[test]
    fn catalog_rows_share_menu_padding_margins_gaps_and_inset_selection() {
        for color in [
            crate::settings::ColorTheme::Dark,
            crate::settings::ColorTheme::Light,
        ] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            crate::icons::configure(&ctx);
            crate::theme::configure(&ctx);
            crate::theme::apply(
                &ctx,
                crate::settings::AppSettings {
                    theme: color,
                    ..Default::default()
                },
            );
            let mut rows = Vec::new();
            let mut origin = egui::Pos2::ZERO;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(600.0, 500.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    origin = ui.cursor().min;
                    let panel = ControlPanel::begin(
                        ui,
                        Rect::from_min_size(origin, Vec2::new(250.0, 500.0)),
                    );
                    panel.catalog_body(ui, |ui| {
                        rows.push(panel.catalog_folder(ui, "Folder", true).rect);
                        rows.push(panel.catalog_item(ui, "Selected", true, true).rect);
                        rows.push(panel.catalog_item(ui, "Other", false, true).rect);
                    });
                },
            );
            assert_eq!(rows[0].left() - origin.x, 4.0);
            assert_eq!(rows[0].top() - origin.y, 4.0);
            assert_eq!(origin.x + 249.0 - rows[0].right(), 4.0);
            for pair in rows.windows(2) {
                assert_eq!(pair[1].top() - pair[0].bottom(), 4.0);
                assert_eq!(pair[0].width(), pair[1].width());
            }
            assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rows[1] && r.fill == crate::theme::toggle_colors(&ctx, true).0 && r.corner_radius.nw > 0)));
            let label = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Folder" => Some(t),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                label.pos.y + label.galley.size().y * 0.5,
                rows[0].center().y
            );
            assert_eq!(
                label.pos.x - rows[0].left(),
                4.0 + 2.0 * (crate::icons::TOOLBAR_ICON_SIZE + 4.0)
            );
            output.textures_delta.clear();
        }
    }
}
