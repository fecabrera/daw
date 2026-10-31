use daw_core::Id;
use daw_project::Session;
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("daw-background-document-{}", Id::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("source.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..480 {
            writer.write_sample(1000_i16).unwrap();
        }
        writer.finalize().unwrap();
        let mut session = Session::default();
        session.import(&path, None, 0).unwrap();
        session.save(&root).unwrap();
        Self(root)
    }
    fn document(&self) -> Session {
        Session::open_document(&self.0)
            .unwrap()
            .into_editor_session()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn document_opens_without_decoding_and_missing_sources_keep_their_warning_policy() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join("source.wav")).unwrap();
    let mut document = fixture.document();
    assert!(document.audio.is_empty());
    let project = document.project.clone();
    document
        .prepare_project_audio(&|| false, &|_, _| {})
        .unwrap();
    assert_eq!(document.project, project);
    assert!(document.audio.is_empty());
    assert!(
        document
            .warnings
            .iter()
            .any(|warning| warning.contains("Missing source"))
    );
}
#[test]
fn failed_media_completion_keeps_the_document_and_all_original_audio_maps() {
    let fixture = Fixture::new();
    let mut document = fixture.document();
    let mut bad = document.project.assets[0].clone();
    bad.id = Id::new_v4();
    bad.source.path = fixture.0.join("bad.wav").to_string_lossy().into_owned();
    bad.source.path_kind = "absolute".into();
    fs::write(&bad.source.path, b"invalid audio").unwrap();
    document.project.assets.push(bad);
    let project = document.project.clone();
    assert!(
        document
            .prepare_project_audio(&|| false, &|_, _| {})
            .is_err()
    );
    assert_eq!(document.project, project);
    assert!(document.audio.is_empty());
    assert!(document.sampler_audio.is_empty());
}
