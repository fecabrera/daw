//! Prepared non-overlapping ranges. Saved clip order is independent of playback order.
use super::RenderPlan;
use daw_core::Track;
use daw_media::AudioData;

#[derive(Clone)]
pub(super) struct Ranges {
    sorted: Vec<(u64, u64, usize)>,
    window: (u64, u64, Option<usize>),
}
impl Ranges {
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.sorted)
    }

    pub fn new(ranges: impl Iterator<Item = (u64, u64)>) -> Self {
        let mut sorted: Vec<_> = ranges.enumerate().map(|(i, (a, b))| (a, b, i)).collect();
        sorted.sort_unstable_by_key(|r| r.0);
        Self {
            sorted,
            window: (0, 0, None),
        }
    }
    pub fn at(&mut self, frame: u64) -> Option<usize> {
        if frame >= self.window.0 && frame < self.window.1 {
            return self.window.2;
        }
        let next = self.sorted.partition_point(|r| r.0 <= frame);
        let previous = next.checked_sub(1).map(|i| self.sorted[i]);
        self.window = match previous {
            Some((start, end, index)) if frame < end => (start, end, Some(index)),
            _ => (
                previous.map_or(0, |r| r.1),
                self.sorted.get(next).map_or(u64::MAX, |r| r.0),
                None,
            ),
        };
        self.window.2
    }
}

#[derive(Clone)]
struct Transition {
    owner: usize,
    start: u64,
    length: u64,
    left: bool,
    fade: daw_core::ClipFade,
}

#[derive(Clone)]
pub(super) struct AudioTrack {
    ranges: Ranges,
    fingerprint_id: u64,
    clips: Vec<daw_core::Clip>,
    compile_error: Option<String>,
    audio: Vec<Option<AudioData>>,
    compounds: Vec<Option<Box<AudioTrack>>>,
    layouts: Vec<Option<super::compound_audio::CompoundLayout>>,
    contexts: Vec<Option<daw_media::PreparedSourceContext>>,
    context_readers: Vec<Option<usize>>,
    clocks: Vec<Vec<daw_core::CompiledAudioEnvelope>>,
    rate: u32,
    envelopes: Vec<daw_core::ClipFades>,
    transitions: Vec<Transition>,
    transition_ranges: Ranges,
    staged: Option<(usize, u64, [f32; 2])>,
    loop_pin: Option<(u64, Option<(usize, u64)>)>,
    readers: Vec<daw_media::FrameReader>,
    reader_indices: Vec<Option<usize>>,
}
impl AudioTrack {
    pub fn same_source(&self, other: &Self) -> bool {
        self.compile_error.is_none()
            && other.compile_error.is_none()
            && self.fingerprint_id == other.fingerprint_id
            && !other.source_failed()
    }
    pub fn source_identity(&self) -> u64 {
        self.fingerprint_id
    }
    fn fingerprint(&self, hash: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.rate.hash(hash);
        self.clips.len().hash(hash);
        for (i, clip) in self.clips.iter().enumerate() {
            // Private provenance IDs and presentation do not change source samples.
            format!(
                "{:?}",
                (
                    clip.asset_id,
                    clip.start_frame,
                    clip.source_offset_frame,
                    clip.length_frames,
                    clip.repeat,
                    clip.stretch,
                    clip.gain_db,
                    clip.transpose,
                    clip.muted,
                    clip.channels,
                    self.envelopes[i],
                    &self.clocks[i]
                )
            )
            .hash(hash);
            if let Some(source) = &self.compounds[i] {
                source.fingerprint(hash);
            }
            if let Some(audio) = &self.audio[i] {
                std::sync::Arc::as_ptr(&audio.samples).hash(hash);
                audio.disk.as_ref().map(std::sync::Arc::as_ptr).hash(hash);
                audio.sample_rate_hz.hash(hash);
                audio.metadata.channels.hash(hash);
            }
        }
        for t in &self.transitions {
            format!("{:?}", (t.owner, t.start, t.length, t.left, t.fade)).hash(hash);
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.contexts)
            + daw_unit_core::memory::vec_bytes(&self.context_readers)
            + self
                .contexts
                .iter()
                .flatten()
                .map(|c| c.audio.metadata.sample_format.capacity() as u64)
                .sum::<u64>()
            + daw_unit_core::memory::vec_bytes(&self.layouts)
            + self
                .layouts
                .iter()
                .flatten()
                .map(|l| l.heap_bytes())
                .sum::<u64>()
            + daw_unit_core::memory::vec_bytes(&self.compounds)
            + self
                .compounds
                .iter()
                .flatten()
                .map(|s| s.heap_bytes() + std::mem::size_of::<AudioTrack>() as u64)
                .sum::<u64>()
            + daw_unit_core::memory::vec_bytes(&self.clocks)
            + self
                .clocks
                .iter()
                .map(daw_unit_core::memory::vec_bytes)
                .sum::<u64>()
            + daw_unit_core::memory::vec_bytes(&self.clips)
            + self
                .clips
                .iter()
                .map(|c| c.name.capacity() as u64)
                .sum::<u64>()
            + self
                .compile_error
                .as_ref()
                .map_or(0, |s| s.capacity() as u64)
            + self.ranges.heap_bytes()
            + self.transition_ranges.heap_bytes()
            + daw_unit_core::memory::vec_bytes(&self.envelopes)
            + daw_unit_core::memory::vec_bytes(&self.transitions)
            + daw_unit_core::memory::vec_bytes(&self.audio)
            + daw_unit_core::memory::vec_bytes(&self.readers)
            + daw_unit_core::memory::vec_bytes(&self.reader_indices)
            + self
                .readers
                .iter()
                .map(daw_media::FrameReader::heap_bytes)
                .sum::<u64>()
            + self
                .audio
                .iter()
                .flatten()
                .map(|a| a.metadata.sample_format.capacity() as u64)
                .sum::<u64>()
    }

