//! Copy a layer's owned identities and retarget its internal modulation graph.
use crate::{InstrumentComponent, InstrumentLayerSettings, Modulation};
use std::collections::HashMap;
use uuid::Uuid;

impl InstrumentLayerSettings {
    /// Copy all retained settings. External providers and immutable sample inputs stay shared.
    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = Uuid::new_v4();
        let mut providers = HashMap::new();
        for id in copy
            .envelopes
            .iter_mut()
            .map(|e| &mut e.id)
            .chain(copy.lfos.iter_mut().map(|l| &mut l.id))
            .chain(copy.macro_module.controls.iter_mut().map(|c| &mut c.id))
        {
            let old = *id;
            *id = Uuid::new_v4();
            providers.insert(old, *id);
        }
        // Oscillator destination IDs and provider IDs have separate scopes.
        let mut oscillators = HashMap::new();
        for oscillator in &mut copy.oscillators {
            let old = oscillator.id;
            oscillator.id = Uuid::new_v4();
            oscillators.insert(old, oscillator.id);
        }
        for component in &mut copy.component_order {
            match component {
                InstrumentComponent::Envelope(id) | InstrumentComponent::Lfo(id) => {
                    if let Some(new) = providers.get(id) {
                        *id = *new;
                    }
                }
                _ => {}
            }
        }
        if let Some(sampler) = &mut copy.sampler {
            sampler.id = Uuid::new_v4();
        }
        for route in &mut copy.oscillator_modulations {
            route.target = route.target.map(|id| *oscillators.get(&id).unwrap_or(&id));
        }
        for route in &mut copy.macro_module.modulations {
            route.target = route.target.map(|id| *providers.get(&id).unwrap_or(&id));
        }
        // Effects use their existing copy contract, including fresh route identities.
        copy.effects = self
            .effects
            .iter()
            .map(crate::EffectInstance::duplicate)
            .collect();
        for route in routes(&mut copy, false) {
            route.id = Uuid::new_v4();
        }
        let owner = copy.id;
        for route in copy.modulation_routes_mut() {
            if let Some(source) = &mut route.source
                && source.layer == self.id
                && let Some(unit) = providers.get(&source.unit)
            {
                source.layer = owner;
                source.unit = *unit;
            }
        }
        copy
    }

    /// All layer-owned routes, including retained components and its serial effect chain.
    pub fn modulation_routes_mut(&mut self) -> impl Iterator<Item = &mut Modulation> {
        routes(self, true)
    }
}

