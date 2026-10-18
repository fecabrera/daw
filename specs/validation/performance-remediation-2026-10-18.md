# Performance remediation — 2026-10-18

Follow-up to the [performance audit](performance-audit-2026-10-18.md). The original audit and its JSON retain the historical measurements. Current implementation requirements are in [prepared rendering and performance](../architecture/performance.md).

## Changes by finding

| Finding | Applied change | Remaining scope |
| --- | --- | --- |
| F01 | Cache the prepared arrangement end. | Native long-session verification. |
| F02 | Shared sorted audio/MIDI ranges, cached active/gap windows, bound source buffers. | Native dense-arrangement check at small device periods. |
| F03 | Cull empty waveform viewports; virtualize offscreen Timeline rows with shared placeholders; clone only visible/interactive rows. | Instrument rows and waveform geometry/pyramids remain profiling-led work. The audit did not show an Instrument frame-budget violation. |
| F04 | Shared generation-checked source slots, cached missing sources, and compatible slice fallback. | Native dense-modulation workload. |
| F05 | Worker transport/pre-roll, bounded activation, stale-request rejection, exact indexed MIDI restart, and latency-change suspension. | Native audible seek/loop/edit bursts; initial device creation remains on the caller thread. |
| F06 | Prepared track/effect/delay identity indices; allocation-free hashed route matching with collision checks. | Very large compatible instrument updates still need native timing. |
| F07 | Indexed track/effect/layer/oscillator snapshot lookup; preserve processed values and power semantics. | Active-voice aggregation and 19-slot publication remain; native total callback cost must be measured. |
| F08 | Conditional repaint, worker completion wakeup, coalesced immutable plans, cached queue-full retry. | Native idle CPU/power and initial device-open latency. |
| F09 | Single staged project clone and indexed asset validation. | No change to transactional edit semantics. |
| F10 | Checked shared audio-memory admission, weak decoded-source reuse, cancellable bounded preview workers. | Controlled parallel-job RSS stress; streaming/disk-backed PCM remains a separate architecture task. |
| F11 | Reused conversion buffers, checked exact output capacity, no final full-buffer copy. | Native long-source RSS check. |
| F12 | Cache unchanged Limiter timing coefficients. | Bypass history is retained; broader DSP approximations require separate signal/listening acceptance. |
| F13 | Prepared solo/audibility and gain/pan values in offline mixing. | Native large-session export throughput. |
| F14 | Document exact replay cost and add persistent `render_offline_at` range consumption. | Arbitrary-position DSP checkpoints are not introduced. |
| F15 | Cache idle-voice state and shared release ownership; use indexed layer/source telemetry. | Dense piano-roll and large Sampler-polyphony profiling remain prerequisites for further indexing. |
| F16 | Preallocated callback diagnostics, source hashes, serial before/after comparisons, optional controlled-hardware ratio gate. | No stable native runner is provisioned. Device xruns, GPU presentation, queue bursts, idle power, and Windows/Linux timing remain unmeasured. |

## Measurement method

The [primary results](performance-remediation-2026-10-18.json) compare serial release measurements with the unchanged [audit baseline](performance-audit-2026-10-18.json). The same Apple M5, 16 GB machine, macOS 27.0.1, pinned Rust 1.97.0, lockfile, generated fixtures, and workload sizes are used. The working tree contains the remediations; the report records its source-tree hash separately from the unchanged base commit. No private recordings are used.

The run contains 221 cases: 217 match baseline names and four measure prepared transport activation. Timings are medians unless marked p95. Most cases have seven repetitions; the JSON records each case's actual count and all samples. The [additional DSP run](performance-remediation-dsp-2026-10-18.json) uses 21 repetitions to check variation. CPU scheduling, background activity, thermal state, and power mode are not controlled. Small timing differences are not reliable evidence of a speedup.

The added activation cases time a batch of 1,024 calls with black-box inputs and consumed results. They isolate the final prepared transport-state transfer. They exclude preparation, queues, retirement, conversion, snapshots, and the rest of the native callback. The existing `callback_seek_with_limiter` case name remains for comparison; it still measures the synchronous engine API, which native output no longer invokes on the callback.

Allocation counters cover Rust allocations and deallocations on the measured thread. Cumulative allocated bytes are allocation traffic, not retained memory. Native allocations and other threads are not counted. Peak process resident memory covers the complete synthetic run and has no per-case attribution. These generated repeated-source fixtures do not establish memory use for long recordings or concurrent jobs.

## Results

| Workload | Baseline median | Current median | Current p95 |
| --- | ---: | ---: | ---: |
| Dense audio, eight tracks × 1,024 clips, last clip, 192 kHz, 512 frames, Loop off | 6.295 ms | 0.042 ms | 0.045 ms |
| Timeline, 128 tracks × 64 clips, including CPU tessellation | 50.810 ms | 4.191 ms | 4.335 ms |
| 128 modulation sources/routes, 4,096 frames | 31.964 ms | 2.153 ms | 2.161 ms |
| MIDI restart into 16,384 notes plus first 32 samples | 1.003 ms | 0.035 ms | 0.036 ms |
| Compatible inheritance of 2,048 reordered routes | 0.789 ms | 0.090 ms | 0.091 ms |
| Publication of 1,152 modulation targets | 0.258 ms | 0.044 ms | 0.046 ms |
| Validation, 2,048 assets and 2,048 clips | 1.736 ms | 0.151 ms | 0.155 ms |
| Offline audio block, 128 tracks | 3.205 ms | 0.521 ms | 0.740 ms |
| Eight active MIDI voices, 128 reserved slots | 0.337 ms | 0.257 ms | 0.292 ms |
| Four-second PCM24 export, eight tracks, 48 kHz | 24.547 ms | 13.065 ms | 13.926 ms |
| Four-second PCM24 export, eight tracks, 192 kHz | 95.914 ms | 52.711 ms | 53.275 ms |

