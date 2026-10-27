//! Shared dropdown/context-menu presentation and catalog list spacing.
use egui::{ComboBox, InnerResponse, Style, TextStyle, Ui, Vec2, WidgetText};

pub const ITEM_PADDING: i8 = 4;
pub const OUTER_MARGIN: i8 = 4;
pub const ITEM_SPACING: f32 = 4.0;
pub const MIN_WIDTH: f32 = 140.0;

/// Keep a visible gap between action labels and right-aligned shortcuts.
pub fn action_button(label: impl Into<WidgetText>) -> egui::Button<'static> {
    egui::Button::new(label.into()).gap(ITEM_SPACING)
}

/// Style popup contents without changing the collapsed selector's geometry.
pub fn popup_style(style: &mut Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = Vec2::splat(f32::from(ITEM_PADDING));
    style.spacing.icon_spacing = 0.0;
    style.spacing.item_spacing.y = ITEM_SPACING;
    style.spacing.menu_margin = egui::Margin::same(OUTER_MARGIN);
    style.override_font_id = None;
    style.override_text_style = Some(TextStyle::Button);
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    let text = style.visuals.widgets.noninteractive.fg_stroke.color;
    let secondary = style.visuals.weak_text_color();
    let fill = crate::theme::TOOLBAR_HOVER_FILL;
    let radius = style.visuals.widgets.inactive.corner_radius;
    style.visuals.selection.stroke = egui::Stroke::new(0.0, text);
    style.visuals.widgets.inactive.fg_stroke.color = secondary;
    for visuals in [
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        visuals.weak_bg_fill = fill;
        visuals.bg_fill = fill;
        visuals.bg_stroke = egui::Stroke::NONE;
        visuals.fg_stroke.color = text;
        visuals.corner_radius = radius;
        visuals.expansion = 0.0;
    }
}

/// A divider owns only its line height; adjacent item spacing supplies the gaps.
pub fn separator(ui: &mut Ui) -> egui::Response {
    ui.add(egui::Separator::default().spacing(1.0))
}

/// A checked action keeps the shared full-row button highlight and shortcut layout.
pub fn checked_button(
    ui: &mut Ui,
    selected: bool,
    label: impl Into<WidgetText>,
    shortcut: impl Into<WidgetText>,
) -> egui::Response {
    let check = egui::Id::new("menu_check");
    let row = egui::Button::new((
        egui::Atom::custom(check, Vec2::splat(ui.spacing().icon_width)),
        label.into(),
    ))
    .shortcut_text(shortcut.into())
    .selected(selected)
    .atom_ui(ui);
    if selected && let Some(slot) = row.rect(check) {
        let mark =
            egui::Rect::from_center_size(slot.center(), Vec2::splat(ui.spacing().icon_width_inner));
        ui.painter().add(egui::Shape::line(
            vec![mark.left_center(), mark.center_bottom(), mark.right_top()],
            egui::Stroke::new(
                ui.style().interact(&row.response).fg_stroke.width,
                ui.visuals().selection.stroke.color,
            ),
        ));
    }
    row.response
}

/// Fill every popup row, retaining enough width for regular text and long labels.
pub fn layout<R>(ui: &mut Ui, width: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    ui.set_width(width.max(MIN_WIDTH));
    ui.with_layout(
        egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
        content,
    )
    .inner
}

