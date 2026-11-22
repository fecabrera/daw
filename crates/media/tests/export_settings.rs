use daw_media::{
    AudioEncoder, ExportChannels, ExportEncoder, ExportFormat, ExportSettings, Mp3Bitrate,
    WavCodec, decode_audio_at_rate,
};
use std::path::PathBuf;

struct Output(PathBuf);
impl Output {
    fn new(format: ExportFormat) -> Self {
        Self(std::env::temp_dir().join(format!(
            "daw-export-settings-{}.{}",
            daw_core::Id::new_v4(),
            format.extension()
        )))
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn chosen_export_rates_and_channel_counts_are_written_to_each_file() {
    let source: Vec<_> = (0..4_801)
        .map(|i| {
            let sample = (i as f32 * 0.1).sin() * 0.25;
            [sample, -sample]
        })
        .collect();
    for project_rate in [48_000, 192_000] {
        for format in ExportFormat::ALL {
            for &rate in format.export_sample_rates() {
                for channels in ExportChannels::ALL {
                    let file = Output::new(format);
                    let settings = ExportSettings {
                        format,
                        channels,
                        sample_rate_hz: Some(rate),
                        wav_codec: WavCodec::Float32,
                        mp3_bitrate: Mp3Bitrate::Kbps320,
                        aac_bitrate: Mp3Bitrate::Kbps320,
                    };
                    let mut encoder =
                        ExportEncoder::at_sample_rate(&file.0, settings, project_rate).unwrap();
                    for chunk in source.chunks(137) {
                        encoder.write_frames(chunk).unwrap();
                    }
                    assert!(!encoder.finish_with_clipping().unwrap());
                    let decoded = decode_audio_at_rate(&file.0, rate).unwrap();
                    assert_eq!(decoded.metadata.sample_rate_hz, rate);
                    if channels == ExportChannels::Mono {
                        assert_eq!(decoded.metadata.channels, 1);
                        assert!(decoded.samples.iter().all(|frame| frame[0].abs() < 1e-6));
                    } else {
                        assert_eq!(decoded.metadata.channels, 2);
                        assert!(decoded.samples.iter().any(|frame| frame[0].abs() > 0.1));
                    }
                    if format == ExportFormat::Wav {
                        assert_eq!(
                            decoded.frame_count(),
                            daw_media::resampled_frames(source.len() as u64, project_rate, rate)
                                .unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn mono_wav_averages_both_channels_before_clipping() {
    let file = Output::new(ExportFormat::Wav);
    let settings = ExportSettings {
        channels: ExportChannels::Mono,
        wav_codec: WavCodec::Float32,
        ..Default::default()
    };
    let mut encoder = ExportEncoder::create(&file.0, settings).unwrap();
    encoder
        .write_frames(&[[1.5, -1.0], [0.0, 0.5], [-0.75, -0.25]])
        .unwrap();
    assert!(!encoder.finish_with_clipping().unwrap());
    let samples: Vec<_> = hound::WavReader::open(&file.0)
        .unwrap()
        .samples::<f32>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(samples, [0.25, 0.25, -0.5]);
}

#[test]
fn unsupported_explicit_rate_is_rejected_before_creating_output() {
    let file = Output::new(ExportFormat::Mp3);
    let settings = ExportSettings {
        format: ExportFormat::Mp3,
        sample_rate_hz: Some(96_000),
        ..Default::default()
    };
    assert!(ExportEncoder::create(&file.0, settings).is_err());
    assert!(!file.0.exists());
}
