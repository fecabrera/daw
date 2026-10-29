# Effect plugins

Every built-in effect has a separate CLAP and VST3 development preview. Each uses its portable DAW processor. Host bypass, automation, and presets work independently for each instance. Plugin editors use the same control bodies as the built-in effects, including strip layout, labels, footers and meters.

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

[Build, package, and download instructions](../development/plugins.md) apply to all products. The packager accepts one slug, a comma-separated list, or `--units all`. Build both formats for all selected units first. Extract the downloaded ZIP and keep the included licenses beside the plugin. Close your host before replacing or removing it.

Meters follow each built-in effect. Effects with an output rail use the [shared level meter and held sample peak](output-meter.md). Limiter shows reduction on its left strip and output level on its right strip. Compressor also shows reduction, and OTT shows signed band gain changes. Click a held reading to reset it. Reset does not change audio or saved parameters. Right-click the background to choose Dark, Light, or Mist; Deep Orange is the default accent.

Auto-filter and Chorus use the host's tempo and sample position. Their clock starts locally at 120 BPM. Stopped transport and missing updates keep the last valid tempo and advance a local position; playback resumes phase alignment with the host. Negative host positions start at frame zero. Hz mode keeps its frequency; Note mode follows tempo. The editors show the active rate control. Bar labels use four beats per bar. CLAP tempo changes follow block snapshots and transport events; continuous tempo ramps within a block are not applied.

Multiband EQ starts with one band. Click Plus to add bands, up to 64. Use the band power control to bypass it; right-click its footer to remove it. Scroll horizontally when needed. Automation IDs stay fixed. Presets restore enabled or changed bands; a bypassed band keeps its settings. Limiter reports rate-dependent processing latency, and Saturator reports 32 frames. Other plugins report zero latency. Leave host delay compensation enabled.

These previews accept stereo planar 32-bit floating-point audio at 44.1, 48, 88.2, 96, 176.4, and 192 kHz. They do not include MIDI, sidechain inputs, DAW modulation routes, or DAW project data. Logic Pro requires Audio Units and cannot load these formats. VST3 meter delivery requires a processor/controller connection.

macOS bundles use ad-hoc signing without notarization. Windows/Linux builds are unsigned. Native editor attachment, external hosts, accessibility, and clean-machine release acceptance still need platform checks.
