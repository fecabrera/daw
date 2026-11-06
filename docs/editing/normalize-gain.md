# Normalize clip gain

Normalize audio clips to a chosen peak or loudness level. Normalization changes
clip Gain before track effects. It does not change the source file, track Gain,
Master Gain, clip timing, or mute state.

1. Select one or more audio clips in Timeline or Editor.
2. Right-click a selected clip and choose **Normalize gain**. You can also use the
   **Clip** menu. Right-clicking an unselected clip targets only that clip.
3. Choose a measurement and target:

   | Measurement | Target unit | Default target |
   | --- | --- | --- |
   | Sample peak | dBFS: highest stored sample across both channels | −1 dBFS |
   | True peak | dBTP: estimated peak between samples | −1 dBTP |
   | Integrated loudness | LUFS: gated loudness over the clip | −23 LUFS |

4. Set **Target** between −70 and 0 in the selected unit. Leave **Allow gain
   increase** enabled to raise quiet clips. Clear it to allow gain reductions only,
   relative to each clip’s current Gain.
5. Click **Normalize**. The options dialog closes. The status bar shows progress
   and **Cancel**. No completion dialog opens. An error opens the standard error
   dialog.

Each clip reaches the chosen target independently. Both channels receive the
same gain change. The measurement uses only the clip’s arranged source section,
including its trim, loop phase, stretch, and transpose. It excludes existing clip
Gain, boundary fades, track and Master processing, and effects. Audio outside the
clip’s source section does not affect the result. A selection containing MIDI or
beat clips cannot be normalized.

Silent clips stay unchanged. Loudness mode needs at least 400 ms and measurable
audio above its gate; use a peak mode for shorter or very quiet clips. Loudness
normalization changes gain only; it does not limit peaks. Clip Gain supports
silence through +60 dB. An unreachable target, missing audio, or another failure
leaves every selected clip unchanged. Cancel also retains all original settings.
Undo restores the selected clips’ previous gains together.
