# Built-in units and separate plugin builds

Status: accepted direction; implementation plan. [Phase 0 local feasibility](drive-plugin-phase0.md) is implemented with isolated development probes. [Phase 1 shared metadata, parameters, and state](drive-plugin-phase1.md) are implemented. [Phase 2 headless CLAP processor](drive-plugin-phase2.md) is implemented and checked on macOS arm64. [Phase 3 headless VST3 processor/controller](drive-plugin-phase3.md) is implemented and checked on macOS arm64. [Phase 4 shared editor implementation](drive-plugin-phase4.md) now exposes the two-strip body in both formats. Native accessibility, other platforms, external hosts, and release gates remain open.

Date: 2026-10-25.

## Decision and first delivery

Keep units built into the DAW through their existing portable Rust crates. Add separate CLAP and VST3 builds that reuse those processors. Start with the catalog's original **Drive**, type `daw.drive`, implemented by `daw-drive::Drive` with `DriveSettings` state version 1. [Drive v2](../effects/drive-v2.md), type `daw.drive-v2`, is a separate effect and is outside the first delivery. Its subsequent independent targets are documented in [Drive v2 plugin builds](drive-v2-plugin-builds.md).

The first release must include both formats and the custom two-strip Drive editor described below. An editor-free adapter is an intermediate development milestone only.

The DAW continues to construct and call Rust processors directly. Plugin distribution does not require a CLAP/VST3 host, runtime unit discovery, an external-unit ABI, or hot reload inside the DAW. Third-party hosting remains separate roadmap work. Existing project identities, settings, modulation, transport checkpoints, and audio behavior remain the baseline.

Building an individual plugin target must not build the desktop or CLI application. Changing shared DSP can still rebuild its static DAW dependents when the DAW itself is built. Plugin exports do not remove that dependency.

Related contracts: [Drive](../effects/drive.md), [shared modulation](../modulation/contract.md), [modular units](modular-audio-units.md), and [prepared rendering](performance.md).

## Plugin editor

The user's visual reference is the existing Drive body: two adjacent strips, with no DAW unit header. Treat this layout as a release requirement:

| Strip | Top control | Middle control | Bottom control | Footer |
| --- | --- | --- | --- | --- |
| Drive | Volume knob and dB readout | Gain knob and percent readout | Tone knob and percent readout | Drive |
| Color | Shape knob with response diagrams | Strength knob and percent readout | Frequency knob and Hz readout | Color |

Reuse the shared knob interaction and painting, current value formatting, gain scale with exact silence, reference ticks, endpoint labels, shape diagrams, strip padding, footer typography, theme helpers, Outfit, and Lucide assets. Derive logical geometry from the existing 90-point strips and shared body-height constants; do not infer logical dimensions from screenshot pixels. Keep the central divider and the current relative control/readout positions.

Color Off dims and disables the editor's Strength and Frequency controls without discarding their values. Host automation may still change those retained parameters while Color is Off. The Shape control retains Off, Mid scoop, Mid hump, and Mid hump + bass cut. Match the built-in control's reset, keyboard, Shift sensitivity, cyclic choice behavior, tooltips, and accessibility.

Exclude the DAW title/header power dot, collapse control, selection outline, effect-chain dragging, project menus, and instrument modulation connections. The host supplies its own plugin window frame. Expose plugin bypass to the host even though this body has no bypass button. Do not add a redundant bypass control or extra parameter rows to the reference layout.

The editor must work when embedded by a host, handle OS display scaling, and open/close repeatedly during playback. Fixed logical size with correct scaling is sufficient for the first release; free resizing is deferred. Use shared application-theme primitives, with plugin-local editor preferences where needed. Appearance preferences must not enter automatable DSP parameters or require access to DAW application storage. The screenshot fixes the control arrangement; its current accent and light palette do not define a new hardcoded theme.

## Ownership and dependency direction

Package names and paths below describe the implementation direction. `crates/drive-plugin`, `crates/plugin-support`, both format bridges/export targets, and `plugins/drive/unit.toml` now exist; shared editor packages now exist.

