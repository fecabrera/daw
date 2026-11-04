# Plate Reverb

Plate Reverb adds a dense stereo tail, with input filtering, a gated option and an
optional Drive stage on the wet signal. Add it from **Reverb** in the Effects catalog. It works
on tracks, groups, instrument layers and buses, beat instruments, and Master.

Use **Plate Reverb** to add a dense stereo tail, give a sound depth or create a short gated reverb.

| Strip | Control | Range | Default |
| --- | --- | --- | --- |
| Options | Quality | ECO / HQ | ECO |
| Options | Tail | Normal / Gated | Normal |
| Options | Color | Bright / Dark | Bright |
| Reverb | Pre-delay | 0–300 ms | 0 ms |
| Reverb | Decay | 0.3–15 s | 3 s |
| Hi-Pass | High-pass | 20–1000 Hz | 20 Hz |
| Hi-Pass | Slope | 6 / 12 / 24 dB/oct | 12 dB/oct |
| Hi-Pass | Hi-Pass power | Off / On | On |
| Lo-Pass | Low-pass | 500 Hz–15 kHz | 15 kHz |
| Lo-Pass | Slope | 6 / 12 / 24 dB/oct | 12 dB/oct |
| Lo-Pass | Lo-Pass power | Off / On | On |
| Drive | Mode | Soft / Hard / Tube | Soft |
| Drive | Drive | 0–100% | 20% |
| Drive | Tone | 0–100% | 50% |
| Drive | Drive power | Off / On | Off |
| Out | Level | Exact silence to +12 dB | 0 dB |
| Out | Width | 0–100% | 100% |
| Out | Dry/Wet | 0–100% | 50% |

Pre-delay separates the source from the reverb onset. Decay sets the feedback
decay target; filtering and diffusion also affect the audible tail. Gated selects
a short response. It is not a level-triggered noise gate. Gated overrides Quality
and retains the saved quality choice for Normal mode.

ECO uses mono excitation with stereo output. HQ retains stereo excitation and a
longer convolution span. Color changes the tail damping. Width affects only the
wet signal: zero makes it mono. Dry/Wet uses a curved blend, so its midpoint is
not a half-amplitude linear mix.

The wet signal has calibrated makeup gain before Dry/Wet. At the default 3 s
Decay it adds 16 dB in ECO or 13 dB in HQ. Compensation decreases for longer
decays and is bounded for shorter ones. Very short and gated tails remain
quieter. Compensation rises gradually when settings change, so a stored tail
does not receive a sudden boost. The gain follows the settings, not the incoming
audio, so it does not raise a fading tail automatically. Drive intensity and
feedback decay are unchanged. Out Level still adjusts the complete blend.

Existing projects use the louder wet output with their saved settings. Reduce
Dry/Wet or Out Level as needed. Makeup gain does not impose an output ceiling.

Filter frequency readouts use whole Hz below 1 kHz and one decimal place in kHz
from 1 kHz upward. The labels below each frequency knob show its range.
Use each filter strip's power button to bypass that filter. Its frequency knob
and bottom Slope dropdown are disabled while off; their saved values remain
unchanged. Each Slope selector offers 6, 12 and 24 dB/oct, with 12 as the default.
The filters use Butterworth responses, approximately −3 dB at cutoff, without a
resonance boost. Power and slope changes use 5 ms ramps and retain the reverb
tail. Existing projects keep both filters on with 12 dB/oct selected.

Drive processes only the wet signal before the dry/wet blend. At 0% wet, Drive
has no effect on the dry signal. Out Level still applies to the complete blend.

Use the **Drive** power button in the strip footer to enable it. The **Mode**
selector at the top of the strip chooses Soft (the original symmetric saturation),
Hard (clipping at a fixed ceiling), or Tube (asymmetric saturation). Drive applies
input gain and partial output gain compensation in every mode. Drive maps
0–100% to 0–12 dB of input gain. Tone reuses the shared centered control: 50% is
neutral, lower values darken the wet signal and higher values brighten it through
a 2.5 kHz high shelf (−12 to +12 dB). Mode changes
crossfade over 5 ms. Drive power disables Mode, Drive and Tone
while retaining their saved values. Projects without a saved mode load Soft. Older negative Drive settings load at
0%; saved positive gain values remain unchanged. Older projects load neutral Tone.
The **Options** strip groups Quality, Tail, and Color. Level follows Drive.
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
