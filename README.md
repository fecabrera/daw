# Desktop DAW

A desktop digital audio workstation for macOS, Windows and Linux, written in Rust. It imports external WAV, MP3, and AAC sources, and shares its rendering engine with a headless command-line interface.

## Current features

- **Timeline:** arrange, trim, loop, stretch, transpose, split, move, copy, and duplicate audio and MIDI clips or a selected time range, with grid snapping, edit previews, and clip settings.
- **MIDI editor:** create patterns and edit notes in a piano roll, with track controls and optional scale snapping.
- **Instruments:** build layered synthesizers, samplers, and granular instruments, with modulation and a shared Output bus.
- **Instrument components:** [Oscillator](docs/instruments/oscillator.md), [Sampler](docs/instruments/sampler.md), [Granulizer](docs/instruments/granulizer.md), [Envelope](docs/instruments/envelope.md), [LFO](docs/instruments/lfo.md), [Amplitude](docs/instruments/amplitude.md), and [Macro](docs/instruments/macro.md).
- **Effects:** [3-band EQ](docs/effects/equalizer.md), [Drive](docs/effects/drive.md), [Bitcrusher](docs/effects/bitcrusher.md), [Saturator](docs/effects/saturator.md), [Filter](docs/effects/filter.md), [Auto-filter](docs/effects/auto-filter.md), [Compressor](docs/effects/compressor.md), [OTT](docs/effects/ott.md), [Limiter](docs/effects/limiter.md), [Direction Mixer](docs/effects/direction-mixer.md), and [Stereo Spread](docs/effects/stereo-spread.md).
- **Mixer:** track gain, pan, mute, and solo controls, [track group buses](docs/editing/track-groups.md) with shared effects, and master gain.
- **Playback:** tempo controls, loop playback, and stereo level meters.
- **Editing history:** [undo and redo](docs/editing/undo-redo.md) for project edits.
- **Projects:** save and open projects, with configurable audio processing settings.
- **Appearance:** dark and light themes, accent colors, and colored clips.
- **Export and CLI:** export stereo WAV, MP3, or AAC files; validate projects and render audio from the command line.
- **Standalone audio tools:** experimental offline time stretching and pitch shifting, with three-engine comparisons.

Stem separation uses local Charon HTDemucs models, with CoreML on Apple Silicon macOS and CPU on supported platforms. Either validated model enables separation; CoreML takes priority, with optional CPU fallback. It creates aligned stem tracks in a group and retains the disabled original clip. See [Stem separation](docs/audio/stem-separation.md) for setup and limits.

## Status and planned work

Audio and MIDI workflows are available in four views: **Timeline**, **Instrument**, **Editor**, and **Mixer**.

The project format is experimental. Saved projects reference external recordings. Generated stems use temporary storage before the first save, then stay inside the project folder. Missing audio files remain visible and play as silence.

Planned work includes audio and beat editors, beat tracks, more effects, effects for individual beat lanes, and custom instrument routing. See the [features and roadmap](docs/roadmap.md) for details.

## Build and run

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](Cargo.toml) and [Cargo.lock](Cargo.lock). Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band, LAME, or ONNX Runtime installation. The first build downloads the pinned ONNX Runtime binaries; it does not download model weights.

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

The script builds the desktop with the lockfile, includes the same font, icon, UI, media, oscillator, and EQ notices as the macOS bundle, and replaces an existing ZIP only after packaging succeeds. It includes no CLI, projects, recordings, build cache, or debug symbols. The ZIP is unsigned. It does not include the Microsoft C++ runtime: destination PCs need the [Microsoft Visual C++ v14 Redistributable for x64](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) and an OpenGL-capable graphics driver. The selected native inference archive also imports system DirectML (`DMLCreateDevice1`) and Direct3D 12 even though separation uses CPU. Those Windows components are not included in the ZIP; older Windows installations must provide the required APIs. See the [native dependency audit](specs/audio/stem-native-dependencies.md) for the verified imports and deployment limits. Rust, LLVM, Rubber Band, and LAME installations are not needed to run it.

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

