# Editor

MIDI patterns contain notes. A MIDI track's instrument supplies their sound. Several clips can use the same pattern content.

## Create and edit a pattern

1. Add a MIDI track from the Track menu.
2. Double-click empty MIDI lane space to create a one-beat pattern.
3. Double-click the clip to open Editor.
4. Hold Command on macOS or Ctrl on Windows/Linux and drag in the piano roll to draw a note.
5. Drag a note body to move it or an edge to change its length. Double-click a note to delete it.

Each pattern has a named header below the lower ruler and one continuous outline around its header and visible note area. Click a header to select that pattern without changing zoom or scroll. Headers stay visible when you scroll the piano keys vertically.

Drag a header to move the selected patterns. Drag a header edge to loop them, or an edge beside the notes to trim them. Hold Option on macOS or Ctrl on Windows/Linux while dragging either edge to stretch. Hold Shift to bypass snapping. Changes preview during the drag and apply on release; Escape cancels. These gestures keep the lower editor’s zoom and scroll.

Right-click a header or border for the shared clip menu, including Velocity and Delete note when a note is selected.

A plain empty-space drag selects notes with a rectangle. Shift-click toggles individual notes; Shift-drag toggles notes within the rectangle. Delete or Backspace removes selected notes. Hold Option on macOS or Ctrl on Windows/Linux while moving notes to copy them. Escape cancels an active gesture.

Timeline and Editor offer the same clip selection, time-range selection, copy/paste, split, move, trim, loop, and stretch actions. Command-drag on macOS, or Ctrl-drag on Windows and Linux, selects a time range on the clip strip. The pointer uses the same crosshair as creating a loop range. Split at playhead uses the same Clip menu command and shortcut as Timeline. MIDI notes that cross the playhead follow Split MIDI notes in Settings. Timeline also supports vertical placement onto other compatible tracks or new track space. Editor's clip moves keep their track assignments. Shift bypasses grid snapping.

## Track and scale controls

The left panel stays visible while you scroll the piano roll. Its track header is beside the clip strip. The options section below it aligns with the lower editor toolbar. Both editor panes use the same floating horizontal scrollbar as the effects lane. It appears over the content and does not reduce the editing area. Hover near the bottom of a pane to reveal and expand its scrollbar. Use the name dropdown to select a track. It selects that track's first clip, or shows an empty editor if the track has no clips. With no selected track, the editor stays blank below the ruler. Mute, Solo, Gain, Pan, and the stereo meters work as they do in Timeline.

For MIDI tracks, **MIDI options** contains the scale controls. Root defaults to **Off**, which disables Scale and Snap to scale. Select a root to enable those controls. Choose a scale to dim the piano keys and notes outside it. Enable **Snap to scale** to constrain newly drawn notes and pitch moves. Major is the initial scale and snapping starts off. Returning Root to Off removes the dimming and bypasses snapping; it retains your scale and snapping choices.

Each MIDI track keeps its own Root, Scale, and Snap to scale settings. Save the project to retain them after reopening. Changing these settings marks the project modified and supports Undo/Redo. Duplicating a MIDI track copies its options. Older projects start with Root Off, Major, and snapping off.

Changing the root, scale, or snapping option leaves existing notes unchanged. Drawing snaps to the nearest scale note; equal distances use the lower note. Moving a group snaps the dragged note and keeps the intervals between selected notes. Timing-only moves and edge resizes keep the original pitches. Hold Shift to bypass both grid and scale snapping.

## Editor zoom and scroll

The track overview and lower editor have independent horizontal viewports. The playback toolbar’s zoom control and the scrollbar below the overview control the overview. **Fit all clips**, immediately left of that zoom control, shows all clips on the displayed track, including gaps between them. It leaves the lower editor’s view unchanged and is disabled when the track has no clips. The **Fit clip** button (outward arrows between two vertical edges), the zoom control immediately to its right above the lower ruler, and the bottom scrollbar control the waveform or piano roll. Pinch or scroll horizontally over the pane you want to change. The selected clip initially fills the lower editor width. Fit clip restores that view; while fitted, resizing the window keeps the clip fitted.

