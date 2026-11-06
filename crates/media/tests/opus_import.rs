use daw_media::{decode_audio_at_rate, decode_audio_cached, decode_audio_cancellable};
use std::{path::PathBuf, sync::Arc};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn opus_import_preserves_exact_duration_channels_metadata_and_all_project_rates() {
    for channels in [1, 2] {
        let path = fixture(&format!("tone-{channels}.opus"));
        let source = decode_audio_at_rate(&path, 48_000).unwrap();
        assert_eq!(source.metadata.channels, channels);
        assert_eq!(source.metadata.sample_format, "opus");
        assert_eq!(source.metadata.sample_rate_hz, 48_000);
        assert_eq!(source.metadata.bits_per_sample, 0);
        assert_eq!(source.samples.len(), 12_000);
        assert!(source.samples.iter().flatten().all(|s| s.is_finite()));
        assert!(source.samples.iter().any(|s| s[0].abs() > 0.1));
        assert_eq!(source.samples.iter().all(|s| s[0] == s[1]), channels == 1);
        for rate in daw_core::SAMPLE_RATES {
            let audio = decode_audio_cached(&path, rate, &|| false).unwrap();
            assert_eq!(audio.sample_rate_hz, rate);
            assert_eq!(
                audio.samples.len() as u64,
                daw_media::resampled_frames(12_000, 48_000, rate).unwrap()
            );
            assert!(!audio.peaks.is_empty());
            let again = decode_audio_cached(&path, rate, &|| false).unwrap();
            assert!(Arc::ptr_eq(&audio.samples, &again.samples));
        }
    }
}

#[test]
fn opus_import_trims_short_and_multi_page_files_and_applies_header_gain() {
    for (name, length) in [("short.opus", 144), ("long.opus", 60_000)] {
        let audio = decode_audio_at_rate(&fixture(name), 48_000).unwrap();
        assert_eq!(audio.samples.len(), length, "{name}");
    }
    let original = decode_audio_at_rate(&fixture("tone-2.opus"), 48_000).unwrap();
    let delayed = decode_audio_at_rate(&fixture("delayed.opus"), 48_000).unwrap();
    assert_eq!(*delayed.samples, original.samples[1920..]);
    let gained = decode_audio_at_rate(&fixture("gain.opus"), 48_000).unwrap();
    assert_eq!(gained.samples.len(), original.samples.len());
    let gain = 10.0_f32.powf(6.0 / 20.0);
    for (actual, expected) in gained
        .samples
        .iter()
        .flatten()
        .zip(original.samples.iter().flatten())
    {
        assert!((actual - expected * gain).abs() < 1e-6);
    }
}

#[test]
fn opus_import_rejects_truncation_chains_surround_and_obeys_cancellation() {
    let path = fixture("tone-2.opus");
    assert!(decode_audio_at_rate(&fixture("surround.opus"), 48_000).is_err());
    assert!(decode_audio_cancellable(&path, 48_000, &|| true).is_err());
    let calls = std::cell::Cell::new(0);
    assert!(
        decode_audio_cancellable(&path, 48_000, &|| {
            calls.set(calls.get() + 1);
            calls.get() > 3
        })
        .err()
        .unwrap()
        .to_string()
        .contains("cancelled")
    );
    let bytes = std::fs::read(&path).unwrap();
    let temp = std::env::temp_dir().join(format!("daw-opus-{}.ogg", daw_core::Id::new_v4()));
    for end in [0, 20, bytes.len() / 2, bytes.len() - 1] {
        std::fs::write(&temp, &bytes[..end]).unwrap();
        assert!(
            decode_audio_at_rate(&temp, 48_000).is_err(),
            "truncation at {end}"
        );
    }
    let mut corrupted = bytes.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    std::fs::write(&temp, corrupted).unwrap();
    assert!(decode_audio_at_rate(&temp, 48_000).is_err());
    let mut chained = bytes.clone();
    chained.extend_from_slice(&bytes);
    std::fs::write(&temp, chained).unwrap();
    assert!(decode_audio_at_rate(&temp, 48_000).is_err());
    std::fs::write(&temp, &bytes).unwrap();
    let audio = decode_audio_at_rate(&temp, 48_000).unwrap();
    assert_eq!(audio.samples.len(), 12_000);
    assert_eq!(
        daw_media::source_file_extension(&temp, &audio.metadata).unwrap(),
        "opus"
    );
    assert!(
        daw_media::check_prepared_source(&temp, &audio, &|| false)
            .unwrap()
            .matches_audio
    );
    std::fs::remove_file(temp).unwrap();
}

#[test]
fn opus_import_accepts_recorder_pages_without_an_end_of_stream_flag() {
    for rate in [44_100, 48_000, 96_000] {
        let audio = decode_audio_at_rate(&fixture("recorder.opus"), rate).unwrap();
        assert_eq!(audio.metadata.channels, 1);
        assert_eq!(
            audio.samples.len() as u64,
            daw_media::resampled_frames(102_408, 48_000, rate).unwrap()
        );
        assert!(
            audio
                .samples
                .iter()
                .all(|frame| frame[0] == frame[1] && frame[0].is_finite())
        );
    }
}
