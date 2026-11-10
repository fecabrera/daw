//! GTK input and clipboard event mapping for the Linux desktop.
use super::*;

pub(super) fn modifiers(value: gdk::ModifierType) -> egui::Modifiers {
    egui::Modifiers {
        alt: value.contains(gdk::ModifierType::ALT_MASK),
        ctrl: value.contains(gdk::ModifierType::CONTROL_MASK),
        shift: value.contains(gdk::ModifierType::SHIFT_MASK),
        mac_cmd: false,
        command: value.contains(gdk::ModifierType::CONTROL_MASK),
    }
}

fn key(value: gdk::Key) -> Option<egui::Key> {
    let name = value.name()?;
    egui::Key::from_name(match name.as_str() {
        "Return" | "KP_Enter" => "Enter",
        "ISO_Left_Tab" => "Tab",
        "BackSpace" => "Backspace",
        "KP_Delete" => "Delete",
        "KP_Home" => "Home",
        "KP_End" => "End",
        "space" => "Space",
        "Left" => "ArrowLeft",
        "Right" => "ArrowRight",
        "Up" => "ArrowUp",
        "Down" => "ArrowDown",
        "Page_Up" => "PageUp",
        "Page_Down" => "PageDown",
        other => other,
    })
    .or_else(|| {
        value
            .to_unicode()
            .and_then(|c| egui::Key::from_name(&c.to_ascii_uppercase().to_string()))
    })
}

