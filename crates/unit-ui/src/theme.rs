use egui::{Color32, Context, CornerRadius, FontId, Stroke, TextStyle, Vec2, Visuals};

pub const BACKGROUND: Color32 = Color32::from_rgb(18, 18, 18);
pub const PANEL: Color32 = Color32::from_rgb(30, 30, 30);
pub const INPUT: Color32 = Color32::from_rgb(15, 15, 15);
pub const BORDER: Color32 = Color32::from_rgb(55, 55, 55);
pub const DIVIDER: Color32 = Color32::from_rgb(8, 8, 8);
pub const TEXT: Color32 = Color32::from_rgb(199, 201, 204);
pub const TIME_UNIT: Color32 = Color32::from_rgb(170, 174, 179);
pub const SECONDARY: Color32 = Color32::from_rgb(139, 143, 148);
/// Default accent. Runtime colors come from the current context.
pub const ACCENT: Color32 = Color32::from_rgb(156, 39, 176);
pub const SELECTION_OUTLINE: Stroke = Stroke {
    width: 2.0,
    color: ACCENT,
};
pub const SELECTED: Color32 = toggle_fill(PANEL, ACCENT);
pub const GRID: Color32 = Color32::from_rgb(40, 40, 40);
pub const GRID_SUBDIVISION: Color32 = Color32::from_rgb(28, 28, 28);
/// Loop button and enabled ruler handles share one semantic color.
pub const LOOP_COLOR: Color32 = Color32::from_rgb(175, 140, 62);
/// Lighter shade of the supplied gold, preserving its hue and saturation.
const LOOP_RANGE_FILL: Color32 = Color32::from_rgb(208, 172, 60);
pub const CLIP: Color32 = Color32::from_rgb(43, 81, 99);
pub const CLIP_HEADER: Color32 = Color32::from_rgb(35, 66, 83);
pub const CLIP_CHANNEL: Color32 = Color32::from_rgb(47, 87, 105);
pub const WAVEFORM: Color32 = Color32::from_rgb(171, 208, 223);
pub const CLIP_TEXT: Color32 = Color32::from_rgb(215, 231, 239);
pub const MISSING: Color32 = Color32::from_rgb(47, 47, 47);
pub const WARNING: Color32 = Color32::from_rgb(213, 165, 89);
pub const ERROR: Color32 = Color32::from_rgb(223, 100, 100);
pub const METER: Color32 = Color32::from_rgb(92, 179, 80);
pub const TOOLBAR_PADDING: i8 = 8;
pub const TOOLBAR_HOVER_FILL: Color32 = Color32::from_rgba_unmultiplied_const(128, 128, 128, 40);

/// Surface and foreground colors for custom painting. Clip palettes are independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: Color32,
    pub panel: Color32,
    pub input: Color32,
    pub border: Color32,
    pub divider: Color32,
    pub text: Color32,
    pub time_unit: Color32,
    pub secondary: Color32,
    pub grid: Color32,
    pub grid_subdivision: Color32,
    pub missing: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub meter: Color32,
}

pub const DARK: Palette = Palette {
    background: BACKGROUND,
    panel: PANEL,
    input: INPUT,
    border: BORDER,
    divider: DIVIDER,
    text: TEXT,
    time_unit: TIME_UNIT,
    secondary: SECONDARY,
    grid: GRID,
    grid_subdivision: GRID_SUBDIVISION,
    missing: MISSING,
    warning: WARNING,
    error: ERROR,
    meter: METER,
};

/// Near-white surfaces and pale separators sampled from the Light reference.
pub const LIGHT: Palette = Palette {
    background: Color32::from_rgb(251, 251, 251),
    panel: Color32::WHITE,
    input: Color32::from_rgb(246, 246, 251),
    border: Color32::from_rgb(242, 242, 242),
    divider: Color32::from_rgb(220, 220, 220),
    text: Color32::from_rgb(40, 43, 47),
    time_unit: Color32::from_rgb(85, 89, 94),
    secondary: Color32::from_rgb(106, 110, 115),
    grid: Color32::from_rgb(208, 210, 213),
    grid_subdivision: Color32::from_rgb(221, 223, 225),
    missing: Color32::from_rgb(219, 221, 223),
    warning: Color32::from_rgb(150, 93, 0),
    error: Color32::from_rgb(179, 38, 30),
    meter: Color32::from_rgb(56, 142, 60),
};

/// Cool grey surfaces sampled from the supplied Mist reference.
pub const MIST: Palette = Palette {
    background: Color32::from_rgb(230, 233, 238),
    panel: Color32::from_rgb(239, 241, 245),
    input: Color32::from_rgb(216, 218, 224),
    border: Color32::from_rgb(221, 224, 231),
    divider: Color32::from_rgb(221, 224, 231),
    grid: Color32::from_rgb(216, 218, 224),
    grid_subdivision: Color32::from_rgb(221, 224, 231),
    missing: Color32::from_rgb(216, 218, 224),
    ..LIGHT
};

fn light_palette_id() -> egui::Id {
    egui::Id::new("application-light-palette")
}

pub fn palette(ctx: &Context) -> Palette {
    if ctx.global_style().visuals.dark_mode {
        DARK
    } else {
        ctx.data(|data| data.get_temp(light_palette_id()).unwrap_or(LIGHT))
    }
}

/// Subdued unit dividers must contrast with the panel in either theme.
pub fn unit_separator(ctx: &Context) -> Color32 {
    let colors = palette(ctx);
    blend(colors.panel, colors.border, 50)
}

