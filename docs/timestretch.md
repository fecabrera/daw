# Timestretch: functional specification

Status: experimental implementation, 2026-10-14. The engine, Settings selector, CLI commands, and scripts exist. This document and the [acceptance plan](../specs/timestretch-acceptance.md) remain the target contract. See [validation](validation.md#experimental-timestretch-engine-and-comparison-tool--2026-10-14) for completed checks and remaining coverage. Rubber Band remains the default backend. This specification does not certify independent authorship or legal clearance.

## Mandatory independent implementation rule

**Under no circumstances may the Timestretch implementation take code directly from Rubber Band.** Do not copy, translate, port, adapt, or mechanically rewrite any Rubber Band implementation code, including through an AI tool, intermediary project, or another language. This prohibition also covers implementation-specific pseudocode, tests, comments, tables, and other implementation material. Attribution, available source, a different license offer, schedule pressure, quality targets, and failing acceptance tests do not create an exception to this project requirement.

Develop the engine and its tests independently from public papers, mathematical descriptions, and documented behavior. Public documentation may supply behavioral requirements and research references; it must not serve as a route to reuse implementation material. Do not inspect Rubber Band implementation source to guide the new engine. Record research sources and prior source exposure honestly in the provenance record.

The comparison tool and existing DAW backend may call the separately licensed Rubber Band library through its public API. Write those adapters independently from API documentation. Keep Rubber Band dependencies and their required notices isolated from the BSD-2-Clause engine. Permission to run a reference backend does not permit taking its code into the new implementation.

Any prohibited material blocks acceptance. Remove it and replace the affected work with an independently developed implementation before proceeding. Passing signal-quality or performance tests does not waive this rule.

## Purpose and scope

Develop an independent, reusable Rust time stretch library. Name the package and library `timestretch`, at `crates/timestretch`. License its original code under BSD-2-Clause, consistent with the repository [LICENSE](../LICENSE). The current use cases bound the initial requirements; using the library must not require a DAW, project model, or this application's runtime. Keep it suitable for extraction into its own repository once stable.

The processor changes duration while preserving pitch. It processes a complete in-memory source at one constant ratio. The caller owns threading and result publication. Provide an independent command-line tool at `crates/timestretch-cli`, package `timestretch-cli`, binary `timestretch`, for file processing and three-engine comparisons. The DAW is one optional consumer, described separately below.

| Item | Contract |
| --- | --- |
| Processing | Offline, complete source; mono and linked stereo |
| Samples | `f32`, including finite headroom above unity |
| Processing rates | 44100, 48000, 88200, 96000, 176400, 192000 Hz; fixed initial supported set, independent of any project |
| Duration ratio | Output duration / source duration; inclusive range 1/8–8 |
| Timing | Exact integer output length; internal delay compensated |
| Evaluation | Standalone file tool comparing Timestretch, Rubber Band R2, and Rubber Band R3; optional DAW engine selector |

Live stretching, ratio automation, independent pitch/formant controls, tempo detection, warp markers, more than two channels, plugin/device management, and GPU processing are outside this version. The portable API has no track, clip, MIDI, project, transport, or device concepts. Within the DAW, MIDI stretching continues to retime MIDI data through its existing model; there are no per-clip engine or quality choices in this version.

## Portable crate

`crates/timestretch` owns DSP, request validation, output-length calculation, progress, and cancellation checkpoints. It must build and test without DAW crates, graphics, devices, filesystem access, native C/C++ libraries, or Rubber Band. `crates/timestretch-cli` adds file decoding/conversion, reference adapters, reports, and orchestration through portable dependencies. Neither package may depend directly or transitively on `daw-*`, application configuration, project manifests, or DAW build artifacts. The CLI may require native build tools for its Rubber Band references; the core crate must not.

Dependency direction is consumer → portable engine. The DAW's media layer adapts portable types to project types. Share any common reference-adapter code through a small DAW-independent support library if needed; do not make the engine depend on the CLI or the CLI depend on the DAW. A tiny external Cargo consumer must be able to use the engine with only sample buffers and parameters. Standalone builds, examples, and scripts must work from arbitrary directories without a DAW installation or running window.

Use narrow concrete types for a validated request, reduced positive integer ratio, processor, and typed errors. Exact Rust signatures remain an implementation choice. A request identifies sample rate, channel count, input frames, and ratio; buffers have an explicit layout. One shared time map and linked analysis decisions govern stereo. Unrelated mono processing of L and R is not sufficient.

Use safe Rust for new engine code and permissively licensed Rust dependencies. Record resolved versions, enabled features, licenses, and notices. The standalone crate must not need Rubber Band for its default build or ordinary tests. The combined DAW retains its existing Rubber Band obligations while that backend remains linked.

## Input, timing, and output

Let `N` be source frames and the reduced ratio be `p/q`, where `p` denotes output duration and `q` denotes source duration. One frame contains one sample per channel.

```text
M = floor((N * p + floor(q / 2)) / q)
```

Use checked widened integer arithmetic. `3/2` makes audio 1.5 times as long. Do not derive `M` from rounded seconds or FFT hop counts. Return exactly `M` frames. Fractional-hop scheduling and draining must account for length; do not hide large processing errors by appending silence or truncating events. The DAW adapter verifies compatibility with `ClipStretch::scale` without introducing that type into the library or CLI.

Validate the request before shortcuts. Empty input returns an empty result. Nonempty input that rounds to `M == 0` returns `OutputTooShort`, rather than silently rounding to one frame. Invalid ratios/components, layout, rates, arithmetic, and resource limits return typed errors without publication. The DAW additionally retains its positive clip-length guards.

At ratio 1, preserve every finite input sample bit, including signed zero. No spectral processing or additional delay is permitted. File metadata, WAV encoding, and waveform peaks are caller concerns.

The processor must:

- Reject NaN and infinity. Retain finite headroom without input/output clipping or automatic normalization.
- Produce finite output or return an explicit numerical/resource error. Large finite values must not silently generate nonfinite audio or cause input-dependent panics.
- Preserve pitch within the acceptance limits. Short inputs cannot silently fall back to pitch-changing resampling.
- Preserve mono equivalence, channel order, silence in a silent channel, and fixed channel relationships. Linked analysis must not collapse stereo to mono.
- Compensate analysis/synthesis delay internally so returned frame zero corresponds to source time zero. Preserve source events near both boundaries within the stated tolerances.
- Bound the result to `M`, without an additional audible tail or unexplained leading/trailing silence.
- Be deterministic for identical samples, parameters, processing revision, build, target, and configuration. Cross-platform sample-bit identity is not required; numerical criteria apply on every platform.

Published spectral and time-domain methods may be combined. Method choice does not exempt stereo coherence, transient handling, or short-input behavior. Neither Rubber Band's internal structure nor sample-exact output is a requirement.

## Experimental engine

The engine maintains phase continuity when spectral peaks change and protects detected attacks during expansion. The attack detector uses shared stereo energy. Short regions around detected attacks retain their original speed and phase; gaps absorb the remaining requested duration. Onsets stay anchored to the global ratio, and the exact output-length contract remains unchanged. Compression uses a linear time map with the same phase-continuity rule. See the [independent design record](../crates/timestretch/PROVENANCE.md#phase-continuity-and-attack-preservation) for equations, thresholds, sources, and limits.

The engine applies no automatic gain matching or normalization. Attack protection can reduce average level on sparse percussion as the interval between intact pulses increases. Missed/masked attacks, dense events, source-wide window choice, and cymbal-tail quality remain listening targets.

## Resources and failure handling

This offline API may allocate and block the calling thread; it does not spawn an application worker or initialize an event loop. The caller chooses where to run it. Check frame and buffer arithmetic and use fallible reservation for major buffers where available. Preflight configured resource limits, document scratch-storage bounds, and return errors rather than silently reducing quality. This does not promise recovery from every allocator failure or an OS process kill.

Provide caller-controlled progress/cancellation with bounded checkpoints during validation, analysis, synthesis, and draining. Progress is monotonic. Callers report completion only after publication. The DAW runs this API through its worker path; no decoding, stretching, allocation, locks, logging, or file I/O enters its audio callback or UI painting path.

## Console tools

The following commands are implemented by the standalone `timestretch` binary in `crates/timestretch-cli`. It accepts ordinary audio files and parameters, without projects, DAW crates, DAW configuration, or a DAW installation. The portable library does not depend on this tool.

### Three-engine comparison

`timestretch compare` is the main evaluation command. One invocation must process the supplied file with all three engines; callers do not need to run separate commands:

| Engine ID | Processing configuration |
| --- | --- |
| `timestretch` | Independent Rust engine, current implementation |
| `rubberband-r2` | Rubber Band R2/Faster, offline, linked stereo, pitch factor 1, internal threading disabled |
| `rubberband-r3` | Rubber Band R3/Finer, offline, linked stereo, pitch factor 1, internal threading disabled |

Use an explicit engine selection in the reference API and verify the reported engine version. Never label an R3 result R2 or let either reference silently fall back. Pin the native Rubber Band/binding versions and record all actual options, including defaults, in reports. Keep native reference dependencies and notices isolated from the BSD engine crate. A standard comparison build includes both reference engines. A build lacking a reference fails comparison clearly instead of producing an incomplete success.

```sh
# Command; runs Timestretch, R2, and R3 on the same audio.
timestretch compare --input input.wav --ratio 3/2 --sample-rate 48000 \
  --output-dir comparison
```

| Argument | Contract |
| --- | --- |
| `--input PATH` | Required mono/stereo PCM16, PCM24, or float32 WAV |
| `--ratio P/Q` | Required output/source duration ratio; finite decimal text also parses as an exact rational |
| `--sample-rate HZ` | Processing rate, default 48000; explicit and recorded, independent of DAW settings |
| `--output-dir PATH` | Required new or empty directory for all renders and reports |
| `--fixture-manifest PATH` | Optional versioned synthetic-fixture metadata for known-signal metrics |
| `--timeout-seconds N` | Positive per-engine timeout; default 600 seconds |

Use one DAW-independent decoder/converter path for all three backends. Validate, decode, and convert the complete source before stretching. Preserve native mono/stereo channel count in the tool; the DAW may adapt mono to its stereo bus separately. Each reference adapter receives identical finite samples, rate, ratio, and target-length policy. The command applies no clip fades, effects, pan, normalization, or limiter. Use reference APIs rather than external Rubber Band command defaults.

Write `timestretch.wav`, `rubberband-r2.wav`, `rubberband-r3.wav`, three per-engine JSON reports, and `comparison.json`. Audio is float32 WAV with the prepared channel count and rate, retaining headroom without dither or clipping. Keep raw reference frames/length diagnostics before any documented adapter delay/one-frame length correction; do not hide missing tails or larger discrepancies to force a match. DSP acceptance limits apply to the new engine; a reference's measured quality failure is reported as a reference result, not used to relax a new-engine criterion.

The orchestrator may use isolated child invocations of its own `process` command to measure engine memory and enforce timeouts. Each child may decode independently through the common portable path. Require matching hashes of prepared `f32` input: canonical little-endian sample bits plus rate/channel/frame metadata. Preserve the original file hash separately. No dependency on the repository working directory is permitted.

### Individual processing

Also provide `timestretch process` for diagnostics and benchmark orchestration:

```sh
# Command; individual processing remains available for each engine.
timestretch process --input input.wav --output integrated.wav \
  --engine timestretch --ratio 3/2 --sample-rate 48000 \
  --report integrated.json
```

Require `--input`, `--output`, `--engine`, `--ratio`, and `--report`; use the same rates/formats/ratio rules as `compare`. Accepted engine IDs are the three above. Each engine runs its current implementation. Reports record the processing identity and build metadata. `--overwrite` permits output/report replacement after successful staging, but never source replacement. Backend dispatch is shared by `process`, `compare`, and benchmarks. Do not introduce a `daw-cli stretch` dependency or duplicate the new engine's DSP in tools.

### Scripts and reports

Ship portable Python batch wrappers with the standalone tool sources at `crates/timestretch-cli/scripts/compare.py` and `crates/timestretch-cli/scripts/benchmark.py`. They accept an explicit prebuilt `--binary` and work from any directory. They must not find/build `daw-cli`, assume a DAW project, or download dependencies. Keep external script dependencies explicit and pinned; use argument arrays for child processes.

```sh
# Wrapper interfaces; script paths may be absolute or installed elsewhere.
python3 /path/to/timestretch-cli/scripts/compare.py --binary /path/to/timestretch \
  --input input.wav --ratio 3/2 --sample-rate 48000 --output-dir comparison

python3 /path/to/timestretch-cli/scripts/benchmark.py --binary /path/to/timestretch \
  --input input.wav --ratios 1/8 1/2 1 3/2 2 8 \
  --sample-rates 44100 48000 96000 192000 --warmups 1 --runs 5 \
  --timeout-seconds 600 --output-dir benchmark
```

Comparison reports raw peak/RMS, lengths, nonfinite counts, times, and applicable signal metrics for all three engines, plus pairwise comparisons with each reference. Waveform/null/spectral differences are diagnostics, not universal sound-quality gates. Metrics needing a known synthetic source are unavailable for arbitrary music, never automatic passes. For `--fixture-manifest`, verify the prepared hash against metadata naming equations/seed, channel relationships, events, and metric-profile version. Analysis is separate from backend timing. Raw renders remain available for listening; gain-matched audition files are separate derivatives with disclosed gain.

Benchmark invokes all three engines for every case. Time setup, analysis, synthesis, draining, and output allocation. Separately report decoding, conversion, encoding, startup, analysis of results, and total command wall time; exclude these from engine time. Run measured operations serially with a monotonic timer, rotate all three engine orders, discard warmups, and report median/minimum/maximum/median absolute deviation. Five runs cannot establish a p99. Output per-run JSON and summary JSON/CSV; a single-file run does not certify full-corpus coverage.

Reports include schema version, file/prepared/output hashes, input/output channels and frames, rate, rational ratio, engine ID/revision/native versions/options, source/build identity and dirty state, target/profile, CPU/OS, thread policy, timings, and failure status. Collect peak process resident memory with units/method where supported, otherwise mark unavailable; it is not engine scratch memory. The R3 run remains the initial performance/listening acceptance baseline; report R2 separately, without selecting whichever reference gives the easier pass.

Reject input/output/report aliases, including canonical paths and existing file identities. Refuse nonempty comparison/benchmark destinations. Validate first and stage complete files beside destinations. Publish the success summary last with output hashes. On partial publication, report failure and identify artifacts; multiple renames are not an atomic group replacement. Never overwrite the source, even with `--overwrite`.

Exit 0 means every requested engine completed successfully with matching prepared identities and valid finite/length results; 1 means processing/input/I/O failure or incomplete comparison; 2 means invalid arguments. Missing references, unequal hashes, engine substitution, failed children, timeout, or cancellation fail comparison while retaining diagnostic artifacts. Terminate cancelled children and forward useful stderr. A metric failure must be recorded explicitly; success of ordinary file processing alone does not certify acceptance. JSON must not contain NaN/Infinity numeric tokens. No window, display, audio device, uploads, or telemetry is required. On Windows, executable paths use `.exe` as appropriate.

## Optional DAW integration: existing behavior to preserve

The [current adapter](../crates/media/src/stretch.rs) uses Rubber Band R3/Finer, offline processing, linked stereo, pitch factor 1, and no internal threading. [Session preparation](../crates/project/src/lib.rs) caches complete stretched sources by asset and reduced ratio. Clip source offsets, repeat lengths, and repeat phases refer to prepared-source coordinates.

- Process the original decoded source, never a previous stretched result. Repeated edits compose the saved ratio and prepare from the original.
- Stretch the whole source before taking a clip's source range. Copies, splits, trims, and loops sharing a source must resolve consistent audio coordinates.
- Preserve source files, stable IDs, metadata, colors, selection, timing, overlap resolution, and group-edit rules.
- Publish edits only after release and successful worker preparation. Failure or cancellation retains the complete previous project, prepared audio, and playback.
- Retain the renderer's existing 5 ms boundary fades. The processor adds no undocumented fade, normalization, limiter, or output clamp.
- Convert source audio to the project rate in media before stretching. Device-rate conversion remains separate.

This extends [MVP stretch behavior](MVP.md) and [iteration 1](../specs/mvp-iteration-1.md); existing editing requirements remain in force.

## Optional DAW integration: Project Settings and persistence

Add **Time stretch engine** to **Project Settings**, beside the project audio controls, using shared row/dropdown styles. This is a saved project processing choice, separate from application appearance settings.

| Label | Stable engine ID | Evaluation default |
| --- | --- | --- |
| Rubber Band | `rubberband-r3` | Existing and new projects |
| Integrated (experimental) | `timestretch` | Explicit selection |

Rubber Band R2 is a required standalone comparison reference, not an additional DAW dropdown requirement in this version. The portable engine and command tool do not read or write project settings. The following persistence and cache requirements apply only to the DAW adapter.

Save the engine ID and positive processing revision at project level. Missing legacy fields select the default Rubber Band identity. Behavior-changing algorithm updates require a new processing revision and explicit migration decision; a dependency update must not silently reinterpret a saved revision. Reports also record concrete library/build versions.

Expose the selectable Integrated (experimental) item only after real DSP passes the automated engine/integration gates. Builds without it identify it as unavailable. Unknown IDs/revisions or unavailable backends cause actionable preparation/open errors, retaining the current session and leaving the saved file untouched. Do not silently substitute another backend. Migration of unavailable engines remains outside this version. Each backend accepts only its current processing identity. The normal worker transaction rebuilds buffers when the engine changes and retains the current session on failure or cancellation.

Applying a choice follows the current project audio-settings transaction:

1. Editing the dropdown changes only the dialog draft. Cancel/Escape discard changes; unchanged Apply is a clean no-op.
2. Apply snapshots the session and prepares every required source/ratio for the requested engine through the existing job path. Reuse decoded originals rather than reimporting recordings.
3. Publish the engine, matching audio/peaks, and any other accepted settings together after preparation succeeds. Stop playback for successful replacement; the user presses Play to audition it. An engine-only change preserves playhead, loop, selection, IDs, clip/source/repeat coordinates, ratios, rate, and buffer size.
4. Error or cancellation retains the old engine, all fields, dirty state, playback, and caches. Discard partial output and identify the failing source/cause through existing dialogs.
5. Disable conflicting editing during preparation and reject stale results after session/source replacement. Mark modified only on a successful change.

The choice applies to all audio clips, with no automatic/per-clip engine mixing. Switching back is another explicit transaction. Real-time dual-engine A/B and loudness matching are not part of the selector. Choosing an engine in a MIDI-only or unstretched project saves it for future audio stretches without changing those signals.

### Cache and rendering identity

Prepared audio identity includes source identity/content generation, processing rate, reduced ratio, engine ID, and processing revision. The current `(asset ID, ratio)` key alone is insufficient. Use an explicit key or equivalently explicit session-scoped identity. Never serve one backend's audio under another's identity. Bound retained caches during repeated auditions.

Rate changes convert originals first and rebuild the selected backend. Engine changes retain rate. Source-length mapping, waveform lookup, playback plans, and offline export resolve the same prepared buffer. A missing/mismatched cache must not silently use original or other-engine audio. Missing-source placeholders retain current silence/warning behavior. Save/Open and Save As retain the engine; project CLI rendering uses that saved choice.

## Delivery sequence

1. Define portable types, fixtures, provenance records, independent measurements, and tests that reject deliberately faulty output.
2. Prototype real DSP in `crates/timestretch`. Test short sources and extremes early; meet the engine gate before selectable integration.
3. Add the standalone three-engine command tool and portable scripts; pass its gate without any DAW dependencies. Separately add the DAW adapter, saved identity/caches, and transactional Settings. Keep Rubber Band the DAW default.
4. Complete portable correctness/performance/listening gates and, separately, DAW integration/native checks. Publish failures by signal/rate/ratio. Other projects can use the library/tool without completing DAW-specific gates.
5. Consider a stable standalone release or repository extraction after portable acceptance. Consider changing the DAW default separately, after its integration/native gates. Full acceptance and changing the default require separate validation decisions.

## Research provenance

Maintain [crates/timestretch/PROVENANCE.md](../crates/timestretch/PROVENANCE.md) with papers, equations/methods, independent design choices, dependencies/licenses, and any reused permissive material/notices. Record prior exposure to relevant implementation source honestly. Rust/new-crate status alone does not establish clean-room development. Document a formal source/specification/implementation separation separately if adopted.

Follow the [mandatory independent implementation rule](#mandatory-independent-implementation-rule) for all engine, test, and tool work. Independently generate tests and metrics. Keep Rubber Band reference tooling outside the standalone crate's ordinary build/test graph.

Initial references:

- [Bernardini, De Götzen, and Arfib: Traditional (?) implementations of a phase-vocoder: the tricks of the trade, DAFx 2000](https://www.dafx.de/paper-archive/2000/pdf/Bernardini.pdf): research on windowing, phase, and synthesis; not permission to copy accompanying code.
- [Rubber Band technical notes](https://breakfastquay.com/rubberband/technical.html): behavioral context and bibliography, describing R2 rather than our R3 backend and not specifying every adaptive stretch/coherence detail.
- [WIPO: Copyright Protection of Computer Software](https://www.wipo.int/en/web/copyright/activities/software): distinguishes methods, expression, and patent protection; not clearance for a particular implementation.

Review further primary publications before selecting transient/stereo/multi-resolution methods. Record license/patent questions separately from signal-quality results. Passing acceptance does not establish legal clearance or Rubber Band equivalence.

## Sampler consumer — 2026-10-17

[Sampler](sampler.md) reuses the selected project stretch backend/revision for its prepared region. The project worker first composes the input clip's trim/repeat/stretch mapping and selected region, then prepares one complete mono/stereo buffer at a constant duration ratio. Time sets milliseconds; Bars derives duration from project tempo. The existing positive-frame and 1/8–8 bounds apply, and failure/cancellation retains the original project and audio. This consumer adds no live stretching, ratio automation, pitch shifting, or library algorithm changes. Sampler root note remains metadata until a separate pitch-support design is implemented.
