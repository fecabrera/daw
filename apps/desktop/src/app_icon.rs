//! Platform application identity; called only during main-thread startup.

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn window_icon() -> egui::IconData {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSBundle, ns_string};
        if NSBundle::mainBundle()
            .objectForInfoDictionaryKey(ns_string!("CFBundleIconName"))
            .is_some()
        {
            // eframe treats an explicitly empty icon as "leave the native icon
            // alone". A PNG override would discard the bundle's layered icon.
            return egui::IconData::default();
        }
    }
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../../crates/ui/assets/app-icon/macos.png").as_slice();
    #[cfg(target_os = "windows")]
    let bytes = include_bytes!("../../../crates/ui/assets/app-icon/windows.png").as_slice();
    // Direct Cargo launches have no icon catalog; provide the Apple-rendered
    // static fallback through eframe, which applies it after app creation.
    eframe::icon_data::from_png_bytes(bytes).expect("valid embedded application icon")
}

#[cfg(target_os = "linux")]
pub fn install(display: &gtk4::gdk::Display) {
    let bytes = gtk4::glib::Bytes::from_static(include_bytes!(
        "../../../crates/ui/assets/app-icon/icons.gresource"
    ));
    match gtk4::gio::Resource::from_data(&bytes) {
        Ok(resource) => {
            gtk4::gio::resources_register(&resource);
            gtk4::IconTheme::for_display(display).add_resource_path("/org/daw/DAW/icons/hicolor");
        }
        Err(error) => tracing::warn!(%error, "Cannot register embedded application icons"),
    }
}
