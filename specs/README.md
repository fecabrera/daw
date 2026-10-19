# Implementation specifications

This tree contains implementation requirements, architecture, data formats, acceptance criteria, and engineering validation history. User instructions belong in the [user guide](../docs/README.md). Follow the audience and folder rules in [AGENTS.md](../AGENTS.md#documentation-and-dependency-licenses).

## Architecture and baseline

- [MVP scope and architecture](architecture/mvp-scope.md).
- [Iteration 1 implementation specification](architecture/iteration-1.md).
- [Modular audio units](architecture/modular-audio-units.md).
- [Prepared rendering and performance](architecture/performance.md).

## Instruments and editing

- [MIDI patterns, Oscillator, Envelope, and rendering](instruments/midi-patterns.md).
- [Sampler](instruments/sampler.md).
- [Envelope inputs](instruments/envelope.md).
- [LFO](instruments/lfo.md).
- [Macro](instruments/macro.md).
- [Instrument Main](instruments/main.md).
- [Per-layer effects](instruments/layer-effects.md): saved stereo bus chains, shared controls, modulation, and latency alignment.
- [Component editing and registration](editing/components.md).
- [Beat editor design](views/beat-editor.md), pending implementation.

## Effects

- [3-band EQ](effects/equalizer.md)
- [Drive](effects/drive.md)
- [Bitcrusher](effects/bitcrusher.md)
- [Auto-filter](effects/auto-filter.md)
- [Compressor](effects/compressor.md)
- [OTT](effects/ott.md)
- [Limiter](effects/limiter.md)
- [Direction Mixer](effects/direction-mixer.md)
- [Stereo Spread](effects/stereo-spread.md)

## Modulation and audio

- [Shared modulation contract](modulation/contract.md).
- [Oscillator pitch modulation](modulation/oscillator-pitch.md).
- [Timestretch, offline pitch, and independent implementation rule](audio/timestretch.md).
- [Timestretch acceptance plan](acceptance/timestretch.md).

## Evidence

[Validation history](validation/README.md) records completed checks and unverified behavior. It is historical evidence, not a substitute for current requirements. Keep dates and original results when moving or correcting records.

- [Performance audit — 2026-10-18](validation/performance-audit-2026-10-18.md): measured CPU/allocation baselines, scaling findings, proposed remediations, and native verification gaps.

- [Performance remediation — 2026-10-18](validation/performance-remediation-2026-10-18.md): implemented changes, before/after evidence, and remaining checks.
