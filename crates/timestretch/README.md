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

Add a constant pitch offset with `Request::with_pitch`. Positive values raise pitch; negative values lower it. Fractional semitones are supported from −24 to +24 (two octaves each way). The duration ratio stays independent: use ratio 1 to retain duration.

```rust
use timestretch::{Pitch, Ratio, Request};
let input = vec![0.25_f32; 4800];
let output = Request::new(48000, 1, Ratio::new(1, 1)?)
    .with_pitch(Pitch::from_semitones(7.0)?)
    .process(&input)?;
assert_eq!(output.len(), input.len());
# Ok::<(), timestretch::Error>(())
```

`PitchRequest` composes the existing `Request` with a validated `Pitch`. Existing constructors and request fields retain their API. `Pitch::UNISON` delegates to the unchanged stretch path, preserving its output, including exact sample bits when duration is also 1. Processing identity remains `REVISION = 2`; nonzero-pitch consumers must also identify the pitch offset and `PITCH_REVISION = 1` in their caches/reports.

Pitch processing uses linked-stereo stretching followed by a shared band-limited resampling filter. It does not preserve vocal formants or support automation. Upward shifts remove frequencies that would exceed the output Nyquist frequency (half the sample rate); upper-band attenuation and ringing near sharp boundaries are possible. No normalization or clipping is applied. Very short results retain exact length, but few samples cannot establish a meaningful pitch. Full-corpus listening and performance acceptance remain incomplete.

## Progress and resources

`process_with_progress` calls a caller-owned closure with monotonic progress. Return `ControlFlow::Break(())` to cancel. Processing allocates and blocks; call it on a worker, never an audio callback. Default accounted-memory limit is 1 GiB, configurable through `Request::memory_limit_bytes`. The estimate covers output, overlap buffers, transient regions, and a conservative FFT allowance; it does not guarantee recovery from OS out-of-memory termination or allocations inside the FFT planner.

Pitch requests use the same progress/cancellation and memory limit. Configure the limit before `with_pitch`, or through `PitchRequest::stretch`. The estimate also includes the intermediate audio and resampling table. Large upward pitch and duration changes need more memory and processing time. Failure or cancellation returns no partial output.

The engine uses a linked stereo phase vocoder with peak phase locking. It carries the applied phase between frames and protects detected attacks during expansion by retaining their short waveform regions at original speed; intervening regions supply the extra duration. Onsets remain anchored to the requested ratio. No gain matching or normalization is applied. A full-source spectral-flatness analysis, including broad occupied frequency bands, selects shorter windows for broadband material. Masked attacks, dense percussion, and sustained cymbal tails still need listening evaluation. This is an initial experimental engine, not a claim of Rubber Band quality equivalence.

```sh
cargo test -p timestretch
cargo test -p timestretch --release -- --include-ignored
```

The expanded release suite includes the six-rate tone and short-input matrices. See [provenance](PROVENANCE.md), [license](LICENSE), and [dependency notices](licenses). No Rubber Band implementation code may be copied, translated, adapted, or indirectly reused under any circumstances.
