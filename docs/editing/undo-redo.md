# Undo and redo

Use **Edit → Undo** to reverse the last project edit. Use **Edit → Redo** to apply it again.

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| Undo | Command+Z | Ctrl+Z |
| Redo | Command+Shift+Z | Ctrl+Shift+Z |

The Edit menu is in the macOS menu bar. On Windows and Linux, open the application menu in the toolbar. Unavailable actions are disabled.

History covers committed track, clip, MIDI-note, instrument, effect, modulation, mix, loop-range, and project-settings changes. Imports, sample preparation, and time stretching enter history only after they succeed. Group edits return in one step. A knob or fader drag also returns in one step.

Undo retains the current playback position. Restoring a different sample rate or buffer size stops playback; press Play to resume. Prepared audio stays available while its history step is retained, even if the source file later becomes unavailable.

Selection, zoom, scrolling, playback, and seeking do not add undo steps. Text fields keep their own text undo behavior while you edit them. Project undo is unavailable during an active drag, blocking operation, or dialog.

A new edit after Undo clears Redo. A cancelled edit or an edit with no change keeps Redo. The session retains up to 100 undo steps and limits their estimated memory to 128 MiB. Large edits or audio memory pressure can remove older steps sooner. The app shows a notice when memory pressure shortens history. The current project stays intact. Prepared audio shared with the current project does not count toward the history limit.

Save and Save As retain history. Undo keeps the current save folder and does not change source recordings or exported files. Returning to the saved project state clears the unsaved marker unless you also changed the saved playhead position. Opening, closing, or creating a project clears history. History is not saved in the project file.
