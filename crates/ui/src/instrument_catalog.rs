//! Catalog adapter for the components available within an instrument layer.
use super::*;
use crate::instrument_component_drag::{ComponentDrag, ComponentDragSource};
use daw_core::ModulationLaneKind;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Category {
    Sources,
    Modulators,
    Processors,
}
impl Category {
    const ALL: [Self; 3] = [Self::Sources, Self::Modulators, Self::Processors];

    fn name(self) -> &'static str {
        match self {
            Self::Sources => "Sources",
            Self::Modulators => "Modulators",
            Self::Processors => "Processors",
        }
    }
}

/// Register construction once. Available constructors also define valid catalog drop targets.
#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub type_id: &'static str,
    pub name: &'static str,
    category: Category,
    add_layer: fn(&mut daw_core::InstrumentLayerSettings),
    add_main: Option<fn(&mut daw_core::InstrumentBusSettings)>,
    add_provider: Option<fn(&mut daw_core::InstrumentBusSettings)>,
    source: bool,
    provider_lane: Option<ModulationLaneKind>,
    layer_available: fn(&daw_core::InstrumentLayerSettings) -> bool,
}
impl Entry {
    const fn layer(
        type_id: &'static str,
        name: &'static str,
        category: Category,
        add: fn(&mut daw_core::InstrumentLayerSettings),
    ) -> Self {
        Self {
            type_id,
            name,
            category,
            add_layer: add,
            add_main: None,
            add_provider: None,
            source: false,
            provider_lane: None,
            layer_available: |_| true,
        }
    }
    const fn source(mut self, available: fn(&daw_core::InstrumentLayerSettings) -> bool) -> Self {
        self.source = true;
        self.layer_available = available;
        self
    }
    const fn with_main(mut self, add: fn(&mut daw_core::InstrumentBusSettings)) -> Self {
        self.add_main = Some(add);
        self
    }
    pub(super) fn supports_main(self) -> bool {
        self.add_main.is_some()
    }
    const fn provider(
        type_id: &'static str,
        name: &'static str,
        lane: ModulationLaneKind,
        add: fn(&mut daw_core::InstrumentBusSettings),
    ) -> Self {
        Self {
            provider_lane: Some(lane),
            add_provider: Some(add),
            ..Self::layer(type_id, name, Category::Modulators, |_| {})
        }
    }
    fn can_add(self, settings: &daw_core::InstrumentSettings, owner: Id) -> bool {
        if owner == settings.lfo_lane.id || owner == settings.envelope_lane.id {
            self.provider_lane == settings.modulation_lane_kind(owner)
        } else if self.provider_lane.is_some() {
            false
        } else if owner == settings.main.id {
            self.supports_main()
        } else {
            settings
                .layers
                .iter()
                .any(|layer| layer.id == owner && (self.layer_available)(layer))
        }
    }
    fn add_to(self, settings: &mut daw_core::InstrumentSettings, owner: Id) {
        if settings.modulation_lane_kind(owner).is_some() {
            if let Some(add) = self.add_provider {
                add(settings.bus_mut(owner).unwrap());
            }
        } else if owner == settings.main.id {
            if let Some(add) = self.add_main {
                add(&mut settings.main);
            }
        } else if let Some(layer) = settings.layers.iter_mut().find(|layer| layer.id == owner) {
            (self.add_layer)(layer);
        }
    }
}

pub(super) const ENTRIES: [Entry; 7] = [
    Entry::layer("daw.oscillator", "Oscillator", Category::Sources, |l| {
        l.oscillators.push(Default::default())
    })
    .source(|l| l.sampler.is_none()),
    Entry::provider(
        "daw.envelope",
        "Envelope",
        ModulationLaneKind::Envelope,
        |bus| add_envelope(&mut bus.envelopes),
    ),
    Entry::provider("daw.lfo", "LFO", ModulationLaneKind::Lfo, |bus| {
        add_lfo(&mut bus.lfos)
    }),
    Entry::layer("daw.amplitude", "Amplitude", Category::Processors, |l| {
        l.amplitude.present = true
    }),
    Entry::layer("daw.macro", "Macro", Category::Modulators, |l| {
        l.macro_module.add()
    })
    .with_main(|main| main.macro_module.add()),
    Entry::layer("daw.sampler", "Sampler", Category::Sources, |l| {
        l.sampler = Some(Default::default())
    })
    .source(|l| l.sampler.is_none() && l.oscillators.is_empty()),
    Entry::layer("daw.granulizer", "Granulizer", Category::Sources, |l| {
        l.sampler = Some(daw_core::SamplerSettings::granular())
    })
    .source(|l| l.sampler.is_none() && l.oscillators.is_empty()),
];

