# Command-line interface

Inspect and edit saved projects, collect sources and export mixes, ranges and stems without opening a window or audio device. Save and close the project in the desktop app before editing it from the CLI.

Use the [headless tool build instructions](../development/README.md#headless-tools)
to prepare the executable. From the repository root, you can prefix the
arguments with `cargo run -p daw-cli --`.

The [complete command catalog](catalog.md) covers every dotted command.

| Command | Purpose |
| --- | --- |
| [`track.list`](tracks.md) | List tracks, with an optional exact-name filter. |
| [`track.show`](tracks.md) | Inspect one track. |
| [`track.add`](tracks.md#add-an-empty-track) | Add an empty audio, MIDI or beat track. |
| [`track.rename`](tracks.md#rename-a-track) | Rename one track by name or ID. |
| [`project.validate`](validate.md) | Check a project and its referenced audio. |
| [`project.source.collect`](collect-sources.md) | Copy original media into a saved project. |
| [`project.export.mix`](render.md) | Export a stereo mix as WAV, MP3 or AAC. |
| [`plugin.catalog.list`](plugins.md) | Read the saved machine-local plugin catalog. |
| [`plugin.catalog.scan`](plugins.md) | Refresh the catalog with isolated VST3 and CLAP scans. |

Place `--project FOLDER` before a project command. Use `--project .` for the
current folder. Quote paths and names that contain spaces. Plugin, effect, and chord catalog commands take no project folder.

Use `daw-cli --help` for the namespaces, `daw-cli project.export` to list a
namespace's children, and `daw-cli project.export.mix --help` for the command's
arguments. Every public command supports `--help` without its usual arguments.

Place `--json` before a dotted command to get one structured success result.
It contains `schema_version`, `command` and `result`. Path strings in JSON are
for display and may lose characters that cannot be represented in Unicode.

Existing scripts can keep using `tracks`, `plugins`, `validate`,
`collect-sources` and `render` with their current options and output.

Successful commands return exit code **0**. Project, media, processing or output failures return **1**. Invalid arguments return **2**. Warnings and errors appear on standard error; results appear on standard output. Missing sources produce warnings and silence rather than failing validation or export. Collection requires every source to pass verification.

The separate [time-stretch and pitch tool](timestretch.md) processes WAV files without a DAW project.

Audio validation and rendering reuse the application's saved audio cache. The CLI still waits for complete preparation and uses the saved project state. Missing or invalid cache entries are rebuilt from the source recordings. See [background audio preparation](../projects/README.md#background-audio-preparation) and [cache configuration](../development/README.md#audio-cache-and-preparation-benchmark).

Beat tracks are listed as **Beat**. `track.show` also shows their subtrack instruments, trigger pitches and output mix settings. Validation, rendering and source collection support embedded beat patterns and their Sampler recordings.
