//! Direct touch sequences, plus one pointer for the shared mouse interactions.
use super::*;
use std::collections::HashMap;

#[derive(Default)]
struct Touches {
    active: HashMap<u64, egui::Pos2>,
    pointer: Option<u64>,
}

impl Touches {
    fn push(
        &mut self,
        events: &mut Vec<egui::Event>,
        id: u64,
        phase: egui::TouchPhase,
        pos: egui::Pos2,
        modifiers: egui::Modifiers,
    ) {
        if phase == egui::TouchPhase::Start {
            if self.active.is_empty() {
                self.pointer = Some(id);
            }
            self.active.insert(id, pos);
        } else if let Some(previous) = self.active.get_mut(&id) {
            *previous = pos;
        } else {
            return;
        }
        events.push(egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase,
            pos,
            force: None,
        });
        if self.pointer == Some(id) {
            match phase {
                egui::TouchPhase::Start | egui::TouchPhase::Move => {
                    events.push(egui::Event::PointerMoved(pos));
                    if phase == egui::TouchPhase::Start {
                        events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers,
                        });
                    }
                }
                egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                    let release_pos = if phase == egui::TouchPhase::Cancel {
                        // Cancel shared editing gestures before clearing the held
                        // pointer. Release outside the UI to avoid button clicks.
                        for pressed in [true, false] {
                            events.push(egui::Event::Key {
                                key: egui::Key::Escape,
                                physical_key: None,
                                pressed,
                                repeat: false,
                                modifiers,
                            });
                        }
                        let outside = egui::pos2(-1_000_000.0, -1_000_000.0);
                        events.push(egui::Event::PointerMoved(outside));
                        outside
                    } else {
                        pos
                    };
                    events.push(egui::Event::PointerButton {
                        pos: release_pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers,
                    });
                    events.push(egui::Event::PointerGone);
                    self.pointer = None;
                }
            }
        }
        if matches!(phase, egui::TouchPhase::End | egui::TouchPhase::Cancel) {
            self.active.remove(&id);
        }
    }
}

