//! Conditional audio-to-control settings in the existing destination-owned connection.
use crate::{
    knobs::{self, Knob, KnobSpec},
    panels, rows, theme,
};
use daw_core::{Id, Project, TrackKind};
use daw_unit_core::*;
use egui::{Context, Ui};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(super) struct SourceContext {
    pub choices: Vec<Choice>,
    pub readings: HashMap<Id, [f32; 2]>,
}
#[derive(Clone)]
pub(super) struct Choice {
    pub id: Id,
    pub name: String,
    pub group: &'static str,
    pub available: bool,
}
fn key() -> egui::Id {
    egui::Id::new("audio-sidechain-source-context")
}
pub(super) fn context(ctx: &Context) -> SourceContext {
    ctx.data(|d| d.get_temp(key())).unwrap_or_default()
}
impl crate::DawUi {
    pub(super) fn sidechain_context(&self, ctx: &Context, destination: Id) {
        let project = &self.session.project;
        let destination = project
            .tracks
            .iter()
            .find(|t| {
                t.beat.as_ref().is_some_and(|b| {
                    b.output.id == destination || b.subtracks.iter().any(|s| s.id == destination)
                })
            })
            .map_or(destination, |t| t.id);
        let choices = choices(project, destination);
        let readings = project
            .audio_sidechain_routes()
            .iter()
            .filter_map(|(_, r)| {
                self.output
                    .as_ref()?
                    .modulation_snapshot(daw_engine::ModulationTarget::Sidechain { route: r.id })
                    .map(|v| (r.id, [v[0], v[1]]))
            })
            .collect();
        ctx.data_mut(|d| d.insert_temp(key(), SourceContext { choices, readings }));
    }
}
fn choices(project: &Project, destination: Id) -> Vec<Choice> {
    project
        .tracks
        .iter()
        .map(|t| Choice {
            id: t.id,
            name: t.name.clone(),
            group: match t.kind {
                TrackKind::Group => "Groups",
                TrackKind::Return => "Returns",
                _ => "Tracks",
            },
            available: project.sidechain_source_available(destination, t.id),
        })
        .collect()
}
pub(super) fn source_label(ui: &Ui, settings: AudioSidechain) -> String {
    context(ui.ctx())
        .choices
        .iter()
        .find(|s| s.id == settings.track)
        .map_or_else(
            || "Unavailable".into(),
            |s| format!("{} · {}", s.group, s.name),
        )
}
pub(super) fn source_menu(
    ui: &mut Ui,
    source: &mut Option<ModulationSourceId>,
    settings: &mut Option<AudioSidechain>,
) {
    let ctx = context(ui.ctx());
    if ctx.choices.is_empty() {
        return;
    }
    crate::menus::separator(ui);
    for group in ["Tracks", "Groups", "Returns"] {
        egui::containers::menu::SubMenuButton::from_button(
            egui::Button::new(egui::RichText::new(group).small())
                .right_text(
                    crate::icons::Lucide::ChevronRight
                        .size(crate::icons::TOOLBAR_ICON_SIZE)
                        .image(),
                )
                .image_tint_follows_text_color(true),
        )
        .config(egui::containers::menu::MenuConfig::new().style(crate::menus::small_popup_style))
        .ui(ui, |ui| {
            let width = crate::modulation_panel::source_menu_width(
                ui,
                ctx.choices
                    .iter()
                    .filter(|s| s.group == group)
                    .map(|s| s.name.as_str()),
                false,
            );
            crate::menus::layout(ui, width, |ui| {
                for choice in ctx.choices.iter().filter(|s| s.group == group) {
                    let response = ui.add_enabled(
                        choice.available,
                        egui::Button::new(egui::RichText::new(&choice.name).small())
                            .selected(settings.is_some_and(|s| s.track == choice.id)),
                    );
                    let response = if choice.available {
                        response
                    } else {
                        response.on_hover_text(
                            "This source would create an audio/control feedback cycle",
                        )
                    };
                    if response.clicked() {
                        *source = None;
                        if settings.is_none_or(|s| s.track != choice.id) {
                            *settings = Some(AudioSidechain::new(choice.id));
                        }
                        ui.close();
                    }
                }
            });
        });
    }
}
fn select<T: Copy + PartialEq>(ui: &mut Ui, name: &str, value: &mut T, choices: &[(T, &str)]) {
    let _ = select_response(ui, name, value, choices);
}
fn select_response<T: Copy + PartialEq>(
    ui: &mut Ui,
    name: &str,
    value: &mut T,
    choices: &[(T, &str)],
) -> egui::Response {
    // Keep the shared knob columns fixed. Selectors use the padding around each
    // column, matching the padded width of the ordinary Mod connection strip.
    let (slot, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), knobs::CHOICE_SIZE),
        egui::Sense::hover(),
    );
    let inset =
        daw_unit_ui::layout::unit_column_gap() - f32::from(daw_unit_ui::layout::PANEL_MARGIN);
    let mut expanded = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(name)
            .max_rect(slot.expand2(egui::vec2(inset, 0.0)))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    crate::modulation_panel::selector_row(&mut expanded, name, |ui| {
        crate::menus::Dropdown::from_id_salt(name)
            .small_items()
            .selected_text(
                egui::RichText::new(
                    choices
                        .iter()
                        .find(|(v, _)| v == value)
                        .map_or("", |(_, s)| *s),
                )
                .small(),
            )
            .width(ui.available_width())
            .truncate()
            .show_ui(ui, |ui| {
                for (v, label) in choices {
                    ui.selectable_value(value, *v, egui::RichText::new(*label).small());
                }
            })
            .response
    })
}
fn number(
    ui: &mut Ui,
    value: &mut f32,
    name: &'static str,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    unit: &'static str,
) {
    let format: fn(f32) -> String = match unit {
        "ms" => knobs::format_time,
        "Hz" => knobs::format_frequency,
        _ => |v| format!("{v:.1} dB"),
    };
    let previous = *value;
    let editable = range.start() < range.end();
    let paint_range = if editable {
        range.clone()
    } else {
        *range.start()..=*range.end() + 1.0
    };
    ui.add_enabled_ui(editable, |ui| {
        knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, v| {
            ui.add(Knob::new(
                v,
                KnobSpec::new(
                    name,
                    paint_range,
                    default.clamp(*range.start(), *range.end()),
                    0.1,
                )
                .format(format)
                .value_label(format),
            ))
        })
    });
    // Clamp after the shared knob's floating-point range conversion. A rejected
    // gesture stays a no-op rather than reaching project validation as an error.
    if *value != previous {
        *value = if value.is_finite() {
            value.clamp(*range.start(), *range.end())
        } else {
            previous
        };
    }
}
fn toggle(ui: &mut Ui, value: &mut bool, label: &str) {
    rows::centered(ui, knobs::CHOICE_SIZE, |ui| {
        ui.checkbox(value, egui::RichText::new(label).small())
    });
}
pub(super) fn body(ui: &mut Ui, route: Id, s: &mut AudioSidechain, compact: bool) {
    let height = crate::envelope_panel::body_height(compact);
    let live = context(ui.ctx()).readings.get(&route).copied();
    for section in 0..3 {
        let result = ui
            .push_id((route, "audio-mapping", section), |ui| {
                daw_unit_ui::layout::unit_strip_with_width(
                    ui,
                    height,
                    daw_unit_ui::layout::unit_columns_width(2),
                    egui::UiBuilder::new(),
                    |ui| {
                        let top = ui.cursor().top();
                        ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                        if !compact {
                            daw_unit_ui::layout::unit_columns(ui, 2, |ui, column| {
                                for row in 0..if section == 2 { 1 } else { 3 } {
                                    ui.push_id(row, |ui| match (section, column, row) {
                                        (0, 0, 0) => select(
                                            ui,
                                            "Tap",
                                            &mut s.tap,
                                            &[
                                                (SidechainTap::PreFader, "Pre-fader"),
                                                (SidechainTap::PostFader, "Post-fader"),
                                            ],
                                        ),
                                        (0, 1, 0) => select(
                                            ui,
                                            "Channel",
                                            &mut s.channel,
                                            &[
                                                (SidechainChannel::Stereo, "Stereo"),
                                                (SidechainChannel::Left, "Left"),
                                                (SidechainChannel::Right, "Right"),
                                                (SidechainChannel::Mid, "Mid"),
                                                (SidechainChannel::Side, "Side"),
                                            ],
                                        ),
                                        (0, 0, 1) => select(
                                            ui,
                                            "Detector",
                                            &mut s.detector,
                                            &[
                                                (SidechainDetector::Peak, "Peak"),
                                                (SidechainDetector::Rms, "RMS"),
                                            ],
                                        ),
                                        (0, 1, 1) => toggle(ui, &mut s.filter, "Filter"),
                                        (0, 0, 2) => {
                                            let maximum = (s.low_pass_hz - 0.1).max(s.high_pass_hz);
                                            ui.add_enabled_ui(s.filter, |ui| {
                                                number(
                                                    ui,
                                                    &mut s.high_pass_hz,
                                                    "High-pass",
                                                    20.0..=maximum,
                                                    30.0,
                                                    "Hz",
                                                )
                                            });
                                        }
                                        (0, 1, 2) => {
                                            let minimum = (s.high_pass_hz + 0.1).min(s.low_pass_hz);
                                            ui.add_enabled_ui(s.filter, |ui| {
                                                number(
                                                    ui,
                                                    &mut s.low_pass_hz,
                                                    "Low-pass",
                                                    minimum..=20_000.0,
                                                    20_000.0,
                                                    "Hz",
                                                )
                                            });
                                        }
                                        (1, 0, 0) => {
                                            let maximum = (s.upper_db - 0.1).max(s.lower_db);
                                            number(
                                                ui,
                                                &mut s.lower_db,
                                                "Lower input",
                                                -120.0..=maximum,
                                                -36.0,
                                                "dB",
                                            );
                                        }
                                        (1, 1, 0) => {
                                            let minimum = (s.lower_db + 0.1).min(s.upper_db);
                                            number(
                                                ui,
                                                &mut s.upper_db,
                                                "Upper input",
                                                minimum..=24.0,
                                                -6.0,
                                                "dB",
                                            );
                                        }
                                        (1, 0, 1) => select(
                                            ui,
                                            "Curve",
                                            &mut s.curve,
                                            &[
                                                (SidechainCurve::Decibels, "dB"),
                                                (SidechainCurve::Amplitude, "Amplitude"),
                                            ],
                                        ),
                                        (1, 1, 1) => toggle(ui, &mut s.invert, "Invert"),
                                        (1, column, 2) => {
                                            rows::centered(ui, knobs::CHOICE_SIZE, |ui| {
                                                ui.vertical_centered(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(if column == 0 {
                                                            "Input"
                                                        } else {
                                                            "Value"
                                                        })
                                                        .small()
                                                        .color(theme::palette(ui.ctx()).secondary),
                                                    );
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(
                                                                live.filter(|v| {
                                                                    v[column].is_finite()
                                                                })
                                                                .map_or("—".into(), |v| {
                                                                    if column == 0 {
                                                                        format!("{:.1} dBFS", v[0])
                                                                    } else {
                                                                        format!("{:.3}", v[1])
                                                                    }
                                                                }),
                                                            )
                                                            .font(crate::fonts::numeric(9.0)),
                                                        )
                                                        .wrap_mode(egui::TextWrapMode::Extend),
                                                    );
                                                });
                                            });
                                        }
                                        (2, 0, 0) => number(
                                            ui,
                                            &mut s.attack_ms,
                                            "Attack",
                                            0.0..=10_000.0,
                                            5.0,
                                            "ms",
                                        ),
                                        (2, 1, 0) => number(
                                            ui,
                                            &mut s.release_ms,
                                            "Release",
                                            0.0..=10_000.0,
                                            120.0,
                                            "ms",
                                        ),
                                        _ => {
                                            ui.allocate_exact_size(
                                                egui::Vec2::splat(knobs::CHOICE_SIZE),
                                                egui::Sense::hover(),
                                            );
                                        }
                                    });
                                }
                            });
                            if section == 2 {
                                transfer_preview(ui, s, live);
                            }
                            // Readout text can use less height than a knob row.
                            // Keep every footer at the shared strip baseline.
                            let footer_top = top + height - crate::icons::COMPACT_BUTTON_SIZE;
                            ui.add_space((footer_top - ui.cursor().top()).max(0.0));
                        }
                        rows::centered(
                            ui,
                            if compact {
                                height
                            } else {
                                crate::icons::COMPACT_BUTTON_SIZE
                            },
                            |ui| rows::unit_label(ui, ["Detector", "Mapping", "Response"][section]),
                        );
                    },
                )
            })
            .inner
            .response
            .rect;
        panels::unit_divider(ui, result.left(), result.y_range().into());
    }
}

