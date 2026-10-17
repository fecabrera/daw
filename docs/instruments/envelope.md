# Envelope

An Envelope changes a control value over a note's life. Add it from the Instrument component catalog, then assign it in a receiving unit's [modulation controls](../modulation/README.md). Adding it alone makes no connection.

## Stages and timing

Attack raises the value, Decay moves it toward Sustain, Sustain holds it while the note continues, and Release lowers it after note-off. Curve selectors change the stage shapes.

Time timing sets Attack and Decay in milliseconds. Note timing sets them as percentages of the triggering note's complete duration. Release remains in milliseconds. A 100% note-relative Attack rises across the full note duration; 0% Decay reaches Sustain immediately after Attack.

## Voice behavior

| Mode | Behavior |
| --- | --- |
| Per note | Each voice has its own contour. Use for Amplitude, Oscillator Pitch, or Sampler Gain. |
| Retrigger | Each new note restarts one shared contour, including overlapping notes. |
| Legato | The contour starts when a note arrives with no other note held. Overlapping notes keep it running. |

Main offers Retrigger and Legato. Shared track effects cannot use Per note Envelopes. Note-relative Legato keeps the first held note's duration through overlapping notes.

Turning an Envelope off removes its contribution while retaining its settings. Removing it retains settings and connections for restoration from the catalog. Use the receiving connection's Amount and power controls to adjust or disable its effect on a particular parameter.
