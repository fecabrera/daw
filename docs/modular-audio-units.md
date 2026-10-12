# Modular audio units design and decisions

Status: design baseline; implementation pending. Last updated: 2026-10-11.

Users will build synths and effect chains from modular units in a dedicated Units view. The planned track types are audio, MIDI, and beat. Audio tracks will accept effects; MIDI tracks will accept sources and effects. A future Edit view will provide an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. The beat pattern editor will provide step toggles in audio or MIDI lanes, each with its own sources and effects. Every unit will use a shared core and support independent distribution. These are confirmed requirements. The Units MVP scope below is accepted. Detailed API, layout, and packaging choices remain proposals or implementation details.

## Terms

| Term | Meaning |
| --- | --- |
| Unit | A reusable component with declared ports, parameters, and processing behavior. |
| Source unit | A unit that generates instrument audio, such as an oscillator or sampler. |
| Clip source | Content referenced by a clip: an audio file, MIDI pattern, or beat pattern. Clip editing preserves this content; editing a pattern creates a new source. |
| Effect | A unit that processes incoming audio. |
| Modulator | A component that controls another component, such as a low-frequency oscillator (LFO) or envelope. Modulators may operate inside instruments and effects, including audio-track effects. |
| DSP | Digital signal processing: the calculations that generate or change audio samples. |
| Graph | Units and their connections, including serial chains and parallel paths. |
| Instrument | A configured source graph that produces sound from notes or triggers. It can contain multiple source units, effects, and modulators. |
| Voice | Processing state for one active note or triggered hit of an instrument. |
| Beat track | A track containing beat patterns with audio and/or MIDI lanes. |
| Host | The DAW or another application that supplies audio buffers, events, and transport information. |

## Confirmed decisions

| ID | Decision | Consequence |
| --- | --- | --- |
| D01 | Units must be modular so users can build synths and effect chains. | Avoid monolithic instruments or effects that cannot expose reusable components. |
| D02 | Add the Units view for audio sources and processing. | Users can add oscillators, effects, and other units through this view. Its layout remains open. |
| D03 | Audio tracks accept effects only. MIDI tracks accept sources and effects. | Track capabilities must constrain unit placement. MIDI tracks are a future addition. |
| D04 | All units share a base core and can ship independently. | Separate common contracts from individual implementations and DAW integration. Distribution targets are defined in D10. |
| D05 | Add a beat pattern editor in the Edit view with lanes and step toggles for easy beat creation. | Keep step editing distinct from unit/source/effect configuration. |
| D06 | Support three track types: audio, MIDI, and beat. | Keep track identity separate from editor view mode. |
| D07 | Every beat lane is an audio lane or a MIDI lane and has its own sources and effects. | Model lane type and processing independently within each beat track. |
| D08 | Future polyphonic MIDI tracks use multiple voices of one instrument. Each beat lane has one instrument; MIDI lanes can gain polyphony later, and different lanes can use different instruments. | Keep instrument configuration separate from MIDI voice state. |
| D09 | Name the views Timeline, Units, Edit, and Mixer. | Edit chooses an editor for the clip type; use these names in future view buttons and menus. |
| D10 | Distribute units as reusable Rust crates and standalone tools. | Keep unit implementations independent of the DAW and provide standalone host adapters. |
| D11 | Treat audio files, MIDI patterns, and beat patterns as clip sources. | Clip trims and loops preserve their underlying source. |
| D12 | Editing a MIDI or beat pattern creates a new source. | Other clips retain their original source and content. |
| D13 | Beat tracks support lane-level effects, parent-track effects, and independent lane controls in Units. | Keep both effect levels and lane controls separate; Mixer controls the beat track as a whole. |
| D14 | Beat-pattern length and resolution are user-set and shared by all lanes. | Keep pattern grid settings separate from lane processing. |
| D15 | Customized trigger duration is future work, including one step, until the next trigger, and stopping based on another lane. | Shared event processing must support these modes once their detailed rules are specified. |
| D16 | Users can connect beat lanes so triggering one stops another, such as a closed hi-hat stopping an open hi-hat. | The Beat MVP uses simple lane stops with a short fade; the stopping lane wins for simultaneous connected triggers. |
| D17 | Initially allow at most one active instrument voice per MIDI track and per beat lane. | Limit voices independently for each track or lane; retain future MIDI polyphony in the design. |
| D18 | Use the accepted Beat MVP scope for grid settings, step controls, playback, view navigation, and serial routing. | Follow B19–B24 in the Beat design; keep deferred features separate from the initial implementation. |
| D19 | Use serial effect chains and a small node graph for instrument construction and modulation (Q01). | Defer nested graph editing. |
| D20 | Declare unit capabilities and typed ports; validate placement in the shared core (Q02). | Allow modulators inside audio-track effects without allowing source instruments on audio tracks. |
| D21 | Use latest-note replacement and envelope restart for the initial MIDI voice (Q03). | Note-off affects only the current note; do not resume older held notes or provide legato initially. |
| D22 | Support mono/stereo audio, modulation signals, and sample-timed note/control events (Q04). | Use the project rate and reject graph cycles; delay feedback stays inside the delay unit. |
| D23 | Put effects before gain/pan on tracks and beat lanes; include serial Master effects before Master gain (Q05). | Defer sends and retain Mixer control of parent tracks. |
| D24 | Use explicit transport, mute, bypass, and atomic graph-replacement rules (Q06). | Follow the accepted lifecycle below in playback and export. |
| D25 | Initial units add no processing latency; export reported tails up to 10 seconds (Q07). | Warn if tails are truncated; defer automatic latency compensation. |
| D26 | Version unit state and preserve missing-unit data; separate pattern content from track/lane processing (Q08). | Missing instruments render silence; missing effects pass audio through. Defer nested presets. |
| D27 | Use FunDSP behind the shared unit API for the initial unit set and performance target (Q10). | Validate integration before expanding the set; keep backend types out of public and saved formats. |
| D28 | Use one Edit view with an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. | Choose the editor from the clip type. Replace the earlier standalone Beat view. |

