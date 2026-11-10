//! GTK owns the top-level window, controls and GLArea. All GTK and GL access stays
//! on its main thread; the DAW's existing audio and background workers stay shared.
use daw_ui::{
    DawUi,
    settings::{AppSettings, TitlebarStyle},
    titlebar::{self, CaptionMetrics},
};
use egui_glow::glow;
use gtk4::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[path = "gtk_host/input.rs"]
mod input;
#[path = "gtk_host/preferences.rs"]
mod preferences;
#[path = "gtk_host/textures.rs"]
mod textures;
#[path = "gtk_host/touch.rs"]
mod touch;

struct State {
    ui: DawUi,
    ctx: egui::Context,
    input: egui::RawInput,
    painter: Option<egui_glow::Painter>,
    textures: textures::Textures,
    renderer_generation: u32,
    // The loader must outlive every GL entry point used by the painter.
    gl_library: Option<libloading::Library>,
    started: Instant,
    pointer: egui::Pos2,
    close_requested: bool,
    close_approved: bool,
    smoke: bool,
    saved: String,
    last_saved: Instant,
    window: preferences::WindowState,
}

pub fn run() -> glib::ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--font-license")
    {
        print!("{}", daw_ui::fonts::LICENSE);
        return glib::ExitCode::SUCCESS;
    }
    daw_ui::configure_preparation_cache(daw_ui::preparation_cache_directory());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let smoke = std::env::args_os().skip(1).any(|arg| arg == "--smoke-test");
    let passed = Rc::new(Cell::new(false));
    let project = (!smoke)
        .then(|| std::env::args_os().nth(1).map(std::path::PathBuf::from))
        .flatten();
    let app = gtk4::Application::builder()
        .application_id("org.daw.DAW")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let result = passed.clone();
    app.connect_activate(move |application| {
        build(application, project.clone(), smoke.then(|| result.clone()))
    });
    // GTK must not parse the optional project path as application command-line options.
    let code = app.run_with_args::<&str>(&[]);
    if smoke && !passed.get() {
        glib::ExitCode::FAILURE
    } else {
        code
    }
}

