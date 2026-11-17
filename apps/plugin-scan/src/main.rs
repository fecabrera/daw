//! Optional standalone scanner for tools and custom hosts.
fn main() {
    if let Err(error) = daw_plugin_host::run_scanner(std::env::args_os().skip(1)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
