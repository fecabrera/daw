use super::*;
use daw_core::{
    HostedAudioInput, HostedInputId, HostedRole, HostedSettings, HostedSource, PluginFormat,
    PluginIdentity,
};
use daw_plugin_host::{HostService, PluginRegistry, StateBank};

pub(super) fn run(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let text = opt::<String>(args, "owner").map(String::as_str);
    if path == "plugin.instance.list" {
        if let Some(text) = text {
            return Ok(json!(owned(&session.project, text)?));
        }
        return Ok(json!(
            session
                .project
                .hosted_units()
                .into_iter()
                .map(|(track, unit)| { json!({"trackId": track, "unit": unit}) })
                .collect::<Vec<_>>()
        ));
    }
    let text = text.expect("required owner");
    if path == "plugin.instance.add" {
        return install(session, text, raw(args, "plugin-id"));
    }
    let id = *get(args, "unit");
    let mut unit = owned(&session.project, text)?
        .into_iter()
        .find(|unit| unit.id == id)
        .ok_or_else(|| error("Hosted unit not found in the selected owner"))?
        .clone();
    if path == "plugin.instance.remove" {
        if let Some((id, layer_id)) = source_owner(text)? {
            let mut instrument = session
                .project
                .instrument_snapshot(id)
                .ok_or_else(|| error("Instrument not found"))?;
            let layer = instrument
                .layers
                .iter_mut()
                .find(|layer| layer.id == layer_id)
                .ok_or_else(|| error("Layer not found"))?;
            layer.clear_sources();
            prepare_edit(
                session,
                Edit::SetInstrument {
                    track_id: id,
                    instrument: Some(instrument),
                },
            )?;
        } else {
            let target = chain(&session.project, text)?;
            let (effects, compact) = session
                .project
                .effect_chain(target)
                .expect("resolved chain");
            let effects = effects.iter().filter(|effect| !matches!(&effect.unit, EffectSettings::Hosted(hosted) if hosted.id == id)).cloned().collect();
            prepare_edit(session, target.edit(effects, compact))?;
        }
        let states = session.project.hosted_units();
        session.plugin_states.retain(
            states
                .into_iter()
                .filter_map(|(_, unit)| unit.state.as_ref()),
        );
        return Ok(json!({"deleted": id}));
    }
    let source = opt(args, "source-track")
        .map(|selector| track(&session.project, selector))
        .transpose()?;
    if path == "plugin.instance.sidechain.set" && source.is_none() == !args.get_flag("clear") {
        usage("Choose either --source-track or --clear");
    }

    if path == "plugin.instance.parameter.set" {
        let mut host = host()?;
        let native = host
            .control_instance(&unit, &session.plugin_states)
            .map_err(error)?;
        let key = raw(args, "parameter");
        let parameter = unit
            .parameters
            .iter_mut()
            .find(|binding| {
                binding.target.to_string() == key || binding.definition.id.to_string() == key
            })
            .ok_or_else(|| error("Parameter not found. See plugin.instance.list"))?;
        if parameter.definition.read_only {
            return Err(error("Plugin parameter is read-only"));
        }
        let value = raw(args, "value").parse::<f64>().map_err(error)?;
        let (min, max) = parameter.definition.domain.bounds();
        if !(min..=max).contains(&value) {
            return Err(error(format!("Parameter range is {min} to {max}")));
        }
        if !host.native.metadata(native).map_err(error)?.state_supported {
            return Err(error(
                "The plugin cannot save this parameter in restorable state",
            ));
        }
        let rev = unit
            .edit_revision
            .checked_add(1)
            .ok_or_else(|| error("Plugin revision limit reached"))?;
        host.native
            .set_parameter(native, parameter.definition.id, value)
            .map_err(error)?;
        // State capture flushes queued parameter changes before the native read.
        let state = host.native.save_state(native).map_err(error)?;
        parameter.base_value = host
            .native
            .parameter_value(native, parameter.definition.id)
            .map_err(error)?;
        parameter.edit_revision = rev;
        unit.edit_revision = rev;
        let mut states = StateBank::default();
        unit.state = Some(states.capture(state, rev).map_err(error)?);
        session.plugin_states.merge(&states);
    } else if let Some(source_track) = source {
        let mut host = host()?;
        let native = host
            .control_instance(&unit, &session.plugin_states)
            .map_err(error)?;
        let metadata = host.native.metadata(native).map_err(error)?;
        let inputs = metadata
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, port)| unit.plugin.role == HostedRole::Instrument || !port.main)
            .collect::<Vec<_>>();
        let [(index, port)] = inputs.as_slice() else {
            return Err(error(
                "This command requires exactly one available sidechain input",
            ));
        };
        let port = match unit.plugin.identity.format() {
            PluginFormat::Clap => HostedInputId::Clap { id: port.id },
            PluginFormat::Vst3 => HostedInputId::Vst3 {
                index: *index as u32,
                layout: metadata.audio_layout_signature(),
            },
        };
        let mut input = unit
            .audio_inputs
            .iter()
            .find(|entry| entry.port == port)
            .cloned()
            .unwrap_or(HostedAudioInput {
                id: Id::new_v4(),
                source_track,
                port,
                tap: Default::default(),
                channels: Default::default(),
                gain_db: 0.0,
                enabled: true,
            });
        input.source_track = source_track;
        input.enabled = true;
        unit.audio_inputs = vec![input];
    } else {
        unit.audio_inputs.clear();
    }
    let result = json!(unit);
    prepare_edit(
        session,
        Edit::SetHosted {
            unit_id: id,
            settings: Box::new(unit),
        },
    )?;
    Ok(result)
}

