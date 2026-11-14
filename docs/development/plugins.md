# Plugins

[Development guides](README.md)

Run commands from the repository root.

The registered effect products have separate CLAP and VST3 development builds. Multiband Exciter is built-in only; `--units all` packages every available plugin product. They reuse the portable DAW processors, shared controls, and meters. See the [plugin list and controls](../../docs/effects/plugins.md).

Use the [native platform prerequisites](README.md) and Python 3.11 or later. Use a unit slug from the [plugin list](../../docs/effects/plugins.md) for `<unit>`, and select `<target>` below:

| Platform | Target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |

```sh
# Build and package both APIs in debug mode.
python3 tools/package_plugins.py --units <unit> --target <target>

# Build and package selected APIs in release mode.
python3 tools/package_plugins.py --release --units <unit> --api clap,vst3 --target <target>

# Build only VST3 for every registered product.
python3 tools/package_plugins.py --units all --api vst3 --target <target>
```

`--units` accepts one slug, a comma-separated list such as `saturator,bitcrusher`,
or `all`. `--api` accepts `clap`, `vst3`, a comma-separated list, or `all`; it
selects both formats when omitted. The script builds the selected packages from
source before packaging them. A failed build leaves existing archives unchanged.
Cargo reuses its cached outputs when the sources and build settings match.

Omit `--release` for debug, or add it for an optimized release build. Debug bundle,
binary, ZIP and checksum names append `-debug` before the extension. For example,
`Drive-debug.clap` is inside `Drive-0.1.0-macos-arm64-clap-debug.zip`. Release names
retain their existing form. Both modes retain the same public plugin identities.

ZIPs, checksums, licenses and installation instructions go to
`target/plugin-artifacts`. Use `--output-dir` to change that location and
`--target-dir` to change Cargo's build directory. Relative paths use the calling
directory. Debug and release artifacts can share an output directory. A run that
selects only one API writes an API-specific checksum file, preserving checksum
files for the other API or both APIs.

The [plugin CI workflow](../../.github/workflows/plugins.yml) builds and tests each product on all three platforms. Download `<product>-CLAP-<target>`, `<product>-VST3-<target>`, and `<product>-checksums-<target>` artifacts. Extract the inner ZIP before installation. Artifacts and logs are retained for 14 days.

These are development previews. macOS bundles are ad-hoc signed, without notarization; Windows/Linux binaries are unsigned. External-host and release acceptance checks remain open. See [developer checks](../../specs/architecture/plugin-development-tools.md) and [validation records](../../specs/validation/README.md).
