//! Saved Macro strips: independent shared providers in one targeted destination module.
use crate::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MACRO_TYPE_ID: &str = "daw.macro";
pub const MACRO_STATE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroSettings {
    pub version: u32,
    pub value: f32,
}
impl Default for MacroSettings {
    fn default() -> Self {
        Self {
            version: MACRO_STATE_VERSION,
            value: 0.0,
        }
    }
}
impl MacroSettings {
    pub fn valid(&self) -> bool {
        self.version == MACRO_STATE_VERSION
            && self.value.is_finite()
            && (0.0..=1.0).contains(&self.value)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroInstance {
    pub id: Uuid,
    /// None keeps the legacy, position-based "Macro N" label.
    #[serde(default)]
    pub name: Option<String>,
    pub present: bool,
    pub enabled: bool,
    pub settings: MacroSettings,
}
impl Default for MacroInstance {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: None,
            present: true,
            enabled: true,
            settings: MacroSettings::default(),
        }
    }
}
impl MacroInstance {
    pub fn display_name(&self, index: usize) -> std::borrow::Cow<'_, str> {
        self.name.as_deref().map_or_else(
            || std::borrow::Cow::Owned(format!("Macro {}", index + 1)),
            std::borrow::Cow::Borrowed,
        )
    }
    pub fn valid_name(name: &str) -> bool {
        !name.trim().is_empty() && !name.chars().any(char::is_control)
    }
}
impl ModulationSourceSettings for MacroInstance {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = ModulationOutputDescriptor {
        name: "Macro",
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    };
    fn modulation_source_id(&self) -> Uuid {
        self.id
    }
    fn modulation_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    fn modulation_scope(&self) -> ModulationScope {
        ModulationScope::Shared
    }
    fn modulation_present(&self) -> bool {
        self.present
    }
    fn modulation_enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroModuleSettings {
    pub present: bool,
    pub enabled: bool,
    pub collapsed: bool,
    pub modulation_open: bool,
    pub controls: Vec<MacroInstance>,
    pub modulations: Vec<Modulation>,
}
impl Default for MacroModuleSettings {
    fn default() -> Self {
        Self {
            present: false,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            controls: Vec::new(),
            modulations: Vec::new(),
        }
    }
}
impl MacroModuleSettings {
    /// Restore the first retained strip before creating a new stable provider.
    pub fn add(&mut self) {
        if !self.present && !self.controls.is_empty() {
            self.present = true;
            return;
        }
        self.present = true;
        if let Some(control) = self.controls.iter_mut().find(|c| !c.present) {
            control.present = true;
        } else {
            self.controls.push(MacroInstance::default());
        }
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.controls.iter().all(|c| {
            !c.id.is_nil()
                && ids.insert(c.id)
                && c.settings.valid()
                && c.name.as_deref().is_none_or(MacroInstance::valid_name)
        }) && self.valid_modulations()
    }
    pub fn sources<'a>(
        &'a self,
        layer: Uuid,
        name: &'a str,
    ) -> impl Iterator<Item = ModulationSourceInfo<'a>> {
        self.controls
            .iter()
            .enumerate()
            .map(move |(index, control)| {
                let mut source = control.modulation_source(layer, name, index);
                source.present &= self.present;
                source.enabled &= self.enabled;
                source
            })
    }
}
impl ModulationDestinationSettings for MacroModuleSettings {
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &[ModulationParameter::MacroValue];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulations(&self) -> &[Modulation] {
        &self.modulations
    }
    fn valid_modulations(&self) -> bool {
        valid_targeted_modulations(
            &self.modulations,
            Self::MODULATION_PARAMETERS,
            &self.controls.iter().map(|c| c.id).collect::<Vec<_>>(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn macro_names_default_for_legacy_state_and_publish_without_changing_identity() {
        let mut module = MacroModuleSettings::default();
        module.add();
        module.add();
        let mut saved = serde_json::to_value(&module).unwrap();
        for strip in saved["controls"].as_array_mut().unwrap() {
            strip.as_object_mut().unwrap().remove("name");
        }
        assert_eq!(
            serde_json::from_value::<MacroModuleSettings>(saved).unwrap(),
            module
        );
        assert_eq!(module.controls[1].display_name(1), "Macro 2");
        let id = module.controls[1].id;
        module.controls[1].name = Some("Brightness 🎛".into());
        let source = module.sources(Uuid::new_v4(), "Main").nth(1).unwrap();
        assert_eq!(source.id.unit, id);
        assert_eq!(source.name, Some("Brightness 🎛"));
        assert_eq!(source.display_name(), "Brightness 🎛");
        assert_eq!(module.controls[1].display_name(1), "Brightness 🎛");
        assert_eq!(
            serde_json::from_slice::<MacroModuleSettings>(&serde_json::to_vec(&module).unwrap())
                .unwrap(),
            module
        );
        for name in ["", " \t", "Tone\nControl", "Tone\u{7f}"] {
            module.controls[1].name = Some(name.into());
            assert!(!module.valid());
        }
    }
    #[test]
    fn independent_strips_validate_and_restore_without_changing_identity_or_routes() {
        let mut module = MacroModuleSettings::default();
        module.add();
        module.add();
        assert_ne!(module.controls[0].id, module.controls[1].id);
        let retained = module.controls[0].id;
        module.controls[0].present = false;
        module.add();
        assert_eq!(module.controls.len(), 2);
        assert_eq!(module.controls[0].id, retained);
        let before = module.clone();
        module.present = false;
        module.add();
        assert_eq!(module, before);
        let mut route = Modulation::new(ModulationParameter::MacroValue);
        route.target = Some(retained);
        module.modulations.push(route);
        assert!(module.valid());
        module.controls.swap(0, 1);
        assert!(module.valid());
        assert_eq!(module.modulations[0].target, Some(retained));
        module.modulations[0].target = None;
        assert!(!module.valid());
        for value in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
            assert!(
                !MacroSettings {
                    value,
                    ..Default::default()
                }
                .valid()
            );
        }
    }
    #[test]
    fn old_projects_default_empty_and_malformed_state_never_falls_back() {
        let instrument = InstrumentSettings::default();
        let mut saved = serde_json::to_value(&instrument).unwrap();
        saved["layers"][0]
            .as_object_mut()
            .unwrap()
            .remove("macro_module");
        saved["main"]
            .as_object_mut()
            .unwrap()
            .remove("macro_module");
        assert_eq!(
            serde_json::from_value::<InstrumentSettings>(saved.clone()).unwrap(),
            instrument
        );
        for owner in ["layers", "main"] {
            let mut bad = saved.clone();
            if owner == "layers" {
                bad[owner][0]["macro_module"] = serde_json::Value::Null;
            } else {
                bad[owner]["macro_module"] = serde_json::Value::Null;
            }
            assert!(serde_json::from_value::<InstrumentSettings>(bad).is_err());
        }
    }
    #[test]
    fn graph_resolves_targeted_macro_dependencies_and_rejects_mixed_cycles_and_local_sources() {
        let mut instrument = InstrumentSettings::default();
        let owner = instrument.layers[0].id;
        let module = &mut instrument.layers[0].macro_module;
        module.add();
        module.add();
        let a = module.controls[0].id;
        let b = module.controls[1].id;
        let mut route = Modulation::new(ModulationParameter::MacroValue);
        route.target = Some(a);
        route.source = Some(ModulationSourceId {
            layer: owner,
            unit: b,
        });
        module.modulations.push(route);
        let lfo = LfoInstance::default();
        let lfo_id = lfo.id;
        instrument.layers[0].lfos.push(lfo);
        assert!(instrument.valid());
        let nodes: Vec<_> = instrument.provider_destinations().collect();
        let order = modulation_dependency_order(&nodes).unwrap();
        assert!(
            order.iter().position(|i| nodes[*i].0.unit == b)
                < order.iter().position(|i| nodes[*i].0.unit == a)
        );
        let mut outgoing = Modulation::new(ModulationParameter::LfoDepth);
        outgoing.source = Some(ModulationSourceId {
            layer: owner,
            unit: a,
        });
        instrument.layers[0].lfos[0].modulations.push(outgoing);
        assert!(instrument.valid());
        instrument.layers[0].macro_module.modulations[0].source = Some(ModulationSourceId {
            layer: owner,
            unit: lfo_id,
        });
        assert!(!instrument.valid());
        instrument.layers[0].macro_module.modulations[0].source = Some(ModulationSourceId {
            layer: owner,
            unit: a,
        });
        assert!(!instrument.valid());
        let envelope = &mut instrument.layers[0].envelopes[0];
        envelope.present = true;
        let id = envelope.id;
        instrument.layers[0].macro_module.modulations[0].source = Some(ModulationSourceId {
            layer: owner,
            unit: id,
        });
        assert!(!instrument.valid());
        instrument.layers[0].envelopes[0].mode = EnvelopeMode::Legato;
        assert!(instrument.valid());
        let saved = serde_json::to_vec(&instrument).unwrap();
        assert_eq!(
            serde_json::from_slice::<InstrumentSettings>(&saved).unwrap(),
            instrument
        );
    }
}
