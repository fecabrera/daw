# Modular audio units design and decisions

Status: design baseline; implementation pending. Last updated: 2026-10-12.

Users will build synths and effect chains from modular units in a dedicated Units view. The planned track types are audio, MIDI, and beat. Audio tracks will accept effects; MIDI tracks will accept sources and effects. A future Editor view will provide an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. The beat pattern editor will provide step toggles in audio or MIDI lanes, each with its own sources and effects. Every unit will use a shared core and support independent distribution. These are confirmed requirements. The Units MVP scope below is accepted. Detailed API, layout, and packaging choices remain proposals or implementation details.

## Terms

| Term | Meaning |
| --- | --- |
| Unit | A reusable component with declared ports, parameters, and processing behavior. |
| Source unit | A unit that generates instrument audio, such as an oscillator or sampler. |
| Source container | The track-owned processing root that combines its source units or lane instruments into one audio output bus. |
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
| D03 | Audio tracks accept effects only. MIDI tracks accept sources and effects. | Track capabilities must constrain unit placement. Basic MIDI tracks and oscillator instruments are implemented; effect chains remain pending. |
| D04 | All units share a base core and can ship independently. | Separate common contracts from individual implementations and DAW integration. Distribution targets are defined in D10. |
| D05 | Add a beat pattern editor in the Editor view with lanes and step toggles for easy beat creation. | Keep step editing distinct from unit/source/effect configuration. |
| D06 | Support three track types: audio, MIDI, and beat. | Keep track identity separate from editor view mode. |
| D07 | Every beat lane is an audio lane or a MIDI lane and has its own sources and effects. | Model lane type and processing independently within each beat track. |
| D08 | Future polyphonic MIDI tracks use multiple voices of one instrument. Each beat lane has one instrument; MIDI lanes can gain polyphony later, and different lanes can use different instruments. | Keep instrument configuration separate from MIDI voice state. |
| D09 | Name the views Timeline, Units, Editor, and Mixer. | Editor chooses an editor for the clip type; use these names in future view buttons and menus. |
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
| D28 | Use one Editor view with an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. | Choose the editor from the clip type. Replace the earlier standalone Beat view. |

D29 (2026-10-11) updates the MIDI voice scope: MIDI patterns support simultaneous pitches in playback and export. Drawing replaces notes only on the same pitch, with trim, split, and removal behavior. Prepare voice slots for the actual note overlap and release tails before publication; no fixed voice cap or callback allocation. Beat lanes retain their accepted one-voice MVP scope.

D30 (2026-10-12) defines one source container per track: clip playback for Audio, one instrument for MIDI, and one beat instrument containing lanes for Beat. Reusable oscillator units belong inside an instrument. Each container exposes one audio output bus to track effects and gain/pan. This is the accepted target architecture; the first MIDI source container now holds a summed oscillator list. Full graph containers remain pending.

