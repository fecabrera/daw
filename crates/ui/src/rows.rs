use egui::{Align, InnerResponse, Layout, Ui, Vec2};

pub(super) use daw_unit_ui::theme::panel_button_style;
pub(super) const GROUP_DIVIDER_WIDTH: f32 = 1.0;

pub const NAME_FONT_SIZE: f32 = 13.0;

/// Shared full-width track/subtrack name button with text anchored to the row inset.
pub(super) fn name_button(
    ui: &mut Ui,
    id: impl egui::AsIdSalt,
    name: &str,
    font_size: f32,
    selected: bool,
    sense: egui::Sense,
    colors: Option<crate::theme::ClipColors>,
) -> (egui::Response, Option<egui::Rect>) {
    use egui::AtomExt as _;
    let width = ui.available_width();
    let text_id = ui.make_persistent_id(id);
    let mut atoms = egui::Atoms::new(
        egui::RichText::new(name)
            .font(crate::fonts::semibold(font_size))
            .color(colors.map_or(crate::theme::palette(ui.ctx()).text, |colors| colors.text))
            .atom_id(text_id),
    );
    atoms.push_right(egui::Atom::grow());
    let mut button = egui::Button::new(atoms)
        .sense(sense)
        .selected(selected)
        .frame(false)
        .truncate()
        .gap(0.0);
    if let Some(colors) = colors {
        button = button
            .fill(colors.header)
            .stroke(egui::Stroke::NONE)
            .frame(false)
            .gap(0.0);
    }
    let output = button
        .min_size(Vec2::new(width, crate::toolbars::CONTROL_HEIGHT))
        .atom_ui(ui);
    let text = output.rect(text_id);
    (output.response, text)
}

/// Shared inline name field for track, beat-subtrack and instrument-layer headers.
/// Select the complete Unicode name when the caller starts a rename.
pub(super) fn name_input(
    ui: &mut Ui,
    text: &mut String,
    id: egui::Id,
    font_size: f32,
    focus: &mut bool,
) -> egui::Response {
    let width = ui.available_width();
    let mut output = egui::TextEdit::singleline(text)
        .id(id)
        .font(egui::FontId::proportional(font_size))
        .text_color(crate::theme::palette(ui.ctx()).text)
        .horizontal_align(egui::Align::Min)
        .desired_width(width)
        .min_size(Vec2::new(width, crate::toolbars::CONTROL_HEIGHT))
        .show(ui);
    if *focus {
        output.response.request_focus();
        output
            .state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(text.chars().count()),
            )));
        output.state.store(ui.ctx(), output.response.id);
        *focus = false;
    }
    output.response.response
}

/// Standard wrapped advice and model information below dialog controls.
pub(super) fn advice(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    let color = crate::theme::palette(ui.ctx()).secondary;
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().color(color)).wrap())
}

/// Keep adjacent pane rows aligned even when text or frames round differently.
pub(super) fn fixed<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    InnerResponse {
        inner: content(&mut child),
        response,
    }
}

/// Reserve the tallest control's height before placing vertically centered items.
pub use daw_unit_ui::rows::centered;

/// Keep header text at the left inset while centering it vertically.
pub fn left_aligned<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center).with_main_align(Align::Min),
        |ui| {
            ui.set_min_height(height);
            content(ui)
        },
    )
}

/// Keep row widget identities attached to an item when its position changes.
pub fn centered_with_id<R>(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.push_id(id_salt, |ui| centered(ui, height, content))
        .inner
}

/// Preserve fixed row geometry and offscreen drop bounds without constructing controls.
/// Call only outside active interactions or text editing.
pub fn placeholder(
    ui: &mut Ui,
    id: impl egui::AsIdSalt,
    width: f32,
    height: f32,
    left_inset: f32,
) -> (egui::Rect, egui::Rect) {
    centered_with_id(ui, id, height, |ui| {
        let mut panel = ui
            .allocate_exact_size(Vec2::new(width, height), egui::Sense::hover())
            .0;
        panel.min.x += left_inset;
        let lane = ui
            .allocate_exact_size(
                Vec2::new(ui.available_width(), height),
                egui::Sense::hover(),
            )
            .0;
        (panel, lane)
    })
    .inner
}

/// Keep form controls aligned after a shared label column.
pub fn field<R>(
    ui: &mut Ui,
    label_width: f32,
    label: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    field_with_height(
        ui,
        label_width,
        crate::toolbars::CONTROL_HEIGHT,
        label,
        control,
    )
}

/// Reuse field alignment in panels whose rows also own vertical padding.
pub(super) fn field_with_height<R>(
    ui: &mut Ui,
    label_width: f32,
    height: f32,
    label: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    centered(ui, height, |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(label_width, height),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(label_width);
                label(ui);
            },
        );
        control(ui)
    })
}

pub fn wrapped<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), height),
        Layout::left_to_right(Align::Center).with_main_wrap(true),
        content,
    )
}

/// Standard centered unit-footer pair: 18-point Power, 8-point gap, 11-point label.
/// The caller supplies state and handles the returned release action.
pub fn power_label(ui: &mut Ui, label: &str, name: &str, on: bool) -> egui::Response {
    power_label_with(ui, label, name, on, |ui, label, width| {
        ui.add_sized(
            Vec2::new(width, crate::icons::COMPACT_BUTTON_SIZE),
            egui::Label::new(label).selectable(false).truncate(),
        )
    })
    .0
}

