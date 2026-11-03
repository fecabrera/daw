# Plate Reverb

Plate Reverb adds a dense stereo tail, with input filtering, a gated option and an
optional output amplifier. Add it from **Reverb** in the Effects catalog. It works
on tracks, groups, instrument layers and buses, beat instruments, and Master.

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Reverb | Pre-delay | 0–300 ms | 0 ms |
| Reverb | Decay | 0.3–15 s | 3 s |
| Reverb | Tail | Normal / Gated | Normal |
| Filters | High-pass | 20–1000 Hz | 20 Hz |
| Filters | Low-pass | 500 Hz–15 kHz | 15 kHz |
| Filters | Color | Bright / Dark | Bright |
| Character | Drive | −12 to +12 dB | 0 dB |
| Character | Drive power | Off / On | Off |
| Character | Quality | ECO / HQ | ECO |
| Out | Width | 0–100% | 100% |
| Out | Level | Exact silence to +12 dB | 0 dB |
| Out | Dry/Wet | 0–100% | 50% |

Pre-delay separates the source from the reverb onset. Decay sets the feedback
decay target; filtering and diffusion also affect the audible tail. Gated selects
a short response. It is not a level-triggered noise gate. Gated overrides Quality
and retains the saved quality choice for Normal mode.

ECO uses mono excitation with stereo output. HQ retains stereo excitation and a
longer convolution span. Color changes the tail damping. Width affects only the
wet signal: zero makes it mono. Dry/Wet uses a curved blend, so its midpoint is
not a half-amplitude linear mix.

Filter frequency readouts use whole Hz below 1 kHz and one decimal place in kHz
from 1 kHz upward. The labels below each frequency knob show its range.

Drive processes the complete dry/wet blend. It can therefore change the sound at
0% wet. Use the power button beside **Drive** to enable it. Level follows it.
Header power bypasses the complete unit while keeping
its internal history running. The Out rail shows final sample peaks; click it to
reset the held peak. Double-click a knob to reset it and hold Shift for fine drag.

All knobs accept shared modulation through the standard connection strips.
Discrete selectors are not modulation destinations. A compatible edit preserves
the tail. Pause and looping retain history; seeking prepares history before
playback. DAW Stop clears it.

Export appends an effect tail, capped at ten seconds after the arrangement. A
warning reports truncation. Leave silence at the end of the arrangement when a
longer tail must fit inside the export. Bypassed reverbs add no tail budget.

Independent [CLAP and VST3 previews](plugins.md) have the same controls and sound.
This is an original plate-style processor. Sample-identical reproduction of a
reference plugin has not been established. Native-host and listening acceptance
are separate from automated checks.
