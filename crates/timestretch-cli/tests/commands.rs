use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn wav(path: &Path) {
    let mut out = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for i in 0..24000 {
        let s = (i as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.5;
        out.write_sample(s).unwrap();
        out.write_sample(-s).unwrap();
    }
    out.finalize().unwrap();
}
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_timestretch"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn process_paths_identity_ratios_and_failures() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    wav(&dir.join("input $ literal.wav"));
    let before = fs::read(dir.join("input $ literal.wav")).unwrap();
    let args = [
        "process",
        "--input",
        "input $ literal.wav",
        "--output",
        "out.wav",
        "--report",
        "out.json",
        "--engine",
        "timestretch",
        "--ratio",
        "1.5",
    ];
    let success = run(dir, &args);
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(dir.join("out.json")).unwrap()).unwrap();
    assert_eq!(report["output_frames"], 36000);
    assert_eq!(report["channels"], 2);
    assert_eq!(report["ratio"]["numerator"], 3);
    assert_eq!(run(dir, &args).status.code(), Some(2));
    let mut fraction = args;
    fraction[4] = "fraction.wav";
    fraction[6] = "fraction.json";
    fraction[10] = "3/2";
    assert!(run(dir, &fraction).status.success());
    assert_eq!(
        fs::read(dir.join("out.wav")).unwrap(),
        fs::read(dir.join("fraction.wav")).unwrap()
    );
    let mut invalid = args;
    invalid[10] = "1/9";
    assert_eq!(run(dir, &invalid).status.code(), Some(2));
    fs::hard_link(dir.join("input $ literal.wav"), dir.join("alias.wav")).unwrap();
    let mut alias = args.to_vec();
    alias[4] = "alias.wav";
    alias.push("--overwrite");
    assert_eq!(run(dir, &alias).status.code(), Some(2));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.join("input $ literal.wav"), dir.join("symlink.wav"))
            .unwrap();
        alias[4] = "symlink.wav";
        assert_eq!(run(dir, &alias).status.code(), Some(2));
    }
    assert_eq!(fs::read(dir.join("input $ literal.wav")).unwrap(), before);
}
#[test]
fn comparison_requires_all_three_engines_and_matching_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    wav(&dir.join("source.wav"));
    let result = run(
        dir,
        &[
            "compare",
            "--input",
            "source.wav",
            "--ratio",
            "3/2",
            "--output-dir",
            "comparison",
        ],
    );
    let summary: Value =
        serde_json::from_slice(&fs::read(dir.join("comparison/comparison.json")).unwrap()).unwrap();
    if cfg!(feature = "rubberband-reference") {
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(summary["reports"].as_array().unwrap().len(), 3);
        let reports = summary["reports"].as_array().unwrap();
        for report in reports {
            assert_eq!(report["prepared_sha256"], reports[0]["prepared_sha256"]);
            assert_eq!(report["output_frames"], 36000);
        }
        assert_eq!(reports[1]["backend"]["engine_version"], 2);
        assert_eq!(reports[2]["backend"]["engine_version"], 3);
    } else {
        assert!(!result.status.success());
        assert_eq!(summary["status"], "failed");
        assert_eq!(summary["failures"].as_array().unwrap().len(), 2);
    }
    assert_eq!(
        run(
            dir,
            &[
                "compare",
                "--input",
                "source.wav",
                "--ratio",
                "1",
                "--output-dir",
                "comparison"
            ]
        )
        .status
        .code(),
        Some(2)
    );
}