| Location | Responsibility |
| --- | --- |
| `crates/drive` | Existing shared stereo DSP, ramps, bypass, history, and final bounds. No plugin SDK or UI dependencies. |
| `crates/unit-core` | Existing portable settings and modulation contracts. No plugin SDK, window, or host dependencies. |
| `crates/drive-plugin` | Format-neutral Drive parameter model, state envelope, and block/event adapter over the existing processor. |
| `crates/plugin-support` | Format-neutral ABI containment, atomic snapshots, and bounded text helpers. |
| `crates/plugin-clap` | Narrow reusable CLAP binding facade over the common boundary helpers. |
| `crates/plugin-vst3` | Narrow reusable VST3 processor/controller/event/state bridge. |
| `crates/plugin-window` | Selected Phase 0 direction: thin native editor/window adapter over the verified egui backend, shared by both format bridges. |
| `crates/unit-ui` | Minimal reusable controls, formatting, theme/assets, and Drive body extracted from the current UI. No DAW project, engine, output, or file-dialog dependencies. |
| `plugins/drive-clap` | CLAP entry point and independent plugin artifact. |
| `plugins/drive-vst3` | VST3 entry points and independent plugin artifact. |
| `plugins/drive/unit.toml` | Build-time metadata, stable parameter bindings, and the two-strip panel description. |
| `tools` | Native plugin bundling, validator invocation, artifact inspection, and parity/performance harnesses. |

Keep common bridges limited to behavior used by Drive and required format contracts. Share lifecycle and data conversion where useful; retain format-specific semantics. Do not introduce a universal DAW panel or rendering interface. A framework selected in Phase 0 may supply the bridge packages instead; record the resulting concrete dependency layout before implementation.

The existing DAW panel delegates its reusable body to `unit-ui` and retains the DAW shell, routing, selection, and history handling in `crates/ui`. Plugin editors embed that same body with host parameter bindings. Do not make plugin builds depend on `daw-ui` merely to obtain a knob: it currently pulls in DAW application responsibilities. Extract only the controls and Drive body needed for this delivery, preserving their tests and licenses.

Keep plugin framework dependencies outside the desktop and CLI dependency graph. Share versions in the workspace and retain the lockfile. Native platform window/FFI code belongs in the plugin boundary packages; document unsafe operations and ownership. DSP remains safe portable Rust.

## Manifest, identity, and parameter contract

The manifest is declarative build input, not a runtime plugin-loader contract. Generate or validate descriptor metadata, parameter declarations, and panel bindings from one definition. Other hosts discover the compiled CLAP/VST3 descriptors; they do not need to understand `unit.toml`. Include the manifest schema version and validate duplicate IDs, unknown controls, missing bindings, invalid ranges/defaults, and unsupported diagram references during the build.

Use standard control/scale/diagram identifiers and typed parameter bindings. Do not embed executable scripts or arbitrary drawing code. Keep final parameter validation in the portable settings/DSP. If introducing a manifest requires duplicating authoritative ranges, add generation or consistency checks against the shared definitions. Avoid a repository-wide metadata conversion in this first delivery.

Freeze public plugin identity before producing compatibility fixtures: CLAP plugin ID, VST3 processor/controller class IDs, vendor/product names, and stable parameter IDs. Use the original internal type ID as provenance; it does not replace the VST3 class IDs. Do not regenerate class IDs per build, use list position as identity, or reuse Drive v2 identities.

The following numeric IDs are frozen by [Phase 1](drive-plugin-phase1.md):

| ID | Key | Plain value / default | Adapter behavior |
| --- | --- | --- | --- |
| 100 | `bypass` | Boolean; false | Maps to `!DriveSettings.enabled`; VST3 declares its standard bypass flag. |
| 101 | `volume` | `SILENT_GAIN_DB` through +12 dB; 0 dB | Exact silence displays as minus infinity; preserve the shared gain mapping and finite saved representation. |
| 102 | `gain` | 0–1; 0.2 | Displays 0–100%; retain the existing approximately 36 dB maximum drive. |
| 103 | `tone` | 0–1; 0.5 | Displays 0–100%; center remains neutral. |
| 104 | `color_shape` | Integer 0–3; 0 | Freeze enum order as Off, Mid scoop, Mid hump, Mid hump + bass cut. |
| 105 | `color_strength` | 0–1; 0.5 | Displays 0–100%. |
| 106 | `color_frequency` | 250–2000 Hz; 750 Hz | Logarithmic editor/normalized mapping. |

