//! MIDI device ownership stays outside portable project/rendering crates.
use daw_ui::{MidiBackend, MidiDevice, MidiIngress};
use midir::{MidiInput, MidiInputConnection};
use std::path::PathBuf;
#[derive(Default)]
pub struct NativeMidi {
    connection: Option<MidiInputConnection<()>>,
}
impl MidiBackend for NativeMidi {
    fn devices(&mut self) -> Result<Vec<MidiDevice>, String> {
        let input = MidiInput::new("DAW device list").map_err(|e| e.to_string())?;
        input
            .ports()
            .iter()
            .map(|p| {
                Ok(MidiDevice {
                    id: p.id(),
                    name: input.port_name(p).map_err(|e| e.to_string())?,
                })
            })
            .collect()
    }
    fn connect(&mut self, device: &str, mut ingress: MidiIngress) -> Result<(), String> {
        self.disconnect();
        let input = MidiInput::new("DAW MIDI input").map_err(|e| e.to_string())?;
        let matching: Vec<_> = input
            .ports()
            .into_iter()
            .filter(|port| port.id() == device)
            .collect();
        if matching.len() != 1 {
            return Err("Selected MIDI input is unavailable or its identity is ambiguous".into());
        }
        let port = &matching[0];
        self.connection = Some(
            input
                .connect(
                    port,
                    "DAW live input",
                    move |_, bytes, _| ingress.receive(bytes),
                    (),
                )
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    }
    fn disconnect(&mut self) {
        self.connection = None;
    }
}
pub fn recovery_root() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "linux")]
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    root.map(|p| p.join("DAW/recovery"))
}
