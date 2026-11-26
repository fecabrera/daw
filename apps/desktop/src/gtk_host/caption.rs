use gtk4::{gdk, glib, prelude::*};

pub(super) struct Style {
    display: gdk::Display,
    settings: gtk4::Settings,
    provider: gtk4::CssProvider,
    dark: bool,
    colors: daw_ui::theme::Palette,
}

impl Style {
    pub(super) fn new(window: &gtk4::ApplicationWindow, ctx: &egui::Context) -> Self {
        let style = Self {
            display: WidgetExt::display(window),
            settings: window.settings(),
            provider: gtk4::CssProvider::new(),
            dark: ctx.theme() == egui::Theme::Dark,
            colors: daw_ui::theme::palette(ctx),
        };
        style.apply_scheme();
        style.apply_colors();
        gtk4::style_context_add_provider_for_display(
            &style.display,
            &style.provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        style
    }

    pub(super) fn update(&mut self, ctx: &egui::Context) {
        let dark = ctx.theme() == egui::Theme::Dark;
        if self.dark != dark {
            self.dark = dark;
            self.apply_scheme();
        }
        let colors = daw_ui::theme::palette(ctx);
        if self.colors != colors {
            self.colors = colors;
            self.apply_colors();
        }
    }

    fn apply_scheme(&self) {
        // Look up the GTK 4.20 property at runtime to keep the GTK 4.8 minimum.
        const SCHEME: &str = "gtk-interface-color-scheme";
        if let Some(property) = self.settings.find_property(SCHEME) {
            let nick = if self.dark { "dark" } else { "light" };
            if let Some(value) = glib::EnumClass::with_type(property.value_type())
                .and_then(|scheme| scheme.to_value_by_nick(nick))
            {
                self.settings.set_property_from_value(SCHEME, &value);
            }
        } else if self.settings.is_gtk_application_prefer_dark_theme() != self.dark {
            self.settings
                .set_gtk_application_prefer_dark_theme(self.dark);
        }
    }

    fn apply_colors(&self) {
        let text = css_color(self.colors.text);
        let hover = css_color(self.colors.button_background);
        let pressed = css_color(self.colors.border);
        self.provider.load_from_data(&format!(
            "
            windowcontrols.daw-caption {{
                padding: 7px 0;
            }}
            windowcontrols.daw-caption button {{
                min-width: 24px;
                min-height: 24px;
                margin: 0 2px;
                padding: 0;
                border: none;
                border-radius: 5px;
                box-shadow: none;
                background: transparent;
                color: {text};
                -gtk-icon-shadow: none;
            }}
            windowcontrols.daw-caption button:hover {{
                background: {hover};
            }}
            windowcontrols.daw-caption button:active {{
                background: {pressed};
            }}
            "
        ));
    }
}

impl Drop for Style {
    fn drop(&mut self) {
        gtk4::style_context_remove_provider_for_display(&self.display, &self.provider);
    }
}

fn css_color(color: egui::Color32) -> String {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub(super) fn matches_foreground(groups: [&gtk4::WindowControls; 2], ctx: &egui::Context) -> bool {
    let expected = gdk::RGBA::parse(css_color(daw_ui::theme::palette(ctx).text))
        .expect("caption color is valid CSS");
    for controls in groups {
        let mut child = controls.first_child();
        while let Some(widget) = child {
            if widget.is::<gtk4::Button>() && widget.style_context().color() != expected {
                return false;
            }
            child = widget.next_sibling();
        }
    }
    true
}