CLAP exposes plain parameter values; VST3 transfers normalized values. Share plain/normalized/text conversions and pin their endpoint, enum, logarithmic, and silence behavior in compatibility fixtures before release. Extract pure gain conversion/formatting helpers from the UI if needed. Test low finite dB values and floating-point round trips; do not silently collapse supported finite values into the silence sentinel. Resolve conversion precision limits in Phase 1 and document the tolerances. Add new parameter IDs in future versions rather than renumbering released ones.

Plugin version, plugin state schema version, manifest schema version, and the existing Drive settings version are separate. Ordinary compatible releases retain plugin identity and parameter meaning. Do not promise that arbitrary hosts can select multiple installed versions with the same identity. Deliberately incompatible products need distinct identities and documented migration.

## Processing and lifecycle contract

- Initial audio configuration is one stereo input and one stereo output, 32-bit floating point, with no sidechain or note input. Reject unsupported bus/sample formats through the format's normal negotiation/error path. Mono, extra buses, and 64-bit sample processing are deferred.
- Support the six project rates: 44.1, 48, 88.2, 96, 176.4, and 192 kHz. Reject unsupported or nonfinite rates cleanly at setup/activation; never silently truncate a fractional rate or clamp a rate. Additional external-host rates require explicit tests and a contract extension.
- Accept changing block lengths within the negotiated maximum, including one-sample and irregular blocks. Preallocate resources from that maximum. Handle the format's legal zero-audio/parameter-flush cases. Larger supported blocks may be processed in internal chunks without inserting a block of delay.
- Walk planar host buffers and call the existing `Drive` processor for each stereo frame within the plugin block. Do not rewrite the saturation/filter kernel, normalize the output differently, add oversampling, or route audio through `daw-engine`. Handle legal in-place/separate buffers and silence flags correctly.
- Apply parameter changes at their sample positions. CLAP timed value events and VST3 parameter queues retain their format semantics. For VST3, reconstruct automation curves as required by the pinned SDK; do not use only the last point of each block. Define deterministic simultaneous-event ordering and preserve values across blocks. Compare formats using equivalent reconstructed parameter streams, not by assuming their event representations are identical.
- Use the existing five-millisecond parameter, shape, and bypass ramps exactly once. Adapter/framework smoothing must not duplicate portable smoothing. Initialization/reset takes the current state through the existing reset behavior; an ordinary parameter edit retains compatible filter histories and in-flight ramps.
- Run legal processing/parameter paths with no allocation, deallocation of final owners, locks, file I/O, logging, GUI calls, serialization, or unbounded waits. Lifecycle allocation and state conversion occur on the threads permitted by each SDK. Bound editor-to-processor communication and define queue-full coalescing/retry so gesture-end and latest values cannot be lost. Consume host event queues in order without a fixed event limit silently dropping automation.
- CLAP init/activate/deactivate/destroy, callbacks, and state methods follow its declared thread/lifecycle rules. VST3 separates controller/UI duties from processor duties. No Rust panic may unwind across either FFI boundary; validate expected failures and map them to format errors without adding work to the normal callback path.
- Editor changes notify the host with gesture begin/change/end; host automation and restored state update the editor. The host remains responsible for its automation and Undo history. Do not use DAW `Edit` commands or copy DAW history into the plugin.
- V1 exports base parameter automation in both formats. It does not export DAW-specific Envelope/LFO/Macro connections, provider UUIDs, or modulation-routing strips. CLAP modulation flags remain off until a separately specified bridge can reuse the typed shared modulation contract. No second route evaluator is introduced.
- Report zero algorithmic latency and add no adapter buffering delay. Measure this through the exported binaries. Filter residuals are not latency: determine a conservative tail/sleep policy in Phase 0, report it through the relevant format interfaces, and verify impulse-to-silence behavior across settings/rates. Do not infer zero tail from zero latency or unconditionally advertise sleep while residual audio remains.
- Keep histories processing during plugin-owned bypass as the portable processor does. A host that hard-bypasses or suspends the entire plugin controls whether processing continues. Reset only for the format's reset/setup lifecycle; do not reset filters on every block or every playhead discontinuity. Realtime/offline modes use the same DSP/settings; no hidden quality switch or sound change.

## State and compatibility

Use a small versioned plugin state envelope containing the seven parameter values and only the additional editor preferences needed for reliable reopening. Validate the complete candidate before publication. Bound input size and stream reads, reject truncated/corrupt/nonfinite/unsupported state, and retain the current working state on failure. Handle short stream reads/writes and report failures using each SDK.

