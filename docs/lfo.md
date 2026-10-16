# LFO component

Implemented on 2026-10-16. An LFO (low-frequency oscillator) is a modulation component, separate from the audio Oscillator. It produces one shared control signal. It does not generate audio or prolong a note.

## Controls and placement

Add LFO from the Instrument component catalog. Double-click, Enter, multi-selection, and dragging onto a layer use the existing catalog actions. Each layer can contain multiple independent instances, named LFO 1, LFO 2, and so on. Adding one creates no connections. Select it in an Oscillator, Amplitude, Auto-filter, Drive, or another LFO modulation strip to connect it, then set that strip's signed Amount.

| Control | Values | Default |
| --- | --- | --- |
| Wave | Sine, Triangle, Saw up, Saw down, Square | Sine |
| Rate mode | Hz, Note | Note |
| Rate | 0.01–20 Hz; 27 straight, dotted, and triplet note durations from 4 bars through 1/64 | 1/4 note; retained Hz value 2 Hz |
| Phase | −180° to +180° | 0° |
| Polarity | Uni (0–1), Bi (−1–1) | Uni |
| Smooth | 0–100 ms | 1 ms |
| Depth | 0–100% of the control output | 100% |
| Restart | Transport, Retrigger, Legato | Transport |

The panel has three 90-point strips with the same three control rows and footer alignment as other components. Wave, Phase, and Polarity occupy the first strip. Rate mode, Rate, and Smooth occupy the second. Depth occupies the first row of the third strip. Restart is the compact header selector; the secondary-color dot beside it opens the shared modulation strips without hiding the normal controls. Depth and Rate use the shared assignment marker, possible-range arc, click-to-open action, and live-value dots. Note-mode rate dots/arcs map the effective Hz value to the ordered note scale; the selected note and readout stay unchanged. Wave/rate/phase controls share code with Auto-filter. All knobs use the common knob widget. Header power, collapse, compact layer layout, and theme colors use the common unit shell.

The unit header context menu removes an LFO while retaining its settings, identity, and connections. Re-adding restores the first removed LFO before creating another. The layer saves its `lfos` list, including power, collapse, `modulation_open`, and `modulations`, in instrument version 3. Missing LFO Depth defaults to 100%; missing routes and open state default to empty and closed, preserving existing projects. Missing lists in older projects default empty. Explicit malformed settings and duplicate source IDs fail validation. Project replacement, Save As, duplication, and sample-rate changes retain settings and valid references.

## Timing and signal behavior

- **Transport:** phase follows absolute project time. Pausing freezes it. Without Rate modulation, seeking and looping reproduce phase at the destination frame. Rate modulation follows the history rule below.
- **Retrigger:** phase restarts on every note-on, including overlapping notes. All voices and destinations share the same result.
- **Legato:** phase restarts when a note starts with no other held note. Overlapping notes share the existing cycle. A note-off at the same frame as a new note-on occurs first, so adjacent non-overlapping notes restart.

Retrigger and Legato continue cycling after the final note ends. Before any note they use project frame zero as the origin. They do not create release tails or revive ended voices. Notes hidden by a trim retain their original, possibly virtual start position. Header repeats map event origins through each repeat's source offset and phase. MIDI stretch maps note starts to the first output frame at which the source clock reaches that event. Timing tables are prepared off the audio thread and do not expand repeats.

Note rate follows tempo; Hz rate is independent of tempo. During continuous processing, rate/tempo changes use the shared clock's 5 ms shortest-phase correction toward the new project-locked phase. DAW edits that change note timing reconstruct the instrument through the existing renderer transition. Wave, Phase, Smooth, and base Depth changes ramp over 5 ms. Source power and route changes use the existing 5 ms contribution ramps. Off LFOs keep advancing. Polarity and Restart are discrete settings. The selected phase is added to the chosen clock origin.

Smooth applies the periodic response of a one-pole low-pass filter to the waveform, in continuous time. The implementation evaluates this response analytically for all five shapes. This rounds sharp transitions without replaying a long sample history after a seek. Zero bypasses this smoothing. It is separate from the destination's existing parameter-offset smoothing.

## Depth and Rate modulation

LFO uses the existing destination-owned Source, Parameter, Amount, and Power strips. Connections are unassigned and enabled at zero Amount when added. Their order, stable route IDs, assignments, amounts, power, and open state save with the project. The shared strip gesture also reorders these connections.

| Target | Amount | Processing |
| --- | --- | --- |
| Depth | −100% to +100% offset per connection; 1-percentage-point steps | Add the resolved offset to base Depth and clamp to 0–100%. Apply that depth to the published contribution weight. Zero is neutral for every destination. |
| Rate | −6 to +6 octaves per connection; 1/24-octave keyboard steps | Multiply the base frequency by `2^offset`. +1 doubles speed; −1 halves it. Sum and smooth offsets before conversion. |

Hz-mode Rate stays within 0.01–20 Hz. Note-mode Rate stays between the slowest and fastest existing note choices at the current tempo; modulation varies continuously between them without changing the selected duration. Rate Amount shares the Hz control's logarithmic scale, keyboard operation, and pointer sensitivity through the existing amount adapter. Depth shares its native percentage control. Both retain signed amounts, neutral-zero reset, bounds, and Shift sensitivity from the shared knob base.

