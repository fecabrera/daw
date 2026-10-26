# Saturator and Bitcrusher plugins

Saturator and Bitcrusher are available as separate CLAP and VST3 development previews. Each uses the same processor and controls as its built-in DAW effect. They have separate plugin identities and presets.

Download the product and platform artifact from the [plugin build workflow](../../.github/workflows/plugins.yml). Extract the inner ZIP, keep the included licenses, copy the plugin file or bundle to a folder your host scans, and rescan. Build and package instructions are in the [main README](../../README.md#plugins). Close the host before replacing or removing an installed copy.

| Product | Left strip | Output strip |
| --- | --- | --- |
| [Saturator](saturator.md) | Shape, Clip Mode, Drive | Level, Dry/Wet |
| [Bitcrusher](bitcrusher.md) | Drive, Resolution, Downsampling | Level, Dry/Wet |

The host provides bypass. The editor omits the DAW header and modulation connections. Knob drag, fine adjustment, keyboard changes, and reset use the same controls as the built-in effect. Saturator retains its Shape and Clip Mode lists. Bitcrusher retains whole-bit Resolution and whole-ratio Downsampling.

Both Output strips include the [shared level meter and held sample peak](output-meter.md). Click the rail to reset its peak without changing parameters. Right-click the editor background to select Dark, Light, or Mist. Deep Orange is the default accent. Appearance is local to the editor instance and is not saved in a preset.

Saturator reports its 32-frame processing delay to the host, including bypass. Bitcrusher reports zero delay. Use the host's delay compensation for aligned mixing.

The previews accept stereo 32-bit floating-point audio at 44.1, 48, 88.2, 96, 176.4, and 192 kHz. Other rates, mono buses, 64-bit processing, MIDI input, and DAW modulation routes are not supported. Logic Pro requires Audio Units and cannot load these formats.

macOS bundles are ad-hoc signed, without notarization. Windows/Linux previews have no distribution signature. Native editor attachment, external host compatibility, accessibility, and clean-machine behavior still require platform checks. VST3 meter delivery requires the host to connect the processor and controller. These previews are not a release acceptance claim.
