# Transient Shaper

The Dynamics catalog creates `daw.transient-shaper` at settings version 1. Project state includes the unit ID, enable, collapse, modulation panel state, routes, and seven sound controls. Reject nonfinite and out-of-range controls through the normal effect validation path. Preserve unknown future settings versions through the missing-unit path.

| Sound control | Range | Default |
| --- | --- | --- |
| Attack | −12 to +12 dB | 0 dB |
| Pump | 0 to 12 dB of attenuation | 0 dB |
| Sustain | −12 to +12 dB | 0 dB |
| Speed | 0 to 1, displayed as 0% to 100% | 0.5 |
| Level | Silence sentinel to +12 dB | 0 dB |
| Dry/Wet | 0 to 1 | 1 |
| Clip Mode | None, Soft, Hard, Tube | None |

The stereo-linked detector follows the maximum absolute sample of the input pair. It maintains two peak envelopes. Speed interpolates the sample-rate-adjusted coefficients between the following endpoints.

| Envelope | Rise at 0% / 100% | Fall at 0% / 100% |
| --- | --- | --- |
| Fast | 0.8 / 0.06 ms | 25 / 5 ms |
| Slow | 24 / 5 ms | 150 / 35 ms |

Normalize the fast and slow difference by the larger envelope, with a −120 dBFS reference floor. A positive difference weights Attack. A negative difference weights Pump attenuation. The normalized slow envelope, reduced during onset, weights Sustain. Apply the combined gain to both channels. The wet path then applies the corresponding Saturator transfer at the project rate, applies Level, and blends with the dry input. The four clip modes crossfade during edits. The unit adds zero latency and no audible tail, including in bypass. Retain detector history through compatible updates and bypass. Use the shared 5 ms parameter and bypass ramps, meter hold and clear behavior, and fixed-size callback state.

The shared body shows an illustrative gain preview, Shape, Detector, and Out strips. Expanded strips use the shared 52-point knob rows, 8-point spacing, and aligned 18-point footers. Control names use the standard knob tooltips. The preview uses the shared chart geometry and the saved shape and speed settings. Its axes stay fixed at 0–400 ms and −40 to +30 dB. The DAW shell inherits the [standard modulation contract](../modulation/contract.md). All six continuous controls accept shared routes. External tracks provide scalar modulation via those routes. The audio detector receives the effect input pair. Clip Mode is a discrete selector.

The CLAP, VST3, and macOS AUv2 previews use the same processor and shared body. [The unit manifest](../../plugins/transient-shaper/unit.toml) defines the host identities, stable parameter IDs 100–107, and control bindings. Standalone state starts with `DAWTRNS1`, then little-endian state and settings version 1 and one little-endian f64 per host parameter in ID order. Standalone automation owns parameter changes. DAW routing and UI state stay in the project. Follow the [effect plugin build contract](../architecture/all-effect-plugin-builds.md).
