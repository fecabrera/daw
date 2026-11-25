use super::{BLOCK, HZ, MAX_BINS, Work, check_cancelled};
use crate::Error;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::{collections::VecDeque, ops::Range, sync::Arc};

const BAND_TOPS: [u32; 6] = [250, 600, 1_400, 3_200, 7_000, 20_000];

pub(super) struct Envelope {
    pub smooth: Vec<f64>,
    pub flux: Vec<f64>,
    pub rms: Vec<f64>,
    pub balanced: Option<BandEvidence>,
}

pub(super) struct BandEvidence {
    pub smooth: Vec<f64>,
    pub flux: Vec<f64>,
}

pub(super) struct Samples {
    rate: u32,
    frames: u64,
    first: u64,
    cache: VecDeque<[f32; 2]>,
    block: Vec<[f32; 2]>,
}

impl Samples {
    pub fn new(rate: u32, frames: u64) -> Self {
        Self {
            rate,
            frames,
            first: 0,
            cache: VecDeque::with_capacity(rate as usize / 3 + BLOCK),
            block: vec![[0.0; 2]; BLOCK],
        }
    }

    fn fill<E: From<Error>>(
        &mut self,
        wanted: Range<i128>,
        read_end: u64,
        cancelled: &impl Fn() -> bool,
        read: &mut impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
    ) -> Result<(), E> {
        let start = wanted.start.clamp(0, i128::from(self.frames)) as u64;
        let end = wanted.end.clamp(0, i128::from(self.frames)) as u64;
        let mut available = self.first + self.cache.len() as u64;
        if start > available || start < self.first {
            self.cache.clear();
            self.first = start;
            available = start;
        }
        let retain = start.saturating_sub(u64::from(self.rate) / 4);
        let discard = retain.saturating_sub(self.first) as usize;
        self.cache.drain(..discard);
        self.first += discard as u64;
        while available < end {
            check_cancelled(cancelled)?;
            let count = read_end.saturating_sub(available).min(BLOCK as u64) as usize;
            if count == 0 {
                return Err(Error::InvalidLayout.into());
            }
            read(available, &mut self.block[..count])?;
            if self.block[..count]
                .iter()
                .flatten()
                .any(|sample| !sample.is_finite())
            {
                return Err(Error::NonfiniteInput.into());
            }
            self.cache.extend(&self.block[..count]);
            available += count as u64;
        }
        Ok(())
    }

    fn sample(&self, frame: i128) -> [f32; 2] {
        if frame < 0 || frame >= i128::from(self.frames) {
            [0.0; 2]
        } else {
            self.cache[(frame as u64 - self.first) as usize]
        }
    }
}

pub(super) struct Spectrum {
    rate: u32,
    fft: Arc<dyn Fft<f32>>,
    input: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    taper: Vec<f32>,
    gain: f64,
    last_bin: usize,
    bands: Option<Vec<usize>>,
}

