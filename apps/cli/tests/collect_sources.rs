use daw_media::{AudioEncoder, WavCodec, WavEncoder};
use daw_project::Session;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Fixture {
    root: PathBuf,
    project: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("daw-cli-collect-{}", daw_core::Id::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("Original café.wav");
        let mut encoder = WavEncoder::at_sample_rate(&source, WavCodec::Float32, 48000).unwrap();
        encoder.write_frames(&vec![[0.2, -0.1]; 4800]).unwrap();
        encoder.finish().unwrap();
        let project = root.join("Saved project");
        let mut session = Session::default();
        session.import(&source, None, 0).unwrap();
        session.save(&project).unwrap();
        Self {
            root,
            project,
            source,
        }
    }
    fn run(&self, args: &[&std::ffi::OsStr], cwd: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_daw-cli"))
            .args(args)
            .current_dir(cwd)
            .env("DISPLAY", "")
            .env("WAYLAND_DISPLAY", "")
            .output()
            .unwrap()
    }
    fn collect(&self, dry_run: bool) -> Output {
        let mut args = vec![
            std::ffi::OsStr::new("collect-sources"),
            std::ffi::OsStr::new("--project"),
            self.project.as_os_str(),
        ];
        if dry_run {
            args.push(std::ffi::OsStr::new("--dry-run"));
        }
        self.run(&args, &self.root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn dry_run_is_read_only_and_collection_survives_original_removal_and_folder_move() {
    let f = Fixture::new();
    let original = fs::read(&f.source).unwrap();
    let manifest = fs::read(f.project.join("project.json")).unwrap();
    let before = f.root.join("before.wav");
    success(&f.run(
        &[
            "render".as_ref(),
            "--project".as_ref(),
            f.project.as_os_str(),
            "--output".as_ref(),
            before.as_os_str(),
        ],
        &f.root,
    ));
    let dry = f.collect(true);
    success(&dry);
    assert!(String::from_utf8_lossy(&dry.stdout).contains("Files to copy: 1"));
    assert!(String::from_utf8_lossy(&dry.stdout).contains("project unchanged"));
    assert_eq!(fs::read(f.project.join("project.json")).unwrap(), manifest);
    assert_eq!(fs::read_dir(f.project.join("assets")).unwrap().count(), 0);
    let copied = f.run(&["collect-sources".as_ref()], &f.project);
    success(&copied);
    assert!(String::from_utf8_lossy(&copied.stdout).contains("Collected files: 1"));
    assert_eq!(fs::read(&f.source).unwrap(), original);
    let included = daw_project::inspect_project(&f.project).unwrap();
    assert!(included.assets[0].source.is_included_recording());
    let relative = included.assets[0].source.path.clone();
    assert_eq!(fs::read(f.project.join(&relative)).unwrap(), original);
    fs::rename(&f.source, f.root.join("offline-original.wav")).unwrap();
    let moved = f.root.join("移動 project");
    fs::rename(&f.project, &moved).unwrap();
    success(&f.run(
        &["validate".as_ref(), "--project".as_ref(), moved.as_os_str()],
        &f.root,
    ));
    let after = f.root.join("after.wav");
    success(&f.run(
        &[
            "render".as_ref(),
            "--project".as_ref(),
            moved.as_os_str(),
            "--output".as_ref(),
            after.as_os_str(),
        ],
        &f.root,
    ));
    assert_eq!(fs::read(before).unwrap(), fs::read(after).unwrap());
    let repeated = f.run(
        &[
            "collect-sources".as_ref(),
            "--project".as_ref(),
            moved.as_os_str(),
        ],
        &f.root,
    );
    success(&repeated);
    assert!(String::from_utf8_lossy(&repeated.stdout).contains("Collected files: 0"));
    assert_eq!(
        daw_project::inspect_project(&moved).unwrap().assets[0]
            .source
            .path,
        relative
    );
    assert_eq!(
        fs::read_dir(moved.join("assets/sources")).unwrap().count(),
        1
    );
}
#[test]
fn missing_source_blocks_the_entire_collection_including_verified_files() {
    let f = Fixture::new();
    let missing = f.root.join("Missing.wav");
    fs::copy(&f.source, &missing).unwrap();
    let mut session = Session::open(&f.project).unwrap();
    session.import(&missing, None, 0).unwrap();
    session.save(&f.project).unwrap();
    fs::remove_file(missing).unwrap();
    let manifest = fs::read(f.project.join("project.json")).unwrap();
    for dry_run in [false, true] {
        let output = f.collect(dry_run);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Missing.wav") && stderr.contains("Collection blocked"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Project sources collected"));
        assert_eq!(fs::read(f.project.join("project.json")).unwrap(), manifest);
        assert_eq!(fs::read_dir(f.project.join("assets")).unwrap().count(), 0);
    }
}
#[test]
fn help_invalid_arguments_and_missing_project_follow_cli_exit_codes() {
    let f = Fixture::new();
    let help = f.run(&["collect-sources".as_ref(), "--help".as_ref()], &f.root);
    success(&help);
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(
        text.contains("--dry-run")
            && text.contains("--project")
            && text.contains("close the desktop")
    );
    assert_eq!(
        f.run(&["collect-sources".as_ref(), "--unknown".as_ref()], &f.root)
            .status
            .code(),
        Some(2)
    );
    let absent = f.root.join("absent");
    assert_eq!(
        f.run(
            &[
                "collect-sources".as_ref(),
                "--project".as_ref(),
                absent.as_os_str()
            ],
            &f.root
        )
        .status
        .code(),
        Some(1)
    );
    assert!(!absent.exists());
    fs::write(f.project.join("project.json"), b"invalid json").unwrap();
    assert_eq!(f.collect(false).status.code(), Some(1));
    assert_eq!(
        fs::read(f.project.join("project.json")).unwrap(),
        b"invalid json"
    );
}
