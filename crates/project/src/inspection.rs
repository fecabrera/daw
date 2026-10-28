//! Read-only project metadata inspection without media decoding or preparation.
use crate::{Result, error};
use daw_core::Project;
use std::path::Path;

/// Load and validate saved metadata. Referenced media files are not opened.
/// Use `Session::open` when source validation and audio preparation are required.
pub fn inspect_project(folder: &Path) -> Result<Project> {
    load_manifest(folder).map(|(project, _)| project)
}

pub(crate) fn load_manifest(folder: &Path) -> Result<(Project, Option<u32>)> {
    let content = std::fs::read(folder.join("project.json")).map_err(error)?;
    let mut project: Project = serde_json::from_slice(&content).map_err(error)?;
    // Share the loader's one-way legacy revision normalization. Inspection never
    // writes it; Session::open rebuilds prepared audio before publishing a session.
    let previous_revision = (project.stretch_engine.id == daw_core::StretchEngineId::Integrated
        && matches!(project.stretch_engine.revision, 2 | 3))
    .then_some(project.stretch_engine.revision);
    if previous_revision.is_some() {
        project.stretch_engine = daw_core::StretchEngine::INTEGRATED;
    }
    project.validate().map_err(error)?;
    Ok((project, previous_revision))
}
