# Record MIDI

1. Choose the MIDI input and channel under **Settings > MIDI**.
2. Select a MIDI track, or enable R on several MIDI tracks.
3. Place the playhead at the recording start and disable Loop.
4. Press the circle Record button in the transport, or choose Transport > Record.
5. Play the notes. Press Record again to finish the take. Stop and Pause also finish it.

Choose Transport > Count-in or use the Numbers button after the metronome. Set its
length to 1, 2 or 4 bars under **Settings > Recording**. The toggle saves an application preference. Count-in runs only when starting from stopped.
The playhead stays at the recording start during count-in. The click sounds even
if the playback metronome is off. Release and press keys again at the start;
notes held before capture do not become recorded attacks.

All R-enabled MIDI tracks receive the same fresh key presses. With none enabled,
the selected MIDI track receives them. Removing a target closes its held recorded
notes. Adding a target receives future presses. Sustained notes close when the
pedal or recording ends. Sustain becomes note duration; controller lanes are not
created. Notes retain their velocity and stay editable in the piano roll.

A take creates clips from one common start and replaces covered clip ranges using
the normal placement rules. All tracks enter Undo history as one edit. An empty
take does not create an edit. Save and export use the recorded patterns. If sample
preparation is pending, the editor retains the take while playback keeps its
previous prepared version until the new version is ready.

**Recording offset** under **Settings > Recording** shifts captured notes by
up to 200 ms in either direction. Positive values place them later. It does not delay live monitoring.
Notes cannot start before the take origin. Calibrate with your keyboard and audio
device; the app does not measure their latency automatically.

Project, tempo, device and processing changes are guarded during capture. Track
selection and R remain available. Loop overdub, punch, MIDI file exchange,
continuous controllers, expression and MIDI clock synchronization are not supported.

Capture runs outside painting and writes a recoverable journal. A failure ends
the take at the accepted prefix and releases live notes. A failed publication
retains recovery data. Choose **Retry MIDI take publication** under
**Settings > Recording** or recover after restarting. See [Recovery](../projects/recovery.md).

Count-in uses the tempo and signature at the recording start. Recorded notes
follow the song tempo map. The map remains fixed until the take ends.
