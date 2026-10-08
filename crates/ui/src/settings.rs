use egui::{Color32, Context, Pos2, Rect, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "daw.settings";

/// Material Design's 500 shades, in palette order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    Red,
    Pink,
    #[default]
    Purple,
    DeepPurple,
    Indigo,
    Blue,
    LightBlue,
    Cyan,
    Teal,
    Green,
    LightGreen,
    Lime,
    Yellow,
    Amber,
    Orange,
    DeepOrange,
    Brown,
    Grey,
    BlueGrey,
}

impl AccentColor {
    pub const ALL: [Self; 19] = [
        Self::Red,
        Self::Pink,
        Self::Purple,
        Self::DeepPurple,
        Self::Indigo,
        Self::Blue,
        Self::LightBlue,
        Self::Cyan,
        Self::Teal,
        Self::Green,
        Self::LightGreen,
        Self::Lime,
        Self::Yellow,
        Self::Amber,
        Self::Orange,
        Self::DeepOrange,
        Self::Brown,
        Self::Grey,
        Self::BlueGrey,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Pink => "Pink",
            Self::Purple => "Purple",
            Self::DeepPurple => "Deep Purple",
            Self::Indigo => "Indigo",
            Self::Blue => "Blue",
            Self::LightBlue => "Light Blue",
            Self::Cyan => "Cyan",
            Self::Teal => "Teal",
            Self::Green => "Green",
            Self::LightGreen => "Light Green",
            Self::Lime => "Lime",
            Self::Yellow => "Yellow",
            Self::Amber => "Amber",
            Self::Orange => "Orange",
            Self::DeepOrange => "Deep Orange",
            Self::Brown => "Brown",
            Self::Grey => "Grey",
            Self::BlueGrey => "Blue Grey",
        }
    }

    pub const fn color(self) -> Color32 {
        let hex = match self {
            Self::Red => 0xf44336,
            Self::Pink => 0xe91e63,
            Self::Purple => 0x9c27b0,
            Self::DeepPurple => 0x673ab7,
            Self::Indigo => 0x3f51b5,
            Self::Blue => 0x2196f3,
            Self::LightBlue => 0x03a9f4,
            Self::Cyan => 0x00bcd4,
            Self::Teal => 0x009688,
            Self::Green => 0x4caf50,
            Self::LightGreen => 0x8bc34a,
            Self::Lime => 0xcddc39,
            Self::Yellow => 0xffeb3b,
            Self::Amber => 0xffc107,
            Self::Orange => 0xff9800,
            Self::DeepOrange => 0xff5722,
            Self::Brown => 0x795548,
            Self::Grey => 0x9e9e9e,
            Self::BlueGrey => 0x607d8b,
        };
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub accent: AccentColor,
}

impl AppSettings {
    pub fn from_json(json: Option<&str>) -> Self {
        json.and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }
    pub fn to_json(self) -> String {
        serde_json::to_string(&self).expect("Settings contain only serializable enum values")
    }
}

fn swatch(ui: &egui::Ui, rect: Rect, color: Color32) {
    ui.painter().rect_filled(rect, 2.0, color);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, crate::theme::TEXT.gamma_multiply(0.4)),
        StrokeKind::Inside,
    );
}