/// Reuse the footer geometry with an interactive label or inline name editor.
pub(super) fn power_label_with(
    ui: &mut Ui,
    label: &str,
    name: &str,
    on: bool,
    label_ui: impl FnOnce(&mut Ui, egui::RichText, f32) -> egui::Response,
) -> (egui::Response, egui::Response) {
    let palette = crate::theme::palette(ui.ctx());
    let font = egui::FontId::proportional(crate::panels::HEADER_FONT_SIZE);
    let color = if on { palette.text } else { palette.secondary };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), color);
    let gap = f32::from(crate::theme::TOOLBAR_PADDING);
    let label_width = galley
        .size()
        .x
        .min((ui.available_width() - crate::icons::COMPACT_BUTTON_SIZE - gap).max(0.0));
    let pair_width = crate::icons::COMPACT_BUTTON_SIZE + gap + label_width;
    ui.spacing_mut().item_spacing.x = gap;
    ui.add_space(((ui.available_width() - pair_width) * 0.5).max(0.0).round());
    let response = crate::icons::circular_button(
        ui,
        crate::icons::Lucide::Power,
        name,
        Some(on),
        crate::icons::COMPACT_BUTTON_SIZE,
    );
    let label_response = label_ui(
        ui,
        egui::RichText::new(label).font(font).color(color),
        label_width,
    );
    (response, label_response)
}

/// Center a standard unit footer label in its reserved row.
pub use daw_unit_ui::rows::unit_label;

/// Join related inputs inside one standard frame, retaining each widget's interaction.
pub fn input_group<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    input_group_with_height(ui, crate::toolbars::CONTROL_HEIGHT, content)
}

/// Size the shared group frame for compact controls without changing its style.
pub fn input_group_with_height<R>(
    ui: &mut Ui,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let frame = egui::Frame::new()
        .fill(crate::theme::unit_separator(ui.ctx()))
        .corner_radius(5);
    frame.show(ui, |ui| {
        ui.spacing_mut().interact_size.y = height;
        ui.spacing_mut().item_spacing.x = GROUP_DIVIDER_WIDTH;
        // A typed segment keeps selection text feedback without a focus outline.
        ui.visuals_mut().selection.stroke.width = 0.0;
        let widgets = &mut ui.visuals_mut().widgets;
        for widget in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            widget.bg_stroke = egui::Stroke::NONE;
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
        ui.horizontal(content).inner
    })
}

/// Give the first/last joined control only the group's outside corners.
pub(super) fn input_group_edge(ui: &mut Ui, last: bool) {
    let radius = if last {
        egui::CornerRadius {
            nw: 0,
            sw: 0,
            ne: 5,
            se: 5,
        }
    } else {
        egui::CornerRadius {
            nw: 5,
            sw: 5,
            ne: 0,
            se: 0,
        }
    };
    let widgets = &mut ui.visuals_mut().widgets;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = radius;
    }
}

/// Paint only the shared separator; joined controls have no outer outline.
pub fn group_separator(ui: &Ui, rect: egui::Rect, next_control_left: f32) {
    ui.painter().vline(
        next_control_left - GROUP_DIVIDER_WIDTH * 0.5,
        rect.y_range(),
        egui::Stroke::new(
            GROUP_DIVIDER_WIDTH,
            crate::theme::unit_header_fill(ui.ctx(), false),
        ),
    );
}

#[derive(Clone)]
struct InputDraft<T> {
    saved: T,
    value: T,
    generation: u64,
}
/// Keep drag previews local and let the standard input own text entry and stepping.
pub(super) fn staged_input<T: Copy + PartialEq + Send + Sync + 'static>(
    ui: &mut egui::Ui,
    key: &str,
    saved: &mut T,
    add: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
) -> (egui::Response, T) {
    let id = ui.make_persistent_id(key);
    let mut draft = ui
        .data_mut(|d| d.get_temp::<InputDraft<T>>(id))
        .unwrap_or(InputDraft {
            saved: *saved,
            value: *saved,
            generation: 0,
        });
    if draft.saved != *saved {
        draft.saved = *saved;
        draft.value = *saved;
        draft.generation = draft.generation.wrapping_add(1);
    }
    // Capture Escape before TextEdit consumes it. A new widget generation discards
    // its pending text, so losing focus on the next frame cannot commit that text.
    let cancelled =
        !ui.is_enabled() || ui.input(|i| !i.focused || i.key_pressed(egui::Key::Escape));
    let response = ui
        .push_id(draft.generation, |ui| add(ui, &mut draft.value))
        .inner;
    if cancelled {
        draft.value = *saved;
        draft.generation = draft.generation.wrapping_add(1);
        response.surrender_focus();
    } else if response.drag_stopped()
        || (response.changed() && !ui.input(|i| i.pointer.primary_down()))
    {
        *saved = draft.value;
        draft.saved = *saved;
    } else if !response.dragged() && !response.has_focus() {
        draft.value = *saved;
    }
    let preview = draft.value;
    ui.data_mut(|d| d.insert_temp(id, draft));
    (response, preview)
}
