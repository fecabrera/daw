use rubato::Resampler;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub struct Audio {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
}
impl Audio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }
    pub fn hash(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"timestretch-f32-le-v1");
        hash.update(self.rate.to_le_bytes());
        hash.update((self.channels as u64).to_le_bytes());
        hash.update((self.frames() as u64).to_le_bytes());
        for s in &self.samples {
            hash.update(s.to_bits().to_le_bytes());
        }
        format!("{:x}", hash.finalize())
    }
}
pub fn file_hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut block = [0; 65536];
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        hash.update(&block[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn read(path: &Path, rate: u32) -> Result<(Audio, f64, f64)> {
    let started = Instant::now();
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if !(1..=2).contains(&spec.channels) || spec.sample_rate == 0 {
        return Err("expected mono/stereo WAV with positive sample rate".into());
    }
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
        (hound::SampleFormat::Int, bits @ (16 | 24)) => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / (1u32 << (bits - 1)) as f32))
            .collect::<std::result::Result<_, _>>()?,
        _ => return Err("supported WAV formats are PCM16, PCM24, and float32".into()),
    };
    if samples.iter().any(|s| !s.is_finite()) {
        return Err("input contains NaN or infinity".into());
    }
    let channels = usize::from(spec.channels);
    if !samples.len().is_multiple_of(channels) {
        return Err("incomplete WAV frame".into());
    }
    let decode = started.elapsed().as_secs_f64();
    let start_convert = Instant::now();
    let mut audio = Audio {
        samples,
        channels,
        rate: spec.sample_rate,
    };
    if rate != audio.rate && audio.frames() > 0 {
        let mut converter =
            rubato::FftFixedIn::<f32>::new(audio.rate as usize, rate as usize, 1024, 2, channels)?;
        let block_size = converter.input_frames_next();
        let delay = converter.output_delay();
        let target = usize::try_from(
            (audio.frames() as u128 * u128::from(rate)).div_ceil(u128::from(audio.rate)),
        )?;
        let mut output = Vec::new();
        output.try_reserve_exact(
            (target + delay + converter.output_frames_max())
                .checked_mul(channels)
                .ok_or("converted source too large")?,
        )?;
        let mut position = 0;
        while output.len() / channels < target + delay {
            let mut planes = vec![vec![0.0; block_size]; channels];
            for (c, plane) in planes.iter_mut().enumerate() {
                for (i, value) in plane.iter_mut().enumerate() {
                    *value = audio
                        .samples
                        .get((position + i) * channels + c)
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            let converted = converter.process(&planes, None)?;
            for i in 0..converted[0].len() {
                for plane in &converted {
                    output.push(plane[i]);
                }
            }
            position += block_size;
        }
        audio.samples = output[delay * channels..(delay + target) * channels].to_vec();
    }
    audio.rate = rate;
    if audio.samples.iter().any(|s| !s.is_finite()) {
        return Err("conversion produced invalid audio".into());
    }
    Ok((audio, decode, start_convert.elapsed().as_secs_f64()))
}
pub fn write(path: &Path, audio: &Audio) -> Result<()> {
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: audio.channels as u16,
            sample_rate: audio.rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    for s in &audio.samples {
        writer.write_sample(*s)?;
    }
    writer.finalize()?;
    Ok(())
}
