# Compressor

Compressor reduces loud passages according to Threshold and Ratio. Add it to an audio/MIDI track or Master from Dynamics in the Effects catalog.

Use **Compressor** to control loud passages, shape transients and adjust the dynamic range of the full signal.

## Controls

| Control | Range | Default |
| --- | --- | --- |
| Threshold | −60–0 dBFS | −18 dBFS |
| Ratio | 1:1–20:1 | 4:1 |
| Curve | −100%–+100% | 0% |
| Detection | Peak, RMS | Peak |
| Attack | 0.1–200 ms, logarithmic | 10 ms |
| Release | 10–2000 ms, logarithmic | 100 ms |
| Knee | 0–24 dB | 6 dB |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |

Threshold is the level where compression begins. Ratio sets how strongly levels above it are reduced. A ratio of 1:1 gives no compression after any previous reduction releases. Knee softens the transition around Threshold; zero gives a hard transition.

Curve changes the response after the knee. Negative values bend the output upward exponentially and reduce compression at higher levels. Zero keeps the straight response. Positive values bend it downward logarithmically and increase compression. The knee stays continuous. The exponential response stops steepening at 1:1 and cannot amplify above the input. Ratio 1:1 remains uncompressed.

Comp contains Threshold, Ratio and Curve. Response contains Attack, Release and Knee. Out contains Level, Dry/Wet and Peak/RMS.

Peak responds to short peaks. RMS responds to average signal energy. The stereo channels receive the same reduction to retain their balance. Attack controls how quickly reduction increases; Release controls how quickly it recovers. These are response speeds, not fixed playback delays.

Dry/Wet blends original and compressed audio. Level adjusts the complete blend. Header power bypasses the effect and passes the original input without Level gain.

## Live curve

The **Curve** graph shows input level horizontally and compressed output level
vertically, in dBFS, before Dry/Wet and Level. It follows Threshold, Ratio, Knee and Curve.
The small accent dot follows the detected input on this curve. The vertical gain-reduction
meter on the graph's right shows the actual compression, including Attack and Release. The dot uses a
smooth level release so individual waveform cycles do not make it flash.

The round handle sits at the knee midpoint. With a hard knee it sits at the
corner; vertical dragging changes Ratio while the handle stays at that corner.
Drag the round handle left/right to lower/raise Threshold. Drag up to lower Ratio
and down to raise it. Command-drag (Ctrl-drag on Windows/Linux) changes only
Ratio; Option-drag (Alt-drag) changes only Threshold. Switch these modifiers
without releasing the mouse. Holding both keeps both values fixed. Hold Shift
for fine adjustment. The matching knobs follow
the preview. Release applies both values together; Escape or loss of focus cancels.
In the DAW, one Undo restores both. Audio previews use background preparation and
can follow with a delay in large projects. Standalone plugins use host gestures
for both parameters; the host controls its undo grouping.

The curve follows the manual settings. Modulation remains visible in the knobs
and actual gain-reduction reading. Compact and collapsed units hide the graph.
Unit power off hides the detector dot; the preview adds no latency. The existing Comp, Response and Out controls stay available.

## Gain-reduction meter

The GR meter sits inside Curve, to the graph's right. The plot narrows to keep
the Curve strip's width unchanged. Fill grows downward from zero at the top.
The meter shows current reduction before Dry/Wet and Level. A thin line in the secondary text color marks the highest reduction since the peak was reset. Click the meter to reset this line. The current reading and compression settings stay the same. If compression continues, new readings can immediately set another peak.

The rail covers 0–24 dB. The numeric reading and peak tooltip can report larger reductions; the peak line stops at the end of the rail. Peak capture continues while the unit is hidden or collapsed. The held peak remains during pause, stop, and seeking until you reset it or turn the unit off.

Turning the unit off clears the fill, peak line, and numeric reading and stops meter updates. Turning it on resumes monitoring. The current reading also shows zero when playback is stopped or paused, or no output device is available. Peak readings are temporary monitor state and are not saved with the project.

## Modulation and limits

Threshold, Ratio, Attack, Release, Knee, Level, Dry/Wet and Curve accept shared Envelope, LFO, and Macro sources from the same MIDI instrument. Open the secondary-color header dot for [connections](../modulation/README.md). Peak/RMS remains a manual choice. Attack and Release amounts use octaves of time: +1 doubles the duration and −1 halves it. Curve amounts use percentage points, up to ±200; the resulting Curve stays within −100%…+100%.

Lookahead, automatic makeup gain, external sidechain routing, and detector high-pass controls are not available.

## Output level meter

The DAW shows the [stereo output meter](output-meter.md) in the gap after the unit. Standalone editors keep the horizontal meter in **Out**. Click a meter to reset its held peaks. It measures the final stereo output. Its peak reset is independent of the gain-reduction meter.

This effect also has a [standalone CLAP/VST3 plugin](plugins.md).
