use super::*;
use daw_core::{BounceMode, BounceRequest, ClipEdge, ClipFade, ClipPlacement, MidiNoteSplit};
use daw_project::{BounceProgress, NormalizeMode, NormalizeOptions, NormalizeProgress};

pub(super) fn run(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    match path {
        "clip.list" => {
            let owner = scoped_track(&session.project, args, "track")?;
            return Ok(json!(
                session
                    .project
                    .tracks
                    .iter()
                    .filter(|track| owner.is_none_or(|id| id == track.id))
                    .flat_map(|track| track.clips.iter().map(|clip| record(track.id, clip)))
                    .collect::<Vec<_>>()
            ));
        }
        "clip.range.delete" | "clip.range.move" | "clip.range.copy" => {
            return range(path, args, session);
        }
        "clip.audio.import" => return import(args, session),
        "clip.bounce" | "track.bounce" => return bounce(path, args, session),
        "clip.delete"
        | "clip.split"
        | "clip.edge.trim"
        | "clip.repeat.resize"
        | "clip.stretch.resize"
        | "clip.audio.normalize"
        | "clip.audio.merge" => {
            return multiple(path, args, session);
        }
        _ => {}
    }
    let project = &session.project;
    let clip = clip(
        project,
        get(args, "clip"),
        scoped_track(project, args, "track")?,
    )?;
    let clip_id = clip.id;
    let owner = clip_owner(project, clip_id).id;
    let command = match path {
        "clip.show" => return Ok(record(owner, clip)),
        "clip.rename" => Edit::SetClipName {
            clip_id,
            name: raw(args, "name").into(),
        },
        "clip.color.set" => {
            let override_color = opt::<String>(args, "color");
            if override_color.is_some() == args.get_flag("inherit") {
                usage("Choose either --color or --inherit");
            }
            Edit::SetClipColor {
                clip_id,
                color: override_color.map(|value| color(value)),
            }
        }
        "clip.mix.set" => {
            let mut commands = Vec::new();
            if let Some(&gain_db) = opt(args, "gain-db") {
                commands.push(Edit::SetClipGain { clip_id, gain_db });
            }
            if let Some(&muted) = opt(args, "muted") {
                commands.push(Edit::SetClipMuted { clip_id, muted });
            }
            changes(!commands.is_empty());
            Edit::Batch(commands)
        }
        "clip.move" | "clip.copy" => {
            let mut copy = clip.clone();
            if path == "clip.copy" {
                copy.id = Id::new_v4();
            }
            copy.start_frame = *get(args, "at-frame");
            let track_id = scoped_track(project, args, "to-track")?.unwrap_or(owner);
            let id = copy.id;
            edit(
                session,
                Edit::OverwriteClips(vec![ClipPlacement {
                    track_id,
                    clip: copy,
                }]),
            )?;
            return Ok(record(track_id, clip_row(&session.project, id)));
        }
        "clip.boundary.trim" => Edit::TrimBoundary {
            clip_id,
            selected: clip_ids(project, args, "also-clip")?,
            edge: edge(args),
            delta: i128::from(*get::<i64>(args, "delta-frames")),
        },
        "clip.audio.channel.set" => {
            require_audio(project, clip)?;
            let mut channels = clip.channels;
            changes(
                update(args, "left", &mut channels.left)
                    | update(args, "right", &mut channels.right)
                    | update(args, "swap", &mut channels.swapped)
                    | update(args, "invert", &mut channels.inverted),
            );
            Edit::SetClipChannels { clip_id, channels }
        }
        "clip.audio.fade.set" => fade(project, clip, args)?,
        "clip.audio.transpose" => {
            require_audio(project, clip)?;
            Edit::SetClipTranspose {
                clip_id,
                transpose: *get(args, "semitones"),
            }
        }
        "clip.audio.stems.separate" => return separate(args, session, clip_id),
        _ => unreachable!("arrangement route"),
    };
    if matches!(path, "clip.boundary.trim" | "clip.audio.transpose") {
        prepare_edit(session, command)?;
    } else {
        edit(session, command)?;
    }
    Ok(record(owner, clip_row(&session.project, clip_id)))
}

