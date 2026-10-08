use egui::{Color32, Context, CornerRadius, FontId, Stroke, TextStyle, Vec2, Visuals};

pub const BACKGROUND: Color32 = Color32::from_rgb(18, 18, 18);
pub const PANEL: Color32 = Color32::from_rgb(30, 30, 30);
pub const INPUT: Color32 = Color32::from_rgb(15, 15, 15);
pub const BORDER: Color32 = Color32::from_rgb(55, 55, 55);
pub const DIVIDER: Color32 = Color32::from_rgb(8, 8, 8);
pub const TEXT: Color32 = Color32::from_rgb(199, 201, 204);
pub const TIME_UNIT: Color32 = Color32::from_rgb(170, 174, 179);
pub const SECONDARY: Color32 = Color32::from_rgb(139, 143, 148);
/// The single source for accent colors across the app, including tinted fills.
pub const ACCENT: Color32 = crate::settings::AccentColor::Purple.color();
pub const SELECTION_OUTLINE: Stroke = Stroke {
    width: 2.0,
    color: ACCENT,
};
pub const SELECTED: Color32 = accent_tint(PANEL, 18);
pub const GRID: Color32 = Color32::from_rgb(40, 40, 40);
pub const GRID_SUBDIVISION: Color32 = Color32::from_rgb(28, 28, 28);
pub const RULER_SELECTION: Color32 = accent_tint(BORDER, 35);
pub const RULER_SELECTION_INACTIVE: Color32 = accent_tint(PANEL, 12);
pub const CLIP: Color32 = display_color(daw_core::LEGACY_CLIP_COLOR);
pub const CLIP_HEADER: Color32 = Color32::from_rgb(35, 66, 83);
pub const CLIP_CHANNEL: Color32 = Color32::from_rgb(47, 87, 105);
pub const WAVEFORM: Color32 = Color32::from_rgb(171, 208, 223);
pub const CLIP_TEXT: Color32 = Color32::from_rgb(215, 231, 239);
pub const MISSING: Color32 = Color32::from_rgb(47, 47, 47);
pub const WARNING: Color32 = Color32::from_rgb(213, 165, 89);
pub const ERROR: Color32 = Color32::from_rgb(223, 100, 100);
pub const METER: Color32 = Color32::from_rgb(92, 179, 80);
pub const TOOLBAR_PADDING: i8 = 8;

/// Related clip colors resolved together for clips and their drag previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipColors {
    pub body: Color32,
    pub header: Color32,
    pub channel: Color32,
    pub waveform: Color32,
    pub text: Color32,
}

impl Default for ClipColors {
    fn default() -> Self {
        Self {
            body: CLIP,
            header: CLIP_HEADER,
            channel: CLIP_CHANNEL,
            waveform: WAVEFORM,
            text: CLIP_TEXT,
        }
    }
}

/// Clips inherit their track's color unless overridden; saved legacy blue retains its palette.
pub fn clip_colors(
    track_color: daw_core::RgbColor,
    override_color: Option<daw_core::RgbColor>,
) -> ClipColors {
    let color = override_color.unwrap_or(track_color);
    if color == daw_core::LEGACY_CLIP_COLOR {
        return ClipColors::default();
    }
    let body = display_color(color);
    let header = blend(body, Color32::BLACK, 20);
    ClipColors {
        body,
        header,
        channel: blend(body, contrast_color(body), 6),
        waveform: blend(body, contrast_color(body), 75),
        text: blend(header, contrast_color(header), 90),
    }
}

const fn display_color(color: daw_core::RgbColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn contrast_color(color: Color32) -> Color32 {
    let brightness =
        299 * u32::from(color.r()) + 587 * u32::from(color.g()) + 114 * u32::from(color.b());
    if brightness >= 128_000 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Blend the shared accent into an opaque surface at the given percentage.
const fn accent_tint(surface: Color32, percent: u8) -> Color32 {
    blend(surface, ACCENT, percent)
}

/// The current accent belongs to this UI context, not to a global mutable value.
pub fn accent(ctx: &Context) -> Color32 {
    ctx.global_style().visuals.hyperlink_color
}

pub fn selection_outline(ctx: &Context) -> Stroke {
    Stroke::new(2.0, accent(ctx))
}

pub fn ruler_selection(ctx: &Context, enabled: bool) -> Color32 {
    blend(
        if enabled { BORDER } else { PANEL },
        accent(ctx),
        if enabled { 35 } else { 12 },
    )
}

pub fn set_accent(ctx: &Context, color: Color32) {
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        let visuals = &mut style.visuals;
        let selected = blend(PANEL, color, 18);
        visuals.hyperlink_color = color;
        visuals.selection.bg_fill = selected;
        visuals.selection.stroke.color = color;
        visuals.text_cursor.stroke.color = color;
        visuals.widgets.active.bg_fill = selected;
        visuals.widgets.active.weak_bg_fill = selected;
        visuals.widgets.active.bg_stroke.color = color;
        visuals.widgets.active.fg_stroke.color = color;
        visuals.widgets.open.bg_fill = selected;
        visuals.widgets.open.weak_bg_fill = selected;
        visuals.widgets.open.bg_stroke.color = color;
    });
    ctx.request_repaint();
}

const fn blend(surface: Color32, color: Color32, percent: u8) -> Color32 {
    assert!(percent <= 100);
    const fn channel(surface: u8, color: u8, percent: u8) -> u8 {
        let weight = percent as u16;
        ((surface as u16 * (100 - weight) + color as u16 * weight + 50) / 100) as u8
    }
    Color32::from_rgb(
        channel(surface.r(), color.r(), percent),
        channel(surface.g(), color.g(), percent),
        channel(surface.b(), color.b(), percent),
    )
}

/// Shared visual settings for controls, panels, dialogs, and timeline components.
pub fn configure(ctx: &Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = INPUT;
    visuals.text_edit_bg_color = Some(INPUT);
    visuals.faint_bg_color = Color32::from_rgb(34, 34, 34);
    visuals.weak_text_color = Some(SECONDARY);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = CornerRadius::same(2);
    visuals.menu_corner_radius = CornerRadius::same(2);
    visuals.selection.bg_fill = SELECTED;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.text_cursor.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 42, 42);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(49, 49, 49);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 42, 42);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, SECONDARY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, CLIP_TEXT);
    visuals.widgets.active.bg_fill = SELECTED;
    visuals.widgets.active.weak_bg_fill = SELECTED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, ACCENT);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(2);
        widget.expansion = 0.0;
    }
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.item_spacing = Vec2::new(8.0, 5.0);
        style.spacing.button_padding = Vec2::new(7.0, 3.0);
        style.spacing.interact_size.y = 22.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(16.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.0));
    });
}
