use crate::{DawUi, icons, menus, theme};
use egui::{Response, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportAction {
    Play,
    Stop,
    Loop,
    Record,
    Metronome,
    CountIn,
    Panic,
}

impl TransportAction {
    pub const ALL: [Self; 7] = [
        Self::Play,
        Self::Stop,
        Self::Loop,
        Self::Record,
        Self::Metronome,
        Self::CountIn,
        Self::Panic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Loop => "Loop",
            Self::Record => "Record",
            Self::Metronome => "Metronome",
            Self::CountIn => "Count-in",
            Self::Panic => "Panic",
        }
    }

    pub fn starts_section(self) -> bool {
        matches!(self, Self::Metronome | Self::Panic)
    }
}

impl DawUi {
    pub(super) fn transport_toolbar_available(&self) -> bool {
        self.export_dialog.is_none()
            && self.overwrite.is_none()
            && self.export_status.is_none()
            && self.separation_status.is_none()
            && self.bounce_status.is_none()
            && !self.bounce_input_blocked
            && self.normalize_dialog.is_none()
            && self.collection_status.is_none()
            && self.project_name_dialog.is_none()
            && self.project_settings.is_none()
            && !self.settings_open
            && self.diagnostic_details.is_none()
    }

    pub fn transport_action_enabled(&self, action: TransportAction) -> bool {
        match action {
            TransportAction::Panic => self.midi.service.is_some(),
            _ if !self.transport_toolbar_available() => false,
            TransportAction::Play => self.is_playing() || self.media_ready(),
            TransportAction::Stop => self.stop_action() != StopAction::AtOrigin,
            TransportAction::Loop | TransportAction::Metronome => self.job.is_none(),
            TransportAction::Record => self.midi_record_enabled(),
            TransportAction::CountIn => self.job.is_none() && !self.midi.busy(),
        }
    }

    pub fn transport_action_label(&self, action: TransportAction) -> &'static str {
        if action == TransportAction::Play && self.is_playing() {
            "Pause"
        } else {
            action.label()
        }
    }

    pub fn transport_action_selected(&self, action: TransportAction) -> bool {
        match action {
            TransportAction::Loop => self.session.project.transport.r#loop.enabled,
            TransportAction::Record => self.midi.recording,
            TransportAction::Metronome => self.metronome_enabled,
            TransportAction::CountIn => self.settings.midi.count_in_enabled,
            _ => false,
        }
    }

    pub fn perform_transport_action(&mut self, action: TransportAction) {
        if !self.transport_action_enabled(action) {
            return;
        }
        match action {
            TransportAction::Play => self.toggle_playback(),
            TransportAction::Stop => match self.stop_action() {
                StopAction::Stop => self.stop(),
                StopAction::ReturnToOrigin => self.seek(0),
                StopAction::AtOrigin => {}
            },
            TransportAction::Loop => self.toggle_looping(),
            TransportAction::Record => self.record_midi(),
            TransportAction::Metronome => {
                self.metronome_enabled = !self.metronome_enabled;
                if let Some(output) = &self.output {
                    output.set_metronome(self.metronome_enabled);
                }
            }
            TransportAction::CountIn => {
                self.settings.midi.count_in_enabled = !self.settings.midi.count_in_enabled;
            }
            TransportAction::Panic => {
                if let Some(service) = &self.midi.service {
                    service.panic();
                }
            }
        }
    }

    pub(super) fn transport_menu(&mut self, ui: &mut Ui) {
        for action in TransportAction::ALL {
            if action.starts_section() {
                menus::separator(ui);
            }
            if ui
                .add_enabled_ui(self.transport_action_enabled(action), |ui| {
                    menus::checked_button(
                        ui,
                        self.transport_action_selected(action),
                        self.transport_action_label(action),
                        "",
                    )
                })
                .inner
                .clicked()
            {
                self.perform_transport_action(action);
                ui.close();
            }
        }
    }
}

pub(super) const PLAY_FILL: egui::Color32 = egui::Color32::from_rgb(51, 133, 85);
pub(super) const STOP_FILL: egui::Color32 = egui::Color32::from_rgb(197, 78, 88);

// Run before widgets so a focused button or catalog cannot also handle Space.
// Text editors keep their input; holding Space must not cycle play/pause.
pub(super) fn take_play_pause(ctx: &egui::Context) -> bool {
    if ctx.text_edit_focused() || !ctx.input(|input| input.focused) {
        return false;
    }
    ctx.input_mut(|input| {
        let mut pressed = false;
        let mut consumed = false;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key: egui::Key::Space,
                pressed: down,
                repeat,
                modifiers,
                ..
            } = event
                && !modifiers.command
                && !modifiers.ctrl
                && !modifiers.mac_cmd
                && !modifiers.alt
            {
                pressed |= *down && !repeat;
                consumed = true;
                return false;
            }
            true
        });
        if consumed {
            input
                .events
                .retain(|event| !matches!(event, egui::Event::Text(text) if text == " "));
        }
        pressed
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopAction {
    Stop,
    ReturnToOrigin,
    AtOrigin,
}
impl StopAction {
    pub fn from_transport(playing: bool, can_return_to_start: bool, playhead_frame: u64) -> Self {
        if playing || can_return_to_start {
            Self::Stop
        } else if playhead_frame > 0 {
            Self::ReturnToOrigin
        } else {
            Self::AtOrigin
        }
    }
}

