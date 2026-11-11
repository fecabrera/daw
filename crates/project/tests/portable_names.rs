use daw_project::{Session, project_folder_name};

#[test]
fn windows_superscript_devices_are_rejected_before_both_save_paths_create_folders() {
    let root = std::env::temp_dir().join(format!("daw-portable-names-{}", daw_core::Id::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("Música source.wav");
    let mut writer = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..20 {
        writer.write_sample(100_i16).unwrap();
        writer.write_sample(-100_i16).unwrap();
    }
    writer.finalize().unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut session = Session::default();
    session.import(&source, None, 0).unwrap();
    let before = session.project.clone();
    let folder = session.folder.clone();
    for prefix in ["COM", "com", "CoM", "LPT", "lpt", "LpT"] {
        for digit in ["¹", "²", "³"] {
            for suffix in ["", ".session", ".tar.gz"] {
                let name = format!("{prefix}{digit}{suffix}");
                assert!(project_folder_name(&name).is_err(), "{name}");
                assert!(session.save_new(&root, &name).is_err());
                assert!(
                    session
                        .save_snapshot(None)
                        .unwrap()
                        .save_new(&root, &name)
                        .is_err()
                );
                assert_eq!(session.project, before);
                assert_eq!(session.folder, folder);
            }
        }
    }
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(std::fs::read(&source).unwrap(), original);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unicode_boundaries_and_classic_device_rejections_are_preserved() {
    for name in [
        "COM0",
        "COM10",
        "COM¹song",
        "xCOM¹",
        "LPT⁴",
        "LPT١",
        "Música 01",
        "音声",
        "¹",
    ] {
        assert_eq!(project_folder_name(name).unwrap(), name);
    }
    for name in [
        "COM1",
        "com2.wav",
        "LpT9.tar.gz",
        "NUL",
        "aux.txt",
        "COM¹",
        "com².wav",
        "LpT³.session",
    ] {
        assert!(project_folder_name(name).is_err());
        assert!(
            daw_core::validate_included_source_path(&format!("assets/sources/{name}")).is_err()
        );
    }
}