pub(super) fn connect_input(
    area: &gtk4::GLArea,
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<State>>,
) {
    super::touch::connect_touch(area, window, state);
    let motion = gtk4::EventControllerMotion::new();
    let input = state.clone();
    motion.connect_motion(move |controller, x, y| {
        let mut state = input.borrow_mut();
        let pos = egui::pos2(x as f32, y as f32) / state.ctx.zoom_factor();
        state.pointer = pos;
        state
            .input
            .events
            .push(egui::Event::ModifiersChanged(modifiers(
                controller.current_event_state(),
            )));
        state.input.events.push(egui::Event::PointerMoved(pos));
        state.ctx.request_repaint();
    });
    let input = state.clone();
    motion.connect_leave(move |_| {
        let mut state = input.borrow_mut();
        state.input.events.push(egui::Event::PointerGone);
        state.ctx.request_repaint();
    });
    area.add_controller(motion);
    let click = gtk4::GestureClick::new();
    click.set_button(0);
    let input = state.clone();
    let weak_window = window.downgrade();
    let weak_area = area.downgrade();
    click.connect_pressed(move |gesture, count, x, y| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        if let Some(area) = weak_area.upgrade() {
            area.grab_focus();
        }
        let mut state = input.borrow_mut();
        let pos = egui::pos2(x as f32, y as f32) / state.ctx.zoom_factor();
        let touch = gesture
            .current_event()
            .is_some_and(|event| event.event_type() == gdk::EventType::TouchBegin);
        if (gesture.current_button() == 1 || touch)
            && state.ui.settings().titlebar == TitlebarStyle::Custom
            && state.ui.header_layout(&state.ctx).can_drag(pos)
        {
            if count == 2 {
                if window.is_maximized() {
                    window.unmaximize();
                } else {
                    window.maximize();
                }
            } else if let (Some(event), Some(surface)) = (gesture.current_event(), window.surface())
                && let (Some(device), Ok(toplevel), Some((surface_x, surface_y))) = (
                    event.device(),
                    surface.dynamic_cast::<gdk::Toplevel>(),
                    event.position(),
                )
            {
                toplevel.begin_move(
                    &device,
                    if touch { 0 } else { 1 },
                    surface_x,
                    surface_y,
                    event.time(),
                );
            }
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            return;
        }
        pointer_button(
            &mut state,
            gesture.current_button(),
            true,
            gesture.current_event_state(),
            pos,
        );
    });
    let input = state.clone();
    click.connect_released(move |gesture, _, x, y| {
        let mut state = input.borrow_mut();
        let pos = egui::pos2(x as f32, y as f32) / state.ctx.zoom_factor();
        pointer_button(
            &mut state,
            gesture.current_button(),
            false,
            gesture.current_event_state(),
            pos,
        );
    });
    area.add_controller(click);
    let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::BOTH_AXES);
    let input = state.clone();
    scroll.connect_scroll(move |controller, x, y| {
        let mut state = input.borrow_mut();
        let modifiers = modifiers(controller.current_event_state());
        state
            .input
            .events
            .push(egui::Event::ModifiersChanged(modifiers));
        let surface_units = controller.current_event().is_some_and(|event| {
            event
                .downcast_ref::<gdk::ScrollEvent>()
                .is_some_and(|scroll| scroll.unit() == gdk::ScrollUnit::Surface)
        });
        let (unit, delta) = scroll_delta(x, y, surface_units, state.ctx.zoom_factor());
        state.input.events.push(egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit,
            delta,
            modifiers,
        });
        state.ctx.request_repaint();
        glib::Propagation::Stop
    });
    area.add_controller(scroll);
    let im = gtk4::IMMulticontext::new();
    im.set_client_widget(Some(area));
    let input = state.clone();
    im.connect_commit(move |_, text| {
        let mut state = input.borrow_mut();
        state.input.events.push(egui::Event::Text(text.into()));
        state.ctx.request_repaint();
    });
    let keyboard = gtk4::EventControllerKey::new();
    // An attached IM context filters printable keys before key-pressed. Forward
    // keys first so Space and letter shortcuts also work outside text fields.
    keyboard.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let press_im = im.clone();
    let input = state.clone();
    let weak_area = area.downgrade();
    keyboard.connect_key_pressed(move |controller, value, _, flags| {
        let mods = modifiers(flags);
        let mut state = input.borrow_mut();
        state.input.events.push(egui::Event::ModifiersChanged(mods));
        if mods.command {
            match value.to_unicode().map(|c| c.to_ascii_lowercase()) {
                Some('c') => state.input.events.push(egui::Event::Copy),
                Some('x') => state.input.events.push(egui::Event::Cut),
                Some('v') => {
                    if let Some(area) = weak_area.upgrade() {
                        let input = input.clone();
                        area.clipboard().read_text_async(
                            None::<&gtk4::gio::Cancellable>,
                            move |result| {
                                if let Ok(Some(text)) = result {
                                    let mut state = input.borrow_mut();
                                    state.input.events.push(egui::Event::Paste(text.into()));
                                    state.ctx.request_repaint();
                                }
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        if let Some(key) = key(value) {
            state.input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            });
        }
        state.ctx.request_repaint();
        // Filtering can emit commit synchronously and borrow state again.
        drop(state);
        if let Some(event) = controller.current_event() {
            press_im.filter_keypress(&event);
        }
        glib::Propagation::Stop
    });
    let input = state.clone();
    let release_im = im.clone();
    keyboard.connect_key_released(move |controller, value, _, flags| {
        let mut state = input.borrow_mut();
        let mods = modifiers(flags);
        state.input.events.push(egui::Event::ModifiersChanged(mods));
        if let Some(key) = key(value) {
            state.input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: mods,
            });
        }
        state.ctx.request_repaint();
        drop(state);
        if let Some(event) = controller.current_event() {
            release_im.filter_keypress(&event);
        }
    });
    area.add_controller(keyboard);
    let focus = gtk4::EventControllerFocus::new();
    let input = state.clone();
    let focus_im = im.clone();
    focus.connect_enter(move |_| {
        focus_im.focus_in();
        let mut state = input.borrow_mut();
        state.input.events.push(egui::Event::WindowFocused(true));
        state.ctx.request_repaint();
    });
    let input = state.clone();
    focus.connect_leave(move |_| {
        im.focus_out();
        let mut state = input.borrow_mut();
        state.input.events.push(egui::Event::WindowFocused(false));
        state.ctx.request_repaint();
    });
    area.add_controller(focus);
    let drop = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    drop.set_preload(true);
    let input = state.clone();
    drop.connect_motion(move |target, x, y| {
        let mut state = input.borrow_mut();
        let pos = egui::pos2(x as f32, y as f32) / state.ctx.zoom_factor();
        state.input.events.push(egui::Event::PointerMoved(pos));
        state.input.hovered_files = target
            .value_as::<gdk::FileList>()
            .map(|list| {
                list.files()
                    .iter()
                    .filter_map(|file| file.path())
                    .map(|path| egui::HoveredFile {
                        path: Some(path),
                        ..Default::default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        state.ctx.request_repaint();
        gdk::DragAction::COPY
    });
    let input = state.clone();
    drop.connect_leave(move |_| {
        let mut state = input.borrow_mut();
        state.input.hovered_files.clear();
        state.ctx.request_repaint();
    });
    let input = state.clone();
    drop.connect_drop(move |_, value, x, y| {
        let Ok(list) = value.get::<gdk::FileList>() else {
            return false;
        };
        let mut state = input.borrow_mut();
        let pos = egui::pos2(x as f32, y as f32) / state.ctx.zoom_factor();
        state.input.events.push(egui::Event::PointerMoved(pos));
        state.input.hovered_files.clear();
        for file in list.files() {
            if let Some(path) = file.path() {
                state.input.dropped_files.push(Arc::new(NativeFile(path)));
            }
        }
        state.ctx.request_repaint();
        true
    });
    area.add_controller(drop);
}

fn scroll_delta(
    x: f64,
    y: f64,
    surface_units: bool,
    zoom: f32,
) -> (egui::MouseWheelUnit, egui::Vec2) {
    let delta = egui::vec2(-x as f32, -y as f32);
    if surface_units {
        (egui::MouseWheelUnit::Point, delta / zoom)
    } else {
        (egui::MouseWheelUnit::Line, delta)
    }
}

fn pointer_button(
    state: &mut State,
    button: u32,
    pressed: bool,
    flags: gdk::ModifierType,
    pos: egui::Pos2,
) {
    let button = match button {
        1 => egui::PointerButton::Primary,
        2 => egui::PointerButton::Middle,
        3 => egui::PointerButton::Secondary,
        _ => return,
    };
    let modifiers = modifiers(flags);
    state
        .input
        .events
        .push(egui::Event::ModifiersChanged(modifiers));
    state.input.events.push(egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    });
    state.ctx.request_repaint();
}

#[derive(Debug)]
struct NativeFile(std::path::PathBuf);
impl egui::DroppedFile for NativeFile {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touchpad_scroll_uses_points_and_ui_zoom_while_wheel_uses_lines() {
        assert_eq!(
            scroll_delta(4.0, -8.0, true, 2.0),
            (egui::MouseWheelUnit::Point, egui::vec2(-2.0, 4.0))
        );
        assert_eq!(
            scroll_delta(1.0, -2.0, false, 2.0),
            (egui::MouseWheelUnit::Line, egui::vec2(-1.0, 2.0))
        );
    }

    #[test]
    fn gtk_shortcut_and_editing_keys_map_to_egui() {
        for (native, expected) in [
            (gdk::Key::space, egui::Key::Space),
            (gdk::Key::BackSpace, egui::Key::Backspace),
            (gdk::Key::Return, egui::Key::Enter),
            (gdk::Key::ISO_Left_Tab, egui::Key::Tab),
            (gdk::Key::Left, egui::Key::ArrowLeft),
            (gdk::Key::Delete, egui::Key::Delete),
            (gdk::Key::s, egui::Key::S),
            (gdk::Key::S, egui::Key::S),
        ] {
            assert_eq!(key(native), Some(expected));
        }
    }
}
