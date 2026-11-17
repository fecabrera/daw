//! Main-thread entry points for disposable native inspection.
use std::path::PathBuf;

pub const EMBEDDED_SCAN_ARGUMENT: &str = "--daw-plugin-scan";

/// Dispatch before initializing UI, devices, or the application's argument parser.
/// A child uses the same executable, but never starts the application.
pub fn dispatch_embedded_scan() {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_some_and(|arg| arg == EMBEDDED_SCAN_ARGUMENT) {
        match run_scanner(args) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
}

/// Run only in a disposable process, on its main thread.
pub fn run_scanner(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut format = None;
    let mut plugin = None;
    let mut output = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or("Scanner argument needs a value")?;
        if key == "--format" {
            format = Some(match value.to_str() {
                Some("clap") => crate::PluginFormat::Clap,
                Some("vst3") => crate::PluginFormat::Vst3,
                _ => return Err("Unsupported scanner format".into()),
            });
        } else if key == "--plugin" {
            plugin = Some(PathBuf::from(value));
        } else if key == "--result" {
            output = Some(PathBuf::from(value));
        } else {
            return Err(format!("Unknown scanner argument: {}", key.to_string_lossy()).into());
        }
    }
    let format = format.ok_or("Missing --format")?;
    let path = plugin.ok_or("Missing --plugin")?.canonicalize()?;
    let output = output.ok_or("Missing --result")?;
    // SAFETY: this disposable process isolates foreign initializers. Its main
    // thread owns all module calls until synchronous inspection is complete.
    let plugins = unsafe { crate::inspect_native(format, &path) }?;
    if plugins.iter().any(|p| !p.valid()) {
        return Err("Invalid plugin descriptor".into());
    }
    let bytes = serde_json::to_vec(&crate::ScanResponse {
        version: crate::SCAN_PROTOCOL_VERSION,
        plugins,
    })?;
    if bytes.len() as u64 > crate::MAX_SCAN_BYTES {
        return Err("Scanner response exceeds quota".into());
    }
    std::fs::write(output, bytes)?;
    Ok(())
}