Restore processor and controller/editor consistently. State capture must read a coherent parameter snapshot; SDK save/load calls and serialization stay outside processing. Publish validated changes through a bounded prepared handoff or equivalent safe mechanism. Do not acquire a callback mutex for state saving. Define stopped/active restore and parameter/state ordering in adapter tests.

Do not embed DAW project folders, asset paths, effect instance IDs, collapsed DAW panels, or instrument modulation routes. Plugin state is a settings/presentation snapshot, not an exact filter-history checkpoint. Existing built-in transport-cache cloning remains unchanged. Cross-format internal preset interchange can reuse the envelope, but host-specific preset-container interchange is deferred.

Save/reopen, duplicate instances, host bypass recall, editor reopen, older supported schema migration, and processor/controller synchronization are release requirements. Commit synthetic compatibility fixtures once identities and schema are frozen. Do not import private host projects or recordings.

## Implementation sequence

Each phase has an exit gate. These are engineering completion criteria, not additional requests for user approval.

### Phase 0 — Adapter and host feasibility

Local results and the narrow-adapter decision are recorded in [Phase 0 feasibility](drive-plugin-phase0.md). The macOS experiment is reproducible; cross-platform native checks remain open.

1. Compare a maintained dual-format Rust framework with narrow CLAP bindings and VST3 bindings/SDK adapters. Inspect exact pinned releases, transitive licenses, supported platforms/toolchain, editor embedding, automation, bypass, tail reporting, thread rules, and packaging.
2. Produce minimal stereo pass-through test artifacts for both formats in ignored scratch space. Prove discovery, lifecycle, event delivery, host-owned buffers, and native editor-window feasibility before selecting a framework.
3. Record the selected stack, revisions, notices, native build requirements, target architectures, named external test hosts, and validator versions. Confirm the pinned Rust toolchain works without an unrelated upgrade.
4. Resolve the tail policy and record measurement criteria; define quantitative CPU/callback-budget acceptance thresholds from a direct-DSP baseline before measuring the candidate.

Exit: a written dependency/host decision, reproducible prototype build instructions, and a concrete mapping for every required lifecycle and editor operation. A dependency or native editor gap must be addressed explicitly before claiming both formats are deliverable.

### Phase 1 — Identity, manifest, parameters, and state

Implemented contract and compatibility fixtures are recorded in [Phase 1](drive-plugin-phase1.md). Native adapter publication and editor integration remain in their respective phases.

1. Freeze the public identity tuple, parameter IDs, mappings, schema versions, and plugin release-version policy. Public vendor/class identifiers must be assigned once before public builds.
2. Implement Drive's declarative metadata and panel bindings, with build-time validation and generated/checked descriptors. Keep the DAW's catalog and concrete construction unchanged.
3. Implement the shared parameter/state model and pure conversions. Freeze compatibility fixtures and tests for exact silence, Color Off retention, discrete choices, defaults, text input, invalid values, and failed state restoration.

Exit: one reviewed contract used by both adapters, with no duplicate parameter meaning or DSP algorithm.

### Phase 2 — Drive CLAP build

Local implementation/evidence and the retained upstream rate-sweep failure are recorded in [Phase 2](drive-plugin-phase2.md). The headless development artifact is not the completed product release.

1. Add the plugin target and required entry/factory, audio ports, parameters/flush, state, latency/tail, processing status, and host-callback handling.
2. Connect block/events to the portable processor. Exercise activation failures, multiple instances, resets, in-place buffers, variable block sizes, stopped parameter edits, bypass, and repeated lifecycle transitions.
3. Run CLAP validation and direct-DSP parity/allocation tests. An editor-free build may be used here but is not a release artifact.

Exit: a working CLAP processor and state implementation with deterministic parity evidence.

### Phase 3 — Drive VST3 build

Local component/controller implementation, normalized curve reconstruction, and SDK evidence are recorded in [Phase 3](drive-plugin-phase3.md). The custom editor and external-host/platform release acceptance remain later work.

1. Add stable factory/component/controller identities, stereo bus negotiation, processor lifecycle, parameter queues/conversions, state, bypass flag, latency/tail reporting, and controller synchronization.
2. Reuse the same Drive model and processor adapter where semantics match. Preserve VST3 automation curve and gesture requirements.
3. Run the pinned VST3 validator and equivalent parity/allocation/lifecycle tests.

