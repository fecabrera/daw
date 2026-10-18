# Instruments and effects

An instrument creates audio from MIDI notes. Effects change audio from a track or the project Master. Use Instrument to build a MIDI instrument, and use the Effects catalog in Timeline or Instrument to add effects.

## Instrument components

The Instrument components catalog has three folders. Each folder starts open. Click its row to hide or show its components. Folder choices remain in place while the application is open.

| Folder | Component | Use |
| --- | --- | --- |
| Sources | [Oscillator](../instruments/oscillator.md) | Generate Sine, Saw, Square, or Triangle tones. One Oscillator unit can contain several sources. |
| Sources | [Sampler](../instruments/sampler.md) | Play a selected audio region when MIDI notes arrive. |
| Modulators | [Envelope](../instruments/envelope.md) | Shape a parameter over the life of a note. |
| Modulators | [LFO](../instruments/lfo.md) | Repeat a control pattern at a fixed or tempo-related rate. |
| Modulators | [Macro](../instruments/macro.md) | Control several parameters from one knob. |
| Processors | [Amplitude](../instruments/amplitude.md) | Set a layer's note level and connect level modulation. |

Use Up/Down to move through visible folders and components. On a folder, Enter or Space hides or shows its contents; Left closes it and Right opens it. Left on a component closes its folder. Closing a folder deselects its hidden components. Shift-click selects components across open folders, and Shift+Up/Down changes the selection range. Enter on a component adds the selected components. Scroll the catalog when its rows do not fit.

## Layers

A new MIDI instrument starts with one empty layer. Add a source before expecting sound. A layer can contain Oscillator or Sampler. It cannot contain both. Add components from the Instrument catalog by double-clicking an entry, selecting it and pressing Enter, or dragging it onto a layer.

Layer controls stay in the left column; installed components appear to their right. Use the Layers Plus button to add another layer. Each layer has gain, pan, mute, and solo controls. Solo applies among the layers of that instrument. [Main](../instruments/main.md) controls their combined output and can hold shared modulators.

## Amplitude

[Amplitude](../instruments/amplitude.md) controls the level of notes in a layer. Connect an Envelope to shape attack and release, or an LFO for repeated level changes. Without an active Amplitude contour, notes use a short gate. Turning Amplitude off bypasses its level shaping while retaining note gating.

## Effects

The Effects catalog has three folders in Timeline and Instrument:

| Folder | Effects |
| --- | --- |
| EQ & Filters | [3-band EQ](../effects/equalizer.md), [Auto-filter](../effects/auto-filter.md) |
| Dynamics | [Compressor](../effects/compressor.md), [OTT](../effects/ott.md), [Limiter](../effects/limiter.md) |
| Distortion | [Drive](../effects/drive.md) |

Folders start open. Click a folder to hide or show its effects. Use Up/Down to move through visible rows. Enter or Space toggles a focused folder; Left closes it and Right opens it. Left on an effect closes its folder. Closing a folder deselects its hidden effects. Each catalog retains its own folder choices while the application is open.

Select the destination track or Master before adding an effect. Double-click an effect to add it. Shift-click selects effects across open folders; Enter on an effect adds the selection in folder order. Drag selected effects into the lane to choose their insertion point, then release to add them. Escape or an invalid drop cancels. Scroll when the catalog does not fit.

Effects run in lane order. Master effects process the combined track mix.

The header power control turns a unit on or off. Double-click its header to collapse or expand it. Off units keep their settings and remain editable. See [moving and copying units](../editing/units.md) and [modulation](../modulation/README.md).