impl Spectrum {
    pub fn new(rate: u32, bands: bool) -> Result<Self, Error> {
        let width = (f64::from(rate) * 1024.0 / 44100.0).round() as usize;
        let size = width.next_power_of_two();
        if size > 8192 {
            return Err(Error::ResourceLimit);
        }
        let taper: Vec<_> = (0..width)
            .map(|i| {
                (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (width - 1) as f64).cos()) as f32
            })
            .collect();
        let gain = 2.0_f64.sqrt() / taper.iter().map(|&w| f64::from(w)).sum::<f64>();
        let fft = FftPlanner::<f32>::new().plan_fft_forward(size);
        let last_bin = (20_000 * size / rate as usize).min(size / 2 - 1);
        let bands = bands.then(|| {
            (0..=last_bin)
                .map(|bin| {
                    let frequency = bin * rate as usize / size;
                    BAND_TOPS
                        .partition_point(|&top| (top as usize) < frequency)
                        .min(BAND_TOPS.len() - 1)
                })
                .collect()
        });
        Ok(Self {
            rate,
            scratch: vec![Complex::default(); fft.get_inplace_scratch_len()],
            input: vec![Complex::default(); size],
            taper,
            gain,
            fft,
            last_bin,
            bands,
        })
    }

    pub fn measure<E: From<Error>>(
        &mut self,
        pcm: &mut Samples,
        range: Range<u64>,
        work: &mut Work,
        cancelled: &impl Fn() -> bool,
        read: &mut impl FnMut(u64, &mut [[f32; 2]]) -> Result<(), E>,
    ) -> Result<Envelope, E> {
        let hop = u64::from(self.rate / HZ);
        let bins = (range.end - range.start).div_ceil(hop) as usize;
        if bins > MAX_BINS {
            return Err(Error::ResourceLimit.into());
        }
        let context = 8;
        let count = bins + 2 * context;
        let read_end = range
            .end
            .saturating_add(u64::from(self.rate / 10))
            .min(pcm.frames);
        let mut flux = Vec::with_capacity(count);
        let mut rms = Vec::with_capacity(bins);
        let mut previous = vec![0.0_f64; self.last_bin + 1];
        let mut bands = self.bands.as_ref().map(|_| Vec::with_capacity(count));
        for offset in 0..=count {
            if offset % 16 == 0 {
                check_cancelled(cancelled)?;
            }
            work.fft()?;
            work.visit(self.taper.len() + self.last_bin)?;
            // Successive transforms straddle each flux timestamp by half a hop.
            let center = i128::from(range.start)
                + (offset as i128 - context as i128) * i128::from(hop)
                - i128::from(hop / 2);
            let left = center - self.taper.len() as i128 / 2;
            let right = left + self.taper.len() as i128;
            pcm.fill(left..right, read_end, cancelled, &mut |start, out| {
                let clock = work.start();
                let result = read(start, out);
                work.finish(super::Stage::Read, clock);
                if let Some(profile) = &mut work.profile {
                    profile.read_calls += 1;
                    profile.read_frames += out.len();
                }
                result
            })?;
            let mut power = 0.0_f64;
            for (i, (cell, &weight)) in self.input.iter_mut().zip(&self.taper).enumerate() {
                let [a, b] = pcm.sample(left + i as i128);
                let a = a.clamp(-1e6, 1e6);
                let b = b.clamp(-1e6, 1e6);
                power += f64::from(a) * f64::from(a) + f64::from(b) * f64::from(b);
                *cell = Complex::new(a * weight, b * weight);
            }
            self.input[self.taper.len()..].fill(Complex::default());
            self.fft
                .process_with_scratch(&mut self.input, &mut self.scratch);
            let mut band_flux = [0.0_f64; BAND_TOPS.len()];
            let strength = if let Some(band_ids) = &self.bands {
                work.visit(self.last_bin)?;
                self.spectral_flux(&mut previous, |bin, positive| {
                    band_flux[band_ids[bin]] += positive;
                })
            } else {
                self.spectral_flux(&mut previous, |_, _| {})
            };
            if offset > 0 {
                flux.push(strength);
                if let Some(bands) = &mut bands {
                    bands.push(band_flux);
                }
            }
            if offset > context && offset <= context + bins {
                rms.push((power / (2 * self.taper.len()) as f64).sqrt());
            }
        }
        let mut filter: [f64; 15] = std::array::from_fn(|i| {
            let x = i as f64 - 7.0;
            let sinc = if x == 0.0 {
                0.14
            } else {
                (0.14 * std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
            };
            sinc * (0.54 - 0.46 * (std::f64::consts::TAU * i as f64 / 14.0).cos())
        });
        let sum = filter.iter().sum::<f64>();
        for coefficient in &mut filter {
            *coefficient /= sum;
        }
        let smooth = smooth(&flux, context..context + bins, &filter);
        let balanced = if let Some(bands) = &bands {
            work.visit(bands.len() * BAND_TOPS.len() * 4 + bins * filter.len())?;
            let mut prefix = Vec::with_capacity(count + 1);
            prefix.push([0.0; BAND_TOPS.len()]);
            for (i, frame) in bands.iter().enumerate() {
                prefix.push(std::array::from_fn(|band| prefix[i][band] + frame[band]));
            }
            let mut weighted = Vec::with_capacity(count);
            for (i, frame) in bands.iter().enumerate() {
                if i % 32 == 0 {
                    check_cancelled(cancelled)?;
                }
                let lo = i.saturating_sub(48);
                let hi = (i + 49).min(count);
                let weight = if flux[i] >= 0.006 {
                    frame
                        .iter()
                        .enumerate()
                        .map(|(band, value)| {
                            let mean = (prefix[hi][band] - prefix[lo][band]) / (hi - lo) as f64;
                            (value / (0.022 + 2.5 * mean)).min(2.25)
                        })
                        .sum()
                } else {
                    0.0
                };
                weighted.push(weight);
            }
            Some(BandEvidence {
                smooth: self::smooth(&weighted, context..context + bins, &filter),
                flux: weighted[context..context + bins].to_vec(),
            })
        } else {
            None
        };
        Ok(Envelope {
            smooth,
            flux: flux[context..context + bins].to_vec(),
            rms,
            balanced,
        })
    }

    fn spectral_flux(&self, previous: &mut [f64], mut add_band: impl FnMut(usize, f64)) -> f64 {
        let mut strength = 0.0;
        for (bin, old) in previous.iter_mut().enumerate().skip(1) {
            let a = self.input[bin].norm_sqr();
            let b = self.input[self.input.len() - bin].norm_sqr();
            // The conjugate pair carries both channel powers in the packed FFT.
            let magnitude = f64::from((a + b).sqrt()) * self.gain * 0.5;
            let level = if magnitude >= 0.0002 {
                (100.0 * magnitude).ln_1p() / 101.0_f64.ln()
            } else {
                0.0
            };
            let positive = (level - *old).max(0.0);
            strength += positive;
            add_band(bin, positive);
            *old = level;
        }
        strength
    }
}

fn smooth(flux: &[f64], bins: Range<usize>, filter: &[f64; 15]) -> Vec<f64> {
    bins.map(|i| {
        flux[i - 7..=i + 7]
            .iter()
            .zip(filter)
            .map(|(v, w)| v * w)
            .sum::<f64>()
            .max(0.0)
    })
    .collect()
}
