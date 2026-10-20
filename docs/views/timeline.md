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

Hold Command on macOS, or Ctrl on Windows and Linux, and drag across a track to select a time range. The pointer uses the same crosshair as creating a loop range. The range can cover part of a clip, several clips, and the silence between them. Drag the range to move its contents. Hold Option on macOS, or Ctrl on Windows and Linux, and drag the range to duplicate it. On Windows and Linux, start that drag inside the range; Ctrl-drag outside the range selects a new range. Copy with Command+C or Ctrl+C, and paste at the playhead. The copy keeps the gaps and any silence before the first clip. Delete or Backspace removes the selected time and leaves the rest of each clip. The same range is available in Editor. Shift bypasses grid snapping. Escape or a click outside the range clears it.

Drag a waveform edge to trim. Drag a header edge to repeat the current range. Hold the copy/stretch modifier while dragging an edge to [stretch duration](../audio/time-stretch.md). Group edits preserve relative timing and apply together. Drag the dividing line between touching waveforms to move their shared boundary while keeping their outer endpoints fixed.

Copy with Command+C on macOS or Ctrl+C on Windows/Linux. Paste with Command+V or Ctrl+V at the playhead on the selected track. Copies keep their source content, trim, repeats, stretch, and color override.

Split the selected clips at the playhead with Edit > Split at playhead, or with Command+T on macOS or Ctrl+T on Windows/Linux. The same command is available in Editor. It is disabled when no clip is selected, when the playhead is not inside a selected clip, and in Mixer and Instrument. If several clips are selected, every selected clip that crosses the playhead is split together. The original clip ends at the playhead and a new clip starts there. Audio keeps its recording, trim, loop, and stretch. Right-click a clip and choose Split at playhead to split that clip, or the whole selection when the clicked clip is selected.

MIDI notes that cross the playhead follow Settings > Split MIDI notes. Split into two is the default: the note becomes two notes. Preserve first keeps the part before the playhead on the original clip and does not continue it. The choice is saved on this computer, not in the project. Undo restores both pieces in one step.

Duplicating into new track space creates fresh track defaults. Group paste also creates fresh tracks when needed and preserves gaps between tracks. A new MIDI track starts with an empty instrument and needs a source before it makes sound. Existing destination tracks keep their settings. Moving MIDI clips to new track space retains their instrument.

## Navigate and play

Press Space to play or pause from any view, pane, or focused control. Text fields take precedence: Space inserts a space while you edit text. Holding Space does not toggle playback repeatedly. Dialogs and blocking background jobs still disable the shortcut.

The ruler uses four beats per bar, with bars and beats counted from 1. Click to set the playhead. Drag the selection band to choose a loop range; use Loop to enable it. Changing tempo keeps clip starts and the loop range at their musical positions. Audio clip duration stays unchanged; MIDI timing follows tempo.

Seeking and editing notes during playback keep the current audio and playhead running while the change is prepared. A seek then fades to the requested position. A note edit takes effect at the current playback position, without pausing or returning to an earlier position. Changes to effect latency use the same continuous playback behavior.

Recently prepared positions use cached sound state for faster seeks. The first seek, a seek outside the cached range, or a seek after a project edit can still require history reconstruction. This can delay when a change becomes audible. Rapid changes replace pending changes. Pause stops the active audio even while a seek or edit is being prepared; ordinary pause and resume retain the current sound state.

Horizontal scrolling moves the timeline while track controls stay fixed. Pinch zoom keeps the pointer's time in place where possible. Double-click a MIDI clip to open it in [Editor](editor.md).

## Clip settings

Clip settings is available only in Timeline. Open it from the left rail with the headphone-file button. It uses the same column as the Effects catalog; opening either panel closes the other. You can close both. Leaving Timeline closes Clip settings and discards any unconfirmed text.

Select a clip to edit its name, gain, and transpose. The source row shows the audio file name, or MIDI pattern for a MIDI clip. These rows use the smaller catalog text, and the values are aligned to the left. The source is dimmed because it cannot be edited. A long name is shortened with an ellipsis; hover the source to see the full path. Gain is shown in dB, from silence (−∞ dB) to +12 dB, for example 0.0 dB. Transpose is shown in semitones, from −48 to +48, for example 0 semitones.

Drag gain up or down in steps of 0.1 dB. Drag transpose up or down in steps of 1 semitone. Hold Shift while dragging for a finer adjustment: 0.01 dB, or 0.1 semitone. A sideways drag does not change the value. Dragging up from silence starts at −60 dB.

Double-click the name, gain, or transpose to type a value, as with the tempo label. Enter saves it. Escape, or clicking away, restores the previous value. With no clip selected, the fields stay unavailable.

Gain changes the clip's level before the track level. Transpose shifts MIDI notes while they play and leaves the written pattern unchanged. A note that would fall below MIDI note 0 or above 127 plays at that nearest note. Audio transpose changes playback pitch while keeping the clip duration and original recording unchanged. It uses the project's selected [stretch engine](../audio/time-stretch.md#choose-an-engine). For audio, dragging previews the value and releasing starts preparation; Escape cancels the drag. Playback continues with the previous audio until preparation succeeds. Cancel or failure keeps the previous value and audio.

When several clips are selected, Name and Source are disabled and show “Multiple clips selected”, even when they match. Gain and Transpose stay editable. Each shows the shared value, or “Multiple values” if the selected clips differ. Dragging applies the same change to each clip, preserving their differences across tracks. For example, raising clips at 0 dB and −6 dB by 1 dB gives 1 dB and −5 dB. Movement stops for the whole group when any clip reaches a limit. Typed values set the same value on every selected clip. Undo restores the whole group in one step.

While dragging multiple clips, the label shows only the signed change applied so far, for example “+1.00 dB” or “−2 semitones”. This also applies when the clips start with equal values. The change stops increasing at a group limit. Gain shows two decimal places so fine adjustments remain visible. Releasing the drag restores “Multiple values” for differing values, or the shared value when they match.

Silent clips stay silent when selected with audible clips. The audible clips receive the gain change. If all selected clips are silent, dragging up raises them together from −60 dB, using the usual steps.

If any selected clip contains audio, transpose previews for the whole group until release. All selected clips, including MIDI clips, change together after preparation succeeds. Failure or cancellation keeps the previous settings and audio for the whole group. Changing the selection cancels an unconfirmed text edit or transpose preview.

## Track groups

Select track headers and choose **Track > Group selected tracks** to create a shared processing bus. Group and Master labels align left. Use the borderless group arrow to hide or show children. Drag tracks into a group, or move the complete group from its name or blank header area. See [Track groups](../editing/track-groups.md) for routing, shared effects, and Ungroup.

## Add effects

Select a track, group, or Master, then use the [Effects catalog](instrument.md#effects). Browse EQ & Filters, Dynamics, Distortion, and Imaging. Double-click an effect, select effects and press Enter, or drag them into the effects lane. Click a folder to hide or show its entries.
