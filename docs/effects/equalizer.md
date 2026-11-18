# 3-band EQ

Use **3-band EQ** to balance or remove broad Low, Mid and High frequency ranges.

Select an audio/MIDI track or Master, then add it from EQ in the Effects catalog in Timeline or Instrument.

## Controls

| Parameter | Range | Default |
| --- | --- | --- |
| Gain | −24 to +24 dB | 0 dB for each band |
| Frequency | 20 Hz to 20 kHz, logarithmic control | Low 250 Hz; High 2.5 kHz; Mid uses these boundaries |
| Q | 0.1 to 10, logarithmic control | 1/√2 for Low/High; Mid uses these edge controls |

Low and High are shelves. Mid adjusts the range between their Frequency controls and has no separate Frequency or Q control. If the boundaries cross, Mid uses the lower and upper values as its range. Equal boundaries leave no middle range.

A band's footer power button cuts that range rather than bypassing its Gain. Low off removes low frequencies, High off removes high frequencies, and Mid off removes the middle range. Higher Q makes cut edges steeper. Cuts have finite slopes; frequencies near a boundary can still pass. All three bands off produce silence. Header power bypasses the entire EQ and retains the band settings.

## Response graph

The expanded unit shows the combined response, each band's response and a live output spectrum, using the same graph style as Multiband EQ. The response includes band cuts. The spectrum clears when the unit is off or playback stops. Compact and collapsed units hide the graph.

Drag Low or High left or right to change Frequency, or up or down to change Gain. The first drag direction selects the control. Move at least 12 points across the other axis to switch. Hold Command on macOS or Ctrl on Windows/Linux to adjust Q. You can switch modes without releasing the mouse. Mid moves vertically to change Gain. Its frequency follows the outer bands. Hold Shift for fine adjustment.

The knobs and sound follow the drag. Release to apply one Undo step in the DAW. Escape or focus loss restores the previous values. The DAW prepares audio previews in the background. Standalone plugins send host parameter gestures. Click away from the points to deselect the band.

## Output monitors

Each band has a horizontal output-level monitor below the header. It shows the louder stereo channel in that range, from −60 to 0 dBFS. Enabled bands use the same green as track level meters; red indicates values above 0 dBFS. These colors do not follow the accent color. Disabled bands use secondary grey. Hover for the level. The analysis ranges overlap near their boundaries. Turning the unit off clears all three monitors and stops their updates. Turning it on resumes monitoring. Stopped or paused playback shows empty rails. Compact and collapsed units hide the monitors.

## Modulation

Open the header's secondary-color dot to add [modulation](../modulation/README.md). Low/Mid/High Gain and Low/High Frequency and Q accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Per note Envelopes are unavailable for this track effect.

Gain amounts use dB, Frequency uses octaves, and Q uses Q units. The knob pointer retains the base setting while range arcs and live dots show modulation. Band power and the monitors are not modulation destinations.

This effect also has a [standalone plugin](plugins.md).
