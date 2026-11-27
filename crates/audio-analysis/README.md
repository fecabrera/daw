# Audio analysis

Portable analysis over caller-owned PCM. Original code is [BSD-2-Clause](LICENSE).
There is no DAW, timestretch, device or file dependency. Keep
`crates/audio-analysis` and `crates/dsp-support/analysis-kernels` at the same
relative paths when building outside the DAW workspace. The second is a
dependency-free BSD-2-Clause package with the shared live measurements.

## Loudness and peak measurement

Use `audio_analysis::loudness::OfflineAnalyzer` for sample peak, true peak or
integrated loudness. Feed it stereo `[f32; 2]` frames and call `finish` for
the measurement. The [measurement specification](../../specs/audio/analysis.md#loudness-and-peaks)
describes the result and input limits.

Use `RollingLoudness` for prepared 400 ms and three-second windows.
It emits numeric observations at 100 ms intervals at the project rates.
`IntegratedStatistics` reduces those observations to integrated loudness,
loudness range and peak ratios. The caller schedules the reduction and
handles reset, transport and display history.

## Streaming spectrum

Prepare `audio_analysis::spectrum::SpectrumAnalyzer` at the input rate, then
feed it stereo frames. Each ready update gives the powers of 512 logarithmic
frequency cells and the frame position of the observation. The analyzer
prepares its storage in advance and does bounded work per input frame.

## Waveforms, filter response and diagnostics

- `waveform` folds stereo samples into minimum and maximum pairs. The caller
  supplies the initial pair and chooses window sizes.
- `response::FrequencyResponse` evaluates a cascade of up to four biquads in dB
  from their coefficients and sample rate.
- `signal` provides power and spectral flatness helpers.
  `signal::Pcm` measures tones, channel residual, stereo phase and events from
  a borrowed interleaved PCM buffer. Event measurement allocates scratch space.

All of these APIs, including loudness and spectrum, are available with
`default-features = false`. This build has no external crate dependencies.

## Transient detection

The default `detectors` feature includes the transient, tempo and beat detectors.
The transient detector combines energy and spectral evidence. Energy is
available as a baseline.

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
The notices for RustFFT 6.4.1 and its transitive dependencies are in `licenses/`.

Run `cargo test -p audio-analysis --all-targets` and `cargo test -p audio-analysis --doc`.
