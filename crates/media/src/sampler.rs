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
    std::iter::once(0).chain(notes.iter().filter_map(|&note| settings.pitch_offset(note)))
}
fn pitch(offset: i8) -> Pitch {
    // Offsets come only from the shared bounded Sampler mapping.
    Pitch::from_semitones(f64::from(offset)).unwrap()
}
