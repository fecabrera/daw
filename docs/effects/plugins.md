# Effect plugins

Every built-in effect has a separate CLAP and VST3 development preview. Each uses its portable DAW processor. Host bypass, automation, and presets work independently for each instance. The native editors reuse shared knobs, colors, level meters, and held peak controls.

| Product | Unit slug | Controls |
| --- | --- | --- |
| [Drive](drive-plugin.md) | `drive` | Gain, Color, Tone, Level, Dry/Wet |
| [Drive v2](drive-v2-plugin.md) | `drive-v2` | Drive, Shape, Bias, Tone, Level, Dry/Wet |
| [Saturator](distortion-plugins.md) | `saturator` | Drive, Shape, Clip Mode, Level, Dry/Wet |
| [Bitcrusher](distortion-plugins.md) | `bitcrusher` | Drive, Resolution, Downsampling, Level, Dry/Wet |
| 3-band EQ | `equalizer` | Low/Mid/High gain and power; Low/High frequency and Q |
| Multiband EQ | `multiband-eq` | 64 band slots with power, mode, frequency, gain, Q; output Level |
| Filter | `filter` | Shape, Frequency, Q, Level, Dry/Wet |
| Auto-filter | `auto-filter` | Filter controls, LFO power/wave, Hz/Note rate, Strength, Sync/Stereo Phase |
| Compressor | `compressor` | Threshold, Ratio, Detection, Attack, Release, Knee, Level, Dry/Wet |
| OTT | `ott` | Crossovers; per-band power, Depth, Input, Output, Attack, Release; Detection, Level, Dry/Wet |
| Limiter | `limiter` | Input, Release, Lookahead, Ceiling, Peak/True Peak, L/R or M/S |
| Direction Mixer | `direction-mixer` | L/R or M/S input, Direction, Spread |
| Stereo Spread | `stereo-spread` | Lower/Upper Intensity and Frequency, Order |
| Chorus | `chorus` | Delay, Depth, Feedback, Voices, Hz/Note rate, Stereo Phase, Width, Level, Dry/Wet |

[Build, package, and download instructions](../../README.md#plugins) apply to all products. The packager accepts one slug, a comma-separated list, or `--units all`. Build both formats for all selected units first. Extract the downloaded ZIP and keep the included licenses beside the plugin. Close your host before replacing or removing it.

All editors have a [shared output level rail and held sample peak](output-meter.md). Limiter shows reduction on its left strip and output level on its right strip. Compressor also shows reduction, and OTT shows signed band gain changes. Click a meter to reset its held peak. Reset does not change audio or saved parameters. Right-click the background to choose Dark, Light, or Mist; Deep Orange is the default accent.

Auto-filter and Chorus use the host's tempo and sample position. Their clock starts locally at 120 BPM. Stopped transport and missing updates keep the last valid tempo and advance a local position; playback resumes phase alignment with the host. Negative host positions start at frame zero. Hz mode keeps its frequency; Note mode follows tempo. The editors show the active rate control. Bar labels use four beats per bar. CLAP tempo changes follow block snapshots and transport events; continuous tempo ramps within a block are not applied.

Multiband EQ prepares 64 fixed slots to retain stable automation IDs. Band 1 starts enabled. Use each band's power control to enable another slot, and scroll horizontally to reach later slots and Output. A disabled slot keeps its settings and fades out before processing suspends. This plugin bank does not change the DAW's variable band count. Limiter reports rate-dependent processing latency, and Saturator reports 32 frames. Other plugins report zero latency. Leave host delay compensation enabled.

These previews accept stereo planar 32-bit floating-point audio at 44.1, 48, 88.2, 96, 176.4, and 192 kHz. They do not include MIDI, sidechain inputs, DAW modulation routes, or DAW project data. Logic Pro requires Audio Units and cannot load these formats. VST3 meter delivery requires a processor/controller connection.

macOS bundles use ad-hoc signing without notarization. Windows/Linux builds are unsigned. Native editor attachment, external hosts, accessibility, and clean-machine release acceptance still need platform checks.