pub(super) fn catalog() -> catalog::Catalog {
    catalog::Catalog::new(ENTRIES.map(|entry| (entry.type_id, entry.name))).with_folders(
        Category::ALL.map(|category| {
            (
                category.name(),
                ENTRIES
                    .iter()
                    .filter(|entry| entry.category == category)
                    .map(|entry| entry.type_id)
                    .collect(),
            )
        }),
    )
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
    if !can_add_registered(entries, settings, owner, types) {
        return;
    }
    for entry in entries {
        if types.contains(&entry.type_id) && entry.can_add(settings, owner) {
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
    (!(owner == settings.lfo_lane.id || owner == settings.envelope_lane.id)
        || types.iter().all(|id| {
            entries
                .iter()
                .find(|e| e.type_id == *id)
                .is_some_and(|e| e.can_add(settings, owner))
        }))
        && (owner == settings.main.id
            || (entries
                .iter()
                .filter(|e| e.source && types.contains(&e.type_id))
                .count()
                <= 1
                && entries
                    .iter()
                    .filter(|e| e.source && types.contains(&e.type_id))
                    .all(|e| e.can_add(settings, owner))))
        && entries
            .iter()
            .any(|entry| types.contains(&entry.type_id) && entry.can_add(settings, owner))
}

#[cfg(test)]
pub(crate) struct Bounds {
    pub toggle: Rect,
    pub library: Rect,
    pub header: Rect,
    pub entries: [Rect; ENTRIES.len()],
    pub folders: [Rect; Category::ALL.len()],
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
            folders: [Rect::NOTHING; Category::ALL.len()],
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
        if let Some((track, owner)) = target
            && let Some(settings) = self
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == track)
                .and_then(|t| t.instrument.as_ref())
        {
            for (index, response) in &output.rows {
                let entry = ENTRIES[*index];
                if let Some(lane) = settings.modulation_lane_kind(owner)
                    && !entry.can_add(settings, owner)
                {
                    response.clone().on_hover_text(format!(
                        "The {} lane accepts only {} units.",
                        lane.name(),
                        lane.name()
                    ));
                } else if !entry.can_add(settings, owner) && !entry.source {
                    response.clone().on_hover_text(match entry.provider_lane {
                        Some(ModulationLaneKind::Lfo) => "Add LFOs to the LFO lane.",
                        Some(ModulationLaneKind::Envelope) => "Add envelopes to the Envelope lane.",
                        None => "This component is not supported by the selected lane.",
                    });
                } else if entry.source && !entry.can_add(settings, owner) {
                    response.clone().on_hover_text("A layer can contain Oscillator, Sampler, or Granulizer. Remove its source or use Replace source in the installed source header menu.");
                }
            }
        }
        if target.is_some_and(|(track_id, owner)| {
            self.session
                .project
                .tracks
                .iter()
                .find(|t| t.id == track_id)
                .and_then(|t| t.instrument.as_ref())
                .is_some_and(|i| i.main.id == owner)
        }) {
            for (index, response) in &output.rows {
                let entry = ENTRIES[*index];
                if !entry.supports_main() && entry.provider_lane.is_none() {
                    response.clone().on_hover_text(
                        "This component requires a regular layer. Drag it onto a regular layer.",
                    );
                }
            }
        }
        if enabled
            && self.component_drag.is_none()
            && let Some((track, _)) = target
        {
            for (index, response) in &output.rows {
                if let Some(start) = self.instrument_catalog.start_drag(ui, response, *index)
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
            self.instrument_catalog_bounds.entries = [Rect::NOTHING; ENTRIES.len()];
            for (index, response) in &output.rows {
                self.instrument_catalog_bounds.entries[*index] = response.rect;
            }
            for (index, response) in &output.folders {
                self.instrument_catalog_bounds.folders[*index] = response.rect;
            }
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
            let types: Vec<_> = self.instrument_catalog.selected_types();
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
            && let Some(owner) = self.instrument_ui.selected_layer
            && instrument.bus(owner).is_some()
        {
            return Some((track.id, owner));
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
        let entries = [
            Entry::layer("test.future", "Future", Category::Modulators, |layer| {
                CONSTRUCTIONS.with(|n| n.set(n.get() + 1));
                layer.macro_module.add();
            })
            .with_main(|main| {
                CONSTRUCTIONS.with(|n| n.set(n.get() + 1));
                main.macro_module.add();
            }),
        ];
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
