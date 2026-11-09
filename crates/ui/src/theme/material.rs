//! Material Design 2014 family colors, from the same palette as track defaults.
//! https://mui.com/material-ui/customization/color/#color-palette
use egui::Color32;

pub(crate) const GREY_200: Color32 = rgb(0xeeeeee);
pub(super) const BLUE_GREY_200: Color32 = rgb(0xb0bec5);

// Family 400 identifies existing track colors; selection uses the exact 200 shade.
const FAMILIES: [(Color32, Color32); 19] = [
    (rgb(0xff7043), rgb(0xffab91)), // Deep Orange
    (rgb(0xffa726), rgb(0xffcc80)), // Orange
    (rgb(0xffca28), rgb(0xffe082)), // Amber
    (rgb(0xffee58), rgb(0xfff59d)), // Yellow
    (rgb(0xd4e157), rgb(0xe6ee9c)), // Lime
    (rgb(0x9ccc65), rgb(0xc5e1a5)), // Light Green
    (rgb(0x66bb6a), rgb(0xa5d6a7)), // Green
    (rgb(0x26a69a), rgb(0x80cbc4)), // Teal
    (rgb(0x26c6da), rgb(0x80deea)), // Cyan
    (rgb(0x29b6f6), rgb(0x81d4fa)), // Light Blue
    (rgb(0x42a5f5), rgb(0x90caf9)), // Blue
    (rgb(0x5c6bc0), rgb(0x9fa8da)), // Indigo
    (rgb(0x7e57c2), rgb(0xb39ddb)), // Deep Purple
    (rgb(0xab47bc), rgb(0xce93d8)), // Purple
    (rgb(0xec407a), rgb(0xf48fb1)), // Pink
    (rgb(0xef5350), rgb(0xef9a9a)), // Red
    (rgb(0x8d6e63), rgb(0xbcaaa4)), // Brown
    (rgb(0xbdbdbd), GREY_200),
    (rgb(0x78909c), BLUE_GREY_200),
];

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Exact for Material 400 colors; custom RGB colors use the nearest 400 family.
pub(super) fn shade_200(body: Color32) -> Color32 {
    let distance = |base: Color32| {
        [
            body.r() as i32 - base.r() as i32,
            body.g() as i32 - base.g() as i32,
            body.b() as i32 - base.b() as i32,
        ]
        .into_iter()
        .map(|delta| delta * delta)
        .sum::<i32>()
    };
    FAMILIES
        .iter()
        .min_by_key(|(base, _)| distance(*base))
        .map_or(GREY_200, |(_, shade)| *shade)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_track_families_resolve_to_exact_material_200_shades() {
        let expected = [
            0xffab91, 0xffcc80, 0xffe082, 0xfff59d, 0xe6ee9c, 0xc5e1a5, 0xa5d6a7, 0x80cbc4,
            0x80deea, 0x81d4fa, 0x90caf9, 0x9fa8da, 0xb39ddb, 0xce93d8, 0xf48fb1, 0xef9a9a,
        ];
        for (base, expected) in daw_core::DEFAULT_TRACK_COLORS.into_iter().zip(expected) {
            let colors = super::super::clip_colors(base, None);
            let selected = colors.selected_header();
            assert_eq!(selected.header, rgb(expected));
            assert_eq!(selected.body, colors.body);
            assert_eq!(selected.waveform, colors.waveform);
        }
        assert_eq!(shade_200(rgb(0x8d6e63)), rgb(0xbcaaa4));
        assert_eq!(shade_200(rgb(0xbdbdbd)), GREY_200);
        assert_eq!(shade_200(rgb(0x78909c)), BLUE_GREY_200);
        assert_eq!(shade_200(rgb(0x28b4dc)), rgb(0x80deea));
    }
}
