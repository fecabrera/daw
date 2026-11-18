# Development guides

Build, run, and package the desktop app, CLI, and effect plugins. Choose your platform first.

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). The bundled Opus decoder requires CMake and a C compiler. Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band, LAME, Opus, or ONNX Runtime installation. The first build downloads the pinned ONNX Runtime binaries; it does not download model weights.

- [Windows](windows.md): MSVC and libclang setup, builds, ZIP packaging, and troubleshooting.
- [macOS](macos.md): builds, app bundles, and disk images.
- [Linux](linux.md): prerequisites, builds, and portable ZIP packaging.
- [Effect plugins](plugins.md): CLAP/VST3/AUv2 builds and packaging.
- [GitHub Actions builds](ci.md): platform artifacts, downloads, and logs.

For CLI commands, see the [CLI guide](../cli/README.md). For contribution rules and validation commands, see [repository instructions](../../AGENTS.md#build-and-validation-workflow). Architecture and implementation contracts remain in [specifications](../../specs/README.md).
The desktop MIDI adapter uses midir with native CoreMIDI, WinMM and ALSA backends. ALSA development libraries are required on Linux. MIDI device dependencies stay outside the CLI and portable engine. Packaged dependency notices are in `crates/ui/assets/licenses`. Physical keyboard, device-disconnect and latency checks must run on each platform.

## Headless tools

With the platform prerequisites installed, build from the repository root:

```sh
cargo build --locked -p daw-cli
cargo build --locked -p timestretch-cli --release
```

The executables are `target/debug/daw-cli` and `target/release/timestretch`
(`.exe` on Windows). Cargo target and target-directory settings can change these
paths. Add their directories to your command search path or use their full paths.
The [CLI guide](../cli/README.md) and [time-stretch guide](../cli/timestretch.md)
explain operation. The [standalone tool reference](../../crates/timestretch-cli/README.md)
covers reference-enabled and Integrated-only build options.
Keep the [media dependency notices](../../crates/media/assets/licenses), including
RealFFT, with separately distributed tools that use them. Retain the source and
relinking materials required by the enabled backends.

### Plugin catalog scanner

The desktop and CLI include isolated scanning. Build the optional standalone
scanner to run the native scanner checks:

```sh
cargo build --locked -p daw-cli -p daw-desktop -p daw-plugin-scan
cargo build --locked -p daw-drive-clap -p daw-drive-vst3
python3 tools/check_plugin_scanner.py
```

No scanner sidecar is required for normal desktop or CLI use. An explicit
`daw-cli plugins --scanner PATH` selects an optional standalone helper.
The native scanner check uses only the repository's Drive exports in
temporary bundles; it does not scan the user's installed plugins. For a different
target directory, pass the script's scanner and library options.

The desktop searches default locations at startup and launches itself in scanner
mode for each candidate. All desktop packagers include CLAP/VST3/libloading
notices. Keep those notices with
separately distributed CLI/scanner builds. See the [hosting specification](../../specs/architecture/external-plugin-hosting.md).

Run owned native processing fixtures without loading user plugins:

```sh
cargo build --locked -p daw-plugin-host-fixtures
cargo build --locked -p daw-engine --example native_fixture_check --example native_graph_check
cargo build --locked -p daw-plugin-host --example native_host_check --example native_preset_check
python3 tools/check_plugin_hosting.py
python3 tools/check_plugin_scanner.py --host-check target/debug/examples/native_host_check --graph-check target/debug/examples/native_graph_check
```

Use `.exe` paths on Windows. On macOS, build
`cargo build -p daw-desktop --example native_editor_check`, then pass
`--editor-check target/debug/examples/native_editor_check` to the scanner check.
This check requires actual desktop/GPU access. These tests cover owned fixtures;
they do not establish general vendor compatibility or device deadlines.

## Audio cache and preparation benchmark

Desktop and CLI startup enable the shared prepared-audio cache. Default directories are `~/Library/Caches/daw/prepared-audio-v1` on macOS, `%LOCALAPPDATA%/daw/prepared-audio-v1` on Windows, and `$XDG_CACHE_HOME/daw/prepared-audio-v1` on Linux (falling back to `~/.cache`). Set `DAW_AUDIO_CACHE` to override the directory. An empty value disables disk caching while keeping bounded memory reuse. Cache write failures fall back to normal preparation.

Run the generated-source release benchmark without native playback, builds, or other tests running:

```sh
python3 tools/benchmark_audio_preparation.py --runs 3 --seconds 58
```

It builds the release example, generates temporary audio under ignored `work/audio-preparation`, and compares uncached, empty-cache, and new-process saved-cache runs. Results record the source hash, toolchain, timings, and cache hit/build counts. Use `--skip-build` only after rebuilding the example for the current source. This measures headless media preparation, not native presentation or device latency. See the [implementation plan and contracts](../../specs/architecture/background-audio-preparation.md).

The macOS and Linux desktop packagers explicitly build into the repository's `target/` directory. They explicitly select the native Rust host target, overriding `CARGO_BUILD_TARGET` and Cargo `build.target`, as well as `CARGO_TARGET_DIR` and `build.target-dir`. They read and validate the executable path reported by Cargo. The executable is built under `target/<host>/<profile>/`; package output paths remain unchanged.

## Dependency checks in CI

The Test and build workflow runs Cargo Deny 0.21.0. Advisory failures fail the
Cargo Deny job. It checks macOS ARM64, macOS x86-64, Windows x86-64, and Linux
x86-64 in separate commands so dependencies for different targets do not mix.
The workspace license report uses [deny.toml](../../deny.toml) and remains
visible in the job log and the `logs-cargo-deny` artifact. License findings
do not fail the job. This includes the current Rubber Band and LAME findings.

To reproduce the Linux advisory check and the workspace license report locally,
run these commands with Cargo Deny installed:

```sh
cargo deny --locked --workspace --target x86_64-unknown-linux-gnu check advisories
cargo deny --locked --workspace check licenses
```

## Corresponding source in desktop packages

Distribution packaging requires Python 3, Git, Cargo and a clean committed tracked tree.
For a development package from uncommitted files, add `--local` to the platform
packager. Local mode includes icons and license notices but omits the
corresponding-source materials described below. See the [macOS](macos.md),
[Windows](windows.md#package-a-zip), and [Linux](linux.md) guides for commands.
Untracked files, recordings, build outputs and caches are excluded. The packager
builds that revision and includes `DAW-source.zip`, `SOURCE-ACCESS.txt` and
`source-manifest.json` beside the executable on Windows and Linux. On macOS,
`DAW-source.zip` and a source-access note stay inside `DAW.app/Contents/Resources`;
the DMG also contains `DAW-source-metadata` beside the app, with the manifest and
full receipt for the final signed executable. Keep that companion directory when
redistributing the package. The manifest records the exact commit, profile and
source/binary hashes.
CI uploads these materials within the same desktop download.

The source archive contains the committed application, local vendor patches,
Cargo.lock and versioned registry/Git dependencies from `cargo vendor --locked`.
Extract it and read `DAW/SOURCE-BUILD.txt`. Follow the platform prerequisites, then
run `cargo build --locked -p daw-desktop --release` for a release build, or omit
`--release` for debug. Rebuild with modified LGPL components to relink. Native ONNX
inputs still use the pinned download adapter; the included native source inventory
provides immutable source locations, revisions and hashes. Model weights are not
needed or included. Keep the source materials with packages you redistribute.

This supplies build/relink materials; it does not claim bit-identical builds across
different compilers or operating systems. Native packaging and clean-machine checks
remain separate from source-archive fixture tests.

Supplemental upstream notices omitted from published dependency archives are
included in `DAW/SOURCE-LICENSES`, with a package/version/source/checksum index.
Keep that directory with the sources when redistributing them. The packager
rejects a mismatched package revision or notice hash.
