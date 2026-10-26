# Mixer

Mixer controls track levels and the final project output. It uses the same track settings as Timeline, so edits in either view apply to playback and export.

## Track controls

Audio track headers show a waveform icon; MIDI track headers show a keyboard icon. Master shows a boom box icon. Track names align left after the icon, matching group headers.

Gain changes level. Pan positions mono audio and balances stereo audio. Use numeric fields for a precise value or drag the knobs. Shift gives finer adjustment. Double-click a knob to restore its default. Gain accepts a numeric dB value or `−∞` for exact silence. Very low finite values keep a numeric readout.

Mute silences a track. Solo plays the selected soloed tracks and excludes the others. Solo takes priority over mute: a track with both enabled plays. Select several tracks to apply Mute or Solo together from the Track menu or with M and S.

Meters show current output. Held peak readouts help identify the highest recent level; click a peak readout to reset it. Reduce gain if the project output clips.

## Track groups

[Track groups](../editing/track-groups.md) have the same controls and meters as tracks. Select a group to edit its shared effects. Group and Master labels align left. Its borderless arrow hides or shows child strips; hidden children keep playing. Create or ungroup through the Track menu or context menu. Use Timeline to drag tracks between groups.

## Master

Master stays at the right while tracks scroll. It controls the combined track mix. Master effects run before Master gain. Any gain after a Limiter can raise its output above the limiter ceiling.

The project Master is separate from [Instrument Output](../instruments/main.md), which controls the layers of one MIDI instrument. See [instruments and effects](instrument.md) for effect placement.
