//! eframe hosts Settings in one deferred native viewport owned by the DAW session.
use daw_ui::{DawUi, SettingsPicker, settings::PickerKind};
use eframe::native_viewport::{self, Status, Window};
use egui::{Context, ViewportBuilder, ViewportCommand, ViewportId};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    sync::Arc,
    task::{Poll, Wake, Waker},
    time::{Duration, Instant},
};

thread_local! {
    static UI: RefCell<Weak<RefCell<DawUi>>> = const { RefCell::new(Weak::new()) };
}

struct Viewport {
    id: ViewportId,
    generation: u64,
    opened: Instant,
    suspended: bool,
}

type PickerFuture = Pin<Box<dyn Future<Output = Option<rfd::FileHandle>>>>;

enum Chooser {
    Waiting {
        request: SettingsPicker,
        since: Instant,
        use_root: bool,
    },
    Open {
        request: SettingsPicker,
        future: PickerFuture,
        _owner: Arc<Window>,
    },
}

struct Repaint(Context);
impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint_of(ViewportId::ROOT);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint_of(ViewportId::ROOT);
    }
}

pub(super) struct Host {
    app: usize,
    viewport: Option<Viewport>,
    chooser: Option<Chooser>,
    retiring: Option<ViewportId>,
    #[cfg(all(target_os = "macos", debug_assertions))]
    pub startup_error: bool,
}

impl Host {
    pub fn new(ui: &Rc<RefCell<DawUi>>) -> Self {
        UI.with_borrow_mut(|slot| *slot = Rc::downgrade(ui));
        Self {
            app: Rc::as_ptr(ui) as usize,
            viewport: None,
            chooser: None,
            retiring: None,
            #[cfg(all(target_os = "macos", debug_assertions))]
            startup_error: false,
        }
    }

