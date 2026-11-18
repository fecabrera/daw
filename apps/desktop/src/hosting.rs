//! Application-main native service pump, catalog refresh and editor ownership.
#[path = "hosting/window.rs"]
mod window;
use daw_core::Id;
use daw_plugin_host::*;
use daw_ui::{DawUi, PluginRequest, PluginSnapshot};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
struct Editor {
    window: window::Window,
    native: Id,
    size: (u32, u32),
    focus: bool,
    scale: f64,
}
struct Capture {
    snapshot: PluginSnapshot,
    native: Id,
    started: Instant,
}
pub struct Hosting {
    service: HostService,
    client: Arc<HostClient>,
    missing: std::collections::HashSet<PluginIdentity>,
    scan: mpsc::Receiver<Result<(PluginRegistry, Vec<String>)>>,
    cancel: Arc<AtomicBool>,
    scan_running: bool,
    folders: Vec<SearchRoot>,
    default_roots: Vec<SearchRoot>,
    pending_scan: Option<ScanMode>,
    editors: HashMap<Id, Editor>,
    captures: Vec<Capture>,
    bases: HashMap<(Id, u32), u64>,
}
impl Hosting {
    pub fn new(ui: &mut DawUi) -> Self {
        let registry = default_registry_path()
            .and_then(|p| PluginRegistry::load(&p))
            .unwrap_or_default();
        ui.set_plugin_catalog(&registry);
        let (service, client) = HostService::new(registry);
        ui.enable_plugin_hosting(Arc::new(client.allow_missing([])));
        let (_, scan) = mpsc::sync_channel(1);
        let mut hosting = Self {
            service,
            client,
            missing: Default::default(),
            scan,
            cancel: Arc::new(AtomicBool::new(false)),
            scan_running: false,
            folders: ui.settings().plugin_folders,
            default_roots: Vec::new(),
            pending_scan: None,
            editors: Default::default(),
            captures: Vec::new(),
            bases: Default::default(),
        };
        hosting.start_scan(ui, ScanMode::Changed);
        hosting
    }
    fn start_scan(&mut self, ui: &mut DawUi, mode: ScanMode) {
        let roots = match default_search_roots() {
            Ok(roots) => roots,
            Err(error) => {
                ui.set_plugin_scan_status(
                    self.default_roots.clone(),
                    false,
                    Some(error.to_string()),
                );
                return;
            }
        };
        self.default_roots = roots.clone();
        let folders = self.folders.clone();
        let mut combined = roots;
        for root in &self.folders {
            if root.path.is_absolute() && !combined.contains(root) {
                combined.push(root.clone());
            }
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        self.scan = receiver;
        self.scan_running = true;
        ui.set_plugin_scan_status(self.default_roots.clone(), true, None);
        std::thread::spawn(move || {
            let result = save_plugin_folders(&folders)
                .and_then(|()| std::env::current_exe().map_err(Error::from))
                .and_then(|path| {
                    refresh_registry(&path, &combined, mode, || cancel.load(Ordering::Relaxed))
                });
            let _ = sender.send(result);
        });
    }
    pub fn shutdown(&mut self, ui: &mut DawUi) {
        self.cancel.store(true, Ordering::Relaxed);
        self.service.shutdown();
        ui.shutdown_plugin_audio();
        for (_, editor) in self.editors.drain() {
            let _ = self.service.native.close_editor(editor.native);
            drop(editor);
        }
        self.service.native.pump(|_, _| {});
    }
    pub fn update(&mut self, ui: &mut DawUi, context: &egui::Context) {
        let folders = ui.settings().plugin_folders;
        if folders != self.folders {
            self.folders = folders;
            self.pending_scan = Some(ScanMode::Changed);
        }
        let completed = match self.scan.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) if self.scan_running => {
                Some(Err(Error::Invalid("Plugin scan worker stopped".into())))
            }
            _ => None,
        };
        if let Some(result) = completed {
            self.scan_running = false;
            match result {
                Ok((registry, warnings)) => {
                    ui.set_plugin_catalog(&registry);
                    self.service.registry = registry;
                    ui.set_plugin_scan_status(
                        self.default_roots.clone(),
                        false,
                        (!warnings.is_empty()).then(|| warnings.join("\n")),
                    );
                }
                Err(error) => ui.set_plugin_scan_status(
                    self.default_roots.clone(),
                    false,
                    Some(error.to_string()),
                ),
            }
        }
        let mut events = Vec::new();
        self.service.pump(|unit, event| events.push((unit, event)));
        for (unit, event) in events {
            if let NativeNotification::Resize(w, h) = event
                && let Some(editor) = self.editors.get_mut(&unit)
            {
                match self.service.native.resize_editor(editor.native, w, h) {
                    Ok((w, h)) => {
                        editor.window.resize(w, h);
                        editor.size = (w, h);
                    }
                    Err(error) => ui.plugin_diagnostic(error),
                }
            }
            if let NativeNotification::EditorClosed = event
                && let Some(editor) = self.editors.remove(&unit)
            {
                let _ = self.service.native.close_editor(editor.native);
            }
            if event == NativeNotification::TextChanged
                && let Some(native) = self.service.current_native(unit)
            {
                self.bases.retain(|(owner, _), _| *owner != native);
            }
            if let NativeNotification::Telemetry(parameter, value) = event
                && let Some(native) = self.service.current_native(unit)
                && let Ok(text) = self.service.native.parameter_text(native, parameter, value)
            {
                ui.set_plugin_parameter_text(context, unit, parameter, value, text);
            }
            ui.plugin_notification(unit, event, context);
        }
        let snapshots = ui.plugin_snapshots();
        let missing: std::collections::HashSet<_> = snapshots
            .iter()
            .filter(|s| {
                self.service.current_native(s.settings.id).is_none()
                    && self
                        .service
                        .registry
                        .resolve(&s.settings.plugin.identity)
                        .is_none()
            })
            .map(|s| s.settings.plugin.identity.clone())
            .collect();
        if missing != self.missing {
            ui.enable_plugin_hosting(Arc::new(self.client.allow_missing(missing.iter().cloned())));
            self.missing = missing;
        }
        let units = snapshots.iter().map(|s| s.settings.id).collect();
        self.service.retain_units(&units);
        let natives: std::collections::HashSet<_> = snapshots
            .iter()
            .filter_map(|s| self.service.current_native(s.settings.id))
            .collect();
        self.bases.retain(|(native, _), _| natives.contains(native));
        // Explicit UI edits synchronize the native controller and manual base
        // atomics. Audio modulation never writes these authored base values.
        for snapshot in &snapshots {
            let unit = snapshot.settings.id;
            let available = self.service.current_native(unit).is_some()
                || self
                    .service
                    .registry
                    .resolve(&snapshot.settings.plugin.identity)
                    .is_some();
            context.data_mut(|d| {
                d.insert_temp(egui::Id::new(("external-available", unit)), available)
            });
            if let Some(native) = self.service.current_native(unit) {
                if let Ok(metadata) = self.service.native.metadata(native) {
                    ui.set_plugin_metadata(context, unit, metadata.clone());
                }
                for parameter in &snapshot.settings.parameters {
                    let key = (native, parameter.definition.id);
                    let value = parameter.base_value;
                    if self.bases.get(&key).copied() != Some(value.to_bits()) {
                        if (snapshot.settings.state.is_some() || self.bases.contains_key(&key))
                            && parameter.edit_revision
                                > snapshot.settings.state.as_ref().map_or(0, |s| s.revision)
                            && !parameter.definition.read_only
                            && let Err(error) = self.service.native.set_parameter(
                                native,
                                parameter.definition.id,
                                value,
                            )
                        {
                            ui.plugin_diagnostic(error);
                            continue;
                        }
                        self.bases.insert(key, value.to_bits());
                        if let Ok(text) = self.service.native.parameter_text(
                            native,
                            parameter.definition.id,
                            value,
                        ) {
                            ui.set_plugin_parameter_text(
                                context,
                                unit,
                                parameter.definition.id,
                                value,
                                text,
                            );
                        }
                    }
                }
            }
        }
        let mut close = Vec::new();
        let mut reattach = Vec::new();
        for (&unit, editor) in &mut self.editors {
            if !units.contains(&unit) || !editor.window.visible() {
                close.push(unit);
                continue;
            }
            if self.service.current_native(unit) != Some(editor.native) {
                reattach.push(unit);
                continue;
            }
            let scale = editor.window.scale(f64::from(context.pixels_per_point()));
            if scale != editor.scale {
                let _ = self.service.native.editor_scale(editor.native, scale);
                editor.scale = scale;
            }
            let size = editor.window.size();
            if size != editor.size {
                match self
                    .service
                    .native
                    .resize_editor(editor.native, size.0, size.1)
                {
                    Ok(actual) => {
                        if actual != size {
                            editor.window.resize(actual.0, actual.1);
                        }
                        editor.size = actual;
                    }
                    Err(_) => editor.window.resize(editor.size.0, editor.size.1),
                }
            }
            let focus = editor.window.focused();
            if focus != editor.focus {
                let _ = self.service.native.editor_focus(editor.native, focus);
                editor.focus = focus;
            }
        }
        for unit in reattach {
            if let Some(snapshot) = snapshots.iter().find(|s| s.settings.id == unit)
                && let Err(error) = self.open_editor(snapshot, context.pixels_per_point() as f64)
            {
                ui.plugin_diagnostic(error);
            }
        }
        for unit in close {
            if let Some(editor) = self.editors.remove(&unit) {
                let _ = self.service.native.close_editor(editor.native);
            }
        }
        for request in ui.take_plugin_requests(context) {
            match request {
                PluginRequest::Scan(mode) => self.pending_scan = Some(mode),
                PluginRequest::CancelScan => {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.pending_scan = None;
                }
                PluginRequest::Editor(unit) => {
                    if let Some(snapshot) = snapshots.iter().find(|s| s.settings.id == unit)
                        && let Err(error) =
                            self.open_editor(snapshot, context.pixels_per_point() as f64)
                    {
                        ui.plugin_diagnostic(error);
                    }
                }
                PluginRequest::Reopen(unit) => {
                    if let Some(editor) = self.editors.remove(&unit) {
                        let _ = self.service.native.close_editor(editor.native);
                    }
                    match self.service.reopen(unit) {
                        Ok(()) => ui.plugin_reopened(unit),
                        Err(e) => ui.plugin_diagnostic(e),
                    }
                }
                PluginRequest::Capture(snapshot) => {
                    let snapshot = *snapshot;
                    if self.service.current_native(snapshot.settings.id).is_none()
                        && self
                            .service
                            .registry
                            .resolve(&snapshot.settings.plugin.identity)
                            .is_none()
                    {
                        ui.plugin_capture_unavailable(snapshot.settings.id);
                        continue;
                    }
                    if self
                        .captures
                        .iter()
                        .any(|c| c.snapshot.settings.id == snapshot.settings.id)
                    {
                        continue;
                    }
                    match self
                        .service
                        .control_instance(&snapshot.settings, &snapshot.states)
                    {
                        Ok(native) => {
                            self.captures.push(Capture {
                                snapshot,
                                native,
                                started: Instant::now(),
                            });
                        }
                        Err(error) => ui.plugin_capture_failed(snapshot.settings.id, error),
                    }
                }
            }
        }
        let mut pending = Vec::new();
        for capture in self.captures.drain(..) {
            match self.service.native.request_suspend(capture.native) {
                Ok(true) => {
                    let result = self
                        .service
                        .native
                        .save_state(capture.native)
                        .and_then(|state| {
                            ui.plugin_state_receipt(&capture.snapshot.settings, state)
                                .map_err(|e| Error::Invalid(e.to_string()))
                        });
                    if let Err(error) = result {
                        ui.plugin_capture_failed(capture.snapshot.settings.id, error);
                    }
                    let _ = self.service.native.resume(capture.native);
                }
                Ok(false) if capture.started.elapsed() < Duration::from_secs(2) => {
                    pending.push(capture)
                }
                Ok(false) => {
                    let _ = self.service.native.resume(capture.native);
                    ui.plugin_capture_failed(
                        capture.snapshot.settings.id,
                        "Plugin state capture did not reach a processing boundary",
                    );
                }
                Err(error) => ui.plugin_capture_failed(capture.snapshot.settings.id, error),
            }
        }
        self.captures = pending;
        if !self.scan_running
            && let Some(mode) = self.pending_scan.take()
        {
            self.start_scan(ui, mode);
        }
        // Keep plugin timers, state barriers and child-window lifetimes serviced
        // even when transport is stopped and no egui control has changed.
        context.request_repaint_after(Duration::from_millis(16));
    }
    fn open_editor(&mut self, snapshot: &PluginSnapshot, scale: f64) -> Result<()> {
        let unit = snapshot.settings.id;
        let native = self
            .service
            .control_instance(&snapshot.settings, &snapshot.states)?;
        if let Some(editor) = self.editors.remove(&unit) {
            self.service.native.close_editor(editor.native)?;
        }
        let window = window::Window::new(&snapshot.settings.plugin.name).map_err(Error::Invalid)?;
        let scale = window.scale(scale);
        let (width, height) = unsafe {
            self.service
                .native
                .open_editor(native, window.parent(), scale)
        }?;
        window.resize(width, height);
        window.resizable(self.service.native.editor_resizable(native)?);
        window.show();
        self.service.native.editor_focus(native, true)?;
        self.editors.insert(
            unit,
            Editor {
                window,
                native,
                size: (width, height),
                focus: true,
                scale,
            },
        );
        Ok(())
    }
}
impl Drop for Hosting {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        for (_, editor) in self.editors.drain() {
            let _ = self.service.native.close_editor(editor.native);
            drop(editor);
        }
    }
}
