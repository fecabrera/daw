# Performance audit — 2026-10-18

Status: historical audit baseline. Remediations were proposed at this baseline; see the [implementation and follow-up measurements](performance-remediation-2026-10-18.md) for their current status.

Baseline: `430e05f024815c78f3b1bde476feecb681474bbc`. The working tree was clean before this audit. The audit adds a benchmark harness, this report, and its evidence. It does not change application behavior or dependencies.

## Findings at a glance

The initial eight-voice workload has substantial CPU headroom on this machine. Larger arrangements expose avoidable work in scheduling, Timeline painting, modulation, and callback state changes. Allocation-free processing alone does not ensure that a buffer finishes on time.

| Observation | Measured result | Implication |
| --- | --- | --- |
| Eight saw voices, one track EQ and Compressor, all six rates and eight buffer sizes | All 48 renderer cases remain below 50% of the nominal buffer duration; worst observed p95 is 14.55% | The initial workload passes this local CPU check. Native callback acceptance remains open. |
| Eight audio tracks, 1,024 clips each, last clip, Loop off | About 6.3 ms per 512 frames | At 192 kHz, this exceeds the 2.67 ms renderer budget before device conversion or telemetry. |
| Timeline, 128 tracks and 64 clips per track | 50.81 ms median including CPU tessellation | This exceeds a 16.67 ms frame target. Offscreen waveform work is the main measured suspect. |
| Seek with eight Limiter tracks at 192 kHz | 1.23 ms median for seek alone | It exceeds a 32-frame duration of 0.167 ms. Command count limits do not bound command cost. |
| 1,152 modulation snapshot targets | 0.258 ms median per publication | Snapshot lookup alone can exceed a small buffer duration. |
| Core DSP, steady rendering, route resolution, history inheritance, and preallocated device conversion | No counted Rust allocations or deallocations in measured hot paths | Keep this property while removing scans and bounding work. |

No audio underrun, native frame rate, GPU timing, power consumption, or device latency was measured. Budget comparisons below identify CPU risks; they are not claims of observed audible dropouts.

## Scope and method

The review covers shared rendering, portable units, modulation, transport and renderer handoff, output queues and conversion, UI layout/painting, editing and validation, media preparation, project I/O, and export. Source review also covers Sampler buffer ownership, Envelope/LFO/Macro integration, MIDI editor visibility checks, and desktop startup/backend selection. Dedicated timing of every source configuration, editor gesture, operating system adapter, and failure path is outside the measured matrix.

The accepted [Units performance target](../architecture/modular-audio-units.md) is eight voices with representative effects at 48 kHz and 512 frames, below half the callback duration, with no callback allocations or blocking. The harness extends the CPU portion to every supported project rate and buffer size. EQ and Compressor use their defaults on a single instrument track. The voice workload uses one saw oscillator per voice. This is a named representative configuration, not a worst-case effect or modulation graph.

### Environment

| Item | Recorded value |
| --- | --- |
| Machine | Apple M5, Mac17,3; 10 CPU cores, 4 performance and 6 efficiency; 16 GB RAM |
| OS / target | macOS 27.0.1, arm64 / `aarch64-apple-darwin` |
| Compiler | Pinned Rust 1.97.0; LLVM 22.1.6 |
| Build | Cargo release, offline, workspace dependency versions and lockfile; no `RUSTFLAGS` |
| Lockfile SHA-256 | `2df3c26f16065e9fb22dd193914044ec64198d46480293e623f6820c00cef8f8` |
| Run | 217 completed cases: 49 DSP, 96 engine, 29 model, 17 UI, 26 media |
| Peak process resident memory | 370,835,456 bytes, about 353.7 MiB, for the complete synthetic run; excludes compilation |

Run cases serially. The harness uses deterministic generated stereo tones and generated audio/MIDI arrangements. No private recording or project is a fixture. Audio clips share a one-second PCM buffer and repeat it across 60-second clip ranges. Dense arrangement cases stress indexing rather than long-file memory use. No loop boundary is reached during the Loop-on/off control measurements.

Most cases have seven measured repetitions after an untimed warmup. Steady engine cases use at least 32 blocks, the rate/buffer target matrix uses at least 64 blocks, and UI cases use at least 30 frames after eight initial frames plus the common warmup. The JSON records exact sample counts and all sorted timings. p95 is the nearest-rank sample percentile. With seven samples, it is the maximum; it is not a statistical confidence bound. CPU affinity, scheduling priority, thermal state, power mode, and background OS activity were not controlled. Do not infer small percentage improvements from these samples.