Exit: a working VST3 processor using the same DSP and compatible plugin state model.

### Phase 4 — Shared two-strip plugin editor

1. Extract minimal UI primitives and the Drive body into the dependency-light shared UI package. Keep the DAW shell and routing in `daw-ui`; preserve current DAW appearance and interaction tests.
2. Bind the body to host parameters through each format's UI/controller channel. Use native plugin editor windows and the SDK's threading/resize/scaling contracts. Do not pass Rust UI objects across a dynamic binary interface.
3. Check the user's body layout in both formats, themes, supported display scales, and repeated editor open/close cycles while playing and automating. Verify all six controls, Color Off behavior, host gestures, focus, keyboard/text interaction, reset, and accessibility. The headless processing path must not initialize graphics.

Exit: both formats expose the required custom body; native editor checks are recorded separately from synthetic egui tests.

### Phase 5 — Packaging, CI, documentation, and release evidence

Partial implementation: [Build plugins](../../.github/workflows/plugins.yml) builds development previews on independent Windows x64, macOS arm64, and Linux x64 workers. Each worker gates uploads on packaging tests and native unit/loaded-library tests. [The packager](../../tools/package_plugins.py) creates separate native CLAP/VST3 ZIPs with notices, metadata, instructions and checksums. Select units with `--units drive,drive-v2` or `--units all`. It discovers unit manifests and their format target library names without a product allowlist, and stages the full selection before replacing archives. Shared setup, logs and summaries are reused. External SDK validators, instrumented allocation checks, callback budgets, dependency inspection, native GUI/host acceptance and the complete release notice audit remain separate gates; this workflow does not complete Phase 5 or produce release-ready artifacts.

1. Add native packaging scripts for `.clap` and `.vst3` bundles on macOS, Windows, and Linux. Keep binaries/host code inside the correct platform bundle layout; inspect dynamic dependencies on clean test machines.
2. Produce independent plugin archives with version/architecture metadata, project and dependency notices, font/icon notices, installation/removal instructions, and checksums. Development packaging does not install into a user's plugin directories. Developer ID signing/notarization and public upload belong to an explicit release task. Native macOS development bundles receive an ad-hoc resource seal after assembly and library-ID relocation; this does not establish distribution trust.
3. Extend native CI using shared setup/log/summary helpers. Gate each platform's plugin artifacts on its own tests/validators; retain independent DAW test/build behavior. Verify a targeted plugin build does not compile/link desktop, CLI, project, engine, CPAL, dialogs, separation, or media-codec crates. Optional GUI build dependencies are expected only for editor targets.
4. Document actual installation, controls, automation, supported rates/formats, host bypass limits, and state compatibility in a plugin user guide when working artifacts exist. Keep dependency/API/build/validation details in specs and platform build steps in README.
5. Run the release matrix below and append actual commands/results and native gaps to validation history. Require all advertised format/platform combinations to pass before calling the first delivery complete; label partial artifacts as development previews.

Exit: independently packaged Drive CLAP/VST3 plugins with the shared editor and recorded automated/native acceptance evidence.

## Acceptance matrix