fn build(
    application: &gtk4::Application,
    project: Option<std::path::PathBuf>,
    smoke: Option<Rc<Cell<bool>>>,
) {
    let window = gtk4::ApplicationWindow::builder()
        .application(application)
        .title("DAW")
        .default_width(1280)
        .default_height(800)
        .build();
    window.set_size_request(900, 600);
    let area = gtk4::GLArea::builder()
        .hexpand(true)
        .vexpand(true)
        .focusable(true)
        .auto_render(false)
        .build();
    area.set_required_version(3, 3);
    area.set_use_es(false);
    area.connect_resize(|area, _, _| area.queue_render());
    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&area));
    let left = gtk4::WindowControls::new(gtk4::PackType::Start);
    let right = gtk4::WindowControls::new(gtk4::PackType::End);
    for (controls, align) in [(&left, gtk4::Align::Start), (&right, gtk4::Align::End)] {
        controls.set_halign(align);
        controls.set_valign(gtk4::Align::Start);
        controls.set_margin_start(4);
        controls.set_margin_end(4);
        overlay.add_overlay(controls);
    }
    // An empty custom titlebar keeps GTK's client-side resize frame in Custom
    // mode. Native mode leaves the titlebar unset and uses GTK's regular chrome.
    let titlebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&overlay));

    let ctx = egui::Context::default();
    daw_ui::theme::configure(&ctx);
    daw_ui::fonts::configure(&ctx);
    daw_ui::icons::configure(&ctx);
    titlebar::enable_linux_custom(&ctx);
    let loaded = if smoke.is_some() {
        preferences::Loaded::default()
    } else {
        preferences::load()
    };
    let settings = loaded.settings;
    window.set_default_size(loaded.window.width.max(900), loaded.window.height.max(600));
    if loaded.window.maximized {
        window.maximize();
    }
    if loaded.window.fullscreen {
        window.fullscreen();
    }
    let mut ui = DawUi::new_with_project_defaults(
        project,
        settings.audio_quality,
        settings.default_stretch_engine,
    );
    ui.set_settings(&ctx, settings);
    let state = Rc::new(RefCell::new(State {
        ui,
        ctx: ctx.clone(),
        input: Default::default(),
        painter: None,
        textures: textures::Textures::default(),
        renderer_generation: 0,
        gl_library: None,
        started: Instant::now(),
        pointer: egui::Pos2::ZERO,
        close_requested: false,
        close_approved: false,
        smoke: smoke.is_some(),
        saved: String::new(),
        last_saved: Instant::now() - Duration::from_secs(1),
        window: loaded.window,
    }));
    let repaint = Arc::new(AtomicU64::new(0));
    let wake = repaint.clone();
    let repaint_clock = Instant::now();
    ctx.set_request_repaint_callback(move |info| {
        let deadline = repaint_clock
            .elapsed()
            .as_millis()
            .saturating_add(info.delay.as_millis())
            .min(u128::from(u64::MAX)) as u64;
        wake.fetch_min(deadline, Ordering::Relaxed);
    });
    input::connect_input(&area, &window, &state);

    let smoke_controls = (left.clone(), right.clone(), titlebar_box.clone());
    let weak_window = window.downgrade();
    let render_state = state.clone();
    area.connect_render(move |area, _| {
        let Some(window) = weak_window.upgrade() else {
            return glib::Propagation::Stop;
        };
        if let Some(error) = area.error() {
            tracing::error!(%error, "GTK OpenGL context failed");
            return glib::Propagation::Stop;
        }
        let mut state = render_state.borrow_mut();
        if state.painter.is_none() {
            match make_painter() {
                Ok((mut painter, library)) => {
                    state.textures.restore(&mut painter);
                    state.renderer_generation += 1;
                    state.painter = Some(painter);
                    state.gl_library = Some(library);
                }
                Err(error) => {
                    tracing::error!(%error, "Cannot initialize GTK renderer");
                    area.set_error(Some(&glib::Error::new(gdk::GLError::NotAvailable, &error)));
                    return glib::Propagation::Stop;
                }
            }
        }
        let custom = state.ui.settings().titlebar == TitlebarStyle::Custom;
        if window.titlebar().is_some() != custom {
            // GTK requires titlebar changes before realization. Recreate only
            // the native surface; the session, workers and egui state stay live.
            let (window, titlebar, left, right, area) = (
                window.clone(),
                titlebar_box.clone(),
                left.clone(),
                right.clone(),
                area.clone(),
            );
            glib::idle_add_local_once(move || {
                let maximized = window.is_maximized();
                let fullscreen = window.is_fullscreen();
                let size = (window.width(), window.height());
                window.set_visible(false);
                gtk4::prelude::WidgetExt::unrealize(&window);
                if custom {
                    window.set_titlebar(Some(&titlebar));
                } else {
                    window.set_titlebar(None::<&gtk4::Widget>);
                }
                left.set_visible(custom);
                right.set_visible(custom);
                if !maximized {
                    window.set_default_size(size.0, size.1);
                }
                window.present();
                if maximized {
                    window.maximize();
                }
                if fullscreen {
                    window.fullscreen();
                }
                area.grab_focus();
                area.queue_render();
            });
        }
        if let Some(settings) = gtk4::Settings::default() {
            let dark = ctx.global_style().visuals.dark_mode;
            if settings.is_gtk_application_prefer_dark_theme() != dark {
                glib::idle_add_local_once(move || {
                    settings.set_gtk_application_prefer_dark_theme(dark)
                });
            }
        }
        let (_, left_width, _, _) = left.measure(gtk4::Orientation::Horizontal, -1);
        let (_, right_width, _, _) = right.measure(gtk4::Orientation::Horizontal, -1);
        let (_, left_height, _, _) = left.measure(gtk4::Orientation::Vertical, -1);
        let (_, right_height, _, _) = right.measure(gtk4::Orientation::Vertical, -1);
        let zoom = ctx.zoom_factor();
        // Empty GTK control groups can still measure CSS padding. Reserve space
        // only for actual buttons, so the menu starts at the shared left margin.
        state.ui.set_caption_metrics(CaptionMetrics {
            leading_width: if left.is_empty() {
                0.0
            } else {
                (left_width + 8) as f32 / zoom
            },
            width: if right.is_empty() {
                0.0
            } else {
                (right_width + 8) as f32 / zoom
            },
            height: left_height.max(right_height) as f32 / zoom,
            top_inset: 0.0,
        });
        let mut input = std::mem::take(&mut state.input);
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(area.width() as f32 / zoom, area.height() as f32 / zoom),
        ));
        input.time = Some(state.started.elapsed().as_secs_f64());
        input.focused = window.is_active();
        let viewport = input.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(area.scale_factor() as f32);
        viewport.maximized = Some(window.is_maximized());
        viewport.fullscreen = Some(window.is_fullscreen());
        viewport.focused = Some(window.is_active());
        if state.close_requested {
            viewport.events.push(egui::ViewportEvent::Close);
            state.close_requested = false;
        }
        let mut output = ctx.run_ui(input, |ui| state.ui.show(ui));
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        state.textures.apply(&output.textures_delta);
        if let Some(painter) = &mut state.painter {
            let size = [
                (area.width() * area.scale_factor()) as u32,
                (area.height() * area.scale_factor()) as u32,
            ];
            let background = egui::Rgba::from(daw_ui::theme::palette(&ctx).background).to_array();
            painter.clear(size, background);
            painter.paint_and_update_textures(
                size,
                output.pixels_per_point,
                &primitives,
                &mut output.textures_delta,
            );
        }
        for command in output.platform_output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => area.clipboard().set_text(&text),
                egui::OutputCommand::OpenUrl(url) => {
                    gtk4::gio::AppInfo::launch_default_for_uri_async(
                        &url.url,
                        None::<&gtk4::gio::AppLaunchContext>,
                        None::<&gtk4::gio::Cancellable>,
                        |result| {
                            if let Err(error) = result {
                                tracing::warn!(%error, "Cannot open URL");
                            }
                        },
                    );
                }
                _ => {}
            }
        }
        let mut close = false;
        if let Some(viewport) = output.viewport_output.get(&egui::ViewportId::ROOT) {
            for command in &viewport.commands {
                match command {
                    egui::ViewportCommand::Title(title) => window.set_title(Some(title)),
                    egui::ViewportCommand::Close => close = true,
                    egui::ViewportCommand::CancelClose => close = false,
                    egui::ViewportCommand::Maximized(value) => {
                        if *value {
                            window.maximize();
                        } else {
                            window.unmaximize();
                        }
                    }
                    egui::ViewportCommand::Minimized(true) => window.minimize(),
                    egui::ViewportCommand::Fullscreen(value) => {
                        if *value {
                            window.fullscreen();
                        } else {
                            window.unfullscreen();
                        }
                    }
                    egui::ViewportCommand::Focus => window.present(),
                    _ => {}
                }
            }
        }
        area.set_cursor_from_name(Some(cursor_name(output.platform_output.cursor_icon)));
        state.window.capture(&window);
        save_settings(&mut state, close);
        if close {
            state.close_approved = true;
            drop(state);
            window.close();
        }
        glib::Propagation::Stop
    });
    let destroy_state = state.clone();
    area.connect_unrealize(move |area| {
        area.make_current();
        let mut state = destroy_state.borrow_mut();
        if let Some(mut painter) = state.painter.take()
            && area.error().is_none()
        {
            painter.destroy();
        }
        state.gl_library = None;
    });
    let close_state = state.clone();
    let weak_area = area.downgrade();
    window.connect_close_request(move |_| {
        let mut state = close_state.borrow_mut();
        // Before the first successful frame there can be no user edits. Let
        // the native close button work even if GL initialization failed.
        if state.close_approved || state.renderer_generation == 0 {
            return glib::Propagation::Proceed;
        }
        state.close_requested = true;
        if let Some(area) = weak_area.upgrade() {
            area.queue_render();
        }
        glib::Propagation::Stop
    });
    let weak_area = area.downgrade();
    // Poll only the repaint flag; worker callbacks never access GTK objects.
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let Some(area) = weak_area.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if repaint_clock.elapsed().as_millis() >= u128::from(repaint.load(Ordering::Relaxed)) {
            repaint.store(u64::MAX, Ordering::Relaxed);
            area.queue_render();
        }
        glib::ControlFlow::Continue
    });
    let custom = state.borrow().ui.settings().titlebar == TitlebarStyle::Custom;
    if custom {
        window.set_titlebar(Some(&smoke_controls.2));
    }
    smoke_controls.0.set_visible(custom);
    smoke_controls.1.set_visible(custom);
    window.present();
    area.grab_focus();
    if let Some(passed) = smoke {
        let mut step = 0;
        glib::timeout_add_local(Duration::from_millis(600), move || {
            let (left, right, _) = &smoke_controls;
            let mut state = state.borrow_mut();
            if state.painter.is_none() || area.error().is_some() {
                tracing::error!("GTK smoke test: renderer unavailable");
                drop(state);
                window.destroy();
                return glib::ControlFlow::Break;
            }
            let native = state.ui.settings().titlebar == TitlebarStyle::Native;
            let correct = window.titlebar().is_none() == native
                && left.is_visible() != native
                && right.is_visible() != native;
            if !correct {
                tracing::error!("GTK smoke test: titlebar visibility mismatch");
                drop(state);
                window.destroy();
                return glib::ControlFlow::Break;
            }
            if step == 0 || step == 1 {
                if step == 1 {
                    save_snapshot(&window, "native.png");
                }
                let mut settings = state.ui.settings();
                settings.titlebar = if step == 0 {
                    TitlebarStyle::Native
                } else {
                    TitlebarStyle::Custom
                };
                let ctx = state.ctx.clone();
                state.ui.set_settings(&ctx, settings);
                ctx.request_repaint();
                step += 1;
                return glib::ControlFlow::Continue;
            }
            if state.renderer_generation < 3 {
                tracing::error!("GTK smoke test: window surfaces were not recreated");
                drop(state);
                window.destroy();
                return glib::ControlFlow::Break;
            }
            save_snapshot(&window, "custom.png");
            passed.set(true);
            eprintln!(
                "GTK smoke test passed: rendered DAW and switched Custom → Native → Custom with real GTK controls."
            );
            state.close_approved = true;
            drop(state);
            window.close();
            glib::ControlFlow::Break
        });
    }
}