pub(super) struct Buttons {
    pub play: Response,
    pub stop: Response,
    pub looping: Response,
}

pub(super) fn show(
    ui: &mut Ui,
    playing: bool,
    stop_action: StopAction,
    looping: bool,
    can_play: bool,
    can_loop: bool,
) -> Buttons {
    let size = Vec2::new(30.0, icons::TOOLBAR_BUTTON_SIZE);
    let foreground = egui::Color32::WHITE;
    let play = ui
        .add_enabled_ui(can_play, |ui| {
            icons::toggle_with_surface_tooltip(
                ui,
                icons::Lucide::Play,
                if playing { "Pause" } else { "Play" },
                &daw_unit_ui::tooltips::action(
                    if playing { "Pause" } else { "Play" },
                    Some("Space"),
                ),
                playing,
                size,
                (PLAY_FILL, foreground),
                standard_surface(ui),
            )
        })
        .inner;
    let (icon, label) = if ui.is_enabled() && stop_action == StopAction::ReturnToOrigin {
        (icons::Lucide::ChevronFirst, "Return to origin")
    } else {
        (icons::Lucide::Square, "Stop")
    };
    let stop = ui
        .add_enabled_ui(stop_action != StopAction::AtOrigin, |ui| {
            icons::toggle_with_surface(
                ui,
                icon,
                label,
                playing,
                size,
                (STOP_FILL, foreground),
                standard_surface(ui),
            )
        })
        .inner;
    let looping = ui
        .add_enabled_ui(can_loop, |ui| {
            icons::toggle_with_surface(
                ui,
                icons::Lucide::Repeat,
                "Loop",
                looping,
                size,
                (theme::LOOP_COLOR, foreground),
                standard_surface(ui),
            )
        })
        .inner;
    Buttons {
        play,
        stop,
        looping,
    }
}

fn standard_surface(ui: &Ui) -> icons::ToggleSurface {
    icons::ToggleSurface {
        idle: (
            theme::unit_separator(ui.ctx()),
            theme::palette(ui.ctx()).text,
        ),
        hovered: (
            theme::unit_header_fill(ui.ctx(), false),
            ui.visuals().widgets.hovered.fg_stroke.color,
        ),
        corners: icons::TOGGLE_CORNER_RADIUS,
    }
}

/// Standard metronome surface with the Loop timing color and white active artwork.
pub(super) fn metronome(ui: &mut Ui, selected: bool, enabled: bool) -> Response {
    let surface = standard_surface(ui);
    ui.add_enabled_ui(enabled, |ui| {
        icons::toggle_with_surface(
            ui,
            icons::Lucide::Metronome,
            "Metronome",
            selected,
            Vec2::new(30.0, icons::TOOLBAR_BUTTON_SIZE),
            (theme::LOOP_COLOR, egui::Color32::WHITE),
            surface,
        )
    })
    .inner
}

/// Record uses the same geometry, surface and icon treatment as Play/Stop/Loop.
pub(super) fn record(ui: &mut Ui, selected: bool, enabled: bool) -> Response {
    let surface = standard_surface(ui);
    ui.add_enabled_ui(enabled, |ui| {
        icons::toggle_with_surface(
            ui,
            icons::Lucide::Circle,
            "Record MIDI",
            selected,
            Vec2::new(30.0, icons::TOOLBAR_BUTTON_SIZE),
            (STOP_FILL, egui::Color32::WHITE),
            surface,
        )
    })
    .inner
}

/// Count-in shares the metronome's timing color and standard transport surface.
pub(super) fn count_in(ui: &mut Ui, selected: bool, enabled: bool) -> Response {
    let surface = standard_surface(ui);
    let name = if selected {
        "Disable count-in"
    } else {
        "Enable count-in"
    };
    ui.add_enabled_ui(enabled, |ui| {
        icons::toggle_image_with_surface_tooltip(
            ui,
            egui::Image::new(egui::include_image!("../assets/icons/numbers.svg")),
            name,
            name,
            selected,
            Vec2::new(30.0, icons::TOOLBAR_BUTTON_SIZE),
            (theme::LOOP_COLOR, egui::Color32::WHITE),
            surface,
        )
    })
    .inner
}
