# Timeline display state

## Toolbar and sizes

Timeline offers a discrete three-position Track height slider with content heights
Compact 28 pt, Regular 56 pt (default), and Large 84 pt. With the existing 7-point
top/bottom padding, allocated row heights are 42, 70 and 98 pt. Place it between Fit all clips and
horizontal zoom. Place one standard divider between the two sliders and the
row-size/Fit buttons. Retain the vertical expand/collapse button as a Compact /
Regular shortcut; clicking it in Large collapses to Compact. Size controls remain
disabled during dialogs, jobs, track/name editing and active edit gestures.

Timeline content uses one, two or three contiguous 28-point slots. Compact puts
icon/name and M/S on one slot, with gain/pan hidden. Regular puts icon/name on
the first slot and mixing controls on the second. Large inserts the sends slot
between them. Center 22-point name/buttons and compact knob faces in these slots;
keep standard control sizes and the 7-point outer padding. The bottom slot has
M/S at the left and Pan then Gain at the right. All project tracks and nested beat
rows use the selected height so lanes, meters, previews and scrolling align.
Master remains 42 pt. Editor overview headers retain their independent 68-point
layout and have no track-size control. No unit or instrument header changes.

Each Timeline row uses one one-point bottom separator, inside the bottom padding
and outside the content slots. Do not add a top separator based on row index or
another bottom separator only for the final row. First, middle and last rows have
equal content and visible surface heights. Resolve the separator inset from the
following visible row’s depth, leaving expanded group/beat gutters open and
closing them at the next sibling/ancestor. Apply the same policy to virtualized
rows and clip preview restoration. The final Return shares the pinned Master’s
boundary rather than adding a coincident line. Keep the outer header width,
colors, hit areas, settings, project audio and undo behavior unchanged.

Render every installed send using the shared 22-point header-knob face in a
28-point slot. Disabled sends remain visible but disabled. Reuse send gain
mapping, silence reset, fine adjustment and capture behavior; identify the
Return in its tooltip. Overflow scrolls horizontally within the middle row;
do not wrap or grow the header. Commit changes with `Edit::SetTrackSends` and
retain send identities, routing, enable flags and other levels. Beat subtracks,
Returns and tracks without sends retain an empty middle row.

## Project persistence

The optional `Project.display` metadata contains `track_height` (compact,
regular, large), `horizontal_zoom` (logical points per second),
`horizontal_scroll_seconds`, and `vertical_scroll_points` for Timeline. Missing
metadata or missing fields uses Regular, 70 pt/s and zero offsets. Reject
non-finite/non-positive zoom and non-finite/negative offsets on validation.
Fit all can produce positive zoom outside manual slider limits; retain it.

Capture current display state in regular Save, blocking Save, Save As and
Collect project sources snapshots. Collection carries save-only presentation
separately from the immutable model/history fingerprint; keep stale-job checks
unchanged and apply the written display metadata only after confirmed publication. Restore it after successful project installation; apply vertical
offset to the main track scroll area once, then record actual clamped scrolling.
New projects reset to defaults. Failed loads retain the current display. These
are presentation values: they do not alter audio, mark an edit, or enter Undo.
Save records them even when there are no audio/model changes. Do not restore
presentation metadata on audio preparation or Undo/Redo. Exclude it from
history checkpoint comparison, as with the navigation-only playhead.

## Checks

Cover all three slider positions, toolbar order, modal/drag guards, row geometry,
Editor independence, send editing and overflow scrolling. Round-trip real save /
open snapshots for every size and retain legacy/default validation. Native UI
and platform checks are separate from automated egui and project checks.
