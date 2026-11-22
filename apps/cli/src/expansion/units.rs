use super::*;
use daw_core::{
    EffectChainTarget as Chain, EffectInstance, EffectSettings, EnvelopeInstance,
    InstrumentComponent as Component, InstrumentLayerSettings, InstrumentSettings, LfoInstance,
    Modulation, ModulationParameter, ModulationSourceId,
};

mod controls;
mod hosted;
pub(crate) use hosted::identity_key;

pub(super) fn run(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    match path {
        "effect.catalog.list" => Ok(json!(
            daw_core::BUILTIN_EFFECTS
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>()
        )),
        path if path.starts_with("instrument.") => instrument(path, args, session),
        path if path.starts_with("plugin.instance.") => hosted::run(path, args, session),
        path if path.starts_with("unit.parameter.") => controls::parameters(path, args, session),
        path if path.starts_with("modulation.route.") => controls::modulation(path, args, session),
        _ => effect(path, args, session),
    }
}

fn uuid(text: &str) -> Result<Id> {
    text.parse::<Id>().map_err(error)
}

fn owner(text: &str) -> Result<Id> {
    let (kind, id) = text
        .split_once(':')
        .ok_or_else(|| error("Use track:UUID or subtrack:UUID"))?;
    if !matches!(kind, "track" | "subtrack") {
        return Err(error("Use track:UUID or subtrack:UUID"));
    }
    uuid(id)
}

fn instrument_owner(project: &Project, text: &str) -> Result<Id> {
    let id = owner(text)?;
    project
        .instrument_owners()
        .any(|(track, unit, _)| unit == id && (track == unit) == text.starts_with("track:"))
        .then_some(id)
        .ok_or_else(|| error("Instrument owner not found"))
}

fn chain(project: &Project, text: &str) -> Result<Chain> {
    if text == "master" {
        return Ok(Chain::Master);
    }
    let elements = text.split(':').collect::<Vec<_>>();
    let target = match elements.as_slice() {
        ["track", id] => Chain::Track(uuid(id)?),
        ["layer", id, layer] => Chain::Layer {
            track: uuid(id)?,
            layer: uuid(layer)?,
        },
        ["output", id] => {
            let track = uuid(id)?;
            let layer = project
                .beat(track)
                .ok_or_else(|| error("Beat Output not found"))?
                .output
                .id;
            Chain::Layer { track, layer }
        }
        _ => {
            return Err(error(
                "Use master, track:UUID, layer:OWNER_UUID:LAYER_UUID or output:BEAT_TRACK_UUID",
            ));
        }
    };
    project
        .effect_chain(target)
        .ok_or_else(|| error("Effect chain not found"))?;
    Ok(target)
}

fn effect(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let target = chain(project, raw(args, "owner"))?;
    let (units, compact) = project.effect_chain(target).expect("resolved chain");
    if path == "effect.chain.list" {
        return Ok(json!(units));
    }
    if matches!(path, "effect.chain.move" | "effect.chain.copy") {
        let id = *get::<Id>(args, "unit");
        let destination = chain(project, raw(args, "to-owner"))?;
        let (dest_units, _) = project.effect_chain(destination).expect("resolved chain");
        let index = effect_index(dest_units, raw(args, "before"))?;
        let transfer = project
            .transfer_effects(
                target,
                destination,
                &[id],
                index,
                path == "effect.chain.copy",
            )
            .map_err(error)?;
        prepare_edit(session, transfer.edit)?;
        return Ok(json!({"units": transfer.selected}));
    }
    let mut units = units.to_vec();
    let result = match path {
        "effect.chain.add" => {
            let key = raw(args, "type");
            let unit = daw_core::BUILTIN_EFFECTS
                .iter()
                .find(|entry| entry.key == key)
                .map(|entry| (entry.make)())
                .ok_or_else(|| error("Unknown effect key. See effect.catalog.list"))?;
            let index = effect_index(
                &units,
                opt::<String>(args, "before").map_or("end", String::as_str),
            )?;
            let id = unit.id;
            units.insert(index, unit);
            json!({"unitId": id})
        }
        "effect.chain.remove" => {
            let ids = items::<Id>(args, "unit").copied().collect::<Vec<_>>();
            if ids
                .iter()
                .any(|id| !units.iter().any(|unit| unit.id == *id))
            {
                return Err(error("Effect not found in the selected chain"));
            }
            units.retain(|unit| !ids.contains(&unit.id));
            json!({"deleted": ids})
        }
        "effect.power.set" => {
            let id = *get::<Id>(args, "unit");
            let enabled = *get(args, "enabled");
            let unit = units
                .iter_mut()
                .find(|unit| unit.id == id)
                .ok_or_else(|| error("Effect not found in the selected chain"))?;
            *unit
                .unit
                .enabled_mut()
                .ok_or_else(|| error("Effect does not support power controls"))? = enabled;
            json!({"unitId": id, "enabled": enabled})
        }
        _ => unreachable!("effect route"),
    };
    prepare_edit(session, target.edit(units, compact))?;
    Ok(result)
}

