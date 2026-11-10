# Mixer

Selected tracks use a highlighted strip background without a selection outline.

Mixer controls track levels and the final project output. It uses the same track settings as Timeline, so edits in either view apply to playback and export.

The Return and Master toggles stay at the bottom of the upper right toolbar.
Opening the effects lane places them just above its toolbar. Closing the lane
returns them to the bottom of the full-height toolbar.

## Effects toolbar

Use the left **Effects** button to show the effects catalog and the bottom **Effects lane** button to show the selected track’s or Master’s effect chain. These panels start closed and remember their visibility independently of Editor and Timeline. Mixer has no Clip settings, Chords or Instrument components button.

Select a track, group or Master to change the effect target. Add effects from the catalog and use the same unit controls, reordering and compact mode as Timeline. The lane sits below the mixer; Master stays pinned at the right of the mixer area when shown. Toggling panels changes only the workspace layout.

## Track controls

Audio track headers show a waveform icon; MIDI track headers show a keyboard icon. Master shows a boom box icon. Each track and group header centers its icon and semibold name together. All track, group, Return and Master headers use the same light text color for names and icons. Selected strips use the selection background. Master’s icon matches its label. The icon-to-label gap matches the vertical padding around the icon. Master uses the same centered layout and semibold text, with the neutral unit-header background color.

Nested tracks have a colored gutter above their headers. Each gutter connects to its parent header and uses that parent’s color. Deeper groups add another gutter, up to three visible nesting levels. Header sizes stay the same, and the lower output controls remain aligned.

Each non-Master strip shows its output destination just below its header, with a right-arrow icon and a bordered label. Its horizontal and vertical spacing matches the send controls. A track or nested group shows its immediate parent group’s name. Root tracks, root groups and Returns show **Master**. The same labels appear in Timeline’s Track mixer. These labels are read-only.

Output controls have equal visible vertical gaps and outer padding. They stay aligned across tracks, Returns and Master. Gain uses a vertical fader on tracks, groups and Master. Drag it from exact silence (−∞) to +12 dB. Hold Shift for finer adjustment, or double-click the fader to reset it to 0 dB. Pan uses a knob on tracks and groups to position mono audio or balance stereo audio. Master has no Pan knob.

Use numeric fields for precise values. Press Enter or leave the field to apply a valid value; Escape cancels the draft. Gain accepts a finite dB value or `−∞` for exact silence. Track and group numeric gain can exceed the fader's travel range; Master input clamps to silence through +12 dB. Very low finite values keep a numeric readout. Pan accepts −1 (left) through +1 (right), with 0 at center.

Mute and Solo use one joined **M / S** button group. Each button toggles independently.

Mute silences a track’s direct output; its sends remain active. Solo plays the selected soloed tracks and excludes the others. Solo takes priority over mute: a track with both enabled plays. Select several tracks to apply Mute or Solo together from the Track menu or with M and S.

Press Escape to deselect tracks, groups and Master. Active edits and drags cancel first. Clip selection in other views stays unchanged.

Track shortcuts work from other Mixer panes and non-text controls: Mute (M), Solo (S), Rename (Command/Ctrl+R), Group (Command/Ctrl+G), and Ungroup (Command/Ctrl+Shift+G). Text entry, active gestures, dialogs and blocking jobs retain their guards. Selection requirements still apply.

Meters show current output. Each L/R bar has a held-peak marker that stays at its highest level until reset. The peak readout shows the higher of the two held levels. Click either bar, the gap between L and R, or the peak readout to reset both held peaks and the clipping warning. Reduce gain if the project output clips.

## Track groups

[Track groups](../editing/track-groups.md) have the same controls and meters as tracks. Select a group to edit its shared effects. Group and Master center their icons and labels together. Its borderless arrow hides or shows child strips; hidden children keep playing. Create or ungroup through the Track menu or context menu. Use Timeline to drag tracks between groups.

## Sends and Returns

Send rows always appear above Pan, with a grouped power button and Return dropdown on the left and a small level knob on the right. Send stacks align at the bottom of their section, with **+ Send** aligned above the output divider. The stack grows upward with its rows; scroll Mixer vertically if needed. Add a send to an existing Return or create an empty Return with **+ Send > New Return**. Use the **Return tracks** arrow icon at the bottom of the right toolbar to show or hide the Return strips. It starts enabled and remembers your choice while you switch views. Hidden Returns keep processing audio. Use the **Master track** boom box icon after the Return toggle to show or hide Master. Both toggles start enabled, remember their state across view switches, and are also available in the View menu. Hidden strips keep processing audio. Returns stay at the right, immediately left of Master when it is shown. Their bank expands to fit all Returns when space permits, with standard strip widths and independent scrolling for overflow. Master stays pinned and source tracks retain a usable viewport. See [track sends and Returns](../editing/track-sends.md) for routing, defaults and Solo behavior.

## Master

Master stays at the right while tracks scroll, unless you hide it with its toolbar toggle. It controls the combined track mix. Master effects run before Master gain. Any gain after a Limiter can raise its output above the limiter ceiling.

The project Master is separate from [Instrument Output](../instruments/main.md), which controls the layers of one MIDI instrument. See [instruments and effects](instrument.md) for effect placement.
