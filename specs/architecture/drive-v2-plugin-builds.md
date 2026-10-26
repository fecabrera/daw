# Drive v2 plugin builds

Date: 2026-10-26. Status: CLAP/VST3 development targets and shared three-strip editor implemented. This extends the [original Drive plugin plan](unit-plugin-builds.md) after its first product. It reuses the selected Phase 0 bindings/window stack and Phase 2–4 adapter behavior. No new registry dependencies or framework selection is introduced. User instructions: [Drive v2 plugins](../../docs/effects/drive-v2-plugin.md).

## Ownership

`daw-drive::DriveV2` remains the only DSP implementation. `crates/drive-v2-plugin` owns the typed parameter/state model and prepared processor. `plugins/drive-v2/unit.toml` freezes identity and checked panel bindings. Its build uses the original shared manifest validator, which checks types, versions, duplicate IDs, ranges/defaults, control order, diagrams, and Color footer power.

The `daw-drive-v2-clap` and `daw-drive-v2-vst3` targets compile the existing format adapter sources through explicit Cargo library paths. Dependency aliases select the v2 contract and editor. Fixed arrays and parameter counts derive from the typed contract; both products can build in the same workspace invocation without feature selection changing identity. Format-specific lifecycle, buffer handling, normalized VST3 curves, state publication, and GUI ownership remain in the existing adapters. Building either target does not build the DAW apps, project, engine, media, or device crates.

`crates/unit-ui::drive_v2` owns the shared three-strip body. The DAW retains shell, selection, history, and modulation ownership. `crates/drive-v2-editor` binds ten controls to bounded Begin/Value/End batches, reserves gesture-end capacity, and retains unedited f64 host values. The editor is fixed at 270 × 218 logical points. Whole-unit Bypass is host-owned; Color power stays in its footer. Both plugin products use the shared Deep Orange (`#FF5722`) default accent in Dark, Light, and Mist.

## Frozen identity and parameters

Vendor: `Sie`. Product: `Drive v2`. CLAP ID: `io.github.fecabrera.daw.drive-v2`. Canonical VST3 processor UUID: `d4410cb2d051423f8f807d60525a7810`. Controller UUID: `3f02614c11464ba59925ff6f578b960e`. Internal provenance: `daw.drive-v2`. Plugin, settings, manifest and plugin-state versions are separate, initially 0.1.0 / 1 / 1 / 1.

| ID | Key | Plain range / default |
| --- | --- | --- |
| 100 | bypass | Boolean / off |
| 101 | level | Exact silence through +12 dB / 0 dB |
| 102 | gain | 0–1 / 0.2 |
| 103 | tone | 0–1 / 0.5 |
| 104 | color_shape | 0 Mid scoop, 1 Mid hump, 2 Mid hump + bass cut / 0 |
| 105 | color_strength | 0–1 / 0.5 |
| 106 | color_frequency | 250–2000 Hz, logarithmic / 750 Hz |
| 107 | input_gain | Exact silence through +12 dB / 0 dB |
| 108 | dry_wet | 0–1 / 1 |
| 109 | clip | 0 Soft, 1 Hard / 0 |
| 110 | color_enabled | Boolean / off |

IDs 100–106 retain familiar slots where applicable, but v2 has a distinct product identity and different Color shape meaning. Original Drive identity, state, parameter values, and two-strip panel remain covered by their existing compatibility fixtures. No automatic preset migration is claimed.

The shared `tools/package_plugins.py` accepts `--units drive,drive-v2` or `--units all`. It discovers unit manifests and Cargo library names, retains separate product archives/checksums, and stages every selected product before publication.

## State and processing

The state envelope is exactly 104 bytes: eight-byte ASCII `DAWDRV2!`, little-endian u32 plugin-state version 1, little-endian u32 portable settings version 1, then 11 little-endian f64 plain values in ID order. A synthetic default-state hex fixture and identity fixture pin this contract. Decode validates the whole candidate before publication. Bounded stream operations reject corrupt, truncated, oversized, nonfinite and future states. CLAP and VST3 use the same envelope and coherent atomic recall/publication behavior. Appearance preferences are instance-local and not serialized, matching original Drive's current editor limitation.

The processor supports the existing six rates and stereo 32-bit planar audio. It calls portable DriveV2 without another smoother, oversampling stage or processing delay. It reports zero latency and the inherited one-second residual tail policy. The existing legal processing, flush, reset, and state-consumption paths use fixed storage and must allocate/retire no Rust objects.

## Local checks and remaining gates

Executed on macOS arm64:

- Both release libraries built with the pinned toolchain and locked offline dependencies.
- Loaded instrumented libraries passed shared lifecycle/state/buffer/automation tests plus v2 all-parameter direct-DSP comparisons. Allocation positive controls ran; checked callback paths reported zero Rust allocations/deallocations.
- Synthetic editor tests covered ten control gestures, Color Off retention, keyboard edits, focus loss, close, and queue pressure.
- Shared manifest and compatibility tests passed for both products. Seven packaging tests passed, including both products across all supported bundle layouts. Extracted native v2 bundles passed strict signature verification.
- Normal loaded libraries passed 13 CLAP and 12 VST3 tests, including the inherited explicit CPU budgets: eight cases per format. Measurements are retained in the [sanitized check record](../validation/drive-v2-plugin.json).
- Original Drive regression suites, shared control tests, the DAW Drive v2 layout/history test, formatting, and strict affected-package Clippy passed. Workflow YAML parsed; CI itself was not executed locally.

Native UI/device checks are separate from synthetic egui and loaded ABI tests. SDK validators and native editor attachment were not executed for v2: the pinned validator/SDK tools were absent locally. External DAWs, native accessibility, native Windows/Linux execution, clean-machine packaging and the complete release notice audit remain open. Original Drive's CLAP validator unsupported-rate discrepancy remains relevant; no unconditional validator pass is claimed for v2.

Reproduce the loaded-library checks by building both v2 packages with `--features allocation-check`, retaining those libraries, then running their `--release --test host` suites with `DRIVE_CLAP_TEST_LIBRARY` and `DRIVE_VST3_TEST_LIBRARY` set to the retained paths. Set `DRIVE_CLAP_REQUIRE_ALLOCATION_CHECK=1` and `DRIVE_VST3_REQUIRE_ALLOCATION_CHECK=1` to require the guard. Build normal libraries again before packaging. The inherited ignored `callback_cpu_budget` tests require an explicit normal loaded-library release run and `DRIVE_CLAP_BENCH_REPORT` / `DRIVE_VST3_BENCH_REPORT` paths under ignored scratch space; they do not run in ordinary `cargo test`.

## Output meter

The shared Output strip includes the [standard level rail and held-peak indicator](modular-audio-units.md#standard-effect-out-strip-meter), using the same prepared meter and control-thread CLAP/VST3 telemetry paths as Drive. These values are not parameters or saved state. Editor dimensions, identity, and all existing state fixtures remain unchanged.
