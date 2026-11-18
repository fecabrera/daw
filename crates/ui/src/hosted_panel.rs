//! External parameters use the existing unit shell, knobs and destination strips.
use crate::{modulation_panel, panels};
use daw_core::{HostedSettings, ModulationParameter};
use egui::Ui;
#[derive(Clone, Default)]
struct ControlView {
    search: String,
    page: usize,
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut HostedSettings,
    compact: bool,
    height: f32,
    sources: &[modulation_panel::Source],
    readings: impl Fn(daw_core::Id) -> Option<daw_engine::ModulationSnapshot>,
) -> panels::UnitResponse<()> {
    let available = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(egui::Id::new(("external-available", settings.id))))
        .unwrap_or(true);
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let title = settings.plugin.name.clone();
    let targets: Vec<_> = settings
        .parameters
        .iter()
        .filter(|p| p.definition.accepts_modulation())
        .map(|p| {
            modulation_panel::Target::flat(
                ModulationParameter::HostedValue,
                Some(p.target),
                if p.definition.module.is_empty() {
                    p.definition.name.clone()
                } else {
                    format!("{} · {}", p.definition.module, p.definition.name)
                },
            )
            .with_amount_limit(p.amount_limit())
        })
        .collect();
    let view_key = egui::Id::new(("external-control-view", settings.id));
    let mut view = ui
        .ctx()
        .data(|d| d.get_temp::<ControlView>(view_key))
        .unwrap_or_default();
    let query = view.search.to_lowercase();
    let matches: Vec<_> = settings
        .parameters
        .iter()
        .filter(|p| {
            !p.definition.hidden
                && format!("{} {}", p.definition.module, p.definition.name)
                    .to_lowercase()
                    .contains(&query)
        })
        .map(|p| p.target)
        .collect();
    let pages = matches.len().div_ceil(12).max(1);
    view.page = view.page.min(pages - 1);
    let visible: std::collections::HashSet<_> = matches
        .iter()
        .skip(view.page * 12)
        .take(12)
        .copied()
        .collect();
    let markers: Vec<_> = settings
        .parameters
        .iter()
        .map(|p| {
            let (low, high) = p.definition.domain.bounds();
            let span = high - low;
            let routes: Vec<_> = settings
                .modulations
                .iter()
                .filter(|r| r.target == Some(p.target))
                .map(|r| {
                    let mut route = *r;
                    route.amount = if span > 0.0 {
                        (f64::from(route.amount) / span) as f32
                    } else {
                        0.0
                    };
                    route
                })
                .collect();
            let base = if span > 0.0 {
                ((p.base_value - low) / span) as f32
            } else {
                0.0
            };
            let live = readings(p.target);
            (
                p.target,
                modulation_panel::marker(
                    &routes,
                    sources,
                    ModulationParameter::HostedValue,
                    base,
                    settings.enabled,
                    None,
                    [live.map(|v| v[0]), live.map(|v| v[1])],
                ),
            )
        })
        .collect();
    let actions = [
        panels::UnitHeaderAction {
            icon: crate::icons::Lucide::Maximize2,
            name: "Open plugin editor",
            enabled: available,
        },
        panels::UnitHeaderAction {
            icon: crate::icons::Lucide::RefreshCcw,
            name: "Reload last captured plugin state",
            enabled: available,
        },
    ];
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane_with_actions(
        ui,
        (&title, compact),
        height,
        (&mut enabled, &mut collapsed),
        if compact { &[] } else { &actions },
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                panels::unit_strip(
                    ui,
                    crate::envelope_panel::body_height(compact),
                    egui::UiBuilder::new(),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = f32::from(crate::theme::TOOLBAR_PADDING);
                        if !compact {
                            control_slot(ui, |ui| {
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut view.search)
                                            .hint_text("Find parameter")
                                            .desired_width(ui.available_width()),
                                    )
                                    .on_hover_text("Find a mapped plugin parameter")
                                    .changed()
                                {
                                    view.page = 0;
                                }
                            });
                            control_slot(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    if ui
                                        .add_enabled(view.page > 0, egui::Button::new("‹"))
                                        .clicked()
                                    {
                                        view.page -= 1;
                                    }
                                    ui.label(format!("{}/{}", view.page + 1, pages));
                                    if ui
                                        .add_enabled(view.page + 1 < pages, egui::Button::new("›"))
                                        .clicked()
                                    {
                                        view.page += 1;
                                    }
                                });
                            });
                        }
                        if !compact {
                            control_slot(ui, |_| {});
                        }
                        footer(ui, "Plugin", compact).on_hover_text(if !available {
                            "Plugin unavailable. Its saved state is retained."
                        } else if settings.state.is_none() {
                            "State recall unavailable. This plugin opens at its defaults."
                        } else {
                            "Native editor, state reload and mapped parameter search"
                        });
                    },
                );
                pcm_inputs(ui, settings, compact);
                let page_start = view.page * 12;
                let unit = settings.id;
                let mut revision = settings.edit_revision;
                for (index, group) in settings
                    .parameters
                    .iter_mut()
                    .filter(|p| visible.contains(&p.target))
                    .collect::<Vec<_>>()
                    .chunks_mut(3)
                    .enumerate()
                {
                    panels::unit_strip(
                        ui,
                        crate::envelope_panel::body_height(compact),
                        egui::UiBuilder::new().id_salt(("parameters", index)),
                        |ui| {
                            ui.spacing_mut().item_spacing.y =
                                f32::from(crate::theme::TOOLBAR_PADDING);
                            if !compact {
                                for row in 0..3 {
                                    if let Some(p) = group.get_mut(row) {
                                        let marker = markers
                                            .iter()
                                            .find(|(target, _)| *target == p.target)
                                            .and_then(|(_, marker)| marker.as_ref());
                                        let response = parameter_control(
                                            ui,
                                            unit,
                                            p,
                                            available,
                                            marker,
                                            &mut settings.modulation_open,
                                        );
                                        if response.changed() {
                                            revision = revision.saturating_add(1);
                                            p.edit_revision = revision;
                                        }
                                    } else {
                                        control_slot(ui, |_| {});
                                    }
                                }
                            }
                            let first = page_start + index * 3 + 1;
                            footer(
                                ui,
                                &format!("{}–{}", first, first + group.len() - 1),
                                compact,
                            )
                            .on_hover_text(
                                group
                                    .iter()
                                    .map(|p| p.definition.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" · "),
                            );
                        },
                    );
                }
                settings.edit_revision = revision;
                if settings.parameters.iter().all(|p| p.definition.hidden) {
                    ui.label("No mapped parameters");
                }
                if settings.modulation_open {
                    modulation_panel::targeted_body(
                        ui,
                        &mut settings.modulations,
                        sources,
                        &targets,
                        daw_unit_core::ModulationScope::Shared,
                        compact,
                    );
                }
            });
        },
    );
    if ui.is_enabled() && ui.input(|input| input.focused) {
        for (index, response) in result.header_actions.iter().enumerate() {
            if response.clicked() {
                let key = egui::Id::new(if index == 0 {
                    "external-editor-requests"
                } else {
                    "external-reopen-requests"
                });
                ui.ctx().data_mut(|d| {
                    d.get_temp_mut_or_default::<Vec<daw_core::Id>>(key)
                        .push(settings.id)
                });
            }
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(view_key, view));
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    modulation_panel::toggle(ui, &result, collapsed, &mut settings.modulation_open);
    result
}

