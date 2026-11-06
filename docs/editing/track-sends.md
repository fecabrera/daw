# Track sends and Returns

Use a send to share one effect chain across several tracks or to blend parallel processing with the direct signal. A Return sums its sends, applies its effects and mix controls, then feeds Master.

## Create and connect a Return

1. Open Mixer. Send racks appear above Pan.
2. On the source track, click **+ Send**. Choose an existing Return or **New Return**.
3. Select the Return and add effects from the Effects catalog. For shared reverb or delay, set the effect's Dry/Wet control to fully wet if you want only the effect on the Return.
4. Raise the send level from silence to the required amount. Adjust the Return fader to balance the shared output.

You can also create an empty Return with **Track > Add Return**. Projects start without Returns. Returns have no presets or preinstalled effects. Adding an effect keeps that effect's usual defaults.

Audio, MIDI, beat parent tracks and groups can send. Each source can have one send to each Return. Master, Returns, instrument layers and beat subtracks have no outgoing sends.

## Send controls

Each send is a row with a power button and Return dropdown joined in one input group on the left, and a small level knob on the right. Use the dropdown to change its destination; Returns already used by another send on the same source are unavailable. Hover the knob to read its level. A divider separates the send rack from Pan and the output controls. Send stacks align at the bottom, with **+ Send** at the same height across source tracks. All send rows stay in the stack; scroll Mixer vertically if the complete controls do not fit the window.

| Control | Behavior |
| --- | --- |
| Level | Exact silence (−∞) through +12 dB. New sends start at silence. Double-click resets to silence. |
| Power | Stops this feed while retaining its settings. The Return's existing effect tail continues. |
| Pre/Post (context menu) | Post is the default and follows the source fader. Pre ignores the source fader, including when it is at silence. Both follow source effects and Pan. |
| Context menu | Enable/disable the send, select Pre/Post, change its Return, or remove it. |

Mute silences the source's direct output and keeps its sends active. Soloing a source suppresses other sources' sends. Several source solos retain the union of the selected branches. Solo still takes priority over Mute on the same track.

A child's send bypasses its parent group's effects and mix controls. Send from the group to share the processed group sum. Muting a group leaves its own sends and child sends active. Muting a child removes that child from the group sum, but keeps the child's own sends active.

## Return controls and layout

Returns use the normal effects lane, gain, Pan, Mute, Solo and output meters. Click a Return header to select it. Its outline marks the selected track, as for other tracks and Master. Mute silences a Return's output while its effects continue to run. Solo a Return to hear only its wet output; its contributors keep running, including muted sources. With both source and Return solos, source solos restrict the contributors and Return solos restrict the Return outputs. Selected sources retain their direct output.

Returns stay above Master in Timeline and beside Master in Mixer. Use **Returns** at the right of Mixer’s transport toolbar to show or hide Return strips; hiding them keeps their audio processing active. In Mixer, the Return bank expands to show all Returns when space permits, keeping standard strip widths. Overflow Returns scroll independently, with Master pinned and space reserved for source tracks. In Timeline, their bank scrolls independently; drag its top boundary to resize it. Drag Return headers to reorder Returns within the bank. Master stays outside the scrolling bank.

Rename and duplicate Returns through the normal Track menu. Duplicating a source copies its sends to the same Returns. Duplicating a Return copies its effects and mix settings without adding incoming sends. Deleting a Return also removes its incoming sends. Undo restores the Return and those sends together.

Save/open, Undo/Redo and export retain send routing and settings. Return-to-Return sends, feedback, sidechains, send automation and separate send Pan are not available.
