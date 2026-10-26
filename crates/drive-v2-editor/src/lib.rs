//! Drive editor and bounded UI-thread channels. No audio or SDK ownership.
use daw_drive_plugin::{Parameter, PluginState};
use daw_plugin_support::Snapshot;
use daw_unit_ui::{drive_v2 as drive, fonts, theme};
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
    state: Snapshot<{ Parameter::COUNT }>,
    events: Mutex<VecDeque<Edit>>,
    pub appearance: AtomicU32,
    pub scale: AtomicU32,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            state: Snapshot::new(Parameter::ALL.map(|p| PluginState::default().get(p).to_bits())),
            events: Mutex::new(VecDeque::with_capacity(CAPACITY)),
            appearance: AtomicU32::new(0),
            scale: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}
impl Model {
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
        let ready = self.model.remaining() >= 4 * Parameter::COUNT;
        let bounds = ui
            .add_enabled_ui(ready, |ui| {
                drive::show(
                    ui,
                    &mut settings,
                    false,
                    0.0,
                    &std::array::from_fn(|_| None),
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
            [
                225.0, 45.0, 45.0, 135.0, 135.0, 135.0, 225.0, 225.0, 45.0, 115.0,
            ][slot],
            [
                96.0, 96.0, 156.0, 36.0, 96.0, 156.0, 36.0, 156.0, 36.0, 200.0,
            ][slot],
        );
        frame(
            ctx,
            editor,
            vec![Event::PointerMoved(p), pointer(p, true)],
            true,
        );
        let end = if slot == 9 {
            p
        } else {
            p - Vec2::new(
                0.0,
                if slot == 8 {
                    100.0
                } else if slot == 3 {
                    40.0
                } else if slot == 7 {
                    -18.0
                } else {
                    18.0
                },
            )
        };
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
    fn ten_controls_emit_balanced_host_gestures_and_off_color_is_retained() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 4, true);
        assert!(events(&model).is_empty());
        let mut state = PluginState::default();
        state.set(Parameter::ColorEnabled, 1.0).unwrap();
        model.publish(state);
        for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
            frame(&ctx, &mut editor, vec![], true);
            drag(&ctx, &mut editor, slot, true);
            let edits = events(&model);
            assert_eq!(edits.first(), Some(&Edit::Begin(p)), "{p:?}: {edits:?}");
            assert!(edits.iter().any(|e| matches!(e,Edit::Value(q,_)if *q==p)));
            assert_eq!(edits.last(), Some(&Edit::End(p)));
        }
        state.set(Parameter::ColorEnabled, 0.0).unwrap();
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
        let p = Pos2::new(225.0, 96.0);
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
}