fn effect_index(units: &[EffectInstance], before: &str) -> Result<usize> {
    if before == "end" {
        return Ok(units.len());
    }
    let before = uuid(before)?;
    units
        .iter()
        .position(|unit| unit.id == before)
        .ok_or_else(|| error("Insertion anchor not found in the destination chain"))
}

fn resolve_layer<'a>(
    instrument: &'a InstrumentSettings,
    selector: &tracks::Selector,
) -> Result<&'a InstrumentLayerSettings> {
    select(&instrument.layers, selector, |layer| {
        (layer.id, &layer.name)
    })
}

fn require_replace(args: &ArgMatches) {
    if !args.get_flag("replace") {
        usage("Pass --replace to replace the existing source");
    }
}

fn instrument(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let id = instrument_owner(&session.project, raw(args, "owner"))?;
    let mut settings = session
        .project
        .instrument_snapshot(id)
        .ok_or_else(|| error("Instrument not found"))?;
    let result = match path {
        "instrument.layer.list" => {
            return Ok(json!({
                "layers": settings.layers.iter().map(|layer| json!({
                    "id": layer.id, "name": layer.name, "source": source_kind(layer),
                })).collect::<Vec<_>>(),
                "mainId": settings.main.id, "lfoLaneId": settings.lfo_lane.id, "envelopeLaneId": settings.envelope_lane.id,
            }));
        }
        "instrument.layer.add" => {
            let layer = InstrumentLayerSettings {
                name: raw(args, "name").into(),
                ..Default::default()
            };
            let result = json!({"layerId": layer.id});
            settings.layers.push(layer);
            result
        }
        "instrument.component.move" | "instrument.component.copy" => {
            return transfer_component(path, args, session, id, &settings);
        }
        "instrument.component.remove" => {
            let unit = *get(args, "unit");
            let (source, component) = component(&settings, unit)?;
            let removed = if let Some(bus) = settings.bus_mut(source) {
                bus.remove_components(&[component]).unwrap_or(false)
            } else {
                let layer = settings
                    .layers
                    .iter_mut()
                    .find(|layer| layer.id == source)
                    .expect("resolved layer");
                match component {
                    Component::Oscillator(id) => layer.remove_oscillator(id).is_some(),
                    component => layer.remove_components(&[component]).unwrap_or(false),
                }
            };
            if !removed {
                return Err(error("Component cannot be removed"));
            }
            json!({"deleted": unit})
        }
        _ => {
            let layer_id = resolve_layer(&settings, get(args, "layer"))?.id;
            let index = settings
                .layers
                .iter()
                .position(|layer| layer.id == layer_id)
                .expect("resolved layer");
            let layer = &mut settings.layers[index];
            match path {
                "instrument.layer.rename" => layer.name = raw(args, "name").into(),
                "instrument.layer.remove" => {
                    settings.layers.remove(index);
                }
                "instrument.layer.move" => {
                    let moved = settings.layers.remove(index);
                    let before = raw(args, "before");
                    let target = if before == "end" {
                        settings.layers.len()
                    } else {
                        let selector = tracks::parse_selector(before).map_err(error)?;
                        let before = resolve_layer(&settings, &selector)?.id;
                        settings
                            .layers
                            .iter()
                            .position(|layer| layer.id == before)
                            .expect("resolved anchor")
                    };
                    settings.layers.insert(target, moved);
                }
                "instrument.source.set" => {
                    require_replace(args);
                    layer.clear_sources();
                    if raw(args, "type") == "noise" {
                        layer.noise = Some(Default::default());
                    } else {
                        layer.oscillators.push(Default::default());
                    }
                }
                "instrument.source.sample.load" => {
                    require_replace(args);
                    layer.clear_sources();
                    let mut sampler = daw_core::SamplerSettings::default();
                    if raw(args, "type") == "granulizer" {
                        sampler.granulizer = Some(Default::default());
                    }
                    layer.sampler = Some(sampler);
                    edit(
                        session,
                        Edit::SetInstrument {
                            track_id: id,
                            instrument: Some(settings),
                        },
                    )?;
                    *session =
                        session.with_sampler_file(id, layer_id, get::<PathBuf>(args, "input"))?;
                    let input = session
                        .project
                        .instrument_settings(id)
                        .and_then(|instrument| {
                            instrument
                                .layers
                                .iter()
                                .find(|layer| layer.id == layer_id)
                                .and_then(|layer| layer.sampler.as_ref())
                                .and_then(|sampler| sampler.input.as_ref())
                        });
                    return Ok(json!({"layerId": layer_id, "input": input}));
                }
                "instrument.component.list" => {
                    return Ok(json!({
                        "layerId": layer_id,
                        "units": layer.visible_units(),
                        "oscillatorIds": layer.oscillators.iter().map(|osc| osc.id).collect::<Vec<_>>(),
                        "noiseId": layer.noise.as_ref().map(|noise| noise.id),
                        "samplerId": layer.sampler.as_ref().map(|sampler| sampler.id),
                        "hostedId": layer.hosted.as_ref().and_then(daw_core::HostedSource::settings).map(|unit| unit.id),
                        "amplitudeId": layer.amplitude.present.then(|| module_id(layer.id, Component::Amplitude)),
                    "macrosId": layer.macro_module.present.then(|| module_id(layer.id, Component::Macros)),
                    "addKeys": ["oscillator", "envelope", "lfo", "amplitude", "macros"]
                    }));
                }
                "instrument.component.add" => {
                    let key = raw(args, "type");
                    match key {
                        "oscillator" => {
                            if layer.sampler.is_some()
                                || layer.noise.is_some()
                                || layer.hosted.is_some()
                            {
                                return Err(error(
                                    "An oscillator requires an empty or oscillator layer",
                                ));
                            }
                            layer.oscillators.push(Default::default());
                        }
                        "envelope" => layer.envelopes.push(EnvelopeInstance {
                            present: true,
                            ..Default::default()
                        }),
                        "lfo" => layer.lfos.push(LfoInstance {
                            present: true,
                            ..Default::default()
                        }),
                        "amplitude" if !layer.amplitude.present => layer.amplitude.present = true,
                        "macros" if !layer.macro_module.present => {
                            layer.macro_module.present = true
                        }
                        _ => return Err(error("Component type is unknown or already installed")),
                    }
                }
                _ => unreachable!("instrument route"),
            }
            json!({"layerId": layer_id})
        }
    };
    let mut commands = Vec::new();
    if matches!(
        path,
        "instrument.layer.remove" | "instrument.component.remove"
    ) {
        let original = session
            .project
            .instrument_settings(id)
            .expect("resolved instrument");
        let removed = original
            .modulation_sources()
            .filter(|source| source.present)
            .map(|source| source.id)
            .filter(|source| {
                settings
                    .modulation_source(*source)
                    .is_none_or(|source| !source.present)
            })
            .collect::<std::collections::HashSet<_>>();
        if !removed.is_empty() {
            let target = Chain::Track(id);
            let (mut effects, compact) = session.project.effect_chain(target).map_or_else(
                || (Vec::new(), false),
                |(effects, compact)| (effects.to_vec(), compact),
            );
            controls::remove_provider_routes(&mut settings, &mut effects, &removed);
            if session.project.effect_chain(target).is_some() {
                commands.push(target.edit(effects, compact));
            }
        }
    }
    commands.push(Edit::SetInstrument {
        track_id: id,
        instrument: Some(settings),
    });
    if path == "instrument.layer.rename" {
        edit(session, Edit::Batch(commands))?;
    } else {
        prepare_edit(session, Edit::Batch(commands))?;
    }
    Ok(result)
}

