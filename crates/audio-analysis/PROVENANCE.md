# Independent implementation record

Extracted unchanged on 2026-11-08 from the original BSD-2-Clause detector in
`timestretch` revision 6. Energy scanning, high-band novelty, refinement, harmonic
rejection and merging remain unchanged. The new public request owns validation,
memory admission and progress without stretch settings. No reference-engine
implementation code was inspected or reused for this extraction.

The [original design record](../timestretch/PROVENANCE.md) records research inputs,
prior exposure, the independently chosen equations/thresholds and regression
fixes. In particular, the three-bin maximum novelty reference is independently
chosen, inspired by published spectral onset concepts; it is not an implementation
of the full SuperFlux algorithm. The mandatory independent implementation rule
in specs/audio/timestretch.md continues to apply.

RustFFT 6.4.1 (MIT OR Apache-2.0) remains the only direct external dependency.
Bundled notices in licenses/ cover its existing transitive dependencies.

## Source tempo estimator

The tempo module is an original streaming energy, onset and pulse-grid
implementation. Autocorrelation and multiple pulse interpretations follow the
general concepts in [Ellis, 2007](https://www.ee.columbia.edu/~dpwe/pubs/Ellis07-beattrack.pdf).
The module uses no copied source, model data or new dependencies. Generated
regression pulses are authored in the tests. Private Splice loops may be read
locally for comparison and are excluded from repository fixtures.

The loop fallback uses original duration, onset phase and interval checks.
Its synthetic fixtures use generated tonal notes and pulse trains.

The signed-energy recovery uses the estimator's existing three band levels.
Its original implementation combines smoothed, compressed level changes,
normalized lag correlation, phase checks across separate regions, and explicit
competing-rate checks. The complete-loop hint can corroborate a repeat at one
or two beats. No external implementation, model, or new library was used.

The spectral fallback uses the existing RustFFT dependency. It averages stereo
frames onto a smaller sample clock, measures fine-band level changes, and
checks that the changes share a phase across the window. The pulse scoring
reuses this module's original onset and interval checks. No external detector
implementation or model was used.

## Offline OBTAIN

The offline tracker was written independently from the published
[OBTAIN arXiv v2 manuscript](https://arxiv.org/pdf/1704.02216v2), sections II and
III. It follows spectral onset strength, lag hypotheses, cumulative beat
strength and competing beat paths. The 100 Hz timing grid, centered analysis,
bounded search and deterministic path decisions are offline implementation
choices. No third-party tracker code, model, port or authored output was
imported. The tracker reuses the existing RustFFT dependency.

The opt-in experiment adds original band normalization, guarded candidate
selection and common onset scoring. It reuses the original packed stereo FFT,
onset refinement and cumulative recurrence. No external implementation code,
new dependency, or model data was used. The beat benchmark uses generated
signals with exact source-frame references. Splice files and their names
stay outside the repository's tracked fixtures and reports.