pub(super) fn connect_touch(
    area: &gtk4::GLArea,
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<State>>,
) {
    let zoom = gtk4::GestureZoom::new();
    let previous_scale = Rc::new(Cell::new(1.0_f64));
    let scale = previous_scale.clone();
    zoom.connect_begin(move |_, _| scale.set(1.0));
    let input = state.clone();
    zoom.connect_scale_changed(move |gesture, scale| {
        // Direct touches already produce egui multitouch zoom. This path is
        // for touchpad pinch events, which GDK does not report as touches.
        if !gesture
            .current_event()
            .is_some_and(|event| event.event_type() == gdk::EventType::TouchpadPinch)
        {
            return;
        }
        let previous = previous_scale.replace(scale);
        if scale.is_finite() && scale > 0.0 && previous.is_finite() && previous > 0.0 {
            let mut state = input.borrow_mut();
            state
                .input
                .events
                .push(egui::Event::Zoom((scale / previous) as f32));
            state.ctx.request_repaint();
        }
    });
    area.add_controller(zoom);
    let touches = Rc::new(RefCell::new(Touches::default()));
    let controller = gtk4::EventControllerLegacy::new();
    // Consume direct touches before GestureClick can synthesize duplicate clicks.
    controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let input = state.clone();
    let weak_area = area.downgrade();
    let weak_window = window.downgrade();
    let contacts = touches.clone();
    controller.connect_event(move |_, event| {
        let phase = match event.event_type() {
            gdk::EventType::TouchBegin => egui::TouchPhase::Start,
            gdk::EventType::TouchUpdate => egui::TouchPhase::Move,
            gdk::EventType::TouchEnd => egui::TouchPhase::End,
            gdk::EventType::TouchCancel => egui::TouchPhase::Cancel,
            _ => return glib::Propagation::Proceed,
        };
        let (Some(area), Some(window)) = (weak_area.upgrade(), weak_window.upgrade()) else {
            return glib::Propagation::Proceed;
        };
        let id = egui::epaint::util::hash(event.event_sequence());
        if phase == egui::TouchPhase::Start {
            area.grab_focus();
        }
        let mut state = input.borrow_mut();
        // GDK positions are surface-relative, including a native titlebar.
        // Cancellation may have no position; retain the last contact position.
        let pos = event
            .position()
            .and_then(|(x, y)| {
                let (surface_x, surface_y) = window.surface_transform();
                window
                    .compute_point(
                        &area,
                        &gtk4::graphene::Point::new((x - surface_x) as f32, (y - surface_y) as f32),
                    )
                    .map(|point| egui::pos2(point.x(), point.y()) / state.ctx.zoom_factor())
            })
            .or_else(|| contacts.borrow().active.get(&id).copied());
        let Some(pos) = pos else {
            return glib::Propagation::Proceed;
        };
        // Let the existing GTK header gesture start a native window move.
        if phase == egui::TouchPhase::Start
            && state.ui.settings().titlebar == TitlebarStyle::Custom
            && state.ui.header_layout(&state.ctx).can_drag(pos)
        {
            return glib::Propagation::Proceed;
        }
        let mut touches = contacts.borrow_mut();
        if phase != egui::TouchPhase::Start && !touches.active.contains_key(&id) {
            return glib::Propagation::Proceed;
        }
        let modifiers = super::input::modifiers(event.modifier_state());
        state
            .input
            .events
            .push(egui::Event::ModifiersChanged(modifiers));
        state.pointer = pos;
        touches.push(&mut state.input.events, id, phase, pos, modifiers);
        state.ctx.request_repaint();
        glib::Propagation::Stop
    });
    area.add_controller(controller);
    let focus = gtk4::EventControllerFocus::new();
    let input = state.clone();
    focus.connect_leave(move |_| {
        let mut state = input.borrow_mut();
        let mut touches = touches.borrow_mut();
        for (id, pos) in touches.active.clone() {
            touches.push(
                &mut state.input.events,
                id,
                egui::TouchPhase::Cancel,
                pos,
                egui::Modifiers::default(),
            );
        }
        state.ctx.request_repaint();
    });
    area.add_controller(focus);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_contacts_produce_one_pointer_and_keep_touch_identity() {
        let mut touches = Touches::default();
        let mut events = Vec::new();
        for (id, phase) in [
            (1, egui::TouchPhase::Start),
            (2, egui::TouchPhase::Start),
            (2, egui::TouchPhase::Move),
            (2, egui::TouchPhase::End),
            (1, egui::TouchPhase::Move),
            (1, egui::TouchPhase::End),
        ] {
            touches.push(
                &mut events,
                id,
                phase,
                egui::pos2(10.0, 20.0),
                Default::default(),
            );
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, egui::Event::Touch { .. }))
                .count(),
            6
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, egui::Event::PointerButton { pressed: false, .. }))
                .count(),
            1
        );
        assert!(matches!(events.last(), Some(egui::Event::PointerGone)));
        assert!(touches.active.is_empty());
    }

    #[test]
    fn cancelled_contact_does_not_release_as_a_click_or_keep_dragging() {
        let mut touches = Touches::default();
        let mut events = Vec::new();
        for phase in [
            egui::TouchPhase::Start,
            egui::TouchPhase::Cancel,
            egui::TouchPhase::Move,
        ] {
            touches.push(&mut events, 1, phase, egui::Pos2::ZERO, Default::default());
        }
        assert!(matches!(events.last(), Some(egui::Event::PointerGone)));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                assert!(!ui.input(|input| input.pointer.any_down()));
                assert!(ui.input(|input| input.key_pressed(egui::Key::Escape)));
                assert!(!ui.input(|input| input.pointer.any_click()));
            },
        );
        output.textures_delta.clear();
        assert!(touches.pointer.is_none());
        assert!(touches.active.is_empty());
    }
}
