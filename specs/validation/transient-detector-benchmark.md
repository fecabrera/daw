# Combined transient detector benchmark — 2026-11-08

Historical opt-in v1 measurement. Revision 5 subsequently adopted a refined
combined detector by user request; see [R5 audition preparation](transient-listening-r5.md).
The measurements and decision below describe the earlier implementation.

The combined detector is implemented as an opt-in portable stretch path. Default
stretch/pitch processing and saved DAW projects retain the revision-4 energy detector.
No new dependencies were added. This is a synthetic comparison, not full stretch
acceptance, a listening assessment or a comparison with Rubber Band.

## Method

- Parent commit: `91d4e8110e209d9ebbd1272fe8f25f25f8aeac9d`, with this uncommitted implementation.
- Local host: arm64 macOS, Darwin 27.0.0; Rust 1.97.0 (2d8144b78), LLVM 22.1.6, release profile. CPU model was unavailable through the sandbox.
- Seven independently generated one-second cases: silence, sustained bass dyad, 440 Hz vibrato, isolated clicks, decaying noise bursts, 35 ms-spaced bursts, and quiet noise attacks under sustained bass.
- Fixed xorshift32 seed `0x7ac39d21`; input/output fingerprints use FNV-1a-64 over little-endian f32 bits (reproducibility identifiers, not cryptographic integrity).
- Detection matrix: six project rates × seven cases × mono unity / anti-phase stereo at 0.01× and 2× gain = 126 paired inputs (252 CSV rows).
- One warm-up and five timed calls per mode/input; order alternates in detector measurements. Timings include validation, FFT planning, allocations and scanning.
- Interior matching uses one-to-one nearest unmatched annotations within 10 ms, retaining the earlier annotation on ties. Detections in the first 50 ms are excluded as known signal-start boundaries. All annotated events are at or after 100 ms. No ending exclusion is applied.
- Rendering diagnostics: three cases at 48 kHz mono, ratios 3/4, 3/2 and 2. Each mode has a warm-up and five timed calls; mode order is fixed, so render timing has less protection against drift.
- Diagnostic output onset uses the maximum one-millisecond first-difference energy within ±20 ms of each ratio-scaled annotation, with the later block winning equal-energy ties. It is not the formal TS transient-width/onset metric and does not prove perceptual quality. No output timing error is meaningful for the unannotated dyad.

## Detection results

| Detector | Matched | Missed | False | Recall | Mean median runtime per 1 s input |
| --- | ---: | ---: | ---: | ---: | ---: |
| energy-r4 | 324 | 54 | 0 | 85.71% | 0.208 ms |
| energy-spectral-v1 | 378 | 0 | 0 | 100.00% | 3.714 ms |

Detection runtime increased 17.84× by the ratio of summed paired medians.
All 54 added matches were masked attacks (three attacks × six rates × three layouts/gains).
Both detectors retained the other 324 annotated attacks and produced no interior false detections on these fixtures.
This small corpus does not establish a general false-positive rate.

## Rendering diagnostics

| Case | Ratio | Energy median ms | Combined median ms | Energy onset error ms | Combined onset error ms |
| --- | --- | ---: | ---: | ---: | ---: |
| bursts | 3/4 | 4.969 | 4.677 | 0.000 | 0.000 |
| bursts | 3/2 | 7.700 | 8.492 | 0.000 | 0.000 |
| bursts | 2/1 | 9.363 | 10.150 | 0.000 | 1.000 |
| masked | 3/4 | 17.374 | 17.302 | 4.000 | 4.000 |
| masked | 3/2 | 22.590 | 23.100 | 15.000 | 1.000 |
| masked | 2/1 | 27.828 | 28.504 | 10.000 | 1.000 |
| dyad | 3/4 | 19.488 | 19.288 | 0.000 | 0.000 |
| dyad | 3/2 | 25.431 | 25.971 | 0.000 | 0.000 |
| dyad | 2/1 | 31.811 | 32.489 | 0.000 | 0.000 |

All compression pairs and sustained-dyad pairs have matching output fingerprints.
Masked-attack expansion benefits in the localization diagnostic. Unmasked bursts can move slightly under the earliest-candidate merge rule.
Exact output lengths are retained; these measurements do not establish pitch, transient width, tail quality or listening parity.

## Initial failure and decision

The first spectral trial produced 54 false end-boundary detections across the dyad, vibrato and masked cases.
The detector then required complete spectral windows rather than zero-padded windows.
Annotations, matching tolerances and ending evaluation were unchanged. Both initial and corrected artifacts are retained.
The resulting boundary limitation is explicit: masked attacks within half a spectral window of a file boundary rely on Energy and can be missed.

Keep Combined opt-in. Its masked-attack improvement warrants further evaluation, but the higher detector cost and lack of real-audio/listening coverage do not justify migrating saved projects.
The public optional path is zero-pitch Request stretching; PitchRequest and DAW integration have not adopted it.

## Reproduction and artifacts

```sh
cargo run -p timestretch --release --locked --example transient_benchmark > work/transient-detector-benchmark/detection.csv 2> work/transient-detector-benchmark/render-and-build.log
cargo test -p timestretch --all-targets --locked
cargo test -p timestretch --release --all-targets --locked
```

Local raw artifacts (ignored): [detection CSV](../../work/transient-detector-benchmark/detection.csv), [render CSV](../../work/transient-detector-benchmark/render.csv), [initial detection CSV](../../work/transient-detector-benchmark/initial-detection.csv), [initial render/build log](../../work/transient-detector-benchmark/initial-render-and-build.log).
The example regenerates fixtures and fingerprints without external media. No private recordings were used.

Implementation file SHA-256 fingerprints for this measurement:

- `src/lib.rs`: `325d08fe3e9aeba8c035569750f6b60d79363053341a11b716a4843ac155486d`
- `src/transients.rs`: `16c3361ac58b1fb6942c74da9e412a5c46df9b0ffad4672d6972700a58b0ce04`
- `src/spectral_attacks.rs`: `a355b0ef4bbf49a24a98b2edf4aae26b07c75a0209f3f62bb4ba674e2b4460ed`
- `src/spectral.rs`: `bff0fbdfbac4d88e149b84b663f0006a0340fd1d4f7c7145428d224eb85040c4`
- `src/pitch.rs`: `793113009bb3252ce58a25768d64966d4128ea98fd46ff112d9f39d147c48f1d`
- `examples/transient_benchmark.rs`: `d12fb47d06b1f4f34fb57bb56f5eddddf0bed407a0c6abd10d092a391a57517c`