fn source_kind(layer: &InstrumentLayerSettings) -> &'static str {
    if layer.hosted.is_some() {
        "hosted"
    } else if layer.noise.is_some() {
        "noise"
    } else if let Some(sampler) = &layer.sampler {
        if sampler.granulizer.is_some() {
            "granulizer"
        } else {
            "sampler"
        }
    } else if !layer.oscillators.is_empty() {
        "oscillator"
    } else {
        "empty"
    }
}

fn component(settings: &InstrumentSettings, unit: Id) -> Result<(Id, Component)> {
    let mut matches = Vec::new();
    for layer in &settings.layers {
        if layer.oscillators.iter().any(|osc| osc.id == unit) {
            matches.push((layer.id, Component::Oscillator(unit)));
        }
        if layer.noise.as_ref().is_some_and(|noise| noise.id == unit) {
            matches.push((layer.id, Component::Noise));
        }
        if layer
            .sampler
            .as_ref()
            .is_some_and(|sampler| sampler.id == unit)
        {
            matches.push((layer.id, Component::Sampler));
        }
        if layer
            .hosted
            .as_ref()
            .and_then(daw_core::HostedSource::settings)
            .is_some_and(|plugin| plugin.id == unit)
        {
            matches.push((layer.id, Component::Hosted));
        }
        match_modules(
            &mut matches,
            layer.id,
            layer.amplitude.present,
            layer.macro_module.present,
            unit,
        );
        collect_components(&mut matches, layer.id, layer.visible_components(), unit);
    }
    for bus in settings.buses() {
        match_modules(&mut matches, bus.id, false, bus.macro_module.present, unit);
        collect_components(&mut matches, bus.id, bus.visible_components(), unit);
    }
    match matches.as_slice() {
        [only] => Ok(*only),
        [] => Err(error("Component not found")),
        _ => Err(error("Component ID is ambiguous within this instrument")),
    }
}

