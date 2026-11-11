use rubato::Resampler;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
// Bound known input/conversion buffers independently of the DSP request limit.
const AUDIO_MEMORY_LIMIT: u64 = 1024 * 1024 * 1024;

fn preflight(frames: u64, channels: u16, input_rate: u32, output_rate: u32) -> Result<()> {
    let limit = || "WAV conversion exceeds the 1 GiB audio memory limit";
    let input = frames
        .checked_mul(u64::from(channels) * 4)
        .ok_or_else(limit)?;
    let bytes = if input_rate == output_rate || frames == 0 {
        input
    } else {
        let (mut gcd, mut remainder) = (u64::from(input_rate), u64::from(output_rate));
        while remainder != 0 {
            (gcd, remainder) = (remainder, gcd % remainder);
        }
        let in_unit = u64::from(input_rate) / gcd;
        let out_unit = u64::from(output_rate) / gcd;
        // Pinned FftFixedIn uses ceil(512/in_unit) rational FFT blocks.
        // Include channel planes, transform planning/scratch and output overrun.
        let setup = (in_unit + out_unit)
            .checked_mul(512_u64.div_ceil(in_unit))
            .and_then(|n| n.checked_mul(256 * u64::from(channels)))
            .and_then(|n| n.checked_add(16 * 1024 * 1024))
            .ok_or_else(limit)?;
        let output = frames
            .checked_mul(u64::from(output_rate))
            .ok_or_else(limit)?
            .div_ceil(u64::from(input_rate));
        output
            .checked_mul(u64::from(channels) * 8)
            .and_then(|n| n.checked_add(input))
            .and_then(|n| n.checked_add(setup))
            .ok_or_else(limit)?
    };
    if bytes > AUDIO_MEMORY_LIMIT {
        return Err(limit().into());
    }
    Ok(())
}
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
    if !(1..=2).contains(&spec.channels) || spec.sample_rate == 0 || rate == 0 {
        return Err("expected mono/stereo WAV with positive sample rate".into());
    }
    preflight(
        u64::from(reader.duration()),
        spec.channels,
        spec.sample_rate,
        rate,
    )?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(reader.len() as usize)?;
    match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => {
            for sample in reader.samples::<f32>() {
                samples.push(sample?);
            }
        }
        (hound::SampleFormat::Int, bits @ (16 | 24)) => {
            for sample in reader.samples::<i32>() {
                samples.push(sample? as f32 / (1u32 << (bits - 1)) as f32);
            }
        }
        _ => return Err("supported WAV formats are PCM16, PCM24, and float32".into()),
    }
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
        if audio.samples.iter().any(|s| s.abs() > 1.0e12) {
            return Err("input exceeds safe FFT conversion headroom".into());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    thread_local! { static GUARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    struct GuardedAllocator;
    fn check_size(size: usize) {
        if size > 64 * 1024 * 1024 && GUARD.with(|g| g.get()) {
            // Stop a regression before the oversized allocation reaches the host.
            std::process::abort();
        }
    }
    // SAFETY: Layouts/pointers are forwarded unchanged to System. The test guard
    // terminates before a large allocation, and cannot return an invalid pointer.
    unsafe impl GlobalAlloc for GuardedAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            check_size(layout.size());
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            check_size(size);
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: GuardedAllocator = GuardedAllocator;
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            GUARD.with(|g| g.set(false));
        }
    }
    #[test]
    fn tiny_pathological_rate_is_rejected_before_large_allocation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("one-hz.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 1,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        writer.write_sample(1_i16).unwrap();
        writer.finalize().unwrap();
        GUARD.with(|g| g.set(true));
        let _guard = Guard;
        let error = read(&path, 192000).err().unwrap().to_string();
        assert!(error.contains("memory limit"), "{error}");
    }
    #[test]
    fn conversion_admission_checks_source_extent_and_all_supported_rates() {
        assert!(preflight(u64::MAX, 2, 48000, 48000).is_err());
        assert!(preflight(AUDIO_MEMORY_LIMIT / 8 + 1, 2, 48000, 48000).is_err());
        for input in timestretch::SAMPLE_RATES {
            for output in timestretch::SAMPLE_RATES {
                preflight(48000, 2, input, output).unwrap();
            }
        }
    }
    #[test]
    fn conversion_reports_extreme_headroom_and_preserves_ordinary_headroom() {
        let dir = tempfile::tempdir().unwrap();
        for value in [2.0_f32, f32::MAX * 0.5] {
            let path = dir.path().join("float.wav");
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44100,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .unwrap();
            for _ in 0..4096 {
                writer.write_sample(value).unwrap();
            }
            writer.finalize().unwrap();
            let same = read(&path, 44100).unwrap().0;
            assert!(same.samples.iter().all(|s| s.to_bits() == value.to_bits()));
            if value == 2.0 {
                let converted = read(&path, 48000).unwrap().0;
                assert!(converted.samples.iter().all(|s| s.is_finite()));
                assert!(converted.samples.iter().any(|s| *s > 1.9));
            } else {
                assert!(
                    read(&path, 48000)
                        .err()
                        .unwrap()
                        .to_string()
                        .contains("headroom")
                );
            }
        }
    }
}
