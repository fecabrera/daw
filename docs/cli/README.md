# Command-line interface

Inspect tracks, validate projects and export a mix without opening a window or an audio device. Commands use the saved project; save desktop edits first.

Build from the repository root with `cargo build -p daw-cli`. The executable is `target/debug/daw-cli` (`daw-cli.exe` on Windows). Platform prerequisites are in the [development guides](../development/README.md). The examples below use `daw-cli` on your command search path. From the repository root, you can also use `cargo run -p daw-cli --` before the command and its arguments.

| Command | Purpose |
| --- | --- |
| [tracks](tracks.md) | List tracks or show details for a track name. |
| [validate](validate.md) | Check a project and its referenced audio. |
| [render](render.md) | Export the saved mix as WAV, MP3 or AAC. |

Use `daw-cli --help`, `daw-cli <command> --help` or `daw-cli --version` for help and version information. Quote paths and names that contain spaces.

Successful commands return exit code **0**. Project, media, processing or output failures return **1**. Invalid arguments return **2**. Warnings and errors appear on standard error; results appear on standard output. Missing sources produce warnings and silence rather than failing validation or export.

The separate [time-stretch and pitch tool](timestretch.md) processes WAV files without a DAW project.
