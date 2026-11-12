//! Complete worker transaction for audio Merge.
use crate::{Result, Session, error};
use daw_core::Id;
impl Session {
    pub fn merged_audio_clips(
        &self,
        ids: &[Id],
        cancelled: &impl Fn() -> bool,
    ) -> Result<(Self, Id)> {
        if cancelled() {
            return Err(error("Audio Merge cancelled"));
        }
        let (project, id) = self.project.merge_audio_clips(ids).map_err(error)?;
        if project == self.project {
            return Ok((self.clone(), id));
        }
        let mut next = self.clone();
        next.project = project;
        next.prepare_stretches_cancellable(cancelled)?;
        Ok((next, id))
    }
}