/// Use the same three 52-point control rows and semibold footer as built-in units.
fn control_slot(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), daw_unit_ui::knobs::CHOICE_SIZE),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_min_height(daw_unit_ui::knobs::CHOICE_SIZE);
            body(ui);
        },
    );
}
fn footer(ui: &mut Ui, label: &str, compact: bool) -> egui::Response {
    daw_unit_ui::rows::centered(
        ui,
        if compact {
            crate::envelope_panel::body_height(true)
        } else {
            crate::icons::COMPACT_BUTTON_SIZE
        },
        |ui| daw_unit_ui::rows::unit_label(ui, label),
    )
    .inner
}
fn parameter_control(
    ui: &mut Ui,
    unit: daw_core::Id,
    binding: &mut daw_unit_core::HostedParameterBinding,
    available: bool,
    marker: Option<&daw_unit_ui::knobs::ModulationMarker>,
    modulation_open: &mut bool,
) -> egui::Response {
    ui.push_id(binding.target, |ui| {
        let p = &binding.definition;
        let displayed = if p.read_only {
            ui.ctx()
                .data(|d| d.get_temp::<f64>(egui::Id::new(("external-telemetry", unit, p.id))))
                .unwrap_or(binding.base_value)
        } else {
            binding.base_value
        };
        let (lo, hi) = p.domain.bounds();
        let mut position = if hi > lo {
            ((displayed - lo) / (hi - lo)) as f32
        } else {
            0.0
        };
        let reset = if hi > lo {
            ((p.default_value - lo) / (hi - lo)) as f32
        } else {
            0.0
        };
        let step = match p.domain {
            daw_unit_core::HostedParameterDomain::Normalized { step_count } if step_count > 0 => {
                Some(1.0 / step_count as f32)
            }
            daw_unit_core::HostedParameterDomain::Plain { stepped: true, .. } if hi > lo => {
                Some((1.0 / (hi - lo)) as f32)
            }
            _ => None,
        };
        let mut spec =
            daw_unit_ui::knobs::KnobSpec::new("Value", 0.0..=1.0, reset, step.unwrap_or(0.01));
        if let Some(step) = step {
            spec = spec.stepped(step);
        }
        let value_text = crate::hosting::parameter_text(ui.ctx(), unit, p.id, displayed);
        let response = daw_unit_ui::rows::selector_row(ui, &p.name, |ui| {
            ui.add_enabled_ui(available && !p.read_only && hi > lo, |ui| {
                ui.add(
                    daw_unit_ui::knobs::Knob::new(&mut position, spec)
                        .tooltip_name(&p.name)
                        .value_text(&value_text)
                        .modulated(marker, modulation_open),
                )
            })
            .inner
        });
        response.widget_info(|| {
            egui::WidgetInfo::slider(
                ui.is_enabled() && available && !p.read_only,
                displayed,
                &p.name,
            )
        });
        if response.changed()
            && let Some(value) = p.domain.constrain(lo + f64::from(position) * (hi - lo))
        {
            binding.base_value = value;
        }
        response
    })
    .inner
}

