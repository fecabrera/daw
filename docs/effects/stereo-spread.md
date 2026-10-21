# Stereo Spread

Stereo Spread widens audio by distributing successive frequency bands toward alternating left and right channels. It works with mono and stereo recordings. Add it to an audio/MIDI track or Master from Imaging in the Effects catalog in Timeline or Instrument.

## Controls

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Lower | Intensity | −100% to +100% | 0% |
| Lower | Frequency | 20 Hz–20 kHz, logarithmic | 20 Hz |
| Upper | Intensity | −100% to +100% | 0% |
| Upper | Frequency | 20 Hz–20 kHz, logarithmic | 20 kHz |
| Spread | Order | 2–12 whole bands | 8 |

Lower and Upper Frequency define the range to spread. Lower stays at least 1 Hz below Upper. The transition at either end is gradual. Frequencies well outside that range retain their original placement.

Intensity sets the spread contribution at each end of the range, with a gradual blend between them. Zero adds no contribution. Positive and negative amounts of equal size produce opposite left/right patterns. Changing both signs reverses the generated pattern; it does not swap the original recording's channels. Opposite signs at the two ends let the pattern reverse within the frequency range.

Order sets the number of alternating bands. More bands produce a finer frequency pattern. Intensity controls how strongly that pattern affects the audio. Changes transition smoothly during playback.

Both intensities start at zero, so a newly added unit leaves the input unchanged. Raise or lower either Intensity to hear the effect. To keep the bass more centered, raise Lower Frequency. Spreading can increase individual channel peaks; use track and Master meters to check the result.

The original mono sum is retained at normal levels. Summing the output to mono removes the added spread contribution. The effect adds no alignment delay. Export ends at the arranged duration and does not append filter ringing.

Header power bypasses the effect and retains its settings. Double-click a knob to restore its default; hold Shift while dragging for fine adjustment. Compact and collapsed units hide the normal controls.

## Modulation

Open the secondary-color header dot for [modulation connections](../modulation/README.md). Lower/Upper Intensity and Lower/Upper Frequency accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Per note sources cannot feed this track effect. Audio tracks and Master have no instrument providers; connections can remain unassigned.

Intensity amounts use percentage points, from −100 to +100. Frequency amounts use octaves, from −6 to +6. Contributions add to each saved setting and stay within its range. Frequency boundaries retain their 1 Hz separation when modulated. Order remains a manual stepped control.

The base pointer and readout retain the saved value. Range arcs and moving dots show modulation; the dots reflect the actual constrained frequencies.
