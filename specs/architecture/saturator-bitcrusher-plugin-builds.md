# Saturator and Bitcrusher plugin builds

Status: implemented development previews. Release acceptance remains open.

## Ownership and reuse

Each unit has independent `daw-<unit>-plugin`, `daw-<unit>-editor`, `daw-<unit>-clap`, and `daw-<unit>-vst3` packages and a checked `plugins/<unit>/unit.toml` manifest. Export targets compile the existing narrow CLAP/VST3 sources with typed plugin/editor dependency aliases. They do not depend on DAW UI, project, engine, or output crates.

The existing portable Saturator and Bitcrusher own audio processing, 5 ms parameter/bypass ramps, history, bounds, and output measurement. The new headerless bodies in `unit-ui::{saturator,bitcrusher}` serve both the DAW shells and plugin editors. DAW modulation, selection, power, collapse and history stay in `ui`. Keep Saturator's existing dropdowns and Bitcrusher's integer knob behavior. Both editors measure 180 × 218 logical points and include the standard Out-strip rail/held peak. Shared editor/control code retains bounded gestures, queue-pressure handling, scaling, appearance and telemetry. Drive v2 embeds the same editor implementation; its control layout and state remain unchanged.

The new typed plugin models share bounded state I/O, prepared processor lifecycle, parameter conversion, and atomic state exchange. Unit-owned mappings select portable settings/DSP and omit DAW routes, paths, IDs, presentation and DSP histories. State and preset application remain transactional. No callback allocations, final-owner retirement, locking, I/O, serialization, SDK messaging, or GUI work are allowed.

The manifest checker accepts a variable number of bound controls per strip: three on the left and two on Out. Frozen Drive/Drive v2 bindings and identities remain valid. Parameter IDs are stable explicit integers. VST3 units come from typed parameter metadata, so Drive in dB is not mislabeled as percent. Nonzero-based integer choices normalize over minimum to maximum; Resolution and Downsampling remain integers during normalized automation.

## Identity and parameter contract

The manifests and synthetic identity/default-state fixtures freeze each product's vendor, product name, CLAP ID and distinct processor/controller class IDs. Do not regenerate IDs during builds or reuse another product's IDs. CLAP IDs are `io.github.fecabrera.daw.saturator` and `io.github.fecabrera.daw.bitcrusher`. Both use vendor Sie, version 0.1.0, settings version 1 and plugin-state version 1.

| ID | Saturator key / range / default | Bitcrusher key / range / default |
| --- | --- | --- |
| 100 | bypass / 0–1 / 0 | bypass / 0–1 / 0 |
| 101 | level / exact silence through +12 dB / 0 dB | level / exact silence through +12 dB / 0 dB |
| 102 | drive / −36–+36 dB / 0 dB | drive / 0–24 dB / 0 dB |
| 103 | shape / 0–4 / Analog Clip (0) | resolution / 1–24 bits / 24 |
| 104 | clip_mode / 0–2 / No Clip (0) | downsampling / 1–40× / 1 |
| 105 | dry_wet / 0–1 / 1 | dry_wet / 0–1 / 1 |

Choice orders follow the portable enums: Saturator Shape is Analog Clip, Soft Sine, Bass Shaper, Medium Curve, Hard Curve; Clip Mode is No Clip, Soft Clip, Hard Clip. Bypass maps to inverse `enabled`. Level reuses the shared exact-silence gain mapping and finite −1000 dB sentinel. Drive uses linear dB; Dry/Wet uses a linear ratio. Host text uses each control's native units. CLAP truncates stepped plain values; VST3 rounds normalized choices after mapping their full ranges.

State envelopes are 64 bytes: eight-byte ASCII magic (`DAWSAT1!` or `DAWBCR1!`), little-endian u32 state version 1, little-endian u32 settings version 1, and six little-endian f64 plain values in ID order. Validate length, magic, both versions, finite bounds and integer choices before publication. Reject foreign/future/corrupt states without changing current state. Appearance, meters and peak reset do not enter saved state or the host parameter list.

## Formats, latency and telemetry

Both formats accept one stereo input/output bus, f32 planar buffers and the six project rates. Reuse in-place handling, bounded legal zero-audio flushes, format-specific timed automation and coherent active recall. Do not duplicate portable smoothing or add another resampler/oversampling layer.

Saturator reports its portable fixed 32-frame FIR latency through CLAP and VST3, including dry/bypass. Bitcrusher and both Drive products report zero. The shared adapters read unit-owned latency metadata. Retain the existing conservative one-second tail report and continue processing quiet blocks so held samples/filter tails are not cut by the adapter.

Output meters reuse the [standard Out-strip contract](modular-audio-units.md#standard-effect-out-strip-meter). CLAP shares the prepared atomic meter with its editor. VST3 uses the existing control-thread connection messages; the processor/controller exchange scalar values and reset requests, never pointers. Meter capture remains independent of window visibility.

## Packaging and CI

`tools/package_plugins.py --units saturator,bitcrusher` packages both new products; `--units all` discovers Drive, Drive v2, Saturator, and Bitcrusher. No per-product packager allowlist is used. Build both native format libraries for every selected unit first. Keep per-product ZIPs, checksums, manifests, licenses, native layouts and macOS ad-hoc resource signatures. Staging/signing failure must preserve every selected product's prior artifacts.

`plugins.yml` has four unit entries across Windows x64, macOS arm64 and Linux x64: twelve independent jobs. Each builds both formats, tests the typed contracts/editors and native libraries, then packages/uploads separate previews, checksums and logs. Shared host-test library environment names retain the existing `DRIVE_*` spelling because the harness is reused. Native runs supply the selected unit's library paths.

Host ABI tests compare every parameter/choice and dense automation against direct portable DSP at six rates, irregular blocks and both buffer modes. Loaded allocation-check builds require a working positive control and zero Rust allocations/deallocations on checked callback paths. Explicit normal release CPU tests reuse the existing matched-output-monitoring harness and unchanged budgets; they remain ignored in ordinary workspace tests. See [validation history](../validation/README.md) for executed commands and remaining limits. Native Windows/Linux runs, SDK validators, real editor attachment, external hosts, accessibility and clean-machine acceptance are separate gates.
