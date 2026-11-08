# Audio analysis

Portable offline analysis over host-owned interleaved f32 PCM. Original code is
BSD-2-Clause. The first routine detects combined energy/spectral transients;
Energy is available as a baseline. No DAW, timestretch, device or file dependency.

```rust
use audio_analysis::TransientRequest;
let samples = vec![0.0; 4800];
let frames = TransientRequest::new(48000, 1).detect(&samples)?;
assert!(frames.is_empty());
# Ok::<(), audio_analysis::Error>(())
```

Use `detect_with_progress` for worker progress/cancellation. Set `detector` or
`memory_limit_bytes` on the request as needed. Output positions are source-frame
indices, sorted and linked across stereo. Supported rates are 44.1/48/88.2/96/
176.4/192 kHz. Detection allocates; never call it in an audio callback.

The analysis-only memory estimate excludes caller input and FFT planner internals;
no stretch ratio or output allocation is required. Detector identities and
thresholds match Integrated revision 6. Timestretch reexports TransientDetector
and retains its older detection adapter for compatibility.

See the [implementation contract](../../specs/audio/analysis.md) for control flow,
thresholds, errors and limits, and [provenance](PROVENANCE.md) for design history.
RustFFT 6.4.1 and its transitive dependency notices are in licenses/. No dependency
versions were upgraded by the extraction.

Run `cargo test -p audio-analysis --all-targets` and `cargo test -p audio-analysis --doc`.
