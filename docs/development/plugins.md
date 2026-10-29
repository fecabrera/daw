# Plugins

[Development guides](README.md)

Run commands from the repository root.

All fourteen effects have separate CLAP and VST3 development builds. They reuse the portable DAW processors, shared controls, and meters. See the [plugin list and controls](../../docs/effects/plugins.md).

Use the [native platform prerequisites](README.md) and Python 3.11 or later. Use a unit slug from the [plugin list](../../docs/effects/plugins.md) for `<unit>`, and select `<target>` below:

| Platform | Target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |

```sh
cargo build --release --locked --target <target> -p daw-<unit>-clap -p daw-<unit>-vst3 --lib
python3 tools/package_plugins.py --units <unit> --target <target>
```

The packager also accepts `--units saturator,bitcrusher` or `--units all`. Build both formats for every selected unit first. ZIPs, checksums, licenses, and installation instructions go to `target/plugin-artifacts`.

The [plugin CI workflow](../../.github/workflows/plugins.yml) builds and tests each product on all three platforms. Download `<product>-CLAP-<target>`, `<product>-VST3-<target>`, and `<product>-checksums-<target>` artifacts. Extract the inner ZIP before installation. Artifacts and logs are retained for 14 days.

These are development previews. macOS bundles are ad-hoc signed, without notarization; Windows/Linux binaries are unsigned. External-host and release acceptance checks remain open. See [developer checks](../../specs/architecture/plugin-development-tools.md) and [validation records](../../specs/validation/README.md).
