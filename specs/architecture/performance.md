# Prepared rendering and performance

Implemented after the [2026-10-18 audit](../validation/performance-audit-2026-10-18.md). See the [remediation validation](../validation/performance-remediation-2026-10-18.md) for measurements and remaining native checks.

## Prepared state

`engine::Renderer` prepares the arrangement end, sorted clip ranges, source-buffer bindings, track/effect identity tables, alignment identity tables, and gain/pan/audibility coefficients. Treat its project content as immutable. Publish a new renderer after an arrangement or processing edit; changing its public project fields in place does not rebuild these caches.

A track range cursor retains both active clips and empty gaps. A discontinuity or range boundary uses binary search. Saved clip order does not determine playback order. Ends remain exclusive. Audio and MIDI use the same range scheduler. Bound audio uses the existing prepared-stretch identity checks, source mapping, repeats, fades, and missing-source silence. Muted sources and effects continue processing when their histories require it.

Offline mixing uses prepared track and Master gains and the same DSP as live playback. `RenderPlan::sample_at` remains exact: effects and MIDI require replay from zero. For multiple ranges, prepare one dedicated renderer at frame zero and use `render_offline_at`. Forward ranges reuse history; backward ranges reset and replay. This does not introduce approximate random access or a persistent disk render cache.

## MIDI restart and idle capacity

Preparation builds sorted note events, per-slot note-on indices, and shared-note prefix metadata. Restart finds the latest note assigned to each slot. It reconstructs gate, envelope, Sampler position, note duration, and release age through the existing unit APIs. Shared Retrigger uses the latest on-event; Legato uses the first on-event in the current or last group. The group's last off-event determines release. This is exact because Envelope note-on resets its contour. No restart replays an arbitrary-length earlier event prefix.

Prepared slots retain stable assignment and maximum release capacity. An idle slot keeps its age counters but bypasses layer/control traversal until a note, applicable shared release, or control retarget requires it. Shared continuous providers still tick once per frame. Layer and oscillator telemetry uses prepared identity indices and still aggregates actual active voices.

## Modulation and inheritance

The [shared modulation contract](../modulation/contract.md) owns source bindings. A `ModulationSourceIndex` describes one publication order and generation. Rebuild it when identities or order change; changing values, ranges, weights, or release ages does not require rebuilding it. Pass the corresponding local/shared indices in `ModulationContext`. Routes cache generation-checked slots, including missing sources. Local sources retain precedence. Slice-only callers remain supported and validate cached hits.

Compatible routes retain ramps by stable route/target identity. Small connection lists use a bounded direct match. Larger lists build a process-randomized hash table in scratch links already stored in the current routes. Collision chains compare full identities. Matching preserves bucket links across swaps and retains incoming summation order. The hash seed is prepared with route construction. Matching has expected linear cost, uses no allocation or destruction, and does not reorder saved routes. Track, effect, delay, layer, and oscillator lookups use prepared identity tables. Destinations with no routes receive neutral inputs directly through the same shared entry point. Their processing and offset smoothers still run, including the decay to neutral after routes are removed. Snapshot reads do not advance DSP. Unused fields remain NaN; meter power and disabled-band color rules remain unchanged.

Limiter caches attack/release coefficients until the effective lookahead or release changes. It retains exact coefficient calculations, modulation, history, fixed bypass latency, and ramps. Other units retain their required bypass processing; no coefficient-rate approximation or SIMD replacement is introduced.

## Output preparation and activation

`output::preparation` has one worker, one latest pending request, and one ready result. Immutable plans use `Arc`; renderer construction and its deep plan copy run on the worker. Superseded work is discarded by generation. A full audio command queue retains the ready renderer for retry. The callback retires old storage through the existing bounded queue; a full retirement queue retains the pending replacement.

Initial Play, Seek, and Stop prepare transport and latency pre-roll on the worker. Existing playback continues until a current request is ready. Pause and ordinary resume retain the active pipeline and histories. If preparation is pending, Pause also queues a revision-checked command to stop the current audio on the next callback, without marking the pending plan as applied. The worker still prepares the requested paused position. Stale pause commands cannot stop a later resumed transport. The latest pending transport intent survives a superseding plan or transport request. Activation preserves the 5 ms output transition and original playback-start return position.

A compatible plan inherits the active state. For an incompatible MIDI edit, latency topology, or compensated loop, the callback captures the transport and a monotonic project-sample clock, then returns the replacement for worker preparation. The old renderer keeps playing. The worker reconstructs transport through `TransportCache`, then catches up through the shared `Renderer::next_sample` path. This retains modulation, note scheduling, loop transitions, and latency compensation. No destination-specific reconstruction or callback pre-roll is introduced.

The worker prepares ahead of the rendered sample clock with an initial 100 ms delivery lead. It checks cancellation at project-buffer intervals and publishes only when at least half that lead remains. The callback activates at the exact prepared project-sample clock, including inside converter input blocks. It retains the 5 ms transition and retires the old renderer through the bounded queue. A late result or full retirement queue leaves old playback running; a missed deadline returns the result for reconstruction from a new active anchor, with delivery lead doubled up to one second. Stale revisions cannot activate. Compatible edits retain the direct inheritance path.

