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
