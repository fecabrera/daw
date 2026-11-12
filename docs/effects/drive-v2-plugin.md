# Drive v2 plugins

Drive v2 has separate CLAP and VST3 development plugins. Both use the built-in [Drive v2](drive-v2.md) DSP and the same three-strip controls. They have separate identities from original Drive, so both products can be installed together. Neither format loads original Drive plugin state.

For build and package instructions, use the [plugin development guide](../development/plugins.md).
These are development previews without notarization or distribution signatures.
Native host acceptance requires separate checks.

## Install locally

Extract the ZIP. On macOS copy the complete `Drive v2.vst3` bundle into `~/Library/Audio/Plug-Ins/VST3/`, or `Drive v2.clap` into `~/Library/Audio/Plug-Ins/CLAP/`. Use a native Apple Silicon host that supports the selected format. Restart the host or rescan after replacement. Logic Pro requires Audio Units and cannot load these formats. On Windows/Linux, use a plugin search folder supported by the host. Keep the included licenses with the plugin. The build tools do not install plugins.

## Controls and state

The editor has a fixed size. Drive contains the Soft/Hard/Tube dropdown, Gain and Tone; Color contains Shape, Strength and Frequency with its own footer power button; Output contains Input Gain, Output Level and Dry/Wet. Color power disables its three controls without deleting their values. Automation can still update them while Color is off.

The host exposes all ten controls plus whole-unit Bypass. Input Gain and Output Level support exact silence through +12 dB. Audio is stereo 32-bit at 44.1/48/88.2/96/176.4/192 kHz. Latency is zero, with a one-second residual tail declaration. Unsupported rates and formats are rejected. Processing, smoothing, and bypass match built-in Drive v2.

Host state saves the 11 base parameters. DAW modulation routes, project identities, paths, and exact DSP histories are excluded. The default plugin accent is Deep Orange. Dark/Light/Mist appearance survives editor reopen for an instance but is not serialized. Host automation and Undo remain host-owned.

## Verification and limits

See the [implementation and local checks](../../specs/architecture/drive-v2-plugin-builds.md). Native editor attachment, SDK validators, external DAWs, accessibility, Windows/Linux execution, and clean-machine distribution require separate checks. Original Drive's historical validation results do not validate Drive v2.

The editor includes the [standard output rail and held-peak indicator](output-meter.md). Clicking the rail resets monitoring without a host automation gesture. VST3 meter delivery uses the host's processor/controller connection; CLAP reads the instance's prepared atomic meter.
