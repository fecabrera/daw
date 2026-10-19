# Oscillator pitch and gain modulation

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/modulation/oscillator-pitch.md).

Implemented on 2026-10-16. Each oscillator is a separate pitch destination. Envelope and LFO sources use the [shared modulation contract](contract.md), the same source selector, connection strips, power controls, header expansion dot, and knob indicators as Amplitude, Auto-filter, and Drive.

## Controls

1. Add Oscillator and an Envelope or LFO from the Instrument component catalog.
2. Click the secondary-color dot at the right of the Oscillator header. The regular waveform, pitch, gain, and source controls remain visible.
3. Add a connection with Plus. Select its source and an `Osc N · Pitch` target. Set a signed amount in semitones. Each new connection targets the first oscillator, starts enabled, has no source, and has zero amount.
4. Add more connections to modulate other oscillators or combine sources on one oscillator. Use the connection's power control to disable it, or its context menu to remove it.

Like every [destination-matched modulation amount knob](contract.md#destination-matched-amount-knobs), the amount knob reuses the base Pitch knob’s one-semitone steps and octave reference marks across −48 to +48 semitones. Shift slows dragging while retaining whole-semitone steps. Existing saved or programmatic fractional amounts remain supported and are retained until adjusted; +0.25 is +25 cents. Modulation never writes the base Pitch setting.

A Per note Envelope supplies an independent contour for every voice. Retrigger/Legato Envelopes and LFOs supply shared values. Shared sources in Main can also feed oscillator pitch. Connections remain inside the same instrument. Pitch routes do not control note length or amplitude; use Amplitude for that behavior.

The target picker shows current oscillator numbers. Saved routes identify the oscillator by UUID, not its number or position. Removing an oscillator removes only its connections. Remaining targets retain their IDs even when their displayed numbers change. Newly added oscillators receive fresh IDs. Duplicating a track preserves internal identities, scoped to the new track.

## Processing

For each voice and oscillator:

```text
modulation offset = sum(source value × signed semitone amount × source enable weight)
frequency = base MIDI/transpose frequency × 2^(smoothed offset / 12)
```

The shared resolver filters routes by parameter and destination ID and reads cached source values. Reading pitch routes does not advance a source again. Unassigned, missing, disabled, and invalid runtime sources contribute neutral modulation. A source's unipolar or bipolar range determines its signed contribution. Equal opposite amounts on the same source cancel.

Route amount and enable changes reuse the shared 5 ms ramp. Oscillator applies the shared 1 ms additive smoother. The summed, smoothed offset is limited to ±96 semitones before conversion to frequency. Nonpositive, nonfinite, or Nyquist-and-higher frequencies produce silence. Processing continues so a later valid frequency can become audible. These frequency limits do not change the saved base pitch.

Modulation changes retain waveform phase, held notes, and envelope progress. Compatible renderer replacements retain source and route history by stable ID, including oscillator reordering. Source additions/removals and waveform changes use the existing topology reconstruction and boundary crossfade. Live playback and export use `process_modulation` and the same portable `daw-oscillator` implementation. Random-access renders use stateful MIDI rendering even without track effects. Transport seeks retain the existing note reconstruction and boundary fade; offline random access pre-renders history for exact parity.

## Indicators and persistence

Assigned Pitch knobs show the shared status marker, source/amount tooltip, possible-range arc, and click-to-open action. A per-note source is valid for Pitch. Two live dots show the lowest and highest active voice transpose, excluding the MIDI note pitch and including smoothed modulation. When all voices have the same transpose, the dots coincide. The tooltip reports fractional values. Range arcs and dots are limited visually to the base knob scale; the tooltip can report values beyond it. No UI clock estimates the audio value.

Instrument version 3 saves each oscillator's `id`, layer-owned `oscillator_modulations`, and `oscillator_modulation_open`. A pitch connection adds an optional `target` UUID to the shared route format. Existing non-instance destinations omit that field. Route IDs remain independent of source and target IDs. The settings contract validates supported parameters, finite bounded amounts, unique route IDs, and targets that exist in that layer. Core validates source references in the same instrument. Invalid instrument edits fail as one transaction.

Older projects default to no pitch routes and closed controls. Missing oscillator IDs migrate deterministically in original list order, skipping explicit IDs. This preserves identical legacy sources without duplicate identities and gives repeated loads the same IDs. Explicit null, malformed, nil, or duplicate IDs are rejected by deserialization or validation. Saved IDs are never regenerated. Save/reopen, Save As, sample-rate changes, and track duplication preserve routing and presentation.

All route vectors, source instances, and telemetry handles are prepared outside the audio callback. Sampling, smoothing, telemetry publication, reset, and compatible retargeting allocate nothing and perform no file I/O or locking.

## Per-oscillator Gain

Implemented on 2026-10-19. The same stable oscillator target ID accepts `OscillatorGain` alongside `Pitch`, declared once in `ModulationParameter::OSCILLATOR`. Target pickers enumerate `Osc N · Pitch` and `Osc N · Gain` for every strip. Routes use the existing layer-owned vector, validation, source scope, source cache, route ramps, transfer/removal logic, and saved expansion state. Existing projects need no migration; missing connections remain empty. Gain amounts are signed dB in −24..+24. Invalid targets and amounts fail the existing transaction validation.

The portable Oscillator resolves both offsets through the typed entry point and one shared two-value additive smoother. The host supplies its existing 5 ms smoothed linear base gain (including source mute) through `set_base_gain`; the portable unit applies `base_gain × 10^(offset / 20)` before voice velocity and Amplitude. Sum limits are ±96 dB before conversion, and final gain is limited to +12 dB. Exactly zero base gain remains silent. Neutral offsets preserve the former sample multiplication exactly. `SourceUnit::next_sample` retains its standalone unmodulated waveform API. No routing, lookup, allocation, or new gain ramp is added to the host.

Compatible updates retain both smoothers and route history by oscillator UUID. Reordering, moves and copies reuse the shared targeted component transfer rules; copies receive fresh oscillator/route IDs. Gain routes do not own note release. Oscillator telemetry retains Pitch extrema in slots 0–1 and adds processed Gain extrema in dB in slots 2–3, excluding velocity and later mixing. The Gain knob and amount strip reuse the common gain profile; shared markers, range calculations, source metadata and click-to-open behavior accept per-note and shared Gain sources. No UI estimate replaces processed telemetry.
