# Timestretch

Experimental offline Rust time stretching. Original code is BSD-2-Clause. The library has no DAW, device, file, native C/C++, or Rubber Band dependency.

```rust
use timestretch::{Ratio, Request};
let input = vec![0.25_f32; 4800]; // interleaved mono, 48 kHz
let output = Request::new(48000, 1, Ratio::new(3, 2)?).process(&input)?;
assert_eq!(output.len(), 7200);
# Ok::<(), timestretch::Error>(())
```

Use one or two interleaved channels, a supported rate (44.1/48/88.2/96/176.4/192 kHz), and an output/input ratio from 1/8 through 8. Output length uses exact rational rounding. Identity preserves finite sample bits. Finite headroom is retained. Invalid layout, nonfinite audio, numerical failure, resource limits, and cancellation return typed errors.

`process_with_progress` calls a caller-owned closure with monotonic progress. Return `ControlFlow::Break(())` to cancel. Processing allocates and blocks; call it on a worker, never an audio callback. Default accounted-memory limit is 1 GiB, configurable through `Request::memory_limit_bytes`. The estimate covers output, overlap buffers, transient regions, and a conservative FFT allowance; it does not guarantee recovery from OS out-of-memory termination or allocations inside the FFT planner.

The engine uses a linked stereo phase vocoder with peak phase locking. Revision 2 carries the applied phase between frames and protects detected attacks during expansion by retaining their short waveform regions at original speed; intervening regions supply the extra duration. Onsets remain anchored to the requested ratio. No gain matching or normalization is applied. A full-source spectral-flatness analysis, including broad occupied frequency bands, selects shorter windows for broadband material. Masked attacks, dense percussion, and sustained cymbal tails still need listening evaluation. This is an initial experimental engine, not a claim of Rubber Band quality equivalence.

New requests use revision 2. Use `Request::with_revision(1)` to reproduce the original processor. Unsupported revisions return `Error::UnsupportedRevision`, including identity requests.

```sh
cargo test -p timestretch
cargo test -p timestretch --release -- --include-ignored
```

The expanded release suite includes the six-rate tone and short-input matrices. See [provenance](PROVENANCE.md), [license](LICENSE), and [dependency notices](licenses). No Rubber Band implementation code may be copied, translated, adapted, or indirectly reused under any circumstances.
