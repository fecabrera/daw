# Development guides

Build, run, and package the desktop app, CLI, and effect plugins. Choose your platform first.

Run commands from the repository root with the toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml). Dependency and minimum Rust versions are defined in [Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). Rubber Band is built from bundled source and requires a C++ compiler and Clang/libclang for its Rust bindings. The packaged app needs no separate Rubber Band, LAME, or ONNX Runtime installation. The first build downloads the pinned ONNX Runtime binaries; it does not download model weights.

- [Windows](windows.md): MSVC and libclang setup, builds, ZIP packaging, and troubleshooting.
- [macOS](macos.md): builds, app bundles, and disk images.
- [Linux](linux.md): prerequisites, builds, and portable ZIP packaging.
- [Effect plugins](plugins.md): CLAP/VST3 builds and packaging.
- [GitHub Actions builds](ci.md): platform artifacts, downloads, and logs.

For CLI commands, see the [CLI guide](../cli/README.md). For contribution rules and validation commands, see [repository instructions](../../AGENTS.md#build-and-validation-workflow). Architecture and implementation contracts remain in [specifications](../../specs/README.md).