fn transfer_preview(ui: &mut Ui, s: &AudioSidechain, live: Option<[f32; 2]>) {
    let height = 2.0 * knobs::CHOICE_SIZE + f32::from(theme::TOOLBAR_PADDING);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(14.0, 5.0),
        rect.max - egui::vec2(4.0, 20.0),
    );
    let colors = theme::palette(ui.ctx());
    let stroke = egui::Stroke::new(1.0, colors.secondary);
    ui.painter()
        .line_segment([plot.left_top(), plot.left_bottom()], stroke);
    ui.painter()
        .line_segment([plot.left_bottom(), plot.right_bottom()], stroke);
    let points = (0..=64)
        .map(|i| {
            let x = i as f32 / 64.0;
            let db = s.lower_db + x * (s.upper_db - s.lower_db);
            egui::pos2(
                plot.left() + x * plot.width(),
                plot.bottom() - s.map(10.0_f32.powf(db / 20.0)) * plot.height(),
            )
        })
        .collect();
    ui.painter().add(egui::Shape::line(points, stroke));
    let font = egui::TextStyle::Small.resolve(ui.style());
    for (point, anchor, text) in [
        (
            plot.left_top() - egui::vec2(5.0, 0.0),
            egui::Align2::RIGHT_TOP,
            "1",
        ),
        (
            plot.left_bottom() - egui::vec2(5.0, 0.0),
            egui::Align2::RIGHT_BOTTOM,
            "0",
        ),
    ] {
        ui.painter()
            .text(point, anchor, text, font.clone(), colors.secondary);
    }
    ui.painter().text(
        egui::pos2(plot.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        "Input · dBFS",
        font,
        colors.secondary,
    );
    if let Some([db, value]) = live.filter(|v| v.iter().all(|v| v.is_finite())) {
        let x = ((db - s.lower_db) / (s.upper_db - s.lower_db)).clamp(0.0, 1.0);
        ui.painter().circle_filled(
            egui::pos2(
                plot.left() + x * plot.width(),
                plot.bottom() - value * plot.height(),
            ),
            2.5,
            theme::accent(ui.ctx()),
        );
    }
    response.on_hover_text(format!(
        "Audio-to-control mapping: {:.1} to {:.1} dBFS",
        s.lower_db, s.upper_db
    ));
}

