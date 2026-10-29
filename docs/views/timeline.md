# Timeline

The left toolbar lists **Effects** and **Chords**. **Clip settings** uses the toolbar at the right of the timeline area, from the playback toolbar down to the effects lane.

Timeline arranges audio and MIDI clips on tracks. Use Editor for MIDI notes, Instrument for MIDI sounds, and Mixer for track levels. Its horizontal scrollbar floats over the content, like the effects lane’s scrollbar; hover near the bottom of the timeline to reveal and expand it.

Audio track headers show a waveform icon; MIDI track headers show a keyboard icon. Master shows a boom box icon. Their names use the same spacing as group headers. Track gain and pan, and Master gain, use compact knobs without text-entry fields. Tracks start with one row: icon, name, Mute, Solo, Pan, Gain, then meters. In the expanded layout, the second row places Mute and Solo at the left; Pan then Gain sit at the right, immediately before the level meters. Master uses one row, with its gain knob on the right immediately before the level meters. Level meter rails align with the top and bottom padding of the other header controls. Hover or drag a knob to see its value in the tooltip. Master's meters align with the track meters at the right of the control column. Drag an audio or MIDI icon, track name, or blank header area to move the track. Release applies the move; Escape or focus loss cancels it.

Use **Expand tracks to two rows**, beside Fit all clips and zoom in the playback toolbar, for the full layout. **Collapse tracks to one row** restores the default compact layout. Timeline and Editor share this choice for the app session. It does not change the project or audio. Master stays on one row.

The Tracks header’s Add track (Plus) button opens a dropdown with **Add audio track** and **Add MIDI track**. Choose an item to create and select a new empty track. Opening or dismissing the menu does not add a track.

The Add track and Duplicate track buttons stay available when another pane or non-text control has focus. Duplicate still requires a selected track. Text entry, jobs, dialogs, active gestures, and loss of window focus retain their guards.

Press M to toggle Mute and S to toggle Solo on each selected track. These shortcuts work from any focused pane or control in Timeline, except while you enter text. Dialogs, blocking jobs and active drags still disable them. Release a dragged control before using M/S. Holding a key does not toggle repeatedly.

Press Escape to clear selected clips or a selected clip time range first. Tracks and Master stay selected. If no clips or clip time range are selected, Escape clears track or Master selection. Text entry, open menus and active gestures take precedence. Holding Escape does not advance through these steps.

## Add chords

1. Open **Chords** with the piano button in the left toolbar.
2. Expand a note folder, or choose a root note in the header to show only that note’s chords. **All notes** restores all 12 roots. Sharp names also cover their flat equivalents.
3. Drag a chord onto a MIDI track. Drop below the tracks to create a new MIDI track. A drop on a track header starts at frame zero.
4. Follow the preview of the chord’s name, notes, and one-bar bounds. Covered clips preview their remaining ranges. Release to apply the placement. Hold Shift to bypass grid snapping; Escape or loss of window focus cancels.

Each chord is one 4/4 bar at the project tempo. Its notes sound together for the full bar, with roots from C4 through B4 and velocity 100. The catalog includes major, minor, diminished, augmented, sus2, sus4, major sixth, minor sixth, dominant seventh, major seventh, minor seventh, and half-diminished seventh chords. Edit the resulting notes in Editor.

Select a MIDI track, then click a chord or select it with Up/Down to hear it through that track’s instrument and effects. A new selection replaces the previous preview. The preview lasts one bar, up to eight seconds, with up to one second for release and effect tails. Track, group, and Master mix controls apply; other tracks’ Solo settings do not block the preview. Empty instruments and muted tracks remain silent. Audition does not insert notes, move the playhead, or change the project. Stop or Escape cancels it; changing tracks, leaving Chords, or losing window focus also stops it. Sample-based instruments may need a short preparation delay.

Double-click a chord to insert it at the playhead on the selected MIDI track. Enter inserts the selected chords there in catalog order, one after another. These actions require a selected MIDI track and keep the playhead in place. Shift-click selects several chords; dragging one selected chord places the whole selection in order.

With catalog focus, Up/Down navigates visible rows. Shift+Up/Down extends or reduces the chord selection. Enter opens or closes a focused note folder, or inserts selected chords when a chord row has focus. Escape clears catalog selection and follows Timeline’s normal track/clip selection behavior.