fn record(track_id: Id, clip: &Clip) -> Value {
    json!({"trackId": track_id, "endFrame": clip.end(), "clip": clip})
}

fn scoped_track(project: &Project, args: &ArgMatches, key: &str) -> Result<Option<Id>> {
    opt(args, key)
        .map(|selector| track(project, selector))
        .transpose()
}

fn require_audio(project: &Project, clip: &Clip) -> Result<()> {
    project
        .audio_asset(clip.asset_id)
        .ok_or_else(|| error("An audio clip is required"))?;
    Ok(())
}

fn edge(args: &ArgMatches) -> ClipEdge {
    if raw(args, "edge") == "left" {
        ClipEdge::Left
    } else {
        ClipEdge::Right
    }
}

fn fade(project: &Project, clip: &Clip, args: &ArgMatches) -> Result<Edit> {
    require_audio(project, clip)?;
    let left = raw(args, "edge") == "in";
    let mut fades = clip.fades;
    let fade = if left {
        &mut fades.fade_in
    } else {
        &mut fades.fade_out
    };
    let mode = raw(args, "mode");
    let previous_length = fade.length_frames;
    let length = opt::<u64>(args, "length-frames");
    let level = opt::<f32>(args, "midpoint");
    let position = opt::<f32>(args, "position");
    if mode == "default" {
        if length.is_some() || level.is_some() || position.is_some() {
            usage("Omit curve options when restoring the default fade");
        }
        *fade = ClipFade::default();
    } else {
        let length = length.copied().unwrap_or(previous_length);
        let crossfade = mode == "crossfade";
        if length == 0 || length > project.clip_fade_limit(clip.id, left, crossfade) {
            return Err(error(
                "Fade length must be positive and fit the available edge",
            ));
        }
        fade.length_frames = length;
        fade.crossfade = crossfade;
        fade.inherit_source = false;
        if let Some(&value) = level {
            if !(ClipFade::MIN_LEVEL..=ClipFade::MAX_LEVEL).contains(&value) {
                return Err(error("Fade midpoint is outside its range"));
            }
            fade.midpoint = value;
        }
        if let Some(&value) = position {
            if !(ClipFade::MIN_POSITION..=ClipFade::MAX_POSITION).contains(&value) {
                return Err(error("Fade midpoint position is outside its range"));
            }
            fade.midpoint_position = value;
        }
    }
    Ok(Edit::SetClipFades {
        clip_id: clip.id,
        fades,
    })
}

fn multiple(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let ids = clip_ids(&session.project, args, "clip")?;
    match path {
        "clip.delete" => {
            edit(
                session,
                Edit::Batch(ids.iter().copied().map(Edit::DeleteClip).collect()),
            )?;
            Ok(json!({"deleted": ids}))
        }
        "clip.split" => {
            let policy = if opt::<String>(args, "midi-note-mode")
                .is_some_and(|value| value == "preserve-first")
            {
                MidiNoteSplit::PreserveFirst
            } else {
                MidiNoteSplit::Split
            };
            let at = *get(args, "at-frame");
            if ids.iter().any(|id| {
                let clip = clip_row(&session.project, *id);
                !(clip.start_frame < at && at < clip.end())
            }) {
                return Err(error("Split frame must be inside each selected clip"));
            }
            let (candidate, new_ids) = session
                .project
                .split_clips(&ids, at, policy)
                .map_err(error)?;
            session.project = candidate;
            prepare(session)?;
            Ok(records(session, ids.into_iter().chain(new_ids)))
        }
        "clip.audio.normalize" => {
            let mode = match raw(args, "mode") {
                "sample-peak" => NormalizeMode::SamplePeak,
                "true-peak" => NormalizeMode::TruePeak,
                _ => NormalizeMode::Integrated,
            };
            let edits = session.normalize_clips(
                &ids,
                NormalizeOptions {
                    mode,
                    target: *get(args, "target"),
                    allow_increase: !args.get_flag("no-increase"),
                },
                &NormalizeProgress::default(),
            )?;
            edit(session, Edit::Batch(edits))?;
            Ok(records(session, ids))
        }
        "clip.audio.merge" => {
            let (candidate, id) = session.merged_audio_clips(&ids, &|| false)?;
            *session = candidate;
            Ok(records(session, [id]))
        }
        _ => {
            resize(path, args, session, &ids)?;
            Ok(records(session, ids))
        }
    }
}

