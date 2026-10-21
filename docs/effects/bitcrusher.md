# Bitcrusher

Bitcrusher adds digital distortion by reducing amplitude precision and holding samples longer. Add it to an audio/MIDI track or Master from Distortion in the Effects catalog in Timeline or Instrument.

## Controls

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Crush | Drive | 0–24 dB | 0 dB |
| Crush | Resolution | 1–24 bits, whole steps | 24 bits |
| Crush | Downsampling | 1×–40×, whole steps | 1× |
| Output | Level | Exact silence to +12 dB | 0 dB |
| Output | Dry/Wet | 0–100% | 100% |

Drive boosts the processed signal before clipping and crushing. Peaks above full scale are hard-clipped. Lower Resolution produces coarser amplitude steps and stronger distortion. At 1 bit, nonzero samples become full positive or negative levels; exact digital silence remains silent. The default 24-bit setting still applies fine quantization and clips peaks above full scale.

Downsampling reduces how often the effect captures a new sample. At 1× it captures every sample; at 2× it holds each capture for two samples. At a 48 kHz project rate, 40× gives an effective capture rate of 1.2 kHz. This changes the sound without changing playback timing. Aliasing is intentional: high frequencies can turn into lower, metallic tones. Both channels share capture timing and retain separate audio values. The same ratio sounds different at different project rates.

Dry/Wet blends the original audio with the clipped, crushed, and held signal. Level adjusts the complete blend. Dry/Wet at 0% keeps the original sound apart from Level. Header power bypasses the whole unit, including Level, and retains all settings. Stop or seek starts a fresh capture; Pause retains the current state.

Double-click a knob to restore its default. Hold Shift while dragging for fine control. Resolution and Downsampling retain whole steps with Shift. Compact and collapsed units hide the normal controls.

## Modulation

All five controls accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Per note and foreign-instrument sources cannot feed this track effect. Audio tracks and Master have no instrument providers; connections can remain unassigned. Use the secondary-color header dot to open [modulation connections](../modulation/README.md).

| Destination | Signed Amount range |
| --- | --- |
| Drive | −24 to +24 dB |
| Resolution | −23 to +23 bits |
| Downsampling | −39 to +39 ratio steps |
| Level | −24 to +24 dB |
| Dry/Wet | −100 to +100 percentage points |

Contributions add to the base settings and stay within their bounds. Modulated Resolution and Downsampling resolve to whole steps. These steps are part of the effect and can produce abrupt changes in its texture. Exact base Level silence stays silent under modulation.

Base pointers and readouts retain the saved values. Range arcs and moving dots show modulation; the processed Resolution and Downsampling readings remain whole numbers.
