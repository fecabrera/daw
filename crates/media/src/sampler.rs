//! Worker-owned selected region and pitch variants. No note-time processing.
use crate::{AudioData, AudioReservation, Error, PreparedStretch, Result, prepared_audio};
use daw_core::{Clip, ClipStretch, SamplerInput, SamplerSettings, StretchEngine};
use std::{collections::BTreeMap, sync::Arc};
use timestretch::Pitch;

#[derive(Clone)]
pub struct PreparedSampler {
    source: Arc<Vec<[f32; 2]>>,
    input: SamplerInput,
    start_frame: u64,
    end_frame: u64,
    selected: AudioData,
    variants: BTreeMap<i8, PreparedStretch>,
}

impl PreparedSampler {
    pub fn visit_memory(&self, usage: &mut crate::AudioMemoryUsage) {
        usage.samples(&self.source);
        usage.audio(&self.selected);
        for variant in self.variants.values() {
            variant.visit_memory(usage);
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        self.input.name.capacity() as u64
            + self.selected.metadata.sample_format.capacity() as u64
            + daw_unit_core::memory::tree_bytes::<i8, PreparedStretch>(self.variants.len())
            + self
                .variants
                .values()
                .map(PreparedStretch::heap_bytes)
                .sum::<u64>()
    }

    fn matches_region(&self, source: &AudioData, settings: &SamplerSettings) -> bool {
        Arc::ptr_eq(&self.source, &source.samples)
            && self.selected.sample_rate_hz == source.sample_rate_hz
            && settings.input.as_ref() == Some(&self.input)
            && self.start_frame == settings.start_frame
            && self.end_frame == settings.end_frame
    }

    pub fn matches(
        &self,
        source: &AudioData,
        settings: &SamplerSettings,
        output_frames: u64,
        engine: StretchEngine,
        notes: &[u8],
    ) -> bool {
        let Some(ratio) = settings
            .end_frame
            .checked_sub(settings.start_frame)
            .and_then(|count| ClipStretch::new(count, output_frames))
        else {
            return false;
        };
        self.matches_region(source, settings)
            && (settings.granulizer.is_none() || self.variants.len() == 1)
            && offsets(settings, notes).all(|offset| {
                self.variants
                    .get(&offset)
                    .is_some_and(|v| v.matches_pitch(&self.selected, ratio, engine, pitch(offset)))
            })
    }

    /// Root audio is always present for previews and duration/voice scheduling.
    pub fn root_audio(&self) -> &AudioData {
        &self.variants[&0].audio
    }

    pub fn variants(&self) -> impl Iterator<Item = (i8, &AudioData)> {
        self.variants.iter().map(|(&offset, v)| (offset, &v.audio))
    }

    /// Two independent pitch transforms at most. Root/region preparation remains shared.
    /// Serial fallback avoids extra working storage under pressure or thread-start failure.
    pub fn new_parallel(
        source: &AudioData,
        settings: &SamplerSettings,
        output_frames: u64,
        engine: StretchEngine,
        notes: &[u8],
        previous: Option<&Self>,
        cancelled: &(impl Fn() -> bool + Sync),
    ) -> Result<Self> {
        let mut bank = Self::new_cancellable(
            source,
            settings,
            output_frames,
            engine,
            &[],
            previous,
            cancelled,
        )?;
        let ratio =
            ClipStretch::new(settings.end_frame - settings.start_frame, output_frames).unwrap();
        let required: Vec<_> = offsets(settings, notes)
            .filter(|&offset| offset != 0)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let prepare = |offset: i8| -> Result<PreparedStretch> {
            if cancelled() {
                return Err(Error("Sample preparation cancelled".into()));
            }
            if let Some(cached) = previous
                .filter(|old| old.matches_region(source, settings))
                .and_then(|old| old.variants.get(&offset))
                .filter(|v| v.matches_pitch(&bank.selected, ratio, engine, pitch(offset)))
            {
                Ok(cached.clone())
            } else {
                PreparedStretch::with_pitch_cancellable(
                    &bank.selected,
                    ratio,
                    engine,
                    pitch(offset),
                    cancelled,
                )
            }
        };
        let parallel = required.len() >= 2
            && std::thread::available_parallelism().is_ok_and(|slots| slots.get() >= 2)
            && crate::audio_memory_bytes()
                .saturating_add(output_frames.saturating_mul(32))
                .saturating_add(64 * 1024 * 1024)
                < crate::AUDIO_MEMORY_BUDGET_BYTES;
        let results = if parallel {
            let attempted = std::thread::scope(|scope| -> Result<Vec<(i8, PreparedStretch)>> {
                let helper = std::thread::Builder::new()
                    .name("sampler-pitch".into())
                    .spawn_scoped(scope, || {
                        required
                            .iter()
                            .step_by(2)
                            .map(|&offset| prepare(offset).map(|audio| (offset, audio)))
                            .collect::<Result<Vec<_>>>()
                    });
                match helper {
                    Ok(helper) => {
                        let main = required
                            .iter()
                            .skip(1)
                            .step_by(2)
                            .map(|&offset| prepare(offset).map(|audio| (offset, audio)))
                            .collect::<Result<Vec<_>>>();
                        let other = helper.join().map_err(|_| {
                            Error("Sampler pitch worker stopped unexpectedly".into())
                        })?;
                        let mut all = main?;
                        all.extend(other?);
                        Ok(all)
                    }
                    Err(_) => required
                        .iter()
                        .map(|&offset| prepare(offset).map(|audio| (offset, audio)))
                        .collect(),
                }
            });
            match attempted {
                Ok(results) => results,
                Err(error) if error.0.contains("shared audio memory budget") => {
                    // Another worker can consume admission after the preflight.
                    // Join/drop all partial results before retrying with one transform.
                    crate::release_preparation_cache_memory();
                    required
                        .iter()
                        .map(|&offset| prepare(offset).map(|audio| (offset, audio)))
                        .collect::<Result<Vec<_>>>()?
                }
                Err(error) => return Err(error),
            }
        } else {
            required
                .iter()
                .map(|&offset| prepare(offset).map(|audio| (offset, audio)))
                .collect::<Result<Vec<_>>>()?
        };
        for (offset, audio) in results {
            bank.variants.insert(offset, audio);
        }
        if cancelled() {
            return Err(Error("Sample preparation cancelled".into()));
        }
        Ok(bank)
    }

    pub fn new_cancellable(
        source: &AudioData,
        settings: &SamplerSettings,
        output_frames: u64,
        engine: StretchEngine,
        notes: &[u8],
        previous: Option<&Self>,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        let check = || {
            if cancelled() {
                Err(Error("Sample preparation cancelled".into()))
            } else {
                Ok(())
            }
        };
        check()?;
        if !settings.valid() {
            return Err(Error("Invalid sampler settings".into()));
        }
        let input = settings
            .input
            .as_ref()
            .ok_or_else(|| Error("Missing sampler input".into()))?;
        let count = settings
            .end_frame
            .checked_sub(settings.start_frame)
            .filter(|count| *count > 0)
            .ok_or_else(|| Error("Invalid sampler region".into()))?;
        let ratio = ClipStretch::new(count, output_frames)
            .filter(|r| r.valid())
            .ok_or_else(|| Error("Unsupported sampler duration".into()))?;
        let previous = previous.filter(|p| p.matches_region(source, settings));
        let selected = if let Some(previous) = previous {
            previous.selected.clone()
        } else {
            crate::register_audio(source);
            let _reservation = AudioReservation::new(count, 0)?;
            let mut samples = Vec::new();
            samples
                .try_reserve_exact(usize::try_from(count).map_err(|e| Error(e.to_string()))?)
                .map_err(|e| Error(e.to_string()))?;
            let clip = Clip::from(input);
            for local in settings.start_frame..settings.end_frame {
                if local % 4096 == 0 {
                    check()?;
                }
                samples.push(
                    source
                        .samples
                        .get(clip.source_frame(local) as usize)
                        .copied()
                        .unwrap_or_default(),
                );
            }
            prepared_audio(source, samples)
        };
        let mut variants = BTreeMap::new();
        for offset in offsets(settings, notes) {
            check()?;
            if variants.contains_key(&offset) {
                continue;
            }
            let pitch = pitch(offset);
            let cached = previous
                .and_then(|p| p.variants.get(&offset))
                .filter(|v| v.matches_pitch(&selected, ratio, engine, pitch));
            let prepared = match cached {
                Some(cached) => cached.clone(),
                None => PreparedStretch::with_pitch_cancellable(
                    &selected, ratio, engine, pitch, cancelled,
                )?,
            };
            variants.insert(offset, prepared);
        }
        check()?;
        Ok(Self {
            source: source.samples.clone(),
            input: input.clone(),
            start_frame: settings.start_frame,
            end_frame: settings.end_frame,
            selected,
            variants,
        })
    }
}

fn offsets<'a>(settings: &'a SamplerSettings, notes: &'a [u8]) -> impl Iterator<Item = i8> + 'a {
    std::iter::once(0).chain(notes.iter().filter_map(|&note| {
        if settings.granulizer.is_some() {
            None
        } else {
            settings.pitch_offset(note)
        }
    }))
}
fn pitch(offset: i8) -> Pitch {
    // Offsets come only from the shared bounded Sampler mapping.
    Pitch::from_semitones(f64::from(offset)).unwrap()
}
