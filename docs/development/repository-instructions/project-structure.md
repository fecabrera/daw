# Project structure

Keep dependencies flowing from applications toward shared libraries. Do not introduce dependency cycles.

## Unit DSP ownership

Put new DSP functions and stateful DSP building blocks for built-in sources,
effects and modulators in `crates/dsp-support`, including algorithms with one
caller. Check for an existing building block first. When changing an existing
algorithm, move it to this crate if it still lives in a unit crate.

Unit crates map settings and modulation to parameters, own prepared instances,
and call the building blocks in signal order. Keep unit lifecycle, bypass,
monitoring and latency reporting in the unit. Keep backend types private and
follow the [shared unit DSP decision](../../../specs/architecture/modular-audio-units.md#shared-unit-dsp).

## Crate responsibilities

The table includes existing unit implementations at their current locations.

| Location | Responsibility |
| --- | --- |
| `crates/unit-core` | Portable unit descriptors, port types, processing context, source lifecycle, curve definitions, and versioned oscillator/instrument/envelope/effect settings. No DAW, UI, or device dependencies. |
| `crates/granulizer` | Portable bounded stereo grain synthesis, deterministic randomness, scan/pitch separation and windowing over host-owned PCM; no project, UI, media, or device dependencies. |
| `crates/sampler` | Shared sample-source facade plus portable prepared stereo sample playback, local position, gain/power ramps, and boundary fades; no project, UI, media, or device dependencies. |
| `crates/noise` | Portable deterministic stereo White/Pink/Brown noise, Color/Width/Level smoothing and shared modulation; no project, UI, media or device dependencies. |
| `crates/oscillator` | FunDSP-backed oscillator sources behind the shared unit API; keep backend types private. |
| `crates/equalizer` | Reusable stereo 3-band EQ behind the shared effect API; keep FunDSP types private. |
| `crates/saturator` | Portable stereo saturation, fixed 4× FIR oversampling, aligned dry/bypass paths, and shared modulation; no DAW, UI, media, or device dependencies. |
| `crates/harmonic-lift` | Portable six-branch harmonic enhancement, global drive, linked maximization and aligned 2× quality paths; no DAW, UI or device dependencies. |
| `crates/exciter` | Portable variable-band harmonic excitation, shared LR4 crossovers/oversampling, typed modulation and live-only band audition; no DAW, UI or device dependencies. |
| `crates/chorus` | Portable stereo modulated delays, bounded feedback, phase-continuous sine voices, width and shared modulation/monitoring; no DAW, UI, media, or device dependencies. |
| `crates/reverb-core`, `crates/plate-reverb`, `crates/ambient-reverb` | Prepared stereo reverb kernels, diffusion and feedback storage, product processing, shared modulation and output monitoring; no DAW, UI, media or device dependencies. |
| `crates/shimmer-reverb` | Portable stereo vowel processing, bounded two-voice pitch regeneration and contractive late reverb tank, shared modulation and monitoring; no DAW, UI, media or device dependencies. |
| `crates/bitcrusher` | Portable stereo quantization, whole-ratio sample holding, and shared modulation; no DAW, UI, media, or device dependencies. |
| `crates/stereo-spread` | Portable spectral stereo widening with private FunDSP filter banks and shared modulation; no DAW, UI, media, or device dependencies. |
| `crates/direction-mixer` | Portable stereo direction/width and M/S decoding through the shared effect and modulation APIs; no DAW, UI, or device dependencies. |
| `crates/dsp-support/analysis-kernels` | Dependency-free BSD-2-Clause live spectrum and loudness kernels used by DSP support and standalone analysis. |
| `crates/dsp-support` | Home for unit DSP algorithms and building blocks. Provides filter, dynamics, delay, reverb, source, modulation, and numerical measurement blocks. |
| `crates/drive` | Reusable stereo soft-clipping drive with color/tone filters behind the shared effect API; keep FunDSP types private. |
| `crates/saturator-plugin`, `crates/bitcrusher-plugin` | Typed plugin identities, parameters, bounded state and prepared adapters over the portable DSP. Share format/editor infrastructure; no DAW or SDK ownership. |
| `crates/saturator-editor`, `crates/bitcrusher-editor` | Shared headerless body bindings, bounded host gestures, appearance and output telemetry. |
| `plugins/saturator-*`, `plugins/bitcrusher-*` | Independent CLAP/VST3/AUv2 products over the existing narrow format sources. |
| `crates/drive-plugin` | Format-neutral Drive plugin identity, typed parameter mappings, checked panel bindings, bounded state envelope/publication, and prepared processor over the original Drive DSP. No DAW, SDK, window, or device dependencies. |
| `crates/unit-ui` | Shared unit knobs, portable parameter controls, theme/fonts and shared effect bodies. Only egui and unit-core dependencies; no DAW project, SDK, window or device dependencies. |
| `crates/drive-editor` | Drive body bindings, bounded editor/control messages, gestures and instance appearance. No SDK or audio ownership. |
| `crates/plugin-window` | Native child-window adapter over the selected egui backend. Keep platform parent handles, main-thread lifetime and display scaling here. |
| `crates/plugin-support` | Format-neutral ABI containment, fixed atomic snapshots, and bounded UTF-8 text helpers. No SDK, DAW, window, or device dependencies. |
| `crates/plugin-host` | Control-thread external plugin discovery, native executable checks, fingerprints, catalog cache and isolated scan supervision. Uses the existing narrow format bindings; no callback filesystem or loader work. Active-instance hosting remains pending. |
| `crates/plugin-clap` | Narrow CLAP binding facade; reexports shared boundary helpers. No concrete unit, DAW, window, or device dependencies. |
| `crates/plugin-auv2` | Locally generated Apple C bindings, checked native buffer views, Cocoa bridge and ABI containment; follow [AUv2 exports](../../../specs/architecture/auv2-plugin-builds.md). |
| `crates/plugin-vst3` | Narrow VST3 binding facade, platform TUID encoding, UTF-16 text and borrowed stream helpers. No concrete unit, DAW, window, or device dependencies. |
| `plugins/drive-vst3` | Independent Drive VST3 factory, component/audio processor, edit controller, normalized queues, and host/state boundary. Keep COM ownership here; reuse shared parameter/state, publication and DSP. |
| `plugins/drive-clap` | Independent Drive CLAP entry, lifecycle, host/event/state bridge, and planar audio boundary. Keep unsafe SDK ownership here; reuse shared parameter/state and DSP crates. |
| `crates/filter` | Portable stereo filtering, parameter/bypass smoothing and shared modulation; owns private FunDSP coefficients and integrators reused by Auto-filter, plus the shared LR4 crossover used by OTT and Exciter. |
| `crates/auto-filter` | Shared Filter processor plus a transport-locked LFO behind the effect API. |
| `crates/ott` | Portable three-band upward/downward dynamics, phase-matched crossovers, and signed gain monitoring; keep FunDSP types private. |
| `crates/limiter` | Portable stereo Peak/True Peak limiter, L/R and M/S dynamics, fixed latency, and gain-reduction monitoring; no DAW/UI/device dependencies. |
| `crates/compressor` | Portable linked stereo dynamics and gain-reduction monitoring behind the shared effect API; keep FunDSP detector types private. |
| `crates/amplitude` | Reusable per-voice amplitude stage, note-gate bypass, and 5 ms gain/power ramps; no DAW, UI, or device dependencies. |
| `crates/macro` | Portable shared Macro strips behind the common source/destination contract; no DAW, UI, or device dependencies. |
| `crates/lfo` | Portable continuous modulation source, analytic waveform smoothing, and shared Auto-filter clock/wave primitives; no DAW, UI, or device dependencies. |
| `crates/envelope` | Reusable, allocation-free per-note ADSR modulator; no DAW, UI, or device dependencies. |
| `crates/core` | Project entities, stable IDs, frame ranges, colors, validation, and transactional edit commands. No UI, device, or file-dialog dependencies. |
| `crates/timestretch` | Independent BSD-2-Clause offline Rust DSP, portable requests, resource/cancellation checks; no DAW or Rubber Band dependency. |
| `crates/timestretch-cli` | Independent WAV comparison/benchmark tool, optional R2/R3 reference adapters, reports, and portable scripts; no DAW dependency. |
| `crates/media` | WAV decoding, sample-rate conversion, waveform peaks, WAV/MP3 encoding, and selected offline stretch adapters. |
| `crates/engine` | Shared render plans, sample mixing, transport, gain/pan, fades, and meter calculations. No window, file-dialog, or device initialization. |
| `crates/separation` | Offline Charon HTDemucs adapter, local models, CoreML/CPU fallback, cancellation and streaming output through shared media encoding. No UI or device dependencies. |
| `crates/project` | Sessions, manifests, source paths, import orchestration, prepared audio caches, save/open, and export orchestration. |
| `crates/output` | CPAL device output, device-rate conversion, preallocated command queues, atomic status, and renderer retirement. |
| `crates/ui` | egui application state, interactions, previews, reusable controls, and presentation. |
| `apps/desktop` | Platform startup, GPU backend selection, and native menu, window, and file-drag adapters. Linux uses GTK GLArea/OpenGL with Custom/Native titlebar modes. Windows uses eframe Glow/OpenGL for native caption composition; macOS uses eframe wgpu. Keep OS-specific code here. |
| `apps/cli` | clap arguments, diagnostics, and exit codes over the shared project/export APIs. Must remain usable without graphics or an audio device. |
| `apps/plugin-scan` | Disposable main-thread VST3/CLAP factory inspection through plugin-host. Keep foreign factory calls inside this process; no project activation or editor creation during discovery. |
| `plugins/drive` | Build-time Drive plugin manifest; stable public identities, parameter declarations, and two-strip panel bindings. |
| `crates/*-plugin`, `crates/*-editor` | Independent typed contracts and native editor bindings for effect products; reuse bounded state exchange, portable DSP, shared controls, and meters. |
| `plugins/<unit>`, `plugins/<unit>-clap`, `plugins/<unit>-vst3`, `plugins/<unit>-auv2` | Checked product manifests and independent export targets. Shared format boundaries compile through concrete dependency aliases. |
| `tools` | Demo generation, CLI smoke checks, CI summaries, and macOS/Windows/Linux packaging. |
| `.github/workflows` | Native platform tests and release-package artifact uploads. |
| `examples` | Tracked generated audio and demo project fixtures. |
| `docs` | User guides: tasks, controls, observable behavior, and practical limits; `docs/development` contains developer setup/build/package guides. |
| `specs` | Implementation requirements, architecture, data formats, acceptance criteria, and validation history. |
| `work`, `target` | Ignored scratch files, dependency cache, logs, and build outputs. |

## Shared ownership and dependencies

- Put behavior in the crate that owns it. Do not duplicate rendering or validation in the UI or CLI.
- Follow the accepted [source-container architecture](../../../specs/architecture/modular-audio-units.md#one-source-container-per-track): one processing root per track, with reusable source units inside MIDI instruments and independent lanes inside beat instruments. Keep immutable clip content separate from processing state.
- Use focused modules for new cohesive behavior. Keep application composition and public entry points in `lib.rs` or `main.rs`; avoid adding unrelated responsibilities to large files.
- Every modulation provider and destination must use the [shared modulation contract](../../../specs/modulation/contract.md) in `unit-core`. Reuse its source publication, metadata/discovery, scope checks, typed destination entry point, route validation/resolution, and additive offset smoother. Do not implement unit-specific routing loops or source lookup in consumers. Continuous providers must use shared context, dynamic range metadata, and `Continuous` behavior; they cannot own note release. Reuse the contour/continuous resolver composition for amplitude. See [LFO](../../../specs/instruments/lfo.md). Keep note scheduling and concrete unit construction in the host. Apply final parameter bounds in the portable unit before running the DSP building blocks.
- Layer effect chains follow [per-layer effects](../../../specs/instruments/layer-effects.md). Reuse the existing effect chain, catalog, panels, modulation, monitoring, and drag helpers; keep DSP per bus rather than per voice, align layer latency before summing, and validate changes through `Edit::SetLayerEffects`. Cross-layer effect groups use `Project::transfer_effects` for atomic moves/copies and `EffectInstance::duplicate` for fresh copy identities.
- Prefer composition through structs and functions. Use traits when implementations need to vary; avoid a universal panel or engine interface.
- Share dependency versions in the workspace when multiple crates use them. Keep native dependencies behind target-specific configuration. Retain `Cargo.lock` for reproducible application builds.
- Use Rust naming conventions and rustfmt. Add comments for invariants, units, lifetime constraints, and non-obvious decisions. Return useful errors at file, media, and device boundaries; do not use panics for expected failures.