fn routes(
    layer: &mut InstrumentLayerSettings,
    include_effects: bool,
) -> impl Iterator<Item = &mut Modulation> {
    layer
        .oscillator_modulations
        .iter_mut()
        .chain(layer.sampler.iter_mut().flat_map(|s| &mut s.modulations))
        .chain(&mut layer.amplitude.modulations)
        .chain(&mut layer.macro_module.modulations)
        .chain(layer.envelopes.iter_mut().flat_map(|e| &mut e.modulations))
        .chain(layer.lfos.iter_mut().flat_map(|l| &mut l.modulations))
        .chain(
            layer
                .effects
                .iter_mut()
                .filter(move |_| include_effects)
                .flat_map(|e| e.unit.modulations_mut()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use serde_json::Value;

    fn route(
        parameter: ModulationParameter,
        source: ModulationSourceId,
        target: Option<Uuid>,
    ) -> Modulation {
        Modulation {
            source: Some(source),
            target,
            amount: 0.25,
            ..Modulation::new(parameter)
        }
    }
    fn owned_ids(old: &Value, new: &Value, reverse: &mut HashMap<String, String>) {
        match (old, new) {
            (Value::Object(a), Value::Object(b)) => {
                for (key, value) in a {
                    if key == "id" {
                        assert_ne!(value, &b[key]);
                        reverse.insert(
                            b[key].as_str().unwrap().into(),
                            value.as_str().unwrap().into(),
                        );
                    } else {
                        owned_ids(value, &b[key], reverse);
                    }
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    owned_ids(a, b, reverse);
                }
            }
            _ => {}
        }
    }
    fn restore_ids(value: &mut Value, reverse: &HashMap<String, String>) {
        match value {
            Value::String(id) => {
                if let Some(old) = reverse.get(id) {
                    *id = old.clone();
                }
            }
            Value::Array(a) => {
                for v in a {
                    restore_ids(v, reverse);
                }
            }
            Value::Object(a) => {
                for v in a.values_mut() {
                    restore_ids(v, reverse);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn layer_copy_preserves_all_settings_and_retargets_only_its_owned_graph() {
        let mut instrument = InstrumentSettings::default();
        instrument.main.macro_module.add();
        let external = ModulationSourceId {
            layer: instrument.main.id,
            unit: instrument.main.macro_module.controls[0].id,
        };
        let layer = &mut instrument.layers[0];
        layer.name = "Bass".into();
        layer.collapsed = true;
        layer.muted = true;
        layer.soloed = true;
        layer.gain_db = -6.0;
        layer.pan = -0.3;
        layer.oscillators = vec![OscillatorSettings::default(), OscillatorSettings::default()];
        layer.oscillators[1].muted = true;
        layer.envelopes[0].present = true;
        layer.envelopes[0].enabled = false;
        layer.envelopes[0].modulation_open = true;
        layer.macro_module.add();
        layer.macro_module.add();
        layer.macro_module.controls[0].name = Some("Tone".into());
        layer.lfos.push(LfoInstance::default());
        let source = |unit| ModulationSourceId {
            layer: layer.id,
            unit,
        };
        let envelope = source(layer.envelopes[0].id);
        let lfo = source(layer.lfos[0].id);
        let macro_a = source(layer.macro_module.controls[0].id);
        let macro_b = source(layer.macro_module.controls[1].id);
        layer.envelopes[0].modulations.push(route(
            ModulationParameter::EnvelopeAttack,
            external,
            None,
        ));
        layer.lfos[0]
            .modulations
            .push(route(ModulationParameter::LfoDepth, macro_a, None));
        layer.macro_module.modulations = vec![
            route(
                ModulationParameter::MacroValue,
                external,
                Some(macro_a.unit),
            ),
            route(ModulationParameter::MacroValue, lfo, Some(macro_b.unit)),
        ];
        layer.oscillator_modulations = vec![
            route(
                ModulationParameter::Pitch,
                envelope,
                Some(layer.oscillators[0].id),
            ),
            route(
                ModulationParameter::OscillatorGain,
                macro_b,
                Some(layer.oscillators[1].id),
            ),
        ];
        layer.amplitude.present = true;
        layer.amplitude.modulations = vec![
            route(ModulationParameter::Amplitude, envelope, None),
            route(ModulationParameter::Amplitude, external, None),
        ];
        let mut effect = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings
            .modulations
            .push(route(ModulationParameter::Frequency, lfo, None));
        layer.effects = vec![effect, EffectInstance::limiter()];
        layer.component_order = vec![
            InstrumentComponent::Oscillators,
            InstrumentComponent::Macros,
            InstrumentComponent::Lfo(lfo.unit),
            InstrumentComponent::Envelope(envelope.unit),
            InstrumentComponent::Amplitude,
        ];
        let original = layer.clone();
        assert!(instrument.valid());
        let mut copy = original.duplicate();
        let mut before = original.clone();
        let copied_owner = copy.id;
        for (a, b) in before
            .modulation_routes_mut()
            .zip(copy.modulation_routes_mut())
        {
            assert_ne!(a.id, b.id);
            if a.source.is_some_and(|s| s.layer == original.id) {
                assert_eq!(b.source.unwrap().layer, copied_owner);
                assert_ne!(a.source.unwrap().unit, b.source.unwrap().unit);
            } else {
                assert_eq!(a.source, b.source);
            }
            if a.target.is_some() {
                assert_ne!(a.target, b.target);
            }
        }
        let old = serde_json::to_value(&original).unwrap();
        let mut copied = serde_json::to_value(&copy).unwrap();
        let mut reverse = HashMap::new();
        owned_ids(&old, &copied, &mut reverse);
        restore_ids(&mut copied, &reverse);
        assert_eq!(
            copied, old,
            "All values, states, order, and references must survive the copy"
        );
        instrument.layers.push(copy);
        assert!(instrument.valid());
        let saved = serde_json::to_vec(&instrument).unwrap();
        assert_eq!(
            serde_json::from_slice::<InstrumentSettings>(&saved).unwrap(),
            instrument
        );
        assert_eq!(instrument.layers[0], original);
    }

    #[test]
    fn sampler_copies_share_input_and_keep_destination_and_provider_scopes_separate() {
        let mut layer = InstrumentLayerSettings::default();
        layer.envelopes[0].present = true;
        let provider = layer.envelopes[0].id;
        layer.oscillators.push(OscillatorSettings {
            id: provider,
            ..Default::default()
        });
        layer.oscillator_modulations.push(route(
            ModulationParameter::Pitch,
            ModulationSourceId {
                layer: layer.id,
                unit: provider,
            },
            Some(provider),
        ));
        let copy = layer.duplicate();
        assert_ne!(copy.oscillators[0].id, copy.envelopes[0].id);
        assert_eq!(
            copy.oscillator_modulations[0].target,
            Some(copy.oscillators[0].id)
        );
        assert_eq!(
            copy.oscillator_modulations[0].source.unwrap().unit,
            copy.envelopes[0].id
        );
        layer.oscillators.clear();
        layer.oscillator_modulations.clear();
        layer.sampler = Some(SamplerSettings {
            input: Some(SamplerInput {
                asset_id: Uuid::new_v4(),
                name: "Sample".into(),
                source_offset_frame: 20,
                length_frames: 100,
                repeat: None,
                stretch: None,
            }),
            start_frame: 10,
            end_frame: 90,
            modulations: vec![route(
                ModulationParameter::SamplerGain,
                ModulationSourceId {
                    layer: layer.id,
                    unit: provider,
                },
                None,
            )],
            ..Default::default()
        });
        let copy = layer.duplicate();
        let a = layer.sampler.as_ref().unwrap();
        let b = copy.sampler.as_ref().unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.input, b.input);
        assert_eq!((a.start_frame, a.end_frame), (b.start_frame, b.end_frame));
        assert_eq!(b.modulations[0].source.unwrap().layer, copy.id);
        assert_eq!(b.modulations[0].source.unwrap().unit, copy.envelopes[0].id);
        assert!(copy.valid());
    }
}
