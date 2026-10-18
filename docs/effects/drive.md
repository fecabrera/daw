# Drive

Drive adds soft-clipping saturation with adjustable tone and pre-clipping color. Add it to an audio/MIDI track or Master from Distortion in the Effects catalog.

## Controls

| Control | Range | Default | Behavior |
| --- | --- | --- | --- |
| Volume | Exact silence to +12 dB | 0 dB | Output level after saturation and tone. |
| Gain | 0–100% | 20% | Soft-clipping intensity. Zero is clean; full gain provides approximately 36 dB of drive. |
| Tone | 0–100% | 50% | Post-clipping high shelf at 2.5 kHz; −12 to +12 dB, neutral at center. |
| Color shape | Off, Mid scoop, Mid hump, Mid hump + bass cut | Off | Pre-clipping voicing. Off leaves saturation active. |
| Color strength | 0–100% | 50% | Depth of the selected curve. Zero leaves a flat color response. |
| Color frequency | 250 Hz–2 kHz, logarithmic | 750 Hz | Mid center and linked bass-cut position. |

Mid scoop lowers the middle frequencies. Mid hump raises them. Mid hump + bass cut also reduces bass for a tighter sound. Strength controls the depth, and Frequency moves the color's center. Color Off disables that voicing and its controls; Gain, Tone, and Volume remain active.

Gain zero keeps the signal clean apart from the selected Color and Tone. Tone is neutral at 50%. Volume adjusts the output after the other controls. Header power bypasses the whole effect.

## Modulation and limits

Volume, Gain, Tone, Color strength, and Color frequency accept shared Envelope, LFO, and Macro sources from the same MIDI instrument. Open the secondary-color header dot and assign connections as described in [modulation](../modulation/README.md). Color shape remains a manual choice. Color modulation affects the sound only when a color shape is enabled.

Strong drive on high-frequency material can produce aliasing, heard as additional tones. Higher project processing rates reduce this limitation. Drive is not an exact model of a particular pedal. Export ends at the arranged duration.