fn save_snapshot(window: &gtk4::ApplicationWindow, name: &str) {
    let directory = std::path::Path::new("work/gtk-titlebar");
    let snapshot = gtk4::Snapshot::new();
    let paintable = gtk4::WidgetPaintable::new(Some(window));
    paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
    if let Some(node) = snapshot.to_node()
        && let Some(renderer) = window.renderer()
        && let Err(error) = std::fs::create_dir_all(directory).and_then(|()| {
            renderer
                .render_texture(&node, None)
                .save_to_png(directory.join(name))
                .map_err(std::io::Error::other)
        })
    {
        tracing::warn!(%error, "Cannot save GTK smoke screenshot");
    }
}

fn make_painter() -> Result<(egui_glow::Painter, libloading::Library), String> {
    // SAFETY: the GLArea has a current desktop OpenGL context on this thread.
    // Keep libGL loaded until after the painter is destroyed during unrealize.
    unsafe {
        let library = libloading::Library::new("libGL.so.1").map_err(|e| e.to_string())?;
        let get_proc: libloading::Symbol<
            '_,
            unsafe extern "C" fn(*const u8) -> *const std::ffi::c_void,
        > = library
            .get(b"glXGetProcAddressARB\0")
            .map_err(|e| e.to_string())?;
        let gl = glow::Context::from_loader_function(|name| {
            let name = std::ffi::CString::new(name).expect("OpenGL symbol has no NUL");
            get_proc(name.as_ptr().cast())
        });
        let painter =
            egui_glow::Painter::new(Arc::new(gl), "", None, false).map_err(|e| e.to_string())?;
        Ok((painter, library))
    }
}

fn save_settings(state: &mut State, force: bool) {
    if state.smoke || (!force && state.last_saved.elapsed() < Duration::from_secs(1)) {
        return;
    }
    state.last_saved = Instant::now();
    let json = preferences::encode(state.ui.settings(), state.window);
    if state.saved == json {
        return;
    }
    match preferences::save(&preferences::path(), &json) {
        Ok(()) => state.saved = json,
        Err(error) => tracing::warn!(%error, "Cannot save application preferences"),
    }
}

fn cursor_name(cursor: egui::CursorIcon) -> &'static str {
    match cursor {
        egui::CursorIcon::Text | egui::CursorIcon::VerticalText => "text",
        egui::CursorIcon::PointingHand => "pointer",
        egui::CursorIcon::Grab => "grab",
        egui::CursorIcon::Grabbing => "grabbing",
        egui::CursorIcon::ResizeHorizontal => "ew-resize",
        egui::CursorIcon::ResizeVertical => "ns-resize",
        egui::CursorIcon::Crosshair => "crosshair",
        egui::CursorIcon::NotAllowed => "not-allowed",
        _ => "default",
    }
}
