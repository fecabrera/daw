# Timeline display state

Draw the playhead line from the bottom of the ruler across the arrangement
lanes and track workspace. Draw it over lane content and below the scrollbar.

During playback and recording, follow the displayed playhead in the main
Timeline. Begin scrolling when it reaches the right margin. Use the
smaller of 96 logical points and 20 percent of the viewport width. Scroll
before laying out the ruler,
lanes, clips, and recording previews. Keep the lower editor viewport
independent. Honor existing scroll bounds. Bring a wrapping loop back into
view when the playhead lands left of the viewport. Yield to pointer gestures
and manual pan or zoom. Resume after the playhead reenters the visible
area before the follow margin, or after a transport restart.

Reserve a 16-point gap after the track header column. Align the ruler,
arrangement lanes, track lanes, and horizontal scrollbar at the far edge of
the gap. Keep the Master header at the width of the track header column.
Horizontal separators below the ruler and between rows span the gap.

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
Master remains 42 pt. The lower editor retains its own layout.

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

The optional `Project.display` metadata includes `track_height` (legacy strings
compact/regular/large, or a numeric size from 1 to 3), `horizontal_zoom` (logical
points per second), `horizontal_scroll_seconds`, `vertical_scroll_points`, and
`arrangement_expanded`. Missing values default to Regular, 70 pt/s, zero offsets,
and collapsed arrangement lanes. Validation rejects nonfinite heights and
heights outside 1…3, nonfinite or non-positive zoom, and nonfinite or negative
offsets.
Fit all can produce positive zoom outside manual slider limits; retain it.

Store the preferred lower editor height in `timeline_editor_height_points`,
in logical points. The default is 300 points for new projects and old
manifests without the field. Require a finite, positive value. Capture the
last completed resize even if the editor is hidden. During a resize, capture
the height from before the drag. Clamp the displayed height to the current
window without replacing the preference. Keep panel visibility independent
of the saved height.

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
lower editor independence, send editing and overflow scrolling. Round-trip real save /
open snapshots for every size and retain legacy/default validation. Native UI
and platform checks are separate from automated egui and project checks.

## Timeline clip selection shortcut

When the Timeline clip area owns keyboard focus, Command+A on macOS or Ctrl+A
on Windows/Linux replaces the time selection with every clip in the project.
Include clips outside the viewport and in collapsed groups. Retain the primary
clip if it is still present. Text fields, other panes, active gestures and
modal dialogs retain their own shortcut handling. Selection does not change
project data or history.
