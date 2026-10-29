use daw_media::{decode_audio_at_rate, decode_audio_cached, decode_audio_cancellable};
use std::{path::PathBuf, sync::Arc};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn collection_verification_preserves_compressed_container_identity() {
    for (extension, expected) in [("mp3", "mp3"), ("aac", "aac"), ("m4a", "mp4")] {
        let path = fixture(&format!("tone-2.{extension}"));
        let audio = decode_audio_at_rate(&path, 48000).unwrap();
        let result = daw_media::check_prepared_source(&path, &audio, &|| false).unwrap();
        assert!(result.matches_audio);
        assert_eq!(
            daw_media::source_file_extension(&path, &audio.metadata).unwrap(),
            expected
        );
        assert_eq!(
            result.revision.byte_length,
            std::fs::metadata(&path).unwrap().len()
        );
    }
}

#[test]
fn compressed_sources_decode_resample_keep_channels_and_share_cached_audio() {
    for channels in [1, 2] {
        for extension in ["mp3", "aac", "m4a"] {
            let path = fixture(&format!("tone-{channels}.{extension}"));
            let source = decode_audio_at_rate(&path, 44_100).unwrap();
            assert_eq!(source.metadata.channels, channels);
            assert_eq!(
                source.metadata.sample_format,
                if extension == "mp3" { "mp3" } else { "aac" }
            );
            assert_eq!(source.metadata.bits_per_sample, 0);
            assert_eq!(source.metadata.sample_rate_hz, 44_100);
            assert!(
                (11_025..=13_312).contains(&source.samples.len()),
                "{}: {}",
                path.display(),
                source.samples.len()
            );
            if extension == "mp3" {
                assert_eq!(source.samples.len(), 11_025);
            }
            assert!(source.samples.iter().flatten().all(|s| s.is_finite()));
            assert!(source.samples.iter().any(|s| s[0].abs() > 0.1));
            assert_eq!(source.samples.iter().all(|s| s[0] == s[1]), channels == 1);
            for rate in daw_core::SAMPLE_RATES {
                let audio = decode_audio_cached(&path, rate, &|| false).unwrap();
                assert_eq!(audio.sample_rate_hz, rate);
                assert_eq!(
                    audio.samples.len() as u64,
                    daw_media::resampled_frames(source.samples.len() as u64, 44_100, rate).unwrap()
                );
                assert!(!audio.peaks.is_empty());
                let again = decode_audio_cached(&path, rate, &|| false).unwrap();
                assert!(Arc::ptr_eq(&audio.samples, &again.samples));
            }
        }
    }
}

#[test]
fn cancellation_invalid_sources_and_misleading_extensions_are_handled() {
    let path = fixture("tone-2.mp3");
    assert!(
        decode_audio_cancellable(&path, 48_000, &|| true)
            .err()
            .unwrap()
            .to_string()
            .contains("cancelled")
    );
    let calls = std::cell::Cell::new(0);
    let error = decode_audio_cancellable(&path, 48_000, &|| {
        calls.set(calls.get() + 1);
        calls.get() > 3
    })
    .err()
    .unwrap();
    assert!(error.to_string().contains("cancelled"));
    assert!(decode_audio_at_rate(&path, 1).is_err());
    let temp = std::env::temp_dir().join(format!("daw-import-{}.wav", daw_core::Id::new_v4()));
    std::fs::copy(&path, &temp).unwrap();
    assert_eq!(
        decode_audio_at_rate(&temp, 48_000)
            .unwrap()
            .metadata
            .sample_format,
        "mp3"
    );
    std::fs::write(&temp, b"invalid compressed audio").unwrap();
    assert!(decode_audio_at_rate(&temp, 48_000).is_err());
    std::fs::remove_file(temp).unwrap();
}

#[test]
fn unsupported_channels_and_truncated_container_data_are_rejected() {
    assert!(decode_audio_at_rate(&fixture("surround.m4a"), 48_000).is_err());
    for extension in ["mp3", "m4a"] {
        let bytes = std::fs::read(fixture(&format!("tone-2.{extension}"))).unwrap();
        let temp = std::env::temp_dir().join(format!(
            "daw-truncated-{}.{extension}",
            daw_core::Id::new_v4()
        ));
        std::fs::write(&temp, &bytes[..bytes.len() / 2]).unwrap();
        assert!(decode_audio_at_rate(&temp, 48_000).is_err(), "{extension}");
        std::fs::remove_file(temp).unwrap();
    }
}
