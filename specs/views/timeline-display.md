# Timeline display state

## Toolbar and sizes

Timeline offers a discrete three-position Track height slider: Compact (42 pt),
Regular (68 pt, default), and Large (100 pt). Place it between Fit all clips and
horizontal zoom. Place one standard divider between the two sliders and the
row-size/Fit buttons. Retain the vertical expand/collapse button as a Compact /
Regular shortcut; clicking it in Large collapses to Compact. Size controls remain
disabled during dialogs, jobs, track/name editing and active edit gestures.

Regular retains the existing name and mix rows. Compact retains icon/name and
M/S on one row with hidden gain/pan. Large uses a 22-point icon/name row, a
28-point sends row, and a 28-point mix row, separated by 4-point gaps and bounded
by the existing 7-point margins. The bottom row has M/S at the left and Pan then
Gain at the right. All tracks and nested beat rows share the height so lanes,
meters, drop previews and scrolling stay aligned. Master remains 42 points.
Editor overview headers always use Regular and have no track-size control.

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
