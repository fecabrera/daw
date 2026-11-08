//! Compatibility alias for the default Linux GTK launcher.
#[cfg(target_os = "linux")]
#[path = "../src/gtk_host.rs"]
mod gtk_host;

#[cfg(target_os = "linux")]
fn main() -> gtk4::glib::ExitCode {
    gtk_host::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The GTK launcher is available on Linux only.");
}
