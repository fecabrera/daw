use std::{fs, process::Command};
#[test]
fn listening_pack_is_read_only_blinded_and_refuses_overwrite_or_source_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let sources = temp.path().join("sources");
    fs::create_dir(&sources).unwrap();
    let input = sources.join("original.wav");
    let mut writer = hound::WavWriter::create(
        &input,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for i in 0..48000 {
        for hz in [440.0, 660.0] {
            writer
                .write_sample((std::f32::consts::TAU * hz * i as f32 / 48000.0).sin() * 0.2)
                .unwrap();
        }
    }
    writer.finalize().unwrap();
    let original = fs::read(&input).unwrap();
    let modified = fs::metadata(&input).unwrap().modified().unwrap();
    let manifest = temp.path().join("corpus.json");
    fs::write(&manifest,serde_json::to_vec(&serde_json::json!([{"id":"test","path":input,"rights":"independently generated test signal"}])).unwrap()).unwrap();
    let output = temp.path().join("listen");
    let run = |out: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_transient_listen"))
            .args([
                "--manifest",
                manifest.to_str().unwrap(),
                "--output-dir",
                out.to_str().unwrap(),
                "--ratios",
                "3/2",
                "--seconds",
                "1",
            ])
            .output()
            .unwrap()
    };
    let result = run(&output);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("private/report.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "awaiting human listening");
    assert_eq!(report["engine_revision"], timestretch::REVISION);
    assert_eq!(
        report["sources"][0]["cases"][0]["renders"][0]["frames"],
        72000
    );
    assert!(output.join("audition/case-00-00-A.wav").is_file());
    assert!(output.join("audition/case-00-00-B.wav").is_file());
    let ratings = fs::read_to_string(output.join("ratings.csv")).unwrap();
    assert_eq!(ratings.lines().count(), 3);
    assert!(ratings.contains("case-00-00,,A,,,,,,,"));
    let key = fs::read(output.join("private/key.json")).unwrap();
    assert!(!run(&output).status.success());
    assert_eq!(fs::read(output.join("private/key.json")).unwrap(), key);
    assert!(!run(&sources.join("must-not-create")).status.success());
    assert!(!sources.join("must-not-create").exists());
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::metadata(&input).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read_dir(sources).unwrap().count(), 1);
}