| Area | Required evidence |
| --- | --- |
| Build separation | Targeted builds for each format succeed from cached and clean dependency environments; dependency/timing inspection proves the DAW applications are not built. Existing DAW builds keep direct Rust unit calls. |
| DSP parity | Same target/compiler/DSP settings and reconstructed parameter stream match direct `Drive` sample-for-sample where arithmetic is identical. Any conversion-only tolerance is measured and documented, not used to hide different DSP. Cover all Color shapes, gain/tone/output extremes, exact silence, stereo asymmetry, silence, finite headroom, and nonfinite-input policy. |
| Rates and blocks | Six supported rates; block sizes 1, 17, 32, 64, 128, 512, 4096, changing/irregular sizes within the negotiated maximum, and format-legal no-audio calls. Reject unsupported configurations cleanly. |
| Automation and bypass | Multiple points per block, first/last sample, simultaneous parameters, edits during ramps, Color Off retained changes, stopped edits, UI gesture order, plugin-owned bypass transition, host hard bypass/suspension, and normalized/plain/text endpoints. |
| State | Defaults, save/reopen, instance duplication/isolation, active/stopped restore, supported migrations, malformed/oversized/truncated state, future schema rejection, and failure retaining the current state. |
| Realtime behavior | Allocation/final-owner retirement guards around processing and event retargeting; no callback locks/I/O/serialization/GUI calls. Benchmark direct DSP and both exported formats with identical buffers/events, single and multi-instance chains, automated/static settings, small/default buffers, and recorded hardware/build profile. Meet the Phase 0 budget; do not claim a universal overhead percentage. |
| Latency and tails | Exported impulse tests demonstrate zero adapter delay and correct SDK latency reporting. Verify the declared tail/sleep policy and bypass histories; no extra block of buffering. |
| Editor | Two-strip body visual comparison, all controls and shape diagrams, correct silence/log scales, themes/display scaling, automation/UI synchronization, focus/keyboard/accessibility, repeated editor lifecycle, and no DAW-specific controls. Record native checks on every advertised OS. |
| Hosts and validators | CLAP validator and Steinberg VST3 validator, with exact versions/commands. At least one named non-DAW external host per format on every advertised OS; cover scan, instantiate, playback, offline render, automate, bypass, save/reopen, duplicate, and reopen editor. Missing host coverage blocks the corresponding release claim. |
| Regression and packaging | Required workspace checks after Rust changes; existing Drive/project/UI compatibility retained. Verify bundle loading, architecture, dynamic dependencies, licenses/assets, clean-machine installation/removal, and independent archives. |

During implementation run affected checks, then `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` with the pinned toolchain. Run targeted plugin builds, validators, native editor/host checks, and native packaging in addition. CLI smoke checks are required only if shared DAW import/render/export or CLI code changes. Documentation-only plan edits require content/link and diff checks, not audio builds. Phase 0 now also has isolated tool checks documented in its feasibility record.

## Dependency and protocol research

The following primary sources were checked on 2026-10-25. They inform the plan; they do not select or add a dependency. Recheck exact pinned revisions and all transitive terms during Phase 0.

- [CLAP specification](https://github.com/free-audio/clap): stable plugin interface and extension discovery; [lifecycle](https://github.com/free-audio/clap/blob/main/include/clap/plugin.h), [parameters](https://github.com/free-audio/clap/blob/main/include/clap/ext/params.h), [state](https://github.com/free-audio/clap/blob/main/include/clap/ext/state.h), [GUI](https://github.com/free-audio/clap/blob/main/include/clap/ext/gui.h), and [tail](https://github.com/free-audio/clap/blob/main/include/clap/ext/tail.h) define the relevant contracts.
- [CLAP validator](https://github.com/free-audio/clap-validator): upstream plugin-validation tool. Passing it supplements host testing.
- [Steinberg VST3 SDK](https://github.com/steinbergmedia/vst3sdk): SDK and validator; [parameter/automation contract](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Parameters%2BAutomation/Index.html) defines normalized values, stable IDs, bypass, queues, and edit gestures. [Current licensing documentation](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Index.html) states MIT from SDK 3.8 and separate trademark rules. Older SDK/binding terms must be checked independently.
- [clap-sys manifest](https://github.com/micahrj/clap-sys/blob/master/Cargo.toml) declares MIT/Apache-2.0 licensing. [vst3-rs manifest](https://github.com/coupler-rs/vst3-rs/blob/master/Cargo.toml) declares MIT OR Apache-2.0. These are low-level binding candidates; neither supplies the complete Drive editor/host contract on its own.
- [NIH-plug](https://github.com/robbert-vdh/nih-plug) demonstrates dual-format Rust exports and editor adapters. Its inspected README describes ISC framework code, GPLv3 VST3 bindings, and maintenance-mode status. Do not infer a framework's or its bundled bindings' terms from the current Steinberg SDK license. Evaluate a maintained stack and its actual dependency graph before selection.


[Saturator and Bitcrusher](saturator-bitcrusher-plugin-builds.md) now have independent CLAP/VST3 targets, typed contracts, shared DAW/plugin bodies, output meters, per-unit latency reporting, packaging and CI entries. Their release/native-host gates remain open.

[All effect plugin builds](all-effect-plugin-builds.md) extends this architecture to fourteen standalone effect products, with host transport, rate-dependent latency, shared dynamics telemetry, and fixed Multiband EQ slots.
