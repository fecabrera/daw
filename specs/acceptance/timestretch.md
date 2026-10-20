# Timestretch acceptance plan

Status: acceptance contract with initial implementation evidence, 2026-10-14. This is the verification contract for the [functional specification](../audio/timestretch.md). The engine, selector, tools, and initial automated tests are implemented; see the [validation record](../validation/README.md#experimental-timestretch-engine-and-comparison-tool--2026-10-14) for actual coverage. No full-gate or listening acceptance is implied. Numeric limits are initial project acceptance targets, not measured results or claims about Rubber Band.

## Policy

Separate portable engine correctness, standalone tools, performance, listening quality, and optional DAW integration. Passing one does not establish another. Engine/tool acceptance must be runnable without the DAW. Keep Rubber Band as the DAW default during evaluation. Compare the new engine with both Rubber Band R2 and R3; no sample-exact match is required.

Stable case IDs below must appear in tests or reports. Failed, unavailable, timed-out, and skipped required cases prevent the corresponding gate from passing. Keep original failing reports. Threshold changes require a recorded reason and rerun; do not weaken them solely to fit output. Record specification/metric versions, fixture hashes/seeds, and exact builds.

## Independent implementation gate

The [mandatory independent implementation rule](../audio/timestretch.md#mandatory-independent-implementation-rule) applies before every experimental release, DAW exposure, and stable release. **Under no circumstances may the new implementation take code directly from Rubber Band.** Copies, translations, ports, adaptations, mechanical rewrites, and indirect reuse through AI tools or intermediary projects are prohibited. This includes Rubber Band implementation-specific pseudocode, tests, comments, and tables. Signal quality, performance, attribution, and licensing offers do not waive this project requirement.

| ID | Review and pass condition |
| --- | --- |
| IP-01 | Review the engine, tests, tools, dependencies, and provenance records. Each implemented method has a documented independent research/design basis; reused permissive material has its source and notices. No prohibited Rubber Band material is incorporated. Record prior source exposure and review limits honestly. Missing provenance or prohibited material blocks acceptance until resolved; automated similarity checks alone cannot establish independent authorship. |
| IP-02 | Rubber Band use is confined to separately licensed reference/backend dependencies called through public APIs by independently written adapters. No Rubber Band code or dependency enters the BSD-2-Clause engine's build/test graph. Keep reference notices and verify the boundary in resolved dependency graphs. |

## Fixtures and matrix

Generate synthetic signals from equations with fixed seeds. Track generators/metadata; put large WAVs, benchmark artifacts, and user media in ignored `work/` or temporary directories. Real fixtures need documented rights, attribution, hashes, and redistributable files or explicit local setup. Missing listening material leaves that gate incomplete.

| Dimension | Cases |
| --- | --- |
| Rates | 44100, 48000, 88200, 96000, 176400, 192000 Hz |
| Ratios, output/source | 1/8, 1/4, 1/2, 2/3, 9/10, 1, 10/9, 3/2, 2, 4, 8 |
| Short frame lengths | 0, 1, 2, 7, 31, 255, 256, 257, 1023, 1024, 1025, 4095, 4096, 4097; additional non-hop-aligned lengths |
| Musical durations | 2 s, 10 s, 60 s; optional 10 min resource stress |
| Channels | Mono; dual mono; anti-phase; one silent channel; proportional channels; unrelated/delayed stereo |
| Amplitudes | Silence; -60, -24, -12, -3 dBFS; finite headroom above unity; NaN/Inf error fixtures |
| Synthetic material | Sines, detuned tones, harmonic stacks/chords, band-limited sweeps, impulses, click trains, fixed-seed noise, decaying drum-like bursts |
| Real material | Two independent excerpts each: drums, bass, sustained keys/strings, dry speech/singing, reverberant vocals, full mixes |

Run the full rate × ratio × short-length × channel product for layout/length/finite tests. Run calibrated tonal tests at every rate/ratio, using 55, 110, 440, 1000, and 5000 Hz. Use a documented pairwise matrix for additional amplitudes, delays, and complex signals. List actual cases; a test-suite name alone does not establish coverage.

Ordinary range means duration ratios 1/2–2 inclusive; extreme means the rest of 1/8–8. Wider stated extreme tolerances do not permit pitch-changing fallback. Tone metrics require at least 20 input cycles and 20 cycles in the middle 50% of output; extend the fixture when needed. Short cases outside this domain receive separate length/stability/signal-presence tests, not invented pitch results.

## Independent measurements

Metrics must not reuse the engine's phase, onset, analysis, or time-map implementation. Calibrate them with known outputs and intentional defects first.

| Measurement | Definition |
| --- | --- |
| Length | Independent checked rational frame calculation, with channel/layout checks |
| Pitch | Interpolated spectral estimator or sinusoid fit; cents = `1200 * log2(measured_hz / source_hz)` |
| Proportional stereo residual | For `R=a*L`, `20*log10(rms(R-a*L) / max(rms(L), 1e-12))`; zero residual is negative infinity/pass |
| Level | Difference in steady-state RMS, in dB, without gain matching |
| Event time | Independent energy-envelope detector versus rationally mapped source positions; report confidence and unmatched events |
| Transient width | 10%–90% cumulative energy in non-overlapping event regions; compare with the larger of original and ratio-scaled original width |

Steady-state measurements use the middle 50% of output. Preserve full-file audio for boundary/listening checks. Use isolated Hann-tapered noise bursts with 2 ms attack and 8 ms decay at 20%, 50%, and 80% of a sufficiently long source for transient tests. Define onset at 10% of the local energy-envelope peak. Apply the same detector to source/output. The analyzer implementation must define envelope window, event regions, and tie-breaking in a versioned metric profile before evaluating the engine.

Waveform difference, spectral error, crest factor, correlation, and integrated loudness on arbitrary music are diagnostics, not general perceptual pass criteria. Different durations and valid algorithm differences make sample/null matching to Rubber Band unsuitable as a universal gate.

## Engine gate

| ID | Test and pass condition |
| --- | --- |
| TS-01 | Validate rates, channels, ratios/components, layout, and finite input before processing, including identity. Invalid requests return typed errors without input-dependent panics or partial publication. |
| TS-02 | Exactly `M` frames for every valid matrix case. Empty input returns empty output; nonempty input rounding to zero returns `OutputTooShort`. Test half-frame rounding and overflow without allocating huge files. |
| TS-03 | Ratio 1 with pitch 0 preserves finite sample bits, including signed zero, at every rate and both channel layouts. |
| TS-04 | Silence stays exactly silent and accepted output is finite. Very large finite fixtures complete finitely or return an explicit numerical/resource error. Moderate headroom fixtures succeed without clipping/normalization. |
| TS-05 | Qualifying sine pitch error is at most 5 cents ordinary / 15 cents extreme. Inspect harmonic-stack spectra separately for lost/duplicated partials; include them in listening. |
| TS-06 | Qualifying proportional stereo residual is at most -80 dB for `a=0,0.5,1,-1`. Anti-phase does not collapse to silence. Silent-channel peak is at most 1e-7 with active-channel peak 0.5. Swapping channels swaps output within 1e-6 absolute sample error. Mono processing agrees with either channel of dual-mono processing within 1e-6 absolute error. |
| TS-07 | Preserve unrelated/delayed channel identity. On qualifying 440/1000 Hz stereo phase fixtures, interchannel phase error is at most 5 degrees ordinary / 15 degrees extreme. Align common delay only, never each channel separately. Report broadband width/correlation for listening. |
| TS-08 | Steady-state tone RMS differs by at most 0.5 dB ordinary / 1.5 dB extreme. Doubling input amplitude doubles output within -60 dB normalized residual on unclipped tones; no hidden gain matching. |
| TS-09 | Synthetic onset displacement is at most 5 ms ordinary / 15 ms extreme. Energy width is at most 2× ordinary / 4× extreme the defined reference width. Preserve isolated event order/count; unmatched expected events fail. |
| TS-10 | Boundary probes at 0%, 1%, 99%, and the final source frame have exact extent and finite output. For separable events surviving output sampling, detected energy is within TS-09 timing tolerance of the mapped boundary and within a factor of 10 of an equivalent interior probe's energy. Compression-merging events must not be reported as separately measurable. |
| TS-11 | Same request/build/target gives identical samples across repeat calls and fresh processes. No previous-request state or nondeterministic seed changes output. Other platforms meet numerical limits; identical hashes across platforms are not required. |
| TS-12 | Short/non-power-of-two/extreme cases meet length, stability, and signal-presence criteria without pitch-changing fallback. Nonzero constant input with `M>0` gives nonzero output. Add a sustained burst shorter than the chosen analysis window but long enough for a reliable frequency fit; its pitch error is at most 15 cents at extremes. A one-frame signal has no meaningful pitch criterion. |
| TS-13 | Cancel during validation, analysis, synthesis, and draining without publishing partial audio. On the recorded reference machine, cancellation is observed within 250 ms of request; inspect bounded checkpoints structurally. Hosted-runner scheduling delays are reported separately. |
| TS-14 | Preflight arithmetic and resource errors are explicit. Inject reservation/numerical failures without exhausting the real machine. Stress runs obey declared timeout and scratch-memory budgets. |
| TS-15 | Analyzer self-tests reject octave shifts, channel reversal, mono collapse, delay, wrong length, gain doubling, truncated tails, NaN, and an all-silence fake processor. Known correct controls pass. |
| TS-16 | A minimal external Rust consumer processes sample buffers using only the `timestretch` dependency and portable parameters. Inspect resolved dependencies and build/test it without DAW crates, configuration, project files, native C/C++, or Rubber Band. No UI/event-loop initialization or working-directory assumption occurs. |

TS-10/12 must not exempt ordinary audible clips from pitch checks. Requirements apply to the engine's actual output before DAW clip fades, gain, or effects.

## Offline pitch extension gate

TS-01–16 retain their pitch-0 meaning. The new portable pitch path adds the following checks; this does not grant stable release or DAW pitch acceptance. Ordinary combined cases have both duration and pitch factors in 1/2–2. Other supported combinations are extreme. Qualifying tones still need at least 20 cycles in the middle half, and their expected shifted frequency must be below the filter transition band/output Nyquist. Near-cutoff attenuation is a separate filter measurement, not a waived general pitch error.

| ID | Test and pass condition |
| --- | --- |
| TP-01 | Finite fractional −127..=127 semitones, default 0; invalid values return `InvalidPitch`. Existing request API remains usable. Zero pitch equals the original path sample-for-sample, including signed-zero identity. |
| TP-02 | Output frames equal the original rational `M` at every rate and selected duration/pitch endpoints. Cover combined private 1/32 and 32 staging, short/empty/DC/silence, and typed zero-rounded output/resource errors. |
| TP-03 | Qualifying shifted tones meet 5-cent/0.5 dB ordinary and 15-cent/1.5 dB extreme limits without normalization. Cover all six rates, positive/negative/fractional offsets, low/mid/high tones, and combined duration extremes. |
| TP-04 | Stereo proportional/anti-phase/silent-channel, mono equivalence, channel swap, and determinism retain TS-06/11 limits. Unrelated channel frequency and fixed same-tone phase relationships remain intact. Headroom is not clipped; numerical failures are explicit. |
| TP-05 | Independently generated above-Nyquist content is attenuated by at least 70 dB in the isolated +12 semitone/downsample-by-two path. Check passband tones separately. No length-changing fallback or hidden gain matching. |
| TP-06 | Monotonic progress and cancellation through validation, stretching, table setup, and resampling. Major simultaneous buffers are preflighted against the same configured memory limit; overflow and injected small budgets fail before publication. |
| TP-07 | Standalone `process`, `compare`, and wrappers preserve requested pitch identity, duration, rate/channels, source safety, and exit codes. All three engines receive the same factor. Known-tone metrics accept a correct shift and reject an unshifted result; zero remains compatible. |
| TP-08 | External portable consumer and reference-disabled core/tool builds retain the independent dependency boundary. Document the equations, independent interpolation source, table design, revisions, and no-formant/no-live-processing limits. Listening/full-corpus performance and cross-platform acceptance remain separate. |
| TP-09 | Full MIDI-span requests use bounded passes whose offsets sum to the request, with the duration ratio applied once. Check ±127 endpoints, fractional multi-pass offsets, all six rates, linked stereo, exact duration, finite output, monotonic cancellation, and peak simultaneous memory admission. Qualifying wide-shift tones at ±36, ±96, and ±127 have at most 15 cents error and 3 dB steady-state level error. This separate wide-shift envelope does not relax the existing ≤24-semitone limits. |
| TP-10 | Sampler maps every MIDI note 0–127 relative to every valid root without ±24 clamping. Prepare required variants with the selected project backend before publication; failure/cancellation retains the prior session. Verify note/root, duration, rate and engine changes, save/reopen, playback/export parity, and allocation-free note starts, seek and compatible renderer updates. Both native reference engines report each pass for wide CLI comparisons. |

## Optional DAW integration gate

These checks apply to the DAW adapter, not to other consumers of the engine/tool. Reuse transactional edits, workers, and egui tests. Run audio-edit scenarios against the two selector backends (Timestretch and R3). R2 is a standalone reference, not a required selector item. Inject failures through test seams, not user recordings.

| ID | Test and pass condition |
| --- | --- |
| TI-01 | Each backend accepts only its current processing identity. New projects and projects with no saved engine select Rubber Band R3. Save/Open and Save As retain explicit Timestretch ID/revision. Unknown/unavailable choices error without substitution, current-session replacement, or saved-file mutation. |
| TI-02 | Project Settings dropdown labels are exactly Rubber Band and Integrated (experimental), mapping to `rubberband-r3` and `timestretch`. Use shared styles in both themes and mouse/keyboard navigation. Drafts do not process audio. Cancel/Escape and unchanged Apply preserve project, caches, playback, and dirty state. |
| TI-03 | Applying a choice rebuilds all required source/ratio buffers from originals, publishes settings/audio/peaks together, stops playback on success, and marks modified. An engine-only change preserves coordinates, IDs, selection, loop, playhead, ratios, rate, and buffer. |
| TI-04 | Failure/cancellation partway through multiple sources retains old engine, complete project/caches, dirty state, and active playback. Stale workers cannot publish after replacement. Combined engine/rate/name changes remain atomic. |
| TI-05 | Source generation, rate, ratio, engine ID/revision affect cache identity. A→B→A never serves B as A. Equivalent clip requests share prepared audio; source/revision changes invalidate stale entries. Repeated switches retain bounded memory. |
| TI-06 | Split, ordinary/shared-boundary trim, header loop, copy/paste, drag duplication, group stretch, and directional overwrite retain existing coordinates/source rules. Restretch uses the original. Cancellation/no-op edits stay clean. Extend existing group-stretch tests. |
| TI-07 | All project-rate changes rebuild the selected backend. Missing-source placeholders, empty projects, unstretched clips, mixed audio/MIDI, and future imports retain expected behavior. MIDI content never enters this audio stretcher. |
| TI-08 | Source-length lookup, waveform peaks, playback, and float32 export resolve identical prepared audio. The same snapshot renders sample-identically across output block sizes after identical fades/effects. |
| TI-09 | Existing callback allocation/nonblocking tests pass; no preparation runs in the callback or painting. Project rendering/export remains headless and device-free. |
| TI-10 | Seek, Stop, pause/resume, transport looping, reopen, and export retain engine identity. Offline project export ignores transport looping as before. |
| TI-11 | Constant audio clip transpose uses the selected engine and shared pitch preparation. Preserve clip/prepared duration, trim/repeat mapping and source files. Distinct pitches get distinct cache identities; matching requests share buffers. Stage audio pitch drags until release and publish complete worker results atomically. Cover cancellation/failure, source/rate/engine invalidation, save/reopen, history, waveform/playback/export parity and callback heap guards. |

Native Settings navigation, job progress/cancellation, successful-switch stop behavior, and playback continuity after failed Apply need manual checks on macOS, Windows, and Linux. Automated UI/render tests do not establish those native results.

## Console tools gate

The [console interfaces](../audio/timestretch.md#console-tools) are implemented interfaces. Their examples alone are not completed checks.

| ID | Test and pass condition |
| --- | --- |
| TT-01 | One standalone `timestretch compare` invocation runs Timestretch, R2, and R3 with identical parameters and produces all three float32 WAVs/reports plus a summary. `timestretch process` supports each explicit ID, all rates/ratios, and reduced rational reporting. Preserve mono/stereo channel count and unclipped headroom. |
| TT-02 | Equivalent decimal/fraction ratios produce identical output. Invalid ratios, rates, IDs, options, or path combinations return exit 2; decode/processing/I/O errors return 1; complete success returns 0. |
| TT-03 | Paths with spaces/metacharacters are literal on each platform. Identical/symlink/hard-link source aliases cannot be overwritten, including with `--overwrite`. Before-publication failures preserve destinations. Partial publication returns failure with artifacts identified; report/output hash mismatch is never accepted. |
| TT-04 | All three backends receive identical prepared hashes/channels/rate/ratio/expected frames. Verify R2/R3 native engine identities and options explicitly; test detection of a mislabeled/substituted engine. Unequal preparation, any missing backend, crash, timeout, cancellation, invalid output, or nonzero exit prevents success and retains diagnostics. |
| TT-05 | Versioned JSON includes reproducibility fields, units, failures, and explicit unavailable metrics. No invalid JSON NaN/Infinity tokens; encode exact negative-infinite dB as a documented sentinel/string. CSV retains case/engine/run identity and failures. |
| TT-06 | Known synthetic fixtures receive calibrated pitch/time/stereo analysis. Arbitrary files receive applicable diagnostics only, without inferred quality passes. Preserve raw audio for listening. |
| TT-07 | Benchmark runs all three engines, measures specified stages, rotates execution order, discards warmups, and summarizes every case/run. A fake child with known timings validates aggregation. Missing RSS is unavailable, never zero. Keep paired R2/R3 comparisons separate and R3 as the fixed initial acceptance baseline. |
| TT-08 | Run the binary and Python scripts from an unrelated temporary directory with no DAW installation, configuration, projects, graphics, or device. Use the supplied binary without building/downloading anything. Inputs stay unchanged; nonempty destinations are refused. |
| TT-09 | Inspect the CLI's resolved dependency graph: no direct/transitive `daw-*` dependency. Build the engine/tool packages without desktop/CLI applications. Test a relocated/extracted standalone package setup with its documented dependencies and no repository-relative runtime paths. Native prerequisites belong only to reference-enabled tooling. |
| TT-10 | Standard comparison builds contain both reference engines. A deliberately reference-disabled build can process Timestretch but must fail a three-engine comparison rather than skip references. Ship native/reference notices with tooling; the engine's ordinary build/tests remain free of reference dependencies. |

## Performance gate

Use release builds on one recorded, otherwise idle reference machine. Record CPU, RAM, OS, target, compiler, build flags, source revision, thread policy, fixture hashes, and library versions. Set that machine record before evaluating results. Hardware changes require a new baseline; debug results are diagnostic only.

Each of Timestretch, R2, and R3 gets one warmup and five measured runs per case. Rotate all three engine orders and run serially. Use a 60 s stereo fixture from each real-material class at all six rates and all listed ratios. Report every case and both references separately. Engine time includes setup, analysis, synthesis, draining, and output allocation. Processing factor is `engine_elapsed_seconds / input_duration_seconds`; also report output duration.

Initial release-candidate budgets:

- Ordinary ratios: median processing factor at most 1.0 and median engine time at most 2× the paired R3 run. R2 is also reported, without changing the reference used for the pass decision.
- Extreme ratios: median processing factor at most 4.0 and median engine time at most 4× the paired R3 run; report R2 separately.
- Engine-accounted scratch storage, excluding immutable input and final output, at most `64 MiB + 16 * (input_frames + output_frames)` bytes per case. Record allocation accounting and buffer lifetimes. Process RSS is independent and cannot alone establish the scratch bound.
- Cancellation meets TS-13. Stress runs have a declared timeout, initially 10 minutes per case; timeout is a failed case.

These are budgets to test, not predicted performance. Record the first measured baseline before proposing an explicit budget revision. Regular CI enforces correctness and bounded synthetic smoke cases. Wall-time/memory budgets run on a designated benchmark host, not noisy hosted runners.

## Listening gate

After automated correctness passes, use the twelve rights-cleared excerpts above, each 10–30 s, at every non-identity listed ratio. Run the complete panel at 48 kHz and difficult representative cases at 44.1/192 kHz. Objective tests still cover every rate.

Keep raw files for Timestretch, R2, and R3. Create separate audition files using one disclosed constant gain per file, matching integrated loudness within 0.2 LU where measurable. No limiting/clipping; use common attenuation if needed for float playback. Randomize hidden A/B/C labels and retain the seed, mapping, matching gains, and notes. Supply the original for pitch/timbre reference. Record new-engine comparisons against both references separately.

At least three listeners independently rate each three-engine set. Use an impairment scale: 5 = no objectionable artifact, 4 = slight artifact, 3 = audible but usable, 2 = distracting, 1 = unusable. Record an overall score and applicable dimensions: transients, tonal stability, vocals, stereo, boundaries. Mark inapplicable dimensions rather than inventing scores. This panel establishes acceptance for the stated use cases, not statistical equivalence.

Initial promotion requirements:

- Ordinary cases: each case's median overall score at least 4; Timestretch trails R3 by no more than one point on any applicable dimension's median. Publish the same comparison against R2 as diagnostic evidence; do not choose the easier baseline after listening.
- Extreme cases: each case's median overall score at least 3; objective pitch/stereo requirements still apply.
- Repeatable dropouts, reversals, material pitch errors, lost events, or severe boundary defects block promotion regardless of average score.

An experimental standalone release needs automated engine/tool gates and an experimental status; a stable standalone release additionally needs performance, full listening acceptance, and portable checks on macOS/Windows/Linux. DAW-specific gates are not prerequisites for other consumers. Exposing the experimental DAW item also needs its integration gate. Changing the DAW default additionally needs native DAW checks on all three platforms and an explicit validation decision. Missing material/listeners mean incomplete acceptance, not approval.

## Test locations and evidence

| Location | Responsibility |
| --- | --- |
| `crates/timestretch/tests/` | Portable DSP/request/resource tests and independent generators; no Rubber Band dependency |
| `crates/timestretch-cli` tests and `scripts/` | Independent WAV preparation, R2/R3 identity, three-engine orchestration, paths, metrics, reports, and benchmarks |
| `crates/media` tests | Dispatch, decoding, float output, metadata/peaks, adapter errors |
| `crates/project/tests/workflow.rs` | Saved identity, atomic preparation, caches, Save/Open/Save As, export parity |
| `crates/ui/src/tests/` | Dropdown/apply/cancel, workers, group edits, both themes |
| `apps/cli` tests | Optional DAW project validate/render/export regression checks; no dependency from standalone tooling |
| Ignored `work/timestretch-*` | Local renders, benchmarks, corpus metadata, and listening evidence |

Run portable package tests first, then applicable formatting, strict Clippy, independent package builds, reference/tool tests, and platform checks. Only DAW adapter changes require DAW workspace/CLI smoke, packaging, and native integration checks. The core crate must test without Rubber Band/C++/libclang; reference-enabled tooling may need native prerequisites. Record commands, artifacts, gates, coverage, and unresolved cases in [validation](../validation/README.md) while housed here; standalone release evidence must remain usable after extraction. The validation record supplies implementation results and limits.
