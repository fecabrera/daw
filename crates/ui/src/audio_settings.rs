//! Machine-local audio device preferences, separate from project processing settings.
use super::*;
use daw_output::{AudioSettings, DeviceId, DeviceInfo, Devices};

pub(super) struct AudioDialog {
    draft: daw_core::AudioQuality,
    error: Option<String>,
}

pub(super) struct DevicesState {
    discover: fn() -> Result<Devices, String>,
    pub catalog: Option<Devices>,
    pub error: Option<String>,
    pending: Option<std::sync::mpsc::Receiver<Result<Devices, String>>>,
}
impl Default for DevicesState {
    fn default() -> Self {
        Self {
            discover: || {
                #[cfg(test)]
                {
                    Ok(Devices::default())
                }
                #[cfg(not(test))]
                {
                    daw_output::discover_devices()
                }
            },
            catalog: None,
            error: None,
            pending: None,
        }
    }
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
            crate::menus::Dropdown::from_id_salt(label)
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
    fn new(settings: daw_core::AudioQuality) -> Self {
        Self {
            draft: settings,
            error: None,
        }
    }
    fn valid(&self) -> bool {
        self.draft.validate().is_ok()
    }
    fn show(&mut self, ctx: &egui::Context, busy: bool) -> Option<Action> {
        let popup_was_open = egui::Popup::is_any_open(ctx);
        let mut action = None;
        dialogs::Dialog::new("Audio quality defaults", "", &[] as &[(&str, Action)]).show_with_content(ctx, |ui| {
            let width = ui.painter().layout_no_wrap("Output bit depth".into(), egui::TextStyle::Body.resolve(ui.style()), theme::palette(ctx).text).size().x;
            ui.add_enabled_ui(!busy, |ui| {
                rows::field(ui, width, |ui| {ui.label("Sample rate");}, |ui| {
                    crate::menus::Dropdown::from_id_salt("default_sample_rate").selected_text(format!("{} Hz", self.draft.sample_rate_hz)).width(ui.available_width()).show_ui(ui, |ui| {for rate in daw_core::SAMPLE_RATES {ui.selectable_value(&mut self.draft.sample_rate_hz, rate, format!("{rate} Hz"));}});
                });
                rows::field(ui, width, |ui| {ui.label("Output bit depth");}, |ui| {
                    crate::menus::Dropdown::from_id_salt("default_output_format").selected_text(self.draft.output_sample_format.label()).width(ui.available_width()).show_ui(ui, |ui| {for format in daw_core::SampleFormat::ALL {ui.selectable_value(&mut self.draft.output_sample_format, format, format.label());}});
                });
                rows::field(ui, width, |ui| {ui.label("Buffer size");}, |ui| {
                    crate::menus::Dropdown::from_id_salt("default_buffer_size").selected_text(format!("{} samples", self.draft.buffer_size_frames)).width(ui.available_width()).show_ui(ui, |ui| {for frames in daw_core::BUFFER_SIZES {ui.selectable_value(&mut self.draft.buffer_size_frames, frames, format!("{frames} samples"));}});
                });
                rows::advice(ui, "These values are copied into new projects. Use Project Settings to change the current project.");
                rows::advice(ui, "Output bit depth selects the device stream format. The driver may use a different hardware format. Internal processing and export settings stay separate.");
                if let Some(error) = &self.error {ui.colored_label(theme::palette(ctx).error, error);}
                rows::wrapped(ui, toolbars::CONTROL_HEIGHT, |ui| {
                    if ui.add_enabled(self.valid(), egui::Button::new("Apply")).clicked() {action = Some(Action::Apply);}
                    if ui.button("Cancel").clicked() {action = Some(Action::Cancel);}
                });
            });
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
        let discover = self.audio_devices.discover;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("audio-device-discovery".into())
            .spawn(move || {
                let _ = sender.send(discover());
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
            .and_then(|dialog| dialog.show(ctx, self.job.is_some()));
        match action {
            Some(Action::Cancel) => self.audio_settings = None,
            Some(Action::Apply) => {
                let draft = self.audio_settings.as_ref().unwrap().draft;
                if let Err(error) = self.apply_audio_quality_defaults(draft) {
                    self.audio_settings.as_mut().unwrap().error = Some(error);
                }
            }
            None => {}
        }
    }
    fn apply_audio_quality_defaults(
        &mut self,
        draft: daw_core::AudioQuality,
    ) -> Result<(), String> {
        draft.validate().map_err(|e| e.to_string())?;
        self.settings.audio_quality = draft;
        self.audio_settings = None;
        Ok(())
    }
    fn apply_audio_device_settings(
        &mut self,
        draft: AudioSettings,
        open: impl FnOnce(daw_engine::RenderPlan, &AudioSettings) -> Result<AudioOutput, String>,
    ) -> Result<(), String> {
        if draft == self.settings.audio {
            self.audio_settings = None;
            return Ok(());
        }
        // Input selection has no stream yet. It must not interrupt playback.
        if draft.output_device == self.settings.audio.output_device {
            self.settings.audio = draft;
            self.audio_settings = None;
            return Ok(());
        }
        if !self.media_ready() {
            return Err(
                "Wait for project audio preparation before changing the output device".into(),
            );
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
        self.audio_settings = Some(AudioDialog::new(self.settings.audio_quality));
    }
    pub(super) fn apply_audio_device_selection(&mut self, draft: AudioSettings) {
        self.audio_devices.error = self
            .apply_audio_device_settings(draft, |plan, settings| {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_output::{Configuration, SampleFormat};

    #[test]
    fn discovery_uses_injected_results_and_reports_worker_failure_without_hardware() {
        let ctx = egui::Context::default();
        let mut app = DawUi::default();
        for fail in [false, true, false] {
            app.audio_devices.discover = if fail {
                || Err("injected device discovery failure".into())
            } else {
                || Ok(devices())
            };
            app.refresh_audio_devices();
            assert!(app.audio_devices.busy());
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while app.audio_devices.busy() {
                app.poll_audio_devices(&ctx);
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            if fail {
                assert!(app.audio_devices.catalog.is_none());
                assert_eq!(
                    app.audio_devices.error.as_deref(),
                    Some("injected device discovery failure")
                );
            } else {
                assert_eq!(
                    app.audio_devices.catalog.as_ref().unwrap().outputs[0]
                        .id
                        .name,
                    "Speakers"
                );
                assert!(app.audio_devices.error.is_none());
            }
            assert!(!app.dirty);
            assert!(app.output.is_none());
        }
    }

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
    fn assert_advice(ctx: &egui::Context, shapes: &[egui::epaint::ClippedShape], label: &str) {
        let text = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                _ => None,
            })
            .expect("advice message");
        let size = egui::TextStyle::Small.resolve(&ctx.global_style()).size;
        let color = theme::palette(ctx).secondary;
        assert!(text.galley.job.sections.iter().all(|section| section.format.font_id.size == size && section.format.color == color));
    }
    #[test]
    fn dialog_exposes_controls_and_escape_closes_popup_before_discarding_draft() {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        let mut dialog = AudioDialog::new(daw_core::AudioQuality::default());
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
            "Sample rate",
            "Output bit depth",
            "Buffer size",
            "Apply",
            "Cancel",
        ] {
            text_position(label);
        }
        for label in ["Output device", "Input device", "Refresh devices"] {
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)));
        }
        for message in [
            "These values are copied into new projects. Use Project Settings to change the current project.",
            "Output bit depth selects the device stream format. The driver may use a different hardware format. Internal processing and export settings stay separate.",
        ] {
            assert_advice(&ctx, &shapes, message);
        }
        let point = text_position("32-bit float");
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
            audio_quality: daw_core::AudioQuality {
                sample_rate_hz: 48000,
                output_sample_format: SampleFormat::F32,
                buffer_size_frames: 256,
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
        assert_advice(
            &ctx,
            &shapes,
            "Changing output stops playback. Input selection is saved for future recording; input monitoring is not available.",
        );
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
        assert_eq!(preferences.audio_quality, before.audio_quality);
        // The caller probes the output before publishing the selection.
        assert_eq!(preferences, before);
    }
    #[test]
    fn unchanged_preferences_do_not_open_a_stream_or_edit_project() {
        let mut app = DawUi::default();
        let before = app.session.project.clone();
        app.audio_settings = Some(AudioDialog::new(app.settings.audio_quality));
        app.apply_audio_device_settings(app.settings.audio.clone(), |_, _| {
            panic!("No-op must not open output")
        })
        .unwrap();
        assert!(app.audio_settings.is_none());
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
    #[test]
    fn defaults_are_concrete_and_do_not_depend_on_connected_devices() {
        let mut dialog = AudioDialog::new(Default::default());
        assert!(dialog.valid());
        assert_eq!(dialog.draft.sample_rate_hz, 48000);
        assert_eq!(dialog.draft.output_sample_format, SampleFormat::F32);
        assert_eq!(dialog.draft.buffer_size_frames, 512);
        dialog.draft.output_sample_format = SampleFormat::I24;
        assert!(dialog.valid());
        dialog.draft.buffer_size_frames = 33;
        assert!(!dialog.valid());
        dialog.draft.buffer_size_frames = 256;
        dialog.draft.sample_rate_hz = 12345;
        assert!(!dialog.valid());
        assert!(
            device_label(Some(&id("Disconnected speakers")), &devices().outputs)
                .contains("unavailable")
        );
    }
    #[test]
    fn rejected_stream_retains_preferences_project_and_draft() {
        let mut app = DawUi::new_with_audio_defaults(
            None,
            daw_core::AudioQuality {
                sample_rate_hz: 44100,
                output_sample_format: SampleFormat::I16,
                buffer_size_frames: 128,
            },
        );
        app.settings.audio_quality = daw_core::AudioQuality {
            sample_rate_hz: 96000,
            output_sample_format: SampleFormat::I24,
            buffer_size_frames: 256,
        };
        let before_settings = app.settings();
        let before_project = app.session.project.clone();
        let draft = AudioSettings {
            output_device: Some(id("Speakers")),
            ..Default::default()
        };
        let error = app
            .apply_audio_device_settings(draft.clone(), |plan, settings| {
                assert_eq!(settings, &draft);
                assert_eq!(plan.project, before_project);
                Err("Test device rejected the configuration".into())
            })
            .unwrap_err();
        assert_eq!(app.settings(), before_settings);
        assert_eq!(app.session.project, before_project);
        assert!(!app.dirty);
        assert!(app.output.is_none());
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
        app.apply_audio_device_settings(draft.clone(), |_, _| {
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
            },
            audio_quality: daw_core::AudioQuality {
                sample_rate_hz: 96000,
                output_sample_format: SampleFormat::I24,
                buffer_size_frames: 256,
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
    #[test]
    fn changing_defaults_only_seeds_future_projects() {
        let initial = daw_core::AudioQuality {
            sample_rate_hz: 44100,
            output_sample_format: SampleFormat::I16,
            buffer_size_frames: 128,
        };
        let mut app = DawUi::new_with_audio_defaults(None, initial);
        let before = app.session.project.clone();
        let next = daw_core::AudioQuality {
            sample_rate_hz: 96000,
            output_sample_format: SampleFormat::I24,
            buffer_size_frames: 256,
        };
        app.audio_settings = Some(AudioDialog::new(next));
        app.apply_audio_quality_defaults(next).unwrap();
        assert_eq!(app.settings.audio_quality, next);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.output.is_none());
        assert!(app.audio_settings.is_none());
        app.pending = Some(crate::Action::New);
        app.perform_pending();
        assert_eq!(app.session.project.audio_quality(), next);
        assert_ne!(app.session.project.project_id, before.project_id);
        assert!(!app.dirty);
    }
    #[test]
    fn legacy_device_quality_becomes_new_project_defaults() {
        let legacy = settings::AppSettings::from_json(Some(
            r#"{"audio":{"output_device":{"name":"Speakers","occurrence":0},"sample_rate_hz":96000,"sample_format":"i24","buffer_size_frames":256}}"#,
        ));
        assert_eq!(
            legacy.audio_quality,
            daw_core::AudioQuality {
                sample_rate_hz: 96000,
                output_sample_format: SampleFormat::I24,
                buffer_size_frames: 256
            }
        );
        assert_eq!(legacy.audio.output_device, Some(id("Speakers")));
        assert_eq!(
            settings::AppSettings::from_json(Some(&legacy.to_json())),
            legacy
        );
        let automatic = settings::AppSettings::from_json(Some(
            r#"{"audio":{"sample_rate_hz":null,"sample_format":null,"buffer_size_frames":null}}"#,
        ));
        assert_eq!(automatic.audio_quality, daw_core::AudioQuality::default());
    }
    #[test]
    fn opening_project_uses_saved_quality_instead_of_new_project_defaults() {
        let folder =
            std::env::temp_dir().join(format!("daw-project-quality-{}", daw_core::Id::new_v4()));
        let saved = daw_core::AudioQuality {
            sample_rate_hz: 44100,
            output_sample_format: SampleFormat::I16,
            buffer_size_frames: 128,
        };
        let mut session = Session::default();
        session.project = daw_core::Project::new_with_audio_quality(saved).unwrap();
        session.save(&folder).unwrap();
        let defaults = daw_core::AudioQuality {
            sample_rate_hz: 96000,
            output_sample_format: SampleFormat::I24,
            buffer_size_frames: 256,
        };
        let mut app = DawUi::new_with_audio_defaults(Some(folder.clone()), defaults);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.job.is_some() {
            app.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.session.project.audio_quality(), saved);
        assert_eq!(app.settings.audio_quality, defaults);
        assert!(!app.dirty);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
