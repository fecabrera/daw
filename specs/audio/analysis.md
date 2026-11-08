# Audio analysis

`crates/audio-analysis` owns portable offline analysis over host-owned PCM. Its
first routine detects transients. It has no project, UI, media, device, file,
thread, stretch-ratio or rendering dependency. Dependency direction is consumer
→ audio-analysis → RustFFT. Original code is BSD-2-Clause; use only the existing
independently written detector, subject to the [independent implementation rule](timestretch.md#mandatory-independent-implementation-rule).

## API, validation and control flow

`TransientRequest::new(sample_rate, channels)` defaults to Combined and a 256 MiB
memory budget. The caller can select Energy for comparisons. `detect(input)`
returns sorted source-frame indices. `detect_with_progress(input, callback)`
adds cancellation; callbacks return ControlFlow::Break to discard the result.
Samples are interleaved f32 mono or stereo, with complete frames. Supported rates
are 44.1/48/88.2/96/176.4/192 kHz. Keep the existing six-rate scope during this
extraction. No beat labels, confidence scores or automatic normalization are
provided. Channel analysis is linked through power, so anti-phase cannot cancel.

1. Call progress at zero. Validate rate and channels, including empty inputs,
   before dividing by channel count. Reject incomplete frames.
2. Check the memory estimate and budget before allocating analysis buffers.
   `estimated_memory_bytes(frames)` uses checked arithmetic, rejects extents above
   i64::MAX, and reserves 8 MiB plus three usize entries per maximum event
   (`floor(frames / floor(rate * 30/1000)) + 1`). This includes returned positions
   and candidate capacities; excludes caller input, allocator overhead and FFT
   planner internals. Energy uses the same conservative estimate. No rendered
   output memory is charged. The budget is an admission estimate, not a hard
   allocator cap.
3. Reject NaN/Inf in chunks of 8,192 samples. Keep finite headroom and signed
   zero. Validate with monotonic progress in 0–0.1, then scan in 0.1–1.
4. Allocate bounded candidate vectors and optional spectral scratch. Scan once
   in source order, merge candidates, finish progress at one, return positions.
   Cancellation, resource failure and invalid input return typed errors and no
   partial result. Empty/zero input returns no markers after normal validation;
   a nonzero start may be an Energy event.

This routine allocates and can take significant time. Run it on a worker, never
an audio callback or in UI painting. The caller owns publication and caching.
Cache analysis by source identity, rate, channels and detector identity, not
stretch ratio. Identities remain Energy `energy-r4` and Combined
`energy-spectral-v5`; move-only extraction changes neither thresholds nor output.

## Energy scan and combined spectral evidence

The baseline energy detector is unchanged: blocks contain `max(1, floor(rate /
1000))` frames. Sum squared samples across both channels and divide by sample
count. An attack must exceed six times the mean of the populated preceding 80
observations and twice the maximum of the preceding two observations. Require
at least `floor(rate * 30/1000)` frames since its previous attack. Update history
after the decision. Startup uses populated observations; all-zero history does
not impose an absolute amplitude threshold. Polarity cannot cancel summed power.

Combined keeps all baseline candidates and adds spectral evidence:

- Every four energy blocks, analyze a centered Hann window of
  `clamp(next_power_of_two(floor(rate * 16/1000)), 256, 4096)` frames.
- Analyze channels separately with the existing RustFFT library and sum bin
  powers, never channel samples. Use bins from `max(2, floor(700 * width/rate))`
  through the bin below Nyquist. DC, low-frequency bass and Nyquist are excluded.
- At each eligible bin, compare current power against the maximum of that bin
  and its immediate two neighbors in the previous spectrum. Sum only positive
  differences. This is an independently chosen three-bin novelty reference,
  not a claim to implement the full SuperFlux algorithm.
- Require high-band power > `1e-5 * total power`, high-band power > twice its
  previous value, and summed novelty > 0.35 * current high-band power.
- Require novelty participation `(sum novelty)^2 / sum(novelty^2) >= 16`.
  This gain-invariant measure rejects growth concentrated in a few harmonics.
  Such growth can occur while a guitar note sustains and must not add a phase
  reset. The test precedes position refinement and the refractory update; a
  rejected candidate cannot suppress a later broadband event. Energy detection
  remains unchanged, including its handling of tonal attacks.
- Locate the event within that window using 1 ms blocks of summed squared
  first differences of the original channel samples. Choose the block with the
  largest positive difference from the preceding block; ties retain the earlier
  block. Its first frame is the event position. The initial comparison value is
  zero, so refinement is approximate and can favor the window's first block.
- After refinement, compare mean squared first differences in the 8 ms before
  the event with the interval from one energy block after the event to 8 ms
  after it. Require the latter to exceed twice the former. Excluding the first
  block prevents a single cutoff impulse being classified as a sustained attack.
  Require both complete intervals; quiet very short impulses can rely on Energy.
- Require the spectral stream's own 30 ms minimum event spacing. Collect Energy
  and spectral positions separately. Suppress any spectral candidate less than
  30 ms from an original Energy candidate, retaining the Energy timestamp even
  when the spectral candidate is earlier. Sort the combined list and enforce
  30 ms spacing. Thus added spectral evidence cannot shift a known Energy attack.
  The v1 experiment's earliest-candidate precedence is superseded.

## Boundaries and limits

Skip incomplete centered spectral windows; do not invent evidence from padding.
Energy still scans boundaries. The first complete nonzero spectral window warms
history without emitting a marker. Refinement requires complete 8 ms intervals
before/after the candidate. Short or quiet masked events may therefore be missed.
Harmonic-rise rejection can miss narrowband attacks hidden from Energy. A 30 ms
minimum spacing merges dense events. Detection is not instrument separation and
does not guarantee perceived quality. Timestretch owns the decision to protect
attacks during expansion and owns all phase resets and timing regions.

## Verification

Crate tests cover validation, budget boundaries/overflow, monotonic progress,
cancellation including completion, silence, Energy bass/refractory behavior,
masked broadband attacks, harmonic rejection and gain/anti-phase invariance at
all six rates. Timestretch integration tests retain pitch, attack waveform,
exact output and cancellation coverage. The existing paired benchmark and
private Splice render comparisons check extraction parity. No editor markers or
new user controls are introduced by this extraction.

## Audio Editor consumer

The [Editor indicators](../views/audio-transients.md) call the shared detector
through the media storage adapter on an optional worker. The detector and its
identities are unchanged. UI marker caching and mapping do not affect the stretch
engine, playback or saved clip content.
