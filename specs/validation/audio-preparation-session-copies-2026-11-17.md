# Fewer session copies during audio preparation

The background worker now prepares the private session it receives. This removes
two full session copies after submission. The shared exporter prepares its
existing private snapshot and removes one further copy. Borrowed preparation
keeps a private candidate and commits it only after success. Audio samples are
already shared across sessions. The removed copies mainly duplicate project
metadata and lookup tables.

## Benchmark

Release builds on macOS arm64 with Rust 1.97.0 used the same generated fixture
and benchmark program. The before build uses project code at `1cbcd9e1`.
Each result is the median of 11 fresh processes per build
and case. The fixture uses two seconds of generated 44.1 kHz mono audio, a 48 kHz
project and three sampler pitches. The stress case repeats the same chord pattern
across 20,000 clips. Warm cases prepare the project before timing. Fresh
preparation starts with empty DAW preparation caches. The operating system file
cache was unmanaged. Peak resident memory covers the whole process, including
setup. Worker timing starts before copying the editor session for submission and
ends when the result is received.

| Case | Before, ms (range) | After, ms (range) | Peak memory before → after |
| --- | ---: | ---: | ---: |
| Fresh sampler preparation | 131.54 (130.17–134.39) | 129.71 (127.61–153.36) | 102.14 → 102.11 MiB |
| Prepared sampler job | 0.130 (0.116–0.183) | 0.124 (0.111–0.174) | 102.31 → 102.33 MiB |
| Prepared 20,000-clip job | 5.02 (4.78–5.28) | 3.86 (3.79–4.07) | 139.34 → 131.27 MiB |
| Float WAV export | 5.87 (5.76–6.39) | 5.85 (5.78–5.99) | 102.80 → 102.73 MiB |

The large session took 23% less time and used about 8 MiB less peak memory. The
other timing ranges overlap. Export reached its first encoded block in a median
of 0.520 ms before and 0.503 ms after. The two exported WAVs matched byte for byte.
Both exports returned an empty warning list.

The benchmark entry point is
[the audio preparation example](../../crates/project/examples/audio_preparation.rs).
Build with `cargo build --release --offline -p daw-project --example audio_preparation`.
Use `init <scratch-project> 2` to generate the fixture and
`benchmark <scratch-project> <case>` to measure it. Case names are
`worker-cold`, `worker-warm`, `worker-many-clips`, and `export`.
The export case writes or replaces `benchmark.wav` in the scratch project.

## Rollback checks

A borrowed preparation call exercises a validation error after source decoding.
The active model, folder, warnings and derived audio maps are checked after the
error. Worker tests force cancellation and replacement while an old candidate
is running. Both cases check that its decoded event is discarded and its late
completion cannot replace the current revision.

The benchmark is headless and uses generated media. Native UI, audio devices
and other platforms were outside this run.

Final project, UI and output suites passed 1,430 tests with five ignored. The
engine suites passed 256 tests with one ignored. Project Clippy, workspace
formatting and the CLI smoke check passed. The CLI check includes export
repeatability, compound sources, missing sources and overwrite protection.
