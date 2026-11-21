//! Worker preparation and callback preflight for bounded disk PCM.
use super::{Renderer, latency};
impl Renderer {
    /// Prepare bounded disk reads on a worker before publishing to the callback.
    pub fn prepare_streaming(&mut self) -> Result<(), String> {
        if let Some(error) = self.source_error() {
            return Err(error);
        }
        for track in &mut self.audio_tracks {
            track.prepare_streaming().map_err(|e| e.to_string())?;
        }
        for tap in self
            .sidechains
            .taps_mut()
            .chain(self.buses.sends.taps_mut())
            .chain(self.hosted.iter_mut().flat_map(|g| g.taps_mut()))
        {
            tap.reader.prepare_streaming().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Control/worker only: file error text is never copied on the audio thread.
    pub fn source_error(&self) -> Option<String> {
        self.audio_tracks
            .iter()
            .find_map(|track| track.source_error())
            .or_else(|| {
                self.sidechains
                    .taps()
                    .chain(self.buses.sends.taps())
                    .chain(self.hosted.iter().flat_map(|g| g.taps()))
                    .find_map(|t| t.reader.source_error())
            })
    }
    pub fn buffering(&self) -> bool {
        self.buffering
    }
    pub fn source_failed(&self) -> bool {
        self.audio_tracks.iter().any(|track| track.source_failed())
            || self
                .sidechains
                .taps()
                .chain(self.buses.sends.taps())
                .chain(self.hosted.iter().flat_map(|g| g.taps()))
                .any(|t| t.reader.source_failed())
    }
    /// Worker-only. Request the startup range before replacing active playback.
    pub fn fast_seek_streaming_ready(&mut self) -> bool {
        if !self.plan.audio.values().any(|audio| audio.disk.is_some())
            && !self
                .plan
                .stretched_audio
                .values()
                .any(|audio| audio.audio.disk.is_some())
        {
            return true;
        }
        let region = self.loop_region(true);
        let count = (self.plan.project.buffer_size_frames as usize)
            .max(self.hosted.as_ref().map_or(0, |graph| graph.quantum));
        let start = latency::advanced_frame(self.playhead, self.alignment.total, region);
        (0..count as u64).all(|offset| {
            let frame = latency::advanced_frame(start, offset, region);
            self.streaming_ready(frame, region)
        })
    }
    pub(super) fn streaming_ready(&mut self, frame: u64, region: Option<(u64, u64)>) -> bool {
        let frame = latency::wrapped_frame(frame, region);
        let mut ready = true;
        for (index, (runtime, track)) in self
            .audio_tracks
            .iter_mut()
            .zip(&self.plan.project.tracks)
            .enumerate()
        {
            let clock = if self.sidechains.active() {
                latency::delayed_frame(frame, self.alignment.input_frames[index], region)
            } else {
                frame
            };
            ready &= runtime.ready(&track.clips, clock, region.map(|r| r.0));
        }
        for tap in self
            .sidechains
            .taps_mut()
            .chain(self.buses.sends.taps_mut())
            .chain(self.hosted.iter_mut().flat_map(|g| g.taps_mut()))
        {
            ready &= tap.ready(frame, region);
        }
        ready
    }
}
