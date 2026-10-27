//! Drive editor and bounded UI-thread channels. No audio or SDK ownership.
use daw_drive_plugin::{Parameter, PluginState};
use daw_plugin_support::Snapshot;
use daw_unit_ui::{drive, fonts, theme};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
pub const WIDTH: u32 = drive::SIZE.x as u32;
pub const HEIGHT: u32 = drive::SIZE.y as u32;
const CAPACITY: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Begin(Parameter),
    Value(Parameter, f64),
    End(Parameter),
}
/// Access this channel only from editor/control threads. The audio callback never locks it.
pub struct Model {
    pub output_meter: Arc<daw_drive_plugin::OutputMeter>,
    pub meter_reset: std::sync::atomic::AtomicU32,
    pub output_reset: std::sync::atomic::AtomicBool,
    state: Snapshot<7>,
    events: Mutex<VecDeque<Edit>>,
    pub appearance: AtomicU32,
    pub scale: AtomicU32,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            output_meter: Arc::default(),
            meter_reset: std::sync::atomic::AtomicU32::new(0),
            output_reset: std::sync::atomic::AtomicBool::new(false),
            state: Snapshot::new(Parameter::ALL.map(|p| PluginState::default().get(p).to_bits())),
            events: Mutex::new(VecDeque::with_capacity(CAPACITY)),
            appearance: AtomicU32::new(0),
            scale: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}
impl Model {
    /// Apply a host-transported VST3 snapshot on the control thread.
    pub fn receive_output(&self, values: [f32; 2]) {
        use daw_unit_core::MeterStorage;
        self.output_meter.boost_peaks()[0].reset_peak();
        self.output_meter.store([values[1], 0.0, 0.0]);
        self.output_meter.initialize([values[0], 0.0, 0.0]);
    }

