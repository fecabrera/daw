# Envelope

An Envelope changes a control value over a note's life. Select the instrument-wide [Envelope lane](modulation-lanes.md) and add it from Modulators in the Instrument components catalog, then assign it in a receiving unit's [modulation controls](../modulation/README.md). Adding it alone makes no connection.

## Stages and timing

Attack raises the value, Decay moves it toward Sustain, Sustain holds it while the note continues, and Release lowers it after note-off. Curve selectors change the stage shapes.

The first strip, **Shape**, shows the envelope contour at both row heights.
In the expanded row, the horizontal labels mark Attack, Decay, Sustain, and
Release. The vertical labels show the envelope level. Drag the peak left or
right to change Attack. Drag the start of Sustain left or right to change
Decay, or up or down to change the Sustain level. Drag the start of Release
left to lengthen Release or right to shorten it.

Drag the small handle midway through an Attack, Decay, or Release curve
to adjust its strength. Drag up to increase it or down to decrease it.
Choose a curve shape other than Linear to show its handle.

The knobs and the Envelope's effect on sound follow the drag. Release the
pointer to save one undoable change or press Escape to restore the previous
values.
Selecting a handle highlights its part of the gray curve. The larger handle
at the start of Sustain highlights Decay and Sustain.

The larger handle at the start of Sustain follows the first drag direction.
Move at least 12 points across the other axis to switch between Decay time and
Sustain level.

During playback or live MIDI input, a dot shows the actual stage progress and level.
For Per note mode, the dot shows one active voice. The dot disappears when
the Envelope is idle or off. While playback is paused or stopped, the dot
shows live MIDI input. A short Release can finish between display updates.
Note timing previews use a one-second reference
note. The Sustain width is a guide for the held stage.

Time timing sets Attack and Decay in milliseconds. Note timing sets them as percentages of the triggering note's complete duration. Release remains in milliseconds. A 100% note-relative Attack rises across the full note duration; 0% Decay reaches Sustain immediately after Attack.

Hold Shift while dragging an Attack, Decay, or Release millisecond knob for fine adjustment at 100× lower sensitivity. You can press or release Shift during a drag without a jump.

## Voice behavior

| Mode | Behavior |
| --- | --- |
| Per note | Each voice has its own contour. Use for Amplitude, Oscillator Pitch, or Sampler Gain. |
| Retrigger | Each new note restarts one shared contour, including overlapping notes. |
| Legato | The contour starts when a note arrives with no other note held. Overlapping notes keep it running. |

The Envelope lane offers all three modes. Shared track effects cannot use Per note Envelopes. Note-relative Legato keeps the first held note's duration through overlapping notes.

Retrigger and Legato start Release after the last held note ends. Release starts
from the current level, even during Attack or Decay. A clip boundary or the end
of a sample can end the sound before the contour finishes.

Turning an Envelope off removes its contribution while retaining its settings. Removing it retains settings and connections for restoration from the catalog. Use the receiving connection's Amount and power controls to adjust or disable its effect on a particular parameter.

## Modulate an Envelope

Use the secondary dot in the Envelope header to open its modulation controls. Add a connection, select an LFO, Macro, or another Envelope as Source, choose a Destination, and adjust Amount. Parameter dots open these same controls. Connections use the shared power, removal, and reorder gestures.

| Destination | Amount | Effect |
| --- | --- | --- |
| Attack time, Decay time, Release time | ×1/64 to ×64 | ×1 is neutral; ×2 doubles the time; ×0.5 halves it. Times stay within 0–10 seconds. A base time of zero stays zero. |
| Attack · Note, Decay · Note | −100% to +100% of the note | Add percentage points to the saved note fraction and clamp to 0–100%. |
| Sustain | −100% to +100% | Add percentage points and clamp the held level to 0–100%. |
| Attack curve, Decay curve, Release curve | −100% to +100% | Change curve strength within 0–100%. Linear shapes ignore strength. |

The time multiplier describes the effect at full positive source output. For example, ×4 turns a 1-second Decay into 4 seconds at full output; a source at zero leaves the base time unchanged. At half source output, ×4 gives ×2. Bipolar sources can also move time in the opposite direction. Double-click Amount to reset to ×1. Existing projects keep the same sound and knob behavior.

Time and note-relative destinations remain separate, so switching Timing keeps both sets of connections. Attack/Decay use the active Timing setting. Release always uses milliseconds. Timing, Mode, and curve shapes remain discrete controls.

Per note Envelopes can receive per-note and shared sources. Retrigger/Legato Envelopes receive shared sources only. The picker excludes self-connections and feedback loops. Change or remove per-note inputs before selecting a shared Mode.

Modulation keeps the base settings and current stage progress. It does not restart a note or make an idle Envelope start again. Native seeks prepare input history on a worker; long histories can take more time and a new request cancels the old preparation. Hidden history before a trimmed clip and direct loop jumps follow the existing note-state reconstruction rules. Continuous playback and offline export use the same processing.
