# Modular audio units design and decisions

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/views/instrument.md).

Status: design baseline with implemented track/Master effects and MIDI sources; graph and beat work pending. Last updated: 2026-10-17.

Users build MIDI instruments in Instrument and configure track and Master effect chains in Timeline, with MIDI track effects also available in Instrument. These replace the earlier combined Units view. The planned track types are audio, MIDI, and beat. Audio tracks will accept effects; MIDI tracks will accept sources and effects. A future Editor view will provide an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. The beat pattern editor will provide step toggles in audio or MIDI lanes, each with its own sources and effects. Every unit will use a shared core and support independent distribution. These are confirmed requirements. The Units MVP scope below is accepted. Detailed API, layout, and packaging choices remain proposals or implementation details.

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
| D03 | Audio tracks accept effects only. MIDI tracks accept sources and effects. | Track capabilities must constrain unit placement. Basic MIDI tracks and oscillator instruments are implemented; serial track EQ chains are implemented; Master effects are implemented; beat-lane effects remain pending. |
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
| D25 | Earlier units add no latency. The Limiter adds fixed host-compensated latency (2026-10-14); export reported tails up to 10 seconds (Q07). | Align parallel tracks and Master; warn if tails are truncated. |
| D26 | Version unit state and preserve missing-unit data; separate pattern content from track/lane processing (Q08). | Missing instruments render silence; missing effects pass audio through. Defer nested presets. |
| D27 | Use FunDSP behind the shared unit API for the initial unit set and performance target (Q10). | Validate integration before expanding the set; keep backend types out of public and saved formats. |
| D28 | Use one Editor view with an audio editor for audio clips, a MIDI pattern editor for MIDI clips, and a beat pattern editor for beat clips. | Choose the editor from the clip type. Replace the earlier standalone Beat view. |

D29 (2026-10-11) updates the MIDI voice scope: MIDI patterns support simultaneous pitches in playback and export. Drawing replaces notes only on the same pitch, with trim, split, and removal behavior. Prepare voice slots for the actual note overlap and release tails before publication; no fixed voice cap or callback allocation. Beat lanes retain their accepted one-voice MVP scope.

D30 (2026-10-12) defines one source container per track: clip playback for Audio, one instrument for MIDI, and one beat instrument containing lanes for Beat. Reusable oscillator units belong inside an instrument. Each container exposes one audio output bus to track effects and gain/pan. This is the accepted target architecture; the MIDI source container now holds parallel layers with summed oscillator lists and optional amplitude envelopes under D32. Full graph containers remain pending.