pub(super) fn append_sources(
    sources: &mut Vec<crate::modulation_panel::Source>,
    project: &Project,
) {
    for (_, route) in project.audio_sidechain_routes() {
        let s = route.sidechain.unwrap();
        let Some(track) = project.tracks.iter().find(|t| t.id == s.track) else {
            continue;
        };
        let provider = SidechainProviderSettings {
            id: route.id,
            present: true,
            enabled: true,
        };
        let metadata = provider.modulation_source(s.track, "Audio sidechain", 0);
        sources.push(crate::modulation_panel::Source {
            audio: true,
            id: metadata.id,
            label: format!("Audio · {}", track.name),
            layer_name: "Audio sidechain".into(),
            name: track.name.clone(),
            scope: metadata.scope,
            range: metadata.output.range,
            behavior: metadata.output.behavior,
            enabled: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidechain_ordered_knobs_clamp_repeated_dragging_without_invalid_settings() {
        fn frame(
            ctx: &Context,
            settings: &mut AudioSidechain,
            events: Vec<egui::Event>,
        ) -> Vec<egui::Response> {
            let mut controls = Vec::new();
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 400.0),
                    )),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ui| {
                    controls = daw_unit_ui::control_capture::scope(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            body(ui, Id::from_u128(10), settings, false);
                        });
                    })
                    .1;
                },
            );
            out.textures_delta.clear();
            controls
                .into_iter()
                .filter(|r| {
                    (r.rect.width() - knobs::CHOICE_SIZE).abs() < 0.1
                        && (r.rect.height() - knobs::CHOICE_SIZE).abs() < 0.1
                })
                .collect()
        }
        for index in [0, 1, 2, 3] {
            let ctx = Context::default();
            crate::fonts::configure(&ctx);
            crate::theme::configure(&ctx);
            let mut settings = AudioSidechain::new(Id::new_v4());
            settings.filter = true;
            let controls = frame(&ctx, &mut settings, vec![]);
            assert_eq!(controls.len(), 6);
            let origin = controls[index].rect.center();
            frame(
                &ctx,
                &mut settings,
                vec![
                    egui::Event::PointerMoved(origin),
                    egui::Event::PointerButton {
                        pos: origin,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            for step in 0..64 {
                let direction = if index == 0 || index == 2 { -1.0 } else { 1.0 };
                let pos = origin + egui::vec2(0.0, direction * (2000.0 + step as f32));
                frame(&ctx, &mut settings, vec![egui::Event::PointerMoved(pos)]);
                assert!(
                    settings.valid(),
                    "knob {index}, drag frame {step}: {settings:?}"
                );
            }
            let (actual, limit) = match index {
                0 => (settings.high_pass_hz, settings.low_pass_hz - 0.1),
                1 => (settings.low_pass_hz, settings.high_pass_hz + 0.1),
                2 => (settings.lower_db, settings.upper_db - 0.1),
                _ => (settings.upper_db, settings.lower_db + 0.1),
            };
            assert!(
                (actual - limit).abs() < 0.005,
                "knob {index}: {actual} != {limit}"
            );
        }
    }
    #[test]
    fn sidechain_dropdowns_fill_padded_columns_without_moving_knob_centers() {
        let ctx = Context::default();
        crate::fonts::configure(&ctx);
        crate::theme::configure(&ctx);
        let mut detector = SidechainDetector::Peak;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::Frame::new()
                    .inner_margin(daw_unit_ui::layout::PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(daw_unit_ui::layout::unit_columns_width(2) - 20.0);
                        let inner = ui.available_rect_before_wrap();
                        let mut rects = [egui::Rect::NOTHING; 2];
                        daw_unit_ui::layout::unit_columns(ui, 2, |ui, column| {
                            let center =
                                ui.available_rect_before_wrap().left() + knobs::CHOICE_SIZE * 0.5;
                            rects[column] = select_response(
                                ui,
                                "Detector",
                                &mut detector,
                                &[
                                    (SidechainDetector::Peak, "Peak"),
                                    (SidechainDetector::Rms, "RMS"),
                                ],
                            )
                            .rect;
                            assert!((rects[column].center().x - center).abs() < 0.1);
                        });
                        assert!((rects[0].left() - inner.left()).abs() < 0.1);
                        assert!((rects[1].right() - inner.right()).abs() < 0.1);
                        assert!(
                            (rects[0].width() - (daw_unit_ui::layout::UNIT_STRIP_WIDTH - 20.0))
                                .abs()
                                < 0.1
                        );
                        assert!(!rects[0].intersects(rects[1]));
                    });
            },
        );
        out.textures_delta.clear();
        assert_eq!(detector, SidechainDetector::Peak);
    }
    #[test]
    fn sidechain_settings_reuse_three_double_width_strips_and_do_not_edit_while_painting() {
        for compact in [false, true] {
            for input in [-9.7_f32, -39.2, -120.0, 240.0] {
                let ctx = Context::default();
                crate::fonts::configure(&ctx);
                crate::theme::configure(&ctx);
                ctx.data_mut(|d| {
                    d.insert_temp(
                        key(),
                        SourceContext {
                            readings: HashMap::from([(Id::from_u128(10), [input, 0.876])]),
                            ..Default::default()
                        },
                    )
                });
                let mut settings = AudioSidechain::new(Id::new_v4());
                let original = settings;
                let mut rect = egui::Rect::NOTHING;
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1200.0, 400.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        rect = ui
                            .horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                body(ui, Id::from_u128(10), &mut settings, compact);
                            })
                            .response
                            .rect;
                    },
                );
                out.textures_delta.clear();
                if !compact {
                    fn text_y(shape: &egui::Shape, text: &str) -> Option<f32> {
                        match shape {
                            egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos.y),
                            egui::Shape::Vec(v) => v.iter().find_map(|s| text_y(s, text)),
                            _ => None,
                        }
                    }
                    let footer_y = |text| {
                        out.shapes
                            .iter()
                            .rev()
                            .find_map(|s| text_y(&s.shape, text))
                            .expect("strip footer")
                    };
                    for label in ["Mapping", "Response"] {
                        assert!((footer_y(label) - footer_y("Detector")).abs() < 0.1);
                    }
                    fn find(
                        shape: &egui::Shape,
                        text: &str,
                    ) -> Option<std::sync::Arc<egui::Galley>> {
                        match shape {
                            egui::Shape::Text(t) if t.galley.text() == text => {
                                Some(t.galley.clone())
                            }
                            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
                            _ => None,
                        }
                    }
                    let text = format!("{input:.1} dBFS");
                    let galley = out
                        .shapes
                        .iter()
                        .find_map(|s| find(&s.shape, &text))
                        .expect("detector input readout");
                    assert_eq!(galley.rows.len(), 1, "{text}");
                    assert!(
                        galley.size().x <= knobs::CHOICE_SIZE,
                        "{text}: {:?}",
                        galley.size()
                    );
                }
                out.textures_delta.clear();
                assert_eq!(settings, original);
                assert!(
                    (rect.width() - 3.0 * daw_unit_ui::layout::unit_columns_width(2)).abs() < 0.1,
                    "{rect:?}"
                );
                assert!(
                    (rect.height() - (crate::envelope_panel::body_height(compact) + 20.0)).abs()
                        < 1.0,
                    "{rect:?}"
                );
            }
        }
    }
}
