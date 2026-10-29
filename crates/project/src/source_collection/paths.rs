use crate::{Result, error};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};
use std::{
    fs,
    path::{Component, Path},
};

#[derive(Default)]
pub(super) struct DestinationTree {
    entries: BTreeMap<PathBuf, Vec<OsString>>,
}
impl DestinationTree {
    pub fn check(
        &mut self,
        root: &Path,
        relative: &Path,
        file: bool,
        cancelled: &impl Fn() -> bool,
    ) -> Result<()> {
        ordinary(root, relative, file)?;
        let mut parent = root.to_path_buf();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(error("Invalid project media path"));
            };
            if !self.entries.contains_key(&parent) {
                let mut names = Vec::new();
                match fs::read_dir(&parent) {
                    Ok(entries) => {
                        for entry in entries {
                            if cancelled() {
                                return Err(error("Source collection preflight cancelled"));
                            }
                            names.push(entry.map_err(error)?.file_name());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(error(e)),
                }
                self.entries.insert(parent.clone(), names);
            }
            if self.entries[&parent].iter().any(|existing| {
                existing != name
                    && existing.to_string_lossy().to_lowercase()
                        == name.to_string_lossy().to_lowercase()
            }) {
                return Err(error(format!(
                    "Project media path has a case collision: {}",
                    parent.join(name).display()
                )));
            }
            parent.push(name);
        }
        Ok(())
    }
}

/// Check every component below the canonical project folder. Missing paths are
/// allowed for proposals; neither discovery nor collision checks reserve them.
pub(super) fn ordinary(root: &Path, relative: &Path, file: bool) -> Result<()> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(error("Empty project media path"));
    }
    let mut path = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(error("Project media path must stay inside its directory"));
        };
        let name = name
            .to_str()
            .ok_or_else(|| error("Project media filename is not UTF-8"))?;
        daw_core::validate_included_source_path(&format!("assets/sources/{name}"))
            .map_err(error)?;
        path.push(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if crate::file_publish::is_reparse(&metadata) {
                    return Err(error(format!(
                        "Project media path contains a symlink/reparse point: {}",
                        path.display()
                    )));
                }
                let last = index + 1 == components.len();
                if (!last || !file) && !metadata.is_dir() || last && file && !metadata.is_file() {
                    return Err(error(format!("Unexpected file type at {}", path.display())));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(error(format!("{}: {e}", path.display()))),
        }
    }
    Ok(())
}

pub(super) fn relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(error)?;
    relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => name
                .to_str()
                .ok_or_else(|| error("Project media path is not UTF-8")),
            _ => Err(error("Invalid project media path")),
        })
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}
