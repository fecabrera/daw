# Development guides

Build, run, and package the desktop app, CLI, and effect plugins. Choose your platform first.

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). The bundled Opus decoder requires CMake and a C compiler. Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band, LAME, Opus, or ONNX Runtime installation. The first build downloads the pinned ONNX Runtime binaries; it does not download model weights.

- [Windows](windows.md): MSVC and libclang setup, builds, ZIP packaging, and troubleshooting.
- [macOS](macos.md): builds, app bundles, and disk images.
- [Linux](linux.md): prerequisites, builds, and portable ZIP packaging.
- [Effect plugins](plugins.md): CLAP/VST3 builds and packaging.
- [GitHub Actions builds](ci.md): platform artifacts, downloads, and logs.

For CLI commands, see the [CLI guide](../cli/README.md). For contribution rules and validation commands, see [repository instructions](../../AGENTS.md#build-and-validation-workflow). Architecture and implementation contracts remain in [specifications](../../specs/README.md).
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

## Audio cache and preparation benchmark

Desktop and CLI startup enable the shared prepared-audio cache. Default directories are `~/Library/Caches/daw/prepared-audio-v1` on macOS, `%LOCALAPPDATA%/daw/prepared-audio-v1` on Windows, and `$XDG_CACHE_HOME/daw/prepared-audio-v1` on Linux (falling back to `~/.cache`). Set `DAW_AUDIO_CACHE` to override the directory. An empty value disables disk caching while keeping bounded memory reuse. Cache write failures fall back to normal preparation.

Run the generated-source release benchmark without native playback, builds, or other tests running:

```sh
python3 tools/benchmark_audio_preparation.py --runs 3 --seconds 58
```

It builds the release example, generates temporary audio under ignored `work/audio-preparation`, and compares uncached, empty-cache, and new-process saved-cache runs. Results record the source hash, toolchain, timings, and cache hit/build counts. Use `--skip-build` only after rebuilding the example for the current source. This measures headless media preparation, not native presentation or device latency. See the [implementation plan and contracts](../../specs/architecture/background-audio-preparation.md).

The macOS and Linux desktop packagers explicitly build into the repository's `target/` directory. They explicitly select the native Rust host target, overriding `CARGO_BUILD_TARGET` and Cargo `build.target`, as well as `CARGO_TARGET_DIR` and `build.target-dir`. They read and validate the executable path reported by Cargo. The executable is built under `target/<host>/<profile>/`; package output paths remain unchanged.

## Corresponding source in desktop packages

Desktop packaging requires Python 3, Git, Cargo and a clean committed tracked tree.
Untracked files, recordings, build outputs and caches are excluded. The packager
builds that revision and includes `DAW-source.zip`, `SOURCE-ACCESS.txt` and
`source-manifest.json` beside the executable (inside `DAW.app/Contents/Resources`
on macOS). The manifest records the exact commit, profile and source/binary hashes.
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
