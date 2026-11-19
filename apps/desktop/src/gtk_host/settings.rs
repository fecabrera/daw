//! Secondary GTK window with independent input, OpenGL, and an egui context.
use super::*;
use daw_ui::{DesktopPicker, DesktopWindow, settings::PickerKind};

#[derive(Default)]
pub(super) struct Host {
    child: Option<Child>,
    picker: Option<(gtk4::FileChooserNative, Rc<Cell<bool>>)>,
}

struct Child {
    key: DesktopWindow,
    size: [u32; 2],
    window: gtk4::ApplicationWindow,
    area: gtk4::GLArea,
    opened: Instant,
    painted: Rc<Cell<bool>>,
    repaint: Arc<AtomicU64>,
    repaint_clock: Instant,
    root_renderer: u32,
    suspended: bool,
}

struct Graphics {
    // The painter is destroyed with the GL context current.
    painter: egui_glow::Painter,
    _library: libloading::Library,
}

impl Host {
    pub fn update(
        &mut self,
        parent: &gtk4::ApplicationWindow,
        parent_area: &gtk4::GLArea,
        root: &Rc<RefCell<State>>,
    ) {
        let (key, size, focus, blocked, request, root_renderer, smoke_close) = {
            let mut state = root.borrow_mut();
            let key = state.ui.desktop_window();
            (
                key,
                key.map(|key| state.ui.desktop_window_size(key)),
                key.is_some_and(|key| state.ui.take_desktop_focus(key)),
                key.is_some_and(|key| state.ui.desktop_window_blocked(key)),
                state.ui.take_desktop_picker(),
                state.renderer_generation,
                std::mem::take(&mut state.smoke_close_settings),
            )
        };

        if self.picker.as_ref().is_some_and(|(_, done)| done.get()) {
            self.picker = None;
        }
        if self.child.as_ref().map(|child| child.key) != key {
            if let Some(child) = self.child.take() {
                let restore = child.window.is_active();
                child.window.destroy();
                let mut state = root.borrow_mut();
                state.smoke_settings_presented = false;
                state.ui.finish_settings_retirement();
                drop(state);
                if restore && key.is_none() {
                    parent.present();
                    parent_area.grab_focus();
                }
            }
            if let Some((key, size)) = key.zip(size) {
                self.child = Some(Child::new(parent, root, key, size, root_renderer));
            }
        }
        if let Some(child) = &mut self.child {
            if smoke_close && matches!(child.key, DesktopWindow::Settings(_)) {
                child.window.close();
            }
            if child.suspended && !blocked {
                child.opened = Instant::now();
            }
            if child.area.error().is_some()
                || (!blocked
                    && !child.painted.get()
                    && child.opened.elapsed() >= Duration::from_secs(2))
            {
                let mut state = root.borrow_mut();
                state.ui.fail_desktop_window(child.key);
                state.ctx.request_repaint();
                drop(state);
                let restore = child.window.is_active();
                child.window.destroy();
                self.child = None;
                if restore {
                    parent.present();
                    parent_area.grab_focus();
                }
            } else {
                if !child.key.resizable()
                    && let Some(size) = size.filter(|size| *size != child.size)
                {
                    child.size = size;
                    let [width, height] = size.map(|size| size as i32);
                    child.area.set_size_request(width, height);
                    child.window.set_default_size(width, height);
                }
                if root_renderer != child.root_renderer {
                    child.root_renderer = root_renderer;
                    child.window.set_transient_for(None::<&gtk4::Window>);
                    child.window.set_transient_for(Some(parent));
                    if !blocked {
                        child.window.present();
                        child.area.grab_focus();
                    }
                }
                if child.suspended != blocked {
                    child.suspended = blocked;
                    if blocked {
                        child.window.set_visible(false);
                        parent.present();
                    } else {
                        child.window.present();
                        child.area.grab_focus();
                    }
                } else if focus && !blocked {
                    child.window.present();
                    child.area.grab_focus();
                }

                if child.repaint_clock.elapsed().as_millis()
                    >= u128::from(child.repaint.load(Ordering::Relaxed))
                {
                    child.repaint.store(u64::MAX, Ordering::Relaxed);
                    child.area.queue_render();
                }
            }
        }

        if let Some(request) = request {
            let owner = self
                .child
                .as_ref()
                .filter(|child| child.key == request.owner() && child.window.is_visible())
                .map_or(parent, |child| &child.window);
            self.picker = Some(open_chooser(owner, root, request));
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some((picker, _)) = self.picker.take() {
            picker.hide();
        }
        if let Some(child) = self.child.take() {
            child.window.destroy();
        }
    }
}

impl Child {
    fn new(
        parent: &gtk4::ApplicationWindow,
        root: &Rc<RefCell<State>>,
        key: DesktopWindow,
        size: [u32; 2],
        root_renderer: u32,
    ) -> Self {
        let [width, height] = size.map(|size| size as i32);
        let window = gtk4::ApplicationWindow::builder()
            .title(key.title())
            .decorated(true)
            .resizable(key.resizable())
            .default_width(width)
            .default_height(height)
            .transient_for(parent)
            .destroy_with_parent(true)
            .build();
        if let Some(app) = parent.application() {
            window.set_application(Some(&app));
        }
        let area = gtk4::GLArea::builder()
            .hexpand(true)
            .vexpand(true)
            .focusable(true)
            .auto_render(false)
            .build();
        if key.resizable() {
            area.set_size_request(420, 320);
        } else {
            area.set_size_request(width, height);
        }
        area.set_required_version(3, 3);
        area.set_use_es(false);
        area.connect_resize(|area, _, _| area.queue_render());
        window.set_child(Some(&area));

        let ctx = egui::Context::default();
        crate::ui_startup::configure_context(&ctx);
        titlebar::enable_linux_custom(&ctx);
        let input = Rc::new(RefCell::new(input::Surface::new(ctx.clone())));
        input::connect_input(&area, &window, &input, |_| false, false);

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
        let painted = Rc::new(Cell::new(false));
        let graphics = Rc::new(RefCell::new(None::<Graphics>));
        let render_graphics = graphics.clone();
        let weak_root = Rc::downgrade(root);
        let weak_window = window.downgrade();
        let rendered = painted.clone();
        let textures = RefCell::new(textures::Textures::default());
        let started = Instant::now();
        area.connect_render(move |area, _| {
            let (Some(root), Some(window)) = (weak_root.upgrade(), weak_window.upgrade()) else {
                return glib::Propagation::Stop;
            };
            let Ok(mut state) = root.try_borrow_mut() else {
                ctx.request_repaint();
                return glib::Propagation::Stop;
            };
            if state.ui.desktop_window() != Some(key) {
                return glib::Propagation::Stop;
            }
            let mut graphics = render_graphics.borrow_mut();
            if graphics.is_none() {
                match make_painter() {
                    Ok((mut painter, library)) => {
                        textures.borrow().restore(&mut painter);
                        *graphics = Some(Graphics {
                            painter,
                            _library: library,
                        });
                    }
                    Err(error) => {
                        tracing::warn!(%error, title = key.title(), "Cannot initialize GTK dialog renderer");
                        state.ui.fail_desktop_window(key);
                        state.ctx.request_repaint();
                        return glib::Propagation::Stop;
                    }
                }
            }
            let Some(graphics) = graphics.as_mut() else {
                return glib::Propagation::Stop;
            };
            let mut raw = std::mem::take(&mut input.borrow_mut().input);
            let zoom = ctx.zoom_factor();
            raw.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(area.width() as f32 / zoom, area.height() as f32 / zoom),
            ));
            raw.time = Some(started.elapsed().as_secs_f64());
            raw.focused = window.is_active();
            let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
            viewport.native_pixels_per_point = Some(area.scale_factor() as f32);
            viewport.focused = Some(window.is_active());
            let root_ctx = state.ctx.clone();
            let mut output = ctx.run_ui(raw, |ui| state.ui.show_desktop_window(key, &root_ctx, ui));
            drop(state);
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            textures.borrow_mut().apply(&output.textures_delta);
            let size = [
                (area.width() * area.scale_factor()) as u32,
                (area.height() * area.scale_factor()) as u32,
            ];
            let color = egui::Rgba::from(daw_ui::theme::palette(&ctx).panel).to_array();
            graphics.painter.clear(size, color);
            graphics.painter.paint_and_update_textures(
                size,
                output.pixels_per_point,
                &primitives,
                &mut output.textures_delta,
            );
            forward_platform_output(area, output.platform_output.commands);
            area.set_cursor_from_name(Some(cursor_name(output.platform_output.cursor_icon)));
            rendered.set(true);
            if let Ok(mut state) = root.try_borrow_mut()
                && state.smoke && matches!(key, DesktopWindow::Settings(_))
            {
                state.smoke_settings_presented = true;
            }
            glib::Propagation::Stop
        });
        area.connect_unrealize(move |area| {
            area.make_current();
            if let Some(mut graphics) = graphics.borrow_mut().take()
                && area.error().is_none()
            {
                graphics.painter.destroy();
            }
        });
        let weak_root = Rc::downgrade(root);
        window.connect_close_request(move |_| {
            if let Some(root) = weak_root.upgrade() {
                let mut state = root.borrow_mut();
                state.ui.close_desktop_window(key);
                state.ctx.request_repaint();
            }
            glib::Propagation::Stop
        });
        window.present();
        area.grab_focus();
        Self {
            key,
            size,
            window,
            area,
            opened: Instant::now(),
            painted,
            repaint,
            repaint_clock,
            root_renderer,
            suspended: false,
        }
    }
}