Chords use the normal clip overwrite rules and Undo/Redo. Audio and group tracks reject chord drops. Catalog browsing and filtering do not change the project; inserted chords save as ordinary MIDI patterns. A new MIDI track starts with an empty instrument, so add a sound in Instrument before playback.

## Import and arrange

1. Add an audio or MIDI track from the Track menu or the Tracks header.
2. Use Clip > Import clip for WAV, MP3, or AAC audio, or drop a supported audio file on an audio track. Dropping into empty Timeline space creates a new audio track.
3. Drag a clip header to move it. Hold Option on macOS or Ctrl on Windows/Linux to copy it.
4. Release to apply the change. Escape or focus loss cancels an active gesture.

Moves and copies take priority over destination clips. Covered neighbors are trimmed, split, or removed. The preview shows those changes before release. Shift bypasses grid snapping.

Audio clips can move or copy only to audio tracks; MIDI clips can move or copy only to MIDI tracks. This applies over both track headers and timeline lanes. An incompatible destination shows the invalid-action cursor. Releasing there rejects the entire selected group without an error dialog. Continue dragging to a compatible track to complete the move or copy.

Group buses cannot hold clips: dragging onto a bus shows an invalid-action cursor, and releasing there keeps all dragged clips unchanged. Drop clips onto child tracks.

Audio file drags over MIDI tracks or group buses show the invalid-action cursor. Drops there are ignored without an error dialog. Drop audio files onto audio tracks or empty Timeline space instead.

## Clip menu

Open **Clip** in the macOS menu bar or the Windows/Linux application menu. Its actions have three groups, separated by lines:

- **Import clip**, **Add MIDI pattern**.
- **Split at playhead**, **Stem-separation**.
- **Edit clip**, **Disable clip**, **Delete clip**.

Choose **Add MIDI pattern** with a MIDI track selected to place a new pattern at the playhead. It follows the same placement rules as other new clips: covered clips are trimmed, split, or removed. Undo restores the previous arrangement. Import clip accepts WAV, MP3, and AAC-LC audio. Use Command+Shift+I on macOS or Ctrl+Shift+I on Windows/Linux to import a clip.

Edit, Disable and Stem-separation use the primary selected clip. Disable changes to **Enable clip** for a disabled clip. Delete removes all selected clips. Split acts on selected clips that cross the playhead. Unavailable actions are disabled; Stem-separation requires an audio clip in Timeline and a validated model.

Right-click a clip for the same last two groups: Split/Stem-separation, then Edit/Disable/Delete. Edit, Disable and Stem-separation use the clicked clip. Split and Delete use the selected group when the clicked clip belongs to it; otherwise they use that clip. MIDI headers retain the separate selected-note Velocity and Delete note controls.

## Select and edit clips

Click a clip to select it. Shift-click toggles group membership. Drag a rectangle in empty space to select intersecting clips; Shift-drag toggles them against the starting selection.

Hold Command on macOS, or Ctrl on Windows and Linux, and drag across a track to select a time range. The pointer uses the same crosshair as creating a loop range. The range can cover part of a clip, several clips, and the silence between them. Drag the range to move its contents. Hold Option on macOS, or Ctrl on Windows and Linux, and drag the range to duplicate it. On Windows and Linux, start that drag inside the range; Ctrl-drag outside the range selects a new range. Copy with Command+C or Ctrl+C, and paste at the playhead. The copy keeps the gaps and any silence before the first clip. Delete or Backspace removes the selected time and leaves the rest of each clip. The same range is available in Editor. Shift bypasses grid snapping. Escape or a click outside the range clears it.

Drag a waveform edge to trim. Drag a header edge to repeat the current range. Hold the copy/stretch modifier while dragging an edge to [stretch duration](../audio/time-stretch.md). Group edits preserve relative timing and apply together. Drag the dividing line between touching waveforms to move their shared boundary while keeping their outer endpoints fixed.

Copy with Command+C on macOS or Ctrl+C on Windows/Linux. Paste with Command+V or Ctrl+V at the playhead on the selected track. Copies keep their source content, trim, repeats, stretch, and color override.

Split the selected clips at the playhead with Clip > Split at playhead, or with Command+T on macOS or Ctrl+T on Windows/Linux. The same command is available in Editor. It is disabled when no clip is selected, when the playhead is not inside a selected clip, and in Mixer and Instrument. If several clips are selected, every selected clip that crosses the playhead is split together. The original clip ends at the playhead and a new clip starts there. Audio keeps its recording, trim, loop, and stretch. Right-click a clip and choose Split at playhead to split that clip, or the whole selection when the clicked clip is selected.

