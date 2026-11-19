use crate::{settings::ColorTheme, theme::Palette};
use egui::Color32;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    Background,
    Panel,
    ButtonBackground,
    Input,
    Border,
    Divider,
    Text,
    TimeUnit,
    Secondary,
    Grid,
    GridSubdivision,
}

impl Role {
    const ALL: [Self; 11] = [
        Self::Background,
        Self::Panel,
        Self::ButtonBackground,
        Self::Input,
        Self::Border,
        Self::Divider,
        Self::Text,
        Self::TimeUnit,
        Self::Secondary,
        Self::Grid,
        Self::GridSubdivision,
    ];

    fn color(self, palette: &mut Palette) -> &mut Color32 {
        match self {
            Self::Background => &mut palette.background,
            Self::Panel => &mut palette.panel,
            Self::ButtonBackground => &mut palette.button_background,
            Self::Input => &mut palette.input,
            Self::Border => &mut palette.border,
            Self::Divider => &mut palette.divider,
            Self::Text => &mut palette.text,
            Self::TimeUnit => &mut palette.time_unit,
            Self::Secondary => &mut palette.secondary,
            Self::Grid => &mut palette.grid,
            Self::GridSubdivision => &mut palette.grid_subdivision,
        }
    }
}

pub(super) fn export_stock(base: ColorTheme, mut palette: Palette, mut parent: Palette) -> String {
    let mut colors = BTreeMap::new();
    for role in Role::ALL {
        let color = *role.color(&mut palette);
        if base != ColorTheme::Mist || color != *role.color(&mut parent) {
            colors.insert(
                role,
                format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b()),
            );
        }
    }
    let value = serde_json::json!({
        "version": 1,
        "id": base.id(),
        "name": base.name(),
        "base": base.parent().id(),
        "colors": colors,
    });
    format!("{value:#}\n")
}

#[derive(Clone)]
struct Colors(BTreeMap<Role, Color32>);

impl<'de> Deserialize<'de> for Colors {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Roles;
        impl<'de> de::Visitor<'de> for Roles {
            type Value = Colors;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object with theme color roles")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Colors, A::Error> {
                let mut colors = BTreeMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    let role =
                        Role::deserialize(de::value::StrDeserializer::<A::Error>::new(&key))?;
                    let hex = map
                        .next_value::<String>()
                        .map_err(|error| de::Error::custom(format!("{key}: {error}")))?;
                    let digits = hex
                        .strip_prefix('#')
                        .filter(|s| s.len() == 6 && s.bytes().all(|byte| byte.is_ascii_hexdigit()));
                    let rgb = digits
                        .and_then(|digits| u32::from_str_radix(digits, 16).ok())
                        .ok_or_else(|| de::Error::custom(format!("{key} must be #RRGGBB")))?;
                    let color = Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
                    if colors.insert(role, color).is_some() {
                        return Err(de::Error::custom(format!("duplicate color role {key}")));
                    }
                }
                Ok(Colors(colors))
            }
        }
        deserializer.deserialize_map(Roles)
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    #[serde(rename = "version")]
    _version: u64,
    pub id: String,
    pub name: String,
    pub base: ColorTheme,
    colors: Colors,
}

impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Version {
            version: u64,
        }
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let version =
            serde_json::from_slice::<Version>(bytes).map_err(|error| error.to_string())?;
        if version.version != 1 {
            return Err(format!("Unsupported theme version {}", version.version));
        }
        let mut document =
            serde_json::from_slice::<Self>(bytes).map_err(|error| error.to_string())?;
        if document.id == super::LEGACY_MIDNIGHT_ID {
            document.id = ColorTheme::Midnight.id().into();
        }
        document.name = document.name.trim().to_owned();
        if !(1..=48).contains(&document.name.chars().count())
            || document.name.chars().any(char::is_control)
        {
            return Err("Use a name of 1 to 48 characters without control characters".into());
        }
        if let Some(stock) = ColorTheme::ALL
            .into_iter()
            .find(|stock| stock.id() == document.id)
        {
            if document.name != stock.name() || document.base != stock.parent() {
                return Err(format!(
                    "The {} override requires name {} and base {}",
                    stock.id(),
                    stock.name(),
                    stock.parent().id()
                ));
            }
        } else if !valid_custom_id(&document.id) {
            return Err("Use a stock ID or a custom ID such as user.blue-room".into());
        }
        Ok(document)
    }

    pub fn overlay(&self, mut colors: Palette) -> Palette {
        for (&role, &color) in &self.colors.0 {
            *role.color(&mut colors) = color;
        }
        colors
    }
}

pub(super) fn valid_custom_id(id: &str) -> bool {
    let Some(slug) = id.strip_prefix("user.") else {
        return false;
    };
    let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    (1..=48).contains(&slug.len())
        && slug.bytes().all(|byte| alphanumeric(byte) || byte == b'-')
        && slug.bytes().next().is_some_and(alphanumeric)
        && slug.bytes().last().is_some_and(alphanumeric)
}

pub(super) fn validate_contrast(palette: Palette) -> Result<Palette, String> {
    fn luminance(color: Color32) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    for (label, foreground, minimum) in [
        ("text", palette.text, 4.5),
        ("secondary", palette.secondary, 3.0),
    ] {
        let foreground = luminance(foreground) + 0.05;
        for (surface, background) in [
            ("background", palette.background),
            ("panel", palette.panel),
            ("input", palette.input),
        ] {
            let background = luminance(background) + 0.05;
            let ratio = foreground.max(background) / foreground.min(background);
            if ratio < minimum {
                return Err(format!(
                    "{label} against {surface} has {ratio:.2}:1 contrast. Use at least {minimum}:1."
                ));
            }
        }
    }
    Ok(palette)
}
