# Coverage, evidence and review gates

[Review index](README.md) · Baseline `2dbb347`

## Method and limits

This is a source-backed architecture review. It maps all workspace packages and major application pipelines, then follows concrete entry/commit/prepare/process/publish/failure paths. Source links and symbol names in the maps/findings support the claims. It does not inspect every line of the repository or certify all possible unit combinations.

No application behavior, dependency, project fixture or private recording was changed. No third-party Rubber Band implementation source was inspected. No app was launched, benchmark rerun, hardware test performed or code test executed for this documentation-only task. Existing evidence is explicitly historical. No new claim of audio parity, native gesture correctness, crash durability or measured performance follows from this review alone.

## Coverage register

| Area | Inspected definitions/paths | What remains unverified |
| --- | --- | --- |
| Package boundaries | Root/member manifests, 27-package direct dependency inventory | Third-party transitive internals/security; independent distribution readiness |
| Startup/platform/CLI | Desktop entrypoint, adapter composition, CLI dispatch, workflows/package ownership | Native Windows/Linux behavior and physical devices |
| Project model | Project/Track/Asset/Clip, validation, `Edit`, batch/placement/trim/resize/MIDI/transfer modules | Exhaustive combinations/invalid manifests beyond existing tests |
| Session and storage | Open, source paths, import, save/new-save, replace helper, export, history | Crash fault injection, network filesystems, concurrent writers |
| UI | Frame/job/poll/sync stages, regular Save, focus/selection/catalogs, shared clip/unit/strip drags, Sampler guard | Native frame/presentation latency; every control's interaction logic |
| Media/cache | Decode/admission, weak cache, prepared stretch/Sampler keys, rate conversion and peak ownership | Native backend allocation totals; concurrent miss stress |
| Independent Timestretch | Request validation/resources, spectral/transient/pitch module boundaries, host wrapper contract | Full perceptual/acceptance matrix and provenance certification |
| Audio preparation | Renderer construction, identity/schedule tables, worker revisions/catch-up, cache replay/warm | Performance under worst-case graph and CPU contention |
| MIDI/modulation | Voice/sample/restart path, shared/local publication, provider order, source index/runtime contract | Exhaustive history equivalence for all route/unit combinations |
| Effects | Runtime dispatch, every effect's processing entry/stages, layer/Main/track/Master placement, latency | Formal DSP analysis and all listening/aliasing/true-peak conditions |
| Output | Device construction, queues, callback update/install/scheduled activation, conversion, retirement, diagnostics | Hardware xruns, audible latency, device disconnect/recovery timing |
| Documentation | Current specs, user-guide index, validation history and historical performance reports | Every historical statement reconciled into a new canonical spec |

## Checks for this handoff

- Verified baseline and clean initial working tree with Git.
- Derived the package inventory from local manifests; checked the normal workspace dependency graph for cycles.
- Cross-checked key symbols/constants against source: queue sizes/command bound, checkpoint/history bounds, cache key fields, memory budget, latency propagation, asset append/removal behavior, save/export publication and cancellation call sites.
- Checked relative Markdown targets and local heading fragments in the review package and changed index links.
- Checked that only intended documentation files changed and ran `git diff --check`.

Workspace builds/audio tests are intentionally not repeated for documentation-only changes. The findings' proposed acceptance tests have not been implemented or run.

## Existing evidence and what it establishes

