use daw_core::*;
use daw_project::Session;
use std::{fs, process::Command};
#[test]
fn headless_beat_projects_validate_inspect_render_and_collect_embedded_patterns() {
    let root = std::env::temp_dir().join(format!("daw-cli-beat-{}", Id::new_v4()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut session = Session::default();
    let track = session.project.add_beat_track().unwrap();
    let mut row = BeatSubtrack::new("Kick".into());
    row.instrument = InstrumentSettings::from(OscillatorSettings::default());
    let id = row.id;
    session
        .project
        .edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    let clip = session.project.add_beat_pattern(track, 0).unwrap();
    session
        .project
        .edit(Edit::ReplaceBeatPattern {
            clip_id: clip,
            pattern: BeatPattern::default().with_step(id, 0, Some(100)).unwrap(),
        })
        .unwrap();
    let project = root.join("Project");
    session.save(&project).unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_daw-cli"))
            .args(args)
            .args(["--project", "."])
            .current_dir(&project)
            .env("DISPLAY", "")
            .env("WAYLAND_DISPLAY", "")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["validate"]);
    let text = run(&["tracks", "Beat 1"]);
    assert!(text.contains("Type: Beat"));
    assert!(text.contains("Kick"));
    assert!(text.contains("Beat subtracks: 1"));
    run(&["collect-sources", "--dry-run"]);
    let output = root.join("Beat.wav");
    run(&["render", "--output", output.to_str().unwrap()]);
    assert!(fs::metadata(output).unwrap().len() > 44);
}
