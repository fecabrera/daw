use daw_core::Id;
use daw_project::{ProjectWriteGuard, Session};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("daw-cli-namespaced-{}", Id::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn run(&self, arguments: &[&str], status: i32) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_daw-cli"))
            .current_dir(&self.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(status),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn json(&self, route: &str, arguments: &[&str]) -> Value {
        let mut command = vec!["--project", "song", "--json", route];
        command.extend(arguments);
        let output = self.run(&command, 0);
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["schema_version"], 1);
        assert_eq!(result["command"], route);
        result["result"].clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_is_project_free_for_all_public_commands_and_namespaces() {
    let f = Fixture::new();
    let root = f.run(&["--help"], 0);
    assert!(String::from_utf8_lossy(&root.stdout).contains("Namespaces:"));
    assert_eq!(f.run(&["help", "--help"], 0).stdout, root.stdout);
    let catalog = include_str!("../../../docs/cli/catalog.md");
    let leaves = catalog
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|row| row.split_once("` |"))
        .map(|(command, _)| command)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(leaves.len(), 131);
    let mut namespaces = vec![String::new()];
    let mut discovered = std::collections::BTreeSet::new();
    while let Some(prefix) = namespaces.pop() {
        let output = if prefix.is_empty() {
            root.stdout.clone()
        } else {
            f.run(&[&prefix, "--help"], 0).stdout
        };
        for line in String::from_utf8(output)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("  "))
        {
            let mut fields = line.split_whitespace();
            let Some(path) = fields.next() else { continue };
            if fields.next() == Some("Namespace") {
                namespaces.push(path.to_owned());
            } else {
                discovered.insert(path.to_owned());
            }
        }
    }
    assert_eq!(
        discovered
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        leaves
    );
    for command in
        leaves
            .into_iter()
            .chain(["tracks", "validate", "render", "collect-sources", "plugins"])
    {
        let output = f.run(&[command, "--help"], 0);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("Usage:") && text.contains(command), "{text}");
        assert!(output.stderr.is_empty());
        let alias = f.run(&["help", command], 0);
        assert!(String::from_utf8_lossy(&alias.stdout).contains(command));
    }
    let output = f.run(&["--project=missing", "project.export.mix", "--help"], 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("--output"));
    let prefix = f.run(&["project", "--help"], 0);
    let prefix = String::from_utf8(prefix.stdout).unwrap();
    assert!(prefix.contains("project.source") && !prefix.contains("project.source.collect"));
    let leaf_group = f.run(&["plugin.catalog"], 0);
    assert!(String::from_utf8_lossy(&leaf_group.stdout).contains("plugin.catalog.scan"));
    for args in [
        vec!["track"],
        vec!["track", "--wrong"],
        vec!["track..list"],
        vec!["Track.list"],
        vec!["project", "source", "collect"],
        vec!["--project", "absent", "tracks"],
        vec!["--project", "absent", "track.list", "--json"],
        vec!["--project", "first", "--project", "second", "track.list"],
        vec!["--project", "absent", "plugin.catalog.list"],
        vec!["track.list"],
    ] {
        // Bare registered prefixes are discovery, while malformed invocations must fail.
        let expected = if args == ["track"] { 0 } else { 2 };
        f.run(&args, expected);
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn track_commands_use_saved_order_exact_selectors_and_guarded_edits() {
    let f = Fixture::new();
    let folder = f.0.join("song");
    let mut session = Session::default();
    session.save(&folder).unwrap();
    let mut ids = Vec::new();
    for kind in ["audio", "midi", "beat"] {
        let added = f.json("track.add", &["--type", kind, "--name", " Name: ü "]);
        assert_eq!(added["kind"], kind);
        ids.push(added["id"].as_str().unwrap().to_owned());
    }
    let list = f.json("track.list", &["--name", " Name: ü "]);
    let listed: Vec<_> = list["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|track| track["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ids);
    assert_eq!(
        f.json("track.list", &["--name", "name: ü"])["tracks"],
        serde_json::json!([])
    );
    assert_eq!(
        f.json("track.show", &["--track", &format!("id:{}", ids[1])])["kind"],
        "midi"
    );
    let manifest = folder.join("project.json");
    let before = fs::read(&manifest).unwrap();
    let ambiguous = f.run(
        &[
            "--project",
            "song",
            "track.rename",
            "--track",
            "name: Name: ü ",
            "--name",
            "Changed",
        ],
        1,
    );
    let diagnostic = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(
        diagnostic.contains("ambiguous-target") && ids.iter().all(|id| diagnostic.contains(id))
    );
    assert_eq!(fs::read(&manifest).unwrap(), before);
    f.run(
        &["--project", "song", "track.show", "--track", "id:invalid"],
        2,
    );
    let selector = format!("id:{}", ids[1]);
    let changed = f.json("track.rename", &["--track", &selector, "--name", " Lead "]);
    assert_eq!(changed["changed"], true);
    let saved = daw_project::inspect_project(&folder).unwrap();
    assert_eq!(saved.tracks[1].name, " Lead ");
    assert_eq!(saved.tracks[1].id.to_string(), ids[1]);
    let bytes = fs::read(&manifest).unwrap();
    assert_eq!(
        f.json("track.rename", &["--track", &selector, "--name", " Lead "])["changed"],
        false
    );
    assert_eq!(fs::read(&manifest).unwrap(), bytes);
    let guard = ProjectWriteGuard::acquire(&folder).unwrap();
    let busy = f.run(
        &[
            "tracks",
            "--project",
            "song",
            "--add",
            "New",
            "--type",
            "audio",
        ],
        1,
    );
    assert!(String::from_utf8_lossy(&busy.stderr).contains("busy:"));
    assert_eq!(
        f.json("track.list", &[])["tracks"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    drop(guard);

    let mut child = Command::new(env!("CARGO_BIN_EXE_daw-cli"))
        .current_dir(&f.0)
        .args([
            "--project",
            "song",
            "track.add",
            "--type",
            "audio",
            "--name",
            "Closed pipe",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    assert!(child.wait().unwrap().success());
    let saved = daw_project::inspect_project(&folder).unwrap();
    assert_eq!(saved.tracks.len(), 4);
    assert_eq!(saved.tracks[3].name, "Closed pipe");
}

#[test]
fn export_and_collection_report_one_result_and_preserve_protected_files() {
    let f = Fixture::new();
    let folder = f.0.join("song");
    let mut session = Session::default();
    let track = session.project.add_midi_track().unwrap();
    session.project.add_midi_pattern(track, 0).unwrap();
    session.save(&folder).unwrap();
    let before = fs::read(folder.join("project.json")).unwrap();
    assert_eq!(f.json("project.validate", &[])["track_count"], 1);
    assert_eq!(
        f.json("project.source.collect", &["--dry-run"])["dry_run"],
        true
    );
    assert!(!folder.join(".daw-cli-write.lock").exists());
    assert_eq!(fs::read(folder.join("project.json")).unwrap(), before);
    f.run(&["--project", "song", "project.export.mix"], 2);
    f.run(
        &[
            "--project",
            "song",
            "project.export.mix",
            "--output",
            "song/.daw-cli-write.lock",
            "--format",
            "wav",
        ],
        1,
    );
    assert!(!folder.join(".daw-cli-write.lock").exists());
    fs::hard_link(folder.join("project.json"), f.0.join("alias.wav")).unwrap();
    f.run(
        &[
            "--project",
            "song",
            "project.export.mix",
            "--output",
            "alias.wav",
            "--overwrite",
        ],
        1,
    );
    assert_eq!(fs::read(f.0.join("alias.wav")).unwrap(), before);
    let result = f.json(
        "project.export.mix",
        &["--output", "mix.wav", "--codec", "pcm24"],
    );
    assert_eq!(result["format"], "wav");
    let mix = fs::read(f.0.join("mix.wav")).unwrap();
    assert!(mix.starts_with(b"RIFF"));
    f.run(
        &[
            "--project",
            "song",
            "project.export.mix",
            "--output",
            "mix.wav",
        ],
        1,
    );
    f.run(
        &["render", "--project", "song", "--output", "legacy.wav"],
        0,
    );
    assert_eq!(fs::read(f.0.join("legacy.wav")).unwrap(), mix);
    assert_eq!(f.json("project.source.collect", &[])["committed"], true);

    let source = f.0.join("mix.wav");
    let destination = f.0.join("during-render.wav");
    let mut reopened = Session::open(&folder).unwrap();
    reopened.import(&source, None, 0).unwrap();
    let error = reopened
        .export_with_progress(&destination, true, Default::default(), |position, _| {
            if position == 0 {
                fs::hard_link(&source, &destination).unwrap();
            }
        })
        .unwrap_err();
    assert!(error.to_string().contains("source audio file"));
    assert_eq!(fs::read(&source).unwrap(), mix);
    assert_eq!(fs::read(&destination).unwrap(), mix);

    let catalog = f.run(
        &[
            "--json",
            "plugin.catalog.list",
            "--cache",
            "empty/catalog.json",
        ],
        0,
    );
    let result: Value = serde_json::from_slice(&catalog.stdout).unwrap();
    assert_eq!(result["result"]["modules"], serde_json::json!([]));
    assert!(!f.0.join("empty").exists());
}

#[test]
fn catalog_edits_persist_and_a_rejected_batch_rolls_back() {
    let f = Fixture::new();
    let created = f.json("project.create", &[]);
    assert_eq!(f.json("project.show", &[])["id"], created["id"]);
    let track = f.json("track.add", &["--type", "midi", "--name", "Lead"]);
    let owner = format!("track:{}", track["id"].as_str().unwrap());
    f.json(
        "midi.options.set",
        &["--track", "name:Lead", "--channel", "3", "--monitor", "off"],
    );
    let pattern = f.json(
        "midi.pattern.add",
        &["--track", "name:Lead", "--at-frame", "0"],
    );
    let clip = format!("id:{}", pattern["clip"]["id"].as_str().unwrap());
    f.json(
        "midi.pattern.note.set",
        &[
            "--clip",
            &clip,
            "--start-tick",
            "0",
            "--length-ticks",
            "960",
            "--pitch",
            "64",
            "--velocity",
            "99",
        ],
    );
    assert_eq!(
        f.json("midi.pattern.note.list", &["--clip", &clip])[0]["pitch"],
        64
    );
    f.json(
        "instrument.layer.add",
        &["--owner", &owner, "--name", "Noise"],
    );
    f.json(
        "instrument.source.set",
        &[
            "--owner",
            &owner,
            "--layer",
            "name:Noise",
            "--type",
            "noise",
            "--replace",
        ],
    );
    let effect = f.json("effect.chain.add", &["--owner", &owner, "--type", "filter"]);
    let effect = effect["unitId"].as_str().unwrap();
    let controls = f.json(
        "unit.parameter.list",
        &["--owner", &owner, "--unit", effect],
    );
    let frequency = controls
        .as_array()
        .unwrap()
        .iter()
        .find(|control| control["key"].as_str().unwrap().ends_with("_hz"))
        .expect("filter frequency");
    let key = frequency["key"].as_str().unwrap();
    assert_eq!(
        f.json(
            "unit.parameter.set",
            &[
                "--owner",
                &owner,
                "--unit",
                effect,
                "--parameter",
                key,
                "--value",
                "900"
            ]
        )["value"],
        900.0
    );
    f.json(
        "effect.power.set",
        &["--owner", &owner, "--unit", effect, "--enabled", "false"],
    );
    f.json(
        "instrument.component.add",
        &["--owner", &owner, "--layer", "name:Noise", "--type", "lfo"],
    );
    let units = f.json(
        "instrument.component.list",
        &["--owner", &owner, "--layer", "name:Noise"],
    );
    let lfo = units["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["kind"] == "lfo")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let source = format!("{}:{lfo}", units["layerId"].as_str().unwrap());
    let destination = format!("{effect}:frequency");
    let route = f.json(
        "modulation.route.set",
        &[
            "--owner",
            &owner,
            "--source",
            &source,
            "--destination",
            &destination,
            "--amount",
            "0.2",
        ],
    );
    assert_eq!(
        f.json("modulation.route.list", &["--owner", &owner])[0]["route"]["id"],
        route["route"]["id"]
    );
    f.json(
        "instrument.component.remove",
        &["--owner", &owner, "--unit", lfo],
    );
    assert!(
        f.json("modulation.route.list", &["--owner", &owner])
            .as_array()
            .unwrap()
            .is_empty()
    );
    let manifest = f.0.join("song/project.json");
    let before = fs::read(&manifest).unwrap();
    let operations = |gain| {
        serde_json::json!({"version": 1, "operations": [
            {"command":"track.rename", "arguments":["--track","name:Lead","--name","Keys"]},
            {"command":"track.mix.set", "arguments":["--track","name:Keys","--gain-db",gain]}
        ]})
    };
    let input = f.0.join("edits.json");
    fs::write(&input, operations("1e8").to_string()).unwrap();
    f.run(
        &[
            "--project",
            "song",
            "project.edits.apply",
            "--input",
            "edits.json",
        ],
        1,
    );
    assert_eq!(fs::read(&manifest).unwrap(), before);
    fs::write(&input, operations("-6").to_string()).unwrap();
    f.json("project.edits.apply", &["--input", "edits.json"]);
    let project = daw_project::inspect_project(&f.0.join("song")).unwrap();
    assert_eq!(project.tracks[0].name, "Keys");
    assert_eq!(project.tracks[0].gain_db, -6.0);
    assert_eq!(project.tracks[0].midi_options.input_channel, Some(2));
    assert_eq!(
        project.tracks[0].midi_options.monitor,
        daw_core::MidiMonitor::Off
    );
}

#[test]
fn clip_and_beat_edits_range_export_and_stem_publication() {
    use daw_media::{AudioEncoder, WavCodec, WavEncoder};
    let f = Fixture::new();
    f.json("project.create", &[]);
    let track = f.json("track.add", &["--name", "Audio", "--type", "audio"]);
    let input = f.0.join("input.wav");
    let mut encoder = WavEncoder::at_sample_rate(&input, WavCodec::Float32, 48_000).unwrap();
    let signal = (0..1024)
        .map(|index| [(index as f32 / 20.0).sin() * 0.25; 2])
        .collect::<Vec<_>>();
    encoder.write_frames(&signal).unwrap();
    encoder.finish().unwrap();
    let result = f.json(
        "clip.audio.import",
        &[
            "--track",
            "name:Audio",
            "--input",
            "input.wav",
            "--at-frame",
            "120",
        ],
    );
    let clip = format!("id:{}", result[0]["clip"]["id"].as_str().unwrap());
    f.json(
        "clip.audio.fade.set",
        &[
            "--clip",
            &clip,
            "--edge",
            "in",
            "--mode",
            "fade",
            "--length-frames",
            "48",
        ],
    );
    f.json(
        "clip.audio.channel.set",
        &["--clip", &clip, "--invert", "true"],
    );
    assert_eq!(
        f.json("clip.split", &["--clip", &clip, "--at-frame", "500"])
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let before = fs::read(f.0.join("song/project.json")).unwrap();
    f.json("project.export.mix", &["--output", "full.wav"]);
    f.json(
        "project.export.range",
        &[
            "--start-frame",
            "140",
            "--end-frame",
            "700",
            "--output",
            "range.wav",
        ],
    );
    let full = daw_media::decode_wav(&f.0.join("full.wav")).unwrap();
    let range = daw_media::decode_wav(&f.0.join("range.wav")).unwrap();
    assert_eq!(range.frame_count(), 560);
    for (expected, actual) in full.samples[140..700]
        .iter()
        .flatten()
        .zip(range.samples.iter().flatten())
    {
        assert!(
            (expected - actual).abs() <= 3.0 / 8_388_608.0,
            "PCM24 dither tolerance"
        );
    }
    f.json(
        "project.export.stems",
        &["--track", "name:Audio", "--output-dir", "stems"],
    );
    let filename = format!("{}.wav", track["id"].as_str().unwrap());
    let stem = daw_media::decode_wav(&f.0.join("stems").join(filename)).unwrap();
    assert_eq!(stem.frame_count(), 1144);
    assert_eq!(stem.samples[119], [0.0; 2]);
    f.run(
        &[
            "--project",
            "song",
            "project.export.stems",
            "--track",
            "name:Audio",
            "--output-dir",
            "stems",
        ],
        1,
    );
    assert_eq!(fs::read(f.0.join("song/project.json")).unwrap(), before);
    f.json("track.bounce", &["--track", "name:Audio"]);
    let copy = f.json("project.copy", &["--destination", "copy"]);
    let copied = Session::open(&f.0.join("copy")).unwrap();
    assert_eq!(copied.project.project_id.to_string(), copy["id"]);
    assert!(copied.project.assets.iter().any(|asset| {
        asset.source.is_generated()
            && copied
                .source_path(asset)
                .unwrap()
                .starts_with(f.0.join("copy"))
    }));
    f.json("track.add", &["--name", "Kit", "--type", "beat"]);
    f.json(
        "beat.subtrack.add",
        &["--track", "name:Kit", "--name", "Kick"],
    );
    let result = f.json(
        "beat.pattern.add",
        &["--track", "name:Kit", "--at-frame", "0"],
    );
    let clip = format!("id:{}", result["clip"]["id"].as_str().unwrap());
    f.json(
        "beat.pattern.step.set",
        &[
            "--clip",
            &clip,
            "--subtrack",
            "name:Kick",
            "--at-tick",
            "240",
            "--velocity",
            "90",
        ],
    );
    let result = f.json(
        "beat.pattern.grid.set",
        &["--clip", &clip, "--bars", "1", "--resolution", "quarter"],
    );
    assert_eq!(result["pattern"]["steps"].as_array().unwrap().len(), 1);
    assert!(
        f.json("beat.pattern.show", &["--clip", &clip])["visibleStepIds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