The monotonic clock counts generated project samples independently of seeks, loop wraps, and device conversion. It is published once per callback. Explicit seeks keep their requested position and activate when ready; while both the requested and active transport are playing, the UI displays the active audio position. Pending command intent stays separate so subsequent edits and transport actions retain the requested seek. Background edit preparation does not replace transport intent with a stale captured frame.

Worker pre-roll and catch-up run before effect meters are bound, so preparation cannot publish analysis levels into active meters. Exact history reconstruction still takes time, and a worker that cannot catch up postpones activation while current audio continues. Initial device creation remains synchronous.

`engine::TransportCache` retains exact checkpoints before latency priming. Each snapshot owns independent mutable source, envelope, LFO, Macro, route, smoother, filter, delay, and dynamics state; decoded/prepared PCM stays shared. DSP cloning runs only on the worker. `UnitMeter` cloning preserves enable state and detaches publication handles, so snapshots cannot publish into active meters. Cached preparation starts at the closest preceding checkpoint, replays through the existing full renderer, and applies the same latency priming as uncached preparation.

### Idle coverage and exact reuse

The UI starts one device-independent `output::AudioPreparation` worker for a nonempty project. It updates that worker after committed project changes and transfers it to Output on first Play. This retains coverage built before opening an audio device. Once Output exists, its preparation worker continues optional work between requests. Blocking media jobs release idle preparation; it restarts after the job. A failed worker start is retried only after the project revision changes or preparation lifecycle resets; Play can still prepare on demand.

The worker advances an exact cursor through the arrangement in nominal 10 ms work slices, separated by up to 20 ms waits. It checks new requests/shutdown at project-buffer boundaries. A costly block or checkpoint clone can exceed the slice; this is not a real-time deadline. Completed checkpoints survive cancellation. The new request takes priority over optional work. Background coverage is useful only for plans whose note/modulation processing requires prefix history; ordinary plans retain the shared direct preparation path.

Checkpoints have both count and estimated-byte limits: at most 64 entries and 128 MiB. Their spacing is the greater of 250 ms and arrangement duration divided by the available slots. This covers long arrangements with coarser intervals rather than retaining only the latest short window. Slot selection also reserves four renderer equivalents within a 512 MiB preparation allowance for base, warm cursor, active state and candidate. A large renderer can leave zero optional slots. Exact preparation still works without checkpoints. The allowance sizes optional storage; it is not a combined process memory reservation.

The output worker reuses a cache for the same plan allocation or a structurally equal plan with the same prepared sample allocations and rates. Only the requested playhead is excluded from project comparison. Note, source, effect, route, tempo, loop and other project edits invalidate the cache conservatively. Equal sample values in a new allocation do not establish compatibility. Live/paused and stopped reconstruction use different loop rules and invalidate each other's checkpoints. No cache data is saved to the project.

The accepted F05 policy preserves exact history. No timeout switches to approximate processing. A cold or uncovered seek still needs history work; a worker that cannot outrun playback can leave an incompatible edit pending while old audio continues. `PerformanceSnapshot` records completed request-to-activation durations in logarithmic microsecond buckets and an exact maximum, including worker and delivery time. It also reports warm coverage and frames replayed by the last cached preparation. Cancelled/uncompleted requests are not duration samples. Native end-to-end percentiles under load remain a separate measurement task.

The direct engine `seek`, `play`, and incompatible `inherit` APIs remain synchronous for offline/control-thread callers. Native output uses prepared transport activation and does not call those expensive paths. MIDI loop reconstruction uses the prepared note indices. Device creation still runs in the native Play path; its duration needs a device-specific check.

### Cold seek cost

Release builds use ThinLTO and one code-generation unit to optimize calls across the shared DSP crates. Debug builds keep their existing profile. This increases release compilation/link work; it does not enable fast math or change processing precision.

Saturator uses mirrored FIR history rings to remove per-tap wrapping while keeping coefficient order, arithmetic order, oversampling, latency, and reset behavior. EQ caches parameter conversion and filter configuration while base values and smoothed modulation offsets remain unchanged. It still advances ramps and modulation smoothing and processes every filter and meter each sample. Changed values and reset invalidate the configuration cache. Neither optimization substitutes approximate history or changes note-edit scheduling.

These changes reduce full reconstruction cost, including the first uncached seek. Delay still scales with the requested history and project complexity; they do not make every cold seek immediate. Idle coverage and strict plan reuse reduce repeated reconstruction work without changing the exact processing reference.

## UI work

Waveform painting returns before peak lookup when the clipped body is empty. Timeline skips offscreen fixed-height track controls with the shared row placeholder, preserving panel/lane bounds for scrolling and drops. Visible rows retain owned editing snapshots; active drags, marquee selection, name editing, and keyboard editing keep normal row construction. This preserves captured interactions instead of virtualizing their controls away.

Idle UI no longer requests unconditional 16 ms frames. Playback, pending output preparation, jobs, and pending file previews keep polling active. A 250 ms grace period covers final transitions. Output worker completion also wakes egui. Input events and existing interaction repaint requests retain their normal behavior.

