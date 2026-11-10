use egui::{Color32, Frame, Rect, Response, Stroke, Ui, Vec2};

pub(super) use daw_unit_ui::layout::UNIT_STRIP_WIDTH;
pub(super) const HEADER_HEIGHT: f32 = crate::toolbars::CONTROL_HEIGHT + 2.0;
pub(super) const UNIT_LEFT_HEADER_WIDTH: f32 = 26.0;
pub(super) use daw_unit_ui::layout::HEADER_FONT_SIZE;
pub(super) const UNIT_POWER_SIZE: f32 = 10.0;
pub(super) const UNIT_GAP: f32 = 16.0;
// Borderless shells retain the lane height and center their existing strip slots.
pub(super) const UNIT_VERTICAL_MARGIN: f32 = HEADER_HEIGHT * 0.5 + 1.0;
const UNIT_CORNER_RADIUS: u8 = 7;
/// One scrollable unit row. Inline effects reuse their containing row's edge padding.
pub(super) fn unit_row<R>(
    ui: &mut Ui,
    padded: bool,
    content: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let margin = if padded { UNIT_GAP as i8 } else { 0 };
    Frame::new()
        .inner_margin(egui::Margin::symmetric(margin, 0))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = UNIT_GAP;
                content(ui)
            })
            .inner
        })
}

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
        unit_control_height()
            + 2.0 * f32::from(PanelStyle::default().margin)
            + 2.0 * UNIT_VERTICAL_MARGIN
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
    daw_unit_ui::layout::unit_add_strip(ui, height, name)
}

/// Shared geometry and separators for track groups and beat subtrack groups.
pub(super) struct GroupPanel;

impl GroupPanel {
    /// Eight-point nesting, capped at three levels to retain usable controls.
    pub(super) fn indent(depth: usize) -> f32 {
        depth.min(3) as f32 * 8.0
    }

    /// Inset the left edge while keeping every control column's right edge aligned.
    pub(super) fn bounds(row: Rect, width: f32, depth: usize) -> Rect {
        Rect::from_min_max(
            row.min + Vec2::new(Self::indent(depth), 0.0),
            egui::pos2(row.left() + width, row.bottom()),
        )
    }

    /// Color is paint-only; the header controls retain their existing geometry.
    pub(super) fn track_color_strip(ui: &Ui, panel: Rect, color: Color32) {
        ui.painter().rect_filled(
            Rect::from_min_max(
                panel.min,
                egui::pos2((panel.left() + 16.0).min(panel.right()), panel.bottom()),
            ),
            0.0,
            color,
        );
    }

    /// Group boundaries start at the row's depth; a header with visible children uses their
    /// depth for the bottom edge so the hierarchy gutter remains open.
    pub(super) fn row_dividers(ui: &Ui, row: Rect, depth: usize, top: bool, bottom: bool) {
        let row = Rect::from_min_max(row.min + Vec2::new(Self::indent(depth), 0.0), row.max);
        ControlPanel::row_dividers(ui, row, top, bottom);
    }
}

