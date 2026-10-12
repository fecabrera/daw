use egui::epaint::text::{Tag, VariationCoords};
use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, FontTweak};

pub const LICENSE: &str = include_str!("../assets/fonts/OFL.txt");

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Outfit Semibold".into()))
}

pub fn menu_shortcut_family() -> FontFamily {
    FontFamily::Name("Menu Shortcut".into())
}

pub fn numeric(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Numeric".into()))
}

/// Set the shared UI font. Keep egui's fallbacks for symbols absent from Outfit.
pub fn configure(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Outfit".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Outfit.ttf"))
            .tweak(FontTweak {
                coords: VariationCoords::new([(Tag::new(b"wght"), 400.0)]),
                ..Default::default()
            })
            .into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Outfit".into());
    // Outfit lacks infinity. Enlarge its Ubuntu fallback in numeric controls only;
    // egui centers fallback glyphs within the primary font's unchanged line height.
    let mut infinity_font = (*fonts.font_data["Ubuntu-Light"]).clone();
    infinity_font.tweak.scale = 1.8;
    fonts
        .font_data
        .insert("Numeric Symbols".into(), infinity_font.into());
    let mut numeric_family = fonts.families[&FontFamily::Proportional].clone();
    numeric_family[1] = "Numeric Symbols".into();
    fonts
        .families
        .insert(FontFamily::Name("Numeric".into()), numeric_family);
    fonts.font_data.insert(
        "Outfit Semibold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Outfit.ttf"))
            .tweak(FontTweak {
                coords: VariationCoords::new([(Tag::new(b"wght"), 600.0)]),
                ..Default::default()
            })
            .into(),
    );
    let mut semibold_family = fonts.families[&FontFamily::Proportional].clone();
    semibold_family[0] = "Outfit Semibold".into();
    fonts
        .families
        .insert(FontFamily::Name("Outfit Semibold".into()), semibold_family);
    // Outfit has no Command glyph. Give menu shortcuts their own smaller fallback,
    // while leaving letters and symbols elsewhere in the UI unchanged.
    let mut command_font = (*fonts.font_data["emoji-icon-font"]).clone();
    command_font.tweak.scale = 0.72;
    fonts
        .font_data
        .insert("Menu Command".into(), command_font.into());
    let mut shortcut_family = fonts.families[&FontFamily::Proportional].clone();
    for font in &mut shortcut_family {
        if font == "emoji-icon-font" {
            *font = "Menu Command".into();
        }
    }
    fonts
        .families
        .insert(menu_shortcut_family(), shortcut_family);
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_infinity_matches_digits_without_changing_numbers() {
        for pixels_per_point in [1.0, 1.5, 2.0] {
            let ctx = Context::default();
            configure(&ctx);
            ctx.set_pixels_per_point(pixels_per_point);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                for size in [8.0, 11.0] {
                    let layout = |text: &str, font| {
                        ui.painter()
                            .layout_no_wrap(text.into(), font, egui::Color32::WHITE)
                    };
                    let normal = layout("0−∞", FontId::proportional(size));
                    let adjusted = layout("0−∞", numeric(size));
                    let ink = |glyph: &egui::epaint::text::Glyph| {
                        egui::Rect::from_min_size(
                            glyph.pos + glyph.uv_rect.offset,
                            glyph.uv_rect.size,
                        )
                    };
                    let glyphs = &adjusted.rows[0].glyphs;
                    let digit = ink(&glyphs[0]);
                    let minus = ink(&glyphs[1]);
                    let infinity = ink(&glyphs[2]);
                    assert!(infinity.height() > ink(&normal.rows[0].glyphs[2]).height());
                    assert!(infinity.height() >= digit.height() * 0.5);
                    assert!(infinity.height() <= digit.height());
                    assert!(
                        (infinity.center().y - minus.center().y).abs() <= 1.0,
                        "size={size} scale={pixels_per_point}: {minus:?} {infinity:?}"
                    );
                    assert_eq!(normal.size().y, adjusted.size().y);
                    let numbers = "−12.0 +6.0";
                    let normal = layout(numbers, FontId::proportional(size));
                    let adjusted = layout(numbers, numeric(size));
                    assert_eq!(normal.rows[0].glyphs, adjusted.rows[0].glyphs);
                    assert_eq!(normal.size(), adjusted.size());
                }
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn menu_font_supports_command_symbol() {
        let ctx = Context::default();
        configure(&ctx);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label("⌘");
            ctx.fonts_mut(|fonts| {
                let mut font = fonts.fonts.font(&FontFamily::Proportional);
                assert!(font.has_glyphs("⌘"));
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn menu_command_matches_letter_height_and_alignment_without_changing_letters() {
        for pixels_per_point in [1.0, 1.5, 2.0] {
            let ctx = Context::default();
            configure(&ctx);
            ctx.set_pixels_per_point(pixels_per_point);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                for size in [13.0, 18.0] {
                    let layout = |text: &str, family| {
                        ui.painter().layout_no_wrap(
                            text.into(),
                            FontId::new(size, family),
                            egui::Color32::WHITE,
                        )
                    };
                    let normal = layout("⌘R", FontFamily::Proportional);
                    let shortcut = layout("⌘R", menu_shortcut_family());
                    let glyphs = &shortcut.rows[0].glyphs;
                    let command = &glyphs[0];
                    let letter = &glyphs[1];
                    let ink_rect = |glyph: &egui::epaint::text::Glyph| {
                        egui::Rect::from_min_size(
                            glyph.pos + glyph.uv_rect.offset,
                            glyph.uv_rect.size,
                        )
                    };
                    let command_rect = ink_rect(command);
                    let letter_rect = ink_rect(letter);
                    // Compare rendered glyph bounds, since fallback-font baselines differ.
                    assert!(
                        (command_rect.height() - letter_rect.height()).abs() <= 1.5,
                        "size={size} scale={pixels_per_point}: {command_rect:?} {letter_rect:?}"
                    );
                    assert!(
                        (command_rect.bottom() - letter_rect.bottom()).abs() <= 1.0,
                        "size={size} scale={pixels_per_point}: {command_rect:?} {letter_rect:?}"
                    );
                    assert!(command_rect.height() < ink_rect(&normal.rows[0].glyphs[0]).height());
                    // At the same origin, all Outfit letters retain identical raster and layout.
                    let letters = "Shift+R";
                    let normal_letters = layout(letters, FontFamily::Proportional);
                    let shortcut_letters = layout(letters, menu_shortcut_family());
                    assert_eq!(
                        normal_letters.rows[0].glyphs,
                        shortcut_letters.rows[0].glyphs
                    );
                    assert_eq!(normal_letters.size(), shortcut_letters.size());
                }
            });
            output.textures_delta.clear();
        }
    }
}
