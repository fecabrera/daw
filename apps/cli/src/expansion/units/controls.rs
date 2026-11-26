use super::*;

type Controls = Vec<(String, Vec<String>, Value)>;

pub(super) fn parameters(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let text = raw(args, "owner");
    let id = *get(args, "unit");
    if let Ok(target) = chain(&session.project, text) {
        let (effects, compact) = session
            .project
            .effect_chain(target)
            .expect("resolved chain");
        if let Some(index) = effects.iter().position(|effect| effect.id == id) {
            let mut effects = effects.to_vec();
            let unit = &mut effects[index].unit;
            if matches!(
                unit,
                EffectSettings::Hosted(_) | EffectSettings::Missing { .. }
            ) {
                return Err(error("Select a built-in unit"));
            }
            let result = scalar(unit, path, args, &["state"])?;
            if path.ends_with(".set") {
                prepare_edit(session, target.edit(effects, compact))?;
            }
            return Ok(result);
        }
    }
    let project = &session.project;
    let mut parts = text.split(':');
    let (owner, required_layer) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("layer"), Some(track), Some(layer), None) => {
            let (track, layer) = (uuid(track)?, uuid(layer)?);
            let owner = project.beat(track).filter(|beat| beat.output.id == layer);
            (owner.map_or(track, |beat| beat.output.id), Some(layer))
        }
        (Some("output"), Some(track), None, None) => {
            let beat = project
                .beat(uuid(track)?)
                .ok_or_else(|| error("Beat Output not found"))?;
            (beat.output.id, Some(beat.output.id))
        }
        _ => (instrument_owner(project, text)?, None),
    };
    let mut instrument = session
        .project
        .instrument_snapshot(owner)
        .ok_or_else(|| error("Instrument not found"))?;
    let (container, kind) = component(&instrument, id)?;
    if required_layer.is_some_and(|layer| layer != container) {
        return Err(error("Unit not found in the selected layer"));
    }
    let result = if let Some(bus) = instrument.bus_mut(container) {
        match kind {
            Component::Lfo(id) => scalar(
                bus.lfos.iter_mut().find(|unit| unit.id == id).unwrap(),
                path,
                args,
                &[],
            ),
            Component::Envelope(id) => scalar(
                bus.envelopes.iter_mut().find(|unit| unit.id == id).unwrap(),
                path,
                args,
                &[],
            ),
            Component::Macros => scalar(&mut bus.macro_module, path, args, &[]),
            _ => Err(error("This component has no editable parameters")),
        }
    } else {
        let layer = instrument
            .layers
            .iter_mut()
            .find(|layer| layer.id == container)
            .unwrap();
        match kind {
            Component::Oscillator(id) => scalar(
                layer
                    .oscillators
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap(),
                path,
                args,
                &[],
            ),
            Component::Lfo(id) => scalar(
                layer.lfos.iter_mut().find(|unit| unit.id == id).unwrap(),
                path,
                args,
                &[],
            ),
            Component::Envelope(id) => scalar(
                layer
                    .envelopes
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap(),
                path,
                args,
                &[],
            ),
            Component::Noise => scalar(layer.noise.as_mut().unwrap(), path, args, &[]),
            Component::Sampler => scalar(layer.sampler.as_mut().unwrap(), path, args, &[]),
            Component::Amplitude => scalar(&mut layer.amplitude, path, args, &[]),
            Component::Macros => scalar(&mut layer.macro_module, path, args, &[]),
            _ => Err(error("This component has no editable parameters")),
        }
    }?;
    if path.ends_with(".set") {
        prepare_edit(
            session,
            Edit::SetInstrument {
                track_id: owner,
                instrument: Some(instrument),
            },
        )?;
    }
    Ok(result)
}

