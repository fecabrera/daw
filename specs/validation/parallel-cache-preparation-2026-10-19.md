# Parallel cache preparation — 2026-10-19

Status: implementation, automated regressions and isolated release benchmarks complete. Native device and cross-platform checks remain open.

## Scope and design

The owner approved parallel cache preparation across tracks with two workers and a serial fallback, preserving exact history and shared processing. The implementation follows the [processing contract](../architecture/performance.md#parallel-track-preparation). Live playback and offline export retain their serial scheduling; their track and Master DSP stages are shared with cache jobs.

A bounded pair of persistent workers processes independent tracks. Each track remains sequential. The coordinator retains project-order summation and serial Master processing. Checkpoints publish only after every stage reaches the same boundary. Auto falls back for Light, small/single-track/cheap-only work, missing execution slots, thread-start failure or working-memory limits. Cancellation discards partial cursors; worker failure reconstructs through complete checkpoints using the serial path.

## Regression coverage

- Exact sample equality for serial cache, parallel cache and full-history reference, including forward/backward seeks, 44.1/48/192 kHz, 32/4096-frame project buffers, nonzero loops, bypassed Saturator, layered effects, instrument Main, Master effects and latency priming.
- Mixed stereo/mono audio tracks and prepared Sampler playback, source offsets, repeated input, gaps, fades and positions.
- Cancellation retains complete checkpoints and cannot publish partial track state. Subsequent preparation matches the reference.
- Injected worker-channel failure recovers the complete requested state through serial reconstruction.
- Light/single-track fallback and worker release; parallel memory is accounted and released with optional preparation.
- Returned pooled renderers retain allocation/deallocation-free sample processing and bounded transport activation on the callback test path.

## Benchmark method

Run from the repository root with the pinned toolchain:

```sh
CARGO_HOME="$PWD/work/cargo-home" python3 tools/performance_audit.py --suite cache --runs 7 --output work/parallel-cache-benchmark-final.json
```

The runner builds the workspace's release `cache_preparation` example, then records source/runner/lock hashes, toolchain, hardware, repetitions, per-case median/p95/max, and whole-suite child peak RSS where supported. Use `--skip-build` only after rebuilding that example for the current source. Results stay in ignored `work/`; no private media is required or modified.

The synthetic cases use 48 kHz, four held modulated voices per MIDI track and Balanced cache settings. Cold requests prepare frame 48,000, except the short request at frame 128. Warm requests prepare 123 frames before the cold target using an earlier checkpoint. Cases cover one/two/eight tracks, source-only voices, skewed track cost, heavy Master processing, short requests, inexpensive audio tracks, and 32/4096-frame buffers. Each case compares Serial, forced two-worker Parallel and Auto. Mode order rotates across repetitions after an untimed warmup. Cold timings include model/runtime construction and thread startup. Destruction/joins occur outside the timed section. Worker counts, processed block counts and estimated checkpoint/pool bytes accompany each row.

The audio-only cost case retains one silent modulated MIDI instrument to exercise prefix-history reconstruction; eight additional tracks play prepared stereo audio without effects. The skewed case adds eight Saturators to one track. The heavy Master case uses four serial Master Saturators. Balanced cases use one Saturator per track. All use the same source fixture and processing code as the regressions.

## Release measurements

Measured after all owned build/test jobs finished, on Apple M4 Pro (14 logical CPUs, 48 GB), macOS 27.0.1 ARM64, Rust 1.97.0, workspace release profile, without custom `RUSTFLAGS`. Seven measured repetitions follow one untimed warmup for each case/mode. This is one machine/session; external system activity and thermal state were not controlled.

Cold preparation medians, in milliseconds:

| Workload | Serial | Forced Parallel | Auto | Serial / Auto | Auto workers |
| --- | ---: | ---: | ---: | ---: | ---: |
| One MIDI track | 20.466 | 20.484 | 20.502 | 1.00× | 0 |
| Two balanced MIDI tracks | 39.020 | 19.443 | 19.520 | 2.00× | 2 |
| Eight balanced MIDI tracks | 166.182 | 80.019 | 80.910 | 2.05× | 2 |
| Eight source-only MIDI tracks | 91.079 | 44.256 | 44.425 | 2.05× | 2 |
| Eight tracks, uneven processing | 166.786 | 96.030 | 95.852 | 1.74× | 2 |
| Eight tracks, heavy Master | 147.672 | 88.043 | 90.466 | 1.63× | 2 |
| Two tracks, 128-frame request | 0.198 | 0.197 | 0.199 | 1.00× | 0 |
| Eight cheap audio tracks plus silent instrument | 15.334 | 11.458 | 15.279 | 1.00× | 0 |
| Four MIDI tracks, 32-frame buffer | 82.064 | 40.665 | 40.375 | 2.03× | 2 |
| Four MIDI tracks, 4,096-frame buffer | 80.270 | 40.821 | 40.745 | 1.97× | 2 |

Warm preparation medians for two balanced tracks fall from 5.781 ms to 2.922 ms with Auto; eight balanced tracks fall from 23.273 ms to 12.161 ms. Cold p95 values for these cases fall from 40.452/167.500 ms to 20.425/81.829 ms. With only seven samples, the reported p95 is the observed maximum, not a stable tail-latency estimate.

The eight-track automatic pool estimates 1.230 MiB of added storage, including reserved thread stacks; the two-track pool estimates 1.104 MiB. Whole-suite child peak RSS is 11.0 MiB. RSS includes shared models, buffers and all modes; it cannot establish the pool's resident-memory cost. Checkpoint limits still subtract the pool estimate from the shared preparation allowance.

The results support two workers for substantial independent track work. Uneven tracks and the serial Master stage limit gains. Auto deliberately keeps cheap audio work serial; forcing parallelism helped that synthetic case, but this does not establish that its extra workers are useful under native playback pressure. No timing threshold is added to functional CI.

The complete run is in ignored `work/parallel-cache-benchmark-final.json` (60 timing rows). Source-tree SHA-256: `0ca4c8e52687c39ae95d45c3253302041d58bad5db99ef01267c0237b84eac2c`; harness SHA-256: `3fdcff9e47479ecf87931c993c02f607d9910613a5f4fe9cbe6cd6c5dc4f31e5`; runner SHA-256: `dd728f3592bfa7156906c66cdb12547c2e94545d5cd47a2cd93f12a3e8de2a46`. The run records the uncommitted implementation above base commit `fff4d078ca512fb00232bf45257db23e598a5a37`.

The report does not measure native request-to-activation latency, physical-device underruns, battery/thermal cost, or per-case RSS. Whole-suite peak RSS is not a per-mode comparison. Numbers from these synthetic projects do not predict a private project's exact speedup.

## Executed checks

- Focused engine regressions pass in debug and release (four cases per run).
- Cache unit tests pass, including injected channel failure (six cases).
- Output callback allocation/deallocation guard passes for a renderer prepared across tracks.
- `cargo test --workspace`: **971 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices.
- `cargo fmt --all --check` and strict workspace Clippy pass.
- CLI build and `python3 tools/check_cli.py` pass, including WAV/MP3, deterministic output, missing sources, eight-track projects, overwrite handling and exit codes.
- `sh tools/package_macos.sh release` passes and rebuilds `target/release/DAW.app`. The app was not relaunched.
- Release benchmark completes all 60 timing rows.

Changed-document relative file links and `git diff --check` pass. Native listening, device-pressure response, and Windows/Linux execution were not checked.
