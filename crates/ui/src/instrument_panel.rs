use super::*;
use crate::envelope_panel;
use daw_core::{InstrumentLayerSettings, TrackKind};
use midi_editor::OSCILLATOR_STRIP_WIDTH;

const LAYER_HEADER_WIDTH: f32 = TRACK_WIDTH;

#[derive(Default)]
pub(super) struct InstrumentUi {
    pub(super) track: Option<Id>,
    pub(super) selected_layer: Option<Id>,
    pub(super) selected_components: HashSet<(Id, daw_core::InstrumentComponent)>,
    pub(super) component_refocus: bool,
    pub(super) component_focus: crate::selection_focus::SelectionFocus,
    pub(super) component_viewports: Vec<(Id, Rect)>,
    name_edit: Option<LayerNameEdit>,
    pub(super) macro_name_edit: Option<crate::macro_panel::NameEdit>,
    drag: Option<Id>,
    pub(super) inputs: HashMap<Id, Inputs>,
    pub(super) oscillator_slots: Vec<(Id, Id, Rect)>,
    pub(super) component_slots: Vec<(Id, daw_core::InstrumentComponent, Rect)>,
    #[cfg(test)]
    pub(super) main_bounds: crate::instrument_main::MainBounds,
    #[cfg(test)]
    pub(super) component_bounds: Vec<(Id, daw_core::InstrumentComponent, Rect)>,
}
struct LayerNameEdit {
    layer: Id,
    text: String,
    focus: bool,
}
impl InstrumentUi {
    pub(super) fn cancel(&mut self) {
        self.selected_components.clear();
        self.component_refocus = false;
        self.name_edit = None;
        self.macro_name_edit = None;
        self.drag = None;
        self.inputs.clear();
    }
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub(super) fn editing_name(&self) -> bool {
        self.name_edit.is_some() || self.macro_name_edit.is_some()
    }
}

#[cfg(test)]
pub(crate) struct LayerUiBounds {
    pub row: Rect,
    pub label: Rect,
    pub grip: Rect,
    pub name: Rect,
    pub mix_controls: [Rect; 6],
    pub collapse: Rect,
    pub oscillator_panel: Rect,
    pub oscillator_header: Rect,
    pub oscillator_power: Rect,
    pub envelope_panel: Rect,
    pub envelope_header: Rect,
    pub envelope_power: Rect,
    pub oscillators: Vec<Rect>,
    pub source_powers: Vec<Rect>,
    pub envelope_stages: [Rect; 4],
    pub amplitude_panel: Rect,
    pub amplitude_header: Rect,

    pub filter_envelope_panel: Rect,
    pub filter_envelope_header: Rect,
    pub filter_envelope_power: Rect,
    pub filter_envelope_stages: [Rect; 4],
}

