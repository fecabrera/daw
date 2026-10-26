pub use daw_unit_ui::theme::*;
use egui::{Color32, Context};
/// Related clip colors resolved together for clips and their drag previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipColors {
    pub body: Color32,
    pub header: Color32,
    pub channel: Color32,
    pub waveform: Color32,
    pub text: Color32,
}

impl ClipColors {
    /// Match disabled units with a neutral header and secondary foregrounds.
    /// Painter opacity would let the timeline grid show through the clip.
    pub fn dimmed(self, ctx: &Context) -> Self {
        let colors = palette(ctx);
        Self {
            body: colors.panel,
            header: unit_header_fill(ctx, false),
            channel: colors.panel,
            waveform: colors.secondary,
            text: colors.secondary,
        }
    }
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
    clip_palette(display_color(color))
}

/// Derive readable foregrounds and related surfaces from one clip body color.
fn clip_palette(body: Color32) -> ClipColors {
    let header = blend(body, Color32::BLACK, 20);
    ClipColors {
        body,
        header,
        channel: blend(body, contrast_color(body), 6),
        waveform: blend(body, contrast_color(body), 75),
        text: blend(header, contrast_color(header), 90),
    }
}

/// Match colored strip text with contrast against the unit header surface.
pub(crate) fn unit_header_text(background: Color32) -> Color32 {
    let header = blend(background, Color32::BLACK, 20);
    blend(header, contrast_color(header), 90)
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
