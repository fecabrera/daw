# Saved audio caches and background preparation

Status: accepted direction; implementation pending. All stages below are planned.

Date: 2026-10-30.

## Goal

Reduce project-open and MIDI-edit delays without changing the selected pitch/stretch engine or the resulting sound. Reuse saved preparation results, open the editor before audio is ready, and prepare new MIDI pitches without blocking editing.

This plan extends [prepared rendering](performance.md), [Sampler preparation](../instruments/sampler.md), and [session history](../editing/undo-redo.md). Their existing implementation remains the baseline until the relevant stage lands. In particular, persistent transformed audio and editable models without complete audio maps are new requirements, not existing capabilities.

## Findings and limits

An initial headless debug probe used a project with a 58-second Sampler region at 48 kHz and three chord pitches. Opening took 27.0 seconds; opening a copy without the Sampler took 5.1 seconds. A velocity-only edit took 0.2 ms; adding one uncached pitch took 7.75 seconds. Renderer construction took about 42 ms. These are single-run diagnostic measurements, not release or native UI benchmarks. Do not check private project data, source paths, or recordings into the repository.

- [Project preparation](../../crates/project/src/sampler.rs) gathers root and referenced pattern pitches, including hidden notes, then prepares missing variants sequentially.
- [PreparedSampler](../../crates/media/src/sampler.rs) processes the full selected region for each missing pitch. A short note does not reduce that work. Rebuilding a bank retains only requested variants, so earlier pitches can be lost from the current bank.
- [Decoded-source caching](../../crates/media/src/cache.rs) reuses live process-local buffers. Prepared pitches have no persistent store.
- [UI audio edits](../../crates/ui/src/audio_edits.rs) currently wait for complete preparation before publishing the edited project.

Saved caches remove repeated work. Background preparation removes the editor's dependence on completion. Neither makes a first-time uncached pitch instantly playable. Required decoding, preparation, and exact renderer reconstruction still take time.

## Scope and invariants

- Persist decoded/resampled PCM, waveform peaks, and exact prepared Sampler pitch/duration results in a disposable application cache outside project folders. Reuse transform infrastructure where appropriate; do not add saved DSP seek checkpoints in this scope.
- Keep manifests and original media authoritative. Cache files are neither project assets nor recordings, and [Collect project sources](../projects/source-bundling.md) must exclude them.
- Preserve pitch tracking, root-note mapping, duration, linked stereo, selected engine/revisions, source mapping, hidden-note coverage, and current live/export behavior. No new pitch-tracking option, approximate saved audio, note-length truncation, or silent backend fallback.
- Cache misses, corruption, eviction, and write failure must not change sound or make an otherwise valid project unusable. Source/processing failures remain real errors. A cache must not conceal a missing or changed original source.
- All hashing, decoding, filesystem work, transforms, cache bookkeeping, and final-owner destruction run outside the audio callback. Preserve bounded queues, atomic status, memory admission, and existing renderer activation/retirement.
- Prepare complete immutable audio banks before renderer publication. Never substitute silence merely because a required pitch is still preparing. Preserve the existing explicit missing-source warning/silence policy.
- Keep source importing, sample replacement, processing-rate changes, and source collection on their existing transactional paths initially. This plan first makes project opening and validated MIDI-pattern edits nonblocking; do not remove all job guards globally.

## State and user behavior

Separate the **editor revision** (the latest validated project shown and saved) from the **prepared revision** (a complete playable session). A revision includes a project-open generation so an old project's job cannot publish into a later opening of that project.

