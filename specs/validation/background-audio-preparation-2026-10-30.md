# Saved audio caches and background preparation — 2026-10-30

Status: implementation and automated validation; native acceptance remains open. See the [stage plan and contracts](../architecture/background-audio-preparation.md).

## Delivered behavior

Desktop opening now validates and displays the project model before media preparation finishes. Verified original PCM/peaks can arrive separately for previews. Play and source-changing operations require complete media; manifest failure retains the previous workspace, while later media failure retains the visible document and an explicit retry state.

Validated MIDI edits enter the editor and history immediately. The prior complete revision can continue playing. Preparation uses one persistent coordinator with bounded latest-request/result/preview slots, generation/revision checks, cancellation, progress and UI wake-ups. Completion merges derived audio only. Ordinary Save captures pending edits; export completes its exact captured snapshot. First Save, Save As and source transactions retain readiness guards.

Desktop and CLI enable exact float PCM/peak caches outside project folders. SHA-256 content/build/processing keys support reopening and repeated pitch reuse; invalid caches rebuild. Memory retention is bounded, disk cleanup targets 4 GiB, and required allocation can release disposable cache ownership. Prepared pitch transforms share two execution slots, with independent-pitch parallelism and serial fallback. These are media caches; DSP seek checkpoints retain their separate exact-history policy.

Neighboring-pitch speculation remains off. Required preparation, recent-pitch retention and saved reuse are implemented. No pitch-tracking option or alternate saved-audio algorithm was added.

## Automated coverage

- Media: exact PCM/metadata/peak round trips, strong memory reuse, concurrent identical-request deduplication, key invalidation, corrupt/truncated/incompatible files, cache write failure, cancellation, and source replacement with restored timestamps.
- Project: manifest-only opening without source decoding, missing-source warnings, failed preparation retaining the original maps/model, latest-request coalescing, stale-result rejection, cancel/retry, worker failure, nonjoining shutdown, serial/parallel bank equality with both engines, and existing Sampler live/export/seek/rate/history contracts.
- A separate-process integration test proves saved decoded/pitch entries survive restart with zero builds and exact PCM. Returning to a dropped pitch uses the cache without transforming again.
- UI: immediate chord/note publication, rapid successive edits, selection/history preservation, paused/failed preparation retaining dirty edits, Undo availability, saving before audio completion, exact pending-snapshot export, readiness guards, and existing pitch-audition behavior.
- Existing engine/output suites continue to check callback allocation/deallocation constraints, transport transitions and live/export processing. No cache I/O or preparation was added to the callback.

## Commands and results

- `cargo test --offline --locked --workspace --no-fail-fast`: **1,949 passed, 11 ignored, no failures**, across 255 suites.
- Final affected-crate/suite reruns pass, including all **742 UI tests**, 13 media unit tests, both document-loading regressions, two-engine serial/parallel bank parity, and the isolated saved-cache subprocess test. The pending-export regression and the two new integration targets were added after the workspace run started and were executed separately. Counts overlap and must not be added.
- The saved-cache regression also verifies cancellation and invalid-engine rejection on the identity/cache fast path; these retain the original media API contract.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, and whitespace checks pass.
- Locked offline CLI build and `python3 tools/check_cli.py` pass: source collection, tracks, WAV/MP3/AAC encoding, repeatability, missing sources, group buses, overwrite and exit codes.
- `sh tools/package_macos.sh debug` passes and rebuilds the unsigned debug app. The app was not launched for native acceptance.
- Relative Markdown file targets are checked across the documentation tree. No dependency versions or lockfile entries change.

Logs remain in ignored `work/background-*` files. Final benchmark data is `work/audio-preparation/results.json`. The synthetic benchmark uses generated audio only.

## Benchmark method

```sh
python3 tools/benchmark_audio_preparation.py --runs 3 --seconds 58
```

The runner builds the release project example, generates a 44.1 kHz mono source processed at 48 kHz, and opens a Sampler chord containing MIDI 60/64/67. Each repetition measures uncached preparation, an empty saved cache, and a fresh process using the saved cache. Additional cases change velocity, introduce pitch 61, return to the original chord, and revisit pitch 61. Root/region duration remains 58 seconds. It measures manifest/model readiness, required media, renderer construction and synchronous shared edit preparation separately; native editor input uses the nonblocking coordinator.

Results include cache hit/build counts, toolchain and source-tree hash. Empty-cache writes add first-use work. Cached and uncached cases use the same current processing code and bounded parallel opening; this is a cache comparison, not a release comparison against the earlier debug diagnostic. The observed maximum across three repetitions is not a stable p95 estimate.

## Release measurements

Measured on macOS-27.0.1-arm64-arm-64bit-Mach-O (arm64, 14 logical CPUs), rustc 1.97.0 (2d8144b78 2026-07-07) (Homebrew), with the workspace release profile. Build/test jobs had finished before the final three repetitions. OS file caches were not flushed; external activity and thermal state were not controlled.

Median milliseconds:

| Operation | Cache disabled | Empty saved cache | Saved cache, new process |
| --- | ---: | ---: | ---: |
| Validated editor model | 0.136 | 0.142 | 0.145 |
| Required audio ready | 3029.458 | 3181.224 | 163.285 |
| Velocity-only edit | 0.043 | 0.042 | 0.041 |
| Pitch 61 introduced | 2025.335 | 2068.970 | 45.360 |
| Pitch 61 revisited | 2013.849 | 0.319 | 0.326 |

Every new-process saved-cache opening records three disk hits and zero cache-managed builds (one decoded source and two shifted pitches). Its pitch-61 edit records one more disk hit and zero builds, because the preceding process saved that pitch. Revisiting pitch 61 in the same process records a memory hit and zero builds. Cache-disabled reference runs do not collect cache-managed build counters. Empty-cache writes add first-use work; they do not accelerate the initial transform.

The validated editor-model timing excludes native presentation and event dispatch. Shared-audio byte snapshots accompany each raw row and include predecessor/cache owners at operation return. They are not peak RSS, total process memory, or native-backend scratch measurements. No physical-device latency or universal millisecond target is claimed.

Source-tree SHA-256: `c0245a6bf1fe67150e210138b9ffd4397468468e299557651061c3dc161c2c4b`. The runner hashes current Rust/Python/Cargo inputs, including untracked source files, and records the full toolchain and each repetition. Documentation-only changes do not affect this source hash.

## Remaining acceptance limits

The native computer-use inventory failed with `Sky Computer Use native pipe startup failed`. Native visual interaction, device-pressure tests, listening, physical-device activation latency and Windows/Linux execution remain unverified. Package builds and egui input tests do not establish native acceptance. The benchmark does not measure process RSS, physical-device underruns, battery/thermal cost or native presentation latency.

The application still decodes complete sources and prepares complete selected Sampler regions. An uncached pitch can take seconds and consume substantial memory. Saved caches are disposable and do not replace original recordings, project source collection or complete-bank publication. No speculative neighboring-pitch benefit is claimed.
