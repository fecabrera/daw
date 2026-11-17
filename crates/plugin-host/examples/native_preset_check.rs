//! A native preset rescan must become an authored base before exact capture.
use daw_plugin_host::*;
use std::path::PathBuf;
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Expected owned CLAP fixture")?,
    )
    .canonicalize()?;
    let candidate = discover(&[SearchRoot {
        format: PluginFormat::Clap,
        path,
    }])
    .candidates
    .remove(0);
    let descriptor = unsafe { inspect_native(PluginFormat::Clap, &candidate.path) }?.remove(0);
    let mut host = NativeHost::new();
    let id = unsafe { host.load(&candidate, &descriptor.identity, 48000, 512, None) }?;
    let before = host.save_state(id)?;
    let library = unsafe { libloading::Library::new(&candidate.executable) }?;
    let choose =
        unsafe { library.get::<unsafe extern "C" fn(f64) -> bool>(b"daw_fixture_clap_preset\0") }?;
    let processor = host.take_processor(id)?;
    assert!(unsafe { choose(0.75) });
    let mut events = Vec::new();
    host.pump(|_, event| events.push(event));
    assert!(events.contains(&NativeNotification::Value(10, 0.75)));
    assert!(events.contains(&NativeNotification::StateDirty));
    assert!(
        events
            .iter()
            .position(|e| *e == NativeNotification::PresetBegin)
            .unwrap()
            < events
                .iter()
                .position(|e| *e == NativeNotification::Value(10, 0.75))
                .unwrap()
    );
    assert!(
        events
            .iter()
            .position(|e| *e == NativeNotification::Value(10, 0.75))
            .unwrap()
            < events
                .iter()
                .position(|e| *e == NativeNotification::PresetEnd)
                .unwrap()
    );
    assert!(
        processor.configuration_valid(),
        "A value rescan must preserve the active processor"
    );
    drop(processor);
    let after = host.save_state(id)?;
    assert_ne!(before, after);
    let NativeState::Clap(bytes) = after else {
        unreachable!()
    };
    assert_eq!(f64::from_le_bytes(bytes[8..16].try_into()?), 0.75);
    host.remove(id)?;
    host.pump(|_, _| {});
    drop(library);
    println!(
        "PASS: native preset rescan, authored base publication, grouped notification and exact opaque state capture"
    );
    Ok(())
}
