# Multiband EQ spectrum resolution — 2026-11-13

The [effect contract](../effects/multiband-eq.md#response-graph-and-live-spectrum)
replaces the earlier 64-cell, non-overlapping analyzer with 512 measured log cells,
a long decimated bass branch, and a shorter overlapping upper branch. Both
publish about every 22 ms. The existing shared contour and UI-time easing remain.
Audio, latency, parameter IDs, product identities and persistent state are unchanged.
No dependency or license was added. FFT, mapping and halfband code was written
independently from mathematical definitions; no Rubber Band implementation material
was inspected.

## Automated signal and lifecycle coverage

The portable analyzer tests cover all six project rates:

- 40/46, 80/88, 200/220 and 230/250 Hz pairs, two phase choices, at least 6 dB valleys.
- Off-center tones at 20.3, 43.7, 121.3, 242, 317, 997.3 and 12345 Hz, within 3 dB of calibrated amplitude.
- In-phase, opposite-phase, quadrature and independent-channel/weak-tone inputs.
- First complete upper publication within 125 ms and bass within 800 ms; no unavailable short-window bass substitute.
- Warmed onset within 3 dB inside the 150/650 ms upper/bass limits, with 35 ms reserved for shared display easing.
- At least 26 dB decay within the 1.8-second release gate, including old bass-window evidence.
- Analytic cascaded FIR response: less than 0.2 dB ripple through 400 Hz and folded aliases below −80 dB for sampled source frequencies that fold into 20–250 Hz.
- Prepared map contribution caps, no scheduler overruns, at most 80 counted work items per input frame, and less than 600 KiB per analyzer including inline storage.
- Sample partitions 1/17/32/64/128/512/4096, reset, bypass, nonfinite analysis input, missing-copy-service rejection, and logical recovery.
- Pending-job retargeting produces exactly the same next magnitudes and clocks; repeated preparation does not clear active frames.

Heap guards cover sustained monitored processing at all six rates, compatible
inheritance, reset and power changes. PCM equals the detached reference exactly.
Existing engine integration covers track, instrument-layer, Main and Master
bindings, repeated point edits and callback allocation checks. The longer engine
fixtures now wait for both analysis branches. Beat and other discovered scopes
continue through the same unchanged renderer binding; no new dedicated beat
spectrum fixture was added.

Coherent-frame tests exercise 4000 patterned concurrent publications, complete
bin/metadata agreement, bounded retries, cache retention under ordinary contention,
and invalidation before the clear writer enters its payload interval. Prepared
binding/power configuration never writes active frames. Plugin tests cover all
668 fields, full-width u64 clocks, malformed-frame rejection before scalar mutation,
configured rate before first audio, and stop clearing without resetting held peaks.

Shared display tests cover time-based easing, duplicate layout passes, rate and
visibility changes, immediate clears, directly initialized first-ready bass at
60/120 Hz and irregular refresh, bounded cubic geometry, and pixel-column extrema.
The first-ready target is applied on the next tested UI update (at most 41 ms),
within the proposed 50 ms input-to-paint gate. This is a deterministic headless
state/geometry check, not a measured native GPU presentation time.

## Plugin transport

The actual in-process VST3 factory test processes one second at each project rate,
requests schema 2 through SDK connection interfaces, verifies complete ready bass,
passes the frame into the actual controller, and checks legacy/stopped masking.
The upper observation-end age at synchronous polling is below 50 ms. This does
not measure host timer delivery or end-to-end screen latency. Additional shared
adapter tests reject short schema-2 payloads and mask legacy spectrum. Compressor
and Loudness retain their own larger v1 payloads. Existing factory suites verify
parameter/state/PCM and editor contracts for CLAP/VST3. The serialized CLAP stop
hook invalidates spectrum through the shared optional meter hook.

## Release CPU measurement

Reference environment: macOS arm64 host, Rust 1.97.0, optimized release tests.
Exact CPU model was unavailable through the sandbox. The ignored analyzer benchmark
processes one second per case, including publication and 32-frame block timing.
These are analyzer-only costs, not complete EQ/DAW CPU or device deadline results.

| Rate | One instance (% of one core) | 32 instances (per instance) | Worst 32-frame interval at 32 instances |
| --- | --- | --- | --- |
| 44.1 kHz | 0.371% | 0.406% | 273.750 µs |
| 48 kHz | 0.441% | 0.445% | 254.542 µs |
| 88.2 kHz | 0.785% | 0.831% | 276.084 µs |
| 96 kHz | 0.912% | 0.927% | 260.833 µs |
| 176.4 kHz | 1.644% | 1.894% | 308.041 µs |
| 192 kHz | 1.798% | 2.057% | 297.541 µs |

Eight-instance cases also passed. All cases meet the proposed per-instance average
limits of 3% at 48 kHz / 8% at 192 kHz. Large synchronized instance banks can exceed
a 32-frame high-rate interval in the worst measured burst. This benchmark does
not establish an xrun-free instance count; actual device/host capacity remains open.
No performance limit was relaxed.

Command:

```sh
cargo test -p daw-equalizer --release --offline analyzer_cpu_and_32_frame_tails -- --ignored --nocapture
```

## Build and workspace checks

Completed on macOS arm64 with the pinned Rust 1.97.0 toolchain:

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo test --workspace --offline`: passed with debug assertions enabled.
- `cargo test --workspace --release --offline`: passed.
- Final focused Multiband EQ contract/editor/CLAP/VST3 suites: passed, including
  the pre-audio sample-rate regression and actual-factory spectrum transport.
- Native macOS arm64 debug desktop executable build: passed through
  `tools/build_desktop.py debug macos`, using the existing local ORT runtime.
- Documentation relative-link checks and `git diff --check`: passed.

The initial release run used a meter library compiled before the empty-frame
rate correction; the new regression rejected its stale 48 kHz value. The rebuilt
full release suite passes, including that regression. Local scratch logs use
`work/eq-resolution-*`.

## Remaining native coverage

No native playback/listening, graphical visual inspection, actual GPU presentation
latency, CLAP/VST3 host timer delivery, SDK validator, device xrun or Windows/Linux
run was completed here. Native plugin UI/host acceptance remains separate from
headless factory and shared body tests. Source packaging requires a clean committed
snapshot, so no distribution package is published from this uncommitted tree.
