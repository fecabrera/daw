# Findings and options

[Review index](README.md) · Baseline `2dbb347`

These are review findings, not changes made by this task. “Confirmed” below means confirmed in code unless a measurement is named. It does not mean an audible/native failure was reproduced. Relative effort includes regression work, not a delivery estimate.

## Priority register

| ID | Priority | Evidence class | Topic | Effort |
| --- | --- | --- | --- | --- |
| F01 | P1 | Code mismatch | Effect and Main clock latency | S–M |
| F02 | P1 | Code; scaling risk | Unused asset retention | M |
| F03 | P1 | Code; latency risk | UI-dependent audio handoff/retirement | M–L |
| F04 | P2 | Code; scaling risk | Broad clones/rebuilds for small edits | M |
| F05 | P1 | Historical measurement + code | Cold seek and catch-up cost | M–L |
| F06 | P1 | Code; resource risk | Incomplete total-memory policy | M–L |
| F07 | P1 | Code; crash-risk analysis | Save publication/durability | M |
| F08 | P1 | Code; race analysis | Export no-overwrite race | S–M |
| F09 | P2 | Code | Uneven job cancellation and scheduling | M |
| F10 | P2 | Code + historical measurement | Synchronous output startup | M |
| F11 | P2 | Code; maintenance risk | Duplicate live/offline mix traversal | M |
| F12 | P2 | Code; maintenance risk | Multiple host type switches | M |
| F13 | P2 | Code + historical profile | Single-level waveform peaks | M |
| F14 | P2 | Assurance gap | Missing end-to-end latency evidence | S–M |
| F15 | P2 | Code + documented decision | Nonuniform seek-history semantics | M–L |
| F16 | P3 | Code; product decision | No export-tail extension | M |
| F17 | P2 | Documentation mismatch | Current design mixed with old proposals | S |

## F01 Effect-clock latency is incomplete

**Evidence.** [`latency::chain_frames`](../../../crates/engine/src/latency.rs) counts both Limiter and Saturator. Saturator declares 32 frames through [`LATENCY_FRAMES`](../../../crates/saturator/src/lib.rs) and its [FIR constants](../../../crates/saturator/src/oversampling.rs). But [`EffectChain::sample`](../../../crates/engine/src/effects.rs) subtracts latency from the frame passed to later effects only for `RuntimeEffect::Limiter`. Auto-filter uses that frame for its internal LFO. Separately, [`MidiTrackRuntime::voice_sample`](../../../crates/engine/src/midi.rs) passes `frame.saturating_sub(parallel_latency)` to Main effects rather than the loop-aware delayed-frame helper used elsewhere.

**Impact.** Saturator → Auto-filter receives a control clock 32 frames ahead of the delayed audio (about 0.667 ms at 48 kHz), per upstream Saturator. This is a confirmed clock-propagation omission; audible significance depends on modulation speed and chain. Near a nonzero loop start, Main subtraction can produce a frame outside the loop instead of wrapping. Audio alignment alone does not prove control alignment.

**Suggestion.** Prepare/query latency through one common runtime-effect accessor and use the existing loop-aware delayed-frame helper at every serial/bus boundary. Audit cached shared-source timing separately; subtracting the clock does not by itself delay published provider values.

**Pros:** closes a current correctness gap; future latency effects use the same path. **Cons:** a correction can change rendered modulation phase in saved projects; needs loop, bypass and chained-effect tests. **Expected impact:** correctness, not a promised CPU speedup. **Acceptance:** Saturator/each latency effect before a clock-sensitive effect, multiple latency effects, layer→Main→track chains, nonzero loop start, bypass, live/export and seek checks against an explicit delayed-clock reference.

## F02 Unused assets accumulate

**Evidence.** [`Project::replace_midi_pattern`](../../../crates/core/src/midi.rs) appends a new asset for every changed pattern and switches the edited clip ID reference. Resize paths also append assets. DeleteClip/DeleteTrack remove clips/tracks without pruning assets in [`apply_edit`](../../../crates/core/src/lib.rs). No current reachability cleanup was found in the inspected Core/Session/UI paths. [`Session::open`](../../../crates/project/src/lib.rs) visits all saved external assets, and Save serializes the complete asset list.

