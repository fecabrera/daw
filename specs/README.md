# Implementation specifications

This tree contains implementation requirements, architecture, data formats, acceptance criteria, and engineering validation history. User instructions belong in the [user guide](../docs/README.md). Follow the audience and folder rules in [AGENTS.md](../AGENTS.md#documentation-and-dependency-licenses).

## Architecture and baseline

- [MVP scope and architecture](architecture/mvp-scope.md).
- [Iteration 1 implementation specification](architecture/iteration-1.md).
- [Modular audio units](architecture/modular-audio-units.md).
- [Separate CLAP/VST3 unit builds](architecture/unit-plugin-builds.md): implementation plan for built-in units with independent plugin exports, starting with Drive and its shared two-strip editor.
- [Drive plugin Phase 0](architecture/drive-plugin-phase0.md): reproducible local probes, narrow-adapter decision, native editor feasibility, baseline/tail policy, and remaining platform/host gates.
- [Drive plugin Phase 1](architecture/drive-plugin-phase1.md): frozen identity, checked manifest/panel bindings, shared parameter mappings, and bounded state compatibility.
- [Drive plugin Phase 2](architecture/drive-plugin-phase2.md): headless CLAP processor, bounded active recall, loaded-binary parity/allocation/CPU evidence, and retained validator rate-policy failure.
- [Drive plugin Phase 3](architecture/drive-plugin-phase3.md): separate VST3 component/controller, normalized linear automation queues, common state publication, SDK validation and remaining release gates.
- [Drive plugin Phase 4: shared editor](architecture/drive-plugin-phase4.md).
- [Saturator and Bitcrusher plugin builds](architecture/saturator-bitcrusher-plugin-builds.md).
- [Drive v2 plugin builds](architecture/drive-v2-plugin-builds.md): separate CLAP/VST3 products, shared three-strip editor, state contract and local validation.
- [Prepared rendering and performance](architecture/performance.md).
- [Stem-separation native dependency audit](audio/stem-native-dependencies.md): pinned native components, advisories, licenses and platform packaging.
- [Parallel cache preparation validation — 2026-10-19](validation/parallel-cache-preparation-2026-10-19.md).
- [Architecture review — 2026-10-19](architecture/review-2026-10-19/README.md): current architecture, pipelines, workflow stages, cache ownership, findings and review decisions.

## Instruments and editing

- [MIDI patterns, Oscillator, Envelope, and rendering](instruments/midi-patterns.md).
- [Granulizer](instruments/granulizer.md)
- [Sampler](instruments/sampler.md).
- [Envelope inputs](instruments/envelope.md).
- [LFO](instruments/lfo.md).
- [Macro](instruments/macro.md).
- [Instrument Output](instruments/main.md).
- [Per-layer effects](instruments/layer-effects.md): saved stereo bus chains, shared controls, modulation, and latency alignment.
- [Track groups as processing buses](editing/track-groups.md).
- [Project undo and redo](editing/undo-redo.md).
- [Component editing and registration](editing/components.md).
- [Clip settings](editing/clip-settings.md).
- [Chord catalog](editing/chords.md).
- [Beat editor design](views/beat-editor.md), pending implementation.

## Effects

- [3-band EQ](effects/equalizer.md)
- [Multiband EQ](effects/multiband-eq.md)
- [Drive](effects/drive.md)
- [Drive v2](effects/drive-v2.md)
- [Saturator](effects/saturator.md)
- [Bitcrusher](effects/bitcrusher.md)
- [Filter](effects/filter.md)
- [Auto-filter](effects/auto-filter.md)
- [Compressor](effects/compressor.md)
- [OTT](effects/ott.md)
- [Limiter](effects/limiter.md)
- [Direction Mixer](effects/direction-mixer.md)
- [Stereo Spread](effects/stereo-spread.md)
- [Chorus](effects/chorus.md)

## Modulation and audio

- [AAC-LC export](audio/aac-export.md): portable streaming encoding, rate conversion and ADTS limits.
- [Audio import](audio/import.md): WAV, MP3 and AAC-LC decoding, metadata and preparation limits.
- [Audio device preferences](audio/devices.md): machine-local device selection and output format/buffer negotiation.

- [Shared modulation contract](modulation/contract.md).
- [Oscillator pitch and gain modulation](modulation/oscillator-pitch.md).
- [Timestretch, offline pitch, and independent implementation rule](audio/timestretch.md).
- [Timestretch acceptance plan](acceptance/timestretch.md).
- [Stem separation implementation](audio/stem-separation.md): local models, CoreML/CPU fallback, atomic import, disabled originals, and persistence.
- [Stem separation backend evaluation](audio/stem-separation-evaluation.md): local quality, speed, memory, model-license evidence and original integration proposal.
- [Charon CoreML benchmark](audio/stem-separation-coreml-evaluation.md): same-audio GPU/CPU quality, startup, memory, same-export control and reused-model measurements.

## Evidence

- [DAW branch and PR review skill](../.agents/skills/daw-review/SKILL.md): independent Documentation, Code Quality and Dependencies reviews, with source/dependency scope gates, separate findings validation, and a PASS/FAIL report that excludes SKIPPED categories.

[Validation history](validation/README.md) records completed checks and unverified behavior. It is historical evidence, not a substitute for current requirements. Keep dates and original results when moving or correcting records.

- [Performance audit — 2026-10-18](validation/performance-audit-2026-10-18.md): measured CPU/allocation baselines, scaling findings, proposed remediations, and native verification gaps.

- [Performance remediation — 2026-10-18](validation/performance-remediation-2026-10-18.md): implemented changes, before/after evidence, and remaining checks.
