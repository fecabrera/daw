//! Worker-side diagnostics use the same complete chain walk as core validation.
use daw_core::{EffectChainTarget, EffectSettings, Project};

pub(super) fn missing(project: &Project) -> Vec<String> {
    project
        .effect_chains()
        .flat_map(|(owner, effects)| {
            effects.iter().filter_map(move |effect| {
                let EffectSettings::Missing { type_id, .. } = &effect.unit else {
                    return None;
                };
                let label = match owner {
                    EffectChainTarget::Master => "Master".into(),
                    EffectChainTarget::Track(id) => {
                        format!("track {}", project.tracks.iter().find(|t| t.id == id)?.name)
                    }
                    EffectChainTarget::Layer { track: id, layer } => {
                        if let Some((parent, row)) = project.beat_subtrack(id) {
                            let parent = project.tracks.iter().find(|t| t.id == parent)?;
                            let bus = row
                                .instrument
                                .layers
                                .iter()
                                .find(|l| l.id == layer)
                                .map_or("Output", |l| l.name.as_str());
                            format!("track {} / subtrack {} / {bus}", parent.name, row.name)
                        } else if let Some(track) = project
                            .tracks
                            .iter()
                            .find(|t| t.beat.as_ref().is_some_and(|b| b.output.id == id))
                        {
                            format!("track {} / beat Output", track.name)
                        } else {
                            let track = project.tracks.iter().find(|t| t.id == id)?;
                            let bus = track
                                .instrument
                                .as_ref()?
                                .layers
                                .iter()
                                .find(|l| l.id == layer)
                                .map_or("Output", |l| l.name.as_str());
                            format!("track {} / {bus}", track.name)
                        }
                    }
                };
                Some(format!(
                    "Unavailable effect {type_id} on {label}; bypassed with settings retained"
                ))
            })
        })
        .collect()
}
