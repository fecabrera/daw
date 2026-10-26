//! Logical geometry shared by unit shells and headerless plugin bodies.
pub const UNIT_STRIP_WIDTH: f32 = 90.0;
pub const PANEL_MARGIN: i8 = 10;
pub const COMPACT_BUTTON_SIZE: f32 = 18.0;
pub const HEADER_FONT_SIZE: f32 = 11.0;
/// Three standard knob rows and a footer, before margins and any host header.
pub const fn unit_control_height() -> f32 {
    3.0 * crate::knobs::CHOICE_SIZE
        + 3.0 * crate::theme::TOOLBAR_PADDING as f32
        + COMPACT_BUTTON_SIZE
}