fn records(session: &Session, ids: impl IntoIterator<Item = Id>) -> Value {
    json!(
        ids.into_iter()
            .map(|id| record(
                clip_owner(&session.project, id).id,
                clip_row(&session.project, id)
            ))
            .collect::<Vec<_>>()
    )
}

fn resize(path: &str, args: &ArgMatches, session: &mut Session, ids: &[Id]) -> Result<()> {
    let edge = edge(args);
    let project = &session.project;
    let delta = i128::from(opt::<i64>(args, "delta-frames").copied().unwrap_or(0));
    let anchor = u128::from(clip_row(project, ids[0]).length_frames);
    let length = u128::from(opt::<u64>(args, "length-frames").copied().unwrap_or(0));
    let invalid = || error("Clip resize is outside the allowed bounds");
    let clips = ids
        .iter()
        .map(|id| {
            let clip = clip_row(project, *id);
            match path {
                "clip.edge.trim" => {
                    if let Ok(bounds) = project.pattern_trim_bounds(clip, edge) {
                        let target = u64::try_from(i128::from(bounds.edge_frame()) + delta)
                            .map_err(error)?;
                        if !bounds.frame_limits().contains(&target) {
                            return Err(invalid());
                        }
                        let mut preview = clip.clone();
                        match edge {
                            ClipEdge::Left => {
                                preview.start_frame = target;
                                preview.length_frames = clip.end() - target;
                            }
                            ClipEdge::Right => preview.length_frames = target - clip.start_frame,
                        }
                        Ok(preview)
                    } else {
                        let frames = project
                            .assets
                            .iter()
                            .find(|asset| asset.id == clip.asset_id)
                            .ok_or_else(invalid)?
                            .decoded_frame_count;
                        clip.trimmed_by(edge, delta, frames).ok_or_else(invalid)
                    }
                }
                "clip.repeat.resize" => project.looped_clip(clip, edge, delta).ok_or_else(invalid),
                _ => {
                    let scaled = u128::from(clip.length_frames) * length;
                    let length = u64::try_from(
                        scaled / anchor + u128::from(scaled % anchor >= anchor.div_ceil(2)),
                    )
                    .map_err(error)?;
                    let start = match edge {
                        ClipEdge::Left => clip.end().checked_sub(length).ok_or_else(invalid)?,
                        ClipEdge::Right => clip.start_frame,
                    };
                    project
                        .stretched_clip(clip, start, length)
                        .ok_or_else(invalid)
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;
    match path {
        "clip.edge.trim" => {
            session.project = project.trimmed_clips(&clips, edge).map_err(error)?;
            prepare(session)
        }
        "clip.repeat.resize" => prepare_edit(session, Edit::ResizeClips { clips, edge }),
        _ => prepare_edit(session, Edit::StretchClips { clips, edge }),
    }
}

fn range(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let tracks = track_ids(&session.project, args, "track")?;
    let start = *get(args, "start-frame");
    let end = *get(args, "end-frame");
    let mut candidate = if path == "clip.range.copy" {
        session.project.clone()
    } else {
        session
            .project
            .without_clip_range(&tracks, start, end)
            .map_err(error)?
    };
    let mut placed = Vec::new();
    if path != "clip.range.delete" {
        let destination = *get::<u64>(args, "at-frame");
        let mut slices = session
            .project
            .clip_range_slices(&tracks, start, end)
            .map_err(error)?;
        destination
            .checked_add(end - start)
            .ok_or_else(|| error("Destination range exceeds timeline bounds"))?;
        for slice in &mut slices {
            slice.clip.start_frame = destination + (slice.clip.start_frame - start);
            placed.push(slice.clip.id);
        }
        candidate = candidate.placed_clips(&slices).map_err(error)?;
    }
    session.project = candidate;
    Ok(json!({"tracks": tracks, "placed": records(session, placed)}))
}

fn import(args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let file = daw_project::absolute(get::<PathBuf>(args, "input"))?;
    let track = scoped_track(&session.project, args, "track")?;
    let start = *get(args, "at-frame");
    let id = if opt::<String>(args, "placement").is_some_and(|value| value == "overwrite") {
        let data = daw_media::decode_audio_cached(&file, session.project.sample_rate_hz, &|| false)
            .map_err(error)?;
        session.import_decoded_overwrite(&file, data, track, start)?
    } else {
        session.import(&file, track, start)?
    };
    Ok(records(session, [id]))
}

fn bounce(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let (mode, ids) = if path == "clip.bounce" {
        (BounceMode::Clips, clip_ids(&session.project, args, "clip")?)
    } else {
        (
            BounceMode::Tracks,
            track_ids(&session.project, args, "track")?,
        )
    };
    let before: std::collections::HashSet<_> = session
        .project
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter().map(|clip| clip.id))
        .collect();
    let prepared =
        crate::plugins::render_hosted(session.clone(), false, report::warning, move |session| {
            let mut prepared = session
                .bounce(&BounceRequest { mode, ids }, &BounceProgress::default())
                .map_err(error)?;
            prepared.session.plugin_host = None;
            Ok(prepared)
        })?;
    for warning in prepared.warnings {
        report::warning(&warning);
    }
    *session = prepared.session;
    let created = session
        .project
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .map(|clip| clip.id)
        .filter(|id| !before.contains(id));
    Ok(records(session, created))
}

fn separate(args: &ArgMatches, session: &mut Session, id: Id) -> Result<Value> {
    let directory = get::<PathBuf>(args, "model-dir");
    let mut models = daw_project::SeparationModels::default();
    for entry in std::fs::read_dir(directory).map_err(error)? {
        let path = entry.map_err(error)?.path();
        let kind = match path.extension().and_then(|ext| ext.to_str()) {
            Some("onnx") => daw_project::SeparationModelKind::Cpu,
            Some("mlpackage" | "mlmodelc") => daw_project::SeparationModelKind::CoreMl,
            _ => continue,
        };
        if kind.available() {
            if models.path(kind).is_some() {
                return Err(error(format!(
                    "Choose a directory with one {}",
                    kind.name()
                )));
            }
            daw_project::validate_separation_model(kind, &path, &Default::default())
                .map_err(error)?;
            models.set(kind, Some(path));
        }
    }
    if !models.is_configured() {
        return Err(error(
            "No compatible separation model in the selected directory",
        ));
    }
    let prepared = session.separate_clip_for_save(id, &models, &Default::default())?;
    for warning in &prepared.warnings {
        report::warning(warning);
    }
    *session = prepared.publish();
    Ok(json!({"sourceClipId": id}))
}

pub(super) fn transients(args: &ArgMatches, folder: &Path) -> Result<report::Outcome> {
    let session = Session::open(folder)?;
    let clip = clip(&session.project, get(args, "clip"), None)?;
    require_audio(&session.project, clip)?;
    let audio = session.render_clip_source(clip, &|| false)?;
    let onsets = daw_media::audio_transients(&audio, &|| false).map_err(error)?;
    let mut frames = Vec::new();
    for onset in onsets {
        let Some((first, period)) = clip.source_occurrence(onset) else {
            continue;
        };
        let mut frame = first;
        loop {
            if frames.len() == 1_000_000 {
                return Err(error("Too many transient positions in this clip"));
            }
            frames.push(clip.start_frame + frame);
            let Some(next) = period
                .and_then(|period| frame.checked_add(period))
                .filter(|next| *next < clip.length_frames)
            else {
                break;
            };
            frame = next;
        }
    }
    frames.sort_unstable();
    Ok(read(json!({"clipId": clip.id, "frames": frames})))
}
