use crate::{
    Error, HostedPluginDescriptor, HostedRole, PluginCandidate, PluginFormat, PluginIdentity,
    Result,
};
use libloading::Library;
use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "windows"))]
use std::ffi::c_void;
use std::{
    ffi::{CString, c_char},
    io::Read,
    marker::PhantomData,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_CLASSES: usize = 4096;
pub const MAX_SCAN_BYTES: u64 = 8 * 1024 * 1024;
pub const SCAN_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResponse {
    pub version: u32,
    pub plugins: Vec<HostedPluginDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub candidate: PluginCandidate,
    pub plugins: Vec<HostedPluginDescriptor>,
}

/// A scan executes foreign initializers only in a disposable process. Call from a worker.
/// Passing `current_exe()` selects embedded mode; the executable must call
/// `dispatch_embedded_scan()` before normal startup. Other paths use the
/// standalone scanner argument protocol.
pub fn scan_isolated(
    scanner: &Path,
    candidate: &PluginCandidate,
    scratch: &Path,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<ScanResult> {
    if crate::discovery::candidate(candidate.format, candidate.path.clone())? != *candidate {
        return Err(Error::Invalid("Plugin changed before scanning".into()));
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(scratch)?;
    let path = scratch.join(format!(
        "scan-{}-{}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // Reserve the destination. Never truncate a pre-existing file, including a symlink.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    drop(file);
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let mut command = Command::new(scanner);
    // The desktop and CLI embed the scanner. Explicit helper paths retain the
    // standalone protocol for custom hosts and scanner supervision tests.
    if std::env::current_exe().is_ok_and(|path| path == scanner) {
        command.arg(crate::EMBEDDED_SCAN_ARGUMENT);
    }
    let child = command
        .arg("--format")
        .arg(candidate.format.extension())
        .arg("--plugin")
        .arg(&candidate.path)
        .arg("--result")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(child);
    let start = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                return Err(Error::Invalid(format!(
                    "Plugin scanner exited with {status}: {}",
                    candidate.path.display()
                )));
            }
            break;
        }
        let oversized = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SCAN_BYTES);
        if cancelled() || start.elapsed() >= timeout || oversized {
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err(Error::Invalid(
                if oversized {
                    "Plugin scanner output exceeded 8 MiB"
                } else if start.elapsed() >= timeout {
                    "Plugin scanner timed out"
                } else {
                    "Plugin scan cancelled"
                }
                .into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let file = std::fs::File::open(&path)?;
    if file.metadata()?.len() > MAX_SCAN_BYTES {
        return Err(Error::Invalid(
            "Plugin scanner output exceeded 8 MiB".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SCAN_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SCAN_BYTES {
        return Err(Error::Invalid(
            "Plugin scanner output exceeded 8 MiB".into(),
        ));
    }
    let response: ScanResponse = serde_json::from_slice(&bytes)?;
    if response.version != SCAN_PROTOCOL_VERSION {
        return Err(Error::Invalid(
            "Incompatible plugin scanner protocol".into(),
        ));
    }
    let plugins = response.plugins;
    if plugins.len() > MAX_CLASSES
        || plugins
            .iter()
            .any(|p| !p.valid() || p.identity.format() != candidate.format)
    {
        return Err(Error::Invalid("Invalid plugin scanner response".into()));
    }
    let mut ids = std::collections::HashSet::new();
    if plugins.iter().any(|p| !ids.insert(&p.identity)) {
        return Err(Error::Invalid(
            "Duplicate plugin identities in scanner response".into(),
        ));
    }
    if crate::discovery::candidate(candidate.format, candidate.path.clone())? != *candidate {
        return Err(Error::Invalid("Plugin changed during scanning".into()));
    }
    Ok(ScanResult {
        candidate: candidate.clone(),
        plugins,
    })
}

/// Execute only in the scanner process, on its main thread. Opens native executable code.
///
/// # Safety
/// The caller must isolate foreign code and uphold each module's ABI/thread contract.
/// No Rust boundary can recover from arbitrary native pointer corruption or a hang.
pub unsafe fn inspect_native(
    format: PluginFormat,
    path: &Path,
) -> Result<Vec<HostedPluginDescriptor>> {
    match format {
        PluginFormat::Clap => unsafe { inspect_clap(path) },
        PluginFormat::Vst3 => unsafe { inspect_vst3(path) },
    }
}

pub(crate) unsafe fn text(pointer: *const c_char) -> Result<String> {
    if pointer.is_null() {
        return Ok(String::new());
    }
    for length in 0..=4096 {
        // SAFETY: the ABI promises a readable terminated plugin-owned string.
        if unsafe { *pointer.add(length) } == 0 {
            return Ok(String::from_utf8_lossy(unsafe {
                std::slice::from_raw_parts(pointer.cast::<u8>(), length)
            })
            .into_owned());
        }
    }
    Err(Error::Invalid("Plugin string exceeds 4096 bytes".into()))
}

unsafe fn inspect_clap(path: &Path) -> Result<Vec<HostedPluginDescriptor>> {
    use daw_plugin_clap::sys::version::clap_version_is_compatible;
    let module =
        unsafe { crate::clap_module::ClapModule::open(path, crate::clap_module::Use::Scan) }?;
    let factory = module.factory();
    let count = factory
        .get_plugin_count
        .ok_or_else(|| Error::Invalid("Missing CLAP descriptor count".into()))?;
    let descriptor = factory
        .get_plugin_descriptor
        .ok_or_else(|| Error::Invalid("Missing CLAP descriptor lookup".into()))?;
    let count = unsafe { count(factory) } as usize;
    if count > MAX_CLASSES {
        return Err(Error::Invalid(
            "CLAP factory exceeds 4096 descriptors".into(),
        ));
    }
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let d = unsafe { descriptor(factory, index as u32).as_ref() }
            .ok_or_else(|| Error::Invalid("Null CLAP descriptor".into()))?;
        if !clap_version_is_compatible(d.clap_version) {
            return Err(Error::Invalid(
                "Incompatible CLAP descriptor version".into(),
            ));
        }
        let mut role = HostedRole::Unknown;
        if !d.features.is_null() {
            let mut terminated = false;
            for n in 0..128 {
                let feature = unsafe { *d.features.add(n) };
                if feature.is_null() {
                    terminated = true;
                    break;
                }
                match unsafe { text(feature) }?.as_str() {
                    "instrument" => role = HostedRole::Instrument,
                    "audio-effect" if role != HostedRole::Instrument => role = HostedRole::Effect,
                    _ => {}
                }
            }
            if !terminated {
                return Err(Error::Invalid(
                    "CLAP feature list exceeds 128 entries".into(),
                ));
            }
        }
        result.push(HostedPluginDescriptor {
            identity: PluginIdentity::Clap(unsafe { text(d.id) }?),
            name: unsafe { text(d.name) }?,
            vendor: unsafe { text(d.vendor) }?,
            version: unsafe { text(d.version) }?,
            role,
        });
    }
    Ok(result)
}

pub(crate) fn path_cstring(path: &Path) -> Result<CString> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let bytes = path
        .to_str()
        .ok_or_else(|| Error::Invalid("Plugin path cannot be represented as UTF-8".into()))?
        .as_bytes()
        .to_vec();
    CString::new(bytes).map_err(|_| Error::Invalid("Plugin path contains NUL".into()))
}

/// Keeps the executable and platform module entry alive until all COM objects are released.
pub(crate) struct VstModule {
    #[cfg(target_os = "macos")]
    _bundle: crate::macos_bundle::Bundle,
    pub(crate) library: Library,
    exit: unsafe extern "system" fn() -> bool,
    _main: PhantomData<*mut ()>,
}
impl Drop for VstModule {
    fn drop(&mut self) {
        // SAFETY: every factory is released before module exit. Field order then
        // releases bundle metadata and finally unloads the executable.
        unsafe {
            (self.exit)();
        }
    }
}
impl VstModule {
    pub(crate) unsafe fn open(path: &Path) -> Result<Self> {
        let executable = crate::module_executable(PluginFormat::Vst3, path)?;
        #[cfg(target_os = "macos")]
        let bundle = crate::macos_bundle::Bundle::open(path)?;
        let library = unsafe { Library::new(executable) }?;
        #[cfg(target_os = "macos")]
        let (entry_name, exit_name, argument) = (
            b"bundleEntry\0".as_slice(),
            b"bundleExit\0".as_slice(),
            bundle.0.cast_mut(),
        );
        #[cfg(target_os = "linux")]
        let (library, argument) = {
            let native: libloading::os::unix::Library = library.into();
            let handle = native.into_raw();
            (
                Library::from(unsafe { libloading::os::unix::Library::from_raw(handle) }),
                handle,
            )
        };
        #[cfg(target_os = "linux")]
        let (entry_name, exit_name) = (b"ModuleEntry\0".as_slice(), b"ModuleExit\0".as_slice());
        #[cfg(target_os = "windows")]
        let exit_name = b"ExitDll\0".as_slice();
        let exit = *unsafe { library.get::<unsafe extern "system" fn() -> bool>(exit_name) }?;
        #[cfg(not(target_os = "windows"))]
        let initialized = unsafe {
            library.get::<unsafe extern "system" fn(*mut c_void) -> bool>(entry_name)?(argument)
        };
        #[cfg(target_os = "windows")]
        let initialized =
            unsafe { library.get::<unsafe extern "system" fn() -> bool>(b"InitDll\0")?() };
        if !initialized {
            return Err(Error::Invalid("VST3 module initialization failed".into()));
        }
        Ok(Self {
            #[cfg(target_os = "macos")]
            _bundle: bundle,
            library,
            exit,
            _main: PhantomData,
        })
    }
}

unsafe fn inspect_vst3(path: &Path) -> Result<Vec<HostedPluginDescriptor>> {
    use daw_plugin_vst3::sys::{ComPtr, Steinberg::*};
    let module = unsafe { VstModule::open(path) }?;
    let get_factory = unsafe {
        module
            .library
            .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0")
    }?;
    let factory = unsafe { ComPtr::from_raw(get_factory()) }
        .ok_or_else(|| Error::Invalid("Null VST3 factory".into()))?;
    let count = unsafe { factory.countClasses() };
    if !(0..=MAX_CLASSES as i32).contains(&count) {
        return Err(Error::Invalid("Invalid VST3 class count".into()));
    }
    let mut info: PFactoryInfo = unsafe { std::mem::zeroed() };
    if unsafe { factory.getFactoryInfo(&mut info) } != kResultOk {
        return Err(Error::Invalid("VST3 factory info failed".into()));
    }
    let vendor = array_text(&info.vendor);
    let extended = factory.cast::<IPluginFactory2>();
    let mut result = Vec::new();
    for index in 0..count {
        let mut info: PClassInfo = unsafe { std::mem::zeroed() };
        if unsafe { factory.getClassInfo(index, &mut info) } != kResultOk {
            return Err(Error::Invalid("VST3 class info failed".into()));
        }
        if array_text(&info.category) != "Audio Module Class" {
            continue;
        }
        let mut role = HostedRole::Unknown;
        let mut version = String::new();
        let mut class_vendor = vendor.clone();
        if let Some(factory) = &extended {
            let mut more: PClassInfo2 = unsafe { std::mem::zeroed() };
            if unsafe { factory.getClassInfo2(index, &mut more) } == kResultOk {
                let categories = array_text(&more.subCategories);
                if categories.split('|').any(|s| s == "Instrument") {
                    role = HostedRole::Instrument;
                } else if categories.split('|').any(|s| s == "Fx") {
                    role = HostedRole::Effect;
                }
                version = array_text(&more.version);
                if !array_text(&more.vendor).is_empty() {
                    class_vendor = array_text(&more.vendor);
                }
            }
        }
        result.push(HostedPluginDescriptor {
            identity: PluginIdentity::Vst3(canonical_tuid(info.cid)),
            name: array_text(&info.name),
            vendor: class_vendor,
            version,
            role,
        });
    }
    Ok(result)
}

pub(crate) fn array_text(bytes: &[c_char]) -> String {
    let length = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..length].iter().map(|b| *b as u8).collect::<Vec<_>>())
        .into_owned()
}
fn canonical_tuid(id: daw_plugin_vst3::sys::Steinberg::TUID) -> [u8; 16] {
    let mut bytes = id.map(|b| b as u8);
    if cfg!(target_os = "windows") {
        bytes[..4].reverse();
        bytes[4..6].reverse();
        bytes[6..8].reverse();
    }
    bytes
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("daw-scan-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn module(&self) -> PluginCandidate {
            let module = self.0.join("Native.clap");
            // Header-valid executable; the test scanner never opens it as a plugin.
            std::fs::copy(std::env::current_exe().unwrap(), &module).unwrap();
            crate::discovery::candidate(PluginFormat::Clap, module).unwrap()
        }
        fn scanner(&self, body: &str) -> PathBuf {
            let path = self.0.join("scanner");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn bounded_child_response_rejects_corruption_and_reaps_timeout() {
        let scratch = Scratch::new();
        let candidate = scratch.module();
        let scanner = scratch.scanner("printf '{\"version\":1,\"plugins\":[]}' > \"$6\"");
        assert!(
            scan_isolated(
                &scanner,
                &candidate,
                &scratch.0,
                Duration::from_secs(2),
                || false
            )
            .unwrap()
            .plugins
            .is_empty()
        );
        let scanner = scratch.scanner("printf 'invalid json' > \"$6\"");
        assert!(
            scan_isolated(
                &scanner,
                &candidate,
                &scratch.0,
                Duration::from_secs(2),
                || false
            )
            .is_err()
        );
        let scanner = scratch.scanner("exec sleep 5");
        assert!(
            scan_isolated(
                &scanner,
                &candidate,
                &scratch.0,
                Duration::from_millis(20),
                || false
            )
            .unwrap_err()
            .to_string()
            .contains("timed out")
        );
        assert!(std::fs::read_dir(&scratch.0).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("scan-")
        }));
    }
    #[test]
    fn scan_rejects_changed_modules_before_spawning() {
        let scratch = Scratch::new();
        let candidate = scratch.module();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&candidate.path)
            .unwrap();
        std::io::Write::write_all(&mut file, b"changed").unwrap();
        let error = scan_isolated(
            &scratch.0.join("missing-scanner"),
            &candidate,
            &scratch.0,
            Duration::from_secs(1),
            || false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed before scanning"));
    }
}
