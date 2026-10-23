use daw_media::{
    AacBitrate, AudioEncoder, ExportEncoder, ExportFormat, ExportSettings, decode_audio_at_rate,
};
use std::path::PathBuf;

struct Output(PathBuf);
impl Output {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("daw-aac-export-{}.aac", daw_core::Id::new_v4())))
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn encode(samples: &[[f32; 2]], rate: u32, bitrate: AacBitrate, block: usize) -> Vec<u8> {
    let file = Output::new();
    let mut encoder = ExportEncoder::at_sample_rate(
        &file.0,
        ExportSettings {
            format: ExportFormat::Aac,
            aac_bitrate: bitrate,
            ..Default::default()
        },
        rate,
    )
    .unwrap();
    for chunk in samples.chunks(block) {
        encoder.write_frames(chunk).unwrap();
    }
    encoder.finish().unwrap();
    std::fs::read(&file.0).unwrap()
}
#[test]
fn aac_exports_are_decodable_deterministic_block_independent_and_rate_correct() {
    for rate in daw_core::SAMPLE_RATES {
        let source: Vec<_> = (0..rate / 10)
            .map(|i| {
                [
                    0.25 * (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin(),
                    0.125 * (i as f32 * 880.0 * std::f32::consts::TAU / rate as f32).sin(),
                ]
            })
            .collect();
        for bitrate in AacBitrate::ALL {
            let bytes = encode(&source, rate, bitrate, 512);
            assert_eq!(bytes, encode(&source, rate, bitrate, 137));
            let file = Output::new();
            std::fs::write(&file.0, &bytes).unwrap();
            let output_rate = rate.min(48_000);
            let decoded = decode_audio_at_rate(&file.0, output_rate).unwrap();
            assert_eq!(decoded.metadata.sample_rate_hz, output_rate);
            assert_eq!(decoded.metadata.channels, 2);
            let frames =
                daw_media::resampled_frames(source.len() as u64, rate, output_rate).unwrap();
            assert_eq!(
                decoded.samples.len() as u64,
                (frames.div_ceil(1024) + 1) * 1024
            );
            assert!(decoded.samples.iter().flatten().all(|s| s.is_finite()));
            let energy = |ch: usize| {
                decoded
                    .samples
                    .iter()
                    .map(|s| f64::from(s[ch]).powi(2))
                    .sum::<f64>()
            };
            assert!(energy(0) > 2.0 * energy(1));
            assert!(energy(1) > decoded.samples.len() as f64 * 0.002);
        }
    }
}
#[test]
fn aac_handles_partial_blocks_silence_clipping_and_nonfinite_input() {
    for length in [0, 1, 1023, 1024, 1025] {
        let bytes = encode(&vec![[0.0; 2]; length], 48_000, AacBitrate::Kbps192, 32);
        if length == 0 {
            assert!(bytes.is_empty());
        } else {
            let file = Output::new();
            std::fs::write(&file.0, bytes).unwrap();
            let audio = decode_audio_at_rate(&file.0, 48_000).unwrap();
            assert!(audio.samples.iter().flatten().all(|s| s.abs() < 1e-6));
        }
    }
    let file = Output::new();
    let mut encoder = ExportEncoder::at_sample_rate(
        &file.0,
        ExportSettings {
            format: ExportFormat::Aac,
            ..Default::default()
        },
        48_000,
    )
    .unwrap();
    assert!(encoder.write_frames(&[[f32::NAN, 0.0]]).is_err());
    encoder.write_frames(&[[2.0, -2.0]; 1024]).unwrap();
    assert!(encoder.clipped());
    encoder.finish().unwrap();
}
