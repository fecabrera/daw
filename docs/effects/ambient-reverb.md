# Ambient Reverb

Ambient Reverb adds a diffuse stereo space with an evolving texture and long
feedback decay. Add it from **Reverb** in the Effects catalog. It is available on
tracks, groups, instrument layers and buses, beat instruments, and Master.

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Reverb | Decay | 1–60 s | 7.5 s |
| Reverb | Bloom | 0–100% | 50% |
| Reverb | Quality | ECO / HQ | ECO |
| Texture | Tone | 0–100% | 50% |
| Texture | Modulation | 0–100% | 49.75% |
| Texture | High-pass | Off / On | Off |
| Out | Width | 0–100% | 100% |
| Out | Level | Exact silence to +12 dB | 0 dB |
| Out | Dry/Wet | 0–100% | 50% |

Decay sets the feedback decay target. Bloom increases diffusion delays and changes
the buildup; it is not a simple pre-delay. Modulation moves the diffusion delays.
Tone adjusts brightness and damping, and also changes the low-frequency cut when
High-pass is on. HQ uses the longer convolution span. Both quality modes retain
stereo input information. Use the power button beside **High-pass** to toggle it.

Width affects only the wet signal. Dry/Wet uses a curved blend. Level drives the
output transformer model, which processes the complete blend. Consequently,
0% wet can still change the dry signal. Use header power for a dry comparison.
Level at exact silence remains silent even with modulation.

All knobs accept shared modulation through the standard connection strips.
Quality and High-pass remain discrete. The unit uses the standard knob reset,
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