Project edits use one staged clone through `Project::edited`. Final validation builds one asset-ID lookup and validates the complete proposed transaction before publication.

## Media admission and cancellation

Decoded/prepared PCM, peaks, and known temporary buffers share a 2 GiB preparation budget. A weak-owner registry counts retained buffers, including those held by retired renderers, without keeping them alive. Reservations include current owners and concurrent jobs. Arithmetic is checked and final PCM reservation is fallible. Sampler region selection, decoding, rate conversion, and stretch adapters participate. A failed or cancelled staged operation leaves the current session intact.

This is not an operating-system memory cap. Native encoder/stretch allocations, DSP state, UI/GPU resources, allocator overhead, and unrelated process memory are outside the PCM registry. FFT setup has a conservative rational-rate scratch estimate; pathological input rates are rejected before converter construction. The Integrated stretch engine retains its own resource checks.

Decoded audio uses a weak cache keyed by canonical source path, processing rate, file length, and modification time. Independent assets retain independent IDs while sharing immutable PCM/peaks. Replaced files invalidate the cache; a source changed during decoding is rejected. The cache does not detect a deliberate same-length edit with a restored timestamp. It does not retain unused PCM or provide disk-backed streaming.

File previews use two workers and at most 64 waiting requests. Dropping a hover cancels its work; only its owner can consume the result. A full queue reports a retryable error. Decode, resampling, and peak generation check cancellation between bounded work chunks. Offline conversion reuses planar input/output buffers and writes only the exact delay-compensated output range.

## Diagnostics and acceptance

`output::Status::performance.snapshot()` exposes cumulative callback count, log2 duration buckets, maximum callback and command-processing durations, maximum ready-result wait, nominal device-period overruns, and device failures. Callback code only reads the clock and writes preallocated atomics. It does not format or log diagnostics. A snapshot can span concurrent publications; it is diagnostic telemetry, not a transaction.

The overrun budget uses actual callback frame count and device rate. An overrun is not a measured hardware xrun. Ready-result wait excludes renderer construction. Device failure counts reflect CPAL error callbacks; a backend that does not report an xrun cannot supply an xrun count.

Run `tools/performance_audit.py --compare <baseline.json>` for serial release comparisons. An optional `--max-regression <ratio>` gate is intended for controlled hardware; normal functional CI remains independent of timing noise. Results record the source-tree, harness, runner, and lockfile hashes. Native callback/presentation, queue bursts, physical device failure, idle power, and parallel-job resident memory still require controlled hardware runs. Do not claim those results from headless tests.

## Estimated memory policy

Accounting stays on the control/worker threads. `unit-core::memory` supplies capacity-based container/settings estimates; Core accounts for project models; portable units report owned heap storage; Engine separates model, schedules, DSP and shared audio. `EffectUnit::heap_bytes` defaults to zero for inline-only processors. Units with dynamic storage must override it. New model/processor storage must extend its owner's estimate. Hash/tree containers include spare-storage allowances. These are estimates, not allocator or resident-set measurements.

| Owner | Policy | Release/failure behavior |
| --- | --- | --- |
| Shared PCM, peaks and known media scratch | Existing 2 GiB `AudioReservation` admission | Shared allocations count once; cancellation/final owner release returns reservations |
| Transport checkpoints | 64 entries, estimated 128 MiB; working-state allowance described above | Old entries evict first; zero slots disables optional coverage; worker shutdown releases base/cursor/checkpoints |
| Output renderers | Estimated 512 MiB across active, candidate, queued and retired `PreparedRenderer` owners | Atomic leases follow owners until retirement. Failed admission releases the candidate/cache and reports an error; current playback continues |
| Undo/redo | 100 undo entries and estimated 128 MiB of retained models plus audio exclusive to history | Remove oldest undo entries, then farthest redo states if required; show a visible notice. Current/pending/saved states remain intact |

Before a blocking job starts, UI releases device-independent idle preparation. If shared audio usage is within 256 MiB of its limit, it also releases optional undo/redo states. This is a pressure trigger, not a guaranteed 256 MiB reservation: essential/current/audio-output owners can still retain samples. History stores per-snapshot model estimates and refreshes deduplicated audio totals on edits, not every paint. See [history policy](../editing/undo-redo.md).

Hovering the processing-rate/buffer status label shows live session metadata, history metadata and exclusive audio, shared media/scratch, base renderer model/schedule/DSP, checkpoints, warm cursor, and output renderer owners. History-exclusive audio is a subset of shared media, so do not add both categories when estimating a total. Values are independent snapshots, not a transactional memory census.

The output lease is acquired after constructing the candidate; it bounds retained estimated renderer state, not transient construction peaks. Meter handles, allocator fragmentation, native-library storage, GPU allocations, job/save/clipboard metadata and current project growth are not all covered by these budgets. Essential base/current state can exceed optional budgets. No strict process RSS or track-count limit is claimed. Callback publication, timing counters and lease retirement use atomics; accounting traversals, snapshot cloning and final buffer destruction stay outside the callback.
