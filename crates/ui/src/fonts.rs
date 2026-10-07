use egui::epaint::text::{Tag, VariationCoords};
use egui::{Context, FontData, FontDefinitions, FontFamily, FontId, FontTweak};

pub const LICENSE: &str = include_str!("../assets/fonts/OFL.txt");

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Outfit Semibold".into()))
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
    ctx.set_fonts(fonts);
}
