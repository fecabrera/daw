# Chorus

Chorus thickens audio by mixing the original signal with short delays whose times move. Add it from **Imaging** in the Effects catalog. It works on audio/MIDI tracks, instrument layers, instrument Main and Master.

Motion uses two columns with equal inner and outer spacing: Hz/Note and Rate occupy the left column; Stereo phase and Voices occupy the right column.

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Chorus | Delay | 5–30 ms | 15 ms |
| Chorus | Depth | 0–100% | 25% |
| Chorus | Feedback | −80% to +80% | 0% |
| Motion | Rate mode | Hz / Note | Hz |
| Motion | Rate | 0.05–10 Hz / dotted 4 bars to triplet 1/64 | 0.8 Hz / 1/4 |
| Motion | Stereo phase | −180° to +180° | 90° |
| Motion | Voices | 1–4 per channel | 2 |
| Out | Width | 0–200% | 100% |
| Out | Level | Exact silence to +12 dB | 0 dB |
| Out | Dry/Wet | 0–100% | 35% |

Delay sets the average delay time. Depth controls its movement. At rates up to 0.8 Hz, 100% sweeps 45% above and below Delay. Above 0.8 Hz, the delay excursion decreases as Rate increases, keeping the amount of detuning approximately constant at settled settings. This compensation follows Hz, note duration, tempo and rate modulation. It does not increase the excursion at slower rates. Depth at zero leaves a static delay. Feedback returns some delayed sound to the input and adds resonance; negative values invert that return.

Rate controls a sine LFO. Select **Hz** for an independent frequency or **Note** to follow project tempo and transport, using the same selector as Auto-filter. Note includes straight, dotted (D), and triplet (T) durations. Switching modes retains both rate settings. Stereo phase offsets the right channel's modulation from the left. Voices adds delayed copies with evenly spaced modulation phases. More voices are averaged to avoid a level jump from their count alone. The LFO uses a fixed sine shape.

Width affects the wet signal only: 0% makes it mono, 100% retains its normal stereo spread, and 200% increases its Side component. Dry/Wet blends that result with the original signal. Level adjusts the final blend. The rail above Out shows the louder output channel and turns empty when bypassed or stopped.

Header power bypasses the complete effect, including Level, while retaining delay and modulation history. Changing controls or rearranging a compatible unit retains that history. Double-click a knob to reset it; hold Shift while dragging for fine adjustment. Collapse and compact views hide the normal controls.

All controls except Voices accept shared Envelope, LFO and Macro modulation through the existing [connection strips](../modulation/README.md). Rate amounts use octaves in both modes and stay within the active mode’s rate range; Delay uses milliseconds; Stereo phase uses degrees; Depth, Feedback, Width and Dry/Wet use percentage points; Level uses dB. Audio tracks and Master have no instrument providers, so their connections can remain unassigned.

Chorus continues processing through timeline gaps. Export ends at the arranged duration and does not append its delay tail. Seeking reconstructs history on a worker; long arrangements can need preparation time. The dry signal has no added delay; the wet delays are part of the effect's sound.

The output strip uses the [standard effect output meter and held-peak indicator](output-meter.md). Click its rail to reset only the held peak.

This effect also has a [standalone CLAP/VST3 plugin](plugins.md).
