# Desktop DAW

A desktop digital audio workstation for macOS, Windows and Linux, written in Rust. It uses external WAV sources, and shares its rendering engine with a headless command-line interface.

## Current features

- **Timeline:** arrange audio clips, trim, loop, stretch, move, copy, and duplicate selections. Placement and resizing make room by trimming, splitting, or removing covered clips. Edits preview before committing, with grid snapping and Shift to bypass it.
- **MIDI:** create MIDI tracks and patterns. Edit notes in a piano roll and add sine, saw, square, or triangle oscillators to one instrument in Units. Each oscillator has its own pitch and gain. Patterns support playback, clip editing, and export.
- **Mixer:** adjust track gain, pan, mute, and solo, plus master gain. Timeline and Mixer share controls, values, and selection.
- **Playback:** tempo-based ruler, loop selection, stereo meters, and trackpad scrolling and zoom.
- **Projects:** folder-based saves, project naming, and configurable processing sample rate and buffer size. Source recordings remain external and unchanged.
- **Appearance:** dark and light themes, Material accent colors, colored clips, and native/custom titlebars on macOS and Windows.
- **Export:** stereo WAV (PCM16, PCM24, or float32) and MP3, with progress and diagnostics. The CLI validates projects and exports without a window or audio device.

## Status and planned work

The project format is experimental. Tracks support external audio files and embedded MIDI patterns. Undo/redo is outside the current MVP. Missing sources remain visible and render as silence; saving a project does not bundle its recordings.

The views are **Timeline**, **Units**, **Editor**, and **Mixer**. Units currently configures the MIDI oscillator instrument. Editor provides a basic MIDI piano roll. Audio and beat editors, beat tracks, effect chains, and instrument graphs are planned.

MIDI patterns are immutable clip sources: trimming and looping preserve the source, while pattern edits create a new source without changing other clips. MIDI tracks play overlapping pitches with multiple voices of their configured instrument. Beat patterns will follow the same source model.

## Build and run

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](Cargo.toml) and [Cargo.lock](Cargo.lock). Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band or LAME installation.

### Windows

These steps target Windows x64 with the MSVC Rust toolchain. Install the following software before building:

| Software | Required components and purpose |
| --- | --- |
| [Rustup](https://rust-lang.org/tools/install/) | Installs Rust and Cargo. Select the MSVC toolchain. The repository pins the build toolchain in [rust-toolchain.toml](rust-toolchain.toml). |
| [Visual Studio Build Tools](https://learn.microsoft.com/en-us/cpp/overview/acquire-msvc) | Select **Desktop development with C++**, including MSVC x64/x86 build tools and a Windows SDK. These provide `cl.exe`, `link.exe`, headers, and libraries. You can also add this workload to an existing Visual Studio installation. |
| [LLVM](https://github.com/llvm/llvm-project/releases/latest) | Download the **Windows x64 installer** from the official release page. It must include Clang and `libclang.dll`, which bindgen uses to generate the Rubber Band Rust bindings. Install LLVM separately from Rustup. |

[Git for Windows](https://git-scm.com/downloads/win) is needed if you clone the repository. [Python 3](https://www.python.org/downloads/windows/) is needed only for the demo generator and CLI smoke checks. Rubber Band and LAME sources are bundled, so you do not need separate installations of these audio libraries.

The desktop requires an OpenGL-capable graphics driver on Windows. No separate Windows App SDK installation is needed. Include the font, icon, UI dependency, and media notices under `crates/ui/assets` and `crates/media/assets/licenses` when distributing the application.

#### Configure libclang

1. Install LLVM and check that `libclang.dll` exists, usually at `C:\Program Files\LLVM\bin\libclang.dll`.
2. Open Windows **Editor environment variables for your account**. Add a user variable named `LIBCLANG_PATH` with value `C:\Program Files\LLVM\bin`. If LLVM is installed elsewhere, use the folder that contains the DLL. Set the folder, not the DLL file.
3. Fully close and reopen your terminal and IDE so they receive the new environment variable.

See the [bindgen requirements](https://rust-lang.github.io/rust-bindgen/requirements.html#windows) for LLVM installation details. Setting `LIBCLANG_PATH` does not install the DLL.

#### Build from a terminal

1. Open **x64 Native Tools Command Prompt for Visual Studio**. This configures the compiler, linker, and Windows SDK environment.
2. Change to the repository root.
3. Install the pinned Rust toolchain, then build and run:

   ```bat
   rustup show
   cargo build -p daw-desktop
   cargo run -p daw-desktop
   ```

The debug executable is `target\debug\daw-desktop.exe`. For an optimized build, run `cargo build --release -p daw-desktop`; the executable is `target\release\daw-desktop.exe`. Build the CLI with `cargo build -p daw-cli`.

If you prefer to set libclang only for the current shell, use `set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"` in Command Prompt, or `$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'` in PowerShell, before running Cargo.

#### Troubleshoot build errors

| Error | Check and fix |
| --- | --- |
| `linker link.exe not found` | In the developer command prompt, run `where.exe link` and `where.exe cl`. If either is missing, add or repair the C++ workload through Visual Studio Installer. Adding only the linker folder to `PATH` does not configure SDK headers and libraries. |
| `rubberband-sys`: `Unable to find libclang` | Check that `libclang.dll` exists and that `LIBCLANG_PATH` points to its containing folder. Install Windows x64 LLVM if the DLL is missing. Restart the build application after changing Windows environment variables. |

The `CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG` suggestion adds backtrace detail; it does not fix missing build tools or libclang. Windows build, native UI, and audio-device verification remain separate from these setup instructions.

### macOS

Install Rust through Rustup and the native C++ development tools with Clang/libclang. Build and run with:

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

Create an app bundle with `sh tools/package_macos.sh debug`. Open `target/debug/DAW.app`, then select a project folder with File > Open. Use `release` instead of `debug` for an optimized build. The local bundle is unsigned.

### Linux

Install Rust through Rustup, a C++ compiler, Clang/libclang, and ALSA development libraries. Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

## Documentation

- [Project scope](docs/MVP.md) and [implementation specification](specs/mvp-iteration-1.md).
- [Modular audio units](docs/modular-audio-units.md) and [beat pattern editor](docs/beat-editor.md) designs and decisions.
- [MIDI patterns and oscillators](docs/midi-patterns.md), including current controls and limits.
- [Validation records](docs/validation.md).
- [Contribution and development instructions](AGENTS.md), including architecture, documentation rules, and checks.

## Licenses

Original code uses the [BSD 2-Clause License](LICENSE). Bundled dependencies have separate terms. Their notices are included with the [media libraries](crates/media/assets/licenses), [fonts](crates/ui/assets/fonts), [icons](crates/ui/assets/icons), [UI dependencies](crates/ui/assets/licenses), and [oscillator dependencies](crates/oscillator/assets/licenses).
