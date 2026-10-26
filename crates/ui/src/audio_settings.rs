//! Machine-local audio device preferences, separate from project processing settings.
use super::*;
use daw_output::{AudioSettings, DeviceId, DeviceInfo, Devices};

pub(super) struct AudioDialog {
    draft: AudioSettings,
    devices: Option<Devices>,
    error: Option<String>,
}

#[derive(Default)]
pub(super) struct DevicesState {
    pub catalog: Option<Devices>,
    pub error: Option<String>,
    pending: Option<std::sync::mpsc::Receiver<Result<Devices, String>>>,
}
impl DevicesState {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Apply,
    Cancel,
}

fn device_label(selection: Option<&DeviceId>, devices: &[DeviceInfo]) -> String {
    let Some(selection) = selection else {
        return "System default".into();
    };
    let suffix = if selection.occurrence > 0 {
        format!(" ({})", selection.occurrence + 1)
    } else {
        String::new()
    };
    let missing = if devices.iter().any(|device| device.id == *selection) {
        ""
    } else {
        " (unavailable)"
    };
    format!("{}{suffix}{missing}", selection.name)
}
pub(super) fn device_row(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    selected: &mut Option<DeviceId>,
    devices: &[DeviceInfo],
) -> bool {
    let previous = selected.clone();
    rows::field(
        ui,
        width,
        |ui| {
            ui.label(label);
        },
        |ui| {
            egui::ComboBox::from_id_salt(label)
                .selected_text(device_label(selected.as_ref(), devices))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(selected, None, "System default");
                    for device in devices {
                        ui.selectable_value(
                            selected,
                            Some(device.id.clone()),
                            device_label(Some(&device.id), devices),
                        );
                    }
                });
        },
    );
    *selected != previous
}
impl AudioDialog {
    fn new(settings: AudioSettings) -> Self {
        Self {
            draft: settings,
            devices: None,
            error: None,
        }
    }
    fn selected_output(&self) -> Option<&DeviceInfo> {
        let devices = self.devices.as_ref()?;
        let id = self
            .draft
            .output_device
            .as_ref()
            .or(devices.default_output.as_ref())?;
        devices.outputs.iter().find(|device| &device.id == id)
    }
    fn valid(&self) -> bool {
        self.selected_output().is_some_and(|device| {
            device
                .configurations
                .iter()
                .any(|config| config.supports(&self.draft))
        })
    }

    fn show(&mut self, ctx: &egui::Context, busy: bool) -> Option<Action> {
        let popup_was_open = egui::Popup::is_any_open(ctx);
        let mut action = None;
        dialogs::Dialog::new("Audio quality defaults", "", &[] as &[(&str, Action)]).show_with_content(ctx, |ui| {
            let width = ui.painter().layout_no_wrap("Output sample rate".into(), egui::TextStyle::Body.resolve(ui.style()), theme::palette(ctx).text).size().x;
            ui.add_enabled_ui(!busy, |ui| {
                let configurations = self.selected_output().map(|device| device.configurations.clone()).unwrap_or_default();
                rows::field(ui, width, |ui| { ui.label("Output sample rate"); }, |ui| {
                    egui::ComboBox::from_id_salt("device_sample_rate").selected_text(self.draft.sample_rate_hz.map_or_else(|| "Automatic (prefer project)".into(), |rate| format!("{rate} Hz")))
                        .width(ui.available_width()).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.draft.sample_rate_hz, None, "Automatic (prefer project)");
                            for rate in daw_core::SAMPLE_RATES {
                                let candidate = AudioSettings { sample_rate_hz: Some(rate), ..self.draft.clone() };
                                if configurations.iter().any(|config| config.supports(&candidate)) {
                                    ui.selectable_value(&mut self.draft.sample_rate_hz, Some(rate), format!("{rate} Hz"));
                                }
                            }
                        });
                });
                rows::field(ui, width, |ui| { ui.label("Output bit depth"); }, |ui| {
                    egui::ComboBox::from_id_salt("device_sample_format").selected_text(self.draft.sample_format.map_or("Automatic (driver default)", |format| format.label()))
                        .width(ui.available_width()).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.draft.sample_format, None, "Automatic (driver default)");
                            let mut formats = Vec::new();
                            for config in &configurations {
                                let candidate = AudioSettings { sample_format: Some(config.format), ..self.draft.clone() };
                                if !formats.contains(&config.format) && config.supports(&candidate) {
                                    formats.push(config.format);
                                }
                            }
                            for format in formats {
                                ui.selectable_value(&mut self.draft.sample_format, Some(format), format.label());
                            }
                        });
                });
                rows::field(ui, width, |ui| { ui.label("Output buffer size"); }, |ui| {
                    egui::ComboBox::from_id_salt("device_buffer_size").selected_text(self.draft.buffer_size_frames.map_or_else(|| "Automatic (prefer project)".into(), |frames| format!("{frames} samples")))
                        .width(ui.available_width()).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.draft.buffer_size_frames, None, "Automatic (prefer project)");
                            for frames in daw_core::BUFFER_SIZES {
                                let candidate = AudioSettings { buffer_size_frames: Some(frames), ..self.draft.clone() };
                                if configurations.iter().any(|config| config.supports(&candidate)) {
                                    ui.selectable_value(&mut self.draft.buffer_size_frames, Some(frames), format!("{frames} samples"));
                                }
                            }
                        });
                });
                ui.label("These are device stream settings. The driver can use a different hardware format. Project processing and export settings stay separate.");
                ui.label("Applying output changes stops playback. Press Play to resume.");
                if let Some(error) = &self.error { ui.colored_label(theme::palette(ctx).text, error); }
                if self.devices.is_some() && !self.valid() {
                    ui.label("Choose available devices and a supported output configuration.");
                }
                rows::wrapped(ui, toolbars::CONTROL_HEIGHT, |ui| {
                    if ui.add_enabled(self.valid(), egui::Button::new("Apply")).clicked() { action = Some(Action::Apply); }
                    if ui.button("Cancel").clicked() { action = Some(Action::Cancel); }
                });
            });
            if busy { ui.spinner(); ui.label("Checking audio devices…"); }
        });
        if !busy && !popup_was_open && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            action = Some(Action::Cancel);
        }
        action
    }
}