Times exclude destruction of a value returned by the measured operation. Destruction inside the operation is included. The allocator counter covers the measured thread's Rust `System` allocation/reallocation calls and deallocations. Allocated bytes are cumulative allocation traffic, not retained memory or peak heap. Native C/C++ allocations and other threads are not counted. The process resident-memory figure includes native memory but has no per-case attribution.

UI measurements run the real public UI entry point with egui at 1,600 × 1,000 logical points and no audio output. They include CPU layout/painting; separate cases also include CPU tessellation. They exclude native event delivery, texture upload, GPU submission, swap/presentation, and display scaling. Media tests use warm filesystem caches and temporary files under ignored `work/`; no cold-disk throughput claim is made. Export timing uses eight audio tracks without effects over four seconds.

### Reproduce and inspect

The [Python runner](../../tools/performance_audit.py) builds the [Rust harness](../../tools/performance_audit.rs) through an isolated manifest under ignored `work/performance-audit/`. It does not add a workspace member or alter the root lockfile. Python 3.11 or newer and the repository's native build prerequisites are required. This invocation was verified on macOS; Windows execution of the runner is not verified.

```sh
python3 tools/performance_audit.py --suite all --runs 7 --output work/performance-audit/results.json
```

Choose `dsp`, `engine`, `model`, `ui`, or `media` to narrow the run. Avoid concurrent builds, tests, or benchmarks when comparing timings. The [recorded evidence](performance-audit-2026-10-18.json) includes the baseline, lockfile and harness hashes, hardware, samples, allocation counters, workload facts, and completion status. Use the normal build path when reproducing; `--skip-build` can reuse a stale binary after source changes.

All source locations below refer to the audit baseline. Proposed targets are engineering recommendations, not new accepted product limits. P1 means a measured nominal-budget violation or a substantial resource risk; P2 means a scaling or efficiency problem; P3 means an assurance gap. Priority is not a claim that every user encounters the issue.

## Prioritized findings and remediations

### F01 — P1: Project end is recalculated for each live sample

**Evidence.** [`Renderer::next_sample`](../../crates/engine/src/lib.rs) calls `Project::end` on each playing sample with Loop off. [`Project::end`](../../crates/core/src/lib.rs) scans all clips. With eight tracks and 1,024 clips per track, rendering 512 frames at the first clip takes 3.109 ms with Loop off and 0.069 ms with Loop on at 48 kHz. The Loop-on control avoids the end scan and does not cross a loop boundary. These are observations of the current paths, not a measured patch speedup.

**Remediation.** Cache the arrangement end in prepared renderer state. Refresh it when a new project generation is prepared. Keep the exclusive-end rule, stop ramp, tails, and loop behavior unchanged. Do not update a shared mutable project from the callback.

**Owner / acceptance.** `engine`. Remove work proportional to total clips from this per-sample check. On the same fixture, Loop-off rendering should be close to Loop-on rendering after accounting for the remaining clip lookup. Verify the same final sample, stop/fade behavior, empty projects, edits that change the end, and zero callback allocations.

### F02 — P1: Each track searches its clip list for each sample

**Evidence.** [`audio_source`](../../crates/engine/src/lib.rs) and [`MidiRuntime::sample`](../../crates/engine/src/midi.rs) use linear clip searches. With Loop on to avoid F01, eight tracks of 1,024 clips take 0.069 ms at the first clip and 3.372 ms at the last clip for 512 frames at 48 kHz. At 192 kHz, the last-clip case also exceeds the 2.667 ms duration. Source buffer lookup and source mapping also repeat per sample.

**Remediation.** Prepare sorted clip ranges and a current-clip cursor per track. Advance the cursor at clip boundaries; use binary search on seek, loop discontinuity, or changed arrangement. Bind immutable source/prepared-stretch buffers during preparation. Reuse the scheduler for live and offline rendering; retain clip fades, repeat phase, missing-source silence, exclusive ends, and effect-tail processing in gaps.

**Owner / acceptance.** `engine`. A steady block inside a clip must have no work proportional to the number of earlier clips. Verify early/late/gap, trim/repeat/stretch, loop/seek, and live/export parity. Re-run dense 192 kHz cases and require the initial half-duration headroom target on named hardware before treating this issue as closed.

### F03 — P1: Timeline computes waveforms for offscreen rows

**Evidence.** [`tracks` and `lane`](../../crates/ui/src/lib.rs) lay out every row. `lane` tests a clip against its lane bounds, not the viewport's vertical bounds. [`waveforms::paint`](../../crates/ui/src/waveforms.rs) intersects clipping rectangles but still loops across horizontal pixels when their vertical intersection is empty. Clipped painting does not prevent peak calculation. At 128 tracks × 64 clips, Timeline layout/painting takes 48.02 ms median. Removing source audio from the same-size scene reduces it to 1.51 ms. This control implicates the waveform path but is not a function-level profile.

