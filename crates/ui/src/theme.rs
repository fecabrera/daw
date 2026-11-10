pub use daw_unit_ui::theme::*;
use egui::{Color32, Context};
mod material;
pub(super) use material::GREY_200;

/// Selected lanes share the supplied dark shade and subtle neutral/cool
/// equivalents in light palettes.
pub(super) fn selected_track_background(ctx: &Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        Color32::from_gray(24)
    } else {
        blend(palette(ctx).background, Color32::BLACK, 3)
    }
}

/// Selected track headers and instrument labels contrast with the lane surface.
pub(super) fn selected_track_header_background(ctx: &Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        Color32::from_gray(38)
    } else {
        blend(palette(ctx).panel, Color32::BLACK, 3)
    }
}

/// Related clip colors resolved together for clips and their drag previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipColors {
    pub body: Color32,
    pub header: Color32,
    pub selection_header: Color32,
    pub channel: Color32,
    pub waveform: Color32,
    pub text: Color32,
}

impl ClipColors {
    /// Reduce repeat contrast against the editor surface while retaining its hue.
    pub fn repeated(self, ctx: &Context) -> Self {
        Self {
            body: dimmed_repeat_color(ctx, self.body),
            header: dimmed_repeat_color(ctx, self.header),
            selection_header: dimmed_repeat_color(ctx, self.selection_header),
            channel: dimmed_repeat_color(ctx, self.channel),
            waveform: dimmed_repeat_color(ctx, self.waveform),
            text: dimmed_repeat_color(ctx, self.text),
        }
    }

    /// Use the resolved Material 200 header while retaining clip body and content colors.
    pub fn selected_header(self) -> Self {
        let header = self.selection_header;
        Self {
            header,
            text: blend(header, contrast_color(header), 90),
            ..self
        }
    }

    /// Match disabled units with a neutral header and secondary foregrounds.
    /// Translucent body/channel surfaces retain the underlying musical grid.
    pub fn dimmed(self, ctx: &Context) -> Self {
        let colors = palette(ctx);
        Self {
            body: colors.panel.gamma_multiply(0.5),
            header: unit_header_fill(ctx, false),
            selection_header: material::GREY_200,
            channel: colors.panel.gamma_multiply(0.5),
            waveform: colors.secondary,
            text: colors.secondary,
        }
    }
}

pub(super) fn dimmed_repeat_color(ctx: &Context, color: Color32) -> Color32 {
    blend(color, palette(ctx).background, 45)
}

impl Default for ClipColors {
    fn default() -> Self {
        Self {
            body: CLIP,
            header: CLIP_HEADER,
            selection_header: material::BLUE_GREY_200,
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
    clip_palette(display_color(color))
}

/// Derive readable foregrounds and related surfaces from one clip body color.
fn clip_palette(body: Color32) -> ClipColors {
    let header = blend(body, Color32::BLACK, 20);
    ClipColors {
        body,
        header,
        selection_header: material::shade_200(body),
        channel: blend(body, contrast_color(body), 6),
        waveform: blend(body, contrast_color(body), 75),
        text: blend(header, contrast_color(header), 90),
    }
}

const fn display_color(color: daw_core::RgbColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

/// Apply application preferences without changing layout or project data.
pub fn apply(ctx: &Context, settings: crate::settings::AppSettings) {
    let theme = match settings.theme {
        crate::settings::ColorTheme::Dark => egui::Theme::Dark,
        crate::settings::ColorTheme::Light | crate::settings::ColorTheme::Mist => {
            egui::Theme::Light
        }
    };
    let light_colors = if settings.theme == crate::settings::ColorTheme::Mist {
        MIST
    } else {
        LIGHT
    };
    daw_unit_ui::theme::apply(ctx, theme, light_colors, settings.accent.color());
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match theme {
        egui::Theme::Dark => egui::SystemTheme::Dark,
        egui::Theme::Light => egui::SystemTheme::Light,
    }));
}
pub fn configure(ctx: &Context) {
    daw_unit_ui::theme::configure(ctx);
    apply(ctx, crate::settings::AppSettings::default());
}
