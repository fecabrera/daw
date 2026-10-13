# Desktop DAW

A desktop digital audio workstation for macOS, Windows and Linux, written in Rust. It uses external WAV sources, and shares its rendering engine with a headless command-line interface.

## Current features

- **Timeline:** arrange audio clips, trim, loop, stretch, move, copy, and duplicate selections. Placement and resizing make room by trimming, splitting, or removing covered clips. Edits preview before committing, with grid snapping and Shift to bypass it.
- **MIDI:** create MIDI tracks and patterns. Edit notes in a piano roll and build one instrument from parallel layers in Instrument. New MIDI tracks start with an empty layer; add sine, saw, square, or triangle sources and an optional ADSR envelope to each layer. Drag layer names or blank header space to reorder and double-click names to rename; both persist with the project. Each oscillator has its own pitch and gain. Each layer’s ADSR supports stage curves and curve strength. Shared unit headers provide circular on/off controls, including when collapsed. Collapse a layer to the track row height with its header button, retaining unit headers and footers. Layer and unit collapse states are saved with the project. Patterns support playback, shared clip actions in Timeline and Editor, and export.
- **Effects:** add serial Auto-filter, Drive, and 3-band EQ instances to audio or MIDI tracks by double-clicking the catalog name. Adjust Gain/Frequency/Q for Low and High, and Gain for the middle shelf. Mid uses the outer frequency boundaries and Q slopes; Mid off cuts that range. All bands off produces silence. Drive provides Volume/Gain/Tone and Color Shape/Strength/Frequency strips with soft clipping. Auto-filter adds four filter shapes, a tempo/Hz LFO with signed phase controls, and output Level/Dry-Wet. Click to select an installed unit; Shift-click toggles selection. Click the empty lane to clear selection. Drag a header to move the selected group in chain order. Compact and horizontal collapse states persist.
- **Mixer:** adjust track gain, pan, mute, and solo, plus master gain. Timeline and Mixer share controls, values, and selection.
- **Playback:** tempo-based ruler, loop selection, stereo meters, and trackpad scrolling and zoom.
- **Projects:** folder-based saves, project naming, and configurable processing sample rate and buffer size. Source recordings remain external and unchanged.
- **Appearance:** dark and light themes, Material accent colors, colored clips, and native/custom titlebars on macOS and Windows.
- **Export:** stereo WAV (PCM16, PCM24, or float32) and MP3, with progress and diagnostics. The CLI validates projects and exports without a window or audio device.

## Status and planned work

The project format is experimental. Tracks support external audio files and embedded MIDI patterns. Undo/redo is outside the current MVP. Missing sources remain visible and render as silence; saving a project does not bundle its recordings.

The views are **Timeline**, **Instrument**, **Effects**, **Editor**, and **Mixer**. Instrument configures MIDI sources and their layer envelopes under the Layers panel. Effects has an effects catalog at the upper left, the shared Editor ruler and selected-track strip at the top right, and a full-width effects lane at the bottom. The timeline scrollbar sits immediately above the lane; one thin separator separates it from the status bar. It accepts audio and MIDI tracks and provides serial [3-band EQ](docs/equalizer.md), [Drive](docs/drive.md), and [Auto-filter](docs/auto-filter.md) instances. Editor provides a basic MIDI piano roll. Audio and beat editors, beat tracks, additional effects, Master/lane effect chains, and instrument graphs are planned.

MIDI patterns are immutable clip sources: body-edge trims crop or pad a new source for the edited clip, while header-edge looping preserves the source. Other clips retain their previous source. MIDI tracks play overlapping pitches with multiple voices of their configured instrument. Beat patterns will follow the same source model.

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

#### Package a ZIP