CPU tessellation brings the full scene to 50.81 ms. That frame makes up to 23,784 counted allocations and allocates 45,866,451 cumulative bytes, about 43.7 MiB. This is allocation traffic, not a leak. Even the eight-track scene takes 3.99 ms including tessellation. The fixed 256-frame peak cache also makes wide source windows scan many peak blocks per pixel.

**Remediation.** First return before waveform work when the visible rectangle is empty. Then virtualize track/layer rows, search horizontally visible clips, and borrow immutable row data instead of cloning complete tracks. Add a multiresolution peak pyramid only after measuring zoomed-out long files. Cache reusable waveform geometry by source identity, prepared-buffer revision, visible range, zoom, and theme where beneficial.

**Owner / acceptance.** `ui`, with peak generation in `media`. Keep interaction regions, drag capture, offscreen drop targets, selection/focus, edge scrolling, shared dividers, and previews correct. At a fixed viewport, offscreen row count must not multiply waveform work. Proposed local target: p95 below 16.67 ms for the 128-track fixture including CPU tessellation, with a substantial drop in allocation traffic. Add a native GPU/presentation check separately.

### F04 — P1: Modulation source lookup scales as routes × providers

**Evidence.** [`ModulationRoute::sample`](../../crates/unit-core/src/modulation/runtime.rs) searches local and shared provider slices by stable source ID for each route on each sample. The resolver benchmark places the source last. With 128 routes, moving from one to 128 published providers increases median cost from 344 ns to 7,804 ns per destination sample. The latter is about 4.00 ms per 512-frame block for one resolver, before DSP and host work. This is a deliberately dense valid resolver workload, not the default project.

**Remediation.** Extend the shared preparation contract with validated source-slot bindings. Resolve stable IDs and local/shared scope on the control thread; use generation-checked indices during processing. Keep local precedence, dynamic range/behavior, disabled-source weights, destination filtering, and additive smoothing. Retain an explicit unbound/missing-source case. Do not add source lookup or routing loops in individual units.

**Owner / acceptance.** `unit-core` with host preparation in `engine`. The shared/local provider matrix must retain numerical, scope, lifetime, and route-reorder behavior. With route count fixed, steady cost should be nearly independent of unrelated provider count. Keep the callback allocation-free and verify continuous sources do not acquire note-release ownership.

### F05 — P1: Seek and latency priming perform variable work in the callback

**Evidence.** [`Callback::update`](../../crates/output/src/lib.rs) applies transport commands on the audio thread. [`Renderer::seek`, `play`, and `prime_latency`](../../crates/engine/src/lib.rs) can reset processing and render the entire alignment lookahead before returning. One Limiter track at 192 kHz primes 1,952 frames and takes 0.177 ms median for seek alone; eight tracks take 1.232 ms. A 32-frame duration at that rate is 0.167 ms.

The MIDI restart path in [`midi.rs`](../../crates/engine/src/midi.rs) finds the event position, then replays preceding events to reconstruct state. Seeking into generated patterns of 256, 4,096, and 16,384 sequential notes and rendering the first 32 samples takes 0.048, 0.254, and 1.003 ms at 48 kHz. The largest case exceeds a 0.667 ms duration. This test includes the first block because MIDI reconstruction is deferred until sampling. It does not include a native device callback.

**Remediation.** Separate expensive transport preparation from the bounded activation step. Prepare required MIDI state/checkpoints and alignment pre-roll on a worker using the shared renderer. Use project/transport generations to discard obsolete work. Define how playback continues or fades while a requested position is being prepared; preserve pause/stop/start-position semantics and transactional failure. For repeated loop boundaries, prepare reusable bounded state rather than repeating an arbitrary-length event prefix in the callback.

**Owner / acceptance.** `engine`, `output`, `project`/UI job coordination. Measure command application separately from steady blocks at every buffer size. No command may synchronously replay work proportional to the full earlier pattern or an unbounded effect-chain delay. Verify source/effect history, tails, fades, latency alignment, rapid seek cancellation, live/export results, and queue saturation. A worker design must not create a separate DSP implementation.

### F06 — P1: Compatible renderer inheritance contains quadratic ID searches

**Evidence.** [`retarget_modulation_routes`](../../crates/unit-core/src/modulation/runtime.rs) finds each incoming route by scanning current routes. Track/effect/alignment inheritance also searches by ID in [`Renderer::inherit`](../../crates/engine/src/lib.rs), [`inherit_chain`](../../crates/engine/src/effects.rs), and [`latency.rs`](../../crates/engine/src/latency.rs). Reordering 128, 512, and 2,048 routes takes 3.17, 38.75, and 788.75 µs median; the last p95 is 1,408.83 µs over seven samples. Counts remain allocation-free. The 2,048-route case is a stress case, not a typical preset.

