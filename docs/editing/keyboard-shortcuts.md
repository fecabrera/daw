# Keyboard shortcuts

**Cmd** means Command on macOS. **Ctrl** means Control on Windows and Linux.
Where the guide says **Control on macOS**, use the separate Control key.
Menus show shortcuts for commands that have one.

Click the pane or control you want to use before pressing a shortcut that
depends on focus. Text fields keep their own editing keys. Open menus, dialogs,
active drags, or blocking jobs can take priority over project commands.

- [Projects and application](#projects-and-application)
- [Views and playback](#views-and-playback)
- [Clips, tracks and editing history](#clips-tracks-and-editing-history)
- [Selection and deletion](#selection-and-deletion)
- [Tracks list and catalogs](#tracks-list-and-catalogs)
- [Modifier gestures](#modifier-gestures)
- [Controls and text fields](#controls-and-text-fields)

## Projects and application

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| New project | Cmd+N | Ctrl+N |
| Open project | Cmd+O | Ctrl+O |
| Save | Cmd+S | Ctrl+S |
| Save as | Cmd+Shift+S | Ctrl+Shift+S |
| Export | Cmd+Shift+E | Ctrl+Shift+E |
| Close project | Cmd+W | Ctrl+W |
| Open application Settings | Cmd+, | Ctrl+, |

On macOS, the app menu also provides **Cmd+H** to hide the app,
**Cmd+Option+H** to hide other apps, and **Cmd+Q** to quit. See
[Projects, settings, and export](../projects/README.md).

## Views and playback

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| Open Timeline | Cmd+1 | Ctrl+1 |
| Open Instrument | Cmd+2 | Ctrl+2 |
| Open Mixer | Cmd+3 | Ctrl+3 |
| Play or pause | Space | Space |
| Toggle the transport loop | Cmd+L | Ctrl+L |
| Increase interface zoom | Cmd+Plus or Cmd+Equals | Ctrl+Plus or Ctrl+Equals |
| Decrease interface zoom | Cmd+Minus | Ctrl+Minus |
| Reset interface zoom | Cmd+0 | Ctrl+0 |

Space works from any DAW view when playback controls are available. Use
**Transport** for Stop, Record, Metronome, Count-in, and Panic. See
[Timeline transport and seeking](../views/timeline.md#transport-and-seeking).

## Clips, tracks and editing history

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| Import an audio clip | Cmd+Shift+I | Ctrl+Shift+I |
| Copy selected clips or a time range | Cmd+C | Ctrl+C |
| Paste clips or a copied time range at the playhead | Cmd+V | Ctrl+V |
| Split selected clips that cross the playhead | Cmd+T | Ctrl+T |
| Merge compatible selected clips | Cmd+J | Ctrl+J |
| Bounce selected whole clips | Cmd+B | Ctrl+B |
| Bounce selected tracks | Cmd+Shift+B | Ctrl+Shift+B |
| Rename the selected track or regular instrument layer | Cmd+R | Ctrl+R |
| Group selected tracks | Cmd+G | Ctrl+G |
| Ungroup the selected group | Cmd+Shift+G | Ctrl+Shift+G |
| Toggle Mute | M | M |
| Toggle Solo | S | S |
| Undo a project edit | Cmd+Z | Ctrl+Z |
| Redo a project edit | Cmd+Shift+Z | Ctrl+Shift+Z |

In Timeline and Mixer, **M** and **S** apply to the selected tracks or beat
subtracks as appropriate. In Instrument, they apply to the selected regular
layers or to the displayed track or subtrack when no regular layer is selected.
In Instrument, Rename targets the primary selected regular layer when one is
selected. Press **Enter** to finish a rename or **Escape** to cancel it.

See [Timeline clip editing](../views/timeline.md#select-and-edit-clips),
[track groups](track-groups.md), [bounce](bounce.md), and
[undo and redo](undo-redo.md).

## Selection and deletion

| Focus or selection | Shortcut | Result |
| --- | --- | --- |
| Timeline clip area | Cmd+A / Ctrl+A | Select all timeline clips and clear any selected time range. |
| Arrangement lanes with a section marker, tempo point, or signature selected | Cmd+A / Ctrl+A | Select all arrangement items of the selected type. |
| Installed instrument or effect lane | Cmd+A / Ctrl+A | Select all installed units in that lane. |
| Clip editing | Delete or Backspace | Remove the selected time range, if any, or the selected clips. |
| Piano roll with notes selected | Delete or Backspace | Remove the selected MIDI notes. |
| Arrangement lanes | Delete or Backspace | Remove the selected section markers, tempo points, or signatures. The initial tempo and signature are protected. |
| Installed instrument or effect lane | Delete or Backspace | Remove the selected units in the focused lane. |
| Focused beat subtrack name | Delete or Backspace | Remove selected beat subtracks where the subtrack header supports deletion. |
| Open menu, active edit, or drag | Escape | Dismiss the menu or cancel the edit or gesture. |
| Timeline | Escape | Clear the beat subtrack selection if active. Otherwise clear selected clips or a time range first, then the track selection on the next press. With arrangement lane focus, clear the selected arrangement items. |
| Instrument | Escape | Clear layer and beat subtrack selections. |
| Mixer | Escape | Clear the track, group, and Master selection. |
| Focused catalog | Escape | Clear the catalog selection. |

Click a clip header to return to clip editing after selecting MIDI notes.
See [Timeline Editor](../views/editor.md), [beat subtracks](../views/beat-editor.md),
and [moving and copying units](units.md).

## Tracks list and catalogs

Click a row to give its list keyboard focus.

| Shortcut | Instrument Tracks list | Components, Effects, and Chords catalogs |
| --- | --- | --- |
| Up or Down | Move through visible rows. Moving onto a track or subtrack selects it. | Move through visible folders and entries. |
| Left | Collapse an open branch or move to its parent. | Close the focused folder or the folder containing the focused entry. |
| Right | Expand a group or beat track. | Open the focused folder. |
| Enter | Toggle a group or select a track or subtrack. | Toggle a folder or add the selected entries. Chords insert at the playhead on the selected MIDI track. |
| Shift+Up or Shift+Down | — | Extend or reduce the catalog selection. |

See the [Instrument view](../views/instrument.md) and
[adding chords](../views/timeline.md#add-chords).

## Modifier gestures

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| Toggle a clip or track in a selection | Shift-click or Cmd-click | Shift-click or Ctrl-click |
| Toggle a note, arrangement block, beat subtrack, layer, unit, or catalog entry in its selection | Shift-click | Shift-click |
| Toggle clips or notes using a selection rectangle | Shift-drag in empty space | Shift-drag in empty space |
| Select a time range on the Timeline lane or lower editor clip strip | Cmd-drag | Ctrl-drag. Start outside an existing range to create a new one. |
| Copy clips, a selected time range, MIDI notes, layers, or installed units during a move | Option-drag | Ctrl-drag. For a time range, start inside the range. |
| Stretch a clip | Option-drag a trim or loop edge | Ctrl-drag a trim or loop edge |
| Draw a note in empty piano-roll space | Cmd-drag | Ctrl-drag |
| Bypass snapping while editing | Hold Shift. In the piano roll this bypasses grid and scale snapping. | Hold Shift. In the piano roll this bypasses grid and scale snapping. |
| Zoom the timeline under the pointer | Cmd+scroll | Ctrl+scroll |
| Scroll horizontally over the timeline | Shift+scroll | Shift+scroll |
| Fine-adjust a knob, fader, or draggable numeric value | Shift-drag | Shift-drag |
| Change Q from a supported EQ graph handle | Cmd-drag | Ctrl-drag |
| Change only Ratio from the Compressor curve handle | Cmd-drag | Ctrl-drag |
| Change only Threshold from the Compressor curve handle | Option-drag | Alt-drag |

For copy gestures, keep the modifier held when you release the mouse. For
examples, see [clip editing](../views/timeline.md#select-and-edit-clips),
[piano-roll editing](../views/editor.md#create-and-edit-a-pattern),
[EQ graphs](../effects/equalizer.md#response-graph), and the
[Compressor](../effects/compressor.md).

## Controls and text fields

| Shortcut | Action |
| --- | --- |
| Tab or Shift+Tab | Move keyboard focus to the next or previous control. |
| Enter | Activate a focused button or confirm a name or value in an inline editor. |
| Up or Right on a focused knob | Increase the value by a keyboard step. |
| Down or Left on a focused knob | Decrease the value by a keyboard step. |
| Arrow keys on a focused slider | Adjust the value, including fine adjustment of the Timeline track height. |
| Cmd/Ctrl+A in a text field | Select all text. |
| Cmd/Ctrl+X, Cmd/Ctrl+C, or Cmd/Ctrl+V in a text field | Cut, copy, or paste text. |
| Cmd/Ctrl+Z in a text field | Undo a text change. |
| Cmd/Ctrl+Shift+Z or Cmd/Ctrl+Y in a text field | Redo a text change. |
| Arrow keys, Home, or End in a text field | Move the caret. Hold Shift to extend the selection. |
| Option+Left/Right on macOS or Ctrl+Left/Right on Windows/Linux | Move by one word in a text field. |
| Cmd+Left/Right on macOS | Move to the start or end of the current line in a text field. |
| Cmd/Ctrl+Up/Down, or Ctrl+Home/End on Windows/Linux | Move to the start or end of the text field. |
| Option+Backspace/Delete on macOS or Ctrl+Backspace/Delete on Windows/Linux | Delete the preceding or following word in a text field. |
| Cmd+Backspace/Delete on macOS | Delete to the start or end of the current paragraph in a text field. |
| Escape in a name or value editor | Cancel the draft. |

In macOS text fields, **Control+A/E** moves to the start or end of the line,
**Control+B/F** moves left or right, and **Control+P/N** moves up or down.
**Control+H** deletes the previous character, **Control+W** deletes the
previous word, and **Control+U/K** deletes to the start or end of the paragraph.
