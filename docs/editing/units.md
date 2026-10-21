# Moving and copying units

Use header dragging to arrange installed instrument components and effects. Use Option on macOS or Ctrl on Windows/Linux during a drag to copy instead of move. The modifier held at release determines the action.

## Select and remove

Click a unit to select it. Shift-click adds or removes units from the selection within the same lane. Click empty lane space to focus the lane and clear the selection. You do not need to select a unit first. With that lane focused, Command+A on macOS or Ctrl+A on Windows/Linux selects all installed units in that lane, including units outside the visible area. This works in regular layers, Output, and track or Master effects lanes. Delete or Backspace removes selected units. Right-click a header for its removal menu.

Select All changes only the unit selection. It does not select units in other lanes or change the project. In a text field, the shortcut selects its text. Unit selection shortcuts are unavailable during active gestures, dialogs, or background work.

Envelope, LFO, Macro, and Amplitude retain settings when removed; add them from the catalog to restore them. Removing Oscillator discards its sources and pitch connections. Removing Sampler discards its sample settings. Removing a component does not delete its external recording.

## Move or copy components

1. Select one or more installed units.
2. Drag a selected header to the destination layer or Output.
3. Use the insertion line to choose the position. Drop on a layer label to append.
4. Release to apply the transfer, or press Escape to cancel.

All selected units must be compatible with the destination. A rejected drop leaves the complete group unchanged. Output accepts shared Envelope, LFO, and Macro components. It does not accept Oscillator, Sampler, Amplitude, or Per note Envelopes.

A regular layer can have Oscillator or Sampler. Multiple oscillators can share an Oscillator unit; an occupied Sampler or a conflicting source type blocks a source transfer. Use Replace source in the source header menu for an explicit replacement. Only one Amplitude stage is allowed in a layer. Macro transfers merge strips into the destination Macro component.

## Reorder sources, connections, and effects

Drag unused space inside an oscillator strip to change its source order. Drag unused space in a modulation strip to reorder its connections. Parameters and connections stay attached to their source as displayed numbers change.

Drag effect headers to reorder selected effects in chain order. Modifier dragging copies the selected effects and keeps the originals. Effect copies retain their settings and source assignments.

An insertion line previews the result. Hold near a lane edge to scroll. Changes take effect on release. Escape, focus loss, or an outside drop cancels. A same-position move makes no change.

In Instrument, layer effects sit after the components in the same layer lane. Click a layer before adding effects from the catalog, or drag catalog effects after its components. Effect selection, removal, reordering, and drag duplication work within that chain. Output has its own inline effect chain. Click the bottom effects lane to target track effects.

Drag an installed effect header to another regular layer, Output, or the track effects lane to move it. Hold Option on macOS or Ctrl on Windows/Linux to duplicate it. If the effect is selected with other effects, the group transfers in its chain order. Release after the destination layer's components or at the insertion line in the track effects lane to place the group. Escape or an outside drop cancels.
