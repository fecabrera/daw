#[cfg(target_os = "macos")]
mod macos_menu;

struct App {
    ui: daw_ui::DawUi,
    #[cfg(target_os = "macos")]
    menu: macos_menu::NativeMenu,
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        self.menu.dispatch(&mut self.ui);
        self.ui.show(ui);
        #[cfg(target_os = "macos")]
        self.menu.update_enabled(&self.ui);
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
            let ui = daw_ui::DawUi::new(project);
            #[cfg(target_os = "macos")]
            let (ui, menu) = {
                let mut ui = ui;
                let menu = macos_menu::NativeMenu::new(cc.egui_ctx.clone());
                ui.use_native_file_menu();
                menu.update_enabled(&ui);
                (ui, menu)
            };
            Ok(Box::new(App {
                ui,
                #[cfg(target_os = "macos")]
                menu,
            }))
        }),
    )
}
