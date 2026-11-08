//! GTK input and clipboard event mapping for the Linux desktop.
use super::*;

fn modifiers(value: gdk::ModifierType) -> egui::Modifiers {
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
        if gesture.current_button() == 1
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
                toplevel.begin_move(&device, 1, surface_x, surface_y, event.time());
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
        state.input.events.push(egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(-x as f32, -y as f32),
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
    keyboard.set_im_context(Some(&im));
    let input = state.clone();
    let weak_area = area.downgrade();
    keyboard.connect_key_pressed(move |_, value, _, flags| {
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
        glib::Propagation::Stop
    });
    let input = state.clone();
    keyboard.connect_key_released(move |_, value, _, flags| {
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