**Remediation.** Prepare stable-ID match tables or sorted indirection arrays outside the callback. Inherit through validated old/new slot pairs with bounded linear traversal. Preserve processing order and floating-point accumulation order; do not sort project-visible routes or replace stable IDs with list indices. Avoid callback allocation and destruction.

**Owner / acceptance.** `unit-core`, `engine`. Route/track/layer/effect permutations must retain compatible ramps, phases, sample positions, and envelopes. Verify removal/addition and incompatible topology. Measure increasing sizes and require approximately linear growth; include the largest supported project in native small-buffer handoff tests.

### F07 — P1: Meter publication repeats whole-renderer lookup for each target

**Evidence.** [`PreparedRenderer::publish_track_peaks`](../../crates/output/src/lib.rs) calls [`Renderer::modulation_snapshot`](../../crates/engine/src/lib.rs) per bound target, then stores all 19 slots. Effect snapshot lookup searches tracks/chains. Sampler/oscillator readings can also traverse voices and layers. A benchmark mirroring effect snapshot lookup plus the relaxed atomic stores takes 1.71 µs for 72 targets, 18.71 µs for 288, and 258.29 µs for 1,152. The last p95 is 267.33 µs, already above a 32-frame/192 kHz duration. It does not include track-peak publication or the full callback.

**Remediation.** Bind target snapshot slots once in prepared state, or traverse the renderer once to fill a prepared flat snapshot buffer. Publish only supported native fields, with a defined UI consumption cadence. Keep actual DSP effective values. Preserve the meter rules: disabled bands use secondary grey; disabled units clear all meters and stop updates until enabled. Keep off-unit state changes observable without stale readings.

**Owner / acceptance.** `engine`, `output`. Publication should grow linearly with targets and avoid per-target whole-renderer searches. Re-run snapshot discovery, power, empty/removed target, NaN-unused-slot, and source/effect matrices. Test total callback cost with many meters at small buffers; reduced UI publication frequency must not lose peaks or break meter decay.

### F08 — P2: Idle repaint and synchronous renderer preparation consume UI time

**Evidence.** [`DawUi::show`](../../crates/ui/src/lib.rs) requests another repaint after 16 ms even when idle. The same path calls `Session::plan` and `AudioOutput::sync` when `sync_needed`; [`sync`](../../crates/output/src/lib.rs) prepares a full renderer and meter bindings on the caller thread. Core-only preparation plus plan cloning costs 2.109 ms and 7.57 MB cumulative allocations for 128 tracks × 64 clips. This excludes output meter construction and is a lower bound on that synchronization path. If the queue is full, the UI can retry preparation on another frame. Initial `AudioOutput::new` also runs from the Play handler; device startup duration was not measured.

**Remediation.** Request continuous frames only for playback, active interaction, meter decay, or progress that requires polling. Worker completion must wake egui explicitly. Distinguish presentation changes and compatible parameter updates from structural preparation. Coalesce superseded preparations by generation and prepare structural state on a worker. Keep the existing shared retargeting and 5 ms ramps. Cache a prepared pending update rather than rebuilding it on every queue-full retry.

**Owner / acceptance.** `ui`, `output`, shared preparation orchestration. Measure idle wakeups/CPU/power natively; no battery saving is quantified here. Verify worker completion, errors, cancelled jobs, drag/keyboard feedback, and meter decay without periodic idle repaint. Rapid knob updates must remain responsive, preserve audio history, and publish the final value.

### F09 — P2: Edits clone twice in the UI and validation repeatedly searches assets

**Evidence.** [`DawUi::edit`](../../crates/ui/src/lib.rs) clones the project before [`Project::edit`](../../crates/core/src/lib.rs) clones it again for transactional validation. Validation repeatedly finds each clip's asset in a vector. With 2,048 clips, increasing assets from one to 2,048 increases validation from 69 µs to 1,736 µs and a core mute edit from 100 µs to 1,851 µs. The latter makes 12,327 counted allocations. With one asset and 8,192 clips, a core mute edit takes 412 µs; a full plan clone takes 85 µs. These core measurements do not include the UI's extra clone or synchronization.

**Remediation.** Use an ID index during validation/preparation. Let the UI transfer a single staged owned project through the transaction rather than clone it twice. Consider validated narrow parameter updates or immutable shared structures after profiling. Preserve final-group validation and atomic rollback; do not bypass safety checks to reduce time.

