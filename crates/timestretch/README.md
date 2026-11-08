# Timestretch

Experimental offline Rust time stretching and pitch shifting. Original code is BSD-2-Clause. The library has no DAW, device, file, native C/C++, or Rubber Band dependency.

```rust
use timestretch::{Ratio, Request};
let input = vec![0.25_f32; 4800]; // interleaved mono, 48 kHz
let output = Request::new(48000, 1, Ratio::new(3, 2)?).process(&input)?;
assert_eq!(output.len(), 7200);
# Ok::<(), timestretch::Error>(())
```

Use one or two interleaved channels, a supported rate (44.1/48/88.2/96/176.4/192 kHz), and an output/input ratio from 1/8 through 8. Output length uses exact rational rounding. Identity preserves finite sample bits. Finite headroom is retained. Invalid layout, nonfinite audio, numerical failure, resource limits, and cancellation return typed errors.

## Offline pitch changes

Add a constant pitch offset with `Request::with_pitch`. Positive values raise pitch; negative values lower it. Fractional semitones are supported from −127 to +127 (the maximum difference between MIDI notes). The duration ratio stays independent: use ratio 1 to retain duration.

```rust
use timestretch::{Pitch, Ratio, Request};
let input = vec![0.25_f32; 4800];
let output = Request::new(48000, 1, Ratio::new(1, 1)?)
    .with_pitch(Pitch::from_semitones(7.0)?)
    .process(&input)?;
assert_eq!(output.len(), input.len());
# Ok::<(), timestretch::Error>(())
```

`PitchRequest` composes the existing `Request` with a validated `Pitch`. Existing constructors and request fields retain their API. `Pitch::UNISON` delegates to the same stretch path, including exact sample bits when duration is also 1. Processing identity is `REVISION = 4`; nonzero-pitch consumers must also identify the pitch offset and `PITCH_REVISION = 2` in their caches/reports.

Pitch processing combines linked-stereo stretching with a shared band-limited resampling filter. Offsets beyond ±24 use up to six bounded passes to limit memory. Large upward shifts resample before spectral processing; downward shifts retain the original order. `Pitch::stages()` exposes the common pass schedule for host adapters. It does not preserve vocal formants or support automation. Upward shifts remove frequencies that would exceed the output Nyquist frequency (half the sample rate); upper-band attenuation and ringing near sharp boundaries are possible. No normalization or clipping is applied. Very short results retain exact length, but few samples cannot establish a meaningful pitch. Full-corpus listening and performance acceptance remain incomplete.

## Progress and resources

`process_with_progress` calls a caller-owned closure with monotonic progress. Return `ControlFlow::Break(())` to cancel. Processing allocates and blocks; call it on a worker, never an audio callback. Default accounted-memory limit is 1 GiB, configurable through `Request::memory_limit_bytes`. The estimate covers output, overlap buffers, transient regions, and a conservative FFT allowance; it does not guarantee recovery from OS out-of-memory termination or allocations inside the FFT planner.

Pitch requests use the same progress/cancellation and memory limit. Configure the limit before `with_pitch`, or through `PitchRequest::stretch`. The estimate also includes the intermediate audio and resampling table. Large upward pitch and duration changes need more memory and processing time. Failure or cancellation returns no partial output.

Hosts can call `PitchRequest::working_memory_bytes(input_frames)` before processing to reserve the same conservative working-storage estimate used by the request. This excludes immutable input, FFT planner internals and allocator overhead. It does not allocate or process audio.

The engine uses a linked stereo phase vocoder with peak phase locking. It carries the applied phase between frames and protects detected attacks during expansion by retaining their short waveform regions at original speed; intervening regions supply the extra duration. Onsets remain anchored to the requested ratio. No gain matching or normalization is applied. A full-source spectral-flatness analysis, including broad occupied frequency bands, selects shorter windows for broadband material. Bass-dominated sources without significant broadband activity in the upper bands use longer analysis windows to separate nearby low notes, reusing the bounded overlap schedule and guarded attack timing. The attack detector uses an 80 ms energy background to avoid resetting phases on bass cycles and beats. Masked attacks, dense percussion, source-wide window selection, and sustained cymbal tails still need listening evaluation. This is an initial experimental engine, not a claim of Rubber Band quality equivalence.

```sh
cargo test -p timestretch
cargo test -p timestretch --release -- --include-ignored
```

The expanded release suite includes the six-rate tone and short-input matrices. See [provenance](PROVENANCE.md), [license](LICENSE), and [dependency notices](licenses). No Rubber Band implementation code may be copied, translated, adapted, or indirectly reused under any circumstances.

## Experimental detector comparison

Default processing is revision 6 with the combined energy/spectral detector
(`energy-spectral-v5`), including pitch composition. To compare the unchanged
energy baseline, call `Request::process_with_transient_detector` with
`TransientDetector::Energy` and a progress/cancellation callback. Explicit-mode
callers must include the detector identity as well as processing revision in
caches/reports. DAW Integrated revisions 2–5 upgrade to revision 6 with rebuilt
prepared audio and a warning; Rubber Band remains the initial DAW backend. `detect_transients` exposes the same linked,
sorted source-frame event positions for analysis. Its request validation and
conservative full stretch memory estimate apply even to detection-only calls.

Run the independent synthetic benchmark (no Rubber Band or DAW dependency):

```sh
cargo run -p timestretch --release --example transient_benchmark > detection.csv 2> render.csv
cargo test -p timestretch --example transient_benchmark
```

Detection CSV records seeded fixture hashes, matched/missed/false events and
median/min/max runtime. Rendering diagnostics go to stderr together with Cargo
build messages. Timings are local measurements, not portable performance gates.
See the repository's `specs/audio/timestretch.md` for control flow and edge cases.

## Private real-audio listening comparisons

The standalone tool includes `transient_listen`. It reads WAV files without
writing to their directories and creates raw renders, blinded A/B auditions,
original references, a blank rating sheet and private hash/mapping reports in a
new output directory. Never place outputs inside a source directory.

```sh
cargo run -p timestretch-cli --release --no-default-features --bin transient_listen -- \
  --manifest corpus.json --output-dir ./new-listening-pack --seconds 10
```

The manifest is a JSON array of objects with `id`, `path`, `rights` and optional
`start_seconds`. Relative paths resolve from the manifest directory. Ratios
default to 3/4, 3/2 and 2/1; sample rate defaults to 48 kHz. Output directories
must not exist. Keep licensed/private recordings and renders out of version
control. `LISTENING.md` describes the human rating procedure; a prepared pack
is not a passed listening test. No ratings are generated automatically.

Transient detection is owned by the portable [audio-analysis crate](../audio-analysis/README.md).
Use its TransientRequest directly for analysis without stretch settings. The
existing timestretch detection API remains a compatibility adapter.