/// Base control column shared by Tracks, Layers, and the Effects catalog. Row content stays with each view.
/// Paint the neutral border after row fills and scrolling, then overlay selection outlines.
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
        Self::with_width(ui, bounds, Self::WIDTH)
    }

    pub(super) fn with_width(ui: &Ui, bounds: Rect, width: f32) -> Self {
        let rect = Rect::from_min_max(
            bounds.min,
            egui::pos2(bounds.left() + width, bounds.bottom()),
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
        trailing_width: f32,
    ) -> Response {
        self.catalog_row_with_trailing(
            ui,
            name,
            selected,
            depth,
            leading
                .1
                .map_or(CatalogIcon::Track(leading.0), CatalogIcon::Group),
            trailing_width,
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

    /// Empty inspectors share form padding, row height and muted small text.
    pub(super) fn placeholder(&self, ui: &mut Ui, text: &str) {
        self.padded_body(ui, |ui| {
            crate::rows::centered(ui, Self::content_row_height(ui), |ui| {
                ui.label(
                    egui::RichText::new(text)
                        .text_style(egui::TextStyle::Small)
                        .color(crate::theme::palette(ui.ctx()).secondary),
                )
            });
        });
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
        self.catalog_row_with_trailing(ui, name, selected, depth, leading, 0.0)
    }

    fn catalog_row_with_trailing(
        &self,
        ui: &mut Ui,
        name: &str,
        selected: bool,
        depth: usize,
        leading: CatalogIcon,
        trailing_width: f32,
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
        let (id, rect) = ui.allocate_space(Vec2::new(
            ui.available_width(),
            galley.size().y.max(crate::icons::TOOLBAR_ICON_SIZE) + 2.0 * padding,
        ));
        let hit = Rect::from_min_max(
            rect.min,
            egui::pos2(
                (rect.right() - trailing_width).max(rect.left()),
                rect.bottom(),
            ),
        );
        let mut response = ui.interact(
            hit,
            id,
            if trailing_width > 0.0 || matches!(leading, CatalogIcon::None) {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            },
        );
        // Layout/selection covers the full row; controls own the reserved hit area.
        response.rect = rect;
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
        let content_clip = ui.clip_rect();
        ui.set_clip_rect(hit.intersect(content_clip));
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
        ui.painter()
            .with_clip_rect(
                hit.shrink2(Vec2::new(padding, 0.0))
                    .intersect(ui.clip_rect()),
            )
            .galley_with_override_text_color(
                egui::pos2(
                    rect.left() + padding + text_offset,
                    rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                color,
            );
        ui.set_clip_rect(content_clip);
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
        self.unit_shell(ui, title, height, (enabled, collapsed), content)
    }

    /// Lane units use the same floating shell and preserve the lane's allocated height.
    pub fn unit_in_lane<R>(
        &self,
        ui: &mut Ui,
        title: (&str, bool),
        height: f32,
        enabled: &mut bool,
        collapsed: &mut bool,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        self.unit_shell(ui, title, height, (enabled, collapsed), content)
    }

    fn unit_shell<R>(
        &self,
        ui: &mut Ui,
        title: (&str, bool),
        height: f32,
        states: (&mut bool, &mut bool),
        content: impl FnOnce(&mut Ui) -> R,
    ) -> UnitResponse<R> {
        let (enabled, collapsed) = states;
        let (name, compact_title) = title;
        let id = ui.id().with(("unit-header", name));
        let is_collapsed = *collapsed;
        let body_height = (height - 2.0 * UNIT_VERTICAL_MARGIN).max(0.0);
        let frame = self
            .frame(ui.ctx(), false)
            .inner_margin(0)
            .fill(daw_unit_ui::theme::unit_separator(ui.ctx()))
            .stroke(Stroke::NONE)
            .corner_radius(UNIT_CORNER_RADIUS);
        // Reserve the previous lane height, with the entire unit centered in it.
        let spacing = ui.spacing().item_spacing;
        let result = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .ui_stack_info(
                        egui::UiStackInfo::default()
                            .with_tag(daw_unit_ui::meters::HIDE_OUTPUT_LEVELS_TAG),
                    ),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(UNIT_VERTICAL_MARGIN);
                    let result = frame.show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.allocate_exact_size(
                                Vec2::new(UNIT_LEFT_HEADER_WIDTH, body_height),
                                egui::Sense::hover(),
                            );
                            if is_collapsed {
                                None
                            } else {
                                // Add the other half-gap outside the end strip surfaces.
                                // Use logical-point spacing so odd gaps keep their half points.
                                let edge = daw_unit_ui::layout::UNIT_STRIP_GAP * 0.5;
                                ui.add_space(edge);
                                let body = Frame::new().inner_margin(self.margin).show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = spacing;
                                    content(ui)
                                });
                                ui.add_space(edge);
                                ui.allocate_exact_size(
                                    Vec2::new(HEADER_HEIGHT, body_height),
                                    egui::Sense::hover(),
                                );
                                Some(body.inner)
                            }
                        })
                        .inner
                    });
                    ui.add_space(UNIT_VERTICAL_MARGIN);
                    result
                },
            )
            .inner;
        let unit_rect = result.response.rect;
        let header_rect = if is_collapsed {
            unit_rect
        } else {
            Rect::from_min_max(
                unit_rect.min,
                egui::pos2(
                    unit_rect.left() + UNIT_LEFT_HEADER_WIDTH,
                    unit_rect.bottom(),
                ),
            )
        };
        let modulation_header = if is_collapsed {
            Rect::NOTHING
        } else {
            Rect::from_min_max(
                egui::pos2(unit_rect.right() - HEADER_HEIGHT, unit_rect.top()),
                unit_rect.max,
            )
        };
        #[cfg(test)]
        let frame_number = ui.ctx().cumulative_frame_nr();
        #[cfg(test)]
        ui.data_mut(|data| {
            let key = egui::Id::new("unit-test-geometry");
            let frame = frame_number;
            let (previous, mut entries) = data
                .get_temp::<(u64, Vec<(Rect, Rect, Rect)>)>(key)
                .unwrap_or_default();
            if frame != previous {
                entries.clear();
            }
            entries.push((header_rect, unit_rect, modulation_header));
            data.insert_temp(key, (frame, entries));
        });
        let interaction_rect = header_rect;
        let mut header = ui.interact(interaction_rect, id, egui::Sense::click_and_drag());
        header.rect = header_rect;
        let power_rect = Rect::from_min_size(
            header_rect.min + Vec2::new((UNIT_LEFT_HEADER_WIDTH - HEADER_HEIGHT + 2.0) * 0.5, 1.0),
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
        let fill = crate::theme::unit_header_fill(ui.ctx(), false);
        let left_radius = if is_collapsed {
            egui::CornerRadius::same(UNIT_CORNER_RADIUS)
        } else {
            egui::CornerRadius {
                nw: UNIT_CORNER_RADIUS,
                sw: UNIT_CORNER_RADIUS,
                ne: 0,
                se: 0,
            }
        };
        ui.painter().rect_filled(header_rect, left_radius, fill);
        if !is_collapsed {
            ui.painter().rect_filled(
                modulation_header,
                egui::CornerRadius {
                    ne: UNIT_CORNER_RADIUS,
                    se: UNIT_CORNER_RADIUS,
                    nw: 0,
                    sw: 0,
                },
                fill,
            );
        }
        let palette = crate::theme::palette(ui.ctx());
        let header_text = if crate::unit_selection::is_selected(ui) {
            palette.text
        } else {
            palette.secondary
        };
        let title_rect = Rect::from_min_max(
            header_rect.min + Vec2::new(1.0, HEADER_HEIGHT + 1.0),
            header_rect.max - Vec2::splat(1.0),
        );
        let strip_title = if compact_title {
            format!("{}...", name.chars().take(3).collect::<String>())
        } else {
            name.to_owned()
        };
        paint_unit_title(ui, title_rect, &strip_title, header_text, true);
        let color = if *enabled {
            crate::theme::accent(ui.ctx())
        } else {
            palette.secondary
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
        UnitResponse {
            inner: result.inner,
            modulation_header,
            response: result.response,
            header: header.on_hover_text(format!(
                "Drag to move {name}; double-click to {}",
                action.to_lowercase()
            )),
            power: power.on_hover_text(format!(
                "{} {name}",
                if *enabled { "Turn off" } else { "Turn on" }
            )),
        }
    }

    /// Track rows share their horizontal borders instead of stacking two strokes.
    pub fn track_frame(&self, ctx: &egui::Context, selected: bool) -> Frame {
        Frame::new()
            .fill(if selected {
                crate::theme::selected_track_header_background(ctx)
            } else {
                crate::theme::palette(ctx).panel
            })
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
    pub modulation_header: Rect,
}

fn paint_unit_title(ui: &Ui, rect: Rect, name: &str, color: Color32, vertical: bool) {
    let mut galley = ui.painter().layout_no_wrap(
        name.to_owned(),
        crate::fonts::semibold(HEADER_FONT_SIZE),
        color,
    );
    if vertical && galley.size().x > rect.height() - 2.0 {
        let size = HEADER_FONT_SIZE * (rect.height() - 2.0).max(1.0) / galley.size().x;
        galley = ui
            .painter()
            .layout_no_wrap(name.to_owned(), crate::fonts::semibold(size), color);
    }
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

#[cfg(test)]
mod unit_side_header_tests {
    use super::*;
    #[test]
    fn side_headers_center_whole_units_keep_gaps_and_round_outside_corners() {
        for light in [false, true] {
            for compact in [false, true] {
                for collapsed in [false, true] {
                    let ctx = egui::Context::default();
                    crate::theme::configure(&ctx);
                    crate::fonts::configure(&ctx);
                    if light {
                        ctx.set_theme(egui::Theme::Light);
                    }
                    let mut units = Vec::new();
                    let mut row = Rect::NOTHING;
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        row = unit_row(ui, true, |ui| {
                            for enabled in [false, true] {
                                let (mut on, mut closed) = (enabled, collapsed);
                                let mut slots = Vec::new();
                                let unit = ui
                                    .push_id(enabled, |ui| {
                                        PanelStyle {
                                            margin: 0,
                                            ..Default::default()
                                        }
                                        .unit(
                                            ui,
                                            ("Test unit", compact),
                                            unit_height(compact),
                                            &mut on,
                                            &mut closed,
                                            |ui| {
                                                ui.horizontal_top(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 0.0;
                                                    for _ in 0..2 {
                                                        slots.push(
                                                            unit_strip(
                                                                ui,
                                                                crate::envelope_panel::body_height(
                                                                    compact,
                                                                ),
                                                                egui::UiBuilder::new(),
                                                                |_| {},
                                                            )
                                                            .response
                                                            .rect,
                                                        );
                                                    }
                                                    slots.push(
                                                        unit_add_strip(
                                                            ui,
                                                            crate::envelope_panel::body_height(
                                                                compact,
                                                            ),
                                                            "Add test strip",
                                                        )
                                                        .response
                                                        .rect,
                                                    );
                                                });
                                            },
                                        )
                                    })
                                    .inner;
                                units.push((
                                    unit.response.rect,
                                    unit.header.rect,
                                    unit.modulation_header,
                                    slots,
                                ));
                            }
                        })
                        .response
                        .rect;
                    });
                    output.textures_delta.clear();
                    for (panel, left, right, slots) in &units {
                        assert_eq!(row.height(), unit_height(compact));
                        assert_eq!(panel.top() - row.top(), UNIT_VERTICAL_MARGIN);
                        assert_eq!(row.bottom() - panel.bottom(), UNIT_VERTICAL_MARGIN);
                        assert_eq!(left.left(), panel.left());
                        assert_eq!(left.height(), panel.height());
                        assert_eq!(left.width(), 26.0);
                        assert_eq!(right.is_positive(), !collapsed);
                        if !collapsed {
                            assert_eq!(right.width(), HEADER_HEIGHT);
                            assert_eq!(right.right(), panel.right());
                            let cards: Vec<_> = slots
                                .iter()
                                .map(|slot| daw_unit_ui::layout::unit_strip_rect(*slot))
                                .collect();
                            assert_eq!(cards[0].left() - left.right(), 4.0);
                            assert_eq!(right.left() - cards.last().unwrap().right(), 4.0);
                            for (index, card) in cards.iter().enumerate() {
                                assert_eq!(card.top() - panel.top(), 4.0);
                                assert_eq!(panel.bottom() - card.bottom(), 4.0);
                                if index > 0 {
                                    assert_eq!(card.left() - cards[index - 1].right(), 4.0);
                                }
                                if index < 2 {
                                    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.rect == *card && r.stroke.width == 0.0 && r.corner_radius == egui::CornerRadius::same(5))));
                                } else {
                                    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.rect == *card && r.fill != Color32::TRANSPARENT)));
                                }
                            }
                        }
                        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                            if r.rect == *panel && r.stroke.width == 0.0 && r.corner_radius == egui::CornerRadius::same(7))));
                        assert!(
                            output
                                .shapes
                                .iter()
                                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                            if r.rect == *left && r.fill == crate::theme::unit_header_fill(&ctx, false)))
                        );
                    }
                    assert_eq!(units[0].0.left() - row.left(), UNIT_GAP);
                    assert_eq!(row.right() - units[1].0.right(), UNIT_GAP);
                    assert_eq!(units[1].0.left() - units[0].0.right(), UNIT_GAP);
                    output.textures_delta.clear();
                }
            }
        }
    }
}