    pub fn poll(&mut self, root: &Context) -> Option<(SettingsPicker, Option<std::path::PathBuf>)> {
        let Some(Chooser::Open {
            future, request, ..
        }) = &mut self.chooser
        else {
            return None;
        };
        let waker = Waker::from(Arc::new(Repaint(root.clone())));
        if let Poll::Ready(result) = future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(&waker))
        {
            let request = *request;
            self.chooser = None;
            Some((request, result.map(|file| file.path().to_owned())))
        } else {
            None
        }
    }

    pub fn show(
        &mut self,
        root: &Context,
        root_window: Option<Arc<Window>>,
        app: &Rc<RefCell<DawUi>>,
    ) {
        if let Some(id) = self.retiring {
            if root.input(|input| input.raw.viewports.contains_key(&id)) {
                root.request_repaint();
            } else {
                self.retiring = None;
                app.borrow_mut().finish_settings_retirement();
            }
        }
        let (generation, root_prompt, focus, picker) = {
            let mut ui = app.borrow_mut();
            (
                ui.native_settings_generation(),
                ui.settings_root_prompt(),
                ui.take_settings_focus(),
                ui.take_settings_picker(),
            )
        };
        if let Some(request) = picker {
            self.chooser = Some(Chooser::Waiting {
                request,
                since: Instant::now(),
                use_root: generation.is_none(),
            });
        }

        if self.viewport.as_ref().map(|viewport| viewport.generation) != generation {
            if let Some(old) = self.viewport.take() {
                let restore = matches!(native_viewport::status(root, old.id),
                    Some(Status::Presented(window)) if
                        window.upgrade().is_some_and(|window| window.has_focus()));
                native_viewport::forget(root, old.id);
                self.retiring = Some(old.id);
                root.request_repaint();
                if restore {
                    root.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
                }
            }
            if let Some(generation) = generation {
                let id = ViewportId::from_hash_of(("application-settings", generation));
                native_viewport::watch(root, id);
                self.viewport = Some(Viewport {
                    id,
                    generation,
                    opened: Instant::now(),
                    suspended: false,
                });
            }
        }
        let mut owner = None;
        let mut suspend_for_chooser = false;
        if let Some(viewport) = &self.viewport {
            let failure = match native_viewport::status(root, viewport.id) {
                Some(Status::Presented(window)) => {
                    owner = window.upgrade();
                    owner.is_none()
                }
                Some(Status::Failed(error)) => {
                    #[cfg(all(target_os = "macos", debug_assertions))]
                    {
                        self.startup_error = true;
                    }
                    tracing::warn!(%error, "Cannot open native Settings");
                    true
                }
                _ => root.embed_viewports() || viewport.opened.elapsed() >= Duration::from_secs(2),
            };
            if failure {
                app.borrow_mut().fail_native_settings(viewport.generation);
                self.retire(root);
                root.request_repaint();
            } else if owner.is_none() {
                root.request_repaint_after(Duration::from_millis(50));
            }
        }

        if let Some(Chooser::Waiting {
            request,
            since,
            use_root,
        }) = &mut self.chooser
        {
            if let Some(child) = owner.take().filter(|_| !*use_root) {
                self.chooser = Some(Self::open_chooser(*request, child));
                root.request_repaint();
            } else if *use_root {
                suspend_for_chooser = true;
                if self
                    .viewport
                    .as_ref()
                    .is_none_or(|viewport| viewport.suspended)
                {
                    if let Some(window) = root_window {
                        self.chooser = Some(Self::open_chooser(*request, window));
                        root.request_repaint();
                    } else {
                        app.borrow_mut().complete_settings_picker(
                            *request,
                            Err("Cannot open the chooser without a desktop window.".into()),
                        );
                        self.chooser = None;
                    }
                } else {
                    root.request_repaint();
                }
            } else if since.elapsed() >= Duration::from_millis(350) || self.viewport.is_none() {
                *use_root = true;
                suspend_for_chooser = true;
                root.request_repaint();
            } else {
                root.request_repaint_after(Duration::from_millis(40));
            }
        } else if let Some(Chooser::Open { _owner, .. }) = &self.chooser {
            suspend_for_chooser = root_window
                .as_ref()
                .is_some_and(|root| Arc::ptr_eq(root, _owner));
        }

        let Some(viewport) = &mut self.viewport else {
            return;
        };
        let suspended = root_prompt || suspend_for_chooser;
        if suspended != viewport.suspended {
            viewport.suspended = suspended;
            if suspend_for_chooser {
                root.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Minimized(false));
            }
            let id = if suspended {
                ViewportId::ROOT
            } else {
                viewport.id
            };
            root.send_viewport_cmd_to(id, ViewportCommand::Focus);
        } else if focus && !suspended {
            root.send_viewport_cmd_to(viewport.id, ViewportCommand::Focus);
        }
        let generation = viewport.generation;
        let app_id = self.app;
        let root_context = root.clone();
        let mut builder = ViewportBuilder::default()
            .with_title("Settings")
            .with_decorations(true)
            .with_transparent(false)
            .with_visible(!suspended)
            .with_inner_size([680.0, 510.0])
            .with_min_inner_size([420.0, 320.0]);
        if cfg!(target_os = "macos") {
            builder = builder
                .with_fullsize_content_view(false)
                .with_titlebar_shown(true)
                .with_title_shown(true)
                .with_titlebar_buttons_shown(true);
        }
        root.show_viewport_deferred(viewport.id, builder, move |ui, class| {
            UI.with_borrow(|slot| {
                let Some(app) = slot
                    .upgrade()
                    .filter(|app| Rc::as_ptr(app) as usize == app_id)
                else {
                    return;
                };
                let Ok(mut daw) = app.try_borrow_mut() else {
                    ui.ctx().request_repaint();
                    return;
                };
                if daw.native_settings_generation() != Some(generation) {
                    return;
                }
                if class == egui::ViewportClass::EmbeddedWindow {
                    daw.fail_native_settings(generation);
                    root_context.request_repaint_of(ViewportId::ROOT);
                } else {
                    daw.show_native_settings(&root_context, ui);
                }
            });
        });
    }

    #[cfg(all(target_os = "macos", debug_assertions))]
    pub fn viewport_id(&self) -> Option<ViewportId> {
        self.viewport.as_ref().map(|viewport| viewport.id)
    }

    fn retire(&mut self, root: &Context) {
        if let Some(viewport) = self.viewport.take() {
            native_viewport::forget(root, viewport.id);
        }
    }

    fn open_chooser(request: SettingsPicker, owner: Arc<Window>) -> Chooser {
        let dialog = rfd::AsyncFileDialog::new().set_parent(owner.as_ref());
        let future: PickerFuture = match request.kind {
            PickerKind::Model(_) => {
                Box::pin(dialog.add_filter("ONNX model", &["onnx"]).pick_file())
            }
            PickerKind::PluginFolder => Box::pin(dialog.pick_folder()),
        };
        Chooser::Open {
            request,
            future,
            _owner: owner,
        }
    }
}
