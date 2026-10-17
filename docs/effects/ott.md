# OTT

OTT applies upward and downward compression independently to Low, Mid, and High bands. It can lift quiet detail and reduce louder peaks. Add it to an audio/MIDI track or Master from the Effects catalog.

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

Each band's horizontal meter is centered at 0 dB. Left indicates reduction; right indicates boost. Its visible range is ±24 dB. Hover for the actual value beyond the rail. The reading shows compression gain, excluding Input/Output balance, Dry/Wet, and Level. Stopped or paused playback shows zero.

## Limits

Compression thresholds, ratios, and knees are fixed. Independent upward/downward amounts and automatic makeup are not exposed. OTT has no external modulation controls. It is not an exact emulation of another product. Export does not append an effect tail.
