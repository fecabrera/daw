//! Catalog adapter for the components available within an instrument layer.
use super::*;
use crate::instrument_component_drag::{ComponentDrag, ComponentDragSource};

/// Register construction once. Available constructors also define valid catalog drop targets.
#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub type_id: &'static str,
    pub name: &'static str,
    add_layer: fn(&mut daw_core::InstrumentLayerSettings),
    add_main: Option<fn(&mut daw_core::InstrumentBusSettings)>,
}
impl Entry {
    const fn layer(
        type_id: &'static str,
        name: &'static str,
        add: fn(&mut daw_core::InstrumentLayerSettings),
    ) -> Self {
        Self {
            type_id,
            name,
            add_layer: add,
            add_main: None,
        }
    }
    const fn with_main(mut self, add: fn(&mut daw_core::InstrumentBusSettings)) -> Self {
        self.add_main = Some(add);
        self
    }
    pub(super) fn supports_main(self) -> bool {
        self.add_main.is_some()
    }
    fn can_add(self, settings: &daw_core::InstrumentSettings, owner: Id) -> bool {
        if owner == settings.main.id {
            self.supports_main()
        } else {
            settings.layers.iter().any(|layer| layer.id == owner)
        }
    }
    fn add_to(self, settings: &mut daw_core::InstrumentSettings, owner: Id) {
        if owner == settings.main.id {
            if let Some(add) = self.add_main {
                add(&mut settings.main);
            }
        } else if let Some(layer) = settings.layers.iter_mut().find(|layer| layer.id == owner) {
            (self.add_layer)(layer);
        }
    }
}

pub(super) const ENTRIES: [Entry; 5] = [
    Entry::layer("daw.oscillator", "Oscillator", |l| {
        l.oscillators.push(Default::default())
    }),
    Entry::layer("daw.envelope", "Envelope", |l| {
        add_envelope(&mut l.envelopes)
    })
    .with_main(|main| {
        add_envelope(&mut main.envelopes);
        for envelope in &mut main.envelopes {
            if envelope.mode == daw_core::EnvelopeMode::PerNote {
                envelope.mode = daw_core::EnvelopeMode::Legato;
            }
        }
    }),
    Entry::layer("daw.lfo", "LFO", |l| add_lfo(&mut l.lfos))
        .with_main(|main| add_lfo(&mut main.lfos)),
    Entry::layer("daw.amplitude", "Amplitude", |l| l.amplitude.present = true),
    Entry::layer("daw.macro", "Macro", |l| l.macro_module.add())
        .with_main(|main| main.macro_module.add()),
];

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

pub(super) fn add_components(
    settings: &mut daw_core::InstrumentSettings,
    layer_id: Id,
    types: &[&str],
) {
    add_registered(&ENTRIES, settings, layer_id, types);
}

fn add_registered(
    entries: &[Entry],
    settings: &mut daw_core::InstrumentSettings,
    owner: Id,
    types: &[&str],
) {
    for entry in entries {
        if types.contains(&entry.type_id) {
            entry.add_to(settings, owner);
        }
    }
}

/// Check registration metadata only; previewing must never construct units or assign IDs.
pub(super) fn can_add_components(
    settings: &daw_core::InstrumentSettings,
    owner: Id,
    types: &[&str],
) -> bool {
    can_add_registered(&ENTRIES, settings, owner, types)
}

fn can_add_registered(
    entries: &[Entry],
    settings: &daw_core::InstrumentSettings,
    owner: Id,
    types: &[&str],
) -> bool {
    entries
        .iter()
        .any(|entry| types.contains(&entry.type_id) && entry.can_add(settings, owner))
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
            for (index, entry) in ENTRIES.iter().enumerate() {
                if !entry.supports_main() {
                    output.rows[index].clone().on_hover_text(
                        "This component requires a regular layer. Drag it onto a regular layer.",
                    );
                }
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
                        source: ComponentDragSource::Catalog {
                            types: start.types,
                            index: start.index,
                        },
                        origin: start.origin,
                        focus: start.focus,
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

#[cfg(test)]
mod registration_tests {
    use super::*;
    thread_local! { static CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

    #[test]
    fn new_registration_enables_placement_without_a_type_allowlist_or_preview_construction() {
        let entries = [Entry::layer("test.future", "Future", |layer| {
            CONSTRUCTIONS.with(|n| n.set(n.get() + 1));
            layer.macro_module.add();
        })
        .with_main(|main| {
            CONSTRUCTIONS.with(|n| n.set(n.get() + 1));
            main.macro_module.add();
        })];
        let mut settings = daw_core::InstrumentSettings::default();
        let layer = settings.layers[0].id;
        let main = settings.main.id;
        let before = settings.clone();
        CONSTRUCTIONS.with(|n| n.set(0));
        for owner in [layer, main] {
            assert!(can_add_registered(
                &entries,
                &settings,
                owner,
                &["test.future"]
            ));
            assert!(!can_add_registered(
                &entries,
                &settings,
                owner,
                &["test.unregistered"]
            ));
        }
        assert!(!can_add_registered(
            &entries,
            &settings,
            Id::nil(),
            &["test.future"]
        ));
        assert_eq!(settings, before);
        assert_eq!(CONSTRUCTIONS.with(|n| n.get()), 0);
        for owner in [layer, main] {
            add_registered(&entries, &mut settings, owner, &["test.future"]);
        }
        assert_eq!(CONSTRUCTIONS.with(|n| n.get()), 2);
        assert!(settings.layers[0].macro_module.present && settings.main.macro_module.present);
        assert!(settings.valid());
        let layer_only = [Entry {
            add_main: None,
            ..entries[0]
        }];
        assert!(!can_add_registered(
            &layer_only,
            &settings,
            main,
            &["test.future"]
        ));
        let before = settings.clone();
        add_registered(&layer_only, &mut settings, main, &["test.future"]);
        assert_eq!(settings, before);
        assert_eq!(CONSTRUCTIONS.with(|n| n.get()), 2);
    }
}