These decisions define future work. They do not mean that MIDI/beat tracks, the Units view, Edit view, clip editors, units, or plugin loading already exist. The delivery schedule remains on the [MVP roadmap](MVP.md#future-features).

## Accepted Units MVP scope

Accepted on 2026-10-11. Q01–Q10 are resolved for the MVP through D10 and D19–D27. This scope defines future implementation; it does not add runtime features or dependencies to the current application.

| ID | Initial choice | Deferred work |
| --- | --- | --- |
| Q01 | Serial effect chains; a small node graph for instrument construction and modulation. | Nested graph editing. |
| Q02 | Declared capabilities, typed ports, and shared-core placement validation. Modulators can operate inside audio-track effects. Sound-generating instruments belong on MIDI tracks or beat lanes. | Additional placement capabilities. |
| Q03 | One active voice per MIDI track or beat lane. For general MIDI tracks, the latest note replaces the current note and restarts its envelope. Note-off affects only the current note. Beat playback follows the accepted Beat rules. | Returning to older held notes, legato, MIDI polyphony, and voice stealing. |
| Q04 | Mono/stereo audio, modulation signals, and sample-timed note/control events at the project rate. Reject graph cycles. Delay feedback is internal to the delay unit. | Graph-level feedback and additional port layouts. |
| Q05 | Source/clip output → serial effects → gain/pan → track mix. Beat lanes pass through lane effects and lane gain/pan, then sum into parent effects and parent gain/pan. Serial Master effects precede Master gain and output meters. | Sends and advanced audio routing. |
| Q06 | Stop and seek reset voices and effects. Pause preserves state. Loop boundaries end active notes while effects continue. Mute suppresses output while processing continues. Smooth bypass changes and publish prepared graph replacements atomically. | More advanced transport-state restoration. |
| Q07 | Initial units add no processing latency. Musical delay and reverb are allowed. Export reported effect tails, capped at 10 seconds, and show a notice if the cap truncates a tail. | Automatic latency compensation. |
| Q08 | Versioned unit IDs, parameters, connections, and presets. Pattern sources hold immutable note/step data; tracks and lanes own instrument/effect setups. Preserve and report missing-unit data. Missing instruments render silence; missing effects pass audio through. | Nested presets. |
| Q09 | Reusable Rust crates and standalone tools share DSP implementations; UI/device adapters stay separate. | Standalone interface and packaging details. |
| Q10 | FunDSP behind our API. Initial units: oscillators, noise, sample playback, envelope, LFO, filter, gain, delay, and simple reverb. | SoundFont instruments, spectral effects, and convolution. |

Effect chains are serial. Instrument and modulation graphs may branch internally; this does not add parallel track/lane effect routing to the Beat MVP. Graph cycles remain invalid, including within an instrument graph.

Processing latency means a unit's added input-to-output alignment delay. Musical delay and reverb are intentional sound behavior, so D25 does not exclude them. The tail cap applies to export after the arranged content ends.

The initial performance target is eight simultaneous voices with representative effects at 48 kHz and a 512-sample buffer. On a named test machine, processing should use less than half the callback time, with no callback allocations or blocking. Eight voices is a benchmark workload, not a track limit. Validate every supported project sample rate and buffer size before release; record the unit configuration, hardware, and measured results.

## View names and responsibilities

The view names were revised on 2026-10-11. D28 replaces the standalone Beat view with Edit and updates D05 and D09. The beat pattern editor remains a dedicated editor within Edit.

| View | Responsibility |
| --- | --- |
| Timeline | Arrange clips and patterns on tracks. |
| Units | Configure sources, effects, and modulators for tracks and beat lanes. |
| Edit | Edit the active clip using its audio, MIDI pattern, or beat pattern editor. |
| Mixer | Control tracks, including each beat track as a whole. |

The current application exposes Timeline and Mixer. Units and Edit remain planned additions.

### Clip editors

| Clip type | Editor in the Edit view |
| --- | --- |
| Audio | Audio editor. |
| MIDI | MIDI pattern editor. |
| Beat | Beat pattern editor. |

Select the editor from the clip being edited, rather than its track or a beat lane's instrument type. A beat clip with audio and MIDI lanes still opens the beat pattern editor. Retain the active clip context when switching between Timeline, Edit, and Units; beat clips also retain their selected lane. Unit configuration stays in Units, and Mixer controls tracks.

Audio-editor and MIDI-pattern-editor tools, the gesture for opening a clip, and Edit behavior with no active clip or multiple selected clips remain to be specified. The [beat pattern editor design](beat-editor.md) retains the accepted step-editing scope. All editors must preserve source recordings and the pattern-source isolation rules below.

## Track capabilities

| Track type | Confirmed unit placement |
| --- | --- |
| Audio | Effects only. |
| MIDI | Sources and effects. |
| Beat | Each audio or MIDI lane has its own sources, effects, and controls. The parent track has its own effects. Units configures both levels. |

The [Beat pattern editor design and decisions](beat-editor.md) defines the accepted Beat MVP scope and remaining implementation details. The beat pattern editor in Edit handles beat creation. Units configures lane sources, effects, modulators, independent controls, and parent-track effects. Mixer controls each beat track as a whole. Beat lanes should reuse the shared unit core and processing runtime described here.

### Confirmed instrument and voice model

A beat track contains audio or MIDI lanes, each with one instrument. Initially each MIDI track and each beat lane has at most one active instrument voice. The limit is per lane, so multiple beat lanes can sound at the same time. Later MIDI polyphony will provide multiple voices of the same configured instrument on a MIDI track or MIDI lane. Different beat lanes can use different instruments. One instrument may contain several source units, such as oscillators or sample players; those units form the instrument rather than separate instruments.

D17's initial scope was confirmed on 2026-10-11. Beat lanes replace the active voice on a new trigger, audio hits play to their end, and MIDI notes send note-off after one step. General MIDI tracks use latest-note replacement with envelope restart; note-off affects only the current note. Older held notes do not resume, and legato is deferred. Future MIDI polyphony limits, allocation, and stealing will be specified for a later implementation.

### Clip sources and pattern editing

Audio files, MIDI patterns, and beat patterns share a clip-source model. Clip placement, trimming, and looping operate independently of source content. Editing a pattern creates a new source for the edited clip, while other clips retain their original source. This requirement was confirmed on 2026-10-11 in D11–D12; the [Beat design](beat-editor.md#pattern-sources-and-clip-editing) records the clip-sharing example and validation criteria. Pattern sources hold immutable note/step content. Tracks and lanes own their instrument/effect setups. Changing a track instrument or its effects intentionally affects clips using that track; it is separate from editing a clip's pattern content.

## Existing architecture to preserve

- [core](../crates/core/src/lib.rs) owns project entities, stable IDs, validation, and transactional edits. Keep device and UI dependencies out of this crate.
- [engine](../crates/engine/src/lib.rs) supplies shared mixing and transport. Its current `RenderPlan::sample_at` and `render` read prepared audio at arbitrary frames. Stateful oscillators, filters, and delays require a processing runtime that advances state in sequence; adding them to random frame lookup alone is insufficient.
- [project](../crates/project/src/lib.rs) owns sessions, persistence, asset paths, prepared audio, and export orchestration.
- [output](../crates/output/src/lib.rs) owns CPAL output, device conversion, bounded queues, and renderer retirement. Preserve allocation-free, nonblocking callbacks and retire final audio owners outside them.
- The project processing rate is selectable from 44.1 to 192 kHz. Buffer sizes range from 32 to 4096 samples. Defaults are 48 kHz and 512 samples. Units must receive these settings explicitly and handle variable callback sizes.
- Live playback and headless export must use the same processing behavior. Preserve source files, clip editing rules, and older project manifests.

## Architecture and implementation guidance

### Separate the common core from its implementations

Use a small shared Rust API with composition and focused traits. Keep DSP library types private to each implementation so changing a library does not change saved projects or every other unit.

The proposed layers are:

| Layer | Responsibility | Dependency direction |
| --- | --- | --- |
| Unit core | Descriptors, typed ports, parameters, events, processing lifecycle, and state contracts. | Independent of the DAW UI, devices, and plugin formats. |
| Unit implementations | Oscillators, filters, effects, instruments, and their DSP state. | Depend on the unit core and selected DSP libraries. |
| Graph runtime | Validate connections, prepare buffers, schedule processing, and manage state. | Depends on the unit core; integrates with the engine. |
| Host adapters | DAW integration, UI editors, and standalone tool packaging. | Depend on the common core and implementations. |

Crate names and public signatures remain open. Individual units or cohesive unit families should be publishable without depending on the full DAW. Standalone tools should reuse those implementations through the shared Rust API.

### Define one processing contract

The proposed contract includes:

- Stable unit type IDs, instance IDs, and parameter IDs. Keep display names separate from identity.
- Explicit audio, control, and event ports, with declared channel layouts and connection rules.
- Parameter units, defaults, bounds, display mapping, and smoothing behavior.
- Preparation with the project sample rate and maximum block size, followed by processing with the actual frame count.
- Sample-timed note, parameter, and transport events. Reserve support for automation without implementing its editor yet.
- Reset and transport-change behavior, including seek and loop handling.
- Versioned saved state, latency in frames, and effect-tail reporting.

Prepare memory and expensive resources outside processing. No file access, allocation, blocking locks, or logging may occur in the audio callback. UI parameter changes must reach DSP through bounded, nonblocking communication.

### Store editable graphs separately from running graphs

Save unit identities, state, parameters, and connections in the project. Keep Units-view positions separate from audio behavior. Build and validate a candidate runtime outside the callback, then publish the complete change at a processing boundary. Invalid connections or failed preparation retain the previous graph.

Enforce track restrictions in shared validation and host integration, as well as the UI. A disabled menu item alone is insufficient. Composite units must expose enough capability information to validate their use on a track.

Implement serial effect chains and a small node graph for instrument construction and modulation. Reject graph cycles with a useful error. Keep feedback inside the delay unit; graph-level feedback and nested graph editing are deferred.

### Separate voice processing from track processing

The accepted MIDI path is note events → instrument voice output → serial track effects → track gain/pan → track mix. Instantiate voice state for oscillators and envelopes; share track effects where appropriate. Keep voice allocation and stealing out of individual oscillator implementations.

Use one active instrument voice per MIDI track and per beat lane initially. Keep each track or lane's voice state separate from shared effects so the same instrument model can later support MIDI polyphony. Beat lane duration and retrigger rules follow D18. General MIDI tracks use latest-note replacement with envelope restart and note-off only for the current note. No older held note resumes. Envelope release details, per-voice versus shared state, and source composition remain implementation details within this policy. Future polyphony limits and stealing will be separate decisions. The design must not assume that every unit needs one instance per voice.

### Use one runtime for playback and export

Process stateful units in sequence with the same event timing in both modes. Stop and seek reset voices and effects; pause preserves state. End active notes at loop boundaries while effects continue. Mute suppresses output but keeps processing active. Seed handling and exact state initialization remain implementation details for repeatable exports.

Rebuild affected units when the processing rate changes, using the existing preparation and atomic-publication pattern. Smooth bypass transitions and publish prepared graph replacements atomically. Initial units must add no processing latency. Musical delay/reverb are allowed, but automatic latency compensation is deferred. Export reported tails up to 10 seconds and report truncation.

## Library findings and recommendations

Library findings were reviewed on 2026-10-11. FunDSP is the accepted MVP backend behind the shared unit API. Its integration is pending. The other libraries remain candidates for later or specific needs; their inclusion is not approved by this scope.

| Library | Findings | Proposed role | License reported by the project |
| --- | --- | --- | --- |
| [FunDSP](https://github.com/SamiPerttu/fundsp) | Provides synthesis and effects, static `AudioNode` and dynamic `AudioUnit` components, and dynamic `Net` graphs with separate editing and rendering sides. | Accepted MVP backend for built-in sources and effects behind our unit API. | MIT or Apache-2.0 |
| [dasp](https://github.com/RustAudio/dasp) | Supplies modular low-level sample/frame operations, interpolation, envelope detection, metering, and graph utilities. | Add specific components when custom DSP needs them. | MIT or Apache-2.0 |
| [RustFFT](https://docs.rs/crate/rustfft/latest) | Supplies FFT processing with SIMD support. It is a building block for spectral processing. | Add when implementing spectral effects, convolution, or analysis. | MIT or Apache-2.0 |
| [RustySynth](https://github.com/sinshu/rustysynth) | Provides SoundFont MIDI synthesis in pure Rust for real-time and offline use. | Optional sampled-instrument source unit. | MIT |
| [synfx-dsp](https://docs.rs/synfx-dsp/latest/synfx_dsp/) | Provides synthesis and effect algorithms. Its documentation specifies GPL-3.0-or-later and a nightly requirement for SIMD. | Secondary candidate, subject to toolchain and distribution requirements. | GPL-3.0-or-later |

FunDSP is the selected starting point. Its [block interface](https://github.com/SamiPerttu/fundsp#block-processing) accepts up to 64 samples per call, so the adapter must split larger project buffers and preserve event offsets across those calls. Set its sample rate explicitly. Its [dynamic graph frontend and backend](https://github.com/SamiPerttu/fundsp#net) are candidates for implementation; our saved graph and public unit API should remain independent of them.

Keep the existing Rubato conversion, CPAL device output, and offline Rubber Band clip preparation in their current roles. They do not replace the unit runtime. Verify selected versions, features, transitive dependencies, licenses, and callback behavior before adoption. SoundFont content needs its own distribution terms, separate from RustySynth's code license.

## Independent distribution

Q09 was resolved on 2026-10-11: units will ship as Rust crates and standalone tools, recorded in D10.

| Form | Purpose |
| --- | --- |
| Rust crates | Reuse the unit core and implementations in the DAW or other Rust applications. |
| Standalone tools | Run unit implementations through an independent application that supplies host services. |

Keep UI editors optional and provide standalone adapters without changing DSP code. The standalone tools' interface, device support, and packaging remain to be specified. The roadmap's plugin-engine work remains a separate feature.

## Question status and remaining details

The Q01–Q10 scope choices were accepted on 2026-10-11. Preserve these IDs as decision history. The [accepted Units MVP scope](#accepted-units-mvp-scope) closes the initial behavior and library choices; deferred features are not blockers for the MVP.

| ID | Status | Decision record |
| --- | --- | --- |
| Q01 | Resolved for MVP. | Serial effects and instrument/modulation graphs: D19. |
| Q02 | Resolved for MVP. | Capabilities, typed ports, and placement validation: D20. |
| Q03 | Resolved for MVP. | Latest-note replacement and one-voice behavior: D17 and D21; Beat rules: D18. |
| Q04 | Resolved for MVP. | Initial ports, timing, rate, and cycle rules: D22. |
| Q05 | Resolved for MVP. | Lane, track, and Master routing: D23. |
| Q06 | Resolved for MVP. | Transport, mute, bypass, and graph publication: D24. |
| Q07 | Resolved for MVP. | No added processing latency and capped export tails: D25. |
| Q08 | Resolved for MVP. | Versioned state, ownership, and missing-unit behavior: D26. |
| Q09 | Resolved. | Rust crates and standalone tools: D10. |
| Q10 | Resolved for MVP. | FunDSP, initial unit set, and performance target: D27. |

Remaining implementation details are:

- Public API and crate names; Units layout; exact modulation rates, channel conversion, and simultaneous-event ordering.
- Note identity and stale note-off handling, envelope/release settings, unit defaults, smoothing and bypass transitions, and voice/shared state allocation.
- Saved schema and version migrations, handling unsupported versions, pattern-to-lane identity mapping, and preset format.
- Tail reporting for each effect and repeatable random-seed initialization.
- FunDSP version/features and dependency/license checks; exact benchmark graph, named test hardware, memory measurements, and native-device validation.
- Standalone interface, device support, and packaging.

These details must preserve the accepted behavior. Specify future polyphony, nested editing/presets, advanced routing, and latency compensation when those features enter scope.

## Proposed implementation sequence

1. Define the shared API and persisted graph model within the accepted scope. Specify the remaining implementation details needed for the first units.
2. Validate the architecture with a small MIDI synth graph using one voice per MIDI track, and an audio-track effect graph. Keep the initial unit set small.
3. Integrate block processing, events, transport, and atomic graph publication into the shared engine.
4. Add the Units view and project persistence over that same model. Add Edit with editor selection by clip type. Implement the accepted beat pattern editor MVP scope over shared event scheduling and unit processing, after specifying its remaining implementation details.
5. Package reusable Rust crates and add standalone tools after the unit contract works in both playback and export.

Before expanding the unit set, check source rejection on audio tracks, invalid-graph rollback, parameter smoothing, event timing, every supported project rate/buffer size, variable block sizes, save/reopen, and playback/export consistency. Measure callback allocation and blocking behavior, CPU/memory use, and native-device stability. Verify latest-note replacement and stale note-off handling, transport resets, preserved pause state, loop voice termination with continuing effects, mute processing, smooth bypass, and atomic graph replacement. Test the absence of added processing latency, tail rendering and truncation notices, missing-unit fallback/data preservation, and pattern-versus-track ownership. Measure the accepted eight-voice workload and record its hardware and results.

Verify that audio, MIDI, and beat clips open their respective editors in Edit, including beat clips with mixed audio/MIDI lanes. Check clip/lane context retention across Timeline, Edit, and Units. Beat-specific validation criteria are recorded in the [beat pattern editor design](beat-editor.md#validation-criteria).

## Decision record maintenance

Keep D01–D28 as confirmed requirements and accepted choices. Add accepted design choices with a decision date and rationale. Until accepted, retain proposals and open questions with their current status. Record implementation and validation separately; a library's advertised behavior does not establish integration performance.

Related documents: [beat pattern editor design](beat-editor.md), [repository guidelines](../AGENTS.md), [scope and roadmap](MVP.md), [current implementation specification](../specs/mvp-iteration-1.md), and [validation history](validation.md).