fn scalar<T>(unit: &mut T, route: &str, args: &ArgMatches, prefix: &[&str]) -> Result<Value>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let mut saved = serde_json::to_value(&*unit).map_err(error)?;
    let mut controls = Vec::new();
    let mut base = prefix
        .iter()
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    let root = at(&mut saved, &base).ok_or_else(|| error("Unit controls are missing"))?;
    fields(root, &mut Vec::new(), &mut base, &mut controls);
    if route.ends_with(".list") {
        return Ok(json!(
            controls
                .into_iter()
                .map(|(key, _, value)| {
                    let unit = if key.ends_with("_db") {
                        Some("dB")
                    } else if key.ends_with("_hz") {
                        Some("Hz")
                    } else if key.ends_with("_ms") {
                        Some("ms")
                    } else if key.ends_with("_frames") {
                        Some("frames")
                    } else {
                        None
                    };
                    let kind = if value.is_boolean() {
                        "boolean"
                    } else if value.is_number() {
                        "number"
                    } else {
                        "string"
                    };
                    json!({"key": key, "value": value, "type": kind, "unit": unit})
                })
                .collect::<Vec<_>>()
        ));
    }
    let key = raw(args, "parameter");
    let (_, address, previous) = controls
        .into_iter()
        .find(|(name, _, _)| name == key)
        .ok_or_else(|| error("Unknown editable parameter. See unit.parameter.list"))?;
    let input = raw(args, "value");
    let value = if previous.is_string() {
        json!(input)
    } else {
        let value: Value = serde_json::from_str(input).map_err(error)?;
        if previous.is_boolean() != value.is_boolean() || previous.is_number() != value.is_number()
        {
            return Err(error("Use the parameter type shown by unit.parameter.list"));
        }
        value
    };
    *at(&mut saved, &address).expect("listed parameter") = value.clone();
    let updated = serde_json::from_value::<T>(saved).map_err(error)?;
    let mut serialized = serde_json::to_value(&updated).map_err(error)?;
    let actual = at(&mut serialized, &address)
        .ok_or_else(|| error("Parameter disappeared after validation"))?;
    if value.is_string() && *actual != value {
        return Err(error("Unknown parameter value"));
    }
    *unit = updated;
    Ok(json!({"key": key, "value": actual}))
}

fn at<'a>(mut value: &'a mut Value, address: &[String]) -> Option<&'a mut Value> {
    for part in address {
        value = if value.is_array() {
            value.get_mut(part.parse::<usize>().ok()?)?
        } else {
            value.get_mut(part)?
        };
    }
    Some(value)
}

fn fields(value: &Value, key: &mut Vec<String>, address: &mut Vec<String>, output: &mut Controls) {
    match value {
        Value::Object(object) => {
            for (part, value) in object {
                if matches!(
                    part.as_str(),
                    "id" | "asset_id"
                        | "version"
                        | "type_id"
                        | "enabled"
                        | "collapsed"
                        | "present"
                        | "modulation_open"
                        | "modulations"
                        | "input"
                        | "presentation"
                        | "name"
                ) {
                    continue;
                }
                key.push(part.clone());
                address.push(part.clone());
                fields(value, key, address, output);
                key.pop();
                address.pop();
            }
        }
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                if let Some(id) = entry.get("id").and_then(Value::as_str) {
                    key.push(id.into());
                    address.push(index.to_string());
                    fields(entry, key, address, output);
                    key.pop();
                    address.pop();
                }
            }
        }
        Value::Null => {}
        _ => output.push((key.join("."), address.clone(), value.clone())),
    }
}

type Destinations<'a> = Vec<(Id, &'a mut Vec<Modulation>)>;

pub(super) fn modulation(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let text = raw(args, "owner");
    let target = chain(project, text)?;
    let owner = match target {
        Chain::Track(owner) => Some(project.beat(owner).map_or(owner, |beat| beat.output.id)),
        Chain::Layer { track, .. } => Some(track),
        Chain::Master => None,
    };
    let source = if path == "modulation.route.set" {
        let (layer, provider) = raw(args, "source")
            .split_once(':')
            .ok_or_else(|| error("Use LAYER_UUID:PROVIDER_UUID for the source"))?;
        let source = ModulationSourceId {
            layer: uuid(layer)?,
            unit: uuid(provider)?,
        };
        if !owner
            .and_then(|owner| project.instrument_snapshot(owner))
            .is_some_and(|settings| {
                settings
                    .modulation_source(source)
                    .is_some_and(|source| source.present)
            })
        {
            return Err(error(
                "Modulation provider not found in the owner instrument",
            ));
        }
        Some(source)
    } else {
        None
    };
    let mut instrument = owner
        .filter(|_| matches!(target, Chain::Track(_)))
        .and_then(|owner| project.instrument_snapshot(owner));
    let (effects, compact) = project.effect_chain(target).expect("resolved chain");
    let mut effects = effects.to_vec();
    let mut banks = Destinations::new();
    effect_banks(&mut effects, &mut banks);
    if let Some(instrument) = &mut instrument {
        instrument_banks(instrument, &mut banks);
    }
    if path == "modulation.route.list" {
        let rows = banks
            .iter()
            .flat_map(|(unit, routes)| {
                routes.iter().map(move |route| {
                    let parameter = serde_json::to_value(route.parameter).expect("enum string");
                    let parameter = parameter.as_str().expect("enum string");
                    let mut key = format!("{unit}:{parameter}");
                    if let Some(target) = route.target {
                        key = format!("{key}:{target}");
                    }
                    json!({"destination": key, "amountUnit": route.parameter.unit(),
                "amountLimit": route.parameter.limit(), "route": route})
                })
            })
            .collect::<Vec<_>>();
        return Ok(json!(rows));
    }
    let route_id = opt::<Id>(args, "id").copied();
    if let Some(id) = route_id {
        let mut matching = banks
            .iter_mut()
            .filter(|(_, routes)| routes.iter().any(|route| route.id == id));
        let Some((_, routes)) = matching.next() else {
            return Err(error("Modulation route not found"));
        };
        if matching.next().is_some() {
            return Err(error("Modulation route ID is ambiguous"));
        }
        routes.retain(|route| route.id != id);
    }
    let result = if let Some(source) = source {
        let mut parts = raw(args, "destination").split(':');
        let unit = uuid(parts.next().unwrap_or_default())?;
        let parameter: ModulationParameter = serde_json::from_value(json!(parts.next()))
            .map_err(|_| error("Use UNIT_UUID:PARAMETER[:TARGET_UUID] for the destination"))?;
        let target = parts.next().map(uuid).transpose()?;
        if parts.next().is_some() {
            return Err(error(
                "Use UNIT_UUID:PARAMETER[:TARGET_UUID] for the destination",
            ));
        }
        let mut matching = banks.iter_mut().filter(|(id, _)| *id == unit);
        let Some((_, routes)) = matching.next() else {
            return Err(error("Unit has no modulation destinations"));
        };
        if matching.next().is_some() {
            return Err(error("Modulation destination ID is ambiguous"));
        }
        let mut route = Modulation::new(parameter);
        route.id = route_id.unwrap_or(route.id);
        route.source = Some(source);
        route.target = target;
        route.amount = *get(args, "amount");
        routes.push(route);
        json!({"route": route})
    } else {
        json!({"deleted": route_id})
    };
    let mut edits = vec![target.edit(effects, compact)];
    if let (Some(track_id), Some(instrument)) = (owner, instrument) {
        edits.push(Edit::SetInstrument {
            track_id,
            instrument: Some(instrument),
        });
    }
    prepare_edit(session, Edit::Batch(edits))?;
    Ok(result)
}