impl DawUi {
    pub(super) fn instrument_view(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        self.activate_component_drag(ui);
        self.instrument_ui.oscillator_slots.clear();
        self.instrument_ui.component_slots.clear();
        self.instrument_ui.component_viewports.clear();
        #[cfg(test)]
        self.instrument_ui.component_bounds.clear();
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| Some(t.id) == self.selected_track)
            .cloned()
        else {
            ui.label("Select a MIDI track to configure its instrument.");
            return;
        };
        if track.kind != TrackKind::Midi {
            ui.label("Select a MIDI track to configure its instrument.");
            return;
        }
        let Some(mut settings) = track.instrument.clone() else {
            ui.label("No instrument assigned");
            return;
        };
        #[cfg(test)]
        {
            self.midi_editor.layer_bounds.clear();
        }
        if self.instrument_ui.track != Some(track.id) {
            self.instrument_ui.cancel();
            self.instrument_ui.track = Some(track.id);
            self.instrument_ui.selected_layer = None;
            self.instrument_catalog.clear();
        }
        if self.instrument_ui.selected_layer != Some(settings.main.id)
            && !settings
                .layers
                .iter()
                .any(|layer| Some(layer.id) == self.instrument_ui.selected_layer)
        {
            self.instrument_ui.selected_layer = settings.layers.first().map(|layer| layer.id);
        }
        if self
            .instrument_ui
            .name_edit
            .as_ref()
            .is_some_and(|edit| !settings.layers.iter().any(|layer| layer.id == edit.layer))
        {
            self.instrument_ui.name_edit = None;
        }
        self.instrument_ui.inputs.retain(|id, _| {
            *id == settings.main.id || settings.layers.iter().any(|layer| layer.id == *id)
        });
        if self
            .instrument_ui
            .macro_name_edit
            .as_ref()
            .is_some_and(|edit| !edit.available(&settings))
        {
            self.instrument_ui.macro_name_edit = None;
        }
        let modulation_sources = crate::modulation_panel::sources(Some(&settings));
        let provider_sources = crate::modulation_panel::provider_sources(&settings);
        let shared_sources: Vec<_> = track
            .effects
            .iter()
            .flat_map(|e| e.unit.modulations())
            .chain(
                settings
                    .provider_destinations()
                    .flat_map(|(_, routes)| routes),
            )
            .filter_map(|r| r.source)
            .collect();
        let mut remove_layer = None;
        let column_bounds = ui.max_rect();
        let control_panel = panels::ControlPanel::begin(ui, column_bounds);
        ui.spacing_mut().item_spacing.y = -1.0;
        let header = control_panel.header(ui, "Layers", |ui| {
            let add = icons::button(
                ui,
                icons::Lucide::Plus,
                "Add layer",
                self.component_drag.is_none(),
                theme::palette(ui.ctx()).text,
            );
            #[cfg(test)]
            {
                self.midi_editor.add_layer_bounds = add.rect;
            }
            if add.clicked() {
                let mut layer = InstrumentLayerSettings::default();
                let mut number = 1;
                loop {
                    layer.name = format!("Layer {number}");
                    if !settings
                        .layers
                        .iter()
                        .any(|existing| existing.name == layer.name)
                    {
                        break;
                    }
                    number += 1;
                }
                self.instrument_ui.selected_layer = Some(layer.id);
                settings.layers.push(layer);
            }
            add.rect.width()
        });
        #[cfg(test)]
        {
            self.midi_editor.instrument_header_bounds = header.response.rect;
        }
        let layer_action_width = header.inner;
        // Reserve Main outside the vertical scrolling region.
        let main_label_height = panels::unit_height(settings.main.collapsed);
        let main_height = main_label_height.min(ui.available_height());
        let main_rect = Rect::from_min_max(
            Pos2::new(ui.max_rect().left(), ui.max_rect().bottom() - main_height),
            ui.max_rect().max,
        );
        let count = settings.layers.len();
        let mut layer_rows = Vec::new();
        let scroll = egui::ScrollArea::vertical()
            .max_height((ui.available_height() - main_height).max(0.0))
            .auto_shrink([false, false])
            .id_salt(("instrument-layers", track.id))
            .show(ui, |ui| {
                egui::Frame::new().show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    // Adjacent rows share one border pixel.
                    ui.spacing_mut().item_spacing.y = -1.0;
                    if self.instrument_ui.dragging()
                        || self.component_drag.as_ref().is_some_and(|drag| drag.active)
                    {
                        self.scroll_layer_drag(ui);
                    }
                    for layer in &mut settings.layers {
                        ui.push_id((track.id, layer.id), |ui| {
                            // Use one presentation snapshot for the entire frame.
                            let compact = layer.collapsed;
                            let height = panels::unit_height(compact);
                            let row = ui
                                .add_enabled_ui(self.component_drag.as_ref().is_none_or(|d| !d.active), |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x =
                                            if layer.oscillators.is_empty()
                                                && !layer.envelopes.iter().any(|e| e.present)
                                                && !layer.amplitude.present
                                            {
                                                0.0
                                            } else {
                                                -1.0
                                            };
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        let (label, response) = ui.allocate_exact_size(
                                            Vec2::new(LAYER_HEADER_WIDTH, height),
                                            Sense::click_and_drag(),
                                        );
                                        ui.painter().rect_filled(
                                            label,
                                            0.0,
                                            theme::palette(ui.ctx()).panel,
                                        );
                                        let mut header_ui = ui.new_child(
                                            egui::UiBuilder::new()
                                                .max_rect(Rect::from_min_size(
                                                    label.min + Vec2::splat(7.0),
                                                    Vec2::new(
                                                        label.width() - 14.0,
                                                        toolbars::CONTROL_HEIGHT
                                                            + 4.0
                                                            + knobs::SIZE,
                                                    ),
                                                ))
                                                .layout(egui::Layout::top_down(egui::Align::Min)),
                                        );
                                        header_ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                                        let mixing_enabled = !self.instrument_ui.dragging()
                                            && !self.instrument_ui.editing_name();
                                        let header = rows::centered(
                                            &mut header_ui,
                                            toolbars::CONTROL_HEIGHT,
                                            |ui| {
                                                let buttons_width = 2.0 * toolbars::CONTROL_HEIGHT
                                                    + layer_action_width
                                                    + 2.0 * ui.spacing().item_spacing.x;
                                                let name_width = (ui.available_width()
                                                    - toolbars::CONTROL_HEIGHT
                                                    - buttons_width
                                                    - 2.0 * ui.spacing().item_spacing.x)
                                                    .max(1.0);
                                                let grip = icons::reorder_grip(ui, "layer");
                                                self.start_layer_drag(ui, &grip, layer.id);
                                                let name = ui
                                                    .allocate_ui_with_layout(
                                                        Vec2::new(
                                                            name_width,
                                                            toolbars::CONTROL_HEIGHT,
                                                        ),
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            // Short names must still reserve the full name column.
                                                            ui.set_min_width(name_width);
                                                            self.layer_name(ui, layer)
                                                        },
                                                    )
                                                    .inner;
                                                let toggles = ui
                                                    .add_enabled_ui(mixing_enabled, |ui| {
                                                        mix_controls::mute_solo(
                                                            ui,
                                                            &mut layer.muted,
                                                            &mut layer.soloed,
                                                            [
                                                                "Mute layer",
                                                                "Solo layer within this instrument",
                                                            ],
                                                        )
                                                    })
                                                    .inner;
                                                let collapse = icons::small_button(
                                                    ui,
                                                    if compact {
                                                        icons::Lucide::ArrowDownFromLine
                                                    } else {
                                                        icons::Lucide::ArrowUpFromLine
                                                    },
                                                    if compact {
                                                        "Expand layer"
                                                    } else {
                                                        "Collapse layer"
                                                    },
                                                    mixing_enabled
                                                        && ui.input(|input| input.focused),
                                                    theme::palette(ui.ctx()).text,
                                                );
                                                (
                                                    grip.rect,
                                                    name,
                                                    toggles.map(|response| response.rect),
                                                    collapse,
                                                )
                                            },
                                        )
                                        .inner;
                                        let collapse = header.3;
                                        if collapse.clicked() {
                                            layer.collapsed = !compact;
                                            ui.ctx().request_repaint();
                                        }
                                        let inputs = self
                                            .instrument_ui
                                            .inputs
                                            .entry(layer.id)
                                            .or_insert_with(|| Inputs {
                                                gain: faders::format_gain(layer.gain_db),
                                                pan: format!("{:.2}", layer.pan),
                                            });
                                        let mixing = header_ui
                                            .add_enabled_ui(mixing_enabled, |ui| {
                                                mix_controls::gain_pan(
                                                    ui,
                                                    &mut layer.gain_db,
                                                    &mut layer.pan,
                                                    inputs,
                                                )
                                            })
                                            .inner;
                                        if let Some(error) = mixing.error {
                                            self.fail(error);
                                        }
                                        let controls = (header.0, header.1);
                                        let mixing_bounds = [
                                            header.2[0],
                                            header.2[1],
                                            mixing.bounds[0],
                                            mixing.bounds[1],
                                            mixing.bounds[2],
                                            mixing.bounds[3],
                                        ];
                                        // Click-only buttons can pass a drag to the background. Exclude
                                        // controls at the press origin, even when the pointer leaves them.
                                        let control_press = ui.input(|input| {
                                            input.pointer.press_origin().is_some_and(|point| {
                                                collapse.rect.contains(point)
                                                    || mixing_bounds
                                                        .iter()
                                                        .any(|rect| rect.contains(point))
                                            })
                                        });
                                        if !control_press {
                                            self.start_layer_drag(ui, &response, layer.id);
                                        }
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::WidgetType::Button,
                                                ui.is_enabled(),
                                                self.instrument_ui.selected_layer == Some(layer.id),
                                                format!("Select layer {}", layer.name),
                                            )
                                        });
                                        let response =
                                            response.on_hover_cursor(egui::CursorIcon::Grab);
                                        response.context_menu(|ui| {
                                            if ui
                                                .add_enabled(
                                                    count > 1 && !self.instrument_ui.dragging(),
                                                    egui::Button::new("Remove layer"),
                                                )
                                                .clicked()
                                            {
                                                remove_layer = Some(layer.id);
                                                ui.close();
                                            }
                                        });
                                        #[cfg(test)]
                                        let mut amplitude_bounds = (Rect::NOTHING, Rect::NOTHING);
                                        let mut oscillator_panel = None;
                                        let mut oscillator_header = Rect::NOTHING;
                                        let mut oscillator_power = Rect::NOTHING;
                                        let mut envelope_header = Rect::NOTHING;
                                        let mut envelope_power = Rect::NOTHING;
                                        let mut envelope_stages = [Rect::NOTHING; 4];
                                        let mut envelope_rect = Rect::NOTHING;
                                        #[cfg(test)]
                                        let mut filter_envelope = (
                                            Rect::NOTHING,
                                            Rect::NOTHING,
                                            Rect::NOTHING,
                                            [Rect::NOTHING; 4],
                                        );
                                        let components = egui::ScrollArea::horizontal()
                                            .id_salt("layer-units")
                                            .auto_shrink([false, true])
                                            .show(ui, |ui| {
                                                if self.component_drag.as_ref().is_some_and(|d|d.active) { drag::scroll_drag(ui, drag::Axis::Horizontal, 360.0, self.edit_environment_available(), Rect::from_x_y_ranges(ui.clip_rect().x_range(), label.y_range())); }
                                                crate::strip_reorder::scroll(ui);
                                                ui.horizontal_top(|ui| {
                                                        ui.spacing_mut().item_spacing.x = -1.0;
                                                        if layer.oscillators.is_empty() {
                                                            #[cfg(test)] {
                                                                self.midi_editor.oscillator_bounds.clear();
                                                                self.midi_editor.oscillator_strip_bounds.clear();
                                                                self.midi_editor.oscillator_power_bounds.clear();
                                                                self.midi_editor.oscillator_panel_bounds = Rect::NOTHING;
                                                            }
                                                        }
                                                        let mut previous: Option<Rect> = None;
                                                        for component in layer.visible_components() {
                                                            let rect = ui.push_id(component, |ui| match component {
                                                                daw_core::InstrumentComponent::Oscillators => {
                                                                    let (rect, header, power) = self.oscillator_unit(ui, track.id, layer, height, compact, &modulation_sources);
                                                                    oscillator_panel = Some(rect);
                                                                    oscillator_header = header;
                                                                    oscillator_power = power;
                                                                    Some(rect)
                                                                }
                                                                daw_core::InstrumentComponent::Envelope(id) => {
                                                                    let index = layer.envelopes.iter().position(|e| e.id == id).unwrap();
                                                                    let instance = &mut layer.envelopes[index];
                                                                    let shared_only = shared_sources.contains(&daw_core::ModulationSourceId { layer: layer.id, unit: id });
                                                                    envelope_panel::unit(ui, instance, index + 1, shared_only, height, compact).map(|unit| {
                                                                        self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track.id, layer.id, component);
                                                                        let rect = unit.response.rect;
                                                                        if index == 0 {
                                                                            envelope_rect = rect;
                                                                            envelope_header = unit.header.rect;
                                                                            envelope_power = unit.power.rect;
                                                                            envelope_stages = unit.inner.unwrap_or([Rect::NOTHING; 4]);
                                                                        }
                                                                        #[cfg(test)] if index == 1 {
                                                                            filter_envelope = (rect, unit.header.rect, unit.power.rect, unit.inner.unwrap_or([Rect::NOTHING; 4]));
                                                                        }
                                                                        rect
                                                                    })
                                                                }
                                                                daw_core::InstrumentComponent::Lfo(id) => {
                                                                    let index = layer.lfos.iter().position(|l| l.id == id).unwrap();
                                                                    crate::lfo_panel::unit(ui, &mut layer.lfos[index], index + 1, height, compact, crate::lfo_panel::Controls {
                                                                        sources: provider_sources.iter().find(|(source, _)| source.layer == layer.id && source.unit == id).map_or(&[], |(_, sources)| sources.as_slice()),
                                                                        tempo: self.session.project.tempo_bpm,
                                                                        live: self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Lfo { track: track.id, layer: layer.id, unit: id })).map(|v| [v[0], v[1]]),
                                                                    }).map(|unit| {
                                                                        self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track.id, layer.id, component);
                                                                        unit.response.rect
                                                                    })
                                                                }
                                                                daw_core::InstrumentComponent::Macros => {
                                                                    let live: Vec<_> = layer.macro_module.controls.iter().filter_map(|c|
                                                                        self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Macro { track: track.id, layer: layer.id, unit: c.id })).map(|v| (c.id, v[0]))).collect();
                                                                    crate::macro_panel::unit(ui, &mut layer.macro_module, height, compact, crate::macro_panel::Controls {
                                                                        layer: layer.id, sources: &modulation_sources, available: &provider_sources, live: &live,
                                                                        name_edit: &mut self.instrument_ui.macro_name_edit,
                                                                    }).map(|unit| {
                                                                        self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track.id, layer.id, component);
                                                                        unit.response.rect
                                                                    })
                                                                }
                                                                daw_core::InstrumentComponent::Amplitude => {
                                                                    crate::amplitude_panel::unit(ui, &mut layer.amplitude, &modulation_sources, height, compact,
                                                                        self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Amplitude { track: track.id, layer: layer.id })).map(|v| v[0]))
                                                                        .map(|unit| {
                                                                            self.start_installed_component_drag(ui, &unit.header, &[unit.power.rect], track.id, layer.id, component);
                                                                            #[cfg(test)] { amplitude_bounds = (unit.response.rect, unit.header.rect); }
                                                                            unit.response.rect
                                                                        })
                                                                }
                                                                daw_core::InstrumentComponent::Oscillator(_) => None,
                                                            }).inner;
                                                            if let Some(rect) = rect {
                                                                self.instrument_ui.component_slots.push((layer.id, component, rect));
                                                                if let Some(previous) = previous {
                                                                    if matches!(component, daw_core::InstrumentComponent::Envelope(_)) {
                                                                        ui.painter().vline(previous.right() - 0.5, previous.top()..=rect.bottom(), Stroke::new(1.0, theme::palette(ui.ctx()).border));
                                                                    } else {
                                                                        panels::unit_divider(ui, previous.right() - 0.5, previous.top()..=rect.bottom());
                                                                    }
                                                                }
                                                                previous = Some(rect);
                                                            }
                                                        }
                                                    });
                                            });
                                        self.instrument_ui.component_viewports.push((layer.id, Rect::from_min_size(components.inner_rect.min, Vec2::new(components.inner_rect.width(), height)).intersect(components.inner_rect).intersect(ui.clip_rect())));
                                        #[cfg(test)]
                                        {
                                            self.midi_editor.envelope_panel_bounds = envelope_rect;
                                            self.midi_editor.envelope_stage_bounds =
                                                envelope_stages;
                                            if oscillator_panel.is_none() {
                                                self.midi_editor.add_oscillator_bounds =
                                                    Rect::NOTHING;
                                            }
                                            self.midi_editor.layer_bounds.insert(
                                                layer.id,
                                                LayerUiBounds {
                                                    row: label,
                                                    label,
                                                    grip: controls.0,
                                                    name: controls.1,
                                                    mix_controls: mixing_bounds,
                                                    collapse: collapse.rect,
                                                    oscillator_panel: oscillator_panel
                                                        .unwrap_or(Rect::NOTHING),
                                                    oscillator_header,
                                                    oscillator_power,
                                                    envelope_panel: envelope_rect,
                                                    envelope_header,
                                                    envelope_power,
                                                    oscillators: self
                                                        .midi_editor
                                                        .oscillator_bounds
                                                        .clone(),
                                                    source_powers: self
                                                        .midi_editor
                                                        .oscillator_power_bounds
                                                        .clone(),
                                                    envelope_stages,
                                                    amplitude_panel: amplitude_bounds.0,
                                                    amplitude_header: amplitude_bounds.1,
                                                    filter_envelope_panel: filter_envelope.0,
                                                    filter_envelope_header: filter_envelope.1,
                                                    filter_envelope_power: filter_envelope.2,
                                                    filter_envelope_stages: filter_envelope.3,
                                                },
                                            );
                                        }
                                        #[cfg(not(test))]
                                        {
                                            let _ = (
                                                envelope_stages,
                                                controls,
                                                mixing_bounds,
                                                oscillator_header,
                                                oscillator_power,
                                                envelope_header,
                                                envelope_power,
                                            );
                                        }
                                    })
                                })
                                .inner;
                            if ui.is_enabled()
                                && self.component_drag.is_none()
                                && self.edit_environment_available()
                                && ui.input(|input| {
                                    input.focused
                                        && input
                                            .pointer
                                            .button_pressed(egui::PointerButton::Primary)
                                        && input.pointer.press_origin().is_some_and(|point| {
                                            row.response
                                                .rect
                                                .intersect(ui.clip_rect())
                                                .contains(point)
                                        })
                                })
                            {
                                self.instrument_ui.selected_layer = Some(layer.id);
                                ui.ctx().request_repaint();
                            }
                            layer_rows.push((layer.id, row.response.rect));
                            panels::ControlPanel::row_dividers(ui, row.response.rect, true, true);
                            #[cfg(test)]
                            {
                                self.midi_editor
                                    .layer_bounds
                                    .get_mut(&layer.id)
                                    .unwrap()
                                    .row = row.response.rect;
                            }
                        });
                    }
                });
            });
        let reorder_rows = layer_rows.clone();
        for (_, rect) in &mut layer_rows {
            *rect = rect.intersect(scroll.inner_rect);
        }
        self.instrument_main(
            ui,
            track.id,
            &mut settings.main,
            main_rect,
            &modulation_sources,
            &provider_sources,
        );
        layer_rows.push((settings.main.id, main_rect));
        control_panel.finish(ui);
        // Fixed chrome must paint after scrolling rows, which share its boundary pixel.
        ui.painter().hline(
            ui.max_rect().x_range(),
            header.response.rect.bottom() - 0.5,
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
        // Keep the whole selection stroke above row fills and shared dividers.
        if let Some(selected) = self.instrument_ui.selected_layer {
            if selected == settings.main.id {
                panels::ControlPanel::selected_row(
                    ui,
                    Rect::from_min_size(
                        main_rect.min,
                        Vec2::new(LAYER_HEADER_WIDTH, main_label_height),
                    ),
                    main_rect,
                );
            } else if let Some((_, row)) = reorder_rows.iter().find(|(id, _)| *id == selected) {
                panels::ControlPanel::selected_row(
                    ui,
                    Rect::from_min_max(
                        row.min,
                        Pos2::new(row.left() + LAYER_HEADER_WIDTH, row.bottom()),
                    ),
                    scroll.inner_rect,
                );
            }
        }
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.layer_viewport = scroll.inner_rect;
            self.instrument_catalog_bounds.layer_scroll = scroll.state.offset.y;
        }
        self.component_selection(ui, track.id);
        if let Some((edit, selected)) = self.finish_component_drag(
            ui,
            track.id,
            &mut settings,
            &layer_rows,
            Rect::from_min_max(scroll.inner_rect.min, main_rect.max),
        ) {
            self.edit(edit);
            if self.error.is_none()
                && let Some(owner) = self.instrument_ui.selected_layer
            {
                self.instrument_ui.selected_components =
                    selected.into_iter().map(|c| (owner, c)).collect();
                self.instrument_ui.component_refocus = true;
            }
            return;
        }
        self.layer_reorder(ui, &mut settings, &reorder_rows, scroll.inner_rect);
        if let Some(id) = remove_layer {
            if self
                .instrument_ui
                .name_edit
                .as_ref()
                .is_some_and(|edit| edit.layer == id)
            {
                self.instrument_ui.name_edit = None;
            }
            settings.layers.retain(|layer| layer.id != id);
            if self.instrument_ui.selected_layer == Some(id) {
                self.instrument_ui.selected_layer = settings.layers.first().map(|layer| layer.id);
            }
        }
        if Some(&settings) != track.instrument.as_ref() {
            self.edit(Edit::SetInstrument {
                track_id: track.id,
                instrument: Some(settings),
            });
        }
    }

    fn layer_name(&mut self, ui: &mut egui::Ui, layer: &mut InstrumentLayerSettings) -> Rect {
        let width = ui.available_width();
        if self
            .instrument_ui
            .name_edit
            .as_ref()
            .is_some_and(|edit| edit.layer == layer.id)
        {
            let mut edit = self.instrument_ui.name_edit.take().unwrap();
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(ui.make_persistent_id("layer-name"))
                .font(egui::FontId::proportional(rows::NAME_FONT_SIZE))
                .text_color(theme::palette(ui.ctx()).text)
                .desired_width(width)
                .min_size(Vec2::new(width, toolbars::CONTROL_HEIGHT))
                .show(ui);
            output.response.widget_info(|| {
                egui::WidgetInfo::text_edit(ui.is_enabled(), &layer.name, &edit.text, "Layer name")
            });
            if edit.focus {
                output.response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(edit.text.chars().count()),
                    )));
                output.state.store(ui.ctx(), output.response.id);
                edit.focus = false;
            }
            if let Some(action) = inline_edit_action(ui, &output.response) {
                output.response.surrender_focus();
                if action == InlineEditAction::Commit && !edit.text.trim().is_empty() {
                    layer.name = edit.text.trim().to_owned();
                }
            } else {
                self.instrument_ui.name_edit = Some(edit);
            }
            output.response.rect
        } else {
            let response = ui.add(
                egui::Label::new(
                    egui::RichText::new(&layer.name)
                        .font(egui::FontId::proportional(rows::NAME_FONT_SIZE))
                        .color(theme::palette(ui.ctx()).text),
                )
                .truncate()
                .sense(Sense::click_and_drag()),
            );
            let response = response
                .on_hover_text(format!(
                    "{}\nDrag to reorder. Double-click to rename layer.",
                    layer.name
                ))
                .on_hover_cursor(egui::CursorIcon::Grab);
            if response.double_clicked()
                && self.instrument_ui.name_edit.is_none()
                && !self.instrument_ui.dragging()
            {
                self.instrument_ui.name_edit = Some(LayerNameEdit {
                    layer: layer.id,
                    text: layer.name.clone(),
                    focus: true,
                });
            }
            self.start_layer_drag(ui, &response, layer.id);
            response.rect
        }
    }

    fn start_layer_drag(&mut self, ui: &egui::Ui, response: &egui::Response, layer: Id) {
        if response.drag_started_by(egui::PointerButton::Primary)
            && ui.is_enabled()
            && ui.input(|input| input.focused)
            && self.edit_environment_available()
            && !self.instrument_ui.editing_name()
        {
            self.instrument_ui.selected_layer = Some(layer);
            self.instrument_ui.drag = Some(layer);
        }
    }

    fn layer_reorder(
        &mut self,
        ui: &egui::Ui,
        settings: &mut daw_core::InstrumentSettings,
        rows: &[(Id, Rect)],
        viewport: Rect,
    ) {
        let Some(id) = self.instrument_ui.drag else {
            return;
        };
        if !ui.is_enabled()
            || !self.edit_environment_available()
            || ui.input(|input| !input.focused || input.key_pressed(egui::Key::Escape))
        {
            self.instrument_ui.drag = None;
            return;
        }
        let Some(source) = settings.layers.iter().position(|layer| layer.id == id) else {
            self.instrument_ui.drag = None;
            return;
        };
        let target = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|p| viewport.contains(*p))
            .map(|pointer| {
                let slot = rows
                    .iter()
                    .position(|(_, row)| pointer.y < row.center().y)
                    .unwrap_or(rows.len());
                (slot, slot - usize::from(slot > source))
            });
        if let Some((slot, index)) = target
            && index != source
        {
            let y = rows
                .get(slot)
                .map_or_else(|| rows.last().unwrap().1.bottom(), |(_, row)| row.top())
                .clamp(viewport.top(), viewport.bottom());
            ui.painter_at(viewport).hline(
                viewport.x_range(),
                y,
                Stroke::new(2.0, theme::accent(ui.ctx())),
            );
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NotAllowed
        });
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            self.instrument_ui.drag = None;
            if let Some((_, index)) = target
                && index != source
            {
                let layer = settings.layers.remove(source);
                settings.layers.insert(index, layer);
            }
        } else if !ui.input(|input| input.pointer.button_down(egui::PointerButton::Primary)) {
            self.instrument_ui.drag = None;
        }
    }

    fn scroll_layer_drag(&self, ui: &egui::Ui) {
        if self.edit_environment_available() {
            drag::scroll_near_edges(ui, drag::Axis::Vertical, 240.0);
        }
    }

    fn oscillator_unit(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        layer: &mut InstrumentLayerSettings,
        height: f32,
        compact: bool,
        sources: &[crate::modulation_panel::Source],
    ) -> (Rect, Rect, Rect) {
        let layer_id = layer.id;
        let targets: Vec<_> = layer
            .oscillators
            .iter()
            .enumerate()
            .map(|(i, s)| crate::modulation_panel::Target {
                parameter: daw_core::ModulationParameter::Pitch,
                id: Some(s.id),
                label: format!("Osc {} · Pitch", i + 1),
            })
            .collect();
        let unit_enabled = layer.oscillator_enabled;
        let InstrumentLayerSettings {
            oscillators,
            oscillator_modulations: routes,
            oscillator_modulation_open: modulation_open,
            oscillator_enabled: enabled,
            oscillator_collapsed: collapsed,
            ..
        } = layer;
        let panel = panels::PanelStyle::default();
        let strip_margin = panel.margin;
        let strip_height = crate::envelope_panel::body_height(compact);
        let strip_width = OSCILLATOR_STRIP_WIDTH - 2.0 * f32::from(strip_margin);
        let mut remove = None;
        let mut add = false;
        #[cfg(test)]
        {
            self.midi_editor.oscillator_bounds.clear();
            self.midi_editor.oscillator_strip_bounds.clear();
            self.midi_editor.oscillator_power_bounds.clear();
            self.midi_editor.add_oscillator_bounds = Rect::NOTHING;
            self.midi_editor.add_oscillator_strip_bounds = Rect::NOTHING;
        }
        let open = *modulation_open;
        let result = ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")), |ui| panels::PanelStyle { margin: 0, ..panel }.unit(
            ui,
            ("Oscillator", compact),
            height,
            enabled,
            collapsed,
            |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, theme::TOOLBAR_PADDING as f32);
                    let mut previous_column: Option<Rect> = None;
                    for (index, oscillator) in oscillators.iter_mut().enumerate() {
                        let target_routes: Vec<_> = routes.iter().filter(|r| r.target == Some(oscillator.id)).copied().collect();
                        let live = self.output.as_ref().and_then(|o| o.modulation_values(daw_engine::ModulationTarget::Oscillator { track, layer: layer_id, oscillator: oscillator.id }))
                            .map_or([None; 2], |v| [v[0], v[1]].map(|v| v.is_finite().then_some(v)));
                        let marker = crate::modulation_panel::marker(&target_routes, sources, daw_core::ModulationParameter::Pitch,
                            f32::from(oscillator.transpose), unit_enabled && !oscillator.muted, None, live);
                        let strip = ui.scope_builder(
                            egui::UiBuilder::new()
                                .id_salt(("oscillator-strip", track, oscillator.id))
                                .sense(Sense::click_and_drag()),
                            |ui| {
                                egui::Frame::new()
                                    .inner_margin(strip_margin)
                                    .show(ui, |ui| {
                                        ui.scope_builder(
                                            egui::UiBuilder::new()
                                                .id_salt(("oscillator-settings", track, oscillator.id))
                                                .layout(egui::Layout::top_down(egui::Align::Min))
                                                .sense(Sense::click_and_drag()),
                                            |ui| {
                                                ui.set_width(strip_width);
                                                let mut responses = Vec::with_capacity(4);
                                                if !compact {
                                                    responses.extend([
                                                        knobs::row(
                                                            ui,
                                                            &mut oscillator.waveform,
                                                            knobs::CHOICE_SIZE,
                                                            knobs::waveform,
                                                        ),
                                                        knobs::row(
                                                            ui,
                                                            &mut oscillator.transpose,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                let mut scalar = f32::from(*value);
                                                                let response = ui.add(knobs::Knob::new(&mut scalar, knobs::pitch_spec()).modulated(marker.as_ref(), modulation_open));
                                                                if response.changed() { *value = scalar as i8; }
                                                                response
                                                            },
                                                        ),
                                                        knobs::row(
                                                            ui,
                                                            &mut oscillator.gain_db,
                                                            knobs::CHOICE_SIZE,
                                                            knobs::gain_with_markers,
                                                        ),
                                                    ]);
                                                }
                                                responses.push(
                                                    crate::rows::centered(
                                                        ui,
                                                        if compact {
                                                            strip_height
                                                        } else {
                                                            icons::COMPACT_BUTTON_SIZE
                                                        },
                                                        |ui| {
                                                            let on = !oscillator.muted;
                                                            let response = rows::power_label(
                                                                ui,
                                                                &format!("Osc {}", index + 1),
                                                                &format!(
                                                                    "Toggle oscillator {}",
                                                                    index + 1
                                                                ),
                                                                on,
                                                            );
                                                            if response.clicked() {
                                                                oscillator.muted =
                                                                    !oscillator.muted;
                                                            }
                                                            #[cfg(test)]
                                                            self.midi_editor
                                                                .oscillator_power_bounds
                                                                .push(response.rect);
                                                            response
                                                        },
                                                    )
                                                    .inner,
                                                );
                                                responses
                                            },
                                        )
                                    })
                            },
                        );
                        self.instrument_ui.oscillator_slots.push((layer_id, oscillator.id, strip.response.rect));
                        let column = strip.inner.inner;
                        for response in [&column.response, &strip.response] {
                            self.start_installed_component_drag(ui, response, &column.inner.iter().map(|r|r.rect).collect::<Vec<_>>(), track, layer_id, daw_core::InstrumentComponent::Oscillator(oscillator.id));
                        }
                        if let Some(previous) = previous_column {
                            panels::unit_divider(
                                ui,
                                strip.response.rect.left(),
                                previous.top()..=strip.response.rect.bottom(),
                            );
                        }
                        previous_column = Some(strip.response.rect);
                        #[cfg(test)]
                        {
                            self.midi_editor
                                .oscillator_bounds
                                .push(column.response.rect);
                            self.midi_editor
                                .oscillator_strip_bounds
                                .push(strip.response.rect);
                        }
                        // The column covers gaps; knob responses cover their interactive faces.
                        for response in column
                            .inner
                            .iter()
                            .chain([&column.response, &strip.response])
                        {
                            response.context_menu(|ui| {
                                if ui
                                    .add_enabled(
                                        response.enabled(),
                                        egui::Button::new("Remove oscillator"),
                                    )
                                    .clicked()
                                {
                                    remove = Some(index);
                                    ui.close();
                                }
                            });
                        }
                    }
                    let add_strip = ui.scope(|ui| {
                        icons::circular_strip_button(
                            ui,
                            icons::Lucide::Plus,
                            "Add oscillator",
                            Vec2::new(icons::COMPACT_BUTTON_SIZE, strip_height)
                                + Vec2::splat(2.0 * f32::from(strip_margin)),
                            icons::COMPACT_BUTTON_SIZE,
                        )
                    });
                    if let Some(previous) = previous_column {
                        panels::unit_divider(
                            ui,
                            add_strip.response.rect.left(),
                            previous.top()..=add_strip.response.rect.bottom(),
                        );
                    }
                    #[cfg(test)]
                    {
                        self.midi_editor.add_oscillator_bounds = add_strip.inner.rect;
                        self.midi_editor.add_oscillator_strip_bounds = add_strip.response.rect;
                    }
                    add = add_strip.inner.clicked();
                    if open {
                        panels::unit_divider(ui, add_strip.response.rect.right(), add_strip.response.rect.y_range().into());
                        crate::modulation_panel::targeted_body(ui, routes, sources, &targets, daw_unit_core::ModulationScope::PerNote, compact);
                    }
                });
            },
        )).inner;
        crate::modulation_panel::toggle(ui, &result.header, *collapsed, modulation_open);
        self.start_installed_component_drag(
            ui,
            &result.header,
            &[result.power.rect],
            track,
            layer_id,
            daw_core::InstrumentComponent::Oscillators,
        );

        let remove_id = remove.map(|index| oscillators[index].id);
        if add {
            oscillators.push(Default::default());
        }
        if let Some(id) = remove_id {
            layer.remove_oscillator(id);
        }
        #[cfg(test)]
        {
            self.midi_editor.oscillator_panel_bounds = result.response.rect;
        }
        (result.response.rect, result.header.rect, result.power.rect)
    }
}
