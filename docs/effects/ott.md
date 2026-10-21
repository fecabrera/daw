# OTT

OTT applies upward and downward compression independently to Low, Mid, and High bands. It can lift quiet detail and reduce louder peaks. Add it to an audio/MIDI track or Master from Dynamics in the Effects catalog.

## Controls

| Control | Range | Default |
| --- | --- | --- |
| Low/Mid crossover | 20 Hz–2 kHz, logarithmic | 250 Hz |
| Mid/High crossover | 200 Hz–20 kHz, logarithmic | 2.5 kHz |
| Band Depth | 0–100% | 100% |
| Band Input | −24–+24 dB | 0 dB |
| Band Output | −24–+24 dB | 0 dB |
| Band Attack | 0.1–200 ms, logarithmic | 10 ms |
| Band Release | 10–2000 ms, logarithmic | 100 ms |
| Band compression | On/off | On |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |
| Detection | Peak, RMS | Peak |

The two crossovers define the three bands. Low must remain below High; editing a crossover keeps at least 1 Hz between them. Mid has no separate Frequency control.

Input changes the level entering each band's compression. Output changes it afterward. Depth scales the compression effect; 0% keeps Input and Output active. Attack controls how quickly the gain change grows, and Release controls how quickly it returns toward neutral.

Turning a band off bypasses its compression while keeping the band audible and its Input/Output gains active. Header power bypasses the entire unit. Dry/Wet blends original and processed audio; Level adjusts that blend.

## Gain-change monitors

Each band's horizontal meter is centered at 0 dB. Left indicates reduction; right indicates boost. Its visible range is ±24 dB. A disabled band uses secondary grey for its meter. The reading shows compression gain, excluding Input/Output balance, Dry/Wet, and Level.

A thin line in the secondary text color marks each band's greatest reduction on the left and another marks its greatest boost on the right. Click a band's rail to reset both of that band's lines. Other bands keep their holds. The current readings and compression settings stay the same. If compression continues, new readings can immediately set new peaks.

Hover for the actual signed gain, peak reduction, and peak boost, including values beyond the rail. The lines stop at the rail ends at 24 dB in each direction. Peak capture continues while the unit is hidden, compact, or collapsed. The holds remain during pause, stop, seeking, and individual band bypass.

Turning the whole unit off clears all fills and peak lines and stops meter updates. Turning it on resumes monitoring. Current readings show zero while playback is stopped or paused, or no output device is available. Peak readings are temporary monitor state and are not saved with the project.

## Modulation

Click the secondary-color dot in the header to open connections. Use Plus to add one, select a Source and Parameter, then adjust Amount. Shared Retrigger/Legato Envelopes, LFOs, and Macros from the same MIDI instrument are available, including Output sources. Per note Envelopes cannot feed this track effect. Audio tracks and the project Master have no instrument sources.

Both crossovers, each band's Depth, Input, Output, Attack, and Release, plus Level and Dry/Wet accept modulation.

| Parameters | Amount range |
| --- | --- |
| Crossover frequencies | −6 to +6 octaves |
| Band Depth and Dry/Wet | −100% to +100% |
| Band Input/Output and Level | −24 to +24 dB |
| Band Attack/Release | −6 to +6 octaves of time |

An octave doubles or halves the value. Positive Attack/Release amounts make the response slower; negative amounts make it faster. Modulation stays within each control's range. If modulated crossovers meet, High is raised to keep a 1 Hz gap above Low. The base knobs stay unchanged, and exact Level silence stays silent.

Assignment dots, range arcs, tooltips, and moving values show modulation activity. Power disables a connection, and its context menu removes it. Connections save with the project. Band off still allows Input/Output modulation; Peak/RMS and power remain manual choices. See [modulation](../modulation/README.md) for common connection controls.

## Limits

Compression thresholds, ratios, and knees are fixed. Independent upward/downward amounts and automatic makeup are not exposed. It is not an exact emulation of another product. Export does not append an effect tail.
