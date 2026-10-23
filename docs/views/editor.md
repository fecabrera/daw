# MIDI Editor

MIDI patterns contain notes. A MIDI track's instrument supplies their sound. Several clips can use the same pattern content.

## Create and edit a pattern

1. Add a MIDI track from the Track menu.
2. Double-click empty MIDI lane space to create a one-beat pattern.
3. Double-click the clip to open Editor.
4. Hold Command on macOS or Ctrl on Windows/Linux and drag in the piano roll to draw a note.
5. Drag a note body to move it or an edge to change its length. Double-click a note to delete it.

A plain empty-space drag selects notes with a rectangle. Shift-click toggles individual notes; Shift-drag toggles notes within the rectangle. Delete or Backspace removes selected notes. Hold Option on macOS or Ctrl on Windows/Linux while moving notes to copy them. Escape cancels an active gesture.

Timeline and Editor offer the same clip selection, time-range selection, copy/paste, split, move, trim, loop, and stretch actions. Command-drag on macOS, or Ctrl-drag on Windows and Linux, selects a time range on the clip strip. The pointer uses the same crosshair as creating a loop range. Split at playhead uses the same Clip menu command and shortcut as Timeline. MIDI notes that cross the playhead follow Split MIDI notes in Settings. Timeline also supports vertical placement onto other compatible tracks or new track space. Editor's clip moves keep their track assignments. Shift bypasses grid snapping.

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