Zoom out in either audio or MIDI content to see other clips on that track. MIDI note selection and group edits across clips keep the working viewport. Selecting a clip in the overview fits it in the lower pane. Both rulers control the same project loop. In either ruler’s upper band, drag the loop body to move it, drag an edge to resize it, or drag empty space to select a new range. Click the range to turn looping on or off. Hold Shift to bypass snapping. Escape or loss of window focus cancels a drag. The lower band positions the playhead. The lower ruler uses its own zoom and scroll, and loop gestures keep both viewports unchanged. Zoom, scroll, and Fit clip do not modify the project. Seeking retains the overview ruler’s saved-navigation behavior.

## Audio clips

Select an audio clip in the overview strip to see its enlarged waveform below. The lower editor has its own ruler, horizontal zoom, and scroll. It fits the selected clip to its width when you select a clip in the overview. Zoom out to see other clips on the same track. Both rulers follow the project playhead. Mono uses one waveform lane; stereo shows both channels. Trim offsets, loop phases, and prepared stretch or transpose audio match Timeline. Selecting another clip in the overview fits that clip in the lower editor and keeps the overview viewport. Click a waveform in the lower editor to select its clip and update Clip options without changing either viewport’s zoom or scroll. Right-click a waveform to open the same clip context menu as Timeline. Opening the menu keeps the current selection, zoom, and scroll; actions use the clicked clip or its selected group. Missing recordings show “Missing source”.

The lower waveform editor supports the same clip gestures as the overview. Drag a clip header to move the selected clips. Drag a header edge to loop them, or a waveform edge to trim them. Hold Option on macOS or Ctrl on Windows/Linux while dragging either edge to stretch. Hold Shift to bypass snapping. Drag the dividing line between adjacent waveforms to move their shared boundary while keeping their outer endpoints fixed. These gestures preview changes and apply them on release; Escape or focus loss cancels. Zoom and scroll stay unchanged. A failed stretch keeps the original clips and audio. Dragging inside a waveform body does not move or select clips. If a shared-boundary trim removes the selected clip, the editor shows Select a clip and keeps its zoom and scroll.

The **Clip options** section shows the same Name, Source, Gain, and Transpose controls as [Timeline clip settings](timeline.md#clip-settings). Select a clip in the overview strip to edit it; with no selection, the panel shows “Select a clip”. Changes appear in both views and support Undo/Redo. Switching tracks or views discards unconfirmed text and audio transpose previews.

## Add sound

1. Open Instrument and select the MIDI track and a layer.
2. Add Oscillator or [Sampler](../instruments/sampler.md) from the component catalog.
3. Play the pattern. Add Amplitude and an Envelope if you want level shaping.

See [Oscillator](../instruments/oscillator.md), [Sampler](../instruments/sampler.md), and [Amplitude](../instruments/amplitude.md) for sound controls.

## Envelope unit

An [Envelope](../instruments/envelope.md) shapes a parameter across Attack, Decay, Sustain, and Release. Its guide explains Time/Note timing and Per note, Retrigger, and Legato modes. Connect it through the receiving unit's [modulation controls](../modulation/README.md).

## Copy clips to new tracks

Duplicating a clip into empty Timeline track space creates a fresh audio or MIDI track. Group paste also creates fresh tracks when needed. New MIDI tracks have one empty instrument layer and stay silent until a source is added. Existing destination tracks keep their instruments.

Copies retain the clip's audio or pattern, name, trim, loop, stretch, and color override. Group timing and track gaps stay intact. Release completes a drag; Escape or focus loss cancels it. Moving MIDI clips to new track space retains their instrument. The separate Duplicate track action copies track settings into an empty track.
