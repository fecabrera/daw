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
                .any(|t| t.reader.source_failed())
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
        {
            ready &= tap.ready(frame, region);
        }
        ready
    }
}
