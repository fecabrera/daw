# Direction Mixer

Direction Mixer controls stereo direction and width and decodes Mid/Side recordings. Add it to an audio/MIDI track or Master from Imaging in the Effects catalog in Timeline or Instrument.

## Controls

| Control | Range/options | Default |
| --- | --- | --- |
| Input | L/R, M/S | L/R |
| Direction | −180° to +180° | 0° |
| Spread | 0–2 | 1 |

Use L/R for ordinary stereo audio. Use M/S for a recording with Mid on the left input and Side on the right input. M/S decodes that recording to left/right stereo; it does not select a processing mode for ordinary stereo.

Direction positions the center of the stereo image. At 0° it stays centered. Turning the knob left uses negative angles and moves the center left; turning right uses positive angles and moves it right. −90° moves it fully left and +90° fully right. Moving beyond those angles brings the center back toward the middle with the sides exchanged. Both −180° and +180° exchange the left and right sides at the center position. The image narrows as it approaches either ±90° position.

Spread controls width before Direction. With L/R input at Direction 0°, Spread 0 collapses to mono and 1 retains the original width. Above 1, Spread reduces the center while retaining the stereo difference: 1.5 keeps 65% of the center, and 2 keeps 30%. This makes the stereo detail more prominent and can reduce the overall level. A mono source stays mono, with a lower level above 1.

With M/S input, Spread controls the Side level directly: 0 removes Side, 1 retains its recorded level, and 2 doubles it. Widening can increase output peaks; use the track or Master meters to check the result.

To check width, use L/R input and set Direction to 0°. Compare Spread 0, 1, and 2 with audio that has different left and right channels. Near ±90° Direction, the stereo difference decreases; at ±90° only the center remains, and Spread above 1 still reduces its level.

L/R with Direction 0° and Spread 1 leaves the input unchanged. M/S with those settings produces the decoded recording. Header power bypasses the entire unit, including M/S decoding, and passes the original two input channels. The controls retain their settings while off.

Double-click a knob to restore its default. Hold Shift while dragging for fine adjustment. Compact and collapsed units hide the knobs.

## Modulation

Open the secondary-color header dot for [modulation connections](../modulation/README.md). Direction and Spread accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Per note Envelopes cannot feed this track effect. Audio tracks and Master have no instrument providers; their connection controls can remain unassigned.

Direction amounts use degrees. Spread amounts use linear spread units. Contributions add to each base setting, with the result limited to the control's range. The base pointer and readout retain the saved value; range arcs and live dots show modulation. Input remains a manual choice.