/// A secondary-click popup with the canonical menu style, rows, and egui identity.
#[must_use = "Call .show() to display the context menu"]
pub struct ContextMenu(egui::Popup<'static>);

impl ContextMenu {
    pub fn new(response: &egui::Response) -> Self {
        Self(
            egui::Popup::context_menu(response)
                .width(MIN_WIDTH)
                .style(popup_style),
        )
    }

    /// Retain caller-owned opening guards for compound controls such as track panels.
    pub fn open_memory(mut self, state: impl Into<Option<egui::SetOpenCommand>>) -> Self {
        self.0 = self.0.open_memory(state);
        self
    }

    pub fn show<R>(self, content: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>> {
        self.0.show(|ui| layout(ui, ui.available_width(), content))
    }
}

/// A selector with the canonical popup style and existing caption/size options.
#[must_use]
pub struct Dropdown(ComboBox);
impl Dropdown {
    pub fn from_id_salt(id: impl egui::AsIdSalt) -> Self {
        Self(ComboBox::from_id_salt(id))
    }
    pub fn selected_text(mut self, text: impl Into<WidgetText>) -> Self {
        self.0 = self.0.selected_text(text);
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.0 = self.0.width(width);
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.0 = self.0.height(height);
        self
    }
    pub fn truncate(mut self) -> Self {
        self.0 = self.0.truncate();
        self
    }
    pub fn wrap_mode(mut self, mode: egui::TextWrapMode) -> Self {
        self.0 = self.0.wrap_mode(mode);
        self
    }
    pub fn show_ui<R>(
        self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<Option<R>> {
        self.0
            .popup_style(popup_style.into())
            .show_ui(ui, |ui| layout(ui, ui.available_width(), content))
    }
}

#[cfg(test)]
#[path = "menus/context_menu_tests.rs"]
mod context_menu_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropdown_popup_uses_regular_text_and_full_rows_inside_small_controls() {
        for color in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            crate::theme::configure(&ctx);
            ctx.set_theme(color);
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            let mut selected = 0;
            let mut button = egui::Rect::NOTHING;
            let mut render = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(600.0, 500.0),
                        )),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ui| {
                        ui.style_mut().override_text_style = Some(TextStyle::Small);
                        button = Dropdown::from_id_salt("test")
                            .selected_text(egui::RichText::new("Closed").small())
                            .width(65.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut selected, 0, "First");
                                ui.selectable_value(
                                    &mut selected,
                                    1,
                                    "Long dropdown item without wrapping",
                                );
                                ui.add_enabled_ui(false, |ui| {
                                    ui.selectable_value(&mut selected, 2, "Disabled")
                                });
                            })
                            .response
                            .rect;
                    },
                );
                output.textures_delta.clear();
                (button, output)
            };
            let (closed, _) = render(vec![]);
            let point = closed.center();
            render(vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
            render(vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
            let (_, opened) = render(vec![]);
            for label in ["First", "Long dropdown item without wrapping", "Disabled"] {
                let text = opened
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(t) if t.galley.text() == label => Some(t),
                        _ => None,
                    })
                    .expect("popup item");
                assert_eq!(text.galley.rows.len(), 1);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size == 13.0)
                );
            }
            let row = opened
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(r)
                        if r.fill == crate::theme::toggle_colors(&ctx, true).0
                            && r.rect.width() >= MIN_WIDTH =>
                    {
                        Some(r)
                    }
                    _ => None,
                })
                .expect("full-width selected row");
            assert!(row.rect.width() > MIN_WIDTH, "long labels expand the popup");
            assert!(row.corner_radius.nw > 0);
            let label = opened
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == "Long dropdown item without wrapping" =>
                    {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .unwrap();
            for point in [
                label.center(),
                egui::pos2(label.left() - 2.0, label.center().y),
            ] {
                render(vec![egui::Event::PointerMoved(point)]);
                let (_, hovered) = render(vec![egui::Event::PointerMoved(point)]);
                assert!(
                    hovered.shapes.iter().any(|shape| matches!(
                        &shape.shape, egui::Shape::Rect(rect)
                            if rect.rect.contains(point) && rect.rect.width() >= MIN_WIDTH
                                && rect.fill == crate::theme::TOOLBAR_HOVER_FILL
                                && rect.stroke.width == 0.0 && rect.corner_radius.nw > 0
                    )),
                    "Dropdown labels and padding must use the view-button hover color"
                );
            }
            assert_eq!(selected, 0);
        }
    }
}
