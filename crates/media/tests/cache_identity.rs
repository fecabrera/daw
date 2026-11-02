use std::{cell::Cell, fs, path::Path};
fn wav(path: &Path, value: i16) {
    let mut w = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..480 {
        w.write_sample(value).unwrap();
    }
    w.finalize().unwrap();
}
#[test]
fn verified_source_bytes_must_match_cache_pcm() {
    daw_media::configure_preparation_cache(None);
    let dir = std::env::temp_dir().join(format!("daw-cache-identity-{}", daw_core::Id::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("source.wav");
    wav(&path, 1000);
    let calls = Cell::new(0);
    let first = daw_media::decode_audio_cached(&path, 48000, &|| {
        let i = calls.get() + 1;
        calls.set(i);
        if i == 8 {
            wav(&path, 2000);
            eprintln!("B installed at call {i}");
        }
        if i == 14 {
            wav(&path, 1000);
            eprintln!("A restored at call {i}");
        }
        false
    });
    eprintln!(
        "calls={} first={:?}",
        calls.get(),
        first.as_ref().map(|a| a.samples[0])
    );
    wav(&path, 1000);
    let retry = daw_media::decode_audio_cached(&path, 48000, &|| false).unwrap();
    let direct = daw_media::decode_audio_cancellable(&path, 48000, &|| false).unwrap();
    eprintln!(
        "retry={} direct={}",
        retry.samples[0][0], direct.samples[0][0]
    );
    fs::remove_dir_all(dir).unwrap();
    assert_eq!(
        retry.samples[0], direct.samples[0],
        "cache must agree with verified source"
    );
}

#[test]
fn restored_timestamp_must_not_accept_different_pcm() {
    daw_media::configure_preparation_cache(None);
    let dir = std::env::temp_dir().join(format!("daw-cache-binding-{}", daw_core::Id::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("source.wav");
    wav(&path, 3000);
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let calls = Cell::new(0);
    let prepared = daw_media::decode_audio_cached(&path, 48000, &|| {
        let i = calls.get() + 1;
        calls.set(i);
        if i == 8 {
            wav(&path, 4000);
        }
        if i == 14 {
            wav(&path, 3000);
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
        }
        false
    });
    let direct = daw_media::decode_audio_cancellable(&path, 48000, &|| false).unwrap();

    fs::remove_dir_all(dir).unwrap();
    if let Ok(prepared) = prepared {
        assert_eq!(
            prepared.samples[0], direct.samples[0],
            "accepted PCM must match source"
        );
    }
}