Both targets accept shared Retrigger/Legato Envelopes and other LFOs, including sources on Main. Per note Envelopes cannot feed a shared LFO. Source filtering and saved-reference validation use the shared scope and dependency rules. Self-connections and feedback cycles are rejected, including retained, disabled, and zero-amount connections. The picker excludes sources that would close a cycle. Preparation orders providers before consumers; saved row/strip order never decides audio processing order. Each source still advances once per sample and fans out its cached value.

Depth and Rate offsets use the common 1 ms parameter smoother. Source power and route amount/power use the common 5 ms ramps. Rate modulation integrates the frequency difference from the base clock each sample. It does not multiply elapsed note time by the current modulated frequency. This keeps a continuous sweep free of phase jumps. The selected restart origin clears the accumulated offset when that origin changes. The periodic Smooth response uses the current effective rate; under changing rate it remains an instantaneous periodic approximation.

A direct seek or a nonconsecutive source frame resets the accumulated Rate phase offset and parameter smoother at the destination. It starts from the ordinary base-clock phase, then integrates new modulation samples. It does not reconstruct earlier Rate modulation history in the callback. Starting inside a trimmed or repeated note likewise uses the base-clock phase for the first processed frame; hidden earlier Rate history is not synthesized. Consequently, a direct seek into an already modulated note can have a different LFO phase from uninterrupted playback. Stop/reset and backward loop jumps use the same reset rule. Pause preserves state. Offline export and random-range rendering process from frame zero and retain the full history; uninterrupted live playback, offline export, and different block sizes use the same samples. Existing unmodulated seek behavior stays unchanged.

To make filter movement grow during a held note:

1. Add a shared Envelope in Legato or Retrigger mode. Set its first-row Timing knob to Note, Attack to 100%, Decay to 0%, and Sustain to 100%.
2. Set LFO Depth to 0%. Open its modulation controls, select that Envelope, choose Depth, and set Amount to +100%.
3. Connect the LFO to Auto-filter Frequency and select the wanted signed sweep amount. Use bipolar polarity for movement above and below the base frequency. Disable Auto-filter’s internal LFO for this example.
4. Add a second connection to LFO Rate if the movement should also accelerate. For example, +2 octaves reaches four times the base speed, subject to the rate bounds.

Envelope Note timing scales Attack and Decay to the triggering note's complete duration. Retrigger uses each new note's length; Legato keeps the first held note's length through overlapping notes. Time timing remains available for fixed millisecond stages. Release remains in milliseconds. See [Envelope timing](midi-patterns.md#envelope-unit). Envelope timing is independent of LFO's tempo-synchronized Note rate and Restart selector. Depth scales the published contribution weight, so 0% leaves each destination at its base behavior, including Amplitude. The portable `sample_at` convenience method returns the scaled control signal; hosts use `ModulationOutput` to preserve neutral routing semantics. LFO Depth and destination Amount retain separate meanings.

## Shared modulation contract

The portable `daw-lfo` crate depends only on `daw-unit-core`. It implements the same `ModulationSource` interface as Envelope and the same `ModulationDestination` interface as effects. It declares both modulation ports. `LfoInstance` implements `ModulationSourceSettings` and `ModulationDestinationSettings`. The shared descriptor reports the selected polarity and `Continuous` behavior. Envelope reports `Contour` behavior.

The host resolves LFO inputs through `process_modulation` before publishing each source once per processing sample through `ModulationOutput`, with absolute frame, tempo, and prepared note/legato origins in `ModulationSourceContext`. Cached values fan out through the existing typed route resolver. No destination performs LFO-specific lookup or sampling. Source discovery, scope checks, validation, enable ramps, indicators, and destination processing remain shared; see [the modulation contract](modulation.md).

Amplitude accepts this shared signal alongside local/shared envelopes. It normalizes bipolar continuous signals to 0–1 and multiplies their gain factor by the contour result. At positive Amount 1, the continuous signal controls gain from zero to unity. At Amount 0, it is neutral. Signed amounts and multiple routes use the common resolver. Only contours can own note releases. Auto-filter and Drive use the raw selected polarity in their existing additive parameter units and apply their existing bounds and 1 ms offset smoothing. Their base controls stay unchanged.

The possible-range arc uses the declared polarity for additive targets and the normalized continuous gain range for Amplitude. Current-value dots use actual processed values from existing callback telemetry. The UI does not run another LFO clock.

## Scope and acceptance

All three restart modes are shared sources within the same MIDI instrument. Per-note LFOs, audio-track/Master providers, cross-track routing, stereo control outputs, random/custom waveforms, and a graph editor are outside this change. Auto-filter retains its independent internal stereo LFO, saved format, and behavior. It shares waveform, rate, clock, and UI primitives with the component.

Acceptance checks cover six processing rates; all waveforms, polarities, and note durations; smoothing and deterministic random access; overlap/adjacent-note restart; trim/stretch/repeat origins; source fan-out across supported destinations; contour/continuous amplitude composition; release lifetime; live/export/block/seek/loop parity; compatible retargeting and callback allocation; catalog keyboard/drag actions; shared panel geometry in both themes; saved identity, legacy defaults, invalid settings, input scope/cycle rejection, provider ordering, integrated frequency sweeps, Depth bounds, both rate modes, modulation controls and markers, component transfer/copy reference updates, and allocation-free live routing edits. See [validation records](validation.md) for executed checks and native-platform limits.
