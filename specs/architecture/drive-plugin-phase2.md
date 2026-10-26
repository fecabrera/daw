# Drive plugin Phase 2: headless CLAP processor

Status: implemented and checked locally on macOS arm64. This is a development processor, with no custom editor. [Phase 3](drive-plugin-phase3.md) adds the VST3 processor/controller; shared editor, release notices/packaging, Windows/Linux execution, and external DAW sessions remain later gates. The full upstream validator has one retained failure outside the accepted six-rate contract; it is not reported as green.

This implements [Phase 2](unit-plugin-builds.md#phase-2--drive-clap-build) over the [Phase 1 contract](drive-plugin-phase1.md). The original built-in Drive DSP, catalog construction, DAW project data, and app dependency graph are unchanged.

## Ownership and build separation

Phase 3 moves the shared boundary helpers into `daw-plugin-support` and recall/publication into `daw-drive-plugin::StateExchange`; the table below records the initial Phase 2 ownership. Current common ownership is detailed in [Phase 3](drive-plugin-phase3.md).

| Package | Implementation |
| --- | --- |
| `daw-drive-plugin` | Adds a prepared format-neutral `Processor` over the existing `daw-drive::Drive`. Validates rate/block configuration, projects validated plugin state into portable settings, retargets existing ramps, and resets/processes the original DSP. |
| `daw-plugin-clap` | Small SDK boundary helpers: reexports pinned `clap-sys` 0.5.0, contains panic propagation, copies bounded UTF-8, and publishes fixed atomic snapshots. No concrete unit or DAW dependency. |
| `daw-drive-clap` | Independent `cdylib` entry/factory, instance lifecycle, stereo audio ports, parameter/flush callbacks, state stream bridge, zero latency and one-second tail. Native pointers and unsafe ownership remain here. |
| `tools/check_drive_clap.py` | Native development build, owned scratch bundle, loaded-binary tests, allocation instrumentation, callback budgets, dependency target inspection, and full validator evidence. No installation or public upload. |

The target exports the frozen Sie / Drive identity and IDs from the checked manifest. Descriptor strings are static compile-time arrays, not heap allocations leaked on DSO unload. Entry init/deinit are trivial and support repeated matched calls. Instance creation does not call host callbacks; init queries the optional host parameter extension. Factory/extension results remain stable for their required lifetime.

Targeted artifact inspection confirms no desktop, CLI, UI/window, project, engine, audio-device, separation, or media-codec crate. `clap-sys` is the only new registry package; existing versions are retained. Its original MIT/Apache notices are in [plugin-clap notices](../../crates/plugin-clap/assets/licenses). The development bundle retains these and existing DSP notices; a complete production notice inventory remains Phase 5 work.

## Audio and event contract

Only one main stereo input/output and 32-bit planar audio are advertised. Ports have stable IDs 0/1 and reciprocal in-place pairs. There is no sidechain, MIDI/note input, per-note parameter automation, or CLAP modulation bridge. No DAW routing evaluator is introduced.

Activation accepts exactly 44.1/48/88.2/96/176.4/192 kHz, finite and integral, and block bounds `1 <= minimum <= maximum <= INT32_MAX`. It rejects other rates, fractional rates, invalid bounds, and invalid lifecycle transitions without truncation or clamping. Activation prepares DSP outside processing; deactivation/destruction retire it on main. Stop/start retains histories; reset clears the portable DSP while retaining parameter values. Reactivation prepares a new processor with the latest saved base values.

Process walks host-owned buffers and ordered events directly. No copied event vector, fixed event-count cap, block buffering, or second smoother exists. Values take effect before their sample; equal-time events retain host list order, with the last value winning. Existing Drive ramps run once, including bypass and Color changes. Flush consumes values without sample timing, as required by CLAP, on main when inactive and audio when active. The SDK serializes flush/process/reset mutable ownership; the plugin does not acquire a mutex, spinlock, or busy-wait guard in those calls.

Only global core-space parameter-value events are consumed. Foreign namespaces, note-addressed values, unsupported modulation, unknown IDs, malformed parameter bodies, and invalid values are ignored. Cookies are never dereferenced. CLAP stepped values truncate before shared strict validation, as its pinned parameter ABI specifies; VST3 normalized choice rounding remains separate. Sample-order/bounds errors and unreadable event headers return `CLAP_PROCESS_ERROR` without an unbounded retry or copy.

Input and output channels may be in-place, including an aliased audio-buffer header. The adapter uses raw sample pointers rather than aliased Rust slices. It captures constant input samples before writing output, so in-place constant masks do not feed modified sample zero into later frames. Every output sample is written; output constant-mask hints are cleared conservatively.

Unsupported bus/sample layouts, null required buffers, oversized blocks, aliased output channels, and nonfinite input samples return an error; the host must discard that output block. Finite headroom is not clamped or normalized by the adapter. An error after earlier samples/events may retain their valid DSP history; malformed host audio is not a transactional state load. A zero-frame call can flush offset-zero values without audio ports; ordinary stopped edits use the parameter flush extension.

Latency is zero with no adapter delay. Tail metadata is one second at the active rate. Process returns Continue, including quiet input and plugin-owned bypass, so filter histories are not suspended merely because an input block is silent. The host may hard-bypass/suspend processing. No render extension is advertised because offline/realtime modes use identical DSP, as the pinned render extension recommends when quality is unchanged.

## Active/stopped state publication

Main-thread load validates the complete bounded Phase 1 envelope before publication. A single-writer fixed atomic mailbox holds the latest accepted recall, coalescing multiple loads without a queue-full wait or allocation on audio. Inactive loads apply immediately; active loads request processing and rescan parameter values on main. There is no plugin editor/gesture producer yet, so no outgoing UI gestures or dirty-state callback is generated.

At process/flush/reset boundaries, audio attempts to read the mailbox once. If publication overlaps that read, it retains current DSP and retries at the next boundary. After recall, events in that block run in host order and win over the recalled values. A recall published during a block waits for the next boundary and can supersede preceding-block automation. Main-facing parameter reads/save prefer an accepted pending recall until audio acknowledges it.

Audio publishes seven base values plus the acknowledged recall revision through a separate atomic snapshot. Sequentially consistent version/word operations ensure a reader gets one complete generation. Main performs at most eight attempts; an overlapping save/read may return false, never a torn state. Audio performs no spinning, serialization, stream access, or final-owner retirement. Only atomics and fixed arrays cross this boundary. CLAP's prescribed thread serialization is required for each mailbox's single writer and the mutable DSP cell.

State streams use the existing 72-byte envelope, bounded read/write attempts, short stream handling, EOF/error mapping, and complete-candidate validation. Invalid state never replaces the working snapshot. No paths, DAW identities/routes, GUI state, or DSP histories enter host state. SDK save/load runs on main, including while audio is active. Rust panic propagation is contained at C boundaries; expected input/configuration failures return SDK errors rather than panic. The emergency panic path is not a promise of realtime recovery from arbitrary host memory corruption.

## Executed evidence and remaining validator result

The [sanitized snapshot](../validation/drive-plugin-phase2.json) records the local binary hash, compiler/profile, loaded-binary test evidence and callback measurements. Tests call the exported `clap_entry` and all plugin/extension callbacks through the C ABI. The normal `.clap` binary is loaded dynamically, separately from the direct Rust reference; an opt-in allocation-check binary uses the same DSP/adapter source with a per-thread counting allocator. An allocating/freeing positive control inside the loaded diagnostic library confirms that the guard observes both operations. Normal exports omit the test counters. Mach-O inspection shows only system runtime dependencies and no DAW shared library; its development install name still refers to scratch build output and is not a signed/portable release packaging claim.

Coverage includes six rates, four shapes, stereo asymmetry, blocks 1/17/32/64/128/512/4096 and irregular changes, first/sample-17/last/equal-time events, 4,096 ordered points, exact silence, bypass/history, reset/reactivation, constant buffers, aliased buffers/headers, zero-frame flush, malformed layouts/events, invalid configuration, state failure/short streams, instance isolation, and concurrent active recall/save. Direct Drive and exported audio match sample-for-sample for the finite corpus with the same compiler/profile/settings. Exported impulse histories decay below −120 dBFS within the declared one-second policy in the tested corpus; this is not proof for all possible prior histories.

No Rust allocation or deallocation is observed in processing, active flush, reset, dense events, active recall consumption, or rejected audio calls. Instrumentation counts plugin/framework Rust allocation only, not every system/native allocation. Native ABI lifetime checks and these allocator guards do not replace future sanitizer/platform/external-host gates.

Static and per-sample Gain automation measurements cover 32/512 frames and 1/8 instances at 48 kHz, with 1,000 warm-up and 10,000 measured blocks. Both direct reference and loaded normal binary use the same precomputed stereo stream, settings and retarget events. Input preparation, event construction, allocation, and timing-vector growth stay outside timed regions. Median and p99 meet the unchanged [Phase 0 budgets](../../tools/drive_plugin_phase0/budgets.json) in all eight local cases. Serial per-frame direct chains and serial host blocks have different cache/compiler behavior; these measurements establish the local budget gate, not a universal plugin overhead percentage.

Full clap-validator 0.4.1: **32 success, 11 skipped, 1 failed**. `process-varying-sample-rates` requires 8 kHz, then other rates/fractional rates outside the accepted contract; Drive correctly rejects 8 kHz activation. The unmodified validator stops that case at the first rejection. Its success/skip/failure evidence is retained, and the runner separately tests all six supported rates plus unsupported-rate rejection. No unsupported rate is silently enabled to make this test green. Resolving the standard validator's rate expectation versus the selected public rate policy remains a release gate; do not describe this as an unconditional validator pass.

See [README build/check commands](../../README.md#drive-clap-development-processor) and [validation history](../validation/README.md). Windows/Linux/native external DAW sessions, custom editor, clean-machine release packaging and complete release notices remain open. This headless milestone does not complete the first two-format product delivery.
