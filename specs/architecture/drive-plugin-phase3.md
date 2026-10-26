# Drive plugin Phase 3: headless VST3 processor and controller

Status: implemented locally on macOS arm64. The normal VST3 binary passes the pinned SDK validator. A custom editor, native Windows/Linux execution, external DAW sessions and production packaging remain later gates. This is a development processor, not the completed two-format release.

This implements [Phase 3](unit-plugin-builds.md#phase-3--drive-vst3-build), retaining the [Phase 1 identity/state contract](drive-plugin-phase1.md) and original portable Drive DSP. Built-in DAW construction, project data and sound are unchanged.

## Packages and dependencies

| Package | Responsibility |
| --- | --- |
| `daw-plugin-support` | Format-neutral panic containment, fixed atomic snapshots and bounded UTF-8 copying, extracted from the CLAP helper without changing its API behavior. No SDK or device dependency. |
| `daw-drive-plugin` | Existing metadata, parameter/state and prepared Drive processor; now also owns the shared `StateExchange` recall/publication protocol used by both adapters. |
| `daw-plugin-clap` | CLAP binding facade; reexports the common boundary helpers and uses the shared state exchange. |
| `daw-plugin-vst3` | Pinned `vst3` 0.3.0 bindings, UUID-to-SDK-TUID encoding, bounded UTF-16 copying, and synchronous borrowed state streams. |
| `daw-drive-vst3` | Independent module/factory entry points, component/audio processor, separate edit controller, queue reconstruction and planar buffer boundary. |

VST3 runtime dependencies do not include the CLAP bridge, DAW apps, project/engine, window/UI, devices, separation or media-codec crates. The only new registry packages are `vst3` 0.3.0 and `com-scrape-types` 0.1.1; other locked versions are retained. Original MIT/Apache texts and the missing published workspace-license provenance are retained under [plugin-vst3 notices](../../crates/plugin-vst3/assets/licenses). The SDK checkout/validator remain ignored development tools, with their own notices. Complete distribution notice review remains Phase 5.

## Factory, ownership and lifecycle

The module exports the platform entry/exit functions and `GetPluginFactory`. Each returned factory and created object carries an owned COM reference. The factory declares one Audio Module Class and one Component Controller Class, using the frozen canonical processor/controller UUIDs from `unit.toml`. `class_id` uses the pinned bindings' platform encoding, including Windows GUID byte order. IDs do not depend on build paths, instance order, or timestamps.

The processor implements `IComponent`, `IAudioProcessor` and mandatory `IProcessContextRequirements`. Requirements return zero: Drive does not read transport, tempo, note scheduling or wall-clock context. The controller implements `IEditController` and retains its host handler only on the UI thread. It returns no custom `IPlugView` in this phase. Object references are released before module exit/unload in the loaded-binary host tests.

Setup accepts stereo 32-bit planar audio, six exact rates (44.1/48/88.2/96/176.4/192 kHz), a positive maximum block count within signed 32-bit bounds, and realtime/prefetch/offline process modes. Unsupported rates, fractional rates, other sample formats and other layouts fail explicitly. There is no hidden quality mode or sample-rate truncation.

Buses start inactive as the SDK requires; metadata requests default activation. The host activates input/output independently before component activation. Inactive trailing buses may be omitted; their channel pointers are not accessed. Active ports remain stereo, and legal silence flags supply zero input without reading uninitialized samples. Active output frames are all written and their silence flags are cleared conservatively. In-place channels and aliased input/output buffer headers are supported without aliased Rust slices.

DSP is prepared on UI during activation and retired during deactivation. `setProcessing` only changes processing status; it does not allocate, retire owners, or reset filters for transport changes. Stop/start preserves histories, while deactivation/reactivation constructs a fresh processor. Zero-sample parameter flush works while stopped, without audio buffers. Ordinary positive blocks require activation/processing and must fit the negotiated maximum. SDK callback serialization owns mutable DSP; no callback mutex, spinlock, file access or waiting queue is introduced.

Nonfinite unflagged input and malformed required buffers return an error for host discard. Earlier valid samples/automation may already have advanced DSP if malformed data appears later in a block; only state recall is transactional. Finite signal headroom is not changed by the adapter. Latency is zero and the declared residual tail is one second at the setup rate.

## Normalized automation curves

`IParameterChanges` is borrowed for one process call. The adapter scans it once into at most seven fixed cursors, one per known parameter, while leaving point data in host queues. It does not copy a vector, reserve an arbitrary event capacity, add COM references per point, or drop valid points after a fixed count. Unknown parameter IDs are ignored. Duplicate known queues, invalid point counts, nonfinite/out-of-range normalized values, decreasing offsets and offsets outside the block fail. Equal-offset points are consumed in order; the last one wins.

For each queue, the previous block's final normalized target is an implicit point at offset −1. An explicit first point at offset 0 replaces it immediately. Otherwise, each sample before the next point receives its linear normalized value between the two points. After the last point, the value is held. Empty queues retain the previous value. A jump is represented by the host's old/new points on adjacent samples; the adapter does not replace the curve with steps or use just the final point.

Each normalized value converts through the shared parameter mapping before retargeting the original Drive ramps once. Frequency and Volume therefore keep their existing mappings; discrete choices use the Phase 1 nearest-choice rule. A separate fixed array retains raw normalized targets between blocks, including values which map to the same discrete choice. DSP smoothing state is never used as the implicit automation point. A validated preset recall resets this base array from the recalled parameter values before that block's queues run.

Tests compare the loaded binary against direct Drive with independently reconstructed streams, including `(0, 1.0)` to `(31, 0.0)`, first-point omission across blocks, held blocks, sample-17 jumps, equal-time multi-parameter changes and a 4,096-point queue. This closes the linear-curve and growing-vector gaps found in the rejected Phase 0 framework.

## State and controller synchronization

The component uses the same 72-byte versioned envelope as CLAP. The shared `StateExchange` validates recall on UI, coalesces active recalls into a fixed atomic mailbox, applies them at processing/flush boundaries, and publishes coherent base-parameter snapshots back to UI without audio-thread allocation, stream access or serialization. A recall published during a block waits for the next boundary; that boundary's queues then take precedence. Save uses a bounded coherent read and can fail under contention rather than emit torn state. Inactive recall applies immediately.

IBStream adapters handle short reads/writes, EOF, invalid counts and host errors on the UI thread. Invalid, truncated, oversized, nonfinite or future-version state retains the working snapshot. State contains no DAW identities, routes, paths, exact filter histories or window state.

The component and edit controller are separate COM objects. The host forwards component state through `setComponentState`, and forwards parameter updates through `setParamNormalized` plus processor queues, according to the SDK. The controller never echoes these host updates through `IComponentHandler`. There is no shared Rust pointer shortcut or dependence on the controller living in the processor's process. Host-driven generic-editor gestures remain host-owned; custom editor gesture production is Phase 4 work.

No controller-specific preferences exist yet. Controller `getState`/`setState` return `kNotImplemented`, rather than save a second conflicting copy of DSP parameters. `setComponentState` restores all seven controller base values atomically. Future editor preferences need their own schema/migration policy. VST3 display strings reuse shared formatting but leave units to `ParameterInfo.units`; parser and normalized/plain conversions retain the shared meanings.

## Local evidence and release limits

The [sanitized snapshot](../validation/drive-plugin-phase3.json) records the artifact/compiler profile, native validator, loaded-binary checks and callback measurements. The checker loads both a normal artifact and an opt-in instrumented artifact through exported factory/COM interfaces. A known allocation/free positive control confirms the instrumented DLL's guard is active before asserting zero Rust callback allocations and deallocations.

Coverage includes six rates/four shapes, blocks 1/17/32/64/128/512/4096 and irregular changes, static and curved automation, dense queues, zero-audio flush, stereo asymmetry, silence/in-place/header aliases, bypass/Color Off retained settings, exact silence, state failures/short streams, instance isolation, controller synchronization without echo, inactive buses, reactivation, and concurrent UI recall/save with audio processing. Exported impulse-to-silence results match direct Drive without adapter delay and decay within the declared one-second policy in the tested corpus. This is not proof for arbitrary preceding histories.

Steinberg validator from the pinned SDK checkout reporting 3.8.1: **47 passed, 0 failed**. Its sample-rate test emits diagnostics for rejected unsupported rates but reports that case as Succeeded. These diagnostics are retained. Its unsupported 64-bit/layout cases do not establish support for those formats. Unlike the CLAP validator's separate rate test, this SDK validator accepts clean refusal; CLAP's existing rate-policy discrepancy is not silently waived by this result.

Static and per-sample Gain CPU comparisons use the unchanged [Phase 0 budgets](../../tools/drive_plugin_phase0/budgets.json), two block sizes and one/eight instances. An initial run before the concurrent workspace suite passes all eight cases. A later run concurrent with workspace compilation/testing misses one automated median by about 33 ns; both measurements are retained. After the workspace suite finishes, the unchanged thresholds pass all eight cases in a standalone rerun. Both the contended failure and final measurements are retained in the snapshot; the workspace also passes, as recorded in [validation history](../validation/README.md). Different direct-chain and host-block traversal/cache behavior does not justify a universal overhead percentage.

See [README native build/check commands](../../README.md#drive-vst3-development-processor). The custom two-strip editor, actual external DAW sessions, Windows/Linux/macOS x86_64 execution, portable signed packaging and full release notices remain open. No plugin is automatically installed and this phase does not complete the first product delivery.
