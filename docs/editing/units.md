# Moving and copying units

Use header dragging to arrange installed instrument components and effects. Use Option on macOS or Ctrl on Windows/Linux during a drag to copy instead of move. The modifier held at release determines the action.

## Unit layout

Units have a 26-point vertical title header on the left with semibold title text in both collapsed and expanded states. Double-click it to collapse or expand the unit. The power dot stays at the top of the left header. Header backgrounds stay neutral when power changes; the power dot shows the on/off state.

Expanded units have a right header with the modulation dot. Click it to show or hide connection strips. Collapsed units hide the body and right header. Units have no outer border or lines between ordinary control strips. When modulation is open, a 2-point divider runs the full unit height between the main controls and modulation area, using the unit-header background color, with 4-point clear space on each side. Control strips have their original background fill, no border and a 5-point corner radius, over a subdued unit background. Add buttons have no background. Their Plus symbol and circle brighten to the active strip-label color on hover or keyboard focus. Strips have 4-point gaps, 4-point top and bottom margins, and 4-point space beside both side headers. The side headers retain their previous neutral background. Units keep 7-point outside corner radii and a 16-point gap from adjacent units, plus 16-point space before the first unit and after the last unit in each scrollable row. The whole unit sits within 13-point top and bottom margins; lane heights stay unchanged.

Selected unit titles use the same normal text color as strip labels; unselected titles stay muted. Header backgrounds and power/modulation dot colors do not change with selection. Selection supports the same shortcuts and group actions without adding an outline to the selected unit.

## Select and remove

Click a unit to select it. Shift-click adds or removes units from the selection within the same lane. Click empty lane space to focus the lane and clear the selection. You do not need to select a unit first. With that lane focused, Command+A on macOS or Ctrl+A on Windows/Linux selects all installed units in that lane, including units outside the visible area. This works in regular layers, Output, and track or Master effects lanes. Delete or Backspace removes selected units. Right-click a header for its removal menu.

Select All changes only the unit selection. It does not select units in other lanes or change the project. In a text field, the shortcut selects its text. Unit selection shortcuts are unavailable during active gestures, dialogs, or background work.

Envelope, LFO, Macro, and Amplitude retain settings when removed; add them from the catalog to restore them. Removing Oscillator discards its sources and pitch connections. Removing Sampler discards its sample settings. Removing a component does not delete its external recording.

## Enable or disable

Right-click an installed effect or component header. The first menu item is **Disable** when every target is on, or **Enable** when any target is off. Opening a selected unit's menu applies this action to all selected units in that lane. Opening an unselected unit's menu targets only that unit.

The action keeps the units and their settings. Undo and Redo apply to the group together. The header power control still toggles one unit.

## Move or copy components

1. Select one or more installed units.
2. Drag a selected header to the compatible destination lane.
3. Use the insertion line to choose the position. Drop on a layer label to append.
4. Release to apply the transfer, or press Escape to cancel.

All selected units must be compatible with the destination. A rejected drop leaves the complete group unchanged. Output accepts Macro components. It does not accept audio sources or Amplitude. LFO and Envelope units stay in their dedicated [modulation lanes](../instruments/modulation-lanes.md). Reorder or copy them only within the matching lane type; regular layers and Output do not accept them.

A regular layer can have one source type: Oscillator, Sampler, Granulizer, or Noise. Multiple oscillators can share an Oscillator unit; an occupied sample source or a conflicting source type blocks a source transfer. Use Replace source in the source header menu for an explicit replacement. Only one Amplitude stage is allowed in a layer. Macro transfers merge strips into the destination Macro component.

## Reorder sources, connections, and effects

Drag unused space inside an oscillator strip to change its source order. Drag unused space in a modulation strip to reorder its connections. Parameters and connections stay attached to their source as displayed numbers change.

Drag effect headers to reorder selected effects in chain order. Modifier dragging copies the selected effects and keeps the originals. Effect copies retain their settings and source assignments.

An insertion line previews the result. Hold near a lane edge to scroll. Changes take effect on release. Escape, focus loss, or an outside drop cancels. A same-position move makes no change.

In Instrument, effects share the regular layer or Output lane with control components. Click a lane before adding effects from the catalog, or drag catalog effects to an insertion line between control components, including before Macro. Audio sources stay pinned first. Effect selection, removal, reordering, and drag duplication work within that chain. Output has its own inline effect chain. Click the bottom effects lane to target track effects.

Drag an installed effect header to another regular layer, Output, or the track effects lane to move it. Hold Option on macOS or Ctrl on Windows/Linux to duplicate it. If the effect is selected with other effects, the group transfers in its chain order. Release at the insertion line in the destination lane to place the group. Escape or an outside drop cancels.

Effects process in their serial chain order. Moving control components across effects changes their display position, not source processing, the fixed Amplitude stage, or modulation timing.
