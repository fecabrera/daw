#[cfg(target_os = "linux")]
mod linux_file_drag;
#[cfg(target_os = "macos")]
mod macos_file_drag;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "windows")]
mod windows_file_drag;

struct App {
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
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        self.menu.dispatch(&mut self.ui, ui.ctx());
        self.ui.show(ui);
        #[cfg(target_os = "macos")]
        self.menu.update_enabled(&self.ui, ui.ctx());
    }
}
fn main() -> eframe::Result {
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
    let viewport = viewport.with_decorations(true);
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
    eframe::run_native(
        "DAW",
        options,
        Box::new(move |cc| {
            daw_ui::theme::configure(&cc.egui_ctx);
            daw_ui::fonts::configure(&cc.egui_ctx);
            daw_ui::icons::configure(&cc.egui_ctx);
            let mut ui = daw_ui::DawUi::new(project);
            let stored = cc
                .storage
                .and_then(|storage| storage.get_string(daw_ui::settings::STORAGE_KEY));
            ui.set_settings(
                &cc.egui_ctx,
                daw_ui::settings::AppSettings::from_json(stored.as_deref()),
            );
            #[cfg(target_os = "macos")]
            let (ui, menu) = {
                let mut ui = ui;
                let menu = macos_menu::NativeMenu::new(cc.egui_ctx.clone());
                ui.use_native_menu();
                menu.update_enabled(&ui, &cc.egui_ctx);
                (ui, menu)
            };
            Ok(Box::new(App {
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