1. Configure the build tools and libclang as above.
2. Run the Windows packager from the repository root:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File tools/package_windows.ps1 release
   ```

   Use `debug` for a debug build. The default is `debug`, matching the macOS packager. `RemoteSigned` applies only to this PowerShell process; it does not change the saved execution policy.
3. Take `target\release\DAW.zip` or `target\debug\DAW.zip`. If Cargo uses a different target directory or build target, the ZIP is placed beside the built executable. The script prints its full path.
4. Extract the ZIP and run `DAW\daw-desktop.exe`. Both profiles launch as Windows GUI applications without opening a terminal. Keep `DAW\LICENSE.txt` and the `DAW\licenses` folder with the executable.

The script builds the desktop with the lockfile, includes the same font, icon, UI, media, oscillator, and EQ notices as the macOS bundle, and replaces an existing ZIP only after packaging succeeds. It includes no CLI, projects, recordings, build cache, or debug symbols. The ZIP is unsigned. It does not include the Microsoft C++ runtime: destination PCs need the [Microsoft Visual C++ v14 Redistributable for x64](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) and an OpenGL-capable graphics driver. Rust, LLVM, Rubber Band, and LAME installations are not needed to run it.

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

Create a disk image with `sh tools/package_macos_dmg.sh release`. Use `debug` for a debug build. The image is `target/<profile>/DAW-macos-arm64.dmg` on Apple Silicon or `DAW-macos-x64.dmg` on Intel. Open the image and drag DAW to Applications. The app includes the project license and dependency notices in `Contents/Resources`. The image is unsigned and the app is not notarized.

### Linux

Install Rust through Rustup, a C++ compiler, Clang/libclang, and ALSA development libraries. Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

Create a portable ZIP with `sh tools/package_linux.sh release`. Use `debug` for a debug build. The archive is `target/<profile>/DAW-linux-x64.zip` on x64 or `DAW-linux-arm64.zip` on ARM64. Extract it and run `DAW/daw-desktop`. Keep `DAW/LICENSE.txt` and `DAW/licenses` with the executable. The archive preserves executable permissions and includes the same dependency notices as the other packages. It does not bundle system libraries: the destination needs compatible C/C++ runtime, ALSA, graphics libraries, and a desktop portal for dialogs.

### GitHub Actions builds

The [Test and build workflow](.github/workflows/build-and-test.yml) runs on pushes, pull requests, and manual dispatches. It has separate `test` and `build` jobs, each with Windows, macOS, and Linux workers. Tests run formatting, strict workspace/all-target Clippy, workspace tests, and the headless CLI smoke checks with the pinned Rust toolchain and locked dependencies. Build starts only after all three test workers pass, builds and packages release binaries on fresh workers, and uploads artifacts retained for 14 days. A failing worker does not cancel the other workers in its job. Both jobs use the same [native build setup action](.github/actions/setup-build/action.yml).

| Platform | Runner | Download |
| --- | --- | --- |
| Windows x64 | `windows-2025` | `DAW-windows-x64`: portable `daw-desktop.exe`, project license, and dependency notices |
| macOS Apple Silicon | `macos-15` | `DAW-macos-arm64.dmg` |
| Linux x64 | `ubuntu-24.04` | `DAW-linux-x64.zip` |

Download artifacts from the workflow run's summary. Windows downloads as a ZIP containing the executable and licenses; it is not an installer. The disk image and Linux ZIP upload directly as files. These builds do not sign binaries, notarize the macOS app, or publish a GitHub release. The Linux package targets Ubuntu 24.04 and compatible systems; it is not a static binary for every distribution. Native UI and audio-device checks remain separate from CI.

## Documentation

- [Project scope](docs/MVP.md) and [implementation specification](specs/mvp-iteration-1.md).
- [Modular audio units](docs/modular-audio-units.md) and [beat pattern editor](docs/beat-editor.md) designs and decisions.
- [MIDI patterns and oscillators](docs/midi-patterns.md), including current controls and limits.
- [Validation records](docs/validation.md).
- [Contribution and development instructions](AGENTS.md), including architecture, documentation rules, and checks.

## Licenses

Original code uses the [BSD 2-Clause License](LICENSE). Bundled dependencies have separate terms. Their notices are included with the [media libraries](crates/media/assets/licenses), [fonts](crates/ui/assets/fonts), [icons](crates/ui/assets/icons), [UI dependencies](crates/ui/assets/licenses), and [oscillator dependencies](crates/oscillator/assets/licenses), and [EQ dependencies](crates/equalizer/assets/licenses).
