# Saturator

Saturator changes the shape of a signal to add harmonics and control peaks. Add it from **Distortion → Saturator** to a track, Master, instrument layer, or Output.

## Controls

| Control | Range/options | Default | Behavior |
| --- | --- | --- | --- |
| Shape | Analog Clip, Soft Sine, Bass Shaper, Medium Curve, Hard Curve | Analog Clip | Select the main saturation curve. |
| Clip Mode | No Clip, Soft Clip, Hard Clip | No Clip | Select the final clipping stage on the processed signal. |
| Drive | −36 to +36 dB | 0 dB | Raise or lower the input level before saturation. |
| Level | Exact silence to +12 dB | 0 dB | Adjust the processed level after clipping. |
| Dry/Wet | 0–100% | 100% | Blend the original and processed signals. |

Analog Clip has a linear center and rounded clipping knee. Soft Sine compresses peaks with a smooth sine curve. Bass Shaper has a broad, fixed knee for smoother bass harmonics. Medium Curve gives progressive saturation; Hard Curve has a sharper knee. Bass Shaper has no separate threshold control.

## Set the saturation

1. Choose a Shape and raise Drive until the signal has the desired distortion. Negative Drive reduces saturation; a hot signal can still saturate at 0 dB Drive.
2. Select No Clip to retain the main curve's output headroom. Soft Clip rounds peaks, while Hard Clip clamps them. No Clip still applies the selected Shape.
3. Adjust Level to balance the processed sound against the original. Drive has no automatic level compensation.
4. Reduce Dry/Wet to mix in the original signal. At 0%, the output is the original signal; Level affects only the processed path.

The clipping stage precedes Level and Dry/Wet. Raising Level can lift the output above 0 dBFS, and the dry contribution remains unclipped. Clip modes control sample peaks; they do not provide true-peak limiting. Use the [Limiter](limiter.md) when that is needed.

Header power bypasses saturation while retaining its settings. The two selectors and numeric controls change smoothly. Power and Dry/Wet changes retain time alignment with other tracks and layers.

## Modulation and editing

Drive, Level, and Dry/Wet accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Open the secondary header dot and add a Source/Destination connection. Drive amounts span ±36 dB, Level amounts ±24 dB, and Dry/Wet amounts ±100 percentage points. Amount knobs follow their destination's steps and drag behavior. Shape and Clip Mode remain discrete controls.

Use the standard unit selection, Shift-click groups, drag moves/copies, and Delete/Backspace removal. Saturator can move between the track effects lane, regular layers, and Output using the same gestures as other effects. Connections and presentation settings are saved with the project. See [modulation](../modulation/README.md) and [component editing](../editing/units.md).

## Sound and limits

The effect uses 4× oversampling to reduce aliasing. Its filters roll off the highest frequencies near the project Nyquist limit; strong distortion can still produce aliasing. Saturator uses independently tuned curves and does not reproduce Ableton's algorithms exactly. Live playback and export use the same processing.
