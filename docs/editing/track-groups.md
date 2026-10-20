# Track groups

A track group is a stereo processing bus. Its children send their output to the group instead of directly to Master. Groups can contain audio tracks, MIDI tracks, and other groups.

## Create and arrange a group

1. Select track headers. Shift-click or Command/Ctrl-click toggles selection.
2. Choose **Track > Group selected tracks**, press Command+G on macOS or Ctrl+G on Windows/Linux, or use the track context menu.
3. Use the borderless arrow beside the left-aligned group name to hide or show its children in Timeline and Mixer. Hidden tracks continue to play.

In Timeline, child track panels sit inside their group with a left margin. The group outline encloses all visible children. Their right edges line up, and the last child shares the group's bottom border. Nested groups use the same layout.

The new group starts at 0 dB, with pan centered and Mute and Solo off. It starts without effects. Children keep their clips, instruments, effects, levels, and identities. Grouping tracks that are apart brings them together in their existing order.

In Timeline, drag an audio or MIDI track's icon, name, or blank header area into the middle of a group row to add it to that group. This also works when the group is collapsed. Drag a group to move its complete subtree. Drop near a row edge to reorder. Use the left side of the track header when dropping at an edge to move to an outer group, or drop after the last row to return to Master. The insertion line shows the destination. Release to apply; Escape, focus loss, or dropping outside the track area cancels. A group cannot be moved into itself or one of its children.

## Mix and process

Select the group to use the existing gain, pan, Mute, Solo, meters, and effects lane. Add, reorder, copy, collapse, bypass, and remove group effects as with track effects. Group pan balances the stereo sum.

The signal follows this order:

```text
Child source → child effects → child gain/pan
    → group sum → group effects → group gain/pan
    → parent group, or Master effects → Master gain
```

Group effects process the combined child sound. For example, one group compressor reacts to the combined level. Meters show each track or group's output after its own gain and pan. Muting a group silences its output; its child processing and meters continue.

Solo selects siblings at each level. A soloed child opens the path through its parent groups and excludes other branches. Soloing a group includes its children, subject to their own Mute and Solo controls. Solo takes precedence over Mute on the same track or group.

Effects with latency remain aligned across children, nested groups, other tracks, and Master. Playback and export use the same routing. Changing group membership during playback can require background preparation before the change becomes audible.

A group cannot contain clips or an instrument. Add sources to its child tracks. Dragging clips onto a group bus shows an invalid-action cursor. Releasing there cancels the entire move or copy without an error dialog. You can continue dragging to a child track instead.

Group effects cannot use a child's instrument modulators as shared sources; those assignments remain scoped to their original track.

## Ungroup, duplicate, and delete

Select one group and choose **Ungroup tracks**, or press Command+Shift+G on macOS or Ctrl+Shift+G on Windows/Linux, to remove its bus and keep its children. The children move to the group's parent. The removed group's gain, pan, Mute, Solo, and effects stop affecting the sound.

Duplicate copies the group structure, instruments, and settings into new tracks. As with ordinary track duplication, the copied tracks start without clips. Delete removes the group and its complete subtree. Selecting both a group and a child does not duplicate or delete the child twice.

These edits support [undo and redo](undo-redo.md). Group membership, settings, effects, and collapsed state are saved with the project.
