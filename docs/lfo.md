# LFO component

Implemented on 2026-10-16. An LFO (low-frequency oscillator) is a modulation component, separate from the audio Oscillator. It produces one shared control signal. It does not generate audio or prolong a note.

## Controls and placement

Add LFO from the Instrument component catalog. Double-click, Enter, multi-selection, and dragging onto a layer use the existing catalog actions. Each layer can contain multiple independent instances, named LFO 1, LFO 2, and so on. Adding one creates no connections. Select it in an Amplitude, Auto-filter, or Drive modulation strip to connect it, then set that strip's signed Amount.

| Control | Values | Default |
| --- | --- | --- |
| Wave | Sine, Triangle, Saw up, Saw down, Square | Sine |
| Rate mode | Hz, Note | Note |
| Rate | 0.01–20 Hz; 27 straight, dotted, and triplet note durations from 4 bars through 1/64 | 1/4 note; retained Hz value 2 Hz |
| Phase | −180° to +180° | 0° |
| Polarity | Uni (0–1), Bi (−1–1) | Uni |
| Smooth | 0–100 ms | 1 ms |
| Restart | Transport, Retrigger, Legato | Transport |

The panel has two 90-point strips with the same three control rows and footer alignment as other components. Wave, Phase, and Polarity occupy the first strip. Rate mode, Rate, and Smooth occupy the second. Restart is the compact header selector. Wave/rate/phase controls share code with Auto-filter. All knobs use the common knob widget. Header power, collapse, compact layer layout, and theme colors use the common unit shell.

The unit header context menu removes an LFO while retaining its settings, identity, and connections. Re-adding restores the first removed LFO before creating another. The layer saves its `lfos` list, including power and collapse, in instrument version 3. Missing lists in older projects default empty. Explicit malformed settings and duplicate source IDs fail validation. Project replacement, Save As, duplication, and sample-rate changes retain settings and valid references.

## Timing and signal behavior

- **Transport:** phase follows absolute project time. Pausing freezes it. Seeking and looping reproduce phase at the destination frame.
- **Retrigger:** phase restarts on every note-on, including overlapping notes. All voices and destinations share the same result.
- **Legato:** phase restarts when a note starts with no other held note. Overlapping notes share the existing cycle. A note-off at the same frame as a new note-on occurs first, so adjacent non-overlapping notes restart.

Retrigger and Legato continue cycling after the final note ends. Before any note they use project frame zero as the origin. They do not create release tails or revive ended voices. Notes hidden by a trim retain their original, possibly virtual start position. Header repeats map event origins through each repeat's source offset and phase. MIDI stretch maps note starts to the first output frame at which the source clock reaches that event. Timing tables are prepared off the audio thread and do not expand repeats.

Note rate follows tempo; Hz rate is independent of tempo. During continuous processing, rate/tempo changes use the shared clock's 5 ms shortest-phase correction toward the new project-locked phase. DAW edits that change note timing reconstruct the instrument through the existing renderer transition. Wave, Phase, and Smooth changes ramp over 5 ms. Source power and route changes use the existing 5 ms contribution ramps. Off LFOs keep advancing. Polarity and Restart are discrete settings. The selected phase is added to the chosen clock origin.

Smooth applies the periodic response of a one-pole low-pass filter to the waveform, in continuous time. The implementation evaluates this response analytically for all five shapes. This rounds sharp transitions without replaying a long sample history after a seek. Zero bypasses this smoothing. It is separate from the destination's existing parameter-offset smoothing.

## Shared modulation contract

The portable `daw-lfo` crate depends only on `daw-unit-core`. It implements the same `ModulationSource` interface as Envelope and declares a modulation output port. Its settings implement `ModulationSourceSettings`. The shared descriptor reports the selected polarity and `Continuous` behavior. Envelope reports `Contour` behavior.

The host publishes each source once per processing sample through `ModulationOutput`, with absolute frame, tempo, and prepared note/legato origins in `ModulationSourceContext`. Cached values fan out through the existing typed route resolver. No destination performs LFO-specific lookup or sampling. Source discovery, scope checks, validation, enable ramps, indicators, and destination processing remain shared; see [the modulation contract](modulation.md).

Amplitude accepts this shared signal alongside local/shared envelopes. It normalizes bipolar continuous signals to 0–1 and multiplies their gain factor by the contour result. At positive Amount 1, the continuous signal controls gain from zero to unity. At Amount 0, it is neutral. Signed amounts and multiple routes use the common resolver. Only contours can own note releases. Auto-filter and Drive use the raw selected polarity in their existing additive parameter units and apply their existing bounds and 1 ms offset smoothing. Their base controls stay unchanged.

The possible-range arc uses the declared polarity for additive targets and the normalized continuous gain range for Amplitude. Current-value dots use actual processed values from existing callback telemetry. The UI does not run another LFO clock.

## Scope and acceptance

All three restart modes are shared sources within the same MIDI instrument. Per-note LFOs, audio-track/Master providers, cross-track routing, stereo control outputs, random/custom waveforms, source amount controls, and a graph editor are outside this change. Auto-filter retains its independent internal stereo LFO, saved format, and behavior. It shares waveform, rate, clock, and UI primitives with the component.

Acceptance checks cover six processing rates; all waveforms, polarities, and note durations; smoothing and deterministic random access; overlap/adjacent-note restart; trim/stretch/repeat origins; source fan-out to all three destinations; contour/continuous amplitude composition; release lifetime; live/export/block/seek/loop parity; compatible retargeting and callback allocation; catalog keyboard/drag actions; shared panel geometry in both themes; saved identity, legacy defaults, and invalid settings. See [validation records](validation.md) for executed checks and native-platform limits.