| Event | Planned behavior |
| --- | --- |
| Open | Parse and validate the manifest on a worker. After existing unsaved-change guards pass, show the new editor model and stop the previous project's playback. Show waveform placeholders until peaks arrive. Keep Play disabled until required audio and initial renderer preparation are ready; do not automatically start playback. |
| Edit during opening | Apply valid MIDI edits to the editor revision and update the latest required preparation. Earlier completions cannot replace newer editor data. Operations that need unavailable audio remain guarded with a reason. |
| MIDI edit with ready audio | Record the edit normally and use the existing renderer synchronization path. |
| MIDI edit needing audio | Show and record the validated edit immediately, mark its audio pending, and keep the last prepared revision playing if playback was already active. Status must state that playback uses the previous prepared version. |
| Preparation completes | Publish only a complete result matching the latest editor revision. Reuse existing transport-intent handling, exact catch-up, transitions, and meter isolation. Completion does not create another edit-history entry or move selection. |
| Stop or seek while pending | Stop acts immediately. Preserve explicit seek intent and never restart after Stop because an older worker completes. A new Play request waits for current required audio; do not start an obsolete arrangement silently. |
| Cancel or preparation failure | Retain the last playable revision and the visible pending edits. Mark preparation paused or failed. Offer retry and allow Undo; do not discard dependent edits or silently fall back to partial audio. Invalid core edits are rejected before they enter the editor model. |
| Undo/Redo | Restore the logical edit sequence immediately and request its prepared state. Reuse retained audio where possible; otherwise show pending preparation. Group by user gestures, not worker completion. A stale completion cannot restore an undone edit. |
| Save | Save the exact validated editor snapshot, including pending MIDI changes, through the existing independent save worker. Audio readiness does not determine dirty state. Save completion cannot clear newer edits. Derived cache files are not required for a successful save. |
| Export | Capture a specific editor revision, prepare its exact audio, then render it. Failure or cancellation cannot publish a partial export. Never export the older playable revision in place of the requested one. |
| Close, New, or another Open | Retain normal unsaved-change guards, cancel obsolete requests, invalidate their generations, and retire owned data outside the callback. |

If preparation fails after the editor opens, retain the new model with a clear failure state; do not resurrect the previous project. A malformed manifest fails before replacing the workspace. Source absence retains its established warning behavior; an unavailable pitch caused by unfinished preparation is a different state.

Stage 0 must prototype this separation before changing public session/history APIs. Model history becomes independent of audio readiness for these operations. This deliberately replaces the current rule that failed MIDI audio preparation prevents the model edit from becoming visible or saved. Update the applicable repository instructions and history/Sampler specifications when implementing that change. Do not construct a partial `Session` and pass it to callers that require complete audio maps.

## Ownership

| Owner | Responsibility |
| --- | --- |
| `daw-core` | Validated project edits and stable identity; no cache files, workers, or device state. |
| `daw-media` | Reusable preparation keys, exact PCM/peak encoding, source verification, cache validation, memory admission, and transform execution. |
| `daw-project` | Project dependencies, editor/prepared revisions, preparation requests, history/save snapshots, cancellation, and complete-session results shared by desktop and CLI. |
| `daw-engine` / `daw-output` | Existing exact renderer preparation, transport synchronization, activation, and callback-safe retirement. |
| `daw-ui` | Pending-state presentation, progress, guarded actions, retry/cancel, and worker wake-ups. No duplicated preparation or cache policy. |
| Applications | Resolve the platform application-cache directory and pass configuration to shared code. CLI remains usable without a display or audio device. |

## Stage sequence

| Stage | Status | Deliverable | Exit gate |
| --- | --- | --- | --- |
| 0 | Planned | Baseline and state/cache contracts | Reproducible release probe, state-transition fixtures, and explicit storage/resource policy. |
| 1 | Planned | Reusable memory and disk caches | Exact reuse across edits/restarts; corruption and failure safely fall back. |
| 2 | Planned | Background project opening | Editor ready before audio, with correct readiness and lifecycle guards. |
| 3 | Planned | Nonblocking MIDI preparation | Rapid edits, history, save/export, and stale-result handling satisfy the revision contract. |
| 4 | Planned | Bounded parallel and idle preparation | Required work has priority; resource pressure and cancellation remain safe. |
| 5 | Planned | Acceptance and documentation | Automated parity plus explicit native-platform evidence and measured improvements. |

## Stage 0 — Baseline and contracts