pub fn show(ctx: &Context, settings: &mut AppSettings) -> bool {
    let popup_was_open = egui::Popup::is_any_open(ctx);
    let close = crate::dialogs::Dialog::new("Settings", "", &[("Close", ())])
        .show_with_content(ctx, |ui| {
            crate::rows::centered(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                ui.label("Accent color");
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                swatch(ui, rect, settings.accent.color());
                egui::ComboBox::from_id_salt("accent_color")
                    .selected_text(settings.accent.name())
                    .width(165.0)
                    .height(300.0)
                    .show_ui(ui, |ui| {
                        for accent in AccentColor::ALL {
                            let selected = settings.accent == accent;
                            let response = ui.add(
                                egui::Button::new("")
                                    .selected(selected)
                                    .min_size(Vec2::new(165.0, crate::toolbars::CONTROL_HEIGHT)),
                            );
                            let rect = Rect::from_center_size(
                                Pos2::new(response.rect.left() + 13.0, response.rect.center().y),
                                Vec2::splat(14.0),
                            );
                            swatch(ui, rect, accent.color());
                            ui.painter().text(
                                Pos2::new(rect.right() + 8.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                accent.name(),
                                egui::TextStyle::Body.resolve(ui.style()),
                                crate::theme::TEXT,
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    selected,
                                    accent.name(),
                                )
                            });
                            if response.clicked() {
                                settings.accent = accent;
                                crate::theme::set_accent(ctx, accent.color());
                                ui.close();
                            }
                        }
                    });
            });
        })
        .is_some();
    close || (!popup_was_open && ctx.input(|input| input.key_pressed(egui::Key::Escape)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &Context,
        settings: &mut AppSettings,
        events: Vec<egui::Event>,
    ) -> (bool, Vec<egui::epaint::ClippedShape>) {
        let mut closed = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| closed = show(ui.ctx(), settings),
        );
        output.textures_delta.clear();
        (closed, output.shapes)
    }

    fn click(ctx: &Context, settings: &mut AppSettings, label: &str) -> bool {
        let shapes = frame(ctx, settings, vec![]).1;
        let point = shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing label: {label}"));
        let button = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            ctx,
            settings,
            vec![egui::Event::PointerMoved(point), button(true)],
        );
        frame(ctx, settings, vec![button(false)]).0
    }

    #[test]
    fn preferences_round_trip_all_colors_and_recover_from_missing_or_invalid_data() {
        for accent in AccentColor::ALL {
            let settings = AppSettings { accent };
            assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
            assert!(!accent.name().contains("500"));
        }
        for json in [
            None,
            Some(""),
            Some("broken"),
            Some("{}"),
            Some(r#"{"accent":"unknown"}"#),
        ] {
            assert_eq!(AppSettings::from_json(json), AppSettings::default());
        }
        let settings = AppSettings::from_json(Some(r#"{"accent":"blue_grey","future":true}"#));
        assert_eq!(settings.accent, AccentColor::BlueGrey);
    }

    #[test]
    fn dropdown_paints_named_swatches_on_the_left_and_updates_the_accent() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        let mut settings = AppSettings::default();
        frame(&ctx, &mut settings, vec![]);
        assert!(!click(&ctx, &mut settings, "Purple"));
        for _ in 0..12 {
            frame(&ctx, &mut settings, vec![]);
        }
        let shapes = frame(&ctx, &mut settings, vec![]).1;
        for accent in AccentColor::ALL {
            let label = shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == accent.name() => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing option: {}", accent.name()));
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == accent.color() && rect.rect.size() == Vec2::splat(14.0)
                && rect.rect.right() < label.pos.x && (rect.rect.center().y - (label.pos.y + label.galley.rect.center().y)).abs() < 1.0)), "Missing left swatch: {}", accent.name());
        }
        assert!(!click(&ctx, &mut settings, "Red"));
        assert_eq!(settings.accent, AccentColor::Red);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        assert!(!egui::Popup::is_any_open(&ctx));
        assert!(click(&ctx, &mut settings, "Close"));
    }

    #[test]
    fn accents_update_all_themed_controls_without_leaking_between_contexts() {
        let a = Context::default();
        let b = Context::default();
        crate::theme::configure(&a);
        crate::theme::configure(&b);
        let initial = crate::theme::ruler_selection(&a, true);
        crate::theme::set_accent(&a, AccentColor::Teal.color());
        let visuals = a.global_style().visuals.clone();
        assert_eq!(visuals.hyperlink_color, AccentColor::Teal.color());
        assert_eq!(visuals.text_cursor.stroke.color, AccentColor::Teal.color());
        assert_eq!(visuals.selection.stroke.color, AccentColor::Teal.color());
        assert_eq!(
            visuals.widgets.active.fg_stroke.color,
            AccentColor::Teal.color()
        );
        assert_eq!(
            visuals.widgets.open.bg_stroke.color,
            AccentColor::Teal.color()
        );
        assert_eq!(
            crate::theme::selection_outline(&a).color,
            AccentColor::Teal.color()
        );
        assert_ne!(crate::theme::ruler_selection(&a, true), initial);
        assert_eq!(crate::theme::accent(&b), AccentColor::Purple.color());
    }

    #[test]
    fn escape_closes_the_dropdown_first_then_the_dialog() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings::default();
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, "Purple");
        let key = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        assert!(!frame(&ctx, &mut settings, vec![key(true)]).0);
        assert!(!egui::Popup::is_any_open(&ctx));
        frame(&ctx, &mut settings, vec![key(false)]);
        assert!(frame(&ctx, &mut settings, vec![key(true)]).0);
    }
}