Stem separation requires Apple Silicon on macOS. Intel builds omit that backend and disable its controls; the other app features remain available.

### Linux

Install Rust through Rustup, a C++ compiler, Clang/libclang, ALSA development libraries, pkg-config, and OpenSSL development headers (for build-time runtime downloads). Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

Create a portable ZIP with `sh tools/package_linux.sh release`. Use `debug` for a debug build. The archive is `target/<profile>/DAW-linux-x64.zip` on x64 or `DAW-linux-arm64.zip` on ARM64. Extract it and run `DAW/daw-desktop`. Keep `DAW/LICENSE.txt` and `DAW/licenses` with the executable. The archive preserves executable permissions and includes the same dependency notices as the other packages. It does not bundle system libraries: the destination needs compatible C/C++ runtime, ALSA, graphics libraries, and a desktop portal for dialogs.

### GitHub Actions builds

The [Test and build workflow](.github/workflows/build-and-test.yml) runs on pushes, pull requests, and manual dispatches. It calls the [platform workflow](.github/workflows/platform.yml) separately for Windows, macOS, and Linux. Each platform has separate `test` and `build` jobs. Tests run formatting, strict workspace/all-target Clippy, workspace tests, and the headless CLI smoke checks with the pinned Rust toolchain and locked dependencies. Build starts after that platform's tests pass, builds and packages release binaries on a fresh worker, and uploads artifacts retained for 14 days. A failure on one platform does not block or cancel another platform's build. Both jobs use the same [native build setup action](.github/actions/setup-build/action.yml).

| Platform | Runner | Download |
| --- | --- | --- |
| Windows x64 | `windows-2025` | `DAW-windows-x64`: portable `daw-desktop.exe`, project license, and dependency notices |
| macOS Apple Silicon | `macos-15` | `DAW-macos-arm64.dmg` |
| Linux x64 | `ubuntu-24.04` | `DAW-linux-x64.zip` |

Download artifacts from the workflow run's summary. Windows downloads as a ZIP containing the executable and licenses; it is not an installer. The disk image and Linux ZIP upload directly as files. These builds do not sign binaries, notarize the macOS app, or publish a GitHub release. The Linux package targets Ubuntu 24.04 and compatible systems; it is not a static binary for every distribution. Native UI and audio-device checks remain separate from CI.

Workers also upload command logs as `logs-test-<OS>-<architecture>` and `logs-build-<OS>-<architecture>`, including failed jobs, with 14-day retention. Job summaries show step results, a download link, and the last command output on failure. These artifacts contain formatting, Clippy, test, CLI, or packaging output; checkout and external setup-action details remain in the raw runner logs. If the job page shows no steps, retrieve those logs with `gh run view <run-id> --log` from the repository ([GitHub log documentation](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs)). A skipped build job has no worker or build logs.

## Documentation

- [User guide](docs/README.md): working with clips, instruments, modulation, and effects.
- [Features and roadmap](docs/roadmap.md), including current limits and planned capabilities.
- [Time stretching and offline pitch changes](docs/audio/time-stretch.md) and the [standalone comparison tool guide](crates/timestretch-cli/README.md).
- [Implementation specifications](specs/README.md): architecture, data formats, requirements, and acceptance criteria.
- [Validation records](specs/validation/README.md): completed checks and remaining verification limits.
- [Repository instructions](AGENTS.md): contribution rules and development workflows.

## Licenses

Original code uses the [BSD 2-Clause License](LICENSE). Bundled dependencies have separate terms. Their notices are included with the [media libraries](crates/media/assets/licenses), [fonts](crates/ui/assets/fonts), [icons](crates/ui/assets/icons), [UI dependencies](crates/ui/assets/licenses), and [oscillator dependencies](crates/oscillator/assets/licenses), and [EQ dependencies](crates/equalizer/assets/licenses), and [stem-separation dependencies](crates/separation/assets/licenses).

The [native dependency audit](specs/audio/stem-native-dependencies.md) records platform runtime requirements and license obligations for the combined applications. The original-code BSD license does not replace bundled-library terms or corresponding-source requirements.