**Owner / acceptance.** `core`, `ui`, `project`. Validation should scale approximately with assets + clips rather than their product. Compare equivalent projects with different unused asset counts. Retain invalid-edit rollback, overwrite/resize behavior, stable IDs, media-preparation failure handling, and saved format compatibility.

### F10 — P1 resource risk: Full decoded buffers lack a session-wide memory/admission policy

**Evidence.** [`decode_wav_at_rate`](../../crates/media/src/lib.rs) materializes the full source as `[f32; 2]`, including mono sources. [`Session::open` and `import_decoded_mode`](../../crates/project/src/lib.rs) decode by asset/import and do not deduplicate identical decoded source content across independent assets. Processing-rate replacement retains old and new buffers until successful publication. One hour at 192 kHz needs 5,529,600,000 bytes, about 5.15 GiB, for one stereo PCM buffer before peaks, resampling, stretches, staging, and renderer owners. That figure is an arithmetic bound, not a measured import.

[`FileHover::at_sample_rate`](../../crates/ui/src/file_drop.rs) starts a decode thread; discarding the receiver does not cancel decoding. Fast changes between large hovered files can leave several workers consuming CPU/memory. Ordinary job guards reduce other concurrency, and the Integrated engine has its own request memory checks. Neither is a global import/session budget. No out-of-memory or concurrent-hover failure was induced in this audit.

**Remediation.** Add checked size estimates, fallible reservation, and admission against a session/job memory budget that includes current and staged owners. Use bounded cancellable preview workers with generation checks. Share immutable decoded audio by canonical source identity, source fingerprint, processing rate, and revision while preserving separate asset IDs. Prune unused caches/owners only when safe. Evaluate streaming or disk-backed PCM as a separate architectural change for long sessions.

**Owner / acceptance.** `media`, `project`, `ui`. Oversized or cancelled jobs must leave the original project and playback intact and report a useful error. Test duplicate imports, source replacement, rate changes, many rapid hovers, missing media, and stale worker completion. Record per-job peak resident memory under controlled native stress. Do not impose an undocumented fixed track limit.

### F11 — P2: Offline resampling allocates input/output blocks repeatedly

**Evidence.** [`media::resample`](../../crates/media/src/lib.rs) builds nested channel vectors for each input block, calls the allocating conversion API, grows the output, and copies the delay-trimmed final slice. A ten-second 48→96 kHz conversion takes 6.33 ms median, 2,865 allocation/reallocation calls, and 36.14 MB cumulative allocations. Decoding and converting a ten-second source to 192 kHz takes 13.22 ms and 84.61 MB cumulative allocations. These are fast short jobs but unnecessary allocation traffic scales with duration and concurrency.

The output callback already uses preallocated `process_into_buffer`: measured conversion of a 512-input-frame block takes 1.92–8.54 µs across the three tested rate pairs, with no counted allocations. Reuse that buffer discipline offline rather than replacing the resampling library.

**Remediation.** Reserve checked final capacity, reuse planar input/output blocks, use `process_into_buffer`, and write only the delay-compensated final range. Avoid the last full-buffer copy. Keep conversion and peak generation on workers and preserve output frame count, channel linking, sample-rate metadata, and deterministic cache identity.

**Owner / acceptance.** `media`. Allocation count should be bounded by setup and final buffers rather than input block count. Verify all project rates, short and partial blocks, identity-rate behavior, numerical parity, cancellation/admission policy, and peak resident memory on a long synthetic file.

### F12 — P2: Bypass retains DSP cost; dynamic parameters require targeted optimization

**Evidence.** Most portable units retain nearly all default processing cost while disabled. This preserves history and bypass transitions. For example, OTT is 114.30 ns/frame enabled and 113.89 disabled; Limiter is 32.51/32.31. These are direct unit calls without host routing or attached audio meters. Default zero-intensity Stereo Spread still advances its filters. Moving Stereo Spread frequencies increases median cost from 36.00 to 75.67 ns/frame at 48 kHz. [`Bank::configure`](../../crates/stereo-spread/src/lib.rs) already avoids unchanged coefficients; [`Drive`](../../crates/drive/src/lib.rs), [`EQ`](../../crates/equalizer/src/lib.rs), and dynamics units also cache several coefficients. Do not attribute all cost to redundant coefficient rebuilds.

[`Limiter::process_effective_sample`](../../crates/limiter/src/lib.rs) still calculates lookahead-dependent attack and release exponentials each sample. Other effective gain/log-frequency conversions also merit a profile under static and changing modulation. The measurements do not identify a library replacement or SIMD rewrite as necessary.

