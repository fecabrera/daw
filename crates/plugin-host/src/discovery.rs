use crate::{Error, PluginFormat, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPlatform {
    Mac,
    Windows,
    Linux,
}

/// Injected directories keep path resolution testable without modifying the process environment.
pub struct SearchEnvironment {
    pub home: PathBuf,
    pub application: PathBuf,
    pub windows_common: Option<PathBuf>,
    pub windows_user_common: Option<PathBuf>,
    pub clap_path: Vec<PathBuf>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRoot {
    pub format: PluginFormat,
    pub path: PathBuf,
}

pub fn search_roots(platform: HostPlatform, env: &SearchEnvironment) -> Vec<SearchRoot> {
    let mut roots = Vec::new();
    let mut add = |format, path: PathBuf| {
        if !path.as_os_str().is_empty()
            && !roots
                .iter()
                .any(|r: &SearchRoot| r.format == format && r.path == path)
        {
            roots.push(SearchRoot { format, path });
        }
    };
    match platform {
        HostPlatform::Mac => {
            for path in [
                env.home.join("Library/Audio/Plug-Ins/VST3"),
                PathBuf::from("/Library/Audio/Plug-Ins/VST3"),
                PathBuf::from("/Network/Library/Audio/Plug-Ins/VST3"),
                env.application.join("Contents/VST3"),
            ] {
                add(PluginFormat::Vst3, path);
            }
            // User-first is host policy for CLAP; the format does not define duplicate precedence.
            for path in [
                env.home.join("Library/Audio/Plug-Ins/CLAP"),
                PathBuf::from("/Library/Audio/Plug-Ins/CLAP"),
            ] {
                add(PluginFormat::Clap, path);
            }
        }
        HostPlatform::Windows => {
            for path in [&env.windows_user_common, &env.windows_common]
                .into_iter()
                .flatten()
            {
                add(PluginFormat::Vst3, path.join("VST3"));
            }
            add(PluginFormat::Vst3, env.application.join("VST3"));
            for path in [&env.windows_user_common, &env.windows_common]
                .into_iter()
                .flatten()
            {
                add(PluginFormat::Clap, path.join("CLAP"));
            }
        }
        HostPlatform::Linux => {
            for path in [
                env.home.join(".vst3"),
                PathBuf::from("/usr/lib64/vst3"),
                PathBuf::from("/usr/lib/vst3"),
                PathBuf::from("/usr/local/lib64/vst3"),
                PathBuf::from("/usr/local/lib/vst3"),
                env.application.join("vst3"),
            ] {
                add(PluginFormat::Vst3, path);
            }
            for path in [env.home.join(".clap"), PathBuf::from("/usr/lib/clap")] {
                add(PluginFormat::Clap, path);
            }
        }
    }
    for path in &env.clap_path {
        add(PluginFormat::Clap, path.clone());
    }
    roots
}

pub fn default_search_roots() -> Result<Vec<SearchRoot>> {
    let executable = std::env::current_exe()?;
    let parent = executable
        .parent()
        .ok_or_else(|| Error::Invalid("Application path has no parent".into()))?;
    #[cfg(target_os = "macos")]
    let application = if parent.file_name().is_some_and(|s| s == "MacOS")
        && parent.parent().is_some_and(|p| p.ends_with("Contents"))
    {
        parent
            .parent()
            .and_then(Path::parent)
            .unwrap_or(parent)
            .to_owned()
    } else {
        parent.to_owned()
    };
    #[cfg(not(target_os = "macos"))]
    let application = parent.to_owned();
    #[cfg(target_os = "windows")]
    let (platform, home, windows_common, windows_user_common) = {
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_Profile, FOLDERID_ProgramFilesCommon, FOLDERID_UserProgramFilesCommon,
        };
        (
            HostPlatform::Windows,
            known_folder(&FOLDERID_Profile)?,
            Some(known_folder(&FOLDERID_ProgramFilesCommon)?),
            Some(known_folder(&FOLDERID_UserProgramFilesCommon)?),
        )
    };
    #[cfg(not(target_os = "windows"))]
    let (platform, home, windows_common, windows_user_common) = (
        if cfg!(target_os = "macos") {
            HostPlatform::Mac
        } else {
            HostPlatform::Linux
        },
        PathBuf::from(
            std::env::var_os("HOME")
                .filter(|s| !s.is_empty())
                .ok_or_else(|| Error::Invalid("HOME is unavailable for plugin discovery".into()))?,
        ),
        None,
        None,
    );
    let clap_path = std::env::var_os("CLAP_PATH")
        .map(|s| split_clap_path(&s))
        .unwrap_or_default();
    Ok(search_roots(
        platform,
        &SearchEnvironment {
            home,
            application,
            windows_common,
            windows_user_common,
            clap_path,
        },
    ))
}

fn split_clap_path(value: &OsString) -> Vec<PathBuf> {
    // Empty path entries must not cause arbitrary current-directory plugin loading.
    std::env::split_paths(value)
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

#[cfg(target_os = "windows")]
fn known_folder(id: &windows_sys::core::GUID) -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    let mut path = std::ptr::null_mut();
    // SAFETY: Windows writes a CoTaskMem-owned, terminated path on success.
    let status = unsafe { SHGetKnownFolderPath(id, 0, 0, &mut path) };
    if status < 0 || path.is_null() {
        return Err(Error::Invalid(format!(
            "Known Folder lookup failed: {status}"
        )));
    }
    let mut length = 0;
    // SAFETY: successful SHGetKnownFolderPath returns a terminated UTF-16 string.
    unsafe {
        while *path.add(length) != 0 {
            length += 1;
        }
        let value = OsString::from_wide(std::slice::from_raw_parts(path, length));
        CoTaskMemFree(path.cast());
        Ok(PathBuf::from(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCandidate {
    pub format: PluginFormat,
    pub path: PathBuf,
    pub executable: PathBuf,
    pub fingerprint: String,
}
#[derive(Default, Debug)]
pub struct Discovery {
    pub candidates: Vec<PluginCandidate>,
    pub warnings: Vec<String>,
}

/// Traversal and hashing run on a control worker, never on audio or in UI painting.
pub fn discover(roots: &[SearchRoot]) -> Discovery {
    let mut result = Discovery::default();
    let mut visited = HashSet::new();
    let mut modules = HashSet::new();
    let mut budget = 100_000usize;
    for root in roots {
        let mut pending = vec![(root.path.clone(), 0)];
        while let Some((path, depth)) = pending.pop() {
            if budget == 0 {
                result
                    .warnings
                    .push("Plugin discovery reached its 100000-entry limit".into());
                return result;
            }
            budget -= 1;
            if depth > 64 {
                result.warnings.push(format!(
                    "Plugin directory nesting exceeds 64: {}",
                    path.display()
                ));
                continue;
            }
            let resolved = match crate::aliases::resolve(&path) {
                Ok(p) => p,
                Err(e) => {
                    result.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let canonical = match fs::canonicalize(&resolved) {
                Ok(p) => p,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    result.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if resolved
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(root.format.extension()))
            {
                if !modules.insert((root.format, canonical.clone())) {
                    continue;
                }
                match candidate(root.format, canonical) {
                    Ok(c) => result.candidates.push(c),
                    Err(e) => result.warnings.push(format!("{}: {e}", path.display())),
                }
                continue;
            }
            if !canonical.is_dir() || !visited.insert((root.format, canonical.clone())) {
                continue;
            }
            match read_children(&canonical, budget.saturating_sub(pending.len())) {
                Ok(mut entries) => {
                    entries.sort();
                    pending.extend(entries.into_iter().rev().map(|p| (p, depth + 1)));
                }
                Err(e) => result
                    .warnings
                    .push(format!("{}: {e}", canonical.display())),
            }
        }
    }
    result
}

pub(crate) fn candidate(format: PluginFormat, path: PathBuf) -> Result<PluginCandidate> {
    let path = fs::canonicalize(path)?;
    let executable = module_executable(format, &path)?;
    crate::architecture::check_native_binary(&executable)?;
    let mut hash = Sha256::new();
    hash.update(b"daw-plugin-scan\0clap-sys-0.5.0\0vst3-0.3.0\0");
    hash.update(crate::SCAN_PROTOCOL_VERSION.to_le_bytes());
    hash.update(std::env::consts::OS.as_bytes());
    hash.update(std::env::consts::ARCH.as_bytes());
    let mut remaining = 512 * 1024 * 1024u64;
    let mut entries = 100_000;
    fingerprint_tree(
        &path,
        &path,
        &mut HashSet::new(),
        &mut hash,
        &mut remaining,
        &mut entries,
        0,
    )?;
    Ok(PluginCandidate {
        format,
        path,
        executable,
        fingerprint: format!("{:x}", hash.finalize()),
    })
}

/// Fingerprint resources too: presets or bundle metadata can change without binary changes.
fn fingerprint_tree(
    root: &Path,
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    hash: &mut Sha256,
    remaining: &mut u64,
    entries: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > 64 || visited.len() >= 100_000 {
        return Err(Error::Invalid(
            "Plugin bundle exceeds traversal limit".into(),
        ));
    }
    let resolved = fs::canonicalize(path)?;
    if !visited.insert(resolved.clone()) {
        return Ok(());
    }
    if !resolved.starts_with(root) {
        return Err(Error::Invalid(
            "Plugin resource link leaves its bundle".into(),
        ));
    }
    let name = path
        .strip_prefix(root)
        .unwrap_or(path)
        .as_os_str()
        .as_encoded_bytes();
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name);
    hash.update([u8::from(resolved.is_dir())]);
    if resolved.is_dir() {
        let mut children = read_children(path, *entries)?;
        *entries -= children.len();
        children.sort();
        for child in children {
            fingerprint_tree(root, &child, visited, hash, remaining, entries, depth + 1)?;
        }
    } else {
        let mut file = fs::File::open(&resolved)?;
        let length = file.metadata()?.len();
        if length > *remaining {
            return Err(Error::Invalid(
                "Plugin bundle exceeds 512 MiB scan limit".into(),
            ));
        }
        hash.update(length.to_le_bytes());
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            *remaining = remaining
                .checked_sub(count as u64)
                .ok_or_else(|| Error::Invalid("Plugin bundle grew past scan limit".into()))?;
            hash.update(&buffer[..count]);
        }
    }
    Ok(())
}

fn read_children(path: &Path, maximum: usize) -> std::io::Result<Vec<PathBuf>> {
    let mut children = Vec::new();
    for entry in fs::read_dir(path)? {
        if children.len() == maximum {
            return Err(std::io::Error::other(
                "Plugin directory exceeds traversal entry limit",
            ));
        }
        children.push(entry?.path());
    }
    Ok(children)
}

pub fn module_executable(format: PluginFormat, path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_owned());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = format;
        crate::macos_bundle::Bundle::open(path)?.executable()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let stem = path
            .file_stem()
            .ok_or_else(|| Error::Invalid("Plugin bundle has no name".into()))?;
        let contents = path.join("Contents");
        let directory = if cfg!(target_os = "windows") {
            contents.join(format!(
                "{}-win",
                if cfg!(target_arch = "x86_64") {
                    "x86_64"
                } else if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x86"
                }
            ))
        } else {
            contents.join(format!(
                "{}-linux",
                if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else {
                    std::env::consts::ARCH
                }
            ))
        };
        let mut name = stem.to_os_string();
        if cfg!(target_os = "windows") {
            name.push(format!(".{}", format.extension()));
        } else if cfg!(target_os = "linux") {
            name.push(".so");
        }
        let executable = directory.join(name);
        if executable.is_file() {
            Ok(executable)
        } else {
            Err(Error::Invalid(format!(
                "No compatible native plugin payload at {}",
                executable.display()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roots_cover_user_system_application_and_environment_without_duplicates() {
        let env = SearchEnvironment {
            home: "/users/test".into(),
            application: "/apps/daw".into(),
            windows_common: Some("/common".into()),
            windows_user_common: Some("/usercommon".into()),
            clap_path: vec!["/custom".into(), "/custom".into(), PathBuf::new()],
        };
        for platform in [
            HostPlatform::Mac,
            HostPlatform::Windows,
            HostPlatform::Linux,
        ] {
            let roots = search_roots(platform, &env);
            assert_eq!(
                roots
                    .iter()
                    .filter(|r| r.path == Path::new("/custom"))
                    .count(),
                1
            );
            assert!(
                roots
                    .iter()
                    .any(|r| r.format == PluginFormat::Vst3 && r.path.starts_with("/apps/daw"))
            );
        }
        let linux = search_roots(HostPlatform::Linux, &env);
        assert!(linux.iter().any(|r| r.path == Path::new("/usr/lib64/vst3")));
        let win = search_roots(HostPlatform::Windows, &env);
        assert_eq!(win[0].path, Path::new("/usercommon/VST3"));
        assert_eq!(win[1].path, Path::new("/common/VST3"));
    }
    #[test]
    fn empty_environment_entries_never_scan_current_directory() {
        let separator = if cfg!(windows) { ";" } else { ":" };
        assert!(split_clap_path(&OsString::from(separator)).is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn discovery_deduplicates_symlink_modules_and_stops_directory_cycles() {
        let root = std::env::temp_dir().join(format!("daw-discovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        {
            let module = root.join("Native.clap");
            fs::copy(std::env::current_exe().unwrap(), &module).unwrap();
            std::os::unix::fs::symlink(&module, root.join("Alias.clap")).unwrap();
            std::os::unix::fs::symlink(&root, root.join("Cycle")).unwrap();
            let roots = [SearchRoot {
                format: PluginFormat::Clap,
                path: root.clone(),
            }];
            let found = discover(&roots);
            assert!(found.warnings.is_empty(), "{:?}", found.warnings);
            assert_eq!(found.candidates.len(), 1);
            let before = found.candidates[0].fingerprint.clone();
            let mut file = fs::OpenOptions::new().append(true).open(&module).unwrap();
            std::io::Write::write_all(&mut file, b"changed").unwrap();
            assert_ne!(discover(&roots).candidates[0].fingerprint, before);
            assert!(read_children(&root, 2).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
