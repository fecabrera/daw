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

Compatible routes retain ramps by stable route/target identity. Small connection lists use a bounded direct match. Larger lists build a process-randomized hash table in scratch links already stored in the current routes. Collision chains compare full identities. Matching preserves bucket links across swaps and retains incoming summation order. The hash seed is prepared with route construction. Matching has expected linear cost, uses no allocation or destruction, and does not reorder saved routes. Track, effect, delay, layer, and oscillator lookups use prepared identity tables. Snapshot reads do not advance DSP. Unused fields remain NaN; meter power and disabled-band color rules remain unchanged.

Limiter caches attack/release coefficients until the effective lookahead or release changes. It retains exact coefficient calculations, modulation, history, fixed bypass latency, and ramps. Other units retain their required bypass processing; no coefficient-rate approximation or SIMD replacement is introduced.

## Output preparation and activation

`output::preparation` has one worker, one latest pending request, and one ready result. Immutable plans use `Arc`; renderer construction and its deep plan copy run on the worker. Superseded work is discarded by generation. A full audio command queue retains the ready renderer for retry. The callback retires old storage through the existing bounded queue; a full retirement queue retains the pending replacement.

Initial Play, Seek, and Stop prepare transport and latency pre-roll on the worker. Existing playback continues until a current request is ready. Pause and ordinary resume retain the active pipeline and histories. The latest pending transport intent survives a superseding plan or transport request. Activation preserves the 5 ms output transition and original playback-start return position.

A compatible plan inherits the active state. If latency topology or the compensated loop changes, the callback captures the exact block-boundary transport, pauses the old renderer, and returns the replacement for worker pre-roll. The old renderer produces its pause fade while preparation runs. The prepared result resumes at the captured position. Callback suspension metadata survives superseded requests, so a newer compatible plan cannot accidentally inherit the temporary pause. Worker pre-roll uses unbound effect meters and cannot publish its analysis levels into active meters.

The direct engine `seek`, `play`, and incompatible `inherit` APIs remain synchronous for offline/control-thread callers. Native output uses prepared transport activation and does not call those expensive paths. MIDI loop reconstruction uses the prepared note indices. Device creation still runs in the native Play path; its duration needs a device-specific check.

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
