# Timeline

Timeline arranges audio and MIDI clips on tracks. Use Editor for MIDI notes, Instrument for MIDI sounds, and Mixer for track levels.

## Import and arrange

1. Add an audio or MIDI track from the Track menu or the Tracks header.
2. Use File > Import for WAV audio, or drop a supported WAV file on a track. Dropping into empty Timeline space creates a new audio track.
3. Drag a clip header to move it. Hold Option on macOS or Ctrl on Windows/Linux to copy it.
4. Release to apply the change. Escape or focus loss cancels an active gesture.

Moves and copies take priority over destination clips. Covered neighbors are trimmed, split, or removed. The preview shows those changes before release. Shift bypasses grid snapping.

## Select and edit clips

Click a clip to select it. Shift-click toggles group membership. Drag a rectangle in empty space to select intersecting clips; Shift-drag toggles them against the starting selection.

Drag a waveform edge to trim. Drag a header edge to repeat the current range. Hold the copy/stretch modifier while dragging an edge to [stretch duration](../audio/time-stretch.md). Group edits preserve relative timing and apply together. Drag the dividing line between touching waveforms to move their shared boundary while keeping their outer endpoints fixed.

Copy with Command+C on macOS or Ctrl+C on Windows/Linux. Paste with Command+V or Ctrl+V at the playhead on the selected track. Copies keep their source content, trim, repeats, stretch, and color override.

Duplicating into new track space creates fresh track defaults. Group paste also creates fresh tracks when needed and preserves gaps between tracks. A new MIDI track starts with an empty instrument and needs a source before it makes sound. Existing destination tracks keep their settings. Moving MIDI clips to new track space retains their instrument.

## Navigate and play

Press Space to play or pause from any view, pane, or focused control. Text fields take precedence: Space inserts a space while you edit text. Holding Space does not toggle playback repeatedly. Dialogs and blocking background jobs still disable the shortcut.

The ruler uses four beats per bar, with bars and beats counted from 1. Click to set the playhead. Drag the selection band to choose a loop range; use Loop to enable it. Changing tempo keeps clip starts and the loop range at their musical positions. Audio clip duration stays unchanged; MIDI timing follows tempo.

Seeking and editing notes during playback keep the current audio and playhead running while the change is prepared. A seek then fades to the requested position. A note edit takes effect at the current playback position, without pausing or returning to an earlier position. Changes to effect latency use the same continuous playback behavior.

Recently prepared positions use cached sound state for faster seeks. The first seek, a seek outside the cached range, or a seek after a project edit can still require history reconstruction. This can delay when a change becomes audible. Rapid changes replace pending changes. Pause stops the active audio even while a seek or edit is being prepared; ordinary pause and resume retain the current sound state.

Horizontal scrolling moves the timeline while track controls stay fixed. Pinch zoom keeps the pointer's time in place where possible. Double-click a MIDI clip to open it in [Editor](editor.md).

## Add effects

Select a track or Master, then use the [Effects catalog](instrument.md#effects). Browse EQ & Filters, Dynamics, Distortion, and Imaging. Double-click an effect, select effects and press Enter, or drag them into the effects lane. Click a folder to hide or show its entries.