**Remediation.** Cache derived values whose effective inputs have not changed, starting with Limiter timing coefficients. Profile static, base-ramp, audio-rate modulation, silence, and near-denormal tails separately. Optimize the shared smoother/parameter path where possible. Any coefficient-rate reduction or approximation needs explicit signal/error and listening acceptance. Do not skip disabled unit processing if that changes required histories, tails, ramps, or latency.

**Owner / acceptance.** Each portable unit owns its DSP; `unit-core` owns shared control primitives. Keep exact neutral/dry behavior, finite output, modulation bounds, stereo phase/mono compatibility, 5 ms transitions, and live/export parity at every rate. Measure hot loops before and after changes using representative active settings; the default table is not a worst-case ceiling.

### F13 — P2: Offline mixing repeats audibility and static gain calculations

**Evidence.** [`Renderer::offline_frame`](../../crates/engine/src/lib.rs) calls `Project::audible` for each track on each frame. `audible` scans tracks for Solo. Gain conversion and pan coefficients are also recomputed instead of using the live path's prepared gains. One-clip-per-track offline blocks take 59.7 µs for eight tracks, 313.1 µs for 32, and 3,205.3 µs for 128. Effect/source work is unchanged in this fixture; the code exposes an avoidable quadratic Solo scan.

**Remediation.** Prepare solo/audibility and static gain/pan coefficients once for offline state. Share the live/offline mixing primitives rather than build a second renderer. Continue processing muted or solo-excluded sources/effects when required for history.

**Owner / acceptance.** `engine`. Offline no-effect scaling should approach linear track growth. Verify solo precedence, mono pan/stereo balance, master gain, clipping diagnostics, effect tails, source fades, and export/live numerical parity. Re-run WAV and MP3 end-to-end cases to ensure the renderer gain is visible beyond encoding/I/O noise.

### F14 — P2 API risk: Stateful random access re-renders from frame zero

**Evidence.** [`RenderPlan::render`](../../crates/engine/src/lib.rs) creates a renderer, starts at zero, and discards all earlier samples whenever effects or MIDI require state. `sample_at` takes the same path for a single stateful sample. Rendering 512 frames with one EQ takes 0.079 ms at zero, 7.63 ms at one second, 74.74 ms at ten seconds, and 449.07 ms at 60 seconds. These costs preserve exact history. Current streaming playback/export use persistent renderers; this is not evidence that normal waveform painting invokes the API.

**Remediation.** Document the cost explicitly and favor a persistent renderer for sequential consumers. If interactive random access is needed, add versioned processing checkpoints or a worker-rendered preview/cache with clear history semantics. Do not silently replace exact replay with a reset at the requested frame.

**Owner / acceptance.** `engine` and future callers. Add regression coverage that prevents loops calling stateful `sample_at` across a range. Checkpoint results must match exact rendering within the agreed tolerance after effects, MIDI state, modulation, project edits, and stretch changes; obsolete checkpoints must be invalidated.

### F15 — P2: Voice slots and editor data need visibility/activity indexing at scale

**Evidence.** The eight-active-voice fixture takes 210 µs per 512 frames with eight prepared voice slots and 337 µs with 128 prepared slots. The latter also contains future notes that require the larger capacity. [`midi.rs`](../../crates/engine/src/midi.rs) traverses prepared voice/layer state and shared readings; idle capacity can add work even when only a few notes sound. Sampler PCM uses `Arc` ownership and does not copy audio per voice, which is a useful invariant to retain.

The MIDI editor already limits visible repetitions and tests note rectangles before interaction. However, [`midi_editor.rs`](../../crates/ui/src/midi_editor.rs) clones the displayed pattern and scans its notes per visible repetition. Instrument layout clones a selected track and its instrument and visits all layers. The measured 32-layer Instrument fixture is only 0.661 ms median, so there is no demonstrated frame-budget violation there. Dense MIDI editor and large Sampler-polyphony costs were not separately timed.

**Remediation.** Profile active/releasing/idle voice traversal and telemetry separately. Maintain preallocated active-slot lists if they reduce cost without skipping required continuous-source or note-tail work. Use borrowed editor views and time/pitch indices for dense patterns where profiling justifies them. Apply the shared row virtualization from F03 to layers rather than introduce a second layout mechanism.

**Owner / acceptance.** `engine`, `ui`. Benchmark eight active voices with rising reserved polyphony, many simultaneous Sampler voices sharing one source, and dense/repeated piano-roll patterns. Preserve stable voice assignment, maximum release edits, per-note modulation, One-shot/Gate behavior, selection/draw/drag previews, and callback allocation freedom.

### F16 — P3: Performance acceptance needs native and regression evidence