fn pcm_inputs(ui: &mut Ui, settings: &mut HostedSettings, compact: bool) {
    let metadata = ui.ctx().data(|d| {
        d.get_temp::<daw_plugin_host::NativeMetadata>(egui::Id::new((
            "external-ports",
            settings.id,
        )))
    });
    let Some(metadata) = metadata else {
        return;
    };
    let context = crate::audio_sidechain::context(ui.ctx());
    let signature = metadata.audio_layout_signature();
    for (index, port) in metadata
        .inputs
        .iter()
        .enumerate()
        .filter(|(_, p)| settings.plugin.role == daw_core::HostedRole::Instrument || !p.main)
    {
        let key = match settings.plugin.identity.format() {
            daw_core::PluginFormat::Clap => daw_core::HostedInputId::Clap { id: port.id },
            daw_core::PluginFormat::Vst3 => daw_core::HostedInputId::Vst3 {
                index: index as u32,
                layout: signature.clone(),
            },
        };
        let existing = settings.audio_inputs.iter().position(|r| r.port == key);
        let mut source = existing.map(|i| settings.audio_inputs[i].source_track);
        let label = source.map_or("None", |id| {
            context
                .choices
                .iter()
                .find(|s| s.id == id)
                .map_or("Unavailable", |s| s.name.as_str())
        });
        panels::unit_strip(
            ui,
            crate::envelope_panel::body_height(compact),
            egui::UiBuilder::new().id_salt(("pcm", index)),
            |ui| {
                ui.spacing_mut().item_spacing.y = f32::from(crate::theme::TOOLBAR_PADDING);
                if !compact {
                    daw_unit_ui::rows::selector_row(ui, &port.name, |ui| {
                        crate::menus::Dropdown::from_id_salt("input-source")
                            .selected_text(label)
                            .width(ui.available_width())
                            .truncate()
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(source.is_none(), "None").clicked() {
                                    source = None;
                                    ui.close();
                                }
                                for choice in &context.choices {
                                    if ui
                                        .add_enabled(
                                            choice.available,
                                            egui::Button::selectable(
                                                source == Some(choice.id),
                                                &choice.name,
                                            ),
                                        )
                                        .clicked()
                                    {
                                        source = Some(choice.id);
                                        ui.close();
                                    }
                                }
                            })
                            .response
                    });
                    if source != existing.map(|i| settings.audio_inputs[i].source_track) {
                        if let Some(index) = existing {
                            settings.audio_inputs.remove(index);
                        }
                        if let Some(source_track) = source {
                            settings.audio_inputs.push(daw_core::HostedAudioInput {
                                id: daw_core::Id::new_v4(),
                                source_track,
                                port: key.clone(),
                                tap: daw_unit_core::SidechainTap::PreFader,
                                channels: Default::default(),
                                gain_db: 0.0,
                                enabled: true,
                            });
                        }
                    }
                    if let Some(route) = settings.audio_inputs.iter_mut().find(|r| r.port == key) {
                        control_slot(ui, |ui| {
                            crate::menus::Dropdown::from_id_salt("input-tap")
                                .selected_text(match route.tap {
                                    daw_unit_core::SidechainTap::PreFader => "Pre-fader",
                                    _ => "Post-fader",
                                })
                                .width(ui.available_width())
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for (value, label) in [
                                        (daw_unit_core::SidechainTap::PreFader, "Pre-fader"),
                                        (daw_unit_core::SidechainTap::PostFader, "Post-fader"),
                                    ] {
                                        if ui.selectable_label(route.tap == value, label).clicked()
                                        {
                                            route.tap = value;
                                            ui.close();
                                        }
                                    }
                                });
                            let channel_name = |channel| match channel {
                                daw_core::HostedInputChannels::Stereo => "Stereo",
                                daw_core::HostedInputChannels::Left => "Left",
                                daw_core::HostedInputChannels::Right => "Right",
                                daw_core::HostedInputChannels::Mid => "Mid",
                                daw_core::HostedInputChannels::Side => "Side",
                            };
                            crate::menus::Dropdown::from_id_salt("input-channels")
                                .selected_text(channel_name(route.channels))
                                .width(ui.available_width())
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for channel in [
                                        daw_core::HostedInputChannels::Stereo,
                                        daw_core::HostedInputChannels::Left,
                                        daw_core::HostedInputChannels::Right,
                                        daw_core::HostedInputChannels::Mid,
                                        daw_core::HostedInputChannels::Side,
                                    ] {
                                        if ui
                                            .selectable_label(
                                                route.channels == channel,
                                                channel_name(channel),
                                            )
                                            .clicked()
                                        {
                                            route.channels = channel;
                                            ui.close();
                                        }
                                    }
                                });
                        });
                        let spec = daw_unit_ui::knobs::KnobSpec::new(
                            "Input gain",
                            -120.0..=24.0,
                            0.0,
                            0.1,
                        )
                        .value_label(|v| format!("{v:.1} dB"));
                        daw_unit_ui::knobs::row(
                            ui,
                            &mut route.gain_db,
                            daw_unit_ui::knobs::CHOICE_SIZE,
                            |ui, value| ui.add(daw_unit_ui::knobs::Knob::new(value, spec)),
                        );
                    } else {
                        control_slot(ui, |_| {});
                        control_slot(ui, |_| {});
                    }
                }
                if let Some(route) = settings.audio_inputs.iter_mut().find(|r| r.port == key) {
                    daw_unit_ui::rows::centered(
                        ui,
                        if compact {
                            crate::envelope_panel::body_height(true)
                        } else {
                            crate::icons::COMPACT_BUTTON_SIZE
                        },
                        |ui| {
                            if daw_unit_ui::rows::power_label(
                                ui,
                                "Audio in",
                                &port.name,
                                route.enabled,
                            )
                            .clicked()
                            {
                                route.enabled = !route.enabled;
                            }
                        },
                    );
                } else {
                    footer(ui, "Audio in", compact).on_hover_text(&port.name);
                }
            },
        );
    }
}