MIDI notes that cross the playhead follow Settings > MIDI > Split MIDI notes. Split into two is the default: the note becomes two notes. Preserve first keeps the part before the playhead on the original clip and does not continue it. The choice is saved on this computer, not in the project. Undo restores both pieces in one step.

Duplicating into new track space creates fresh track defaults. Group paste also creates fresh tracks when needed and preserves gaps between tracks. A new MIDI track starts with an empty instrument and needs a source before it makes sound. Existing destination tracks keep their settings. Moving MIDI clips to new track space retains their instrument.

## Navigate and play

Press Space to play or pause from any view, pane, or focused control. Text fields take precedence: Space inserts a space while you edit text. Holding Space does not toggle playback repeatedly. Dialogs and blocking background jobs still disable the shortcut.

The ruler uses four beats per bar, with bars and beats counted from 1. In dark mode, labels outside the loop range use light text. Text on the enabled loop highlight uses dark text for contrast. Click to set the playhead. Drag the selection band to choose a loop range; use Loop to enable it. Changing tempo keeps clip starts and the loop range at their musical positions. Audio clip duration stays unchanged; MIDI timing follows tempo.

Seeking and editing notes during playback keep the current audio and playhead running while the change is prepared. A seek then fades to the requested position. A note edit takes effect at the current playback position, without pausing or returning to an earlier position. Changes to effect latency use the same continuous playback behavior.

Recently prepared positions use cached sound state for faster seeks. The first seek, a seek outside the cached range, or a seek after a project edit can still require history reconstruction. This can delay when a change becomes audible. Rapid changes replace pending changes. Pause stops the active audio even while a seek or edit is being prepared; ordinary pause and resume retain the current sound state.

Use **Fit all clips**, immediately left of the playback toolbar’s zoom control, to show every clip across all tracks. It fits from the earliest clip start to the latest clip end, including gaps. The button is disabled when there are no clips. It changes only the view.

Horizontal scrolling moves the timeline while track controls stay fixed. Pinch zoom keeps the pointer's time in place where possible. Double-click a MIDI clip to open it in [Editor](editor.md).

## Clip settings

In Timeline, use the headphone-file button in the right toolbar to open Clip settings beside it. The panel can stay open with the Effects or Chords catalog on the left. Closing it gives its space back to the timeline. When the effects lane is hidden, the right toolbar extends to the status bar. Leaving Timeline closes this panel and discards any unconfirmed text. Audio tracks also offer these controls under **Clip options** in [Editor](editor.md).

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

Drag an installed effect header from the effects lane onto another track’s header or clip lane to append it to that track’s effects. Audio tracks, MIDI tracks, and groups accept effects. Drop onto the fixed Master block to append effects to Master. Shift-click effects first to move a selected group in chain order. Hold Option on macOS or Ctrl on Windows/Linux to copy instead; the modifier at release determines the action. The target row has an outline during a valid drag. Release to apply the change, or press Escape to cancel.

A drop selects the destination track or Master and the transferred effects. Effects that use modulation sources from another track show a forbidden cursor and reject the whole group without changing either chain. Master cannot use track modulation sources. Remove incompatible source assignments before transferring effects.

## Stem separation

Right-click an audio clip and choose **Stem-separation** to create a group with aligned Drums, Bass, Other, and Vocals tracks. The original stays in place, disabled. Select local models in Settings first. Unsaved projects use temporary stem storage until the first save. See [Stem separation](../audio/stem-separation.md) for setup, progress, cancellation, and file storage.

Right-click a disabled clip and choose **Enable clip** to restore it. **Disable clip** silences only that clip and shows a greyed version of its color. These actions support Undo and Redo.

## Supported imports

Import mono or stereo WAV, MP3, or AAC-LC clips. AAC-LC can use `.aac` (ADTS),
`.m4a`, or `.mp4` containers. WAV supports PCM16, PCM24, and float32. Audio is
converted to the project processing rate, and its waveform is prepared before
the clip is added. The original recording remains unchanged and must remain
available when reopening the project.

HE-AAC, protected audio, other codecs inside MP4 files, and files with more than
two channels are unsupported. A rejected import leaves the arrangement intact.
MP3 encoder delay and padding are removed when the file provides that information.
AAC files can retain encoder padding, so their clip length can be slightly longer
than the original uncompressed recording.
