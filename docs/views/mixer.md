# Mixer

Mixer controls track levels and the final project output. It uses the same track settings as Timeline, so edits in either view apply to playback and export.

## Effects toolbar

Use the left **Effects** button to show the effects catalog and the bottom **Effects lane** button to show the selected track’s or Master’s effect chain. These panels start closed and remember their visibility independently of Editor and Timeline. Mixer has no Clip settings, Chords or Instrument components button.

Select a track, group or Master to change the effect target. Add effects from the catalog and use the same unit controls, reordering and compact mode as Timeline. The lane sits below the mixer; Master stays pinned at the right of the mixer area. Toggling panels changes only the workspace layout.

## Track controls

Audio track headers show a waveform icon; MIDI track headers show a keyboard icon. Master shows a boom box icon. Each track and group header centers its icon and name together. Icons use the same contrasting text color as their header names. Master’s icon matches its label. The icon-to-label gap matches the vertical padding around the icon. Master uses the same centered layout.

Output controls have equal visible vertical gaps and outer padding. They stay aligned across tracks, Returns and Master. Gain uses a vertical fader on tracks, groups and Master. Drag it from exact silence (−∞) to +12 dB. Hold Shift for finer adjustment, or double-click the fader to reset it to 0 dB. Pan uses a knob on tracks and groups to position mono audio or balance stereo audio. Master has no Pan knob.

Use numeric fields for precise values. Press Enter or leave the field to apply a valid value; Escape cancels the draft. Gain accepts a finite dB value or `−∞` for exact silence. Track and group numeric gain can exceed the fader's travel range; Master input clamps to silence through +12 dB. Very low finite values keep a numeric readout. Pan accepts −1 (left) through +1 (right), with 0 at center.

Mute silences a track’s direct output; its sends remain active. Solo plays the selected soloed tracks and excludes the others. Solo takes priority over mute: a track with both enabled plays. Select several tracks to apply Mute or Solo together from the Track menu or with M and S.

Meters show current output. Held peak readouts help identify the highest recent level; click a peak readout to reset it. Reduce gain if the project output clips.

## Track groups

[Track groups](../editing/track-groups.md) have the same controls and meters as tracks. Select a group to edit its shared effects. Group and Master center their icons and labels together. Its borderless arrow hides or shows child strips; hidden children keep playing. Create or ungroup through the Track menu or context menu. Use Timeline to drag tracks between groups.

## Sends and Returns

Send rows always appear above Pan, with a grouped power button and Return dropdown on the left and a small level knob on the right. The stack grows with its rows; scroll Mixer vertically if needed. Add a send to an existing Return or create an empty Return with **+ Send > New Return**. Returns stay immediately left of Master in their own scrolling bank. See [track sends and Returns](../editing/track-sends.md) for routing, defaults and Solo behavior.

## Master

Master stays at the right while tracks scroll. It controls the combined track mix. Master effects run before Master gain. Any gain after a Limiter can raise its output above the limiter ceiling.

The project Master is separate from [Instrument Output](../instruments/main.md), which controls the layers of one MIDI instrument. See [instruments and effects](instrument.md) for effect placement.