fn open_chooser(
    owner: &gtk4::ApplicationWindow,
    root: &Rc<RefCell<State>>,
    request: DesktopPicker,
) -> (gtk4::FileChooserNative, Rc<Cell<bool>>) {
    let (title, action) = match request {
        DesktopPicker::Settings(settings) => match settings.kind {
            PickerKind::Model(_) => ("Choose an ONNX model", gtk4::FileChooserAction::Open),
            PickerKind::PluginFolder => (
                "Choose a plugin folder",
                gtk4::FileChooserAction::SelectFolder,
            ),
        },
        DesktopPicker::ProjectLocation(_) => (
            "Choose project location",
            gtk4::FileChooserAction::SelectFolder,
        ),
        DesktopPicker::ExportFile(_, _) => ("Export audio", gtk4::FileChooserAction::Save),
    };
    let dialog = gtk4::FileChooserNative::new(
        Some(title),
        Some(owner),
        action,
        Some(if action == gtk4::FileChooserAction::Save {
            "Export"
        } else {
            "Choose"
        }),
        Some("Cancel"),
    );
    dialog.set_modal(true);
    if matches!(request, DesktopPicker::Settings(settings) if matches!(settings.kind, PickerKind::Model(_)))
    {
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("ONNX model"));
        filter.add_pattern("*.onnx");
        dialog.add_filter(&filter);
    } else if let DesktopPicker::ExportFile(_, format) = request {
        dialog.set_current_name(&format!("mix.{}", format.extension()));
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some(format.label()));
        filter.add_pattern(&format!("*.{}", format.extension()));
        dialog.add_filter(&filter);
    }
    let done = Rc::new(Cell::new(false));
    let completed = done.clone();
    let weak_root = Rc::downgrade(root);
    dialog.connect_response(move |dialog, response| {
        completed.set(true);
        let result = if response == gtk4::ResponseType::Accept {
            dialog
                .file()
                .and_then(|file| file.path())
                .map(Some)
                .ok_or_else(|| "Choose a local file or folder.".to_owned())
        } else {
            Ok(None)
        };
        if let Some(root) = weak_root.upgrade() {
            let mut state = root.borrow_mut();
            state.ui.complete_desktop_picker(request, result);
            state.ctx.request_repaint();
        }
        dialog.hide();
    });
    dialog.show();
    (dialog, done)
}