// These two modules have one slot per owner. Derive its address from the saved owner.
fn module_id(owner: Id, module: Component) -> Id {
    let mask = match module {
        Component::Amplitude => 0x223f_e1b0_b86a_08ee_b803_94b2_f806_b28c,
        Component::Macros => 0x4338_3da7_dca6_0763_a46a_8b9c_0663_42cb,
        _ => unreachable!("only single-slot modules have derived IDs"),
    };
    Id::from_u128(owner.as_u128() ^ mask)
}

fn match_modules(
    matches: &mut Vec<(Id, Component)>,
    owner: Id,
    amplitude: bool,
    macros: bool,
    unit: Id,
) {
    for (present, module) in [
        (amplitude, Component::Amplitude),
        (macros, Component::Macros),
    ] {
        if present && module_id(owner, module) == unit {
            matches.push((owner, module));
        }
    }
}

fn collect_components(
    matches: &mut Vec<(Id, Component)>,
    owner: Id,
    units: Vec<Component>,
    unit: Id,
) {
    matches.extend(
        units
            .into_iter()
            .filter(|component| match component {
                Component::Envelope(id) | Component::Lfo(id) => *id == unit,
                _ => false,
            })
            .map(|component| (owner, component)),
    );
}

fn transfer_component(
    path: &str,
    args: &ArgMatches,
    session: &mut Session,
    owner: Id,
    settings: &InstrumentSettings,
) -> Result<Value> {
    let id = *get::<Id>(args, "unit");
    let (source, component) = component(settings, id)?;
    let target = resolve_layer(settings, get(args, "to-layer"))?;
    let before = raw(args, "before");
    let destination_index = if before == "end" {
        None
    } else {
        let anchor = uuid(before)?;
        let index = if matches!(component, Component::Oscillator(_)) {
            target
                .oscillators
                .iter()
                .filter(|osc| path == "instrument.component.copy" || osc.id != id)
                .position(|osc| osc.id == anchor)
        } else {
            target.visible_units().into_iter()
                .filter(|entry| path == "instrument.component.copy" || *entry != component)
                .position(|entry| matches!(entry, Component::Envelope(id) | Component::Lfo(id) | Component::Effect(id) if id == anchor))
        };
        Some(index.ok_or_else(|| error("Destination component anchor not found"))?)
    };
    let destination = target.id;
    prepare_edit(
        session,
        Edit::TransferInstrumentComponent {
            track_id: owner,
            source,
            destination,
            destination_index,
            component,
            duplicate: path == "instrument.component.copy",
        },
    )?;
    Ok(json!({"layerId": destination}))
}