**Evidence.** Existing tests and this harness check allocation behavior and signal/interaction correctness. They do not record complete device callback duration, underruns, GPU presentation, idle power, queue latency, or memory under realistic parallel jobs. macOS, Windows, and Linux have different device/backend paths. The configured project buffer may differ from the device's effective period or fallback buffer. No universal capacity claim follows from one M5 run.

**Remediation.** Keep this synthetic matrix as a diagnostic baseline. Add a stable-hardware performance job with representative saved generated fixtures and before/after comparisons. Record callback duration outside the callback with preallocated telemetry; count underruns/device failures, queue depth/age, command activation time, worker latency, resident memory, and native frame times. Capture native CPU/GPU traces only after identifying a reproducible workload. Do not log, allocate, lock, or format strings in the callback.

**Owner / acceptance.** `output`, `ui`, `tools` and native CI. Test every rate/buffer combination with the actual chosen device period, matching and mismatching device rates, edit/seek/loop bursts, large modulation graphs, and full visible meters. Include saturation and device-failure behavior and verify old-renderer destruction remains off the callback. Define repeatable hardware/power conditions and variance tolerances before gating absolute timings. Keep functional CI separate from noisy timing regressions.

## Additional measurements

All times in the next tables are medians. Full sample distributions and workload facts are in the JSON. Figures rounded here do not imply that the measurements have that precision.

### Steady render and UI scaling

| Case | Median | p95 | Notes |
| --- | ---: | ---: | --- |
| Eight voices + EQ/Compressor, 48 kHz / 512 | 0.308 ms | 0.334 ms | 10.667 ms nominal duration |
| Eight voices + EQ/Compressor, 192 kHz / 32 | 0.021 ms | 0.024 ms | 0.167 ms nominal duration; worst target-matrix utilization |
| 128 saw voices, 48 kHz / 512, no effects | 3.333 ms | 3.684 ms | No counted Rust allocation/deallocation |
| Eight saw voices, all nine effects, 48 kHz / 512 | 0.531 ms | 0.589 ms | Defaults, includes Limiter alignment in the prepared renderer |
| Mixer, 128 tracks, including tessellation | 2.434 ms | 2.514 ms | 10.11 MB cumulative allocation traffic/frame |
| Timeline, 32 tracks × 64 clips, including tessellation | 13.681 ms | 15.193 ms | Separate layout/paint case has a 17.99 ms p95 outlier |
| Timeline, 128 tracks × 64 clips, including tessellation | 50.810 ms | 51.206 ms | 45.87 MB cumulative allocation traffic/frame |
| Instrument, 32 layers | 0.661 ms | 0.747 ms | CPU layout/paint; no native presentation |

### Direct portable DSP at 48 kHz

These calls exclude host scheduling/routing, output conversion, and bound audio-meter handles. Settings are defaults; Stereo Spread's default intensities are zero. Treat them as diagnostic costs, not independent maximum polyphony ratings.

| Unit | Enabled ns/frame | Disabled ns/frame |
| --- | ---: | ---: |
| 3-band EQ | 152.18 | 113.47 |
| Drive | 23.14 | 22.87 |
| Auto-filter | 44.00 | 44.00 |
| Compressor | 16.58 | 16.55 |
| OTT | 114.30 | 113.89 |
| Limiter | 32.51 | 32.31 |
| Direction Mixer | 4.42 | 4.56 |
| Stereo Spread | 33.39 | 32.41 |
| Bitcrusher | 14.92 | 12.84 |

Differences such as the Direction Mixer enabled/disabled inversion are within the limits of these small timing samples. The evidence does not establish that disabling that unit increases its true cost.

### Media, storage, and export

| Generated workload | Median | Counted allocation calls | Cumulative allocated bytes |
| --- | ---: | ---: | ---: |
| Decode 10 s float32 WAV, 48→48 kHz | 1.73 ms | 870 | 16,185,429 |
| Decode 10 s float32 WAV, 48→96 kHz | 8.13 ms | 3,735 | 52,356,187 |
| Decode 10 s float32 WAV, 48→192 kHz | 13.22 ms | 3,733 | 84,610,739 |
| Resample 10 s PCM, 48→96 kHz | 6.33 ms | 2,865 | 36,140,758 |
| Integrated raw stretch, 4 s at 3:2 | 86.24 ms | 40 | 7,472,812 |
| DAW Integrated stretch adapter, 4 s at 3:2 | 86.34 ms | 46 | 11,330,902 |
| DAW Rubber Band R3 adapter, 4 s at 3:2 | 223.67 ms | 197 | 2,390,634 |
| Eight-track 4 s PCM24 export, 48 kHz | 24.55 ms | 195 | 37,627 |
| Eight-track 4 s PCM24 export, 192 kHz | 95.91 ms | 195 | 37,629 |
| Eight-track 4 s MP3 export, 48 kHz | 124.30 ms | 197 | 49,559 |
| Eight-track 4 s MP3 export, 192 kHz | 185.76 ms | 197 | 49,561 |
| Save 32 tracks × 64 clips, one source | 3.97 ms | 2,207 | 2,562,454 |
| Open the same project with a 10 s source | 2.99 ms | 11,621 | 20,195,401 |