/// Unit headers share the active view fill when on and the panel border color when off.
pub fn unit_header_fill(ctx: &Context, enabled: bool) -> Color32 {
    if enabled {
        toggle_colors(ctx, true).0
    } else {
        palette(ctx).border
    }
}

/// Shared on/off colors for view controls and unit toggles.
pub fn toggle_colors(ctx: &Context, selected: bool) -> (Color32, Color32) {
    if selected {
        let fill = ctx.global_style().visuals.selection.bg_fill;
        (fill, contrast_color(fill))
    } else {
        (Color32::TRANSPARENT, palette(ctx).secondary)
    }
}

/// Shared muted active background for accent and semantic toolbar toggles.
pub const fn toggle_fill(panel: Color32, color: Color32) -> Color32 {
    blend(panel, color, 18)
}

/// The current accent belongs to this UI context, not to a global mutable value.
pub fn accent(ctx: &Context) -> Color32 {
    ctx.global_style().visuals.hyperlink_color
}

pub fn selection_outline(ctx: &Context) -> Stroke {
    Stroke::new(2.0, accent(ctx))
}

/// Loop-range background and handles, independent of the configured accent.
pub fn ruler_loop_colors(ctx: &Context, enabled: bool) -> (Color32, Color32) {
    if enabled {
        (LOOP_RANGE_FILL, LOOP_COLOR)
    } else {
        (TOOLBAR_HOVER_FILL, toggle_colors(ctx, false).1)
    }
}

pub fn set_accent(ctx: &Context, color: Color32) {
    let theme = if ctx.global_style().visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let panel = palette(ctx).panel;
    ctx.style_mut_of(theme, |style| {
        let visuals = &mut style.visuals;
        let selected = toggle_fill(panel, color);
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
        visuals.widgets.open.fg_stroke.color = color;
    });
    ctx.request_repaint();
}

pub const fn blend(surface: Color32, color: Color32, percent: u8) -> Color32 {
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

fn configure_style(ctx: &Context, theme: egui::Theme, colors: Palette) {
    let mut visuals = match theme {
        egui::Theme::Dark => Visuals::dark(),
        egui::Theme::Light => Visuals::light(),
    };
    visuals.panel_fill = colors.panel;
    visuals.window_fill = colors.panel;
    visuals.extreme_bg_color = colors.input;
    visuals.text_edit_bg_color = Some(colors.input);
    visuals.faint_bg_color = match theme {
        egui::Theme::Dark => Color32::from_rgb(34, 34, 34),
        egui::Theme::Light => blend(colors.panel, colors.text, 3),
    };
    visuals.weak_text_color = Some(colors.secondary);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = colors.warning;
    visuals.error_fg_color = colors.error;
    visuals.window_stroke = Stroke::new(1.0_f32, colors.border);
    visuals.window_corner_radius = CornerRadius::same(2);
    visuals.menu_corner_radius = CornerRadius::same(2);
    let selected = toggle_fill(colors.panel, ACCENT);
    visuals.selection.bg_fill = selected;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.text_cursor.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.noninteractive.bg_fill = colors.panel;
    visuals.widgets.noninteractive.weak_bg_fill = colors.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors.text);
    visuals.widgets.inactive.bg_fill = match theme {
        egui::Theme::Dark => Color32::from_rgb(42, 42, 42),
        egui::Theme::Light if colors == MIST => colors.input,
        egui::Theme::Light => Color32::from_rgb(238, 239, 240),
    };
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors.text);
    visuals.widgets.hovered.bg_fill = match theme {
        egui::Theme::Dark => Color32::from_rgb(49, 49, 49),
        egui::Theme::Light if colors == MIST => blend(colors.input, colors.text, 4),
        egui::Theme::Light => Color32::from_rgb(225, 227, 230),
    };
    visuals.widgets.hovered.weak_bg_fill = visuals.widgets.inactive.bg_fill;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors.secondary);
    visuals.widgets.hovered.fg_stroke = Stroke::new(
        1.0_f32,
        match theme {
            egui::Theme::Dark => CLIP_TEXT,
            egui::Theme::Light => colors.text,
        },
    );
    visuals.widgets.active.bg_fill = selected;
    visuals.widgets.active.weak_bg_fill = selected;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.open = visuals.widgets.active;
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
    ctx.style_mut_of(theme, |style| {
        style.visuals = visuals;
        style.spacing.item_spacing = Vec2::new(8.0, 5.0);
        style.spacing.button_padding = Vec2::new(7.0, 3.0);
        style.spacing.interact_size.y = 22.0;
        // All scrollbars share Timeline's surface palette; retain their geometry/fading.
        style.spacing.scroll.foreground_color = false;
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

/// Apply a context-local palette and accent. Preferences are independent of audio state.
pub fn apply(ctx: &Context, theme: egui::Theme, light_colors: Palette, accent: Color32) {
    if ctx.style_of(egui::Theme::Light).visuals.panel_fill != light_colors.panel {
        configure_style(ctx, egui::Theme::Light, light_colors);
    }
    ctx.data_mut(|data| data.insert_temp(light_palette_id(), light_colors));
    ctx.set_theme(theme);
    set_accent(ctx, accent);
}
pub fn configure(ctx: &Context) {
    configure_style(ctx, egui::Theme::Dark, DARK);
    configure_style(ctx, egui::Theme::Light, LIGHT);
    apply(ctx, egui::Theme::Dark, LIGHT, ACCENT);
}
pub fn contrast_color(color: Color32) -> Color32 {
    let brightness =
        299 * u32::from(color.r()) + 587 * u32::from(color.g()) + 114 * u32::from(color.b());
    if brightness >= 128_000 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