impl DawUi {
    pub(super) fn refresh_audio_devices(&mut self) {
        if self.audio_devices.pending.is_some() {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("audio-device-discovery".into())
            .spawn(move || {
                let _ = sender.send(daw_output::discover_devices());
            }) {
            Ok(_) => {
                self.audio_devices.pending = Some(receiver);
                self.audio_devices.error = None;
            }
            Err(error) => {
                self.audio_devices.error =
                    Some(format!("Cannot start audio device discovery: {error}"))
            }
        }
    }
    pub(super) fn poll_audio_devices(&mut self, ctx: &egui::Context) {
        let result =
            self.audio_devices
                .pending
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Audio device discovery stopped unexpectedly".into()))
                    }
                });
        if let Some(result) = result {
            self.audio_devices.pending = None;
            self.finish_audio_discovery(result);
        }
        if self.audio_devices.busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    pub(super) fn show_audio_settings(&mut self, ctx: &egui::Context) {
        let action = self
            .audio_settings
            .as_mut()
            .and_then(|dialog| dialog.show(ctx, self.job.is_some() || self.audio_devices.busy()));
        match action {
            Some(Action::Cancel) => self.audio_settings = None,
            Some(Action::Apply) => {
                let draft = self.audio_settings.as_ref().unwrap().draft.clone();
                if let Err(error) = self.apply_audio_settings(draft, |plan, settings| {
                    AudioOutput::with_settings(plan, None, settings)
                }) {
                    self.audio_settings.as_mut().unwrap().error = Some(error);
                }
            }
            None => {}
        }
    }
    fn apply_audio_settings(
        &mut self,
        draft: AudioSettings,
        open: impl FnOnce(daw_engine::RenderPlan, &AudioSettings) -> Result<AudioOutput, String>,
    ) -> Result<(), String> {
        if draft == self.settings.audio {
            self.audio_settings = None;
            return Ok(());
        }
        // Input selection has no stream yet. It must not interrupt playback.
        if draft.output_device == self.settings.audio.output_device
            && draft.sample_rate_hz == self.settings.audio.sample_rate_hz
            && draft.sample_format == self.settings.audio.sample_format
            && draft.buffer_size_frames == self.settings.audio.buffer_size_frames
        {
            self.settings.audio = draft;
            self.audio_settings = None;
            return Ok(());
        }
        let mut plan = self.session.plan();
        let frame = self
            .output
            .as_ref()
            .map_or(plan.project.transport.playhead_frame, |output| {
                output.transport_intent().frame
            });
        plan.project.transport.playhead_frame = frame;
        // Keep the accepted stream and preferences until the replacement opens.
        match open(plan, &draft) {
            Ok(output) => {
                self.session.project.transport.playhead_frame = frame;
                self.output = Some(output);
                self.audio_preparation = None;
                self.settings.audio = draft;
                self.configure_audio_cache(false);
                self.audio_devices.error = None;
                self.audio_settings = None;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    pub(super) fn open_audio_settings(&mut self) {
        let mut dialog = AudioDialog::new(self.settings.audio.clone());
        dialog.devices = self.audio_devices.catalog.clone();
        dialog.error = self.audio_devices.error.clone();
        self.audio_settings = Some(dialog);
        if self.audio_devices.catalog.is_none() {
            self.refresh_audio_devices();
        }
    }
    pub(super) fn apply_audio_device_selection(&mut self, draft: AudioSettings) {
        self.audio_devices.error = self
            .apply_audio_settings(draft, |plan, settings| {
                AudioOutput::with_settings(plan, None, settings)
            })
            .err();
    }
    pub(super) fn finish_audio_discovery(&mut self, result: Result<Devices, String>) {
        match result {
            Ok(devices) => {
                self.audio_devices.catalog = Some(devices);
                self.audio_devices.error = None;
            }
            Err(error) => {
                self.audio_devices.catalog = None;
                self.audio_devices.error = Some(error);
            }
        }
        if let Some(dialog) = &mut self.audio_settings {
            dialog.devices = self.audio_devices.catalog.clone();
            dialog.error = self.audio_devices.error.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_output::{Configuration, SampleFormat};

    fn id(name: &str) -> DeviceId {
        DeviceId {
            name: name.into(),
            occurrence: 0,
        }
    }
    fn devices() -> Devices {
        let output = id("Speakers");
        Devices {
            outputs: vec![DeviceInfo {
                id: output.clone(),
                configurations: vec![Configuration {
                    channels: 2,
                    format: SampleFormat::F32,
                    min_rate_hz: 44100,
                    max_rate_hz: 48000,
                    buffer_range: Some((64, 1024)),
                }],
            }],
            inputs: vec![DeviceInfo {
                id: id("Microphone"),
                configurations: vec![],
            }],
            default_output: Some(output),
            default_input: Some(id("Microphone")),
        }
    }
    fn frame(
        ctx: &egui::Context,
        dialog: &mut AudioDialog,
        events: Vec<egui::Event>,
    ) -> (Option<Action>, Vec<egui::epaint::ClippedShape>) {
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| {
                action = dialog.show(ui.ctx(), false);
            },
        );
        output.textures_delta.clear();
        (action, output.shapes)
    }
    #[test]
    fn dialog_exposes_controls_and_escape_closes_popup_before_discarding_draft() {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        let mut dialog = AudioDialog::new(AudioSettings::default());
        dialog.devices = Some(devices());
        frame(&ctx, &mut dialog, vec![]);
        let shapes = frame(&ctx, &mut dialog, vec![]).1;
        let text_position = |label: &str| {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing audio setting: {label}"))
        };
        for label in [
            "Output sample rate",
            "Output bit depth",
            "Output buffer size",
            "Apply",
            "Cancel",
        ] {
            text_position(label);
        }
        for label in ["Output device", "Input device", "Refresh devices"] {
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)));
        }
        let point = text_position("Automatic (driver default)");
        let pointer = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            &ctx,
            &mut dialog,
            vec![egui::Event::PointerMoved(point), pointer(true)],
        );
        frame(&ctx, &mut dialog, vec![pointer(false)]);
        assert!(egui::Popup::is_any_open(&ctx));
        let escape = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        assert_eq!(frame(&ctx, &mut dialog, vec![escape(true)]).0, None);
        assert!(!egui::Popup::is_any_open(&ctx));
        frame(&ctx, &mut dialog, vec![escape(false)]);
        assert_eq!(
            frame(&ctx, &mut dialog, vec![escape(true)]).0,
            Some(Action::Cancel)
        );
    }
    #[test]
    fn main_settings_selects_devices_without_editing_quality_defaults() {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        let catalog = devices();
        let mut preferences = settings::AppSettings {
            audio: AudioSettings {
                sample_rate_hz: Some(48000),
                sample_format: Some(SampleFormat::F32),
                buffer_size_frames: Some(256),
                ..Default::default()
            },
            ..Default::default()
        };
        let before = preferences.clone();
        let mut controls = settings::AudioControls {
            devices: Some(&catalog),
            enabled: true,
            ..Default::default()
        };
        let mut render = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    settings::show(
                        ui.ctx(),
                        &mut preferences,
                        &Default::default(),
                        true,
                        &mut None,
                        &mut controls,
                    );
                },
            );
            output.textures_delta.clear();
            output.shapes
        };
        fn point(shapes: &[egui::epaint::ClippedShape], label: &str) -> Pos2 {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing main setting: {label}"))
        }
        render(vec![]);
        let shapes = render(vec![]);
        for label in [
            "Output device",
            "Input device",
            "Audio quality defaults…",
            "Refresh devices",
        ] {
            point(&shapes, label);
        }
        // The first System default dropdown belongs to Output device.
        let output_point = point(&shapes, "System default");
        let button = |position, pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        render(vec![
            egui::Event::PointerMoved(output_point),
            button(output_point, true),
        ]);
        render(vec![button(output_point, false)]);
        let shapes = render(vec![]);
        let speaker = point(&shapes, "Speakers");
        render(vec![
            egui::Event::PointerMoved(speaker),
            button(speaker, true),
        ]);
        render(vec![button(speaker, false)]);
        let shapes = render(vec![]);
        let quality = point(&shapes, "Audio quality defaults…");
        render(vec![
            egui::Event::PointerMoved(quality),
            button(quality, true),
        ]);
        render(vec![button(quality, false)]);
        assert!(controls.quality_requested);
        let selection = controls.selection.unwrap();
        assert_eq!(selection.output_device, Some(id("Speakers")));
        assert_eq!(selection.sample_rate_hz, before.audio.sample_rate_hz);
        assert_eq!(selection.sample_format, before.audio.sample_format);
        assert_eq!(
            selection.buffer_size_frames,
            before.audio.buffer_size_frames
        );
        // The caller probes the output before publishing the selection.
        assert_eq!(preferences, before);
    }
    #[test]
    fn unchanged_preferences_do_not_open_a_stream_or_edit_project() {
        let mut app = DawUi::default();
        let before = app.session.project.clone();
        app.audio_settings = Some(AudioDialog::new(app.settings.audio.clone()));
        app.apply_audio_settings(app.settings.audio.clone(), |_, _| {
            panic!("No-op must not open output")
        })
        .unwrap();
        assert!(app.audio_settings.is_none());
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
    #[test]
    fn missing_devices_and_incompatible_formats_block_apply() {
        let mut dialog = AudioDialog::new(AudioSettings::default());
        assert!(!dialog.valid());
        dialog.devices = Some(devices());
        assert!(dialog.valid());
        dialog.draft.sample_format = Some(SampleFormat::I16);
        assert!(!dialog.valid());
        dialog.draft.sample_format = None;
        dialog.draft.output_device = Some(id("Disconnected speakers"));
        assert!(!dialog.valid());
        assert!(
            device_label(dialog.draft.output_device.as_ref(), &devices().outputs)
                .contains("unavailable")
        );
        dialog.draft.output_device = None;
        dialog.draft.input_device = Some(id("Disconnected microphone"));
        // Reserved input preferences must not block output quality changes.
        assert!(dialog.valid());
        dialog.draft.input_device = Some(id("Microphone"));
        assert!(dialog.valid());
        dialog.draft.buffer_size_frames = Some(32);
        assert!(!dialog.valid());
    }
    #[test]
    fn rejected_stream_retains_preferences_project_and_draft() {
        let mut app = DawUi::default();
        let before_settings = app.settings();
        let before_project = app.session.project.clone();
        let draft = AudioSettings {
            sample_rate_hz: Some(96000),
            ..Default::default()
        };
        app.audio_settings = Some(AudioDialog::new(draft.clone()));
        let error = app
            .apply_audio_settings(draft.clone(), |plan, settings| {
                assert_eq!(settings, &draft);
                assert_eq!(plan.project, before_project);
                Err("Test device rejected the configuration".into())
            })
            .unwrap_err();
        assert_eq!(app.settings(), before_settings);
        assert_eq!(app.session.project, before_project);
        assert!(!app.dirty);
        assert!(app.output.is_none());
        let dialog = app.audio_settings.as_ref().unwrap();
        assert_eq!(dialog.draft, draft);
        assert_eq!(error, "Test device rejected the configuration");
    }
    #[test]
    fn input_only_change_persists_without_opening_stream_or_editing_project() {
        let mut app = DawUi::default();
        let before_project = app.session.project.clone();
        let draft = AudioSettings {
            input_device: Some(id("Microphone")),
            ..Default::default()
        };
        app.audio_settings = Some(AudioDialog::new(draft.clone()));
        app.apply_audio_settings(draft.clone(), |_, _| {
            panic!("Input-only change must not open output")
        })
        .unwrap();
        assert_eq!(app.settings.audio, draft);
        assert_eq!(app.session.project, before_project);
        assert!(!app.dirty);
        assert!(app.audio_settings.is_none());
        assert_eq!(
            settings::AppSettings::from_json(Some(&app.settings.to_json())),
            app.settings
        );
    }
    #[test]
    fn all_audio_preferences_round_trip_and_legacy_settings_keep_defaults() {
        let settings = settings::AppSettings {
            audio: AudioSettings {
                output_device: Some(id("Speakers")),
                input_device: Some(id("Microphone")),
                sample_rate_hz: Some(96000),
                sample_format: Some(SampleFormat::I24),
                buffer_size_frames: Some(256),
            },
            ..Default::default()
        };
        assert_eq!(
            settings::AppSettings::from_json(Some(&settings.to_json())),
            settings
        );
        let legacy = settings::AppSettings::from_json(Some(r#"{"theme":"light","accent":"teal"}"#));
        assert_eq!(legacy.audio, AudioSettings::default());
        assert_eq!(legacy.theme, settings::ColorTheme::Light);
        assert_eq!(legacy.accent, settings::AccentColor::Teal);
    }
}
