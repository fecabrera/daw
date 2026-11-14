# Timeline display state

## Toolbar and sizes

Timeline offers a continuous Track height slider from 1 to 3 with 0.01 steps.
Inner height is `28 * size` points, plus the existing 7-point top/bottom padding.
Whole sizes retain 28/56/84-point inner heights and 42/70/98-point allocated
heights; size 2 remains the default. Show the decimal value in the tooltip and
accessibility. Pointer input snaps within 0.06 of each whole number. Apply snapping in the slider value setter before painting, so
the handle stays at the integer throughout the snap zone. Keyboard input keeps
its 0.01 steps without snapping, so it can leave an integer.
Place the slider between Fit all clips and horizontal zoom. Place one standard
divider between the two sliders and the row-size/Fit buttons. The expand/collapse
button sets size 2 below 2, or size 1 otherwise. Size controls remain disabled
during dialogs, jobs, track/name editing and active edit gestures.

Controls use `floor(size)` for their layout. Below 2, center the one-row name
and M/S layout in the available height, with gain/pan hidden. From 2 to below 3,
put icon/name in the top slot and mixing controls in the bottom slot; fractional
height becomes space between them. At 3, insert the sends slot
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

The optional `Project.display` metadata contains `track_height` (legacy strings
compact/regular/large, or a numeric size from 1 to 3), `horizontal_zoom` (logical
points per second),
`horizontal_scroll_seconds`, and `vertical_scroll_points` for Timeline. Reject
nonfinite heights and values outside 1…3 during project validation. Missing
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