These decisions define the full target scope. Basic MIDI tracks, patterns, oscillator units, and the MIDI Editor view are implemented; beat tracks, graph tools, other clip editors, and plugin loading remain pending. The delivery schedule remains on the [MVP roadmap](MVP.md#future-features).

## Accepted Units MVP scope

Accepted on 2026-10-11. Q01–Q10 are resolved for the MVP through D10 and D19–D27. This scope defines the full Units MVP. The first MIDI/oscillator implementation is described below; the remaining unit set and graph features are pending.

| ID | Initial choice | Deferred work |
| --- | --- | --- |
| Q01 | Serial effect chains; a small node graph for instrument construction and modulation. | Nested graph editing. |
| Q02 | Declared capabilities, typed ports, and shared-core placement validation. Modulators can operate inside audio-track effects. Sound-generating instruments belong on MIDI tracks or beat lanes. | Additional placement capabilities. |
| Q03 | Polyphonic MIDI tracks under D29, with independent note-offs and prepared voice slots. One active voice per beat lane. Beat playback follows the accepted Beat rules. | Legato, configurable voice limits/stealing, and MIDI-lane polyphony. |
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

The view names were revised on 2026-10-11. D28 replaces the standalone Beat view with Editor and updates D05 and D09. The beat pattern editor remains a dedicated editor within Editor.

| View | Responsibility |
| --- | --- |
| Timeline | Arrange clips and patterns on tracks. |
| Units | Configure sources, effects, and modulators for tracks and beat lanes. |
| Editor | Edit the active clip using its audio, MIDI pattern, or beat pattern editor. |
| Mixer | Control tracks, including each beat track as a whole. |

The clip-editing view was renamed from Edit to Editor on 2026-10-12. The application exposes Timeline, Units, Editor, and Mixer. Units currently configures an oscillator instrument; Editor currently provides the MIDI piano roll. The remaining source/effect graph tools, audio editor, and beat editor are pending.

### Clip editors

| Clip type | Editor in the Editor view |
| --- | --- |
| Audio | Audio editor. |
| MIDI | MIDI pattern editor. |
| Beat | Beat pattern editor. |

Select the editor from the clip being edited, rather than its track or a beat lane's instrument type. A beat clip with audio and MIDI lanes still opens the beat pattern editor. Retain the active clip context when switching between Timeline, Editor, and Units; beat clips also retain their selected lane. Unit configuration stays in Units, and Mixer controls tracks.

The first MIDI editor and opening gesture are defined in [MIDI patterns and oscillators](midi-patterns.md). Audio-editor tools and further multi-clip editor behavior remain to be specified. The [beat pattern editor design](beat-editor.md) retains the accepted step-editing scope. All editors must preserve source recordings and the pattern-source isolation rules below.

## Track capabilities

| Track type | Confirmed unit placement |
| --- | --- |
| Audio | Effects only. |
| MIDI | Sources and effects. |
| Beat | Each audio or MIDI lane has its own sources, effects, and controls. The parent track has its own effects. Units configures both levels. |

The [Beat pattern editor design and decisions](beat-editor.md) defines the accepted Beat MVP scope and remaining implementation details. The beat pattern editor in Editor handles beat creation. Units configures lane sources, effects, modulators, independent controls, and parent-track effects. Mixer controls each beat track as a whole. Beat lanes should reuse the shared unit core and processing runtime described here.

### Confirmed instrument and voice model

A beat track contains audio or MIDI lanes, each with one instrument. Beat lanes initially have one active voice each, so multiple lanes can sound at the same time. MIDI tracks now support multiple simultaneous voices of the same configured instrument under D29. Different beat lanes can use different instruments. One instrument may contain several source units; those units form the instrument rather than separate instruments.

D17 originally specified one voice for both MIDI tracks and beat lanes. D29 supersedes that limit for MIDI tracks. Beat lanes retain retrigger replacement, audio hits playing to their end, and MIDI notes sending note-off after one step. General MIDI notes now sound independently, including after seeking into a chord. Voice slots account for overlapping notes and fixed release tails, and are allocated before processing. Configurable limits, stealing, legato, and polyphonic MIDI beat lanes remain future work.

### One source container per track

| Track | Source container | Contents |
| --- | --- | --- |
| Audio | Clip playback | Audio from the track's clips. Playback is host-managed; users can add effects only. |
| MIDI | Instrument | Reusable oscillator or other source units, modulators, and internal processing. |
| Beat | Beat instrument | Audio and MIDI lanes, each with its own sample source or MIDI instrument, controls, and effects. |

The track owns one container and receives one audio output bus from it. Track-level effects and gain/pan process that output; Mixer controls the track as a whole. A bus can carry multiple channels. Container ownership does not limit the number of oscillators, voices, or sounding beat lanes.

A MIDI instrument owns its internal graph and note/voice allocation. Users can add multiple oscillator units inside that instrument and route them through internal processing. Units reuse the shared core and base panel. Each active voice uses the same configured graph with independent oscillator runtime state. This keeps source implementations reusable without attaching several independent instruments directly to one MIDI track.

A beat instrument owns its lanes and combines their outputs after lane effects and controls, before parent-track effects and gain/pan. Each lane retains its own trigger handling, voice state, and lane-stop connections. The Beat MVP's one-voice limit applies separately to each lane. Internal branching within an instrument does not add parallel lane effects, shared sends, or audio sidechains to that MVP.

Clip sources remain immutable content: audio files, MIDI patterns, and beat patterns. They are separate from the track-owned processing container. Copying or editing a pattern retains the source-isolation rules below; changing an instrument intentionally affects clips on its track or lane.

The current MIDI implementation stores one oscillator instrument per track with a list of source configurations. Each note prepares independent runtime state for all sources and sums their outputs. Full graph container APIs, graph serialization, and the UI for internal routing still need implementation design. Nested graph editing remains deferred under Q01.

### Clip sources and pattern editing

Audio files, MIDI patterns, and beat patterns share a clip-source model. Clip placement, trimming, and looping operate independently of source content. Editing a pattern creates a new source for the edited clip, while other clips retain their original source. This requirement was confirmed on 2026-10-11 in D11–D12; the [Beat design](beat-editor.md#pattern-sources-and-clip-editing) records the clip-sharing example and validation criteria. Pattern sources hold immutable note/step content. Tracks and lanes own their instrument/effect setups. Changing a track instrument or its effects intentionally affects clips using that track; it is separate from editing a clip's pattern content.

## First MIDI/oscillator implementation

Implemented on 2026-10-11. See [MIDI patterns and oscillators](midi-patterns.md) for the model, controls, timing, and limits. `daw-unit-core` owns the portable source contract and versioned settings; `daw-oscillator` hides FunDSP. MIDI patterns are embedded immutable assets, and tracks own their oscillator instrument settings. Since 2026-10-12, the Oscillator panel contains multiple source columns with independent waveform, pitch, and gain. Each source strip has a centered 18-point circular Lucide Power toggle below its knobs, without a strip name label. Retain the source number in its tooltip and accessibility name. On uses the view controls’ accent-tinted fill with a 1-point border and icon in the current accent color. Off uses the secondary text color, matching the former dim label, for both icon and border. Source mute retains its gain/settings, saves with the instrument, and defaults to false when absent. Muted sources keep processing with zero output. Oscillator strips have a fixed 90-point width, including padding, independent of source count. Each strip has its own container with equal internal padding and a full-height separator before the next strip, including Add. The add strip on the right contains an 18-point circular Plus button centered on both axes, using Power’s shared circle, border, inactive colors, icon scale, and hover feedback. Both visible circles have the same 18-point outer diameter, including their border. The whole add strip is one button with a background highlight on hover/focus; clicking anywhere in it, including padding, adds one default source on release; each column and knob exposes Remove oscillator in its context menu. Empty instruments render silence. Saved single-oscillator instruments migrate to a one-item list. Playback and export use sequential per-track polyphonic voices with independent note-offs and prepared voice slots.

### Shared unit panel

All source, effect, and modulator units in Units must use the same base panel, provided by `PanelStyle::unit` in [panels.rs](../crates/ui/src/panels.rs). Do not build separate panel shells for individual units. Each unit supplies its name and controls to this shared component.

- The header spans the panel’s inner width and displays the unit name, without repeating the track name.
- Use the active application accent for the header background and the mixer strip’s contrasting foreground rule for centered text. Deep Orange and Purple use bright text; Yellow uses dark text. Update both when the accent changes.
- Share the mixer strip’s 24-point header height and 11-point text size through the common panel constants.
- Use the current theme’s neutral panel background for controls, with the shared thin border and 10-point body padding. Keep the header flush with the panel edges inside that border. Share one 1-point divider between the playback toolbar and Units, without an extra gap or stacked top borders.
- Reuse the standard knobs, faders, numeric fields, and dropdowns. Individual units define their parameters inside the body; they retain the same panel shell in both themes.

The Oscillator panel uses this base panel. Its body delegates the shared padding to individual source and add containers, with no additional outer padding or gaps. All strips have equal height; each source centers its knobs and Power button in one vertical column. Future units must reuse the base panel.

### Shared knob base

All knob types use the `Knob` widget and `KnobSpec` in [knobs.rs](../crates/ui/src/knobs.rs). The base owns painting, bounds or cyclic wrapping, choice diagrams, focus and disabled-input guards, fractional drag accumulation, Shift sensitivity, arrow-key steps, double-click reset, tooltips, and accessibility. Parameter wrappers declare their ranges, scales, steps, reset values, and formatting. Gain, pan, pitch, and waveform reuse this path; future types must also reuse it. Keep the standard 28-point knob footprint, face, and pointer. Bounded controls retain their value arc and reference marks. Cyclic choices use a full circle with equal angular spacing, and optional diagrams replace the marks. The waveform selector reserves a 52-point footprint for four 10 × 6-point vector diagrams around the same knob face; its active diagram uses the accent color. Stepped knobs retain fractional movement between frames so slow drags can cross a step, and Shift can change during a drag without a jump.

The Oscillator stacks compact rows in this order: Waveform, Pitch, Gain. Each shared row contains an aligned standard knob without a parameter-name label. Retain parameter names and current values in tooltips and accessibility information. Remove the separate waveform name and pitch/gain value boxes. This follows the confirmed vertical layout choice. Waveform cycles through four steps: Sine, Saw, Square, Triangle, then back to Sine. Dragging has no end stop in either direction; arrow keys also wrap. All three Oscillator rows reserve 52 points of height for diagrams or value/endpoint labels; all knob centers remain aligned. Pitch uses one-semitone steps, −48 to +48. Show reference marks at every multiple of 12 semitones across that range, with a longer, brighter zero mark and a small current pitch value above it. Label the pitch endpoints “−48” and “+48” below their marks. Gain knobs support optional reference ticks at −∞, −40, −30, −20, −15, −10, −6, −3, 0, +3, +6, +9, and +12 dB. Enable them in Oscillator, with a stronger unity mark, the current gain in dB above it (−∞ for silence), and −∞/+12 endpoint labels. The base knob supports a reference interval or explicit reference values, optional current-value formatter, and optional endpoint labels independent of the input step, so the hints do not change adjustment. Current-value and endpoint labels use 9-point numeric theme text. Current values stay above the zero/unity position and update through the base knob; endpoint labels sit below their fixed marks. Endpoint labels stay fixed as the current value changes and fit within the existing 52-point footprint. Compact Timeline gain knobs retain their layout without optional scale ticks or labels. All gain knobs reuse the fader’s silence-to-+12 dB scale, adapted to place unity at twelve o’clock. The amplitude tail below −60 dB reaches exact silence; keyboard steps can enter and leave that endpoint. Gain and Pitch remain adjustable through their knobs and tooltips. During playback, changing gain, pitch, or mute retains oscillator phase and note/envelope state, with 5 ms amplitude or frequency ramps. Retain in-flight ramps when an unrelated control changes. The persisted `transpose` field remains compatible with older projects. Double-click resets Waveform to Sine, Pitch to zero, and Gain to 0 dB. Timeline and Mixer gain/pan knobs use the same base; their surrounding layouts stay specific to those views.

### Shared gain fader

All gain faders use [faders.rs](../crates/ui/src/faders.rs). The shared scale runs from exact silence (−∞) to +12 dB, with extra travel near unity. Label +12, +9, +6, +3, 0, −3, −6, −10, −15, −20, −30, −40, and −∞. The 0 dB tick is longer and brighter. Subdivisions use 1 dB around unity, 2 dB between −20 and −40, 5 dB below −40, and 1 dB above unity; omit ticks when spacing is below 3 points. At reduced fader heights, reduce the labels before they collide. Reserve the scale width inside the track or Master strip. The handle and all ticks use the same piecewise mapping; the tail below −60 dB follows amplitude continuously down to zero.

Shift reduces pointer travel tenfold and can change during a gesture without jumping. Idle frames and release preserve the gain; double-click resets it to 0 dB. The silence position changes gain without toggling mute/solo. Gain fields in Timeline and Mixer share formatting and parsing, displaying −∞ and accepting −∞, -inf, or -infinity. Master input clamps to the fader limits; track entry retains finite values outside the slider range on display. Gain knobs cover the same silence-to-+12 dB range and do not alter a silent value until edited.

Project data uses the finite `daw_core::SILENT_GAIN_DB` value (−1000 dB), whose conversion through the existing `linear_gain` underflows to exact zero. This retains the existing manifest schema and avoids non-finite JSON numbers. Live playback, saved/reopened projects, and offline rendering use the same gain. Oscillator source gain also accepts exact silence through +12 dB. The silence constant belongs to portable `daw_unit_core` and is re-exported by `daw_core`; saved oscillator settings remain finite and keep their state version.

This is the first working path. Serial effects, graph connections, other source/modulator units, standalone interfaces, and the full Units performance workload remain pending.

### Shared stereo meter

Track and Master meters in Timeline and Mixer use [meters.rs](../crates/ui/src/meters.rs). Mixer shows one scale for both channels. Timeline shows only the bars, without scale labels, ticks, or reserved scale width. Both use a linear −60 to 0 dBFS display, with Mixer marking 0, −3, −6, −9, −12, −15, −18, −21, −24, −30, −35, −40, −45, −50, and −60. Emphasize zero. Add 1 dB subdivisions above −30 and 5 dB below it when spacing is at least 3 points. Fit labels inside the panel and omit interior labels that collide at short heights. Reserve a 24-point scale column only in Mixer; its strips are 120 points wide.

In Mixer track and Master strips, center the gain field above the complete fader column and a matching read-only peak field above the complete stereo meter column. Both fields share a row and the standard centered 36-point numeric style; include the scale labels and tick marks in column centering. The peak field displays the highest L/R peak since its last reset in dBFS with one decimal place, −∞ for silence, and a plus sign above zero. It holds that maximum as the live bars fall, including pause, Stop, and view changes. Clicking the field resets it to −∞; subsequent output can raise it again. The output callback captures peaks with atomic maximum updates, so brief peaks between UI frames remain available. Track holds follow track IDs through reorder and live plan updates. Holds reset with a new output monitor or project and are not saved in project data. Reset does not change gain, current meter bars, or clipping state.

Meter fill uses the same dBFS mapping as the scale. Silence and peaks at or below −60 dBFS show no fill; peaks above zero fill the rail and retain the existing clipping indication. Keep actual dBFS values in tooltips, independent channel clipping, and click-to-clear. The scale changes display only; source generation, gain, rendering, and project data remain unchanged.

## Existing architecture to preserve

- [core](../crates/core/src/lib.rs) owns project entities, stable IDs, validation, and transactional edits. Keep device and UI dependencies out of this crate.
- [engine](../crates/engine/src/lib.rs) supplies shared mixing and transport. `RenderPlan::sample_at` and `render` read prepared audio at arbitrary frames. MIDI oscillators advance through the sequential runtime shared by `Renderer::next_sample` and `render_offline`. Future filters and delays must use that runtime; do not add stateful processing to random frame lookup alone.
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

MIDI tracks support polyphony under D29; Beat lanes retain one active voice each. Keep voice state separate from shared effects. Beat lane duration and retrigger rules follow D18. General MIDI notes have independent note-offs and oscillator state. The instrument owns voice allocation and its internal source graph under D30. Envelope release details and the placement of per-voice versus shared processing remain implementation details. Future configurable voice limits and stealing will be separate decisions. The design must not assume that every unit needs one instance per voice.

### Use one runtime for playback and export

Process stateful units in sequence with the same event timing in both modes. Stop and seek reset voices and effects; pause preserves state. End active notes at loop boundaries while effects continue. Mute suppresses output but keeps processing active. Seed handling and exact state initialization remain implementation details for repeatable exports.

Rebuild affected units when the processing rate changes, using the existing preparation and atomic-publication pattern. Smooth bypass transitions and publish prepared graph replacements atomically. Initial units must add no processing latency. Musical delay/reverb are allowed, but automatic latency compensation is deferred. Export reported tails up to 10 seconds and report truncation.

## Library findings and recommendations

Library findings were reviewed on 2026-10-11. FunDSP is the accepted MVP backend behind the shared unit API. Its oscillator integration is implemented behind `daw-unit-core`; other unit families remain pending. The other libraries remain candidates for later or specific needs; their inclusion is not approved by this scope.

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
2. Validate the architecture with a polyphonic MIDI instrument containing reusable oscillator units, and an audio-track effect graph. Use one source container per track and keep the initial unit set small.
3. Integrate block processing, events, transport, and atomic graph publication into the shared engine.
4. Add the Units view and project persistence over that same model. Add Editor with editor selection by clip type. Implement the accepted beat pattern editor MVP scope over shared event scheduling and unit processing, after specifying its remaining implementation details.
5. Package reusable Rust crates and add standalone tools after the unit contract works in both playback and export.

Before expanding the unit set, check source rejection on audio tracks, invalid-graph rollback, parameter smoothing, event timing, every supported project rate/buffer size, variable block sizes, save/reopen, and playback/export consistency. Measure callback allocation and blocking behavior, CPU/memory use, and native-device stability. Verify independent MIDI voices and note-offs, Beat lane retrigger replacement, transport resets, preserved pause state, loop voice termination with continuing effects, mute processing, smooth bypass, and atomic graph replacement. Test the absence of added processing latency, tail rendering and truncation notices, missing-unit fallback/data preservation, and pattern-versus-track ownership. Verify that multiple oscillator units form one instrument with independent per-voice oscillator state, one aggregate output bus, and preserved routing/settings after save/reopen. Measure the accepted eight-voice workload and record its hardware and results.

Verify that audio, MIDI, and beat clips open their respective editors in Editor, including beat clips with mixed audio/MIDI lanes. Check clip/lane context retention across Timeline, Editor, and Units. Beat-specific validation criteria are recorded in the [beat pattern editor design](beat-editor.md#validation-criteria).

## Decision record maintenance

Keep D01–D28 as confirmed requirements and accepted choices. Add accepted design choices with a decision date and rationale. Until accepted, retain proposals and open questions with their current status. Record implementation and validation separately; a library's advertised behavior does not establish integration performance.

Related documents: [beat pattern editor design](beat-editor.md), [repository guidelines](../AGENTS.md), [scope and roadmap](MVP.md), [current implementation specification](../specs/mvp-iteration-1.md), and [validation history](validation.md).