The dense-audio case now fits below half its nominal 2.667 ms buffer duration. Timeline CPU tessellation fits the proposed 16.67 ms frame target. These are local headless results, not native callback or presentation measurements. The eight-saw-voice EQ/Compressor target passes all 48 rate/buffer combinations; worst p95 utilization is 13.43% at 192 kHz/256 frames, versus 14.55% in the baseline. That small difference does not establish a separate speedup.

At 128 fixed routes, increasing unrelated providers from one to 128 now changes the 4,096-frame median from 2.025 to 2.153 ms. The old source scan changed it from 1.409 to 31.964 ms. Prepared activation batches of 1,024 calls take 1.667–1.708 µs median and at most 1.792 µs p95 across the four added cases. Only the isolated final transfer is measured by those activation cases; full transport latency is not established.

Timeline allocation calls fall from 23,784 to 3,841 per measured frame; cumulative allocated bytes fall from 45,866,451 to 20,576,256. Ten-second 48→96 kHz conversion falls from 2,865 to 47 allocation calls and from 36,140,758 to 7,833,430 allocated bytes. Its median CPU time changes from 6.330 to 6.006 ms; the robust improvement here is buffer reuse, not an established CPU speedup. Complete-run peak resident memory is 90,193,920 bytes, about 86.0 MiB, versus 353.7 MiB at baseline. This does not verify the 2 GiB admission policy under long-source stress.

Measured steady DSP, route resolution, rendering, inheritance, telemetry, prepared activation, and preallocated device conversion retain zero counted Rust allocations and deallocations. Convenience random access still constructs/replays a renderer and allocates; use the persistent offline API for successive ranges.

### Costs and measurement variation

Prepared tables trade setup work for bounded repeated work. The report retains regressions as well as improvements:

- Preparing the 128-track/64-clip renderer increases from 2.109 to 2.273 ms and from 9,998 to 18,449 allocation calls. Preparing 128 MIDI slots increases from 40.958 to 47.584 µs. Native output performs this work on its preparation worker.
- The 128-route handoff increases from 3.166 to 5.167 µs while the 2,048-route handoff falls by almost nine times. Small route lists keep bounded direct matching; larger lists pay hash-table setup to remove quadratic growth.
- Publishing 72 targets increases from 1.709 to 2.500 µs. The gain is at larger target counts; publication still needs native callback profiling.
- The one-provider/128-route resolver increases from 1.409 to 2.025 ms over 4,096 frames. Shared generation checks add fixed overhead while removing dependence on unrelated provider count. The one-provider/one-route case is approximately unchanged.
- Validation adds one allocation for its asset index. PCM24 export adds small prepared-plan allocations even as elapsed time falls. These are control/worker costs, not callback allocations.

Direct DSP measurements vary between runs. The final primary 192 kHz EQ median is 3.314 ms versus 3.235 ms at baseline; Limiter is 1.115 ms versus 1.202 ms. The separate 21-repeat run gives 3.285 ms for EQ and 1.518 ms for Limiter. That Limiter result is about 26% slower than baseline, despite the faster primary result with the same source hash. It remains an unresolved timing variation that needs a controlled comparison; this report does not dismiss it or select the faster run as proof of improvement. No broad DSP speedup or bypass shortcut is claimed; caching unchanged Limiter coefficients retains the exact equations and required history.

## Automated verification

The workspace test suite passes: 856 tests pass and the two existing expanded time-stretch matrices remain ignored. Subsequent focused checks cover the final route matching, output preparation, import-cache call sites, project workflows, and UI. The complete matrix retains live/offline parity, modulation scopes and ramps, effect bypass/history, source/stretch mapping, and transactional editing checks.

New focused checks cover sorted range boundaries and gaps, MIDI restart against full event replay, shared Legato/Retrigger reconstruction, source-index invalidation and local precedence, route hash collisions/removals/order, prepared transport activation without allocation at all six rates, stale/superseded output requests, latency-change suspension, full retirement queues, admission arithmetic and cancellation, conversion endpoints, decoded-source reuse/invalidation, and persistent offline range rendering.

Commands and final results are recorded in the [validation index](README.md#performance-remediation--2026-10-18). No dependencies or project format versions change.

## Remaining verification and conditional work

No native application was launched. Listening, hardware callback deadlines/xruns, GPU presentation, physical device failure, idle power, long-session/parallel-job resident memory, native gestures/display scaling, Windows, and Linux remain unverified. Headless tests and nominal duration comparisons do not close those acceptance gates.

The finding table distinguishes implemented changes from conditional proposals. Instrument row virtualization, waveform pyramids/geometry caches, dense piano-roll indexing, full active-voice lists, disk-backed PCM, and arbitrary-position DSP checkpoints are not implemented. They require measurements of the relevant workload or a separate architecture decision. Initial native device opening also remains on the caller thread. The shared memory budget covers registered audio and known scratch, not total process memory; the source cache uses file metadata rather than a content hash.

