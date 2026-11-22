use crate::dialogs::FooterAction;
use crate::{
    MonitorDrag, PositionStep, monitor_drag_position, musical_sections, transport_controls,
    transport_monitor_control,
};
use daw_core::Project;
use daw_media::{ExportChannels, ExportFormat, ExportSettings, Mp3Bitrate, WavCodec};
use daw_project::ExportRegion;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub settings: ExportSettings,
    pub region: ExportRegion,
}

pub(super) struct Draft {
    pub request: Request,
    end_limit: u64,
    drag: [Option<MonitorDrag>; 2],
}

impl Draft {
    pub fn new(request: Request, project_end: u64) -> Self {
        Self {
            request,
            end_limit: project_end.max(request.region.end_frame),
            drag: [None, None],
        }
    }

    fn range_controls(&mut self, ui: &mut egui::Ui, project: &Project) {
        let clock = Arc::new(project.musical_clock().expect("validated project timing"));
        let steps = [true, false].map(|bars| PositionStep::Musical {
            clock: clock.clone(),
            bars,
        });
        ui.horizontal(|ui| {
            transport_controls::readout_group(ui, |ui| {
                let region = &mut self.request.region;
                let (bar, beat) = clock.position(region.start_frame);
                let start = transport_monitor_control(
                    ui,
                    &format!("Start {bar:04}.{beat:02}"),
                    "Export start",
                    0,
                    egui::Sense::hover(),
                );
                if let Some(frame) = monitor_drag_position(
                    ui,
                    &musical_sections(ui, &start, 0),
                    &mut self.drag[0],
                    region.start_frame,
                    &steps,
                    0..=region.end_frame.saturating_sub(1),
                ) {
                    region.start_frame = frame;
                }
                transport_controls::readout_separator(ui);
                let (bar, beat) = clock.position(region.end_frame);
                let end = transport_monitor_control(
                    ui,
                    &format!("End {bar:04}.{beat:02}"),
                    "Export end",
                    0,
                    egui::Sense::hover(),
                );
                if let Some(frame) = monitor_drag_position(
                    ui,
                    &musical_sections(ui, &end, 0),
                    &mut self.drag[1],
                    region.end_frame,
                    &steps,
                    region.start_frame.saturating_add(1).min(self.end_limit)..=self.end_limit,
                ) {
                    region.end_frame = frame;
                }
            });
        });
        ui.checkbox(&mut self.request.region.include_tail, "Include audio tail");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Export,
    Cancel,
}

pub(super) fn show(
    ctx: &egui::Context,
    draft: &mut Draft,
    project: &Project,
    surface: Option<&mut egui::Ui>,
) -> Option<Action> {
    let actions = [
        FooterAction::accept("Export", Action::Export),
        FooterAction::cancel("Cancel", Action::Cancel),
    ];
    let dialog = crate::dialogs::Dialog::new("Export audio", "", &actions);
    let content = |ui: &mut egui::Ui| {
        draft.range_controls(ui, project);
        let settings = &mut draft.request.settings;
        let rate_label = |rate: u32| format!("{} kHz", f64::from(rate) / 1000.0);
        egui::Grid::new("export_settings")
            .num_columns(2)
            .spacing(egui::Vec2::new(12.0, 8.0))
            .show(ui, |ui| {
                ui.label("Format");
                crate::menus::Dropdown::from_id_salt("export_format")
                    .selected_text(settings.format.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for format in ExportFormat::ALL {
                            ui.selectable_value(&mut settings.format, format, format.label());
                        }
                    });
                ui.end_row();
                if settings
                    .sample_rate_hz
                    .is_some_and(|rate| !settings.format.export_sample_rates().contains(&rate))
                {
                    settings.sample_rate_hz = None;
                }
                ui.label("Codec");
                match settings.format {
                    ExportFormat::Wav => {
                        crate::menus::Dropdown::from_id_salt("export_codec")
                            .selected_text(settings.wav_codec.label())
                            .width(ui.available_width())
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
                    ExportFormat::Aac => {
                        ui.label("AAC-LC (ADTS)");
                    }
                }
                ui.end_row();
                ui.label("Channels");
                crate::menus::Dropdown::from_id_salt("export_channels")
                    .selected_text(settings.channels.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for channels in ExportChannels::ALL {
                            ui.selectable_value(&mut settings.channels, channels, channels.label());
                        }
                    });
                ui.end_row();
                ui.label("Sample rate");
                let rate = settings.output_sample_rate(project.sample_rate_hz);
                crate::menus::Dropdown::from_id_salt("export_sample_rate")
                    .selected_text(rate_label(rate))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for &candidate in settings.format.export_sample_rates() {
                            if ui
                                .selectable_label(rate == candidate, rate_label(candidate))
                                .clicked()
                            {
                                settings.sample_rate_hz = Some(candidate);
                            }
                        }
                    });
                ui.end_row();
                if settings.format != ExportFormat::Wav {
                    let bitrate = if settings.format == ExportFormat::Aac {
                        &mut settings.aac_bitrate
                    } else {
                        &mut settings.mp3_bitrate
                    };
                    ui.label("Bitrate");
                    crate::menus::Dropdown::from_id_salt("export_bitrate")
                        .selected_text(format!("{} kbps", bitrate.kbps()))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for candidate in Mp3Bitrate::ALL {
                                ui.selectable_value(
                                    bitrate,
                                    candidate,
                                    format!("{} kbps", candidate.kbps()),
                                );
                            }
                        });
                    ui.end_row();
                    ui.label("Bitrate mode");
                    ui.label(if settings.format == ExportFormat::Aac {
                        "Variable (target bitrate)"
                    } else {
                        "Constant (CBR)"
                    });
                    ui.end_row();
                }
            });
    };
    let action = dialog.show_surface(ctx, surface, content);
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
        draft: &mut Draft,
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
            |ui| action = show(ui.ctx(), draft, &Project::default(), None),
        );
        output.textures_delta.clear();
        (action, output.shapes)
    }
    fn click(ctx: &egui::Context, draft: &mut Draft, label: &str) -> Option<Action> {
        let (_, shapes) = frame(ctx, draft, vec![]);
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
            draft,
            vec![egui::Event::PointerMoved(point), event(true)],
        );
        frame(ctx, draft, vec![event(false)]).0
    }
    #[test]
    fn grouped_range_readouts_drag_independently() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut draft = Draft::new(
            Request {
                settings: ExportSettings::default(),
                region: ExportRegion {
                    start_frame: 0,
                    end_frame: 480_000,
                    include_tail: true,
                },
            },
            480_000,
        );
        frame(&ctx, &mut draft, vec![]);
        for is_start in [true, false] {
            let (_, shapes) = frame(&ctx, &mut draft, vec![]);
            let digits: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text().len() == 1
                            && text.galley.text().as_bytes()[0].is_ascii_digit() =>
                    {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(digits.len(), 12);
            let point = digits[if is_start { 5 } else { 11 }];
            let end = point + egui::vec2(0.0, if is_start { -8.0 } else { 8.0 });
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            let before = draft.request.region;
            frame(
                &ctx,
                &mut draft,
                vec![egui::Event::PointerMoved(point), button(point, true)],
            );
            frame(&ctx, &mut draft, vec![egui::Event::PointerMoved(end)]);
            frame(&ctx, &mut draft, vec![button(end, false)]);
            let after = draft.request.region;
            if is_start {
                assert!(after.start_frame > before.start_frame);
                assert_eq!(after.end_frame, before.end_frame);
            } else {
                assert_eq!(after.start_frame, before.start_frame);
                assert!(after.end_frame < before.end_frame);
            }
            assert!(after.start_frame < after.end_frame);
        }
    }

    #[test]
    fn export_dialog_switches_formats_codecs_and_bitrates_and_returns_actions() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut draft = Draft::new(
            Request {
                settings: ExportSettings::default(),
                region: ExportRegion {
                    start_frame: 0,
                    end_frame: 96_000,
                    include_tail: true,
                },
            },
            96_000,
        );
        frame(&ctx, &mut draft, vec![]);
        let (_, shapes) = frame(&ctx, &mut draft, vec![]);
        assert!(draft.request.region.include_tail);
        assert_eq!(click(&ctx, &mut draft, "Include audio tail"), None);
        assert!(!draft.request.region.include_tail);
        let export = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Export" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        let accent = crate::theme::accent(&ctx);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.fill == accent && rect.rect.contains(export)
        )));
        click(&ctx, &mut draft, "PCM 24-bit");
        click(&ctx, &mut draft, "IEEE float 32-bit");
        assert_eq!(draft.request.settings.wav_codec, WavCodec::Float32);
        click(&ctx, &mut draft, "Stereo");
        click(&ctx, &mut draft, "Mono");
        assert_eq!(draft.request.settings.channels, ExportChannels::Mono);
        click(&ctx, &mut draft, "48 kHz");
        let (_, choices) = frame(&ctx, &mut draft, vec![]);
        assert!(!choices.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("Automatic"))));
        click(&ctx, &mut draft, "192 kHz");
        assert_eq!(draft.request.settings.sample_rate_hz, Some(192_000));
        click(&ctx, &mut draft, "WAV");
        click(&ctx, &mut draft, "MP3");
        assert_eq!(draft.request.settings.format, ExportFormat::Mp3);
        assert_eq!(draft.request.settings.sample_rate_hz, None);
        click(&ctx, &mut draft, "48 kHz");
        click(&ctx, &mut draft, "44.1 kHz");
        let (_, shapes) = frame(&ctx, &mut draft, vec![]);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "MPEG Layer III (LAME)")));
        click(&ctx, &mut draft, "192 kbps");
        click(&ctx, &mut draft, "320 kbps");
        assert_eq!(draft.request.settings.mp3_bitrate, Mp3Bitrate::Kbps320);
        click(&ctx, &mut draft, "MP3");
        click(&ctx, &mut draft, "AAC");
        assert_eq!(draft.request.settings.format, ExportFormat::Aac);
        assert_eq!(draft.request.settings.channels, ExportChannels::Mono);
        assert_eq!(draft.request.settings.sample_rate_hz, Some(44_100));
        let (_, shapes) = frame(&ctx, &mut draft, vec![]);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "AAC-LC (ADTS)")));
        click(&ctx, &mut draft, "192 kbps");
        click(&ctx, &mut draft, "256 kbps");
        assert_eq!(
            draft.request.settings.aac_bitrate,
            daw_media::AacBitrate::Kbps256
        );
        assert_eq!(draft.request.settings.mp3_bitrate, Mp3Bitrate::Kbps320);
        click(&ctx, &mut draft, "AAC");
        click(&ctx, &mut draft, "WAV");
        assert_eq!(draft.request.settings.wav_codec, WavCodec::Float32);
        assert_eq!(click(&ctx, &mut draft, "Export"), Some(Action::Export));
        assert_eq!(click(&ctx, &mut draft, "Cancel"), Some(Action::Cancel));
    }
}
