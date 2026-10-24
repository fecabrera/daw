# Drive v2

Drive v2 adds separate input gain, a Color power button, Dry/Wet, and Soft/Hard clipping. Add it from Distortion in the Effects catalog. The original Drive remains available and existing projects keep it.

## Controls

| Strip | Control | Range or choices | Default |
| --- | --- | --- | --- |
| Drive | Clip mode | Soft, Hard | Soft |
| Drive | Drive Gain | 0–100% | 20% |
| Drive | Drive Tone | 0–100% | 50% |
| Color | Power | Off/On | Off |
| Color | Shape | Mid scoop, Mid hump, Mid hump + bass cut | Mid scoop |
| Color | Strength | 0–100% | 50% |
| Color | Frequency | 250 Hz–2 kHz | 750 Hz |
| Output | Input Gain | Silence to +12 dB | 0 dB |
| Output | Output Level | Silence to +12 dB | 0 dB |
| Output | Dry/Wet | 0–100% wet | 100% |

Input Gain feeds both dry and processed audio. The processed path runs through Color, clipping, then Drive Tone. Dry/Wet mixes that result with the input-gained dry signal. Output Level controls the complete mix. At 0% wet, Input Gain and Output Level still apply. The header power bypasses the whole effect and returns the original input.

Drive Gain retains the original intensity mapping: zero skips distortion; maximum gives approximately 36 dB of drive. Soft retains the original normalized tanh curve. With Input Gain at 0 dB, Dry/Wet at 100%, and matching Color, Gain, Tone, and output level, it reproduces Drive sample for sample. The bounded two-way Clip mode knob selects Soft or Hard, with the active label highlighted. Hard selects a normalized hard-clipping curve at the same distortion stage, before Tone. Clip does not add a final output limiter.

Color retains the original pre-clipping responses and frequency mapping. Its separate power button leaves the selected shape and values saved while disabling its controls. The three-way shape knob uses response diagrams. Drive Tone retains the 2.5 kHz high shelf, spanning −12 to +12 dB with a neutral center.

## Modulation and limits

Input Gain, Drive Gain, Drive Tone, Color Strength, Color Frequency, Output Level, and Dry/Wet accept shared Envelope, LFO, and Macro sources in the same MIDI instrument. Use the standard header dot and connections. Input/Output amounts span ±24 dB; Gain/Tone/Strength/Dry-Wet span ±100 percentage points; Color Frequency spans ±3 octaves. Base silence stays silent. Color power, Shape, and Clip remain manual controls.

Controls, power, and Clip changes ramp smoothly. Connections, settings, and panel presentation save per effect. Playback and export share processing. Like Drive, this effect runs at the project rate without oversampling and adds no latency. Strong distortion can alias, especially with high-frequency input. See [Drive](drive.md) for the shared color and tone behavior.
