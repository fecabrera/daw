# Ambient Reverb

Ambient Reverb adds a diffuse stereo space with an evolving texture and long
feedback decay. Add it from **Reverb** in the Effects catalog. It is available on
tracks, groups, instrument layers and buses, beat instruments, and Master.

Use **Ambient Reverb** for spacious, evolving textures and long reverb tails.

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Options | Quality | ECO / HQ | ECO |
| Reverb | Decay | 1–60 s | 7.5 s |
| Reverb | Bloom | 0–100% | 50% |
| Reverb | Modulation | 0–100% | 49.75% |
| Hi-Pass | Frequency | 20–120 Hz | 70 Hz |
| Hi-Pass | Slope | 6 / 12 / 24 dB/oct | 6 dB/oct |
| Hi-Pass | Hi-Pass | Off / On | Off |
| Lo-Pass | Low-pass | 500 Hz–15 kHz | 15 kHz |
| Lo-Pass | Slope | 6 / 12 / 24 dB/oct | 12 dB/oct |
| Lo-Pass | Lo-Pass power | Off / On | On |
| Out | Level | Exact silence to +12 dB | 0 dB |
| Out | Width | 0–100% | 100% |
| Out | Dry/Wet | 0–100% | 50% |

Decay sets the feedback decay target. Bloom increases diffusion delays and changes
the buildup; it is not a simple pre-delay. Modulation moves the diffusion delays.
Frequency adjusts brightness and damping, and also sets the high-pass cutoff when
Hi-Pass is on. HQ uses the longer convolution span. Both quality modes retain
stereo input information. Use the **Hi-Pass** power button in the strip footer to
toggle the filter. Frequency and Slope are disabled when Hi-Pass is off; their
saved values remain unchanged. Modulation remains available independently of
Hi-Pass. Slope sets the low-frequency roll-off: 6 dB/oct
retains the original filter, while 12 and 24 dB/oct use Butterworth filters. Each
mode is approximately −3 dB at the selected cutoff. Slope changes crossfade over
5 ms and retain the reverb tail.

Lo-Pass adds an independent filter to the wet input, in addition to the reverb's
existing brightness and damping. Its strip uses the same frequency knob, bottom
Slope selector and power footer as Plate Reverb. Power disables its controls and
retains their saved values. Changes use 5 ms ramps and retain the tail. New units
start at 15 kHz and 12 dB/oct with Lo-Pass on; older projects load it off to
preserve their sound.

Width affects only the wet signal. Dry/Wet uses a curved blend. Level drives the
output transformer model, which processes the complete blend. Consequently,
0% wet can still change the dry signal. Use header power for a dry comparison.
Level at exact silence remains silent even with modulation.

All knobs accept shared modulation through the standard connection strips.
Quality, Hi-Pass and Slope remain discrete. The unit uses the standard knob reset,
Shift fine adjustment, power, collapse, selection, and effect-chain gestures.
The Out rail measures final sample peaks. Click it to reset the held peak.

Compatible edits, Pause, and looping retain the tail. Seeking prepares history;
DAW Stop clears it. Export appends a tail up to ten seconds and warns when the
reported tail budget exceeds that cap. For longer fades, extend the arrangement
with silence. Reverbs that are bypassed add no tail budget.

Independent [CLAP and VST3 previews](plugins.md) use the same body and processor.
This is an original ambient processor. Exact reference-plugin equivalence has not
been established. Long feedback settings can use substantial processing time and
memory; native-device performance and listening checks remain separate from the
automated tests.
