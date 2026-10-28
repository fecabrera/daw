# Instruments and effects

The left toolbar lists **Instrument components** above **Effects**. The Instrument components button shows a keyboard with a music note.

An instrument creates audio from MIDI notes. Effects change audio from an instrument layer, a track, or the project Master. Use Instrument to build a MIDI instrument, and use the Effects catalog in Timeline or Instrument to add effects.

## Instrument components

The Instrument components catalog has three folders. Each folder starts collapsed. Click its row to hide or show its components. Folder choices remain in place while the application is open.

| Folder | Component | Use |
| --- | --- | --- |
| Sources | [Oscillator](../instruments/oscillator.md) | Generate Sine, Saw, Square, or Triangle tones. One Oscillator unit can contain several sources. |
| Sources | [Granulizer](../instruments/granulizer.md) | Build overlapping grain textures from a selected audio region. |
| Sources | [Sampler](../instruments/sampler.md) | Play a selected audio region when MIDI notes arrive. |
| Modulators | [Envelope](../instruments/envelope.md) | Shape a parameter over the life of a note. |
| Modulators | [LFO](../instruments/lfo.md) | Repeat a control pattern at a fixed or tempo-related rate. |
| Modulators | [Macro](../instruments/macro.md) | Control several parameters from one knob. |
| Processors | [Amplitude](../instruments/amplitude.md) | Set a layer's note level and connect level modulation. |

Use Up/Down to move through visible folders and components. On a folder, Enter hides or shows its contents; Left closes it and Right opens it. Left on a component closes its folder. Closing a folder deselects its hidden components. Shift-click selects components across open folders, and Shift+Up/Down changes the selection range. Enter on a component adds the selected components. Scroll the catalog when its rows do not fit.

## Layers

With an audio track, group, Master, or no track selected, the Layers panel remains visible. Its Add layer button is disabled, and the empty layer area shows **Select a MIDI track**. Select a MIDI track to show its instrument layers and enable the button.

A new MIDI instrument starts with one empty layer. Add a source before expecting sound. A layer can contain one of Oscillator, Sampler, or Granulizer. Add components from the Instrument catalog by double-clicking an entry, selecting it and pressing Enter, or dragging it onto a layer.

Layers show a waveform icon before their names. Output uses a merge icon with its arrow pointing right. Click a regular layer header to select it. Shift-click toggles layers in the selection. Each selected layer has an outline. Output is selected separately and stays pinned at the bottom.

Right-click a regular layer header for **Rename layer**, **Duplicate layer**, and **Delete layer**. Duplicate inserts a copy immediately after that layer and selects it when ready. Delete is disabled for the last layer. Rename shows its Command+R/Ctrl+R shortcut and opens the name editor; press Enter to apply or Escape to cancel. With a regular layer selected in Instrument, Command+R on macOS or Ctrl+R on Windows/Linux opens that layer’s name editor. With several layers selected, it renames the primary layer.

Press M to toggle Mute and S to toggle Solo on each selected regular layer. With Output selected, or with no layer selected, these keys target only the displayed track. Press Escape to clear layer selection. While editing text, Escape first leaves the editor; press it again to clear layer selection. These shortcuts work from any focused pane or non-text control in Instrument. Text entry and active gestures take precedence; dialogs and blocking jobs disable the shortcuts. Release a dragged control before using M/S. Holding a key does not toggle repeatedly.

Drag a selected layer by its icon, name, or empty header space to move the selected layers together. They keep their existing order, even if you selected them in a different order. Dragging an unselected layer moves only that layer. Hold Option on macOS or Ctrl on Windows/Linux to duplicate the group at the insertion line. The modifier held at release chooses move or copy. Escape or dropping outside the layer area cancels the drag and keeps the layer selection. Output cannot be moved or duplicated.

Copies keep their names, controls, components, effects, and modulation settings. Connections within the copied group point to the copied sources. Inputs from Output or layers outside the group keep their original sources. Existing connections outside the group also keep their original sources.

Moved layers remain selected after release. If copies need sample preparation, the original layers stay selected until preparation succeeds. The copies then become selected. Failure or cancellation keeps the original layers selected. Undo restores the previous arrangement.

Layer controls stay in the left column; installed components appear to their right. Use the Layers Plus button to add another layer. Each layer has gain, pan, mute, and solo controls. Layer gain/pan and Output gain use compact knobs without text-entry fields. Output has no pan knob. Layer Mute and Solo sit at the far right of the second row beside the knobs. Expand/collapse stays on the first row. Their row heights stay unchanged. Hover or drag a knob to see its value in the tooltip. Solo applies among the layers of that instrument. [Output](../instruments/main.md) controls their combined output and can hold shared modulators.

## Amplitude

[Amplitude](../instruments/amplitude.md) controls the level of notes in a layer. Connect an Envelope to shape attack and release, or an LFO for repeated level changes. Without an active Amplitude contour, notes use a short gate. Turning Amplitude off bypasses its level shaping while retaining note gating.

## Effects

The Effects catalog has four folders in Timeline and Instrument:

| Folder | Effects |
| --- | --- |
| EQ & Filters | [3-band EQ](../effects/equalizer.md), [Auto-filter](../effects/auto-filter.md) |
| Dynamics | [Compressor](../effects/compressor.md), [OTT](../effects/ott.md), [Limiter](../effects/limiter.md) |
| Distortion | [Drive](../effects/drive.md), [Bitcrusher](../effects/bitcrusher.md) |
| Imaging | [Direction Mixer](../effects/direction-mixer.md), [Stereo Spread](../effects/stereo-spread.md) |

Folders start collapsed. Click a folder to hide or show its effects. Use Up/Down to move through visible rows. Enter toggles a focused folder; Left closes it and Right opens it. Left on an effect closes its folder. Closing a folder deselects its hidden effects. Each catalog retains its own folder choices while the application is open.

Select the destination track or Master before adding an effect. Double-click an effect to add it. Shift-click selects effects across open folders; Enter on an effect adds the selection in folder order. Drag selected effects into the lane to choose their insertion point, then release to add them. Escape or an invalid drop cancels. Scroll when the catalog does not fit.

In Instrument, click a layer or Output to select it, then add effects from the Effects catalog. New effects append to that same layer lane. Drag effect headers or catalog effects between control components, including into an empty layer. Only audio sources stay pinned first. Collapse or expand the layer to change the size of the whole row. Drag effect headers between regular layers, Output, and the track effects lane to move them; hold Option on macOS or Ctrl on Windows/Linux to duplicate them. Selected effects transfer together. Layer effect settings save with the project.

Output effects share the pinned Output lane with its control components. You can place an effect before Macro, LFO, or Envelope. Click the separate bottom effects lane to target track effects, or edit track effects in Timeline.

Layer effects process that layer's notes together, after Amplitude and before layer gain/pan. Output effects process the combined layer sum before Output gain/pan. Track output effects process the instrument after Output. This lets you distort one layer while keeping another layer clean.

Effects run in their left-to-right order. Moving a control component across an effect changes its display position; it does not change modulation timing or the fixed Amplitude processing stage. Master effects process the combined track mix.

The header power control turns a unit on or off. Right-click a header for **Enable** or **Disable** as the first menu item. On a selected unit, this action applies to all selected units in its lane. Double-click its header to collapse or expand it. Off units keep their settings and remain editable. See [moving and copying units](../editing/units.md) and [modulation](../modulation/README.md).
