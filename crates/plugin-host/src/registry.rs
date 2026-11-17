//! Machine-local catalog with persistent scan-failure quarantine.
use crate::{
    Discovery, Error, HostedPluginDescriptor, PluginCandidate, PluginIdentity, Result, ScanResult,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const SCHEMA: u32 = 1;
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MODULES: usize = 16384;

/// Cross-process exclusion for registry refresh. The OS releases the lock on exit.
/// Cached readers need no lock because publication replaces complete files.
pub struct RegistryLock {
    _file: fs::File,
}
impl RegistryLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut name = path
            .file_name()
            .ok_or_else(|| Error::Invalid("Plugin catalog needs a filename".into()))?
            .to_os_string();
        name.push(".lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join(name))?;
        file.try_lock().map_err(|e| {
            Error::Invalid(format!("Plugin catalog is busy or cannot be locked: {e}"))
        })?;
        Ok(Self { _file: file })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum ScanState {
    Ready,
    Pending,
    Scanning,
    Quarantined(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub candidate: PluginCandidate,
    pub plugins: Vec<HostedPluginDescriptor>,
    pub state: ScanState,
    pub present: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginRegistry {
    version: u32,
    /// Entries retain root priority; shadowed identities are not deleted.
    pub entries: Vec<RegistryEntry>,
}
impl Default for PluginRegistry {
    fn default() -> Self {
        Self {
            version: SCHEMA,
            entries: Vec::new(),
        }
    }
}
impl PluginRegistry {
    /// Explicit user rescan; interrupted/failed modules still require retry_failed.
    pub fn request_rescan(&mut self) {
        for entry in &mut self.entries {
            if entry.state == ScanState::Ready {
                entry.state = ScanState::Pending;
            }
        }
    }
    pub fn load(path: &Path) -> Result<Self> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CACHE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            return Err(Error::Invalid("Plugin cache exceeds 32 MiB".into()));
        }
        let mut registry: Self = serde_json::from_slice(&bytes)?;
        registry.validate()?;
        for entry in &mut registry.entries {
            if entry.state == ScanState::Scanning {
                entry.state = ScanState::Quarantined("Previous plugin scan was interrupted".into());
            }
        }
        Ok(registry)
    }
    fn validate(&self) -> Result<()> {
        let mut paths = HashSet::new();
        if self.version != SCHEMA || self.entries.len() > MAX_MODULES {
            return Err(Error::Invalid(
                "Unsupported or oversized plugin registry".into(),
            ));
        }
        for entry in &self.entries {
            let c = &entry.candidate;
            let mut ids = HashSet::new();
            if !paths.insert((c.format, &c.path))
                || c.fingerprint.len() != 64
                || !c.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
                || entry.plugins.len() > 4096
                || entry.plugins.iter().any(|p| {
                    !p.valid() || p.identity.format() != c.format || !ids.insert(&p.identity)
                })
            {
                return Err(Error::Invalid("Invalid plugin registry metadata".into()));
            }
        }
        Ok(())
    }
    /// Failed writes retain the previous cache file.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            return Err(Error::Invalid("Plugin cache exceeds 32 MiB".into()));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".plugin-registry-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            replace(&temporary, path)?;
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        result
    }
    /// Journal before foreign initialization. A coordinator crash cannot cause a startup loop.
    /// Call on a worker with exclusive ownership of this registry.
    pub fn refresh(
        &mut self,
        discovery: &Discovery,
        cache: &Path,
        retry_failed: bool,
        mut cancelled: impl FnMut() -> bool,
        mut scan: impl FnMut(&PluginCandidate) -> Result<ScanResult>,
    ) -> Result<()> {
        if discovery.candidates.len() > MAX_MODULES {
            return Err(Error::Invalid(
                "Plugin discovery exceeds 16384 modules".into(),
            ));
        }
        for entry in &mut self.entries {
            entry.present = false;
        }
        let previous = std::mem::take(&mut self.entries);
        for candidate in &discovery.candidates {
            let prior = previous.iter().find(|e| {
                e.candidate.format == candidate.format && e.candidate.path == candidate.path
            });
            let mut entry = prior.cloned().unwrap_or_else(|| RegistryEntry {
                candidate: candidate.clone(),
                plugins: Vec::new(),
                state: ScanState::Pending,
                present: true,
            });
            let changed = entry.candidate.fingerprint != candidate.fingerprint
                || entry.candidate.executable != candidate.executable;
            entry.present = true;
            entry.candidate = candidate.clone();
            if changed || retry_failed && matches!(entry.state, ScanState::Quarantined(_)) {
                entry.state = ScanState::Pending;
            }
            self.entries.push(entry);
        }
        self.entries.extend(previous.into_iter().filter(|p| {
            !discovery
                .candidates
                .iter()
                .any(|c| c.format == p.candidate.format && c.path == p.candidate.path)
        }));
        for index in 0..self.entries.len() {
            if !self.entries[index].present || self.entries[index].state != ScanState::Pending {
                continue;
            }
            if cancelled() {
                return Err(Error::Invalid("Plugin scan cancelled".into()));
            }
            self.entries[index].state = ScanState::Scanning;
            self.save(cache)?;
            let candidate = self.entries[index].candidate.clone();
            let result = scan(&candidate);
            if cancelled() {
                self.entries[index].state = ScanState::Pending;
                self.save(cache)?;
                return Err(Error::Invalid("Plugin scan cancelled".into()));
            }
            let entry = &mut self.entries[index];
            match result {
                Ok(result)
                    if result.candidate == candidate
                        && result.plugins.len() <= 4096
                        && result
                            .plugins
                            .iter()
                            .all(|p| p.valid() && p.identity.format() == candidate.format)
                        && result
                            .plugins
                            .iter()
                            .map(|p| &p.identity)
                            .collect::<HashSet<_>>()
                            .len()
                            == result.plugins.len() =>
                {
                    entry.plugins = result.plugins;
                    entry.state = ScanState::Ready;
                }
                Ok(_) => {
                    entry.state =
                        ScanState::Quarantined("Scanner returned mismatched metadata".into())
                }
                Err(error) => entry.state = ScanState::Quarantined(error.to_string()),
            }
            self.save(cache)?;
        }
        self.save(cache)
    }
    pub fn resolve(
        &self,
        identity: &PluginIdentity,
    ) -> Option<(&PluginCandidate, &HostedPluginDescriptor)> {
        self.entries
            .iter()
            .filter(|e| e.present)
            .find_map(|e| {
                e.plugins
                    .iter()
                    .find(|p| &p.identity == identity)
                    .map(|p| (e, p))
            })
            .filter(|(e, _)| e.state == ScanState::Ready)
            .map(|(e, p)| (&e.candidate, p))
    }
}
#[cfg(not(target_os = "windows"))]
pub(crate) fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}
#[cfg(target_os = "windows")]
pub(crate) fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are live terminated UTF-16 buffers for this synchronous call.
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn default_registry_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Caches"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CACHE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")));
    Ok(root
        .ok_or_else(|| Error::Invalid("No plugin cache directory is available".into()))?
        .join("daw/plugins-v1/registry.json"))
}
/// Worker-only startup refresh. Foreign enumeration remains in disposable
/// scanner processes; the parent main loop never scans installed modules.
pub fn refresh_default_registry(
    scanner: &Path,
    cancelled: impl Fn() -> bool + Copy,
) -> Result<(PluginRegistry, Vec<String>)> {
    refresh_registry(
        scanner,
        &crate::configured_search_roots()?,
        ScanMode::Changed,
        cancelled,
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanMode {
    Changed,
    Rescan,
    RetryFailed,
}
pub fn refresh_registry(
    scanner: &Path,
    roots: &[crate::SearchRoot],
    mode: ScanMode,
    cancelled: impl Fn() -> bool + Copy,
) -> Result<(PluginRegistry, Vec<String>)> {
    if !scanner.is_file() {
        return Err(Error::Invalid(
            "The scanner executable does not exist".into(),
        ));
    }
    let cache = default_registry_path()?;
    let _lock = RegistryLock::acquire(&cache)?;
    let mut warnings = Vec::new();
    let mut registry = match PluginRegistry::load(&cache) {
        Ok(r) => r,
        Err(e) => {
            warnings.push(format!("Rebuilding plugin catalog: {e}"));
            PluginRegistry::default()
        }
    };
    if mode == ScanMode::Rescan {
        for entry in &mut registry.entries {
            entry.state = ScanState::Pending;
        }
    }
    let discovery = crate::discover(roots);
    warnings.extend(discovery.warnings.iter().cloned());
    let scratch = cache
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("scan-{}", uuid::Uuid::new_v4()));
    let result = registry.refresh(
        &discovery,
        &cache,
        mode == ScanMode::RetryFailed,
        cancelled,
        |candidate| {
            crate::scan_isolated(
                scanner,
                candidate,
                &scratch,
                std::time::Duration::from_secs(10),
                cancelled,
            )
        },
    );
    let _ = fs::remove_dir_all(scratch);
    result?;
    Ok((registry, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostedRole, PluginFormat};
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p =
                std::env::temp_dir().join(format!("daw-plugin-registry-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn candidate(path: &str, byte: char) -> PluginCandidate {
        PluginCandidate {
            format: PluginFormat::Clap,
            path: path.into(),
            executable: path.into(),
            fingerprint: byte.to_string().repeat(64),
        }
    }
    fn descriptor() -> HostedPluginDescriptor {
        HostedPluginDescriptor {
            identity: PluginIdentity::Clap("test.plugin".into()),
            name: "Test".into(),
            vendor: String::new(),
            version: "1".into(),
            role: HostedRole::Effect,
        }
    }
    #[test]
    fn quarantine_survives_restart_and_changed_binary_retries() {
        let scratch = Scratch::new();
        let cache = scratch.0.join("registry.json");
        let mut registry = PluginRegistry::default();
        let mut discovery = Discovery {
            candidates: vec![candidate("test.clap", 'a')],
            warnings: vec![],
        };
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |_| {
                    assert_eq!(
                        PluginRegistry::load(&cache).unwrap().entries[0].state,
                        ScanState::Quarantined("Previous plugin scan was interrupted".into())
                    );
                    Err(Error::Invalid("crashed".into()))
                },
            )
            .unwrap();
        let mut registry = PluginRegistry::load(&cache).unwrap();
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |_| panic!("quarantined scan must not run"),
            )
            .unwrap();
        discovery.candidates[0].fingerprint = "b".repeat(64);
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |c| {
                    Ok(ScanResult {
                        candidate: c.clone(),
                        plugins: vec![descriptor()],
                    })
                },
            )
            .unwrap();
        assert!(registry.resolve(&descriptor().identity).is_some());
        registry
            .refresh(
                &Discovery::default(),
                &cache,
                false,
                || false,
                |_| unreachable!(),
            )
            .unwrap();
        assert!(registry.resolve(&descriptor().identity).is_none());
        assert_eq!(registry.entries[0].plugins.len(), 1);
    }
    #[test]
    fn duplicate_resolution_uses_root_order_and_preserves_shadowed_modules() {
        let scratch = Scratch::new();
        let cache = scratch.0.join("registry.json");
        let discovery = Discovery {
            candidates: vec![candidate("user.clap", 'a'), candidate("system.clap", 'b')],
            warnings: vec![],
        };
        let mut registry = PluginRegistry::default();
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |c| {
                    Ok(ScanResult {
                        candidate: c.clone(),
                        plugins: vec![descriptor()],
                    })
                },
            )
            .unwrap();
        assert_eq!(
            registry.resolve(&descriptor().identity).unwrap().0.path,
            Path::new("user.clap")
        );
        assert_eq!(PluginRegistry::load(&cache).unwrap().entries.len(), 2);
    }
    #[test]
    fn interrupted_scan_quarantines_only_the_attempted_module() {
        let scratch = Scratch::new();
        let cache = scratch.0.join("registry.json");
        let discovery = Discovery {
            candidates: vec![candidate("one.clap", 'a'), candidate("two.clap", 'b')],
            warnings: vec![],
        };
        let mut registry = PluginRegistry::default();
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |c| {
                    let journal = PluginRegistry::load(&cache).unwrap();
                    if c.path == Path::new("one.clap") {
                        assert!(matches!(
                            journal.entries[0].state,
                            ScanState::Quarantined(_)
                        ));
                        assert_eq!(journal.entries[1].state, ScanState::Pending);
                    }
                    Ok(ScanResult {
                        candidate: c.clone(),
                        plugins: vec![],
                    })
                },
            )
            .unwrap();
    }
    #[test]
    fn invalid_metadata_never_replaces_a_successful_scan() {
        let scratch = Scratch::new();
        let cache = scratch.0.join("registry.json");
        let discovery = Discovery {
            candidates: vec![candidate("one.clap", 'a')],
            warnings: vec![],
        };
        let mut registry = PluginRegistry::default();
        registry
            .refresh(
                &discovery,
                &cache,
                false,
                || false,
                |c| {
                    Ok(ScanResult {
                        candidate: c.clone(),
                        plugins: vec![descriptor(), descriptor()],
                    })
                },
            )
            .unwrap();
        assert!(matches!(
            registry.entries[0].state,
            ScanState::Quarantined(_)
        ));
        assert!(registry.entries[0].plugins.is_empty());
        assert!(PluginRegistry::load(&cache).is_ok());
    }
    #[test]
    fn failed_higher_priority_identity_does_not_substitute_a_shadowed_binary() {
        let mut registry = PluginRegistry::default();
        for (path, state) in [
            ("user.clap", ScanState::Quarantined("failed".into())),
            ("global.clap", ScanState::Ready),
        ] {
            registry.entries.push(RegistryEntry {
                candidate: candidate(path, 'a'),
                plugins: vec![descriptor()],
                state,
                present: true,
            });
        }
        assert!(registry.resolve(&descriptor().identity).is_none());
        registry.entries[0].present = false;
        assert_eq!(
            registry.resolve(&descriptor().identity).unwrap().0.path,
            Path::new("global.clap")
        );
    }
    #[test]
    fn refresh_lock_excludes_other_writers_and_releases_without_stale_pid_state() {
        let scratch = Scratch::new();
        let cache = scratch.0.join("registry.json");
        let lock = RegistryLock::acquire(&cache).unwrap();
        assert!(RegistryLock::acquire(&cache).is_err());
        drop(lock);
        assert!(RegistryLock::acquire(&cache).is_ok());
    }
}
