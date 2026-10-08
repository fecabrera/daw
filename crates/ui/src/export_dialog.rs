use daw_media::{ExportFormat, ExportSettings, Mp3Bitrate, WavCodec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Export,
    Cancel,
}

pub fn show(ctx: &egui::Context, settings: &mut ExportSettings) -> Option<Action> {
    let action = crate::dialogs::Dialog::new(
        "Export audio",
        "Stereo · 48 kHz",
        &[("Export…", Action::Export), ("Cancel", Action::Cancel)],
    )
    .show_with_content(ctx, |ui| {
        egui::Grid::new("export_settings")
            .num_columns(2)
            .spacing(egui::Vec2::new(12.0, 8.0))
            .show(ui, |ui| {
                ui.label("Format");
                egui::ComboBox::from_id_salt("export_format")
                    .selected_text(settings.format.label())
                    .show_ui(ui, |ui| {
                        for format in ExportFormat::ALL {
                            ui.selectable_value(&mut settings.format, format, format.label());
                        }
                    });
                ui.end_row();
                ui.label("Codec");
                match settings.format {
                    ExportFormat::Wav => {
                        egui::ComboBox::from_id_salt("export_codec")
                            .selected_text(settings.wav_codec.label())
                            .show_ui(ui, |ui| {
                                for codec in WavCodec::ALL {
                                    ui.selectable_value(
                                        &mut settings.wav_codec,
                                        codec,
                                        codec.label(),
                                    );
                                }
                            });
                    }
                    ExportFormat::Mp3 => {
                        ui.label("MPEG Layer III (LAME)");
                    }
                }
                ui.end_row();
                if settings.format == ExportFormat::Mp3 {
                    ui.label("Bitrate");
                    egui::ComboBox::from_id_salt("export_bitrate")
                        .selected_text(format!("{} kbps", settings.mp3_bitrate.kbps()))
                        .show_ui(ui, |ui| {
                            for bitrate in Mp3Bitrate::ALL {
                                ui.selectable_value(
                                    &mut settings.mp3_bitrate,
                                    bitrate,
                                    format!("{} kbps", bitrate.kbps()),
                                );
                            }
                        });
                    ui.end_row();
                    ui.label("Bitrate mode");
                    ui.label("Constant (CBR)");
                    ui.end_row();
                }
            });
    });
    action.or_else(|| {
        ctx.input(|input| input.key_pressed(egui::Key::Escape))
            .then_some(Action::Cancel)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        settings: &mut ExportSettings,
        events: Vec<egui::Event>,
    ) -> (Option<Action>, Vec<egui::epaint::ClippedShape>) {
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| action = show(ui.ctx(), settings),
        );
        output.textures_delta.clear();
        (action, output.shapes)
    }
    fn click(ctx: &egui::Context, settings: &mut ExportSettings, label: &str) -> Option<Action> {
        let (_, shapes) = frame(ctx, settings, vec![]);
        let point = shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing dialog item: {label}"));
        let event = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            ctx,
            settings,
            vec![egui::Event::PointerMoved(point), event(true)],
        );
        frame(ctx, settings, vec![event(false)]).0
    }
    #[test]
    fn export_dialog_switches_formats_codecs_and_bitrates_and_returns_actions() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        let mut settings = ExportSettings::default();
        frame(&ctx, &mut settings, vec![]);
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, "PCM 24-bit");
        click(&ctx, &mut settings, "IEEE float 32-bit");
        assert_eq!(settings.wav_codec, WavCodec::Float32);
        click(&ctx, &mut settings, "WAV");
        click(&ctx, &mut settings, "MP3");
        assert_eq!(settings.format, ExportFormat::Mp3);
        let (_, shapes) = frame(&ctx, &mut settings, vec![]);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "MPEG Layer III (LAME)")));
        click(&ctx, &mut settings, "192 kbps");
        click(&ctx, &mut settings, "320 kbps");
        assert_eq!(settings.mp3_bitrate, Mp3Bitrate::Kbps320);
        click(&ctx, &mut settings, "MP3");
        click(&ctx, &mut settings, "WAV");
        assert_eq!(settings.wav_codec, WavCodec::Float32);
        assert_eq!(click(&ctx, &mut settings, "Export…"), Some(Action::Export));
        assert_eq!(click(&ctx, &mut settings, "Cancel"), Some(Action::Cancel));
    }
}
