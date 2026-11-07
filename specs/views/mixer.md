# Mixer strip output destinations

For operation, see [Mixer](../../docs/views/mixer.md). Timeline’s Track mixer uses
the same strip renderer; see [View panels](view-menu.md#timeline-track-mixer).

## Accepted behavior

Every non-Master strip shows a read-only output-destination row directly below
its colored name header, before the send rack and output controls. Fill the strip’s
available width within the send rows’ horizontal margins. Use the shared strip
padding plus the send row’s `menus::ITEM_PADDING` inset on each side. Keep the
row eight points below the header and retain the standard eight-point gap below it. Use a one-point
theme border, two-point corner radius, standard control height and Small text.
Place Lucide `ArrowRight` before the destination, using the standard 16-point
artwork, a four-point gap and secondary icon color. Center the icon and visible
text together. Truncate long names and expose the complete destination in the
hover text and accessibility label.

Resolve the saved `group_id` against current project tracks each frame. Show the
immediate parent group’s name, including for nested groups. Root audio, MIDI and
beat tracks, root groups and Return tracks show `Master`. Master has no row.
Renaming or reparenting updates the label without stale references. Painting the label
does not change routing, selection, project history or playback.

`ui::mixer::mixer_output_destination` owns this shared presentation. Both Mixer
and Timeline’s Track mixer call it through `mixer_strip`. Include the row and gap
in the shared minimum strip height. Preserve alignment of faders, meters and
bottom controls across tracks, groups, Returns and Master.

## Validation

Check root and nested destinations in both views and themes, Small text, border
width and placement, ArrowRight artwork, long-name truncation, group renaming and
reparenting. Master must have no row. Painting leaves the project and dirty state
unchanged. Retain existing output-spacing and narrow-layout checks. Native
appearance and scaling require separate checks; record results in
[validation history](../validation/README.md).

## Escape selection

Mixer uses the shared view-level Escape handler. An unmodified, non-repeated
Escape clears the primary track, selected track IDs and Master selection directly,
including when other views retain selected clips. Preserve clip/range selection,
project, history and dirty state. Text editors and active gestures retain
precedence; reuse existing focus, popup, dialog and job guards. Remember Mixer
text focus across frames so cancelling a text field does not also deselect tracks.
