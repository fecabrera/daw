//! Shared machine-local search folders for desktop and headless hosts.
use crate::{Error, Result, SearchRoot};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
struct Folders {
    version: u32,
    roots: Vec<SearchRoot>,
}
fn path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    Ok(root
        .ok_or_else(|| Error::Invalid("No plugin preferences directory is available".into()))?
        .join("daw/plugin-folders-v1.json"))
}
fn validate(roots: &[SearchRoot]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if roots.len() > 64
        || roots.iter().any(|r| {
            !r.path.is_absolute()
                || r.path.as_os_str().len() > 16384
                || !seen.insert((r.format, &r.path))
        })
    {
        return Err(Error::Invalid(
            "Invalid or oversized plugin folder preferences".into(),
        ));
    }
    Ok(())
}
fn load(file: &Path) -> Result<Vec<SearchRoot>> {
    let file = match fs::File::open(file) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Invalid(
            "Plugin folder preferences exceed 1 MiB".into(),
        ));
    }
    let settings: Folders = serde_json::from_slice(&bytes)?;
    if settings.version != 1 {
        return Err(Error::Invalid(
            "Unsupported plugin folder preferences".into(),
        ));
    }
    validate(&settings.roots)?;
    Ok(settings.roots)
}
fn save(file: &Path, roots: &[SearchRoot]) -> Result<()> {
    validate(roots)?;
    let parent = file
        .parent()
        .ok_or_else(|| Error::Invalid("Invalid plugin preferences path".into()))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".plugin-folders-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        output.write_all(&serde_json::to_vec(&Folders {
            version: 1,
            roots: roots.to_vec(),
        })?)?;
        output.sync_all()?;
        drop(output);
        crate::registry::replace(&temp, file)?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}
pub fn save_plugin_folders(roots: &[SearchRoot]) -> Result<()> {
    save(&path()?, roots)
}
pub fn configured_search_roots() -> Result<Vec<SearchRoot>> {
    let mut roots = crate::default_search_roots()?;
    for root in load(&path()?)? {
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn machine_folders_round_trip_and_invalid_update_preserves_saved_preferences() {
        let root = std::env::temp_dir().join(format!("daw-folder-config-{}", uuid::Uuid::new_v4()));
        let file = root.join("folders.json");
        let folders = vec![
            SearchRoot {
                format: crate::PluginFormat::Vst3,
                path: root.join("Unicode μ VST3"),
            },
            SearchRoot {
                format: crate::PluginFormat::Clap,
                path: root.join("CLAP"),
            },
        ];
        save(&file, &folders).unwrap();
        assert_eq!(load(&file).unwrap(), folders);
        let before = fs::read(&file).unwrap();
        let mut invalid = folders.clone();
        invalid.push(folders[0].clone());
        assert!(save(&file, &invalid).is_err());
        assert_eq!(fs::read(&file).unwrap(), before);
        fs::write(&file, b"corrupt").unwrap();
        assert!(load(&file).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