    pub fn new(plan: &RenderPlan, track: &Track) -> Self {
        let mut budget = if track.clips.iter().any(|c| {
            plan.project
                .audio_asset(c.asset_id)
                .is_some_and(|a| a.compound_audio.is_some())
        }) {
            daw_core::COMPOUND_METADATA_LIMIT / 4
        } else {
            u64::MAX
        };
        Self::new_at_depth(plan, track, 0, &mut budget)
    }
    fn new_at_depth(plan: &RenderPlan, track: &Track, depth: usize, remaining: &mut u64) -> Self {
        let (view, mut compile_error) = match plan
            .project
            .compile_audio_track_view(track, |id| plan.audio.contains_key(&id))
        {
            Ok(view)
                if depth < 64
                    && view.track.clips.len() as u64
                        * (std::mem::size_of::<daw_core::Clip>() as u64 + 128)
                        <= *remaining =>
            {
                *remaining -= view.track.clips.len() as u64
                    * (std::mem::size_of::<daw_core::Clip>() as u64 + 128);
                (view, None)
            }
            result => {
                let message = result.err().map_or_else(
                    || "Compound reader nesting admission exceeded".into(),
                    |e| e.to_string(),
                );
                let mut track = track.clone();
                track.clips.clear();
                (
                    daw_core::CompiledAudioTrack {
                        track,
                        envelopes: Vec::new(),
                    },
                    Some(message),
                )
            }
        };
        let compiled = view.track;
        let track = &compiled;
        let compounds: Vec<_> = track
            .clips
            .iter()
            .map(|clip| {
                if clip.muted
                    || clip.stretch.is_some()
                    || clip.transpose != 0
                    || plan.project.compound_window_requires_preparation(clip)
                {
                    return None;
                }
                let source = plan
                    .project
                    .audio_asset(clip.asset_id)?
                    .compound_audio
                    .as_ref()?;
                let mut local = track.clone();
                local.clips = source.members.clone();
                let child = Self::new_at_depth(plan, &local, depth + 1, remaining);
                if let Some(error) = child.source_error() {
                    compile_error = Some(error);
                }
                Some(Box::new(child))
            })
            .collect();
        let layouts = track
            .clips
            .iter()
            .map(|clip| {
                if (clip.stretch.is_some()
                    || clip.transpose != 0
                    || plan.project.compound_window_requires_preparation(clip))
                    && plan
                        .project
                        .audio_asset(clip.asset_id)
                        .is_some_and(|a| a.compound_audio.is_some())
                {
                    match super::compound_audio::CompoundLayout::new(plan, clip.asset_id) {
                        Ok(layout) => Some(layout),
                        Err(error) => {
                            compile_error = Some(error);
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect();
        let mut sources = Vec::new();
        let mut readers = Vec::new();
        let reader_indices: Vec<Option<usize>> = track
            .clips
            .iter()
            .map(|clip| {
                if clip.muted {
                    return None;
                }
                let audio = plan.audio_for_clip(clip)?;
                let source = audio.disk.as_ref()?;
                let pointer = std::sync::Arc::as_ptr(source);
                if let Some(index) = sources.iter().position(|s| *s == pointer) {
                    return Some(index);
                }
                sources.push(pointer);
                readers.push(daw_media::FrameReader::new(audio));
                Some(readers.len() - 1)
            })
            .collect();
        let contexts: Vec<_> = track
            .clips
            .iter()
            .map(|c| {
                daw_media::prepared_clip_audio(
                    c,
                    &plan.audio,
                    &plan.stretched_audio,
                    plan.project.stretch_engine,
                )
                .and_then(|p| p.context.clone())
            })
            .collect();
        let context_readers = contexts
            .iter()
            .map(|c| {
                c.as_ref().and_then(|c| {
                    c.audio.disk.as_ref()?;
                    readers.push(daw_media::FrameReader::new(&c.audio));
                    Some(readers.len() - 1)
                })
            })
            .collect();
        let mut envelopes: Vec<_> = track
            .clips
            .iter()
            .enumerate()
            .map(|(i, _c)| {
                let mut fades = view.envelopes[i][0].fades;
                if fades.fade_in.crossfade && !fades.fade_in.inherit_source {
                    fades.fade_in = daw_core::ClipFade::default();
                }
                if fades.fade_out.crossfade && !fades.fade_out.inherit_source {
                    fades.fade_out = daw_core::ClipFade::default();
                }
                fades
            })
            .collect();
        let mut transitions = Vec::new();
        for (owner, clip) in track.clips.iter().enumerate() {
            for (left, fade) in [(true, clip.fades.fade_in), (false, clip.fades.fade_out)] {
                if !fade.crossfade || fade.length_frames == 0 {
                    continue;
                }
                let mut length = fade
                    .length_frames
                    .min(plan.project.clip_fade_limit_in(track, clip.id, left, true));
                if let Some(context) = &contexts[owner] {
                    let handles = if clip.repeat.is_some() {
                        u64::MAX
                    } else if left {
                        context.start_frame.saturating_add(clip.source_offset_frame)
                    } else {
                        context.audio.frame_count().saturating_sub(
                            context
                                .start_frame
                                .saturating_add(clip.source_offset_frame)
                                .saturating_add(clip.length_frames),
                        )
                    };
                    length = length.min(handles);
                }
                if length == 0 {
                    continue;
                }
                let neighbor = track
                    .clips
                    .iter()
                    .position(|other| {
                        other.id != clip.id
                            && if left {
                                other.end() == clip.start_frame
                            } else {
                                other.start_frame == clip.end()
                            }
                    })
                    .unwrap();
                // Outgoing owners take priority if an unrelated placement joins two saved transitions.
                if left
                    && track.clips[neighbor].fades.fade_out.crossfade
                    && track.clips[neighbor].fades.fade_out.length_frames > 0
                {
                    continue;
                }
                if audio_source_missing(plan, clip)
                    || audio_source_missing(plan, &track.clips[neighbor])
                {
                    continue;
                }
                let start = if left {
                    clip.start_frame - length
                } else {
                    clip.end()
                };
                transitions.push(Transition {
                    owner,
                    start,
                    length,
                    left,
                    fade,
                });
                let edge = if left {
                    &mut envelopes[owner].fade_in
                } else {
                    &mut envelopes[owner].fade_out
                };
                *edge = fade;
                let other = if left {
                    &mut envelopes[neighbor].fade_out
                } else {
                    &mut envelopes[neighbor].fade_in
                };
                other.crossfade = true;
                other.length_frames = length;
            }
        }
        // Extended owner reads have independent reader state even when the PCM is shared.
        let mut reader_indices = reader_indices;
        for owner in transitions.iter().map(|t| t.owner) {
            if let Some(audio) = plan.audio_for_clip(&track.clips[owner])
                && audio.disk.is_some()
            {
                reader_indices[owner] = Some(readers.len());
                readers.push(daw_media::FrameReader::new(audio));
            }
        }
        let transition_ranges =
            Ranges::new(transitions.iter().map(|t| (t.start, t.start + t.length)));
        let mut prepared = Self {
            fingerprint_id: 0,
            clips: compiled.clips.clone(),
            compounds,
            layouts,
            contexts,
            context_readers,
            clocks: view.envelopes,
            compile_error,
            envelopes,
            transitions,
            transition_ranges,
            readers,
            reader_indices,
            staged: None,
            loop_pin: None,
            ranges: Ranges::new(track.clips.iter().map(|c| (c.start_frame, c.end()))),
            audio: track
                .clips
                .iter()
                .map(|c| {
                    if c.muted {
                        None
                    } else {
                        plan.audio_for_clip(c).cloned()
                    }
                })
                .collect(),
            rate: plan.project.sample_rate_hz,
        };
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        prepared.fingerprint(&mut hash);
        prepared.fingerprint_id = std::hash::Hasher::finish(&hash);
        prepared
    }
    pub fn worker_window_available(&mut self, frame: u64) -> bool {
        if let Some(index) = self.ranges.at(frame) {
            let position = self.clips[index].source_frame(frame - self.clips[index].start_frame);
            if !self.worker_clip_available(index, i128::from(position), false) {
                return false;
            }
        }
        if let Some(t) = self.transition_ranges.at(frame) {
            let transition = &self.transitions[t];
            let owner = transition.owner;
            let position = transition_position(transition, &self.clips[owner], frame);
            if !self.worker_clip_available(owner, position, true) {
                return false;
            }
        }
        true
    }
    fn worker_clip_available(&mut self, index: usize, position: i128, continuation: bool) -> bool {
        if self.clips[index].muted {
            return true;
        }
        if let Some(source) = &mut self.compounds[index] {
            return u64::try_from(position).is_ok_and(|p| source.worker_window_available(p));
        }
        if continuation && let Some(context) = &self.contexts[index] {
            let point = position + i128::from(context.start_frame);
            return point >= 0 && point < i128::from(context.audio.frame_count());
        }
        self.audio[index]
            .as_ref()
            .is_none_or(|a| position >= 0 && position < i128::from(a.frame_count()))
    }
    pub fn sample_extended(&mut self, frame: i128) -> Result<([f32; 2], Option<bool>), String> {
        let Some(index) = (if frame < 0 {
            (!self.clips.is_empty()).then_some(0)
        } else {
            self.clips.len().checked_sub(1)
        }) else {
            return Ok(([0.; 2], None));
        };
        let clip = &self.clips[index];
        let local = frame - i128::from(clip.start_frame);
        let signed = clip
            .extended_source_frame(local)
            .map_or_else(|| i128::from(clip.source_offset_frame) + local, i128::from);
        if !self.worker_clip_available(index, signed, true) {
            return Err("Compound continuation source range is unavailable".into());
        }
        let clip = &self.clips[index];
        if clip.muted {
            return Ok(([0.; 2], None));
        }
        let position = self.contexts[index]
            .as_ref()
            .map_or(signed, |c| signed + i128::from(c.start_frame))
            .max(0) as u64;
        let (value, mono) = if let Some(source) = &mut self.compounds[index] {
            source.sample(&[], position, false)
        } else if let Some(audio) = self.contexts[index]
            .as_ref()
            .map(|c| &c.audio)
            .or(self.audio[index].as_ref())
        {
            let reader = if self.contexts[index].is_some() {
                self.context_readers[index]
            } else {
                self.reader_indices[index]
            };
            let value = if let Some(reader) = reader {
                self.readers[reader]
                    .sample(position)
                    .map_err(|e| e.to_string())?
            } else {
                audio
                    .samples
                    .get(position as usize)
                    .copied()
                    .unwrap_or_default()
            };
            let mono =
                self.layouts[index]
                    .as_mut()
                    .map_or(Some(audio.metadata.channels == 1), |layout| {
                        layout.at_continued(clip.stretch.map_or(signed, |s| {
                            i128::from(s.original_frame(signed.max(0) as u64))
                        }))
                    });
            (value, mono)
        } else {
            return Ok(([0.; 2], None));
        };
        let gain = daw_core::linear_gain(clip.gain_db)
            * super::clip_repeat_fade(clip, signed.max(0) as u64, self.rate);
        Ok((clip.channels.apply(value).map(|v| v * gain), mono))
    }
    pub fn source_error(&self) -> Option<String> {
        self.compile_error
            .clone()
            .or_else(|| self.readers.iter().find_map(|reader| reader.error()))
            .or_else(|| {
                self.compounds
                    .iter()
                    .flatten()
                    .find_map(|s| s.source_error())
            })
    }
    pub fn source_failed(&self) -> bool {
        self.compile_error.is_some()
            || self.readers.iter().any(daw_media::FrameReader::failed)
            || self.compounds.iter().flatten().any(|s| s.source_failed())
    }
    pub fn prepare_streaming(&mut self) -> daw_media::Result<()> {
        for source in self.compounds.iter_mut().flatten() {
            source.prepare_streaming()?;
        }
        for reader in &mut self.readers {
            reader.prepare_live()?;
        }
        Ok(())
    }
    fn ready_primary(&mut self, frame: u64, loop_start: Option<u64>) -> bool {
        let clips = &self.clips;
        if let Some(start) = loop_start {
            if self.loop_pin.as_ref().is_none_or(|(old, _)| *old != start) {
                let pin = self.ranges.at(start).and_then(|i| {
                    self.reader_indices[i]
                        .map(|reader| (reader, clips[i].source_frame(start - clips[i].start_frame)))
                });
                self.loop_pin = Some((start, pin));
            }
        } else {
            self.loop_pin = None;
        }
        let Some(index) = self.ranges.at(frame) else {
            return true;
        };
        if let Some(source) = &mut self.compounds[index] {
            return source.ready(
                &[],
                clips[index].source_frame(frame - clips[index].start_frame),
                loop_start
                    .map(|f| clips[index].source_frame(f.saturating_sub(clips[index].start_frame))),
            );
        }
        if self.audio[index].is_none() {
            return true;
        }
        let Some(reader) = self.reader_indices[index] else {
            return true;
        };
        let clip = &clips[index];
        let pin = self.loop_pin.and_then(|(_, pin)| pin);
        if let Some((other, position)) = pin
            && other != reader
        {
            self.readers[other].ready(position, Some(position));
        }
        let pinned = pin
            .filter(|(other, _)| *other == reader)
            .map(|(_, frame)| frame);
        let position = clip.source_frame(frame - clip.start_frame);
        let ready = self.readers[reader].ready(position, pinned);
        self.staged = self.readers[reader]
            .try_live_sample(position)
            .map(|value| (index, position, value));
        ready && self.staged.is_some()
    }
    pub fn ready(
        &mut self,
        _clips: &[daw_core::Clip],
        frame: u64,
        loop_start: Option<u64>,
    ) -> bool {
        let primary = self.ready_primary(frame, loop_start);
        let clips = &self.clips;
        if let Some(i) = self.transition_ranges.at(frame) {
            let t = &self.transitions[i];
            let clip = &clips[t.owner];
            let signed = transition_position(t, clip, frame);
            let position = self.contexts[t.owner]
                .as_ref()
                .map_or(signed, |c| signed + i128::from(c.start_frame))
                .max(0) as u64;
            if let Some(source) = &mut self.compounds[t.owner] {
                return source.ready(&[], position, None) && primary;
            }
            if let Some(reader) = if self.contexts[t.owner].is_some() {
                self.context_readers[t.owner]
            } else {
                self.reader_indices[t.owner]
            } {
                return self.readers[reader].ready(position, None) && primary;
            }
        }
        primary
    }
    pub fn sample(
        &mut self,
        _clips: &[daw_core::Clip],
        frame: u64,
        realtime: bool,
    ) -> ([f32; 2], Option<bool>) {
        let (mut primary, mut mono) = self.sample_primary(frame, realtime);
        let clips = &self.clips;
        if let Some(i) = self.transition_ranges.at(frame) {
            let t = &self.transitions[i];
            let clip = &clips[t.owner];
            let signed = transition_position(t, clip, frame);
            let position = self.contexts[t.owner]
                .as_ref()
                .map_or(signed, |c| signed + i128::from(c.start_frame))
                .max(0) as u64;
            let (extra, extra_mono) = if let Some(source) = &mut self.compounds[t.owner] {
                source.sample(&[], position, realtime)
            } else {
                let audio = self.contexts[t.owner]
                    .as_ref()
                    .map(|c| &c.audio)
                    .or(self.audio[t.owner].as_ref())
                    .unwrap();
                let reader = if self.contexts[t.owner].is_some() {
                    self.context_readers[t.owner]
                } else {
                    self.reader_indices[t.owner]
                };
                let extra = if let Some(reader) = reader {
                    if realtime {
                        self.readers[reader].live_sample(position)
                    } else {
                        self.readers[reader].sample(position).unwrap_or_default()
                    }
                } else {
                    audio
                        .samples
                        .get(position as usize)
                        .copied()
                        .unwrap_or_default()
                };
                let mono = self.layouts[t.owner].as_mut().map_or(
                    Some(audio.metadata.channels == 1),
                    |layout| {
                        layout.at_continued(clip.stretch.map_or(signed, |s| {
                            i128::from(s.original_frame(signed.max(0) as u64))
                        }))
                    },
                );
                (extra, mono)
            };
            let extra = clip.channels.apply(extra);
            let progress = (frame - t.start) as f32 / t.length.saturating_sub(1).max(1) as f32;
            let weight = t.fade.gain(if t.left { progress } else { 1.0 - progress });
            let opposing = t
                .fade
                .opposing_gain(if t.left { progress } else { 1.0 - progress });
            let gain = daw_core::linear_gain(clip.gain_db);
            mono = extra_mono;
            let scale = 1.0;
            for ch in 0..2 {
                primary[ch] = primary[ch] * opposing
                    + extra[ch]
                        * gain
                        * weight
                        * super::clip_repeat_fade(clip, signed.max(0) as u64, self.rate)
                        * scale;
            }
        }
        (primary, mono)
    }
    fn envelope_at(&mut self, frame: u64) -> Option<f32> {
        if self.transition_ranges.at(frame).is_some() {
            return None;
        }
        let index = self.ranges.at(frame)?;
        let clip = &self.clips[index];
        let local = frame - clip.start_frame;
        let clock = &self.clocks[index][0];
        let mut gain = self.envelopes[index]
            .gain(local + clock.offset, clock.length_frames, self.rate)
            .min(super::clip_repeat_fade(
                clip,
                clip.source_frame(local),
                self.rate,
            ));
        for stage in &self.clocks[index][1..] {
            gain *= stage
                .fades
                .gain(local + stage.offset, stage.length_frames, self.rate);
        }
        Some(gain)
    }
    fn sample_primary(&mut self, frame: u64, realtime: bool) -> ([f32; 2], Option<bool>) {
        let clips = &self.clips;
        let Some(index) = self.ranges.at(frame) else {
            return ([0.0; 2], None);
        };
        let clip = &clips[index];
        let local = frame - clip.start_frame;
        let position = clip.source_frame(local);
        let clock = &self.clocks[index][0];
        let mut fade = self.envelopes[index]
            .gain(local + clock.offset, clock.length_frames, self.rate)
            .min(super::clip_repeat_fade(clip, position, self.rate));
        for stage in &self.clocks[index][1..] {
            fade *= stage
                .fades
                .gain(local + stage.offset, stage.length_frames, self.rate);
        }
        if let Some(source) = &mut self.compounds[index] {
            let intrinsic = source.envelope_at(position);
            let (value, mono) = source.sample(&[], position, realtime);
            if clip.repeat.is_some()
                && let Some(intrinsic) = intrinsic
            {
                fade = if intrinsic > 0.0 {
                    fade.min(intrinsic) / intrinsic
                } else {
                    0.0
                };
            }
            return (
                clip.channels
                    .apply(value)
                    .map(|v| v * fade * daw_core::linear_gain(clip.gain_db)),
                mono,
            );
        }
        let Some(audio) = &self.audio[index] else {
            return ([0.0; 2], None);
        };
        let value = if let Some(reader) = self.reader_indices[index] {
            if realtime {
                self.staged
                    .filter(|(i, p, _)| *i == index && *p == position)
                    .map_or_else(
                        || self.readers[reader].live_sample(position),
                        |(_, _, value)| value,
                    )
            } else {
                self.readers[reader].sample(position).unwrap_or_default()
            }
        } else {
            audio
                .samples
                .get(position as usize)
                .copied()
                .unwrap_or_default()
        };
        (
            clip.channels
                .apply(value)
                .map(|v| v * fade * daw_core::linear_gain(clip.gain_db)),
            self.layouts[index]
                .as_mut()
                .map_or(Some(audio.metadata.channels == 1), |layout| {
                    layout.at(clip
                        .stretch
                        .map_or(position, |s| s.original_frame(position)))
                }),
        )
    }
}

fn audio_source_missing(plan: &RenderPlan, clip: &daw_core::Clip) -> bool {
    if plan.audio_for_clip(clip).is_some() {
        return false;
    }
    if clip.stretch.is_some() || clip.transpose != 0 {
        return true;
    }
    let Some(recipe) = plan
        .project
        .audio_asset(clip.asset_id)
        .and_then(|a| a.compound_audio.as_ref())
    else {
        return true;
    };
    // Dependency walks run during preparation, never in the audio callback.
    let mut pending: Vec<_> = recipe.members.iter().collect();
    let mut visited = std::collections::HashSet::new();
    while let Some(member) = pending.pop() {
        if !visited.insert(member.asset_id) {
            continue;
        }
        if member.muted || plan.audio_for_clip(member).is_some() {
            continue;
        }
        if member.stretch.is_some() || member.transpose != 0 {
            return true;
        }
        if let Some(recipe) = plan
            .project
            .audio_asset(member.asset_id)
            .and_then(|a| a.compound_audio.as_ref())
        {
            pending.extend(&recipe.members);
        } else {
            return true;
        }
    }
    false
}
fn transition_position(_t: &Transition, clip: &daw_core::Clip, frame: u64) -> i128 {
    let local = i128::from(frame) - i128::from(clip.start_frame);
    clip.extended_source_frame(local)
        .map_or_else(|| i128::from(clip.source_offset_frame) + local, i128::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsorted_ranges_support_gaps_exclusive_ends_and_arbitrary_seeks() {
        let ranges = [(30, 40), (10, 20), (20, 25)];
        let mut index = Ranges::new(ranges.into_iter());
        for frame in (0..45)
            .chain((0..45).rev())
            .chain([40, 20, 10, u64::MAX, 25, 0])
        {
            assert_eq!(
                index.at(frame),
                ranges.iter().position(|(a, b)| *a <= frame && frame < *b)
            );
        }
    }
}
