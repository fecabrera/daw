//! Shared lifetime for exclusively owned generated media, including history snapshots.
use crate::{Result, error};
use std::{fs, path::PathBuf};
pub(crate) struct GeneratedDirectory(pub(crate) PathBuf);
impl GeneratedDirectory {
    pub(crate) fn cleanup(&self) -> Result<()> {
        match fs::remove_dir_all(&self.0) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(error(format!(
                "Cannot remove unpublished generated media: {e}"
            ))),
        }
    }
    pub(crate) fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("daw-generated-{}", daw_core::Id::new_v4()));
        fs::create_dir(&path).map_err(error)?;
        Ok(Self(path))
    }
}
impl Drop for GeneratedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
