# Instruments and effects

MIDI tracks have an **R** button in its own group after M/S on the same row. Enable several tracks to receive live MIDI together. With none enabled, input follows the selected MIDI track; its R text shows the active color on the regular background. See [live MIDI](../instruments/live-midi.md) and [recording](../editing/midi-recording.md).

Use the left toolbar to switch between **Instrument components** and **Effects**. Instrument components opens by default. Its button shows a keyboard with a music note.
The bottom **Effects lane** toggle shares its open or closed state with the other views.

An instrument creates audio from MIDI notes. Effects change audio from an instrument layer, a track, or the project Master. Use Instrument to build a MIDI instrument, and use the Effects catalog in Timeline, Instrument or Mixer to add effects.

## Project tracks

The track column in Instrument uses the full Timeline track headers. Select a track or beat subtrack there to open its instrument or effects. Groups and beat subtracks appear indented. The track column stays available when you open either catalog.

Click a group chevron to expand or collapse it. Click the name to select the group. Timeline and Instrument use the same disclosure state.

Click an audio, MIDI or beat track in **Tracks** to select it without leaving Instrument. MIDI tracks show their instrument layers. Audio tracks, groups, and beat parents use the Effects Lane at the bottom. The existing clip selection stays in place. Expand a beat parent in Tracks and select a subtrack to open its [Layers page](beat-editor.md#instrument-layers-and-output) and its Effects Lane.

Use **M**, **S**, gain, and pan to adjust each track. Use the header Plus menu, or right-click blank space in Tracks, to add audio, MIDI or beat tracks or an empty group without changing view. With a beat selected, the Plus menu also offers **Add subtrack**. Right-click a beat parent and choose **Add subtrack** to add directly to that beat. The Track menubar also includes **Add group** and **Add subtrack**; Add subtrack requires a selected beat.

The Add buttons in Tracks, Layers and Beat subtracks share the rounded, borderless style of M/S buttons.

Right-click a group to add an audio, MIDI or beat track or a nested empty group inside it.

Double-click a track or group name, or right-click its row and choose **Rename**. The whole name starts selected. Press Enter to save; Escape or moving focus elsewhere cancels.

Drag a track's name or icon to reorder it. Drop in the middle of a group to add it; drop near a row edge to reorder. Drop at the left edge to move to an outer group, or below the last row to return to Master. A group moves with its children. Subtracks can move only within their original beat track. Release to apply; Escape, focus loss or an outside drop cancels. Controls keep their own gestures.

Scroll the track column when the list does not fit. Timeline and Instrument
share the vertical scroll position. Master stays at the bottom.

Use **Track Mixer** at the top of the right toolbar or in **View** to show the selected track and its output bus. It starts closed and remembers its visibility separately from Timeline.

Use the vertical slider or the compact-row button in the transport toolbar to
change track height. Timeline and Instrument share this setting. The
horizontal slider and Fit All Clips are available in Timeline.

## Instrument tabs

**All** is the first tab in the layer group. Its workspace is empty. Click a layer tab to show its sources and processors, its Effects row, and its mix controls in that order. Layer tabs show numbers for default names such as Layer 1 and show custom names in full. The first layer opens by default. Scroll the layer group horizontally when its tabs do not fit. The Add layer button stays visible at the right edge.

**LFOs**, **Envelopes**, and **Macros** stay visible beside the layer tabs. Each opens its own controls. The Macros tab shows shared Output Macros and any layer-owned Macros. All layers continue to process audio and modulation as you switch tabs.

## Instrument components

The Instrument components catalog shows Sources and Processors on a layer tab. A modulation tab shows its own component type in the Modulators folder. Click its row to hide or show its components. Folder choices remain in place while the application is open.

| Folder | Component | Use |
| --- | --- | --- |
| Sources | [Oscillator](../instruments/oscillator.md) | Generate Sine, Saw, Square, or Triangle tones. One Oscillator unit can contain several sources. |
| Sources | [Granulizer](../instruments/granulizer.md) | Build overlapping grain textures from a selected audio region. |
| Sources | [Sampler](../instruments/sampler.md) | Play a selected audio region when MIDI notes arrive. |
| Sources | [Noise](../instruments/noise.md) | Generate White, Pink, or Brown noise. |
| Modulators | [Envelope](../instruments/envelope.md) | Shape a parameter over the life of a note. |
| Modulators | [LFO](../instruments/lfo.md) | Repeat a control pattern at a fixed or tempo-related rate. |
| Modulators | [Macro](../instruments/macro.md) | Control several parameters from one knob. |
| Processors | [Amplitude](../instruments/amplitude.md) | Set a layer's note level and connect level modulation. |

Use Up/Down to move through visible folders and components. On a folder, Enter hides or shows its contents; Left closes it and Right opens it. Left on a component closes its folder. Closing a folder deselects its hidden components. Shift-click selects components across open folders, and Shift+Up/Down changes the selection range. Enter on a component adds the selected components. Scroll the catalog when its rows do not fit.

## Layers

With an audio track, group, Master, or no track selected, the instrument workspace shows **Select an instrument track**. Select a MIDI track or beat subtrack to show its layer tabs. Select a MIDI track or beat subtrack to edit its instrument layers.

A new MIDI track starts with a sine-wave Oscillator in Layer 1. Play MIDI notes or add them to a pattern to hear it. A layer can contain one of Oscillator, Sampler, Granulizer, or Noise. Add components from the Instrument catalog by double-clicking an entry, selecting it and pressing Enter, or dragging it onto a compatible lane.

Layer names use the same semibold text as track headers. Layers show a waveform icon before their names. Output uses a merge icon with its arrow pointing right. Click a layer tab to select it. Shift-click layer tabs to change the selection. The [LFO and Envelope lanes](../instruments/modulation-lanes.md) have their own tabs. Each modulation lane accepts only its own provider type and has no audio mix controls. Click Output to select its controls.

Right-click a layer tab for **Duplicate**, **Mute**, **Solo**, or **Delete**. These actions apply to the clicked layer. Press **Cmd+D** on macOS or **Ctrl+D** on Windows and Linux to duplicate the selected layer.

Right-click a regular layer header for **Rename layer**, **Duplicate layer**, and **Delete layer**. Duplicate inserts a copy immediately after that layer and selects it when ready. Delete is disabled for the last layer. Rename shows its Command+R/Ctrl+R shortcut and opens the name editor; press Enter to apply or Escape to cancel. With a regular layer selected in Instrument, Command+R on macOS or Ctrl+R on Windows/Linux opens that layer’s name editor. With several layers selected, it renames the primary layer. Track, subtrack and layer names use the same text input, with the whole name selected when editing starts. Moving focus away cancels the rename.

Press **Cmd+Delete** on macOS or **Ctrl+Delete** on Windows and Linux to delete
the selected regular layers in one undoable edit. Keep at least one regular
layer unselected. Output and the modulation lanes cannot be deleted.

Press M to toggle Mute and S to toggle Solo on each selected regular layer. With Output or a modulation lane selected, or with no layer selected, these keys target only the displayed track. Press Escape to clear layer selection. While editing text, Escape first leaves the editor; press it again to clear layer selection. These shortcuts work from any focused pane or non-text control in Instrument. Text entry and active gestures take precedence; dialogs and blocking jobs disable the shortcuts. Release a dragged control before using M/S. Holding a key does not toggle repeatedly.

Drag a selected layer tab to move the selected layers together. They keep their existing order, even if you selected them in a different order. Dragging an unselected layer moves only that layer. Hold Option on macOS or Ctrl on Windows/Linux to duplicate the group at the insertion line. The modifier held at release chooses move or copy. Escape or dropping outside the layer area cancels the drag and keeps the layer selection. Output and the two modulation lanes cannot be moved or duplicated.

Copies keep their names, controls, components, effects, and modulation settings. Connections within the copied group point to the copied sources. Inputs from Output or layers outside the group keep their original sources. Existing connections outside the group also keep their original sources.

Moved layers remain selected after release. If copies need sample preparation, the original layers stay selected until preparation succeeds. The copies then become selected. Failure or cancellation keeps the original layers selected. Undo restores the previous arrangement.

Selected layers use the same header and lane shades as selected Timeline tracks
in Dark, Light and Mist. This also applies to Output, LFO and Envelope. Each
selected regular layer is highlighted when several layers are selected.

The selected layer’s mix controls sit in the left column below its Effects row. Use the Plus button at the end of the layer tab group to add another layer.

### Add or replace a source

Adding a source from the component catalog replaces the selected layer's source.
Adding Oscillator to an existing Oscillator appends a strip. Other layer settings,
effects and modulators remain. Drop an audio file on a regular layer to install
Sampler with that file; this also replaces Oscillator or Granulizer. These rules
apply to MIDI instruments and beat subtracks. Output and modulation lanes do not
accept source or file drops. Failed or cancelled preparation retains the layer.

### Layer and Output levels

Each layer has gain, pan, mute, and solo controls. Mute and Solo share the same joined M / S button group as tracks, with independent toggles. Layer gain/pan and Output gain use compact knobs without text-entry fields. Output has no pan knob. On the second row, layer Mute and Solo sit at the left; Pan then Gain sit at the right. Output’s single gain knob also sits at the right. Regular layers and Output have the same stereo level meters as tracks, beside the gain knob. Each layer meter shows its signal after effects, gain, pan and layer mute/solo. Output shows their combined signal after its gain and pan, before the bottom Effects Lane. Either channel exceeding 0 dBFS lights both clipping indicators; click either rail to clear them. Meters do not change sound or project data. Each layer, including Output, has an expand/collapse button at the top of its own right toolbar. The toolbar stays at the right edge while components scroll horizontally. Hover or drag a knob to see its value in the tooltip. Solo applies among the layers of that instrument. [Output](../instruments/main.md) controls their combined output and can hold Macro controls.

Layer and Output meters show the shared held-peak marker for each stereo
channel. Peaks remain visible when the signal becomes silent. Click either
rail or the gap between them to reset both held peaks and the clipping warning.
This changes monitoring only.

## Amplitude

[Amplitude](../instruments/amplitude.md) controls the level of notes in a layer. Connect an Envelope to shape attack and release, or an LFO for repeated level changes. Without an active Amplitude contour, notes use a short gate. Turning Amplitude off bypasses its level shaping while retaining note gating.

## Effects

The Effects catalog has the same seven folders in every view:

| Folder | Effects |
| --- | --- |
| EQ | [3-band EQ](../effects/equalizer.md), [Multiband EQ](../effects/multiband-eq.md) |
| Filters | [Filter](../effects/filter.md), [Auto-filter](../effects/auto-filter.md), [Chorus](../effects/chorus.md) |
| Dynamics | [Compressor](../effects/compressor.md), [OTT](../effects/ott.md), [Limiter](../effects/limiter.md), [Multiband Compressor](../effects/multiband-compressor.md), [Transient Shaper](../effects/transient-shaper.md), [Expander](../effects/expander.md) |
| Distortion | [Drive](../effects/drive.md), [Drive v2](../effects/drive-v2.md), [Bitcrusher](../effects/bitcrusher.md), [Saturator](../effects/saturator.md), [Multiband Exciter](../effects/exciter.md), [Harmonic Lift](../effects/harmonic-lift.md) |
| Imaging | [Direction Mixer](../effects/direction-mixer.md), [Stereo Spread](../effects/stereo-spread.md) |
| Reverb | [Plate Reverb](../effects/plate-reverb.md), [Ambient Reverb](../effects/ambient-reverb.md), [Shimmer Reverb](../effects/shimmer-reverb.md) |
| Analysis | [Loudness Meter](../effects/loudness-meter.md) |

Folders start collapsed. Click a folder to hide or show its effects. Use Up/Down to move through visible rows. Enter toggles a focused folder; Left closes it and Right opens it. Left on an effect closes its folder. Closing a folder deselects its hidden effects. Each catalog retains its own folder choices while the application is open.

Select the destination track, beat subtrack, or Master before adding an effect. Double-click an effect to add it. Shift-click selects effects across open folders; Enter on an effect adds the selection in folder order. Drag selected effects into the lane to choose their insertion point, then release to add them. The insertion preview appears after the preceding unit’s output meter. Escape or an invalid drop cancels. Scroll when the catalog does not fit.

Open a layer tab, then add effects from the Effects catalog or the Effects row’s context menu. Drag effects to set their order. Drop effects on another layer tab to append them to that layer, or drop them in the bottom Effects Lane to move them to the whole instrument. Hold Option on macOS or Ctrl on Windows/Linux to duplicate them. Selected effects transfer together. Click the bottom Effects lane to target track effects.

Layer effects process that layer’s notes before its gain and pan. This lets you distort one layer while keeping another layer clean. The bottom Effects Lane processes the combined instrument after its Output controls. For a beat subtrack, select the subtrack to open its own Effects Lane. Find the instrument’s Output gain and meter in **Macros**.

When you open an older project, its Output effects move to the start of the corresponding track or subtrack Effects Lane, in their original order.

Effects run in their left-to-right order. Master effects process the combined track mix.

The header power control turns a unit on or off. Right-click a header for **Enable** or **Disable** as the first menu item. On a selected unit, this action applies to all selected units in its lane. Double-click its header to collapse or expand it. Off units keep their settings and remain editable. See [moving and copying units](../editing/units.md) and [modulation](../modulation/README.md).

Audio units have a vertical [output meter](../effects/output-meter.md) in the gap
after each unit, including when collapsed. Click a meter to reset its held peak.
Modulation units do not show audio meters.
