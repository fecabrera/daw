use crate::body as drive;
use daw_drive_plugin::{Parameter, PluginState};
use daw_plugin_support::Snapshot;
use daw_unit_ui::{fonts, theme};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
pub const WIDTH: u32 = drive::SIZE.x as u32;
pub const HEIGHT: u32 = drive::SIZE.y as u32;
const CAPACITY: usize = if 4 * Parameter::COUNT > 256 {
    8 * Parameter::COUNT
} else {
    256
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Begin(Parameter),
    Value(Parameter, f64),
    End(Parameter),
    Presentation([u8; 64]),
}
/// Access this channel only from editor/control threads. The audio callback never locks it.
pub struct Model {
    pub output_meter: Arc<daw_drive_plugin::OutputMeter>,
    pub meter_reset: std::sync::atomic::AtomicU32,
    pub output_reset: std::sync::atomic::AtomicBool,
    state: Snapshot<{ daw_drive_plugin::MAX_STATE_BYTES.div_ceil(8) }>,
    events: Mutex<VecDeque<Edit>>,
    /// Presentation-only band inventory; it never enters audio or parameter state.
    pub visible_band_slots: std::sync::atomic::AtomicU64,
    pub appearance: AtomicU32,
    pub scale: AtomicU32,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            output_meter: Arc::default(),
            meter_reset: std::sync::atomic::AtomicU32::new(0),
            output_reset: std::sync::atomic::AtomicBool::new(false),
            state: Snapshot::new(state_words(PluginState::default())),
            events: Mutex::new(VecDeque::with_capacity(CAPACITY)),
            visible_band_slots: std::sync::atomic::AtomicU64::new(1),
            appearance: AtomicU32::new(0),
            scale: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}
impl Model {
    /// Apply a host-transported VST3 snapshot on the control thread.
    pub fn receive_output(&self, values: [f32; daw_drive_plugin::METER_VALUES]) {
        daw_drive_plugin::receive_meter(&self.output_meter, values);
    }

