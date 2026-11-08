use daw_core::{Id, TrackKind};
use daw_project::Session;
use std::{fs, process::Command};

#[test]
fn add_tracks_persists_each_type_and_invalid_arguments_leave_project_unchanged() {
    let root = std::env::temp_dir().join(format!("daw-cli-add-{}", Id::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut session = Session::default();
    session.project.add_track().unwrap();
    let original = session.project.clone();
    session.save(&root).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_daw-cli"))
            .args(args)
            .current_dir(&root)
            .env("DISPLAY", "")
            .env("WAYLAND_DISPLAY", "")
            .output()
            .unwrap()
    };
    for (index, (kind, expected)) in [
        ("audio", TrackKind::Audio),
        ("midi", TrackKind::Midi),
        ("beat", TrackKind::Beat),
    ]
    .into_iter()
    .enumerate()
    {
        let output = run(&["tracks", "--add", "New track ü", "--type", kind]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Added track:"));
        let saved = daw_project::inspect_project(&root).unwrap();
        assert_eq!(saved.tracks.len(), index + 2);
        assert_eq!(saved.project_id, original.project_id);
        assert_eq!(saved.tracks[0], original.tracks[0]);
        let added = saved.tracks.last().unwrap();
        assert_eq!(added.name, "New track ü");
        assert_eq!(added.kind, expected);
        assert!(added.clips.is_empty());
        assert!(added.group_id.is_none());
        assert_ne!(added.id, original.tracks[0].id);
        saved.validate().unwrap();
    }
    let inspected = run(&["tracks", "New track ü"]);
    assert!(inspected.status.success());
    assert_eq!(
        String::from_utf8_lossy(&inspected.stdout)
            .matches("Track: ")
            .count(),
        3
    );
    let manifest = root.join("project.json");
    let before = fs::read(&manifest).unwrap();
    for args in [
        vec!["tracks", "--add", "Name"],
        vec!["tracks", "--type", "audio"],
        vec!["tracks", "--add", "Name", "--type", "return"],
        vec!["tracks", "Existing", "--add", "Name", "--type", "audio"],
        vec!["tracks", "--add", " ", "--type", "audio"],
        vec!["tracks", "--add", "Bad\nname", "--type", "audio"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2), "{args:?}");
        assert_eq!(fs::read(&manifest).unwrap(), before);
    }
    let missing = run(&[
        "tracks",
        "--add",
        "Name",
        "--type",
        "audio",
        "--project",
        "not-a-project",
    ]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(!root.join("not-a-project").exists());
}
