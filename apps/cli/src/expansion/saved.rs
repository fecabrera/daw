use super::*;
use daw_core::{
    ArrangementMarker, SampleFormat, SendTap, SourceStorage, StretchEngine, TempoEvent,
    TimeSignature, TrackKind, TrackSend,
};
use std::fs;

pub(super) fn run(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    match path {
        "project.edits.apply" => batch(args, session),
        "project.show" => Ok(json!({
            "id": project.project_id, "name": project.name, "track_count": project.tracks.len(),
            "asset_count": project.assets.len(), "end_frame": project.end(),
            "settings": settings(project), "tempo_bpm": project.tempo_bpm, "time_signature": project.time_signature
        })),
        "project.rename" => {
            session.project.name = get::<String>(args, "name").clone();
            Ok(json!({"name": session.project.name, "id": session.project.project_id}))
        }
        "project.settings.show" => Ok(settings(project)),
        "project.settings.set" => {
            let mut rate = project.sample_rate_hz;
            let mut buffer = project.buffer_size_frames;
            let format = match opt::<String>(args, "sample-format") {
                Some(key) => {
                    serde_json::from_value::<SampleFormat>(json!(key)).unwrap_or_else(|_| {
                        usage(
                            "Unknown sample format. Use i8/i16/i24/i32/i64/u8/u16/u32/u64/f32/f64",
                        )
                    })
                }
                None => project.output_sample_format,
            };
            let engine = match opt::<String>(args, "stretch-engine").map(String::as_str) {
                Some("rubberband-r3") => StretchEngine::RUBBER_BAND,
                Some("timestretch") => StretchEngine::INTEGRATED,
                Some(_) => usage("Use rubberband-r3 or timestretch"),
                None => project.stretch_engine,
            };
            changes(
                update(args, "sample-rate-hz", &mut rate)
                    | update(args, "buffer-frames", &mut buffer)
                    | opt::<String>(args, "sample-format").is_some()
                    | opt::<String>(args, "stretch-engine").is_some(),
            );
            *session = session.with_processing_settings(rate, buffer, engine)?;
            session.project.output_sample_format = format;
            Ok(settings(&session.project))
        }
        "project.source.list" => {
            let storage = opt::<String>(args, "storage");
            Ok(json!({"assets": project.assets.iter().filter_map(|asset| {
                let storage_key = match asset.source.storage_kind() {
                    SourceStorage::Project => "project", SourceStorage::External => "external", SourceStorage::Embedded => "embedded",
                };
                if storage.is_some_and(|target| target != storage_key) { return None; }
                let clips: Vec<_> = project.tracks.iter().flat_map(|track| &track.clips)
                    .filter(|clip| clip.asset_id == asset.id).map(|clip| clip.id).collect();
                let samplers: Vec<_> = project.instrument_owners()
                    .flat_map(|(track, owner, instrument)| instrument.layers.iter().filter_map(move |layer| {
                        let sampler = layer.sampler.as_ref()?;
                        (sampler.input.as_ref()?.asset_id == asset.id).then(||
                            json!({"trackId": track, "ownerId": owner, "layerId": layer.id, "unitId": sampler.id}))
                    })).collect();
                Some(json!({"id": asset.id, "name": asset.name, "path": asset.source.path,
                    "storage": storage_key, "provenance": asset.source.kind, "clips": clips,
                    "samplers": samplers}))
            }).collect::<Vec<_>>()}))
        }
        "project.source.relink" => relink(session, args),
        "track.duplicate" | "track.delete" | "track.move" | "track.group.form" => {
            track_batch(path, args, session)
        }
        "track.group.add" | "track.return.add" => {
            let id = if path == "track.return.add" {
                session.project.add_return_track()
            } else if let Some(parent) = opt(args, "parent") {
                let parent = track(project, parent)?;
                session.project.add_track_in_group(parent, TrackKind::Group)
            } else {
                session.project.add_group_track()
            }
            .map_err(error)?;
            edit(
                session,
                Edit::SetTrackName {
                    track_id: id,
                    name: raw(args, "name").to_owned(),
                },
            )?;
            Ok(json!({"id": id}))
        }
        "track.group.dissolve"
        | "track.mix.set"
        | "track.color.set"
        | "track.return.move"
        | "track.send.list"
        | "track.send.set"
        | "track.send.remove" => track_single(path, args, session),
        "master.mix.show" => Ok(json!({"gain_db": project.master.gain_db})),
        "master.mix.set" => {
            session.project.master.gain_db = *get(args, "gain-db");
            Ok(json!({"gain_db": session.project.master.gain_db}))
        }
        "timeline.timing.show" => Ok(json!({
            "tempo_bpm": project.tempo_bpm, "time_signature": project.time_signature,
            "tempos": project.tempo_events(), "signatures": project.signature_events(), "markers": project.markers
        })),
        "timeline.marker.list" => Ok(json!({"markers": project.markers})),
        "timeline.loop.show" => Ok(json!(project.transport.r#loop)),
        "timeline.loop.set" => {
            let settings = &mut session.project.transport.r#loop;
            let start = opt::<u64>(args, "start-frame").copied();
            let end = opt::<u64>(args, "end-frame").copied();
            if start.is_some() != end.is_some() {
                usage("Supply both --start-frame and --end-frame");
            }
            changes(start.is_some() | update(args, "enabled", &mut settings.enabled));
            if let Some((start, end)) = start.zip(end) {
                if start >= end {
                    return Err(error("Loop range must be nonempty"));
                }
                settings.start_frame = start;
                settings.end_frame = end;
            }
            Ok(json!(settings))
        }
        "timeline.tempo.set"
        | "timeline.tempo.event.set"
        | "timeline.tempo.event.remove"
        | "timeline.signature.set"
        | "timeline.signature.event.set"
        | "timeline.signature.event.remove"
        | "timeline.marker.add"
        | "timeline.marker.set"
        | "timeline.marker.remove" => timing(path, args, session),
        _ => unreachable!("registry selects saved-project handler"),
    }
}

fn settings(project: &Project) -> Value {
    json!({"sample_rate_hz": project.sample_rate_hz, "buffer_frames": project.buffer_size_frames,
        "sample_format": project.output_sample_format, "stretch_engine": project.stretch_engine})
}

fn optional_track(project: &Project, value: &str, empty: &str) -> Result<Option<Id>> {
    if value == empty {
        return Ok(None);
    }
    let selector = tracks::parse_selector(value).unwrap_or_else(|message| usage(&message));
    Ok(Some(track(project, &selector)?))
}

fn track_batch(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let ids = track_ids(project, args, "track")?;
    match path {
        "track.duplicate" => {
            let copies: Vec<_> = project
                .track_selection_roots(&ids)
                .into_iter()
                .map(|original| (original, Id::new_v4()))
                .collect();
            edit(
                session,
                Edit::Batch(
                    copies
                        .iter()
                        .map(|&(track_id, new_track_id)| Edit::DuplicateTrack {
                            track_id,
                            new_track_id,
                        })
                        .collect(),
                ),
            )?;
            Ok(
                json!({"copies": copies.iter().map(|(from, to)| json!({"from": from, "to": to})).collect::<Vec<_>>()}),
            )
        }
        "track.delete" => {
            for id in &ids {
                if project.track_subtree(*id).map_err(error)?.len() != 1 {
                    return Err(error(
                        "Dissolve or empty a populated Group before deleting it",
                    ));
                }
            }
            edit(
                session,
                Edit::Batch(ids.iter().copied().map(Edit::DeleteTrack).collect()),
            )?;
            Ok(json!({"deleted": ids}))
        }
        "track.move" => {
            let parent = optional_track(project, raw(args, "parent"), "master")?;
            let removed = project.moving_track_indices(&ids);
            let survivors: Vec<_> = project
                .tracks
                .iter()
                .enumerate()
                .filter(|(index, _)| !removed.contains(index))
                .map(|(_, track)| track)
                .collect();
            let siblings: Vec<_> = survivors
                .iter()
                .enumerate()
                .filter(|(_, track)| track.group_id == parent && track.kind != TrackKind::Return)
                .map(|(index, _)| index)
                .collect();
            let position = *get::<usize>(args, "index");
            let index = if let Some(index) = siblings.get(position) {
                *index
            } else if position == siblings.len() {
                match parent {
                    Some(group) => {
                        let range = project.track_subtree(group).map_err(error)?;
                        range.end - removed.iter().filter(|&&index| index < range.end).count()
                    }
                    None => survivors
                        .iter()
                        .position(|track| track.kind == TrackKind::Return)
                        .unwrap_or(survivors.len()),
                }
            } else {
                return Err(error("Destination sibling index is out of range"));
            };
            edit(
                session,
                Edit::MoveTracks {
                    track_ids: ids.clone(),
                    group_id: parent,
                    index,
                },
            )?;
            Ok(json!({"tracks": ids, "parent": parent, "index": position}))
        }
        "track.group.form" => {
            let id = Id::new_v4();
            edit(
                session,
                Edit::GroupTracks {
                    track_ids: ids,
                    group_id: id,
                    name: raw(args, "name").into(),
                },
            )?;
            Ok(json!({"id": id}))
        }
        _ => unreachable!(),
    }
}

fn track_single(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let id = track(project, get(args, "track"))?;
    let saved = track_row(project, id);
    match path {
        "track.group.dissolve" => edit(session, Edit::UngroupTrack { track_id: id })?,
        "track.color.set" => edit(
            session,
            Edit::SetTrackColor {
                track_id: id,
                color: color(raw(args, "color")),
            },
        )?,
        "track.mix.set" => {
            let saved = session
                .project
                .tracks
                .iter_mut()
                .find(|track| track.id == id)
                .unwrap();
            changes(
                update(args, "gain-db", &mut saved.gain_db)
                    | update(args, "pan", &mut saved.pan)
                    | update(args, "muted", &mut saved.muted)
                    | update(args, "soloed", &mut saved.soloed),
            );
            return Ok(json!({"id": id, "gain_db": saved.gain_db, "pan": saved.pan,
                "muted": saved.muted, "soloed": saved.soloed}));
        }
        "track.return.move" => {
            let before = optional_track(project, raw(args, "before"), "end")?;
            if saved.kind != TrackKind::Return {
                return Err(error("Select a Return track"));
            }
            let mut ids: Vec<_> = project
                .tracks
                .iter()
                .filter(|track| track.kind == TrackKind::Return)
                .map(|track| track.id)
                .collect();
            if before != Some(id) {
                ids.retain(|target| *target != id);
                let index = before
                    .map(|target| {
                        ids.iter()
                            .position(|row| *row == target)
                            .ok_or_else(|| error("Destination must be a Return"))
                    })
                    .transpose()?
                    .unwrap_or(ids.len());
                ids.insert(index, id);
                edit(session, Edit::ReorderReturns { return_ids: ids })?;
            }
        }
        "track.send.list" => return Ok(json!({"id": id, "sends": saved.sends})),
        "track.send.set" | "track.send.remove" => {
            let target = track(project, get(args, "return"))?;
            if track_row(project, target).kind != TrackKind::Return {
                return Err(error("Select a Return destination"));
            }
            let mut sends = saved.sends.clone();
            if path == "track.send.remove" {
                sends.retain(|send| send.return_id != target);
            } else {
                if !sends.iter().any(|send| send.return_id == target) {
                    sends.push(TrackSend::new(target));
                }
                let send = sends
                    .iter_mut()
                    .find(|send| send.return_id == target)
                    .unwrap();
                send.gain_db = *get(args, "gain-db");
                send.tap = if raw(args, "tap") == "pre" {
                    SendTap::PreFader
                } else {
                    SendTap::PostFader
                };
                send.enabled = true;
            }
            let result = json!({"id": id, "sends": sends});
            edit(
                session,
                Edit::SetTrackSends {
                    track_id: id,
                    sends,
                },
            )?;
            return Ok(result);
        }
        _ => unreachable!(),
    }
    Ok(json!({"id": id}))
}

fn signature(args: &ArgMatches) -> TimeSignature {
    TimeSignature {
        numerator: *get(args, "numerator"),
        denominator: *get(args, "denominator"),
    }
}

fn timing(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let requested = opt::<Id>(args, "id").copied();
    let mut id = requested.unwrap_or_else(Id::new_v4);
    let operation = match path {
        "timeline.tempo.set" => Edit::SetTempo {
            bpm: *get(args, "bpm"),
        },
        "timeline.signature.set" => Edit::SetTimeSignature {
            signature: signature(args),
        },
        "timeline.tempo.event.set" | "timeline.tempo.event.remove" => {
            let mut events = project.tempo_events();
            if requested.is_some()
                && !events
                    .iter()
                    .any(|event| event.id == id && event.start_frame != 0)
            {
                return Err(error("Select an existing non-origin tempo event"));
            }
            events.retain(|event| event.id != id);
            if path.ends_with(".set") {
                let frame = *get(args, "at-frame");
                if frame == 0 {
                    return Err(error("Use timeline.tempo.set for the origin"));
                }
                events.push(TempoEvent {
                    id,
                    start_frame: frame,
                    bpm: *get(args, "bpm"),
                });
            }
            Edit::SetTempoEvents { events }
        }
        "timeline.signature.event.set" | "timeline.signature.event.remove" => {
            let mut events = project.signature_events();
            if requested.is_some()
                && !events
                    .iter()
                    .any(|event| event.id == id && event.start_frame != 0)
            {
                return Err(error("Select an existing non-origin signature event"));
            }
            events.retain(|event| event.id != id);
            if path.ends_with(".set") {
                let frame = *get(args, "at-frame");
                if frame == 0 {
                    return Err(error("Use timeline.signature.set for the origin"));
                }
                events.push(daw_core::SignatureEvent {
                    id,
                    start_frame: frame,
                    signature: signature(args),
                });
            }
            Edit::SetSignatureEvents { events }
        }
        "timeline.marker.add" | "timeline.marker.set" | "timeline.marker.remove" => {
            let mut markers = project.markers.clone();
            if path.ends_with(".add") {
                markers.push(ArrangementMarker {
                    id,
                    start_frame: *get(args, "at-frame"),
                    name: raw(args, "name").into(),
                    length_frames: None,
                });
            } else {
                id = select(&markers, get(args, "marker"), |marker| {
                    (marker.id, &marker.name)
                })?
                .id;
                if path.ends_with(".remove") {
                    markers.retain(|marker| marker.id != id);
                } else {
                    let length = opt::<u64>(args, "length-frames").copied();
                    let name = opt::<String>(args, "name");
                    let start = opt::<u64>(args, "at-frame").copied();
                    changes(start.is_some() || length.is_some() || name.is_some());
                    if length == Some(0) {
                        return Err(error("A section must have positive length"));
                    }
                    if let Some(start) = start {
                        let original = markers
                            .iter()
                            .find(|marker| marker.id == id)
                            .expect("resolved section");
                        markers = project
                            .edited_sections(
                                id,
                                &[],
                                None,
                                i128::from(start) - i128::from(original.start_frame),
                                false,
                            )
                            .map_err(error)?;
                    }
                    if let Some(length) = length {
                        let changed = project
                            .edited(Edit::SetMarkers { markers })
                            .map_err(error)?;
                        let marker = changed
                            .markers
                            .iter()
                            .find(|marker| marker.id == id)
                            .expect("resolved section");
                        let end = changed
                            .musical_clock()
                            .and_then(|clock| clock.section_end_frame(marker))
                            .map_err(error)?;
                        let target = marker
                            .start_frame
                            .checked_add(length)
                            .ok_or_else(|| error("Section exceeds timeline bounds"))?;
                        markers = changed
                            .edited_sections(
                                id,
                                &[],
                                Some(daw_core::ClipEdge::Right),
                                i128::from(target) - i128::from(end),
                                false,
                            )
                            .map_err(error)?;
                    }
                    if let Some(name) = name {
                        markers
                            .iter_mut()
                            .find(|marker| marker.id == id)
                            .expect("resolved section")
                            .name
                            .clone_from(name);
                    }
                }
            }
            Edit::SetMarkers { markers }
        }
        _ => unreachable!(),
    };
    if path == "timeline.signature.set" {
        edit(session, operation)?;
    } else {
        prepare_edit(session, operation)?;
    }
    Ok(json!({
        "id": if matches!(path, "timeline.tempo.set" | "timeline.signature.set") { None } else { Some(id) },
        "tempo_bpm": session.project.tempo_bpm, "time_signature": session.project.time_signature
    }))
}

fn relink(session: &mut Session, args: &ArgMatches) -> Result<Value> {
    let asset = select(&session.project.assets, get(args, "asset"), |asset| {
        (asset.id, &asset.name)
    })?;
    if !asset.is_file_audio() || asset.source.is_generated() {
        return Err(error("Relink requires a file-backed original recording"));
    }
    let id = asset.id;
    let current = session.source_path(asset)?;
    let replacement = fs::canonicalize(get::<PathBuf>(args, "input")).map_err(error)?;
    let old = daw_media::SourceRevision::read(&current, &|| false)
        .map_err(|e| error(format!("Cannot verify the original encoded bytes: {e}")))?;
    if old != daw_media::SourceRevision::read(&replacement, &|| false).map_err(error)? {
        return Err(error("Relink requires byte-identical original audio"));
    }
    let path = replacement
        .to_str()
        .ok_or_else(|| error("Saved media references require a Unicode path"))?;
    let asset = session
        .project
        .assets
        .iter_mut()
        .find(|asset| asset.id == id)
        .unwrap();
    asset.source.path = path.into();
    asset.source.path_kind = "absolute".into();
    asset.source.storage = Some(SourceStorage::External);
    Ok(json!({"id": id, "path": path}))
}

pub(super) fn special(path: &str, args: &ArgMatches, folder: &Path) -> Result<report::Outcome> {
    match path {
        "project.create" => {
            let mut session = Session::default();
            new_folder(&mut session, folder)?;
            Ok(committed(
                json!({"id": session.project.project_id, "folder": folder.to_string_lossy()}),
            ))
        }
        "project.copy" => {
            let mut session = Session::open(folder)?;
            session.project.project_id = Id::new_v4();
            let destination = get::<PathBuf>(args, "destination");
            new_folder(&mut session, destination)?;
            Ok(committed(
                json!({"id": session.project.project_id, "folder": destination.to_string_lossy()}),
            ))
        }
        "clip.audio.transients.list" => arrangement::transients(args, folder),
        "project.export.range" | "project.export.stems" => export(path, args, folder),
        "project.recovery.list" | "project.recovery.restore" => recovery(path, args, folder),
        "project.source.check" => crate::collect_sources::collect(folder, true, false),
        _ => unreachable!("registry selects special handler"),
    }
}

fn new_folder(session: &mut Session, destination: &Path) -> Result<()> {
    let (parent, name) = destination_parts(destination)?;
    session.save_new(&parent, &name)
}

fn destination_parts(destination: &Path) -> Result<(PathBuf, String)> {
    let destination = daw_project::absolute(destination)?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| error("Supply a Unicode project folder name"))?;
    let parent = destination
        .parent()
        .ok_or_else(|| error("Supply a destination inside an existing folder"))?;
    if daw_project::project_folder_name(name)? != name {
        return Err(error("Destination folder name has surrounding whitespace"));
    }
    Ok((parent.to_path_buf(), name.to_owned()))
}

fn export(path: &str, args: &ArgMatches, folder: &Path) -> Result<report::Outcome> {
    let session = Session::open(folder)?;
    let result = if path == "project.export.range" {
        let output = get::<PathBuf>(args, "output");
        let range = *get::<u64>(args, "start-frame")..*get::<u64>(args, "end-frame");
        let settings = super::super::export_settings(output, None, None, None)?;
        let warnings = crate::plugins::render_hosted(session, false, report::warning, |session| {
            session.export_range(output, settings, range.clone())
        })?;
        for warning in warnings {
            report::warning(&warning);
        }
        json!({"output": output.to_string_lossy(), "startFrame": range.start, "endFrame": range.end})
    } else {
        let directory = get::<PathBuf>(args, "output-dir");
        let tracks = track_ids(&session.project, args, "track")?;
        let (files, warnings) =
            crate::plugins::render_hosted(session, false, report::warning, |session| {
                session.export_stems(directory, tracks)
            })?;
        for warning in warnings {
            report::warning(&warning);
        }
        json!({"files": files})
    };
    Ok(committed(result))
}

fn recovery(path: &str, args: &ArgMatches, folder: &Path) -> Result<report::Outcome> {
    use daw_project::recovery::{RecoveryStore, default_root};
    let id = daw_project::inspect_project(folder)?.project_id;
    let root =
        default_root().ok_or_else(|| error("Recovery storage is unavailable on this system"))?;
    let entries = RecoveryStore::inspect(&root)?
        .into_iter()
        .filter(|entry| entry.project == id);
    if path == "project.recovery.list" {
        return Ok(read(json!(entries.map(|entry| json!({
            "entryId": entry.id, "projectId": entry.project, "name": entry.name, "revision": entry.revision,
        })).collect::<Vec<_>>())));
    }
    let wanted = *get::<Id>(args, "entry");
    let entry = entries
        .into_iter()
        .find(|entry| entry.id == wanted)
        .ok_or_else(|| error("Recovery entry is unavailable for this project"))?;
    let (parent, name) = destination_parts(get::<PathBuf>(args, "destination"))?;
    let id = RecoveryStore::copy_entry(&entry, &parent, &name)?;
    Ok(committed(
        json!({"entryId": wanted, "projectId": id, "folder": parent.join(name)}),
    ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Operations {
    version: u32,
    operations: Vec<Operation>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    command: String,
    arguments: Vec<String>,
}

fn batch(args: &ArgMatches, session: &mut Session) -> Result<Value> {
    use std::io::Read;
    const MAX_BYTES: u64 = 1_048_576;
    const MAX_OPERATIONS: usize = 256;
    let path = get::<PathBuf>(args, "input");
    if !fs::metadata(path).map_err(error)?.is_file() {
        return Err(error("Edit batch input requires an ordinary file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(error)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(error("Edit batch exceeds 1 MiB"));
    }
    let content: Operations = serde_json::from_slice(&bytes).map_err(error)?;
    if content.version != 1
        || content.operations.is_empty()
        || content.operations.len() > MAX_OPERATIONS
    {
        return Err(error("Use edit batch version 1 with 1–256 operations"));
    }
    let mut results = Vec::with_capacity(content.operations.len());
    for (index, operation) in content.operations.into_iter().enumerate() {
        let label = || format!("Operation {} ({})", index + 1, operation.command);
        let command = match operation.command.as_str() {
            "track.rename" => {
                let command = Command::new("track.rename").args([
                    Param {
                        name: "track",
                        kind: Kind::Selector,
                        required: true,
                        many: false,
                    }
                    .argument(),
                    Param {
                        name: "name",
                        kind: Kind::Name,
                        required: true,
                        many: false,
                    }
                    .argument(),
                ]);
                let parsed = command
                    .try_get_matches_from(
                        std::iter::once(operation.command.as_str())
                            .chain(operation.arguments.iter().map(String::as_str)),
                    )
                    .map_err(|failure| error(format!("{}: {failure}", label())))?;
                let id = track(&session.project, get(&parsed, "track"))?;
                edit(
                    session,
                    Edit::SetTrackName {
                        track_id: id,
                        name: raw(&parsed, "name").into(),
                    },
                )?;
                results.push(json!({"command": operation.command, "result": { "trackId": id }}));
                continue;
            }
            "project.rename"
            | "track.mix.set"
            | "track.color.set"
            | "master.mix.set"
            | "clip.rename"
            | "clip.mix.set"
            | "clip.color.set"
            | "clip.move"
            | "clip.copy"
            | "clip.delete"
            | "clip.audio.channel.set"
            | "clip.audio.fade.set"
            | "timeline.loop.set"
            | "beat.subtrack.rename"
            | "beat.subtrack.mix.set"
            | "beat.output.mix.set" => route(&operation.command).expect("registered batch route"),
            _ => {
                return Err(error(format!(
                    "{} is outside the version 1 batch whitelist",
                    label()
                )));
            }
        };
        let parsed = command
            .command()
            .try_get_matches_from(
                std::iter::once(operation.command.as_str())
                    .chain(operation.arguments.iter().map(String::as_str)),
            )
            .map_err(|failure| error(format!("{}: {failure}", label())))?;
        let result = (command.handler)(command.path, &parsed, session)
            .map_err(|failure| error(format!("{}: {failure}", label())))?;
        results.push(json!({"command": operation.command, "result": result}));
    }
    Ok(json!({"operations": results}))
}
