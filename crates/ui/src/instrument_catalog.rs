//! Catalog adapter for the components available within an instrument layer.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Component {
    Oscillator,
    Envelope,
    Lfo,
    Amplitude,
}

pub(super) const ENTRIES: [(&str, &str, Component); 4] = [
    ("daw.oscillator", "Oscillator", Component::Oscillator),
    ("daw.envelope", "Envelope", Component::Envelope),
    ("daw.lfo", "LFO", Component::Lfo),
    ("daw.amplitude", "Amplitude", Component::Amplitude),
];

impl Component {
    pub(super) fn add_to(self, layer: &mut daw_core::InstrumentLayerSettings) {
        match self {
            Self::Oscillator => layer.oscillators.push(Default::default()),
            // Each Envelope is independent; Amplitude is a single retained audio stage.
            Self::Envelope => add_envelope(&mut layer.envelopes),
            Self::Lfo => add_lfo(&mut layer.lfos),
            Self::Amplitude => layer.amplitude.present = true,
        }
    }
}

fn add_envelope(envelopes: &mut Vec<daw_core::EnvelopeInstance>) {
    if let Some(e) = envelopes.iter_mut().find(|e| !e.present) {
        e.present = true;
    } else {
        envelopes.push(Default::default());
    }
}
fn add_lfo(lfos: &mut Vec<daw_core::LfoInstance>) {
    if let Some(l) = lfos.iter_mut().find(|l| !l.present) {
        l.present = true;
    } else {
        lfos.push(Default::default());
    }
}

pub(super) struct ComponentDrag {
    project: Id,
    track: Id,
    instrument: daw_core::InstrumentSettings,
    start: catalog::DragStart,
    pub active: bool,
}

fn add_components(settings: &mut daw_core::InstrumentSettings, layer_id: Id, types: &[&str]) {
    if settings.main.id == layer_id {
        for (id, _, component) in ENTRIES {
            if !types.contains(&id) {
                continue;
            }
            match component {
                Component::Envelope => {
                    add_envelope(&mut settings.main.envelopes);
                    for e in &mut settings.main.envelopes {
                        if e.mode == daw_core::EnvelopeMode::PerNote {
                            e.mode = daw_core::EnvelopeMode::Legato;
                        }
                    }
                }
                Component::Lfo => add_lfo(&mut settings.main.lfos),
                Component::Oscillator | Component::Amplitude => {}
            }
        }
        return;
    }
    if let Some(layer) = settings
        .layers
        .iter_mut()
        .find(|layer| layer.id == layer_id)
    {
        for (id, _, component) in ENTRIES {
            if types.contains(&id) {
                component.add_to(layer);
            }
        }
    }
}

#[cfg(test)]
pub(crate) struct Bounds {
    pub toggle: Rect,
    pub library: Rect,
    pub header: Rect,
    pub entries: [Rect; ENTRIES.len()],
    pub drop_target: Option<Id>,
    pub drop_bounds: Rect,
    pub layer_viewport: Rect,
    pub layer_scroll: f32,
}

#[cfg(test)]
impl Default for Bounds {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            library: Rect::NOTHING,
            header: Rect::NOTHING,
            entries: [Rect::NOTHING; ENTRIES.len()],
            drop_target: None,
            drop_bounds: Rect::NOTHING,
            layer_viewport: Rect::NOTHING,
            layer_scroll: 0.0,
        }
    }
}

