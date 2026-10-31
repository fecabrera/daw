# Development guides

Build, run, and package the desktop app, CLI, and effect plugins. Choose your platform first.

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band, LAME, or ONNX Runtime installation. The first build downloads the pinned ONNX Runtime binaries; it does not download model weights.

- [Windows](windows.md): MSVC and libclang setup, builds, ZIP packaging, and troubleshooting.
- [macOS](macos.md): builds, app bundles, and disk images.
- [Linux](linux.md): prerequisites, builds, and portable ZIP packaging.
- [Effect plugins](plugins.md): CLAP/VST3 builds and packaging.
- [GitHub Actions builds](ci.md): platform artifacts, downloads, and logs.

For CLI commands, see the [CLI guide](../cli/README.md). For contribution rules and validation commands, see [repository instructions](../../AGENTS.md#build-and-validation-workflow). Architecture and implementation contracts remain in [specifications](../../specs/README.md).
## Audio cache and preparation benchmark

Desktop and CLI startup enable the shared prepared-audio cache. Default directories are `~/Library/Caches/daw/prepared-audio-v1` on macOS, `%LOCALAPPDATA%/daw/prepared-audio-v1` on Windows, and `$XDG_CACHE_HOME/daw/prepared-audio-v1` on Linux (falling back to `~/.cache`). Set `DAW_AUDIO_CACHE` to override the directory. An empty value disables disk caching while keeping bounded memory reuse. Cache write failures fall back to normal preparation.

Run the generated-source release benchmark without native playback, builds, or other tests running:

```sh
python3 tools/benchmark_audio_preparation.py --runs 3 --seconds 58
```

It builds the release example, generates temporary audio under ignored `work/audio-preparation`, and compares uncached, empty-cache, and new-process saved-cache runs. Results record the source hash, toolchain, timings, and cache hit/build counts. Use `--skip-build` only after rebuilding the example for the current source. This measures headless media preparation, not native presentation or device latency. See the [implementation plan and contracts](../../specs/architecture/background-audio-preparation.md).
