# LFO

An LFO (low-frequency oscillator) repeats a control pattern. It does not generate audio. Add it from the Instrument component catalog to a layer or Main, then connect it through a receiving unit's [modulation controls](../modulation/README.md).

## Controls

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

Hz rate is independent of tempo. Note rate follows tempo and offers straight, dotted, and triplet durations. Phase shifts the waveform's starting position. Uni moves from zero to positive values; Bi moves above and below zero. Smooth rounds abrupt changes. Depth controls the contribution without changing a destination's base setting.

Restart chooses the clock behavior:

- **Transport:** follow project time.
- **Retrigger:** restart on every note, including overlapping notes.
- **Legato:** restart when a note begins with no other note held.

All three modes share one signal across destinations. Retrigger and Legato continue cycling after the last note ends. Power disables the contribution while keeping its cycle moving. Adding an LFO alone makes no connection. Removing one retains its settings for later restoration.

## Modulate Depth and Rate

Open the LFO's secondary-color header dot to add incoming connections. Shared Envelopes, Macros, and other LFOs in the same instrument can control Depth and Rate. Self-connections and feedback loops are not allowed.

Depth Amount spans −100% to +100%. Rate Amount spans −6 to +6 octaves: +1 doubles speed, −1 halves it. Rate stays within its mode's bounds. The selected base note or Hz value remains unchanged.

For a growing filter sweep, set LFO Depth to 0%, connect a shared rising Envelope to Depth at +100%, then connect the LFO to Auto-filter Frequency. Disable Auto-filter's internal LFO if you want only the external sweep.

## Timing limits

Seeking into a note with Rate modulation can produce a different phase from uninterrupted playback because the earlier rate movement is not replayed. Export follows the complete history from the project start. Pause retains the current state. Per-note LFOs, cross-track sources, random/custom waveforms, and stereo LFO outputs are not available. Auto-filter has its own internal stereo LFO.
