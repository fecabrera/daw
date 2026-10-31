#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod linux_file_drag;
#[cfg(target_os = "macos")]
mod macos_file_drag;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_titlebar;
#[cfg(target_os = "windows")]
mod windows_file_drag;
#[cfg(target_os = "windows")]
mod windows_titlebar;

struct App {
    #[cfg(target_os = "windows")]
    titlebar: Option<windows_titlebar::Titlebar>,
    #[cfg(target_os = "macos")]
    titlebar: Option<macos_titlebar::Titlebar>,
    ui: daw_ui::DawUi,
    #[cfg(target_os = "macos")]
    menu: macos_menu::NativeMenu,
    #[cfg(target_os = "macos")]
    file_drag_pointer: Option<macos_file_drag::FileDragPointer>,
    #[cfg(target_os = "windows")]
    file_drag_pointer: Option<windows_file_drag::FileDragPointer>,
    #[cfg(target_os = "linux")]
    file_drag_pointer: Option<linux_file_drag::FileDragPointer>,
}
impl eframe::App for App {
    #[cfg(target_os = "windows")]
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        // DWM paints native caption buttons through the unpainted header region.
        [0.0; 4]
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(daw_ui::settings::STORAGE_KEY, self.ui.settings().to_json());
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some(pointer) = &self.file_drag_pointer {
            pointer.update(ctx, input);
        }
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &self.titlebar {
            titlebar.pointer_input(input);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.prepare(&mut self.ui, ui.ctx());
        }
        #[cfg(target_os = "macos")]
        self.menu.dispatch(&mut self.ui, ui.ctx());
        #[cfg(target_os = "macos")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.update(self.ui.settings().titlebar);
        }
        self.ui.show(ui);
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.finish(&self.ui, ui.ctx());
        }
        #[cfg(target_os = "macos")]
        self.menu.update_enabled(&self.ui, ui.ctx());
    }
}
fn main() -> eframe::Result {
    daw_ui::configure_preparation_cache(daw_ui::preparation_cache_directory());
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--font-license")
    {
        print!("{}", daw_ui::fonts::LICENSE);
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let project = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([900.0, 600.0]);
    #[cfg(target_os = "windows")]
    let viewport = viewport.with_decorations(true).with_transparent(true);
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false)
        .with_titlebar_buttons_shown(true);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "windows")]
    let options = eframe::NativeOptions {
        // The OpenGL framebuffer retains alpha for DWM's native caption area.
        // An opaque DXGI surface would cover the OS-drawn buttons.
        renderer: eframe::Renderer::Glow,
        ..options
    };
    eframe::run_native(
        "DAW",
        options,
        Box::new(move |cc| {
            daw_ui::theme::configure(&cc.egui_ctx);
            daw_ui::fonts::configure(&cc.egui_ctx);
            daw_ui::icons::configure(&cc.egui_ctx);
            let stored = cc
                .storage
                .and_then(|storage| storage.get_string(daw_ui::settings::STORAGE_KEY));
            let settings = daw_ui::settings::AppSettings::from_json(stored.as_deref());
            let mut ui = daw_ui::DawUi::new_with_audio_defaults(project, settings.audio_quality);
            ui.set_settings(&cc.egui_ctx, settings);
            #[cfg(target_os = "macos")]
            let (ui, menu) = {
                let mut ui = ui;
                let menu = macos_menu::NativeMenu::new(cc.egui_ctx.clone());
                ui.use_native_menu();
                menu.update_enabled(&ui, &cc.egui_ctx);
                (ui, menu)
            };
            #[cfg(target_os = "macos")]
            let titlebar = macos_titlebar::Titlebar::new(cc, ui.settings().titlebar);
            #[cfg(target_os = "windows")]
            let titlebar = windows_titlebar::Titlebar::new(cc);
            #[cfg(target_os = "windows")]
            if titlebar.is_none() {
                let mut settings = ui.settings();
                settings.titlebar = daw_ui::settings::TitlebarStyle::Native;
                ui.set_settings(&cc.egui_ctx, settings);
            }
            Ok(Box::new(App {
                #[cfg(target_os = "macos")]
                titlebar,
                #[cfg(target_os = "windows")]
                titlebar,
                ui,
                #[cfg(target_os = "macos")]
                menu,
                #[cfg(target_os = "macos")]
                file_drag_pointer: macos_file_drag::FileDragPointer::new(cc),
                #[cfg(target_os = "windows")]
                file_drag_pointer: windows_file_drag::FileDragPointer::new(cc),
                #[cfg(target_os = "linux")]
                file_drag_pointer: linux_file_drag::FileDragPointer::new(cc),
            }))
        }),
    )
}