impl DawUi {
    pub(super) fn instrument_catalog_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        allowed: bool,
    ) {
        let target = self.instrument_catalog_target();
        let enabled = allowed && self.effects_drag.is_none() && target.is_some();
        let output = self.instrument_catalog.show(
            ui,
            panel,
            bounds,
            "Instrument components",
            "components",
            catalog::Interaction {
                rows_enabled: enabled
                    && self.component_drag.as_ref().is_none_or(|drag| !drag.active),
                keyboard_enabled: enabled
                    && self.component_drag.is_none()
                    && !ui.input(|input| input.pointer.primary_down()),
                clear_on_empty: allowed
                    && self.effects_drag.is_none()
                    && self.component_drag.is_none(),
                empty_enabled: allowed,
            },
        );
        if target.is_some_and(|(track_id, owner)| {
            self.session
                .project
                .tracks
                .iter()
                .find(|t| t.id == track_id)
                .and_then(|t| t.instrument.as_ref())
                .is_some_and(|i| i.main.id == owner)
        }) {
            for index in [0, 3] {
                output.rows[index].clone().on_hover_text("Main accepts Envelope and LFO components. Drag this component onto a regular layer.");
            }
        }
        if enabled
            && self.component_drag.is_none()
            && let Some((track, _)) = target
        {
            for (index, response) in output.rows.iter().enumerate() {
                if let Some(start) = self.instrument_catalog.start_drag(ui, response, index)
                    && let Some(instrument) = self
                        .session
                        .project
                        .tracks
                        .iter()
                        .find(|candidate| candidate.id == track)
                        .and_then(|track| track.instrument.clone())
                {
                    self.component_drag = Some(ComponentDrag {
                        project: self.session.project.project_id,
                        track,
                        instrument,
                        start,
                        active: false,
                    });
                    break;
                }
            }
        }
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.library = bounds;
            self.instrument_catalog_bounds.header = output.header;
            self.instrument_catalog_bounds.entries =
                std::array::from_fn(|index| output.rows[index].rect);
        }
        #[cfg(not(test))]
        {
            let _ = output.header;
        }
        if enabled
            && (output.add_selected || output.double_clicked.is_some())
            && let Some((track_id, layer_id)) = target
        {
            let mut settings = self
                .session
                .project
                .tracks
                .iter()
                .find(|track| track.id == track_id)
                .and_then(|track| track.instrument.clone())
                .unwrap();
            let original = settings.clone();
            let types: Vec<_> = self.instrument_catalog.selection.iter().copied().collect();
            add_components(&mut settings, layer_id, &types);
            if settings != original {
                self.edit(Edit::SetInstrument {
                    track_id,
                    instrument: Some(settings),
                });
            }
        }
    }

    pub(super) fn cancel_unavailable_component_drag(&mut self, ui: &egui::Ui) {
        let invalid = self.component_drag.as_ref().is_some_and(|drag| {
            self.view_mode != ViewMode::Instrument
                || !self.instrument_catalog_visible
                || self.session.project.project_id != drag.project
                || self.selected_track != Some(drag.track)
                || !self.effects_editing_enabled(ui)
                || self.effects_drag.is_some()
                || drag::cancelled(ui)
                || !self.session.project.tracks.iter().any(|track| {
                    track.id == drag.track
                        && track.kind == daw_core::TrackKind::Midi
                        && track.instrument.as_ref() == Some(&drag.instrument)
                })
        });
        if invalid {
            if self.view_mode == ViewMode::Instrument
                && self.instrument_catalog_visible
                && self.edit_environment_available()
                && ui.input(|input| input.focused && input.key_pressed(egui::Key::Escape))
                && let Some(drag) = &self.component_drag
            {
                drag::restore_focus(ui.ctx(), drag.start.focus);
            }
            self.component_drag = None;
        }
    }

    pub(super) fn activate_component_drag(&mut self, ui: &egui::Ui) {
        if let Some(drag) = &mut self.component_drag
            && !drag.active
            && drag::crossed_threshold(ui, drag.start.origin)
        {
            drag.active = true;
            self.instrument_catalog.select_dragged(drag.start.index);
        }
    }

    pub(super) fn finish_component_drag(
        &mut self,
        ui: &egui::Ui,
        track: Id,
        settings: &mut daw_core::InstrumentSettings,
        rows: &[(Id, Rect)],
        viewport: Rect,
    ) {
        self.cancel_unavailable_component_drag(ui);
        let Some(drag) = &self.component_drag else {
            return;
        };
        if !drag.active {
            if !ui.input(|input| input.pointer.primary_down()) {
                self.component_drag = None;
            }
            return;
        }
        // Rendering never edits the snapshot while a catalog drag owns the gesture.
        if drag.track != track || settings != &drag.instrument {
            self.component_drag = None;
            return;
        }
        let target = ui
            .input(|input| input.pointer.latest_pos())
            .filter(|pointer| viewport.contains(*pointer))
            .and_then(|pointer| {
                rows.iter().find_map(|(id, row)| {
                    let rect = Rect::from_min_max(
                        Pos2::new(viewport.left(), row.top()),
                        Pos2::new(viewport.right(), row.bottom()),
                    )
                    .intersect(viewport);
                    rect.contains(pointer).then_some((*id, rect))
                })
            });
        let target = target.filter(|(id, _)| {
            *id != settings.main.id
                || drag
                    .start
                    .types
                    .iter()
                    .any(|t| ["daw.envelope", "daw.lfo"].contains(t))
        });
        if let Some((_, rect)) = target {
            panels::selection_border(ui, rect);
        }
        #[cfg(test)]
        {
            self.instrument_catalog_bounds.drop_target = target.map(|(id, _)| id);
            self.instrument_catalog_bounds.drop_bounds =
                target.map_or(Rect::NOTHING, |(_, rect)| rect);
        }
        ui.ctx().set_cursor_icon(if target.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NotAllowed
        });
        if ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            let drag = self.component_drag.take().unwrap();
            drag::restore_focus(ui.ctx(), drag.start.focus);
            if let Some((layer, _)) = target {
                add_components(settings, layer, &drag.start.types);
                self.instrument_ui.selected_layer = Some(layer);
                ui.ctx().request_repaint();
            }
        } else if !ui.input(|input| input.pointer.primary_down()) {
            self.component_drag = None;
        }
    }

    fn instrument_catalog_target(&self) -> Option<(Id, Id)> {
        let track = self.session.project.tracks.iter().find(|track| {
            Some(track.id) == self.selected_track && track.kind == daw_core::TrackKind::Midi
        })?;
        let instrument = track.instrument.as_ref()?;
        if self.instrument_ui.track == Some(track.id)
            && self.instrument_ui.selected_layer == Some(instrument.main.id)
        {
            return Some((track.id, instrument.main.id));
        }
        let layer = instrument
            .layers
            .iter()
            .find(|layer| {
                self.instrument_ui.track == Some(track.id)
                    && Some(layer.id) == self.instrument_ui.selected_layer
            })
            .or_else(|| instrument.layers.first())?;
        Some((track.id, layer.id))
    }
}