| Record | Useful evidence | Limit when applying it here |
| --- | --- | --- |
| [2026-10-18 performance audit](../../validation/performance-audit-2026-10-18.md) | Named synthetic workloads, allocation counters, UI/model/media/DSP timings and reproducible harness | Historical baseline before remediation; do not report fixed scans as current failures |
| [Performance remediation](../../validation/performance-remediation-2026-10-18.md) | Indexed scheduling/source lookup, Timeline culling, preparation worker, memory admission and before/after data | Later continuous-playback/cache changes supersede suspension and “no checkpoints” descriptions |
| [Validation log](../../validation/README.md) — continuous playback and seek cache | Revision-safe activation; exact cached/full-history sample comparisons in the tested configurations; native silent interaction checks | Silent native checks do not establish audible dropouts or input-to-speaker latency |
| [Cold seek optimization](../../validation/README.md#cold-seek-optimization) | Recorded worker improvement of roughly 13–15%; longest measured cold preparation about 0.92 s; DSP parity tests; latest recorded workspace result 947 pass, two ignored | Private workload, local hardware, historical run; counts and timings are not new results from this review |
| [Current performance contract](../performance.md) | Current worker/cache/UI/admission rules and remaining measurement gaps | Architecture description, not an additional benchmark |
| [Stretch acceptance plan](../../acceptance/timestretch.md) | Independent correctness/performance/listening gates | Several extended/perceptual gates remain open; specification is not a pass result |

## Proposed validation matrix

All entries below are proposed follow-up work, not results. Use generated public fixtures where possible. Record baseline commit, workload, rate/buffer/device, build flags, hardware, OS, thermal/power conditions and raw samples. Run timing comparisons serially. Keep private media and machine paths out of tracked reports.

| Gate | Workload and method | Report/pass condition |
| --- | --- | --- |
| Latency clock correctness | Each latency effect before Auto-filter; multiple instances; layer/Main/track/Master; nonzero loop start; bypass | Exact expected frame sequence at each processor; parity with explicit delayed-clock reference |
| Asset lifecycle | Repeated note edits, crop/resize, shared clips, Sampler references, delete, undo/redo, clipboard, save/reopen | Current assets equal reachable assets; retained history still works; manifest growth explained |
| First/cached seek | Cold first request, cached nearby, far forward/backward, every view, long arrangement, dense modulation | Separate input→UI, snapshot, worker wait/build/replay, ready→queue, callback activation and audible latency; p50/p95/max |
| Live edit catch-up | Dense Envelope input modulation and latency changes while playing; CPU contention; repeated supersession | Old audio continuous; latest edit eventually activates under declared load; no rewind/stale publication; defined overload response |
| Callback deadlines | Representative and heavy unit chains/voices at all rates/buffers; device-rate mismatch | Alloc/dealloc/lock-free path plus p95/max callback cost; actual xrun evidence separately |
| UI-independent control | Stall painting/main thread deliberately while ready result and retirement accumulate | Bounded retained memory and declared activation behavior; tests before/after coordinator change |
| Memory pressure | Long decoded assets, many Sampler pitches, 100 history steps, checkpoints, concurrent jobs | Retained bytes by category, peak RSS, admission/eviction behavior, cleanup after cancellation |
| Edit classification | Rename/color/collapse/order, knobs, notes, routes, topology, rate/buffer, undo | Only required generations change; no stale audio; no unnecessary plan/cache invalidation |
| File publication | Fail/terminate at each write/sync/rename stage; concurrent destination writer; full disk | Previous valid project or discoverable recovery; no-overwrite never replaces competing file |
| Cancellation | Open, import, transform, bank preparation, settings, export, prefetch | Bounded observation delay, no partial publication, temp/reservation cleanup |
| Waveforms/UI | Long source at widest zoom, many visible layers/units, dense notes, scroll/drag/accessibility | Correct extrema/geometry plus UI CPU, tessellation, GPU/presentation and event latency |
| History semantics | Rate-modulated LFO, modulated ADSR, filter/dynamics history; direct seek versus uninterrupted playback | Exact or documented reset semantics according to chosen policy |
| Export | Full arrangement, near-end impulses, optional tails, integer/float/MP3, high project rates | Shared rendering behavior, codec bounds, clipping/progress/finalization, declared output length |
| Platform lifecycle | Cold start, menu/shortcut ownership, native drag, window scaling, sleep/wake, output disconnect | Native observations for each supported platform; automated UI tests remain separate evidence |

Suggested responsiveness and memory thresholds must be agreed for representative hardware. Do not choose them after seeing results or turn noisy hosted-CI timings into a universal release gate.

## Review worksheet

| Decision | Recommendation | Owner response |
| --- | --- | --- |
| First implementation batch | F01 clock correction, F02 asset cleanup, F08 no-replace publication, plus focused regression coverage | Pending review |
| Transport guarantee | Keep exact mode; explicitly decide whether a quick/reset-history mode is acceptable | Pending review |
| Cache/history budget | Measure retained categories, then choose byte caps and whether memory pressure may shorten Undo | Pending review |
| Output coordination | Measure UI dependence, then move publication/retirement to one control coordinator if the latency target requires it | Pending review |
| Storage guarantee | Define crash recovery and supported filesystem expectations before claiming durable project saves | Pending review |
| Export tails | Decide arrangement-only versus optional bounded tail extension | Pending review |
| Maintenance | Keep this current-state map linked from the architecture index; retain older decision records as history | Pending review |

Acceptance of the review does not require accepting every recommendation. The maps describe the current implementation; the findings supply separately reviewable changes and tests.