**Impact.** Repeated edits grow the current model, not just Undo. More assets increase validation, clone, history, save and open work. Removed external media can remain decoded/referenced; a damaged unused source can still fail a later open. The bounded undo count does not bound this live-project growth. Save also finds each original asset by a linear scan inside an asset loop, adding quadratic lookup work as the list grows.

**Suggestion.** Add a shared Core reachability calculation from clips and Sampler inputs. Prune unreachable assets in committed candidate sessions and corresponding prepared maps. Each retained undo/clipboard snapshot must keep its own required assets/buffers. Save should use an ID index or paired traversal instead of rescanning.

**Pros:** bounds edit-history debris in the current project and avoids unrelated decode work. **Cons:** must preserve shared sources, Sampler references, clipboard snapshots, undo/redo and Save As; do not delete external files. **Expected impact:** smaller manifests/metadata and less retained PCM; benefit grows with edit count. **Acceptance:** thousands of pattern replacements keep current reachable asset count stable; split/shared clips, Sampler inputs, clipboard, undo and reopen still restore exact content. Measure model/history/save time and retained bytes before/after.

## F03 Audio handoff depends on UI progress

**Evidence.** [`DawUi::show`](../../../crates/ui/src/lib.rs) calls [`AudioOutput::drain`](../../../crates/output/src/lib.rs). Drain forwards ready renderers to the command queue, returns incompatible replacements to the worker, and drops retired owners. The worker requests a UI repaint but does not directly publish to the callback. Full retirement capacity postpones activation.

**Impact.** A ready seek/edit can wait for the next UI frame. A blocked dialog/frame can delay publication or reprepare and retain old DSP/PCM. Destroying a large retired renderer or history state on the UI can itself add frame work. Backpressure keeps the callback safe, but does not bound input-to-audio delay. This is a code-supported risk; no new blocked-UI reproduction was run.

**Suggestion.** Give one dedicated control coordinator ownership of publication and retirement. UI sends intents and reads status. Preserve single-producer/single-consumer queue ownership; do not add another producer to the current rtrb queue. A smaller first step is measuring drain/drop time and moving retired destruction off the UI.

**Pros:** separates audio progress from painting and heavy destructors. **Cons:** ownership migration, shutdown/device lifecycle and meter binding become more complex; must avoid locks/final drops on callback. **Expected impact:** lower worst-case handoff delay, especially under UI load. **Acceptance:** intentionally stall UI while preparation completes; current audio and new activation proceed, retired bytes remain bounded, stale revisions never activate, callback allocator/lock checks remain clean.

## F04 Small edits still copy and rebuild broad state

**Evidence.** Core `edited` clones and validates the full Project. [`SessionHistory::record`](../../../crates/project/src/history.rs) snapshots Session on each recorded live update, even inside a gesture. [`Session::plan`](../../../crates/project/src/lib.rs) clones model/maps. The UI's `sync_needed` block creates this plan before checking for Output. Saved collapse/order and other presentation changes share owner-edit hooks. The output cache invalidates on a new plan allocation.

**Impact.** A small control or layout change can pay model/history/plan construction costs and invalidate warm seek state. Latest-request worker coalescing limits pending work but does not undo earlier UI clones. This can matter more after F02 growth. No current large-project per-gesture timing was collected for this review.

**Suggestion.** Classify edit effects: presentation/metadata, parameter, schedule/source, topology, processing-rate. First skip audio synchronization for proven presentation-only changes and avoid building a plan without Output. Then consider shared immutable model subtrees and bounded parameter updates for compatible controls. Cache compatibility must use processing identity, not guessed equality of audible output.

**Pros:** less UI work and unnecessary reconstruction; preserves warm caches for harmless changes. **Cons:** incomplete classification can leave audio stale; parameter messages need ordering with structural edits and undo. **Expected impact:** improved edit responsiveness and cache hit rate; no quantified speedup yet. **Acceptance:** every edit category has an audio-invalidation test; UI-only edits leave audio generation unchanged; parameter updates preserve ramps/history; structural edits and undo still rebuild required state.

## F05 Cold history replay can still take a long time

