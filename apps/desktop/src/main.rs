#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app_icon;
#[cfg(target_os = "linux")]
mod gtk_host;
mod hosting;
#[cfg(target_os = "macos")]
mod macos_file_drag;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_titlebar;
mod midi;
#[cfg(not(target_os = "linux"))]
mod settings_host;
#[cfg(all(target_os = "macos", debug_assertions))]
mod settings_smoke;
mod ui_startup;
#[cfg(target_os = "windows")]
mod windows_file_drag;
#[cfg(target_os = "windows")]
mod windows_titlebar;

#[cfg(not(target_os = "linux"))]
struct App {
    #[cfg(target_os = "windows")]
    titlebar: Option<windows_titlebar::Titlebar>,
    #[cfg(target_os = "macos")]
    titlebar: Option<macos_titlebar::Titlebar>,
    ui: std::rc::Rc<std::cell::RefCell<daw_ui::DawUi>>,
    settings_host: settings_host::Host,
    #[cfg(all(target_os = "macos", debug_assertions))]
    settings_smoke: Option<settings_smoke::Smoke>,
    hosting: hosting::Hosting,
    #[cfg(target_os = "macos")]
    menu: macos_menu::NativeMenu,
    #[cfg(target_os = "macos")]
    file_drag_pointer: Option<macos_file_drag::FileDragPointer>,
    #[cfg(target_os = "windows")]
    file_drag_pointer: Option<windows_file_drag::FileDragPointer>,
}
#[cfg(not(target_os = "linux"))]
impl Drop for App {
    fn drop(&mut self) {
        self.hosting.shutdown(&mut self.ui.borrow_mut());
    }
}
#[cfg(not(target_os = "linux"))]
impl eframe::App for App {
    #[cfg(target_os = "windows")]
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        // DWM paints native caption buttons through the unpainted header region.
        [0.0; 4]
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(
            daw_ui::settings::STORAGE_KEY,
            self.ui.borrow().settings().to_json(),
        );
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if input.viewport_id != egui::ViewportId::ROOT {
            return;
        }
        if let Some(pointer) = &self.file_drag_pointer {
            pointer.update(ctx, input);
        }
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &self.titlebar {
            titlebar.pointer_input(input);
        }
        #[cfg(target_os = "macos")]
        if let Some(titlebar) = &self.titlebar {
            titlebar.prepare_input();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some((request, result)) = self.settings_host.poll(&ctx) {
            self.ui
                .borrow_mut()
                .complete_desktop_picker(request, Ok(result));
        }
        let mut daw = self.ui.borrow_mut();
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.prepare(&mut daw, &ctx);
        }
        #[cfg(target_os = "macos")]
        self.menu.dispatch(&mut daw, &ctx);
        #[cfg(target_os = "macos")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.update(daw.settings().titlebar);
        }
        self.hosting.update(&mut daw, &ctx);
        daw.show(ui);
        #[cfg(target_os = "macos")]
        if let Some(titlebar) = &self.titlebar {
            titlebar.finish(ui.ctx());
        }
        #[cfg(target_os = "windows")]
        if let Some(titlebar) = &mut self.titlebar {
            titlebar.finish(&daw, &ctx);
        }
        #[cfg(target_os = "macos")]
        self.menu.update_enabled(&daw, &ctx);
        drop(daw);
        self.settings_host
            .show(&ctx, frame.winit_window().cloned(), &self.ui);
        #[cfg(all(target_os = "macos", debug_assertions))]
        if let Some(smoke) = &mut self.settings_smoke {
            smoke.update(&ctx, &mut self.ui.borrow_mut(), &self.settings_host);
        }
    }
}
#[cfg(target_os = "linux")]
fn main() -> gtk4::glib::ExitCode {
    gtk_host::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> eframe::Result {
    daw_plugin_host::dispatch_embedded_scan();
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
    #[cfg(all(target_os = "macos", debug_assertions))]
    let smoke = project
        .as_ref()
        .and_then(|arg| settings_smoke::Smoke::from_arg(arg.as_os_str()));
    #[cfg(all(target_os = "macos", debug_assertions))]
    let testing = smoke.is_some();
    #[cfg(not(all(target_os = "macos", debug_assertions)))]
    let testing = false;
    let project = project.filter(|_| !testing);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([900.0, 600.0])
        .with_icon(app_icon::window_icon());
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
        persistence_path: testing.then(|| {
            std::env::temp_dir().join(format!("daw-settings-smoke-{}.ron", std::process::id()))
        }),
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
            crate::ui_startup::configure_context(&cc.egui_ctx);
            let stored = cc
                .storage
                .and_then(|storage| storage.get_string(daw_ui::settings::STORAGE_KEY));
            let settings = daw_ui::settings::AppSettings::from_json(stored.as_deref());
            let mut ui = crate::ui_startup::create_ui(&cc.egui_ctx, project, settings, !testing);
            if !testing && let Some(root) = midi::recovery_root() {
                ui.enable_midi(Box::new(midi::NativeMidi::default()), root);
            }
            #[cfg(target_os = "macos")]
            let (mut ui, menu) = {
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
            ui.enable_native_windows();
            let hosting = hosting::Hosting::new(&mut ui);
            let ui = std::rc::Rc::new(std::cell::RefCell::new(ui));
            let settings_host = settings_host::Host::new(&ui);
            Ok(Box::new(App {
                hosting,
                settings_host,
                #[cfg(all(target_os = "macos", debug_assertions))]
                settings_smoke: smoke,
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
            }))
        }),
    )
}