1. Add a release benchmark with generated long mono/stereo sources and representative chord edits. Time manifest loading, source hashing, decode/resampling, peaks, region construction, individual transforms, renderer preparation, editor readiness, and audio activation separately.
2. Cover empty caches, disk hits after process restart, memory hits, new pitch, previously used pitch, velocity-only edit, rapid pitch changes, undo/redo, and export. Record repeated median/tail timings, cache hits/misses, transformed frames, retained bytes, and peak process memory where available. Functional CI must not depend on wall-clock thresholds.
3. Define typed editor/prepared generations and readiness states. Prototype save/history behavior without relying on complete audio maps. Preserve source ownership and Save As/history relocation receipts, including edits saved before audio is ready.
4. Freeze the cache format, keys, directory policy, default byte limits, eviction, concurrency, cancellation, and failure behavior before implementation. Keep memory limits coordinated with current media/history/renderer accounting; do not treat them as separate unlimited pools.
5. Specify which controls remain usable before decoding completes and which require complete audio. Freeze Cancel/Retry, project-switch, and export-snapshot transitions with tests.

Exit gate: reproducible fixtures and baseline report; reviewed state-transition matrix; concrete policy values and cache/version contracts recorded. No private media is needed for regression tests.

## Stage 1 — Saved and reusable caches

1. Separate reusable prepared variants from the current bank's required-pitch set. Use bounded retention and deduplicate identical in-flight requests. One request's cancellation must not cancel work still required by another consumer.
2. Key decoded data by verified source content, decoder/resampler identity, rate, and format revision. Key selected/transformed data by input content and exact mapping/region, rate, duration ratio, pitch, engine/build/options, and pitch/format revisions. Include platform/build differences where cross-platform bit identity is not established. Names, asset IDs, and paths alone must not determine audio equivalence.
3. Reuse source-verification helpers where suitable. Verify bytes used for preparation against the recorded digest; reject concurrent source mutation. Length/mtime can aid lookup but cannot prove content identity, including same-length replacements with restored timestamps.
4. Persist exact float PCM and peaks with checked dimensions, lengths, finite-sample validation, integrity checks, and bounded reads/allocations. Validate source existence/revision before trusting derived data. Write to unique temporary files and publish atomically; handle concurrent readers/writers and interrupted writes on all platforms.
5. Evict disposable entries by byte budget. Drop cache owners on workers; never invalidate buffers still owned by playback/history. Cache write failure leaves successful required preparation usable. Handle disk-full, read-only storage, cleanup, and cache-disabled operation without changing project semantics.
6. Apply the same cache path to desktop opening/editing and CLI preparation/export. Preserve the complete-session API while adding cache support first.

Exit gate: returning to a retained pitch performs no transform; reopening with valid disk entries performs no repeated decode/pitch transform; cached/uncached PCM and export match. Changed source/rate/region/engine correctly miss. Source relocation with identical content permits reuse without breaking ownership rules. Corrupt, truncated, oversized, or incompatible entries rebuild safely.

## Stage 2 — Background project opening

1. Split manifest/model loading from media preparation behind shared project APIs. Keep existing complete `Session::open` behavior for headless callers by composing the phases.
2. Open the editor after model validation, then load cached audio or prepare misses on a worker. Required preparation precedes optional warming. Deliver peaks incrementally with generation checks; painting must not decode or hash.
3. Show phase/count progress in the status bar and wake the UI on progress/completion. Represent preparing, ready, paused, and failed states explicitly. A saved cache hit still requires bounded file reads and readiness checks.
4. Preserve source warnings and existing asset coverage initially, including unused assets. Lazy asset loading is a separate optimization; do not silently delete assets or skip their diagnostics.
5. Guard audio-dependent operations, project replacement, and renderer construction until their dependencies are ready. Cancellation must leave an editable model and a retry path without leaking workers or reservations.

Exit gate: a deliberately blocked media worker does not prevent editor interaction; Play cannot consume incomplete maps. Missing-source and preparation-error states remain distinct. Opening another project, closing, cancellation, and edits during opening cannot publish stale audio or peaks.

