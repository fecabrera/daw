# Drive v2 plugin development builds

Drive v2 has separate CLAP and VST3 development plugins. Both use the built-in [Drive v2](drive-v2.md) DSP and the same three-strip controls. They have separate identities from original Drive, so both products can be installed together. Neither format loads original Drive plugin state.

## Build and package

Use the native platform prerequisites in the [README](../../README.md). On macOS Apple Silicon:

```sh
cargo build --release --locked --target aarch64-apple-darwin \
  -p daw-drive-v2-clap -p daw-drive-v2-vst3 --lib
python3 tools/package_plugins.py --units drive-v2 --target aarch64-apple-darwin
```

The generic packager requires Python 3.11 or later and both libraries. Use `--units drive,drive-v2` to package both products or `--units all` for every unit manifest; build every selected unit first. On Windows x64 use `x86_64-pc-windows-msvc`; on Linux x64 use `x86_64-unknown-linux-gnu`. It writes separate `Drive-v2-…-clap.zip` and `Drive-v2-…-vst3.zip` archives and SHA-256 checksums into `target/plugin-artifacts`. macOS bundles are signed ad hoc after assembly. These are development previews, without notarization or distribution signatures.

The Build plugins workflow builds both Drive products and both formats independently on macOS arm64, Windows x64, and Linux x64. Artifacts use `Drive-v2-CLAP-<target>` and `Drive-v2-VST3-<target>` names. CI configuration does not establish native host acceptance.

## Install locally

Extract the ZIP. On macOS copy the complete `Drive v2.vst3` bundle into `~/Library/Audio/Plug-Ins/VST3/`, or `Drive v2.clap` into `~/Library/Audio/Plug-Ins/CLAP/`. Use a native Apple Silicon host that supports the selected format. Restart the host or rescan after replacement. Logic Pro requires Audio Units and cannot load these formats. On Windows/Linux, use a plugin search folder supported by the host. Keep the included licenses with the plugin. The build tools do not install plugins.

## Controls and state

The fixed editor is 270 × 218 logical points. Drive contains Soft/Hard, Gain and Tone; Color contains Shape, Strength and Frequency with its own footer power button; Output contains Input Gain, Output Level and Dry/Wet. Color power disables its three controls without deleting their values. Automation can still update them while Color is off.

The host exposes all ten controls plus whole-unit Bypass. Input Gain and Output Level support exact silence through +12 dB. Audio is stereo 32-bit at 44.1/48/88.2/96/176.4/192 kHz. Latency is zero, with a one-second residual tail declaration. Unsupported rates and formats are rejected. Processing, smoothing, and bypass match built-in Drive v2.

Host state saves the 11 base parameters. DAW modulation routes, project identities, paths, and exact DSP histories are excluded. The default plugin accent is Deep Orange (`#FF5722`). Dark/Light/Mist appearance survives editor reopen for an instance but is not serialized. Host automation and Undo remain host-owned.

## Verification and limits

See the [implementation and local checks](../../specs/architecture/drive-v2-plugin-builds.md). Native editor attachment, SDK validators, external DAWs, accessibility, Windows/Linux execution, and clean-machine distribution require separate checks. Original Drive's historical validation results do not validate Drive v2.

The editor includes the [standard output rail and held-peak indicator](output-meter.md). Clicking the rail resets monitoring without a host automation gesture. VST3 meter delivery uses the host's processor/controller connection; CLAP reads the instance's prepared atomic meter.
