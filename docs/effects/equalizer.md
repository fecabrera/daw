# 3-band EQ

Use 3-band EQ to balance or remove Low, Mid, and High frequency ranges. Select an audio/MIDI track or Master, then add it from EQ & Filters in the Effects catalog in Timeline or Instrument.

## Controls

| Parameter | Range | Default |
| --- | --- | --- |
| Gain | −24 to +24 dB | 0 dB for each band |
| Frequency | 20 Hz to 20 kHz, logarithmic control | Low 250 Hz; High 2.5 kHz; Mid uses these boundaries |
| Q | 0.1 to 10, logarithmic control | 1/√2 for Low/High; Mid uses these edge controls |

Low and High are shelves. Mid adjusts the range between their Frequency controls and has no separate Frequency or Q control. If the boundaries cross, Mid uses the lower and upper values as its range. Equal boundaries leave no middle range.

A band's footer power button cuts that range rather than bypassing its Gain. Low off removes low frequencies, High off removes high frequencies, and Mid off removes the middle range. Higher Q makes cut edges steeper. Cuts have finite slopes; frequencies near a boundary can still pass. All three bands off produce silence. Header power bypasses the entire EQ and retains the band settings.

## Output monitors

Each band has a horizontal output-level monitor below the header. It shows the louder stereo channel in that range, from −60 to 0 dBFS; the overload color indicates values above 0 dBFS. Hover for the level. The analysis ranges overlap near their boundaries. During bypass they show the dry output. Stopped or paused playback shows empty rails. Compact and collapsed units hide the monitors.

## Modulation

Open the header's secondary-color dot to add [modulation](../modulation/README.md). Low/Mid/High Gain and Low/High Frequency and Q accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Main. Per note Envelopes are unavailable for this track effect.

Gain amounts use dB, Frequency uses octaves, and Q uses Q units. The knob pointer retains the base setting while range arcs and live dots show modulation. Band power and the monitors are not modulation destinations.