pub(super) fn remove_provider_routes(
    instrument: &mut InstrumentSettings,
    effects: &mut [EffectInstance],
    removed: &std::collections::HashSet<ModulationSourceId>,
) {
    let mut banks = Destinations::new();
    instrument_banks(instrument, &mut banks);
    effect_banks(effects, &mut banks);
    for (_, routes) in banks {
        routes.retain(|route| route.source.is_none_or(|source| !removed.contains(&source)));
    }
}

fn effect_banks<'a>(effects: &'a mut [EffectInstance], banks: &mut Destinations<'a>) {
    banks.extend(
        effects
            .iter_mut()
            .filter_map(|effect| Some((effect.id, effect.unit.modulation_routes_mut()?))),
    );
}

fn provider_banks<'a>(
    envelopes: &'a mut [EnvelopeInstance],
    lfos: &'a mut [LfoInstance],
    banks: &mut Destinations<'a>,
) {
    banks.extend(
        envelopes
            .iter_mut()
            .filter(|unit| unit.present)
            .map(|unit| (unit.id, &mut unit.modulations)),
    );
    banks.extend(
        lfos.iter_mut()
            .filter(|unit| unit.present)
            .map(|unit| (unit.id, &mut unit.modulations)),
    );
}

fn instrument_banks<'a>(instrument: &'a mut InstrumentSettings, banks: &mut Destinations<'a>) {
    for layer in &mut instrument.layers {
        if !layer.oscillators.is_empty() {
            banks.push((layer.id, &mut layer.oscillator_modulations));
        }
        if let Some(sampler) = &mut layer.sampler {
            banks.push((sampler.id, &mut sampler.modulations));
        }
        if let Some(noise) = &mut layer.noise {
            banks.push((noise.id, &mut noise.modulations));
        }
        if let Some(hosted) = layer
            .hosted
            .as_mut()
            .and_then(daw_core::HostedSource::settings_mut)
        {
            banks.push((hosted.id, &mut hosted.modulations));
        }
        if layer.amplitude.present {
            banks.push((
                module_id(layer.id, Component::Amplitude),
                &mut layer.amplitude.modulations,
            ));
        }
        if layer.macro_module.present {
            banks.push((
                module_id(layer.id, Component::Macros),
                &mut layer.macro_module.modulations,
            ));
        }
        effect_banks(&mut layer.effects, banks);
        provider_banks(&mut layer.envelopes, &mut layer.lfos, banks);
    }
    for bus in [
        &mut instrument.main,
        &mut instrument.lfo_lane,
        &mut instrument.envelope_lane,
    ] {
        if bus.macro_module.present {
            banks.push((
                module_id(bus.id, Component::Macros),
                &mut bus.macro_module.modulations,
            ));
        }
        effect_banks(&mut bus.effects, banks);
        provider_banks(&mut bus.envelopes, &mut bus.lfos, banks);
    }
}