**Evidence.** [`TransportCache::prepare`](../../../crates/engine/src/transport_cache.rs) replays from its nearest preceding checkpoint, or zero on a cold history request. The [recorded cold-seek optimization](../../validation/README.md#cold-seek-optimization) reduced a private workload's preparation from about 1.06 s to 0.92 s at the longest measured position. Those are historical worker timings, not new end-to-end measurements. The [worker catch-up loop](../../../crates/output/src/preparation.rs) must get ahead of the active generated-sample clock before publication.

**Impact.** A first or distant seek still scales with elapsed history and DSP complexity. New plans discard checkpoints. If preparation throughput cannot exceed real time under load, an incompatible edit can remain pending while old audio continues. No completion deadline guarantees progress in that case.

**Options.** (A) Byte-budgeted background checkpoint coverage at project load/idle; (B) retain checkpoints only across edits proven not to alter prior processing; (C) define an explicit quick-seek mode with bounded/approximate history, if product requirements allow it. Keep exact mode as the reference. Optimize measured DSP hot spots before adding indiscriminate parallel work.

**Pros:** A improves cold coverage; B reduces needless invalidation; C can bound response time. **Cons:** A spends CPU/memory before use; B needs precise invalidation; C changes audible state and must be disclosed. **Expected impact:** lower cold/uncached latency, with explicit resource or fidelity tradeoffs. **Acceptance:** dense and long projects, first/backward/distant seeks, edit bursts, loop positions and saturated worker CPU; record request-to-activation p50/p95/max and exact-output comparisons for exact mode. Decide a pending-edit timeout/fallback policy.

## F06 Memory limits do not cover total retention

**Evidence.** [`AudioReservation`](../../../crates/media/src/memory.rs) covers PCM/peaks and known scratch at 2 GiB. It excludes model/DSP/native/GPU allocations. TransportCache keeps up to 64 Renderer clones by count. History holds up to 100 undo snapshots plus current/pending/redo/saved data. Queues, jobs and save snapshots retain further owners. All are visible in the [cache inventory](preparation-and-caches.md#cache-and-retained-state-inventory).

**Impact.** Many layers, large latency buffers, dense patterns or wide Sampler banks can consume substantial memory outside the nominal audio budget. Retained history can prevent new admission even after visible deletion. A count limit is predictable in entries, not bytes. The previous private-project checkpoint RSS result is not a general bound.

**Suggestion.** Add accounting for model/schedule/DSP checkpoints and expose retained bytes by category. Give checkpoint/history policies byte and count bounds; evict optional cache state before rejecting essential preparation. Define whether pressure may shorten undo history, with clear user-visible behavior. Keep a reserve for active+candidate renderer coexistence.

**Pros:** predictable failure, useful diagnostics, fewer avoidable admission failures. **Cons:** estimating native/DSP allocations needs conservative accounting; eviction may slow seeks or reduce undo depth; no portable estimate is a strict RSS cap. **Expected impact:** robustness and scalability. **Acceptance:** concurrent preview/export/seek/settings work, long sources, large pitch banks, history retention and full retire queues; track peak RSS and categorized ownership. Verify cancellation releases reservations and optional owners.

## F07 Save publication is not fully crash-durable

**Evidence.** [`Session::save` and `replace_file`](../../../crates/project/src/lib.rs) write and sync a temporary manifest. POSIX uses rename without syncing the parent directory. Windows renames the old target to a backup, then renames the temp into place, and removes the backup on success. Failure rollback ignores a rollback error. There is no corresponding automatic backup recovery path in `Session::open`.

**Impact.** Regular error handling is transactional, but power loss/process interruption has stronger requirements. Windows has a window where `project.json` is absent; a leftover backup may need manual recovery. POSIX rename visibility and durable directory persistence are different guarantees. No crash fault-injection or Windows-native reproduction was run here.

**Suggestion.** Centralize atomic file publication with platform-specific replacement, durability policy and recovery. Use an appropriate Windows replacement API, preserve recoverable backup state, and sync destination directories where supported. Open should detect and report a recoverable interrupted save.

**Pros:** stronger project safety and one implementation for all callers. **Cons:** platform API/error complexity; directory sync can add latency or behave differently on network/removable filesystems. **Expected impact:** data resilience, not steady playback speed. **Acceptance:** fault injection at write/sync/replace/cleanup boundaries, existing and first save, disk-full/permission failures, and native Windows/macOS/Linux tests. Define the guarantee for removable/network storage.

## F08 Export no-overwrite has a check-then-rename race

**Evidence.** [`Session::export_with_progress`](../../../crates/project/src/lib.rs) checks target existence initially and again before `replace_file`. The check and rename are separate. On platforms where rename replaces an existing destination, another writer can create the path between them. The final check therefore does not implement atomic “create only if absent.”

**Impact.** An export requested without overwrite can replace a file created concurrently in that narrow interval. This is a code-level race analysis, not an observed user incident. Source/manifest path checks reduce ordinary mistakes but do not remove this publication race.

**Suggestion.** Separate replace-authorized publication from atomic no-replace publication in the shared file helper. Use platform-supported no-replace semantics or an equivalent safe publish operation; keep output on the destination filesystem. The existing [stretch CLI `publish`](../../../crates/timestretch-cli/src/main.rs) already distinguishes `persist` from `persist_noclobber`; assess reuse of that approach instead of inventing another check-then-rename helper.

**Pros:** honors an explicit user guarantee. **Cons:** cross-platform primitives and fallback behavior need care; an unsafe fallback must fail, not silently replace. **Expected impact:** prevent rare unintended replacement. **Acceptance:** a competing writer creates the target immediately before publish; no-overwrite fails and retains that file on each platform; explicit overwrite still works.

## F09 Background job cancellation is inconsistent

**Evidence.** General [`run_job`](../../../crates/ui/src/lib.rs) starts a thread and one result receiver. Sampler/settings wire cancellation tokens. [`finish_clip_stretch`](../../../crates/ui/src/stretch.rs) uses the noncancellable `prepare_stretches()` wrapper; Open and Export public paths also lack a cancel parameter. File previews and audio preparation have separate bounded scheduling/cancellation models.

**Impact.** “Background” keeps processing off the UI but can still block editing with no Cancel for a long transform/open/export. Dropping a receiver prevents publication, not computation, unless work observes cancellation. Separate queues can compete for CPU and memory; decoded cache misses are not coalesced across requests.

**Suggestion.** Reuse a small job context for cancellation, progress and generation/owner identity across existing workers. Thread cancellation through Timeline stretch/open/export. Add workload priorities/admission only after measurement, favoring audio activation over speculative preview/warming. Coalesce same-source decode misses if concurrent memory/CPU measurements justify it.

**Pros:** consistent user control and less stale work. **Cons:** native calls may only cancel between calls; failure cleanup/publication races need tests; a large generic executor would add unnecessary scope. **Expected impact:** faster cancellation and more predictable contention. **Acceptance:** cancel each stage, submit a replacement, close the project, and verify no stale result/temp files/retained reservation. Measure cancel latency and concurrent-work behavior.

## F10 Output startup remains synchronous

**Evidence.** [`DawUi::play`](../../../crates/ui/src/lib.rs) calls [`AudioOutput::new`](../../../crates/output/src/lib.rs), which chooses/configures CPAL, constructs initial runtime/conversion/queues and starts the stream. [Previous native investigation](../../validation/README.md) recorded about 91–94 ms for device opening on one machine.

**Impact.** First Play can delay a UI frame independently of seek history. Device-specific initialization and large initial renderer construction can add variability. This is not evidence that all current Play actions take that long.

**Suggestion.** Prepare non-device runtime work asynchronously and represent output initialization as explicit state. If a platform allows it, initialize the device on its permitted control thread or pre-open after project load. Preserve cancellation/error recovery and avoid opening hardware unnecessarily for headless/edit-only use.

**Pros:** shorter first-Play frame stalls. **Cons:** platform thread affinity, device changes and lifecycle races; pre-open may consume power or hold exclusive resources. **Expected impact:** responsiveness at first playback. **Acceptance:** cold/warm device start, unavailable device, repeated Play/Pause during initialization and device change on all supported platforms; measure UI and audible start separately.

## F11 Live and offline mix loops can drift

**Evidence.** [`Renderer::next_sample` and `offline_frame`](../../../crates/engine/src/lib.rs) separately traverse tracks/effects/alignment/mixing, with valid live-specific ramp/meter/transport differences. [`MidiTrackRuntime::voice_sample`](../../../crates/engine/src/midi.rs) also retains distinct bus-effects and no-bus-effects accumulation paths.

**Impact.** A future routing/latency/modulation fix can reach one traversal and miss the other. Existing parity tests reduce this risk; the review does not assert that the current whole mix is incorrect.

**Suggestion.** Extract a narrow shared per-frame signal-routing operation with explicit live/offline control inputs. Keep transport and meter policy outside it. Add parity coverage before refactoring; do not replace the engine with a new graph abstraction just to deduplicate loops.

**Pros:** fewer routing integration points. **Cons:** careless abstraction can add per-sample branches or alter arithmetic order and legacy sound. **Expected impact:** maintainability and consistency; CPU effect must be measured. **Acceptance:** all unit types, layer/Main/track/Master placement, mono/stereo pan, mute/solo, loops and latency; sample equality where semantics match, explicit tolerance/reason where they do not.

## F12 Host integration still has separate type switches

**Evidence.** [Catalog registration](../../../crates/ui/src/effects_catalog.rs) drives availability/additions, but saved [EffectSettings](../../../crates/unit-core/src/effects.rs), [RuntimeEffect](../../../crates/engine/src/effects.rs), [latency](../../../crates/engine/src/latency.rs), [meter bindings](../../../crates/output/src/lib.rs) and panel dispatch each know types. F01 is a concrete example of incomplete integration. Component registration likewise automates drop eligibility, not construction of every engine lifecycle path.

**Impact.** New units can appear and drag correctly while missing latency, telemetry, copy/restore or processing behavior. Closed enums provide useful exhaustiveness but `if let`/default branches can silently omit a new type.

**Suggestion.** Centralize runtime-effect operations and latency/monitor metadata behind focused accessors or generated exhaustive adapters. Add a unit-integration contract matrix/test that enumerates registered types. Keep portable metadata free of UI/device types.

**Pros:** safer extension with less repetitive host glue. **Cons:** code generation or macro indirection can hurt readability; a universal dynamic interface is not required. **Expected impact:** lower integration defect risk and review effort. **Acceptance:** a test effect with nonzero latency verifies catalog, serialization, prepare/process/reset/inherit/clone, every bus, meters, modulation, duplication and missing-type behavior.

## F13 Single-level waveform peaks still scale with duration

**Evidence.** [`waveforms::source_extrema`](../../../crates/ui/src/waveforms.rs) walks 256-frame peak blocks and exact partial edges for each pixel range. Offscreen early return is already fixed. A [previous native profile](../../validation/README.md) found repeated extrema scans prominent in Timeline CPU; it did not establish an end-to-end freeze.

**Impact.** Wide zoom on long visible audio can still perform many peak reads per repaint. Loop mapping adds work. Instrument/widget and GPU cost are separate and would not be fixed by waveform caching.

**Suggestion.** Build a multilevel min/max pyramid on the media worker; query the coarsest exact covering blocks plus edges. Optionally cache projected waveform geometry by source revision/range/zoom/theme if profiling still supports it.

**Pros:** query cost tied more closely to pixels than source duration. **Cons:** more peak memory/preparation; cache keys must include mapping/trim/repeat/stretch and avoid stale colors. **Expected impact:** Timeline CPU at wide zoom, not audio DSP. **Acceptance:** compare extrema against raw samples at arbitrary boundaries, repeats and shortened sources; benchmark visible long sources and GPU-inclusive native frames.

## F14 End-to-end latency and pressure evidence is missing

**Evidence.** [`Performance`](../../../crates/output/src/performance.rs) records callback/command duration, overruns, ready wait and device failures. It does not connect input arrival, plan clone, worker start, reconstruction, UI dispatch, activation and audible presentation under one request ID. [Prior validation](../../validation/README.md) explicitly distinguishes worker/UI timing from audible latency and hardware xruns.

**Impact.** A slow seek can be attributed to the wrong stage. Good averages and allocation-free callbacks do not prove deadlines under a worst-case graph, window stall or concurrent preparation.

**Suggestion.** Add bounded per-request stage timestamps/counters outside callback and preallocated callback activation events. Expose a diagnostic snapshot/export with queue occupancy, stale/retry counts, cache hit/prefix distance, bytes and preparation throughput. Keep formatting/logging off callback.

**Pros:** identifies the limiting stage and tests improvements objectively. **Cons:** measurement overhead and trace retention must be bounded; callback duration is not hardware latency. **Expected impact:** engineering confidence and faster diagnosis. **Acceptance:** run the [measurement matrix](validation.md#proposed-validation-matrix) with input-to-display and input-to-audio instrumentation; calibrate instrumentation overhead.

## F15 Transport history semantics are not uniform

**Evidence.** [`envelope_history`](../../../crates/engine/src/midi.rs) becomes true for assigned, enabled, nonzero Envelope input routes. This drives full replay/checkpoints in transport preparation. Otherwise [`prepare_transport_cancellable`](../../../crates/engine/src/lib.rs) resets processing and primes latency without replaying the entire prefix. The [LFO specification](../../instruments/lfo.md#depth-and-rate-modulation) documents resetting accumulated Rate phase on direct discontinuities; uninterrupted export retains full history.

**Impact.** A seek can produce a different state from uninterrupted playback, particularly integrated LFO Rate/filter/dynamics history. Enabling Envelope modulation can also switch the whole plan into a more expensive history path. This is a current documented tradeoff, not proof of a new regression.

**Decision and suggestion.** Define whether seeks promise exact history, reconstructed notes with reset effects, or a selectable quality/latency policy. Express history requirements through shared unit/runtime capabilities rather than one Envelope-specific host predicate. Extend checkpoints only where the declared policy requires them.

**Pros:** predictable sound and performance contracts for future stateful units. **Cons:** exact arbitrary history can be expensive; a fast mode can audibly differ. **Expected impact:** consistency and clearer user expectations, potentially higher cold-seek cost for newly exact units. **Acceptance:** unmodulated/modulated LFO, Envelope, dynamics and combinations at the same destination frame; document and test each mode against uninterrupted export.

## F16 Export stops at the arrangement end

**Evidence.** [`export_with_progress`](../../../crates/project/src/lib.rs) renders `0..Project::end()`. It does not append effect-tail time or silence-until-idle processing. Current latency pre-roll is separate from tail extension.

**Impact.** Filter/dynamics ringing after the final clip is omitted. Future reverb/delay would make this more visible. This may be acceptable arrangement-length behavior; it needs a product decision before adding long-tail effects.

**Suggestion.** Keep an explicit arrangement-only option and add either a fixed tail duration or a bounded unit-tail/energy policy. Model tail reporting separately from algorithm latency.

**Pros:** supports complete decays and future effects. **Cons:** changes output length, progress and export comparisons; energy detection needs a hard cap and deterministic rule. **Expected impact:** export completeness, with longer processing/files. **Acceptance:** impulse near final clip end, long/silent tail, feedback cap, float/integer/MP3 output and exact progress completion.

## F17 Current design is mixed with old proposals

**Evidence.** Early sections of [modular audio units](../modular-audio-units.md) describe a future Editor, initially monophonic MIDI and the old Units view, while later dated sections and current code implement Editor, polyphony, Instrument and per-layer effects. The file's status date predates later entries. [Iteration 1](../iteration-1.md) and the validation log are long chronological records. Some guidance lists only an older subset of effects.

**Impact.** A reader can mistake an old baseline/proposal for current behavior, or incorrectly “fix” a deliberate later change. This review's maps are a current snapshot, not a replacement for the historical decision record.

**Suggestion.** Maintain one short current-state architecture map and a capability/decision index. Keep older requirements as dated history with explicit superseded pointers. Update the current map whenever a unit or lifecycle boundary changes. This handoff supplies the first snapshot and index link.

**Pros:** clearer implementation/review authority. **Cons:** another maintained artifact unless updates are part of change review. **Expected impact:** fewer design misunderstandings. **Acceptance:** new contributor can locate the current signal path, ownership, supported units and pending features without reconciling dated appendices; link checks run in documentation validation.

## Recommended sequence

1. Correct and test F01; add the narrow integration check from F12.
2. Instrument stage timing and memory ownership (F14/F06). Address F02 independently; it does not require a new renderer architecture.
3. Fix no-replace publication and design save recovery (F08/F07).
4. Classify edits and remove unnecessary plans first (F04), then evaluate control-thread ownership (F03/F10).
5. Decide exact/fast transport semantics before expanding history caches (F05/F15).
6. Add cancellation consistency and measured UI cache improvements (F09/F13).
7. Refactor shared mix traversal under parity tests and decide export tails (F11/F16). Keep the current architecture index maintained (F17).

Defer a general plugin graph, multithreaded callback mixer, approximate modulation rate, or a full UI rewrite until measurements identify a need. They would expand the correctness surface beyond the issues demonstrated here.