    pub fn publish(&self, state: PluginState) {
        self.state.publish(state_words(state));
    }
    pub fn current(&self) -> Option<PluginState> {
        let (_, values) = self.state.read()?;
        let bytes: [u8; daw_drive_plugin::MAX_STATE_BYTES] =
            std::array::from_fn(|i| values[i / 8].to_le_bytes()[i % 8]);
        let state = PluginState::decode(&bytes).ok()?;
        Some(state)
    }
    /// A host recall supersedes presentation edits not yet delivered to control.
    pub fn discard_presentation(&self) {
        if let Ok(mut events) = self.events.lock() {
            events.retain(|e| !matches!(e, Edit::Presentation(_)));
        }
    }
    pub fn pop(&self) -> Option<Edit> {
        self.events.lock().ok()?.pop_front()
    }
    pub fn push(&self, edits: &[Edit]) -> bool {
        let Ok(mut queue) = self.events.lock() else {
            return false;
        };
        if queue.len() + edits.len() > CAPACITY {
            return false;
        }
        queue.extend(edits.iter().copied());
        true
    }
    pub fn remaining(&self) -> usize {
        self.events.lock().map_or(0, |q| CAPACITY - q.len())
    }
}
pub struct Editor {
    model: Arc<Model>,
    wake: Box<dyn Fn() + Send>,
    state: PluginState,
    active: [bool; Parameter::COUNT - 1],
    appearance: u32,
}
impl Editor {
    pub fn new(model: Arc<Model>, wake: impl Fn() + Send + 'static) -> Self {
        let state = model.current().unwrap_or_default();
        Self {
            model,
            wake: Box::new(wake),
            state,
            active: [false; Parameter::COUNT - 1],
            appearance: u32::MAX,
        }
    }
    fn finish(&mut self) {
        for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
            if self.active[slot] && self.model.push(&[Edit::End(p)]) {
                self.active[slot] = false;
            }
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.finish();
        (self.wake)();
    }
}
impl daw_plugin_window::App for Editor {
    fn build(
        &mut self,
        ctx: egui::Context,
        _: &mut daw_plugin_window::Frame,
    ) -> Result<(), daw_plugin_window::HandlerError> {
        fonts::configure(&ctx);
        theme::configure(&ctx);
        Ok(())
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut daw_plugin_window::Frame) {
        self.show(ui);
    }
}
impl Editor {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if !cfg!(target_os = "macos") {
            let scale = f32::from_bits(self.model.scale.load(Ordering::Relaxed));
            let native = ui.ctx().native_pixels_per_point().unwrap_or(1.0);
            let zoom = scale / native;
            if (ui.ctx().zoom_factor() - zoom).abs() > 0.001 {
                ui.ctx().set_zoom_factor(zoom);
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::InnerSize(drive::SIZE));
            }
        }
        let appearance = self.model.appearance.load(Ordering::Relaxed);
        if appearance != self.appearance {
            theme::apply(
                ui.ctx(),
                if appearance == 0 {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                },
                if appearance == 2 {
                    theme::MIST
                } else {
                    theme::LIGHT
                },
                theme::PLUGIN_ACCENT,
            );
            self.appearance = appearance;
        }
        // Register the background first so controls retain primary-click priority.
        let background = ui.interact(
            ui.max_rect(),
            ui.id().with("appearance"),
            egui::Sense::click(),
        );
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, theme::unit_separator(ui.ctx()));
        if let Some(state) = self.model.current() {
            self.state.set_presentation(state.presentation());
            // Host bypass is not an editable body control, but it must mask stale telemetry.
            let _ = self
                .state
                .set(Parameter::Bypass, state.get(Parameter::Bypass));
            for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
                if !self.active[slot] {
                    let _ = self.state.set(p, state.get(p));
                }
            }
        }
        let mut settings = self.state.to_settings();
        // Reserve room for every active gesture's End, including focus loss/close.
        let ready = self.model.remaining() >= 4 * Parameter::COUNT;
        let bounds = ui
            .add_enabled_ui(ready, |ui| crate::show_body(ui, &mut settings, &self.model))
            .inner;
        debug_assert!(bounds.sections.iter().all(|rect| rect.is_finite()));
        crate::paint_extra(ui, &bounds, &self.model, &settings);
        let next = PluginState::from_settings(&settings).unwrap_or(self.state);
        for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
            let Some(response) = &bounds.controls[slot] else {
                continue;
            };
            let changed = response.changed();
            let dragging = response.dragged() && ui.input(|i| i.focused);
            if changed {
                let mut batch = Vec::with_capacity(3);
                if !self.active[slot] {
                    batch.push(Edit::Begin(p));
                }
                batch.push(Edit::Value(p, next.get(p)));
                if !dragging {
                    batch.push(Edit::End(p));
                }
                if self.model.push(&batch) {
                    self.active[slot] = dragging;
                    let _ = self.state.set(p, next.get(p));
                }
            } else if self.active[slot] && !dragging && self.model.push(&[Edit::End(p)]) {
                self.active[slot] = false;
            }
        }
        if next.presentation() != self.state.presentation()
            && self.model.push(&[Edit::Presentation(next.presentation())])
        {
            self.state.set_presentation(next.presentation());
        }
        daw_unit_ui::menus::ContextMenu::new(&background).show(|ui| {
            for (index, name) in ["Dark", "Light", "Mist"].into_iter().enumerate() {
                if ui
                    .selectable_label(appearance == index as u32, name)
                    .clicked()
                {
                    self.model.appearance.store(index as u32, Ordering::Relaxed);
                    ui.close();
                }
            }
        });
        (self.wake)();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
}

fn state_words(state: PluginState) -> [u64; daw_drive_plugin::MAX_STATE_BYTES.div_ceil(8)] {
    let bytes = state.encode();
    std::array::from_fn(|i| {
        let mut word = [0; 8];
        for (j, v) in word.iter_mut().enumerate() {
            *v = bytes.get(i * 8 + j).copied().unwrap_or(0);
        }
        u64::from_le_bytes(word)
    })
}
