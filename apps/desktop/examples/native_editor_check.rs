//! Exercise the host-owned parent and SDK editor lifecycle with an owned product.
#[cfg(target_os = "macos")]
#[path = "../src/hosting/window.rs"]
mod window;
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use daw_plugin_host::*;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    let mut args = std::env::args_os().skip(1);
    let format = match args.next().and_then(|a| a.into_string().ok()).as_deref() {
        Some("clap") => PluginFormat::Clap,
        Some("vst3") => PluginFormat::Vst3,
        _ => return Err("Expected clap|vst3 and an owned plugin bundle".into()),
    };
    let path = PathBuf::from(args.next().ok_or("Missing bundle")?).canonicalize()?;
    let app = NSApplication::sharedApplication(MainThreadMarker::new().ok_or("Not main")?);
    app.finishLaunching();
    let candidate = discover(&[SearchRoot { format, path }])
        .candidates
        .remove(0);
    let descriptor = unsafe { inspect_native(format, &candidate.path) }?.remove(0);
    let mut host = NativeHost::new();
    let id = unsafe { host.load(&candidate, &descriptor.identity, 48000, 512, None) }?;
    assert!(host.metadata(id)?.editor_supported);
    for _ in 0..3 {
        let mut parent = window::Window::new(&descriptor.name)?;
        let size = unsafe { host.open_editor(id, parent.parent(), parent.scale(1.0)) }?;
        assert!(size.0 > 0 && size.1 > 0);
        parent.resize(size.0, size.1);
        parent.resizable(host.editor_resizable(id)?);
        parent.show();
        host.editor_focus(id, true)?;
        let until = Instant::now() + Duration::from_millis(250);
        while Instant::now() < until {
            host.pump(|_, event| {
                assert!(!matches!(event, NativeNotification::Fault));
            });
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(parent.visible());
        assert_eq!(parent.size(), size);
        let _focused = parent.focused();
        assert_eq!(host.resize_editor(id, size.0, size.1)?, size);
        host.editor_focus(id, false)?;
        host.close_editor(id)?;
        drop(parent);
        host.pump(|_, _| {});
    }
    host.remove(id)?;
    host.pump(|_, _| {});
    println!("PASS: {format:?} native editor attach, size, focus, close, reopen and retirement");
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Run the native desktop editor smoke test on this platform");
}