pub(super) fn missing(ui: &mut Ui, compact: bool, height: f32) -> panels::UnitResponse<()> {
    panels::PanelStyle::default().unit_in_lane(
        ui,
        ("Unavailable source", compact),
        height,
        &mut false,
        &mut false,
        |ui| {
            ui.label("This source uses a newer format. Its saved data is retained.");
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        HostedParameter, HostedParameterBinding, HostedParameterDomain, HostedPluginDescriptor,
        HostedRole, PluginIdentity,
    };

    #[test]
    fn mapped_controls_use_shared_rows_strip_width_and_footer_in_both_themes() {
        for base in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            crate::theme::configure(&ctx);
            ctx.set_theme(base);
            let mut settings = HostedSettings::new(
                HostedPluginDescriptor {
                    identity: PluginIdentity::Clap("org.daw.panel-fixture".into()),
                    name: "Fixture".into(),
                    vendor: "DAW".into(),
                    version: "1".into(),
                    role: HostedRole::Effect,
                },
                (0..6)
                    .map(|id| {
                        HostedParameterBinding::new(
                            HostedParameter {
                                id,
                                name: format!("Parameter {id}"),
                                module: String::new(),
                                domain: HostedParameterDomain::Normalized { step_count: 0 },
                                default_value: 0.5,
                                automatable: true,
                                modulatable: false,
                                read_only: false,
                                hidden: false,
                            },
                            0.5,
                        )
                    })
                    .collect(),
            );
            let mut controls = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let (_, responses) = daw_unit_ui::control_capture::scope(ui, |ui| {
                        show(ui, &mut settings, false, 240.0, &[], |_| None);
                    });
                    controls = responses;
                },
            );
            output.textures_delta.clear();
            assert_eq!(controls.len(), 6);
            let row_step =
                daw_unit_ui::knobs::CHOICE_SIZE + f32::from(crate::theme::TOOLBAR_PADDING);
            for group in controls.chunks_exact(3) {
                for pair in group.windows(2) {
                    assert!(
                        (pair[1].rect.center().y - pair[0].rect.center().y - row_step).abs() < 0.1
                    );
                    assert_eq!(pair[0].rect.center().x, pair[1].rect.center().x);
                }
            }
            assert!(
                (controls[3].rect.center().x
                    - controls[0].rect.center().x
                    - daw_unit_ui::layout::UNIT_STRIP_WIDTH)
                    .abs()
                    < 0.1
            );
            assert_eq!(settings.edit_revision, 0);
        }
    }
    #[test]
    fn header_editor_and_reload_queue_actions_without_toggling_or_collapsing() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::theme::configure(&ctx);
        let mut settings = HostedSettings::new(
            HostedPluginDescriptor {
                identity: PluginIdentity::Clap("org.daw.header-fixture".into()),
                name: "Fixture".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: HostedRole::Effect,
            },
            Vec::new(),
        );
        let id = settings.id;
        let mut frame = |events: Vec<egui::Event>| {
            let mut bounds = Vec::new();
            let mut header_top = 0.0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: true,
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let result = show(ui, &mut settings, false, 240.0, &[], |_| None);
                    header_top = result.header.rect.top();
                    assert_eq!(result.header_actions.len(), 2);
                    bounds.push(result.power.rect);
                    bounds.extend(result.header_actions.iter().map(|r| r.rect));
                    for rect in &bounds {
                        assert!(result.header.rect.contains_rect(*rect));
                        assert_eq!(rect.size(), egui::vec2(panels::HEADER_HEIGHT - 2.0, 17.0));
                    }
                    for pair in bounds.windows(2) {
                        assert_eq!(pair[0].center().x, pair[1].center().x);
                        assert!(pair[0].bottom() <= pair[1].top());
                    }
                },
            );
            output.textures_delta.clear();
            let dots: Vec<_> = bounds
                .iter()
                .map(|bounds| {
                    output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Circle(dot)
                                if dot.center == bounds.center() && dot.stroke.width > 0.0 =>
                            {
                                let radius = dot.radius + dot.stroke.width * 0.5;
                                Some((dot.center.y - radius, dot.center.y + radius))
                            }
                            _ => None,
                        })
                        .expect("painted header dot")
                })
                .collect();
            assert_eq!(dots[0].0 - header_top, 7.0);
            for pair in dots.windows(2) {
                assert_eq!(pair[1].0 - pair[0].1, 7.0);
            }
            (bounds, settings.enabled, settings.collapsed)
        };
        let (bounds, _, _) = frame(Vec::new());
        for index in [1, 2, 2] {
            let pos = bounds[index].center();
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            let (_, enabled, collapsed) = frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(enabled);
            assert!(!collapsed);
        }
        assert_eq!(ctx.data(|d| d.get_temp::<Vec<daw_core::Id>>(egui::Id::new("external-editor-requests"))).unwrap(), vec![id]);
        assert_eq!(ctx.data(|d| d.get_temp::<Vec<daw_core::Id>>(egui::Id::new("external-reopen-requests"))).unwrap(), vec![id, id]);
    }
}