Stretch ratios here are output duration / input duration. The DAW adapters also prepare the application buffer/peaks; the raw Integrated API does not perform all that work. Rubber Band native C++ allocations are absent from the Rust counter, so its allocated-byte column cannot be compared as total memory. Synthetic timing is not a quality test or permission to change the default engine. Independent implementation/acceptance requirements remain in [the timestretch specification](../audio/timestretch.md) and [acceptance plan](../acceptance/timestretch.md).

## Existing safeguards to retain

- The audio callback uses bounded queues and limits command handling to eight commands per update. The issue is work inside a command, not an unbounded queue-drain loop.
- Renderer preparation, decoding, stretching, file I/O, and encoding normally occur outside the callback. Retired renderers and final audio owners are returned to the control side for destruction. A pending renderer is retained if the retirement queue is full.
- The measured steady renderer, DSP, route resolution, compatible inheritance, telemetry lookup/stores, and preallocated Rubato conversion have zero counted Rust allocations and deallocations. Native device execution is still a separate check.
- Live track gains and transitions are prepared/ramped. Continuous provider publication and route smoothing already have shared APIs. Optimize these common paths instead of adding per-unit routing code.
- Immutable PCM/peaks use shared ownership; clip copies do not duplicate recordings. Sampler voice buffers also share PCM.
- Save/export and prepared-media replacement preserve staged publication and failure rollback. Export uses a persistent renderer and streaming encoder blocks; it does not allocate a whole mixed export buffer.
- The Integrated stretch engine checks request resources and supports cancellation. Keep those checks while adding a broader session/job admission policy.

## Suggested remediation sequence

| Order | Work package | Exit evidence |
| --- | --- | --- |
| 1 | Cache arrangement end and index/cursor clip playback (F01–F02) | Dense first/last/gap and Loop-on/off matrix; exact timing/fade/export parity; allocation checks |
| 2 | Cull invisible waveform work, then virtualize rows (F03) | Fixed-viewport track scaling, allocation/geometry counts, interaction tests, native frame trace |
| 3 | Bind modulation sources/snapshots and prepare inheritance mappings (F04, F06–F07) | Route/provider/target scaling becomes near-linear; shared contract and history matrices |
| 4 | Bound transport activation; move preparation/coalescing to workers (F05, F08) | Small-buffer command-burst tests, cancellation and queue stress, native callback histogram |
| 5 | Index validation and reduce edit cloning (F09) | Asset/clip scaling and transactional rollback tests |
| 6 | Add memory admission/cancellation and reuse resampling buffers (F10–F11) | Long-source/duplicate/rapid-hover stress with peak resident memory and failure rollback |
| 7 | Profile targeted DSP, offline mixing, active slots, and random access (F12–F15) | Numerical/listening checks and measured end-to-end gain, without history regressions |
| Throughout | Record stable native baselines and regression comparisons (F16) | macOS/Windows/Linux device, GPU, queue, memory, and idle measurements |

Do not combine these into one broad refactor. Establish a before/after baseline for each work package and retain the existing signal and editing acceptance tests. No percentage reduction or native latency improvement is promised by this plan before implementation and measurement.

## Verification and remaining limits

The final release benchmark run completed all 217 cases. The standalone harness passes strict release Clippy; workspace rustfmt and the standalone Rust format check pass. The Python runner passes syntax parsing. Workspace validation and documentation-link results are recorded in [validation history](README.md).

No native app was launched or relaunched. Native input/display-scale, device callback timing, underruns, startup latency, GPU presentation, idle power, long-session soak, cold/slow storage, out-of-memory handling, simultaneous preview stress, and Windows/Linux performance remain unverified. No call-graph sampling profiler, hardware performance counters, or contention profiler was attached. Code paths and controlled workload comparisons identify causes to investigate; they do not substitute for those traces. Benchmark output cannot certify all real-time paths as allocation-free, especially native libraries and untested failure branches.

Provenance: this audit reads the DAW's own media adapters and portable engine/specifications and calls the existing Rubber Band backend through its public adapter. It does not inspect or derive changes from Rubber Band implementation source. No time-stretch implementation or acceptance requirement changes.