These decisions define the full target scope. Basic MIDI tracks, patterns, oscillator units, and the MIDI Editor view are implemented; beat tracks, graph tools, other clip editors, and plugin loading remain pending. The delivery schedule remains on the [MVP roadmap](mvp-scope.md#future-features).

D31 (2026-10-12) adds a per-note ADSR Envelope unit immediately to the right of Oscillator. Use equal-height shared unit panels, a single full-height divider in the panel-border color, and four 90-point stage strips with matching padding, separators, three control rows, and bottom labels. Attack/Decay/Release expose time, curve shape, and curve amount; Sustain exposes level only. Curve choices are Linear, Exponential, Logarithmic, and S-curve, with retained amount disabled for Linear. Use one shared instrument configuration and independent envelope state per note. See [Envelope unit](../instruments/midi-patterns.md#envelope-unit) for implemented ranges, defaults, migration, and lifecycle.

D32 (2026-10-12) adds parallel instrument layers. New MIDI tracks start with a Tracks-style Instrument toolbar and one empty Layer 1 row. Add missing sources and either envelope through the component catalog. The toolbar has a left-aligned title and the standard Plus button on the right to add a layer; it stays above the scrolling layer area. The Instrument header and layer labels share the 250-point Tracks column width. The column uses the panel background through its unused space; the header area to its right and unused unit space use the workspace background. Drag the shared grip, name, or unused layer-header space to reorder layers and double-click a regular-size name to rename it. These drag areas share one handler; mixing controls and the active name editor retain their own gestures. Save stable layer IDs, names, and row order; naming and reordering preserve live voice state. Missing components have no placeholder cards, text, buttons, or reserved widths. Installed components sit directly beside the layer column and one another. Populated Oscillator panels retain their Plus strip. Installed units and layer rows retain shared 1-point dividers. Each row owns its oscillator sources, independent generic Envelope instances, and one optional Amplitude stage with destination-owned modulation connections. All expanded rows use the same full unit-panel height, including empty rows; labels and the full-width component drop area span that height. All layers receive the same notes, keep independent DSP state, and sum into one track output. Empty layers are silent; sources without an installed ADSR retain a short note gate. Existing instruments migrate into Layer 1 without changing their sound. Layer controls reuse the existing unit panels and knobs. Samplers, layer effect sections, and user-defined graph routing remain pending. Later decisions below define the implemented destinations and LFO component.

D33 (2026-10-12) splits the former Units view into Instrument and Effects. Instrument handles MIDI instrument sources and layer envelopes only; its 250-point column header is Layers. Its track selector lists only MIDI tracks. Effects handles track effects on audio and MIDI tracks and currently provides the separate view and track selector with an explicit unavailable-processing message. No effect units or DSP are added by this change. Both selectors reuse `track_selector.rs` and preserve clip selection and project state. View buttons and menus share the order Timeline, Instrument, Effects, Editor, Mixer. The earlier combined Units placement and beat-lane UI plan are superseded; beat-lane source/modulation controls still need a dedicated design. The accepted unit and routing requirements below remain valid, while references to the historical Units view follow this split.

D34 (2026-10-13) prepares the Effects workspace before effect implementation. Remove D33's unavailable-processing bar and message. Put a 250-point effects catalog at the left of the upper workspace, above the effects lane, using the same `ControlPanel` shell as Tracks and Layers. At the top right, reuse Editor's 38-point ruler and selected-track strip at the shared Timeline row height through `track_overview.rs`, including clip drawing, playhead, loop gestures, zoom, horizontal scrolling, and clip interactions. Pin an empty effects lane across the full window width at the bottom, below the catalog and timeline. Draw its top panel-border line and let the status panel own the single thin bottom separator, without a gap or second stroke. Derive its 244-point height from the shared expanded unit geometry; place the timeline scrollbar immediately above the strip and extend the shared timeline grid and playhead from the track strip down to the scrollbar. Clamp its height to the available space in short windows. Effects follows the selected audio/MIDI processing track; track changes preserve clip selection. Editor and Effects share their overview viewport, independently of Timeline. With no selected track, retain the catalog/ruler/empty strip and show a selection prompt in the track strip. The catalog and effects strip remain empty until units are implemented. View changes alone do not modify the project.

The first planned effect is a 3-band EQ. Its three strips are Low, Mid, and High; its three control rows are gain, frequency, and Q. Each strip ends with an on/off control and its label. Compact mode retains those footer controls and labels. This layout is accepted; EQ parameters, project data, and processing are not implemented in D34.

D35 (2026-10-13) adds effect unit selection and header drag reordering to the Effects lane. Single-click an installed effect unit to select it. Shift-click toggles its selection. A primary click in the empty effects lane clears all unit selections. Selected units reuse the selected Tracks border. Selection is UI-only, independent of track/clip and catalog selection, and does not mark the project modified. It survives view changes and reordering; removed IDs are pruned and project replacement clears it. Drag an effect header to move the selected units within its track chain. Dragging an already selected instance retains the selection; dragging an unselected instance selects it. Show an insertion line and scroll at the lane edges. Commit the order only on release inside the lane; Escape, focus loss, a blocked editing environment, or an outside drop cancels. Preserve stable IDs, settings, enable/collapse states, and compatible DSP history. Same-position drops leave the project clean. Power and knob gestures retain their behavior, and header double-click still collapses the unit. Move selected units as a group in their existing chain order, even when selection clicks occurred in another order or units were separated. Leave unselected units in their existing order. Selection applies to effect units only; Instrument Oscillator/Envelope units retain their existing behavior.

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
| Q07 | Built-in Limiter latency is reported, aligned across tracks/Master, and primed for playback/export. Other current units add no latency. Musical delay/reverb remain allowed; reported tails are capped at 10 seconds with a notice on truncation. | Arbitrary third-party/plugin latency. |
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

The clip-editing view was renamed from Edit to Editor on 2026-10-12. The application exposes Timeline, Units, Editor, and Mixer. Units currently configures layered oscillator instruments; Editor currently provides the MIDI piano roll. The remaining source/effect graph tools, audio editor, and beat editor are pending.

### Clip editors

| Clip type | Editor in the Editor view |
| --- | --- |
| Audio | Audio editor. |
| MIDI | MIDI pattern editor. |
| Beat | Beat pattern editor. |

Select the editor from the clip being edited, rather than its track or a beat lane's instrument type. A beat clip with audio and MIDI lanes still opens the beat pattern editor. Retain the active clip context when switching between Timeline, Editor, and Units; beat clips also retain their selected lane. Unit configuration stays in Units, and Mixer controls tracks.

The first MIDI editor and opening gesture are defined in [MIDI patterns and oscillators](../instruments/midi-patterns.md). Audio-editor tools and further multi-clip editor behavior remain to be specified. The [beat pattern editor design](../views/beat-editor.md) retains the accepted step-editing scope. All editors must preserve source recordings and the pattern-source isolation rules below.

## Track capabilities

| Track type | Confirmed unit placement |
| --- | --- |
| Audio | Effects only. |
| MIDI | Sources and effects. |
| Beat | Each audio or MIDI lane has its own sources, effects, and controls. The parent track has its own effects. Units configures both levels. |

The [Beat pattern editor design and decisions](../views/beat-editor.md) defines the accepted Beat MVP scope and remaining implementation details. The beat pattern editor in Editor handles beat creation. Units configures lane sources, effects, modulators, independent controls, and parent-track effects. Mixer controls each beat track as a whole. Beat lanes should reuse the shared unit core and processing runtime described here.

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

The current MIDI implementation stores one version 2 instrument per track with stable-ID layers. Each layer holds its oscillator list, optional amplitude envelope presence, and retained envelope settings. Each note prepares independent source and envelope state for every layer; layer outputs sum into the track bus. The Instrument layout is implemented in `instrument_panel.rs`, with the existing Oscillator controls and reusable `envelope_panel.rs` body. Full graph container APIs, graph serialization, and the UI for internal routing still need implementation design. Nested graph editing remains deferred under Q01.

### Clip sources and pattern editing

Audio files, MIDI patterns, and beat patterns share a clip-source model. Clip placement, trimming, and looping operate independently of source content. Editing a pattern creates a new source for the edited clip, while other clips retain their original source. This requirement was confirmed on 2026-10-11 in D11–D12; the [Beat design](../views/beat-editor.md#pattern-sources-and-clip-editing) records the clip-sharing example and validation criteria. Pattern sources hold immutable note/step content. Tracks and lanes own their instrument/effect setups. Changing a track instrument or its effects intentionally affects clips using that track; it is separate from editing a clip's pattern content.

## First MIDI/oscillator implementation

Implemented on 2026-10-11. See [MIDI patterns and oscillators](../instruments/midi-patterns.md) for the model, controls, timing, and limits. `daw-unit-core` owns the portable source contract and versioned settings; `daw-oscillator` hides FunDSP. MIDI patterns are embedded immutable assets, and tracks own their layered instrument settings. Since 2026-10-12, the Oscillator panel contains multiple source columns with independent waveform, pitch, and gain. Each source strip has an 18-point circular Lucide Power toggle followed by an 11-point “Osc N” label with a 8-point gap. Center the pair below its knobs. Use primary text for active labels and secondary text for muted labels. Retain the source number in its tooltip and accessibility name. On uses the view controls’ accent-tinted fill with a 1-point border and icon in the current accent color. Off uses the secondary text color, matching the dim label, for both icon and border. Source mute retains its gain/settings, saves with the instrument, and defaults to false when absent. Muted sources keep processing with zero output. Oscillator strips have a fixed 90-point width, including padding, independent of source count. Each strip has its own container with equal internal padding and a full-height separator before the next strip, including Add. The add strip on the right contains an 18-point circular Plus button centered on both axes, using Power’s shared circle, border, inactive colors, icon scale, and hover feedback. Both visible circles have the same 18-point outer diameter, including their border. The whole add strip is one button with a background highlight on hover/focus; clicking anywhere in it, including padding, adds one default source on release; each column and knob exposes Remove oscillator in its context menu. Empty instruments render silence. Saved single-oscillator instruments migrate to a one-item list inside Layer 1. Playback and export use sequential per-track polyphonic voices with independent note-offs and prepared voice slots.

### Layer mixing controls

Expanded layers keep the shared 244-point unit-panel height, including empty layers. A standard icon button to the right of M/S, with the same width as Add layer, uses Lucide Arrow Up From Line to collapse the layer and Arrow Down From Line to expand it. Collapsed layers use the shared Tracks row height (68 points): keep the layer header’s two control rows, and show only installed unit headers and their footer rows. Hide all unit knob rows; retain source Power/“Osc N” pairs, envelope stage labels, and Oscillator Plus. Preserve unit widths, enable state, parameters, and each unit’s independent horizontal collapse flag. When both the layer and unit are collapsed, use the first three letters of the unit name followed by three periods (“Osc...”, “Env...”, or “Amp...”) as the vertical strip title; otherwise use the full title. Empty layers retain this height and their full-width catalog drop area. Missing components have no placeholder cards, buttons, or reserved width. Adding units or scrolling horizontally does not change the chosen height. Save the layer’s `collapsed` flag in the project; missing fields default to expanded, including legacy instruments. Toggle on release through the shared instrument edit and mark the project modified. Retain this state through save/reopen, Save As, reorder, and view/track switches without changing playback or export. Disable the button during layer rename/reorder, disabled editing, or focus loss; exclude its press origin from background reorder gestures.

Each layer header follows the Tracks layout: grip and editable name beside M/S toggles, with compact gain and pan knobs and numeric fields below. Reserve the full name column even for short names, and align M/S and collapse as one group at the right. Use the same 6-point gap between all three buttons and the same 7-point outer inset as the layer header. Timeline tracks and layers share `mix_controls.rs`. Gain defaults to 0 dB and pan to center (0); mute and solo default off. Gain knobs use the existing −∞ to +12 dB scale; numeric gain accepts finite values with finite linear amplitude or −∞. Pan is −1 (left) through +1 (right). Numeric fields commit on Enter or focus loss, cancel on Escape, and reject invalid input without changing the saved value. Disabled/unfocused inputs cancel their drafts. Toggles apply on release; knobs use the shared immediate adjustment/reset behavior. Layer controls commit through the shared instrument edit and retain row height, reordering, naming, and unit collapse.

Layer solo is scoped to its instrument. The same core `mix_audible` rule serves tracks and layers: if any peer is soloed, only soloed peers sound, even if muted; otherwise muted peers are silent. Layer solo does not solo its track or other tracks. Sources feed the layer envelope, then layer gain/pan/audibility; stereo layers sum before the existing track gain/pan and master. Layer pan uses the existing equal-power law normalized to unity per channel at center. This preserves legacy output, including existing track pan positions. Mute and solo exclusion keep sources/envelopes running with zero output. Live layer mix edits preserve held notes and phases and ramp active stereo coefficients over 5 ms without restarting unrelated in-flight ramps. Idle slots take new controls immediately, so later notes start with the correct mute/gain/pan. Playback and export share this stereo runtime; preparation and control replacement remain allocation-free in the callback.

Save `muted`, `soloed`, `gain_db`, and `pan` in version-2 layer settings. Old version-2 layers and version-1 instruments load with the unity/center/off defaults. Validate finite gain and linear amplitude and bounded finite pan; invalid edits remain atomic. Names, IDs, source settings, envelope settings, and mix controls all survive save/reopen, Save As, and row reorder.

### Shared unit panel

All source, effect, and modulator units in Instrument and Effects must use the same base panel, provided by `PanelStyle::unit` in [panels.rs](../../crates/ui/src/panels.rs). Do not build separate panel shells for individual units. Each unit supplies its name, enable state, and controls to this shared component.

- The header spans the panel’s inner width and displays the unit name, without repeating the track name.
- Use the active view-mode button background for an enabled header and the mixer strip’s contrasting foreground rule for centered text. Resolve foreground contrast against the final tinted surface in both themes. Update both when the accent or theme changes.
- Share the mixer strip’s 24-point header height and 11-point text size through the common panel constants.
- Double-click a unit header to collapse it into a full-height vertical strip with a regular panel body and a power-only header cap. Its inner width equals the shared 24-point header height (26 points including the border). Center the same 11-point title in the neutral body below the cap, rotated bottom-to-top, with primary text when on and secondary text when off. Double-click the strip to expand. A single click does not toggle it; disabled controls cannot toggle it. Keep context menus on the header in either state.
- Every installed unit has a 10-point circular on/off control at the left of its header, with no visible label or icon. On headers use the active view-mode button background through shared theme helpers; off headers use the panel’s main border color with muted secondary text in expanded and collapsed states. A collapsed unit keeps the regular panel background for its body and shows only the power control in its 24-point header cap; center the vertical title below that cap with regular panel text when on and muted secondary text when off. The circle uses the full accent color when on; off is hollow with the muted secondary text color. Its hit area is 22 points. In a collapsed strip it remains at the top. Power clicks toggle on release and never collapse or expand the unit, including double-clicks; header double-clicks outside the power control retain collapse behavior. Disabled or unfocused controls do not toggle. The shared `PanelStyle::unit` owns this control and exposes its response for tooltips/accessibility and interaction checks. Unit enable state and collapsed state are saved separately in the project. Older projects default units to enabled and expanded. Oscillator off silences its layer sources without changing their individual settings. Envelope off fades its modulation contribution to neutral; Amplitude off bypasses its gain and modulation through the existing 5 ms note gate. Live source toggles ramp output over 5 ms. Envelope contribution and Amplitude bypass changes fade over 5 ms, retaining held notes, oscillator phase, and envelope progress without callback allocation. Off units remain editable and retain settings when removed/re-added. Playback and export share these states and processing rules.
- Collapse belongs to saved instrument settings per layer and unit: `oscillator_collapsed` and `envelope_collapsed`, defaulting to false in older version-2 projects and migrated version-1 instruments. The shared shell receives these mutable flags; header double-clicks commit through `Edit::SetInstrument` and mark the project modified. Restore presentation on save/reopen and Save As, and retain it through row reorder, view/track switches, and removal/re-addition. Replacement projects supply their own values; do not cache collapse in temporary UI state. Hidden controls retain all settings and keep processing audio. Keep the layer height fixed and skip the hidden body. Implement collapse once in the shared panel shell for current and future units.
- Use the current theme’s neutral panel background for controls, with the shared thin border and 10-point body padding. Keep the header flush with the panel edges inside that border. Share one 1-point divider between the playback toolbar and Instrument or Effects, without an extra gap or stacked top borders.
- Reuse the standard knobs, faders, numeric fields, and dropdowns. Individual units define their parameters inside the body; they retain the same panel shell in both themes.

The Oscillator panel uses this base panel. Its body delegates the shared padding to individual source and add containers, with no additional outer padding or gaps. All strips have equal height; each source centers its knobs and Power button in one vertical column. Future units must reuse the base panel.

### Shared knob base

All shape selectors follow the [shape-selection control standard](#shape-selection-control-standard--2026-10-13): show diagrams of their actual choices and highlight the active one. Two-option mode selectors use bounded knobs with both values at the upper corners.

All knob types use the `Knob` widget and `KnobSpec` in [knobs.rs](../../crates/ui/src/knobs.rs). The base owns painting, bounds or cyclic wrapping, choice diagrams, focus and disabled-input guards, fractional drag accumulation, Shift sensitivity, arrow-key steps, double-click reset, tooltips, and accessibility. Parameter wrappers declare their ranges, scales, steps, reset values, and formatting. Gain, pan, pitch, and waveform reuse this path; future types must also reuse it. Keep the standard 28-point knob footprint, face, and pointer. Bounded controls retain their value arc and reference marks. Cyclic choices use a full circle with equal angular spacing, and optional diagrams replace the marks. The waveform selector reserves a 52-point footprint for four 10 × 6-point vector diagrams around the same knob face; its active diagram uses the accent color. Stepped knobs retain fractional movement between frames so slow drags can cross a step, and Shift can change during a drag without a jump.

The Oscillator stacks compact rows in this order: Waveform, Pitch, Gain. Each shared row contains an aligned standard knob without a parameter-name label. Retain parameter names and current values in tooltips and accessibility information. Remove the separate waveform name and pitch/gain value boxes. This follows the confirmed vertical layout choice. Waveform cycles through four steps: Sine, Saw, Square, Triangle, then back to Sine. Dragging has no end stop in either direction; arrow keys also wrap. All three Oscillator rows reserve 52 points of height for diagrams or value/endpoint labels; all knob centers remain aligned. Pitch uses one-semitone steps, −48 to +48. Show reference marks at every multiple of 12 semitones across that range, with a longer, brighter zero mark and a small current pitch value above it. Label the pitch endpoints “−48” and “+48” below their marks. Gain knobs support optional reference ticks at −∞, −40, −30, −20, −15, −10, −6, −3, 0, +3, +6, +9, and +12 dB. Enable them in Oscillator, with a stronger unity mark, the current gain in dB above it (−∞ for silence), and −∞/+12 endpoint labels. The base knob supports a reference interval or explicit reference values, optional current-value formatter, and optional endpoint labels independent of the input step, so the hints do not change adjustment. Current-value and endpoint labels use 9-point numeric theme text. Current values stay above the zero/unity position and update through the base knob; endpoint labels sit below their fixed marks. Endpoint labels stay fixed as the current value changes and fit within the existing 52-point footprint. Compact Timeline gain knobs retain their layout without optional scale ticks or labels. All gain knobs reuse the fader’s silence-to-+12 dB scale, adapted to place unity at twelve o’clock. The amplitude tail below −60 dB reaches exact silence; keyboard steps can enter and leave that endpoint. Gain and Pitch remain adjustable through their knobs and tooltips. During playback, changing gain, pitch, or mute retains oscillator phase and note/envelope state, with 5 ms amplitude or frequency ramps. Retain in-flight ramps when an unrelated control changes. The persisted `transpose` field remains compatible with older projects. Double-click resets Waveform to Sine, Pitch to zero, and Gain to 0 dB. Timeline and Mixer gain/pan knobs use the same base; their surrounding layouts stay specific to those views.

### Shared gain fader

All gain faders use [faders.rs](../../crates/ui/src/faders.rs). The shared scale runs from exact silence (−∞) to +12 dB, with extra travel near unity. Label +12, +9, +6, +3, 0, −3, −6, −10, −15, −20, −30, −40, and −∞. The 0 dB tick is longer and brighter. Subdivisions use 1 dB around unity, 2 dB between −20 and −40, 5 dB below −40, and 1 dB above unity; omit ticks when spacing is below 3 points. At reduced fader heights, reduce the labels before they collide. Reserve the scale width inside the track or Master strip. The handle and all ticks use the same piecewise mapping; the tail below −60 dB follows amplitude continuously down to zero.

Shift reduces pointer travel tenfold and can change during a gesture without jumping. Idle frames and release preserve the gain; double-click resets it to 0 dB. The silence position changes gain without toggling mute/solo. Gain fields in Timeline and Mixer share formatting and parsing, displaying −∞ and accepting −∞, -inf, or -infinity. Master input clamps to the fader limits; track entry retains finite values outside the slider range on display. Gain knobs cover the same silence-to-+12 dB range and do not alter a silent value until edited.

Project data uses the finite `daw_core::SILENT_GAIN_DB` value (−1000 dB), whose conversion through the existing `linear_gain` underflows to exact zero. This retains the existing manifest schema and avoids non-finite JSON numbers. Live playback, saved/reopened projects, and offline rendering use the same gain. Oscillator source gain also accepts exact silence through +12 dB. The silence constant belongs to portable `daw_unit_core` and is re-exported by `daw_core`; saved oscillator settings remain finite and keep their state version.

This is the first working path. The ADSR Envelope is implemented under D31. Serial effects, graph connections, other source/modulator units, standalone interfaces, and the full Units performance workload remain pending.

### Shared stereo meter

Track and Master meters in Timeline and Mixer use [meters.rs](../../crates/ui/src/meters.rs). Mixer shows one scale for both channels. Timeline shows only the bars, without scale labels, ticks, or reserved scale width. Both use a linear −60 to 0 dBFS display, with Mixer marking 0, −3, −6, −9, −12, −15, −18, −21, −24, −30, −35, −40, −45, −50, and −60. Emphasize zero. Add 1 dB subdivisions above −30 and 5 dB below it when spacing is at least 3 points. Fit labels inside the panel and omit interior labels that collide at short heights. Reserve a 24-point scale column only in Mixer; its strips are 120 points wide.

In Mixer track and Master strips, center the gain field above the complete fader column and a matching read-only peak field above the complete stereo meter column. Both fields share a row and the standard centered 36-point numeric style; include the scale labels and tick marks in column centering. The peak field displays the highest L/R peak since its last reset in dBFS with one decimal place, −∞ for silence, and a plus sign above zero. It holds that maximum as the live bars fall, including pause, Stop, and view changes. Clicking the field resets it to −∞; subsequent output can raise it again. The output callback captures peaks with atomic maximum updates, so brief peaks between UI frames remain available. Track holds follow track IDs through reorder and live plan updates. Holds reset with a new output monitor or project and are not saved in project data. Reset does not change gain, current meter bars, or clipping state.

Meter fill uses the same dBFS mapping as the scale. Silence and peaks at or below −60 dBFS show no fill; peaks above zero fill the rail and retain the existing clipping indication. Keep actual dBFS values in tooltips, independent channel clipping, and click-to-clear. The scale changes display only; source generation, gain, rendering, and project data remain unchanged.

## Existing architecture to preserve

- [core](../../crates/core/src/lib.rs) owns project entities, stable IDs, validation, and transactional edits. Keep device and UI dependencies out of this crate.
- [engine](../../crates/engine/src/lib.rs) supplies shared mixing and transport. `RenderPlan::sample_at` and `render` read prepared audio at arbitrary frames. MIDI oscillators advance through the sequential runtime shared by `Renderer::next_sample` and `render_offline`. Future filters and delays must use that runtime; do not add stateful processing to random frame lookup alone.
- [project](../../crates/project/src/lib.rs) owns sessions, persistence, asset paths, prepared audio, and export orchestration.
- [output](../../crates/output/src/lib.rs) owns CPAL output, device conversion, bounded queues, and renderer retirement. Preserve allocation-free, nonblocking callbacks and retire final audio owners outside them.
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

Save unit identities, state, parameters, and connections in the project. Keep unit presentation and view state separate from audio behavior. Build and validate a candidate runtime outside the callback, then publish the complete change at a processing boundary. Invalid connections or failed preparation retain the previous graph.

Enforce track restrictions in shared validation and host integration, as well as the UI. A disabled menu item alone is insufficient. Composite units must expose enough capability information to validate their use on a track.

Implement serial effect chains and a small node graph for instrument construction and modulation. Reject graph cycles with a useful error. Keep feedback inside the delay unit; graph-level feedback and nested graph editing are deferred.

### Separate voice processing from track processing

The accepted MIDI path is note events → instrument voice output → serial track effects → track gain/pan → track mix. Instantiate voice state for oscillators and envelopes; share track effects where appropriate. Keep voice allocation and stealing out of individual oscillator implementations.

MIDI tracks support polyphony under D29; Beat lanes retain one active voice each. Keep voice state separate from shared effects. Beat lane duration and retrigger rules follow D18. General MIDI notes have independent note-offs and oscillator state. The instrument owns voice allocation and its internal source graph under D30. D31 implements independent per-note ADSR state over the summed oscillator outputs, before track processing; track effects remain shared. Release capacity is prepared outside processing, and live parameter edits retain stage progress. Further envelope routing remains pending. Future configurable voice limits and stealing will be separate decisions. The design must not assume that every unit needs one instance per voice.

### Use one runtime for playback and export

Process stateful units in sequence with the same event timing in both modes. Stop and seek reset voices and effects; pause preserves state. End active notes at loop boundaries while effects continue. Mute suppresses output but keeps processing active. Seed handling and exact state initialization remain implementation details for repeatable exports.

Rebuild affected units when the processing rate changes, using the existing preparation and atomic-publication pattern. Smooth bypass transitions and publish prepared graph replacements atomically. Earlier units add no latency. Limiter reports a fixed delay and uses shared track/Master alignment and preroll; arbitrary plugin latency remains deferred. Musical delay/reverb are allowed. Export reported tails up to 10 seconds and report truncation.

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
| Q07 | Resolved for MVP. | Fixed Limiter latency with host compensation; earlier units add no latency; capped export tails: D25. |
| Q08 | Resolved for MVP. | Versioned state, ownership, and missing-unit behavior: D26. |
| Q09 | Resolved. | Rust crates and standalone tools: D10. |
| Q10 | Resolved for MVP. | FunDSP, initial unit set, and performance target: D27. |

Remaining implementation details are:

- Public API and crate names; Units layout; exact modulation rates, channel conversion, and simultaneous-event ordering.
- Note identity and stale note-off handling, additional unit defaults, smoothing and bypass transitions, and further voice/shared state allocation; ADSR settings and lifecycle are implemented under D31.
- Saved schema and version migrations, handling unsupported versions, pattern-to-lane identity mapping, and preset format.
- Tail reporting for each effect and repeatable random-seed initialization.
- FunDSP version/features and dependency/license checks; exact benchmark graph, named test hardware, memory measurements, and native-device validation.
- Standalone interface, device support, and packaging.

These details must preserve the accepted behavior. Specify future polyphony, nested editing/presets, advanced routing, and arbitrary plugin latency when those features enter scope.

## Proposed implementation sequence

1. Define the shared API and persisted graph model within the accepted scope. Specify the remaining implementation details needed for the first units.
2. Validate the architecture with a polyphonic MIDI instrument containing reusable oscillator units, and an audio-track effect graph. Use one source container per track and keep the initial unit set small.
3. Integrate block processing, events, transport, and atomic graph publication into the shared engine.
4. Add the Units view and project persistence over that same model. Add Editor with editor selection by clip type. Implement the accepted beat pattern editor MVP scope over shared event scheduling and unit processing, after specifying its remaining implementation details.
5. Package reusable Rust crates and add standalone tools after the unit contract works in both playback and export.

Before expanding the unit set, check source rejection on audio tracks, invalid-graph rollback, parameter smoothing, event timing, every supported project rate/buffer size, variable block sizes, save/reopen, and playback/export consistency. Measure callback allocation and blocking behavior, CPU/memory use, and native-device stability. Verify independent MIDI voices and note-offs, Beat lane retrigger replacement, transport resets, preserved pause state, loop voice termination with continuing effects, mute processing, smooth bypass, and atomic graph replacement. Test zero latency for earlier units and compensated Limiter timing, tail rendering and truncation notices, missing-unit fallback/data preservation, and pattern-versus-track ownership. Verify that multiple oscillator units form one instrument with independent per-voice oscillator state, one aggregate output bus, and preserved routing/settings after save/reopen. Measure the accepted eight-voice workload and record its hardware and results.

Verify that audio, MIDI, and beat clips open their respective editors in Editor, including beat clips with mixed audio/MIDI lanes. Check clip/lane context retention across Timeline, Editor, and Units. Beat-specific validation criteria are recorded in the [beat pattern editor design](../views/beat-editor.md#validation-criteria).

## Decision record maintenance

Keep D01–D28 as confirmed requirements and accepted choices. Add accepted design choices with a decision date and rationale. Until accepted, retain proposals and open questions with their current status. Record implementation and validation separately; a library's advertised behavior does not establish integration performance.

Related documents: [beat pattern editor design](../views/beat-editor.md), [repository guidelines](../../AGENTS.md), [scope and roadmap](mvp-scope.md), [current implementation specification](iteration-1.md), and [validation history](../validation/README.md).

## First track effect — D35, 2026-10-13

The [3-band EQ](../effects/equalizer.md) implements the first serial track effect on audio and MIDI tracks. The catalog name adds one instance on double-click. Low/High use shelves and Mid uses a middle shelf with Gain and on/off only. Low off applies a high-pass cut; High off applies a low-pass cut. Mid off cuts between the Low/High frequency values, with Low/High Q controlling the corresponding slopes. Mid has no independent Frequency/Q controls. This supersedes the earlier bell and Mid-Q design. All bands off produces exact silence after the transition. Whole-unit off bypasses the EQ. The user sets the defaults to Low 250 Hz and High 2.5 kHz. Reuse shared unit controls, saved presentation states, and oscillator footer pairs. The status panel supplies the only thin bottom separator. The linked specification defines ranges, state ownership, processing order, smoothing, and current limits. D34 remains the historical workspace preparation; its empty catalog/lane are now populated by this effect.

## Drive effect — D36, 2026-10-13

[Drive](../effects/drive.md) adds two strips labeled Drive and Color. Drive has Volume, Gain, Tone; Color has Shape, Strength, Frequency. Shapes are Off, Mid scoop, Mid hump, and Mid hump + bass cut. Frequency moves the broad mid center and linked bass shelf; Strength sets the depth. The unit uses pre-clipping color, symmetric soft clipping, post-clipping Tone, and output Volume. Defaults are 0 dB Volume, 20% Gain, neutral Tone, Color Off, 50% saved Strength, and 750 Hz Color frequency (250 Hz–2 kHz range). Compact mode retains the two labels. Reuse the catalog, unit shell, shared knob, saved presentation, bypass, engine order, 5 ms smoothing, and missing-unit handling. The linked specification describes implementation limits and validation. This expands D35's serial track effect set without adding graph or lane/Master routing.

D36 (2026-10-13) adds the reusable [Auto-filter](../effects/auto-filter.md) to the serial track chain. Use one LFO strip with two columns, equal inner/outer horizontal gaps, no internal divider, and one centered Power/LFO footer. Use signed −180° to +180° sync/stereo phase. The portable processor accepts transport frame/tempo, uses FunDSP SVF coefficients with independent stereo state, and shares versioned settings, callback-safe transitions, presentation, selection, and ordering with other effects.

## Shape-selection control standard — 2026-10-13

Every shape-selection knob uses the shared `Knob` and `KnobSpec::diagrams` design. Arrange diagrams of all available choices around the knob. Highlight the active diagram with the accent color and use secondary text color for the others. Each diagram must represent its choice: waveform for a source or LFO, frequency response for a filter or Drive color, and curve for an envelope. Reuse the control design, not unrelated shape drawings. Keep choice names and current values in tooltips and accessibility; omit a separate current-shape text label. Retain the parameter's agreed wrapping or bounded behavior.

Two-option mode selectors are separate from shape selectors. Use the shared bounded two-way knob, with the first value at the upper left and the second at the upper right. Highlight the selected label with the accent color and dim the other choice with secondary text color. These are choice labels, not range endpoints. Draw a neutral arc between the choices, with no markers or accent value arc. Retain the knob face and pointer. Auto-filter Rate mode uses Hz and Note and does not wrap.


## Compressor — 2026-10-13

The accepted [Compressor](../effects/compressor.md) has three strips: Comp (Threshold, Ratio, Peak/RMS), Response (Attack, Release, Knee), and Out (Level, Dry/Wet, live gain-reduction meter). Peak/RMS uses the same bounded two-way knob as Auto-filter's Hz/Note, including upper-corner choice labels, a neutral arc, and no markers or wrapping. The independent `daw-compressor` crate uses a linked stereo detector and a shared soft-knee gain envelope, with private FunDSP RMS state. It adds no processing latency. Level follows Dry/Wet; header power bypasses the whole unit. Versioned state, existing effect-lane interactions, save/open, compatible history inheritance, callback-safe monitoring, and playback/export parity are implemented together. The detector high-pass remains a later proposal.

## OTT — 2026-10-13

The accepted [OTT](../effects/ott.md) adds three stereo-linked upward/downward compression bands. Each band is one two-column strip with rows Freq/Depth, Input/Output, and Attack/Release, plus a centered Power/name footer; Mid leaves Freq empty. Low/High default to 250 Hz/2.5 kHz. Knee is fixed and soft; Depth controls compression intensity independently of Dry/Wet. Out has Level, Dry/Wet, and the shared bounded Peak/RMS mode selector. Band off bypasses compression while preserving audio and Input/Output gains. Each band shows signed boost/reduction. Versioned state, existing chain interactions, shared playback/export processing, phase-matched Dry/Wet, compatible history, and callback-safe monitoring use the same unit architecture. The linked document specifies the implementation curves, ranges, defaults, and limits.

## Timeline effects workspace — 2026-10-14

Timeline owns the effects catalog and bottom lane; the dedicated Effects view is removed. Place the catalog to the left of Tracks; retain the Timeline ruler, grid, clips, scrolling, and pinned Master above the effects lane. The scrollbar stays directly above the lane, with one status separator below. Retain the track-owned chain, saved unit presentation, catalog/lane input handlers, selection/focus guards, and prepared audio updates. Track clicks switch the lane target immediately. This changes workspace composition only; the source-container and effect-processing contracts remain unchanged.

## Timeline toolbar visibility — 2026-10-14

Timeline adds a left toolbar spanning playback to status, beside both the catalog and effects lane. Its borderless Lucide Library toggle shows/hides the catalog without changing track effects or project state. The 38-point rail retains a centered 24-point button and 16-point icon. Use active-view fill on, transparent off, and one translucent gray hover overlay for both themes. The lane continues across the remaining width to the rail's right; hiding the catalog expands the upper timeline viewport. This updates the earlier full-window lane geometry by reserving the rail. A matching Gallery Horizontal button sits at the bottom with a seven-point inset and independently shows/hides the effects lane. Both toggles are 24×24 points with 16-point icons. Lane visibility starts on and remains UI state across view changes. Hiding the lane gives its height back to the timeline/catalog and clears unit selection and pending gestures; the hidden lane has no keyboard removal or drag/drop target. Catalog click/Enter addition remains available.


Tracks and Master meet the catalog, or the toolbar when the catalog is hidden, with one shared 1-point divider and no gap. The catalog/toolbar owns that divider; individual track/Master rows do not draw another left border. Selected-track outlines remain available.


The first installed Timeline effect shares the toolbar's left divider. The shared unit shell omits that first unit's duplicate left stroke through a scoped border-ownership tag; it keeps its top/right border, layout, controls, and selection outline. Other units retain their borders. Apply this to expanded, compact, and collapsed presentations without adding a gap.


## Timeline-only effects

On 2026-10-14, the dedicated Effects view was removed from the shared view enum, buttons, egui menu, and native menus. The remaining views are Timeline, Instrument, Editor, and Mixer. This supersedes D33/D34's separate Effects workspace placement. Timeline retains the catalog, lane, independent visibility toggles, selected audio/MIDI track chain, all unit controls and editing gestures, and saved presentation. Only Editor uses the selected-track overview and its separate zoom/scroll. View removal changes no project schema, effect IDs/settings, or audio processing.

## Master effects implementation — 2026-10-14

Master owns a saved `effects` list and `effects_compact` flag, both defaulted for legacy projects. It accepts the same EQ, Drive, Auto-filter, Compressor, and OTT units as tracks. The Timeline effects lane follows a typed track/Master target; Master selection in Timeline or Mixer preserves clips while clearing track and effect selections. Catalog selection/add/drop, unit presentation/editing, group movement/removal, focus guards, and visibility reuse the shared lane. Master is available even without tracks, and selecting it does not modify project content. New/open/close resets its UI selection.

`Project::all_effects` centralizes instance validation and meter discovery. `Edit::SetMasterEffects` validates and publishes a complete chain atomically, including global ID uniqueness. Runtime `EffectChain` serves both tracks and Master. Serial Master processing receives the stereo sum after track effects and gain/pan, before Master gain and output meters. Persistent rendering, direct render helpers, live playback, and export share this processing order. Prepared updates retain compatible histories and stable compressor/OTT meter handles without callback allocation; seek/stop reset, pause retains settled state, and loop boundaries retain effects. Missing Master units bypass, retain serialized settings, and produce a load warning. No new latency or export-tail policy is introduced.

## Shared Instrument effects workspace — 2026-10-14

Instrument now shares Timeline’s left toolbar, effects catalog, and bottom effects lane through the same workspace composition. This extends the earlier Timeline-only placement. Instrument’s upper content remains the Layers/source/envelope workspace; its catalog is left of Layers and its lane is below the instrument content. Visibility is shared UI state across Timeline and Instrument, with independent catalog/lane toggles. The rail retains its full playback-to-status height and existing size/style.

Instrument edits only the selected MIDI track’s existing serial chain. It reuses catalog selection/keyboard addition, drag previews/drop, unit editing, compact/collapse state, group reorder/removal, and focus/gesture guards. Its MIDI selector retains clip selection. Stale Timeline geometry/drop targets are cleared when Instrument renders. Pane changes do not alter instruments, effects, project data, or processing order; no DSP or saved-schema changes are introduced.

## Limiter and compensated latency — 2026-10-14

[Limiter](../effects/limiter.md) adds Input/Release/Lookahead and Ceiling/Mode/Routing strips, Peak/True Peak modes, linked L/R or independent M/S dynamics, and a linked output guard. Its fixed delay remains active during bypass; the shared engine aligns parallel track chains and serial Master processing, primes seeks, and preserves arranged/export frame positions. This supersedes the previous zero-latency-only requirement for built-in effects. Portable processing lives in `crates/limiter`; the host owns alignment and transport.

## Instrument component catalog — 2026-10-15

Instrument offers Oscillator, Envelope, LFO, and Amplitude in the existing left sidebar’s component catalog. Its Piano toggle sits below the Effects catalog toggle; opening either closes the other. Both reuse the shared catalog selection, keyboard, focus, and row painting in `catalog.rs` and the existing `ControlPanel` shell. The selected layer is an independent UI target with the standard selection outline. Double-click adds a component and Enter adds the selected group through one instrument edit. Oscillator appends a default source. Envelope adds an independent generic modulator. Amplitude restores its optional audio stage without duplicates or resetting retained settings. Add missing components through the catalog; populated Oscillator panels retain their Plus strip. See [the component catalog contract](../instruments/midi-patterns.md#instrument-component-catalog) for target identity, guards, and no-op behavior.

Component catalog rows also drag onto any instrument layer label or body. Selected rows carry the group in library order, while an unselected row carries only itself. The destination outline and vertical edge scrolling are UI-only previews. Release commits through the shared component helper; invalid drops and cancelled/invalidated gestures retain the project. Catalog press/selection capture and pointer/focus/scroll primitives are shared with Effects through `catalog.rs` and `drag.rs`.


## Generic Envelope, Amplitude, and modulation — 2026-10-15

Replace the role-specific envelope components with generic Envelope instances and a separate Amplitude audio stage. Each layer permits multiple independent envelopes and one optional Amplitude. Use the shared ADSR/unit controls, stable identities, retained removal/re-addition state, and Per note/Retrigger/Legato modes. Amplitude, Auto-filter, and Drive share destination-owned modulation strips and an icon-free header dot toggle that reveals modulation strips on the right while retaining the regular controls. Assigned parameter knobs reuse the shared status dot, source/amount tooltip, click-to-open action, possible-range arc, and live processed-value dots. Keep base pointers/readouts unchanged; include the internal LFO and stereo values on Auto-filter Frequency and the highest active voice gain on Amplitude. Use prepared callback telemetry without allocation or a simulated UI envelope clock. New connections are unassigned at zero amount; expansion visibility is saved per unit in the project as `modulation_open`, defaulting missing fields to closed. Preserve legacy instruments with the version-3 migration to explicit amplitude connections.

Implement typed, same-instrument connections with finite bounds, neutral disabled/missing-source behavior, shared last-note release, source fan-out, parameter smoothing, and allocation-free callback retargeting. Keep track effects shared; Auto-filter and Drive accept only shared Envelope and LFO sources from their MIDI track. Full graph editing, audio sidechains, cross-track sources, per-note-to-shared aggregation, and modulation of further units remain outside this step. See [Envelope components](../instruments/midi-patterns.md#envelope-components) for exact controls, defaults, timing, bounds, persistence, and compatibility. Validate migration, route rejection/rollback, overlapping notes, release tails, UI gestures, live/export parity, seek/loop behavior, and callback allocation.

Drive extends these shared modulation controls to Volume, Gain, Tone, Color strength, and Color frequency. It accepts shared Envelope and LFO sources from its own MIDI track and retains version-1 compatibility with empty connections and closed modulation controls as defaults. See [Drive modulation](../effects/drive.md#envelope-modulation) for parameter bounds, smoothing, and saved state.


## Shared modulation protocol — 2026-10-15

Every modulation provider and destination must use the [shared modulation contract](../modulation/contract.md). `unit-core` owns provider metadata, generic source identity, source enable publication, route resolution, target validation, scope compatibility, typed destination inputs, and additive offset smoothing. Envelope and LFO implement the source contract; Amplitude, Auto-filter, and Drive implement the destination contract. Both live and offline rendering call the same portable entry point. The catalog and validator discover sources through metadata rather than concrete Envelope settings. Host construction and note lifecycle remain outside the portable protocol. Existing routes and saved project defaults remain compatible.


## LFO component — 2026-10-16

Implement independent shared LFOs in each instrument layer through the existing component catalog and unit shell. Reuse the source publication, discovery, routing, validation, controls, and indicators in the [shared modulation contract](../modulation/contract.md). Amplitude, Auto-filter, and Drive accept these sources without provider-specific route code. Add Transport/Retrigger/Legato restart, Hz/note timing, waveform, phase, polarity, and smoothing as specified in [LFO component](../instruments/lfo.md). Keep the internal Auto-filter LFO compatible by sharing waveform/clock/control primitives. Continuous outputs cannot extend note lifetime; their normalized amplitude factor multiplies the contour result. Per-note LFOs and graph routing remain deferred.


## Instrument Main bus — 2026-10-16

Add one fixed [Main bus](../instruments/main.md) per instrument, separate from regular layers and project Master. Pin its row below the vertical layer viewport and above the effects lane. Apply its shared gain/pan after the stereo layer sum and before track effects. Reuse shared mix controls, unit panels, catalog actions, provider discovery, and modulation publication/resolution. Main hosts independent shared Envelope/LFO sources; it has no oscillator or Amplitude stage, and no per-note provider mode. Save identity, controls, and presentation with neutral legacy defaults. Preserve stable sources, voices, and allocation-free retargeting.


## Oscillator pitch modulation — 2026-10-16

Implement [per-oscillator pitch modulation](../modulation/oscillator-pitch.md) with stable source and destination identities. Extend the shared typed resolver and controls with an optional target ID instead of interpreting list indices or adding a separate routing path. Accept per-note envelopes and shared Envelope/LFO sources, including Main. Keep semitone amounts independent of base transpose, retain phase on compatible edits, save routing/open state, and preserve legacy unmodulated sound. Removing a source must not redirect surviving connections. All existing destinations also share the route-history retarget helper.


## Destination-matched modulation controls — 2026-10-16

All destination-owned amount knobs use the same [parameter profile](../modulation/contract.md#destination-matched-amount-knobs) as their regular destination control. Reuse native scale, reference marks, keyboard operation, and mouse/Shift sensitivity through a signed amount adapter. Keep existing amount units, bounds, project data, and audio behavior. Use percent, Q, dB, octave, or semitone readouts as appropriate; do not add independent generic amount knobs in unit panels.

## Installed component transfers and effect copies — 2026-10-16

Installed instrument headers now move components between layers; individual oscillator strip gaps transfer one oscillator. Option-drag on macOS or Ctrl-drag elsewhere creates copies. Whole Oscillator transfers merge sources and targeted routes. Provider moves remap source references, including track effects. Main accepts shared Envelope/LFO components; occupied Amplitude stages reject drops. Core applies each transfer atomically. Effect headers use the same modifier to copy the selected group in chain order through their existing insertion preview. See [gestures, identity rules, and limits](../editing/components.md).

## LFO modulation inputs — 2026-10-16

LFO now provides and consumes control signals through the shared modulation contracts. Depth and Rate use the existing destination-owned controls, source discovery, scope validation, route resolver, smoothing, markers, stable IDs, and save format. The host prepares an acyclic provider-first order; self/feedback connections are invalid even when inactive. Shared Envelope and LFO inputs are supported; voice-local Envelope inputs remain invalid. See [LFO modulation](../instruments/lfo.md#depth-and-rate-modulation) for bounds, continuous rate integration, the direct-seek history reset, and setup examples.


### Envelope timing controls — 2026-10-16

Envelope adds Time/Note timing and moves the Mode dropdown from its header to the first body column. One leading 90-point strip stacks the Timing knob above the Mode dropdown before the four ADSR strips; all retain the shared controls, separators, footer alignment, and 244-point height. Note timing scales Attack and Decay to the full triggering note length, preserves independent Time values, and leaves Release in milliseconds. Shared destinations consume the existing source interface without special handling. The detailed requirements and overlap rules are in [Envelope unit](../instruments/midi-patterns.md#envelope-unit). This supersedes the earlier four-strip-only Envelope layout.


### Macro component — 2026-10-16

[Macro](../instruments/macro.md) adds multiple independent shared control strips inside one component on regular layers or Main. Each strip has one 0–100% knob, a shared circular Power control, and a name label that defaults to Macro N. Double-click the label to rename the strip; names save with the project and appear in shared modulation controls. One strip may feed several destinations, and may receive shared Envelope/LFO/Macro modulation through the existing targeted controls. Stable IDs, target-specific cycle checks, neutral power weights, shared source/destination processing, save/load defaults, and component transfer transactions are required. The portable processor lives in `crates/macro`; UI and host construction remain outside it.

## Component selection and presentation order — 2026-10-17

Instrument components reuse effect-unit selection outlines and click/focus helpers. Header drags reorder units within layers or Main and place transferred units at the shared effects insertion line, including empty destinations. Oscillator/source units remain first. Saved `component_order` entries use shared `unit-core::InstrumentComponent` identities and validation; they change only presentation, never provider/source vector order or DSP topology. Retained hidden units keep their positions, and legacy projects keep canonical order. See [selection, placement, and persistence](../editing/components.md#selecting-and-reordering-component-units).

## Component group editing — 2026-10-17

Shift-click toggles component units within one lane. Selected headers move or duplicate the complete group in display order within/between regular layers and Main. Effects and components share selection/group-capture and insertion/no-op helpers. Audio sources stay first, and Oscillator/Macro transfers retain their merge rules. Core prepares one validated Batch edit through the existing member transfer/remapping functions; one incompatible member rejects the whole drop. Existing settings, saved layout, renderer publication, and source-assignment rules apply. See [component editing](../editing/components.md#selecting-and-reordering-component-units). This supersedes the earlier component multi-selection limit.

## Selected component removal — 2026-10-17

Delete/Backspace removes selected installed component units from the focused regular-layer or Main lane through one instrument edit. Reuse the effect key handler and pane guards; preserve text editing, consume repeats, retain lane focus after removal, and protect selected clips. Retain removed Envelope/LFO/Macro/Amplitude settings and routes for catalog restoration; Oscillator removal discards its sources and pitch routes. See [component removal](../editing/components.md#removing-selected-component-units).

## Three-band EQ output monitors — 2026-10-17

[Three-band EQ](../effects/equalizer.md) adds a read-only output-level rail in each Low/Mid/High strip. Reuse the Limiter's thin horizontal rail geometry/painting, the common dBFS mapping, and OTT's prepared three-band atomic handle path. Independent portable analysis filters follow the EQ's smoothed outer frequency boundaries and combine stereo peaks with a 300 ms release. They observe final output, including band cuts and dry bypass, without entering the audio path or adding latency. Keep the existing unit height, control positions, compact/collapse rules, saved format, stable effect IDs, and callback constraints. Stop/seek reset monitoring; inactive output displays empty rails. See the linked guide and [acceptance requirements](iteration-1.md#three-band-eq-output-monitors--2026-10-17) for measurement limits and validation scope.

## Three-band EQ modulation — 2026-10-17

[3-band EQ](../effects/equalizer.md#modulation--2026-10-17) consumes shared Envelope, LFO, and Macro outputs from its MIDI instrument, including Main, through the common source/destination API. Expose Gain for all three fixed bands and Frequency/Q for Low and High; Mid continues using the outer boundaries. Reuse typed route validation/resolution, cached publication, the 1 ms offset smoother, destination-matched amount knobs, modulation strips/indicators, identity-based history retargeting, and prepared atomic snapshots. Band cuts, bypass, level rails, unit dimensions, and zero-route audio retain their behavior. Optional empty/closed modulation fields retain EQ version 1 compatibility. See the guide and [acceptance requirements](iteration-1.md#three-band-eq-modulation--2026-10-17) for parameter units, bounds, scope, persistence, and callback checks.

## Compressor modulation — 2026-10-17

[Compressor](../effects/compressor.md#modulation--2026-10-17) consumes shared Envelope, LFO, and Macro outputs from its MIDI instrument, including Main, through the common source/destination API. Expose Threshold, Ratio, Attack, Release, Knee, Level, and Dry/Wet. Keep Peak/RMS discrete and the gain-reduction meter read-only. Reuse typed route validation/resolution, cached publication, the 1 ms offset smoother, destination-matched amount knobs, connection strips/indicators, stable-ID history retargeting, and the shared processed-value snapshot path. Preserve detector/reduction history, base ramps, bypass, unit geometry, neutral audio, and latency. Optional empty/closed fields retain Compressor state version 1. See the guide and [acceptance requirements](iteration-1.md#compressor-modulation--2026-10-17) for units, bounds, persistence, scope, and validation.

## Sampler source — 2026-10-17

[Sampler](../instruments/sampler.md) is a regular-layer source component in Instrument. A layer accepts Oscillator or Sampler, with multiple oscillator strips still allowed inside Oscillator. This supersedes earlier wording that allowed arbitrary mixed source components within one layer. Instruments can still combine different source types in parallel layers. Sampler reuses catalog registration, unit shells, component editing, immutable clip mapping, waveform/header painting, the shared range bar, Gain knob, mode dropdowns, worker imports/stretching, MIDI voices, Amplitude modulation, layer/Main mixing, and atomic telemetry. Its duration-group Off/Time (ms)/Bars dropdown controls duration; the adjacent One-shot/Gate dropdown controls note-off behavior. Gain accepts per-note/shared Envelope, LFO, and Macro modulation through the [shared destination API](../instruments/sampler.md#gain-modulation--2026-10-17). Root note is saved metadata only. See the guide and [acceptance requirements](iteration-1.md#sampler-source--2026-10-17) for bounds, ownership, persistence, and deferred pitch support.


## Limiter modulation — 2026-10-17

[Limiter](../effects/limiter.md#modulation--2026-10-17) accepts shared Envelope/LFO/Macro modulation for Input, Release, Lookahead, and Ceiling through the common destination API. Mode/Routing and the reduction rail retain their discrete/read-only roles. Reuse source discovery/publication, typed resolution, smoothing, connection controls, amount knobs, indicators, and history retargeting. Modulation retains fixed latency and the existing protection, transport, persistence, and callback rules. See the [acceptance requirements](iteration-1.md#limiter-modulation--2026-10-17) for units, bounds, and reconstruction limits.


## OTT modulation — 2026-10-17

[OTT](../effects/ott.md#modulation--2026-10-17) consumes 19 continuous controls through the shared source/destination API. Reuse the common provider cache, typed routing/scope validation, offset smoother, destination-matched controls/indicators, prepared 19-reading snapshots, and stable-ID history retention. Keep mode/power discrete and meters read-only. Preserve the DSP topology, bypass and band-enable meanings, fixed curves, stereo phase matching, and neutral audio. Portable OTT owns final bounds and crossover ordering. See the [acceptance requirements](iteration-1.md#ott-modulation--2026-10-17).


## Instrument catalog categories — 2026-10-17

Group registered components under Sources (Oscillator, Sampler), Modulators (Envelope, LFO, Macro), and Processors (Amplitude). All folders start open and retain expansion as application UI state. Reuse the shared catalog, `ControlPanel` row painting, selection/focus, keyboard navigation, and drag capture. See [catalog folders](../editing/components.md#instrument-catalog-folders) for registration, visible-row order, scrolling, hidden selection pruning, guards, and acceptance.