fn source_owner(text: &str) -> Result<Option<(Id, Id)>> {
    let Some(text) = text.strip_prefix("source:") else {
        return Ok(None);
    };
    let (owner, layer) = text
        .split_once(':')
        .ok_or_else(|| error("Use source:OWNER_UUID:LAYER_UUID"))?;
    Ok(Some((uuid(owner)?, uuid(layer)?)))
}

fn owned<'a>(project: &'a Project, text: &str) -> Result<Vec<&'a HostedSettings>> {
    if let Some((owner, layer)) = source_owner(text)? {
        let layer = project
            .instrument_settings(owner)
            .and_then(|instrument| instrument.layers.iter().find(|entry| entry.id == layer))
            .ok_or_else(|| error("Instrument layer not found"))?;
        return Ok(layer
            .hosted
            .as_ref()
            .and_then(HostedSource::settings)
            .into_iter()
            .collect());
    }
    let target = chain(project, text)?;
    let (units, _) = project.effect_chain(target).expect("resolved chain");
    Ok(units
        .iter()
        .filter_map(|unit| match &unit.unit {
            EffectSettings::Hosted(settings) => Some(settings),
            _ => None,
        })
        .collect())
}

fn registry() -> Result<PluginRegistry> {
    let path = daw_plugin_host::default_registry_path().map_err(error)?;
    PluginRegistry::load(&path).map_err(error)
}

fn host() -> Result<HostService> {
    Ok(HostService::new(registry()?).0)
}

fn install(session: &mut Session, owner: &str, key: &str) -> Result<Value> {
    let source = source_owner(owner)?;
    let target = if let Some((id, layer)) = source {
        let layer = session
            .project
            .instrument_settings(id)
            .and_then(|settings| settings.layers.iter().find(|entry| entry.id == layer))
            .ok_or_else(|| error("Instrument layer not found"))?;
        if source_kind(layer) != "empty" {
            return Err(error(
                "Installing a hosted source requires an empty instrument layer",
            ));
        }
        None
    } else {
        Some(chain(&session.project, owner)?)
    };
    let registry = registry()?;
    let identity = registry
        .entries
        .iter()
        .filter(|entry| entry.present)
        .flat_map(|entry| &entry.plugins)
        .find(|plugin| {
            identity_key(&plugin.identity) == key && registry.resolve(&plugin.identity).is_some()
        })
        .ok_or_else(|| error("Plugin not available. See plugin.catalog.list"))?
        .identity
        .clone();
    let (_, plugin) = registry.resolve(&identity).expect("resolved plugin");
    let role = if source.is_some() {
        HostedRole::Instrument
    } else {
        HostedRole::Effect
    };
    if plugin.role != role {
        return Err(error("Plugin role is incompatible with the owner"));
    }
    let rate = session.project.sample_rate_hz;
    let frames = session.project.buffer_size_frames;
    let (mut service, client) = HostService::new(registry);
    let installed = service.run(move || -> Result<_> {
        client
            .install(identity, rate, frames)
            .map_err(error)?
            .recv()
            .map_err(error)?
            .map_err(error)
    })?;
    let result = json!({"unit": installed.settings});
    let command = if let Some((id, layer)) = source {
        let mut settings = session
            .project
            .instrument_snapshot(id)
            .expect("resolved instrument");
        settings
            .layers
            .iter_mut()
            .find(|entry| entry.id == layer)
            .expect("resolved layer")
            .hosted = Some(HostedSource::Known(installed.settings));
        Edit::SetInstrument {
            track_id: id,
            instrument: Some(settings),
        }
    } else {
        let target = target.expect("effect target");
        let (units, compact) = session
            .project
            .effect_chain(target)
            .expect("resolved chain");
        let mut units = units.to_vec();
        units.push(EffectInstance {
            id: Id::new_v4(),
            unit: EffectSettings::Hosted(installed.settings),
        });
        target.edit(units, compact)
    };
    session.plugin_states.merge(&installed.states);
    prepare_edit(session, command)?;
    Ok(result)
}

pub(crate) fn identity_key(id: &PluginIdentity) -> String {
    match id {
        PluginIdentity::Clap(id) => format!("clap:{id}"),
        PluginIdentity::Vst3(id) => {
            use std::fmt::Write;
            let mut key = String::from("vst3:");
            for byte in id {
                write!(&mut key, "{byte:02x}").expect("string writes are infallible");
            }
            key
        }
    }
}