    pub fn publish(&self, state: PluginState) {
        self.state
            .publish(Parameter::ALL.map(|p| state.get(p).to_bits()));
    }
    pub fn current(&self) -> Option<PluginState> {
        let (_, values) = self.state.read()?;
        let mut state = PluginState::default();
        for (p, v) in Parameter::ALL.into_iter().zip(values) {
            state.set(p, f64::from_bits(v)).ok()?;
        }
        Some(state)
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
    active: [bool; 6],
    appearance: u32,
}
impl Editor {
    pub fn new(model: Arc<Model>, wake: impl Fn() + Send + 'static) -> Self {
        let state = model.current().unwrap_or_default();
        Self {
            model,
            wake: Box::new(wake),
            state,
            active: [false; 6],
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
            .rect_filled(ui.max_rect(), 0.0, theme::palette(ui.ctx()).panel);
        if let Some(state) = self.model.current() {
            for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
                if !self.active[slot] {
                    let _ = self.state.set(p, state.get(p));
                }
            }
        }
        let mut settings = self.state.to_settings();
        // Reserve room for every active gesture's End, including focus loss/close.
        let ready = self.model.remaining() >= 24;
        let bounds = ui
            .add_enabled_ui(ready, |ui| {
                drive::show(
                    ui,
                    &mut settings,
                    false,
                    0.0,
                    &std::array::from_fn(|_| None),
                    daw_unit_ui::meters::OutputLevel::new(
                        self.model.output_meter.readings()[0],
                        Some(&self.model.output_meter.boost_peaks()[0]),
                    )
                    .with_reset_request(&self.model.output_reset),
                )
            })
            .inner;
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
        background.context_menu(|ui| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2};
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>, focused: bool) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                events,
                focused,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        output.textures_delta.clear();
    }
    fn pointer(p: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn drag(ctx: &egui::Context, editor: &mut Editor, slot: usize, release: bool) {
        let p = Pos2::new(
            if slot < 3 { 45.0 } else { 135.0 },
            36.0 + (slot % 3) as f32 * 60.0,
        );
        frame(
            ctx,
            editor,
            vec![Event::PointerMoved(p), pointer(p, true)],
            true,
        );
        let end = p - Vec2::new(0.0, if slot == 3 { 40.0 } else { 18.0 });
        frame(ctx, editor, vec![Event::PointerMoved(end)], true);
        if release {
            frame(ctx, editor, vec![pointer(end, false)], true);
        }
    }
    fn setup() -> (egui::Context, Arc<Model>, Editor) {
        let ctx = egui::Context::default();
        fonts::configure(&ctx);
        theme::configure(&ctx);
        let model = Arc::new(Model::default());
        let editor = Editor::new(model.clone(), || {});
        (ctx, model, editor)
    }
    fn events(model: &Model) -> Vec<Edit> {
        std::iter::from_fn(|| model.pop()).collect()
    }
    #[test]
    fn six_controls_emit_balanced_host_gestures_and_off_color_is_retained() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 4, true);
        assert!(events(&model).is_empty());
        let mut state = PluginState::default();
        state.set(Parameter::ColorShape, 1.0).unwrap();
        model.publish(state);
        for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
            frame(&ctx, &mut editor, vec![], true);
            drag(&ctx, &mut editor, slot, true);
            let edits = events(&model);
            assert_eq!(edits.first(), Some(&Edit::Begin(p)), "{p:?}: {edits:?}");
            assert!(edits.iter().any(|e| matches!(e,Edit::Value(q,_)if *q==p)));
            assert_eq!(edits.last(), Some(&Edit::End(p)));
        }
        state.set(Parameter::ColorShape, 0.0).unwrap();
        state.set(Parameter::ColorStrength, 0.8).unwrap();
        model.publish(state);
        frame(&ctx, &mut editor, vec![], true);
        assert_eq!(editor.state.get(Parameter::ColorStrength), 0.8);
    }
    #[test]
    fn closing_and_focus_loss_end_drag_and_keyboard_edits_are_single_gestures() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 1, false);
        assert_eq!(events(&model).first(), Some(&Edit::Begin(Parameter::Gain)));
        frame(&ctx, &mut editor, vec![], false);
        assert_eq!(events(&model), [Edit::End(Parameter::Gain)]);
        frame(&ctx, &mut editor, vec![Event::PointerGone], true);
        let p = Pos2::new(45.0, 36.0);
        frame(
            &ctx,
            &mut editor,
            vec![Event::PointerMoved(p), pointer(p, true)],
            true,
        );
        frame(&ctx, &mut editor, vec![pointer(p, false)], true);
        events(&model);
        frame(
            &ctx,
            &mut editor,
            vec![Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            true,
        );
        let edits = events(&model);
        assert!(matches!(
            edits.as_slice(),
            [
                Edit::Begin(Parameter::Volume),
                Edit::Value(Parameter::Volume, _),
                Edit::End(Parameter::Volume)
            ]
        ));
        drag(&ctx, &mut editor, 2, false);
        events(&model);
        drop(editor);
        assert_eq!(events(&model), [Edit::End(Parameter::Tone)]);
    }
    #[test]
    fn full_ui_queue_preserves_batches_and_reserves_gesture_end() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        let filler = vec![Edit::Begin(Parameter::Bypass); CAPACITY - 2];
        assert!(model.push(&filler));
        assert!(!model.push(&[
            Edit::Begin(Parameter::Gain),
            Edit::Value(Parameter::Gain, 0.7),
            Edit::End(Parameter::Gain)
        ]));
        drag(&ctx, &mut editor, 1, true);
        assert_eq!(events(&model).len(), CAPACITY - 2);
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 1, false);
        events(&model);
        assert!(model.push(&vec![Edit::Begin(Parameter::Bypass); CAPACITY - 1]));
        drop(editor);
        let edits = events(&model);
        assert_eq!(edits.last(), Some(&Edit::End(Parameter::Gain)));
    }
    #[test]
    fn output_peak_reset_is_not_a_parameter_gesture() {
        use daw_unit_core::MeterStorage;
        let (ctx, model, mut editor) = setup();
        model.output_meter.store([1.5, 0.0, 0.0]);
        model.output_meter.initialize([0.25, 0.0, 0.0]);
        frame(&ctx, &mut editor, vec![], true);
        let pos = Pos2::new(45.0, 4.0);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut editor,
                vec![Event::PointerMoved(pos), pointer(pos, pressed)],
                true,
            );
        }
        assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(model.output_meter.readings()[0], 0.25);
        assert!(events(&model).is_empty());
        assert_eq!(editor.state, PluginState::default());
    }
}

#[cfg(test)]
fn paint_reference(ui: &mut egui::Ui, settings: &mut daw_unit_core::DriveSettings, model: &Model) {
    let level = daw_unit_ui::meters::OutputLevel::new(
        model.output_meter.readings()[0],
        Some(&model.output_meter.boost_peaks()[0]),
    )
    .with_reset_request(&model.output_reset);
    let _ = daw_unit_ui::drive::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn::<_, 5, _>(|_| None),
        level,
    );
}

#[cfg(test)]
#[path = "../../filter-editor/src/parity_tests.rs"]
mod builtin_parity;

#[cfg(test)]
fn probe_controls(
    ui: &mut egui::Ui,
    settings: &mut daw_unit_core::DriveSettings,
) -> Vec<(daw_drive_plugin::Parameter, egui::Rect)> {
    let b = drive::show(
        ui,
        settings,
        false,
        0.0,
        &std::array::from_fn(|_| None),
        0.0.into(),
    );
    b.controls
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .filter(|r| r.enabled())
                .map(|r| (daw_drive_plugin::Parameter::ALL[i + 1], r.rect))
        })
        .collect()
}

#[cfg(test)]
fn assert_builtin_defaults() {
    let native = daw_drive_plugin::PluginState::default().to_settings();
    let builtin: daw_unit_core::DriveSettings = Default::default();
    assert_eq!(native, builtin);
}