## Stage 3 — Nonblocking MIDI edits

1. Apply validated pattern edits to the editor revision without waiting for pitch preparation. Cover chord insertion, note pitch changes, velocity/duration edits, and shared-pattern users. Retain ordinary transaction validation and gesture grouping.
2. Maintain a bounded latest-request queue. Coalesce obsolete preparation requests, not user history. Reuse complete intermediate variants by key, but never activate an obsolete session. Recheck generation/revision at publication and renderer activation.
3. Keep prior playback explicit while pending. Reuse the existing live pitch-drag preview only within its current gesture contract; do not make its temporary resampling the saved/exported result.
4. Implement the state table's save, history, export, cancel/retry, failure, and transport rules. Unrelated edits made while audio is pending must survive. Keep incompatible import/rate/source-management operations guarded until their existing complete-session contracts can be satisfied.
5. Integrate logical history with bounded memory accounting and source receipts. Preserve already-prepared undo states; a missing disposable cache may require preparation, but cannot erase the logical history entry or select stale audio.

Exit gate: injected worker delays/failures plus rapid edits, Undo/Redo, Save, Stop/seek, and project switching preserve the latest intent and last valid playback. Worker completion adds no history step, clears no newer dirty state, and changes no selection. Export uses its captured revision exactly.

## Stage 4 — Bounded parallel and idle work

1. Prepare independent missing pitches with at most two workers initially. Reserve known memory before dispatch and retain a serial fallback for pressure, small work, or worker-start failure. Coordinate with existing transport/file workers to avoid unbounded CPU or memory growth.
2. Prioritize required opening/edit/export work above optional work. Cancel or yield obsolete requests at supported checkpoints; do not promise a hard interruption deadline inside a native backend call.
3. After a quiet interval, optionally prepare a small bounded set of neighboring pitches for the active Sampler. Define the exact selection and byte/time policy from Stage 0 measurements. Never prepare the full MIDI range by default. Pause optional work during active gestures, required jobs, memory pressure, or playback-pressure signals.
4. Retain completed useful entries after optional cancellation. Optional cache population must not modify projects, history, playback, or dirty state. Keep media-pitch caching distinct from DSP seek checkpoints while sharing resource-pressure signals.

Exit gate: serial/parallel audio equality, deterministic complete-bank publication, bounded queue/storage growth, pressure fallback, and prompt required-work priority. Measure whether speculative work improves real editing latency; omit it if the CPU/memory cost outweighs the benefit.

## Stage 5 — Acceptance and documentation

- Test cold/warm/restarted loads; repeated and unseen pitches; full MIDI endpoints; Off/Time/Bars; Gate/One-shot; mapped/repeated/stretched inputs; supported rates and engines; shared sources; missing sources; save/reopen/Save As/collection; and exact live/export/seek behavior.
- Test corruption, source replacement, concurrent requests, disk-full/read-only cache, eviction, interrupted writes, cancellation, shutdown, generation reuse, out-of-order completion, export failure, and memory pressure. Verify callback allocation/deallocation and file-I/O constraints.
- Compare release results to Stage 0 on the same hardware/build configuration. Report cold computation separately from cache hits and editor readiness separately from audio readiness. Require zero redundant transforms for compatible hits and no synchronous media work in UI handlers. Set numerical latency goals from the baseline rather than inventing universal limits.
- Perform native macOS, Windows, and Linux checks for editor responsiveness, status/Cancel/Retry, playback transitions, device pressure, and filesystem behavior. Record any unavailable checks as open gates, not passes.
- Update Sampler, performance, history, project-loading, CLI, and relevant repository instructions as each stage lands. Update user guides and README feature claims only for delivered behavior. Record measured results and remaining limits in validation history.

Completion means both repeated-work reduction and responsive editing are demonstrated. A first uncached transform may still take time; the interface must explain that state and preserve correct audio and user edits.
