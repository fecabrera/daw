# Plugins

[Development guides](README.md)

Run commands from the repository root.

The registered effect products have separate CLAP/VST3 and macOS AUv2 development builds. Multiband Exciter is built-in only; `--units all` packages every available plugin product. They reuse the portable DAW processors, shared controls, and meters. See the [plugin list and controls](../../docs/effects/plugins.md).

## Build and package locally

Use the [native platform prerequisites](README.md) and Python 3.11 or later. Use a unit slug from the [plugin list](../../docs/effects/plugins.md) for `<unit>`, and select `<target>` below:

| Platform | Target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |

```sh
# Build and package all platform-supported APIs in debug mode.
python3 tools/package_plugins.py --units <unit> --target <target>

# Build and package selected APIs in release mode.
python3 tools/package_plugins.py --release --units <unit> --api clap,vst3 --target <target>

# Build only VST3 for every registered product.
python3 tools/package_plugins.py --units all --api vst3 --target <target>
```

`--units` accepts one slug, a comma-separated list such as `saturator,bitcrusher`,
or `all`. `--api` accepts `clap`, `vst3`, `auv2`, a comma-separated list, or `all`. Omission and
`all` select CLAP/VST3/AUv2 on macOS and CLAP/VST3 on Windows/Linux. Explicit
`auv2` on Windows/Linux fails before building. The script builds the selected packages from
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
selects an API subset writes an API-specific checksum file, preserving checksum
files for the other API or both APIs.

## CI builds

The [plugin CI workflow](../../.github/workflows/plugins.yml) builds and tests each product on all three platforms. Download `<product>-CLAP-<target>`, `<product>-VST3-<target>`, `<product>-AUv2-<target>` on macOS, and `<product>-checksums-<target>` artifacts. Extract the inner ZIP before installation. Artifacts and logs are retained for 14 days.

These are development previews. macOS bundles are ad-hoc signed, without notarization; Windows/Linux binaries are unsigned. External-host and release acceptance checks remain open. See [developer checks](../../specs/architecture/plugin-development-tools.md) and [validation records](../../specs/validation/README.md).

## AUv2 source builds

Use the pinned Rust toolchain and the approved Apple SDK/libclang/CLTools
installation recorded in [the generation contract](../../specs/architecture/auv2-plugin-builds.md#local-binding-generation).
The initial installation is CLTools 27.2 with SDK 27.2 and Apple clang 21.
No external AU binding crate is needed. AU exports and binding generation support
arm64 macOS only. Intel macOS workspace checks compile inert AU crates; these do
not produce usable AU components.
macOS CLAP/VST3-only builds and Windows/Linux builds do not need this SDK.

```sh
python3 tools/package_plugins.py --units all --api auv2 --target aarch64-apple-darwin
python3 tools/package_plugins.py --release --units all --api auv2 --target aarch64-apple-darwin
```

The output contains `.component` bundles in ZIPs. Debug uses names such as
`Drive-debug.component`. Both profiles use the same public identity, so install
one at a time. Package staging/signing/publication failures preserve existing
archives. This rollback does not guarantee recovery after a power loss.

To verify another SDK, run `DAW_AUV2_VERIFY_SDK=1 cargo test -p daw-plugin-auv2`.
This produces a candidate inventory in Cargo's build output. It does not approve
the candidate or permit packaging. Review it against the architecture and
[acceptance gates](../../specs/acceptance/auv2-plugin-builds.md) before updating
`approved.json`. Never commit generated Apple declarations or SDK files.

Native API tests register components only in each test process and do not install
them. For strict Apple validation, install the selected profile in a disposable
test environment, then run `python3 tools/check_auv2.py --units all`. The tool
does not install components. Native host/editor checks remain separate.

Check already packaged AU bundles without installing them with
`python3 tools/check_auv2_exports.py target/plugin-artifacts/*-auv2.zip`. This
headless native host checks signatures, rendering, parameters, state and Cocoa
factory metadata. It does not open custom editors or replace strict auval checks.
