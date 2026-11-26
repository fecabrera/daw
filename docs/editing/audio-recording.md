# Record audio

Record from the selected audio input onto one or more Audio tracks. Each
recorded route becomes a normal, editable audio clip. The take stores the dry
input. You hear the arrangement, count-in, metronome when enabled, and live
MIDI during recording. Use your audio interface's direct monitor if you want
to hear your input. DAW software input monitoring is off.

## Choose input and arm tracks

1. Open **Settings > Playback > Input device** and choose the device that
   receives your signal. Grant the system microphone permission when asked.
2. In Timeline, open the Audio track header's context menu and choose
   **Input**. You can also choose the input under the Mixer strip's R row
   when the selector is visible.
3. Choose one channel, such as **Mono 1**, or an adjacent stereo pair, such
   as **Stereo 1–2**. The default is **Mono 1**. Input routes and R state
   apply to the current editing session.
4. Select an Audio track to make it ready to record when no track is armed.
   Its **R** text highlights. Click **R** to keep recording directed to that
   track when you select another track. Arm several tracks to record them
   together. Turn off all armed tracks to return to the selected track.
   Hover over a recording Audio track's **R** button to check its input peak.

## Record and finish a take

1. Set the playhead and turn Loop off.
2. Use **Transport > Record** or the transport Record button. Starting
   from rest uses the configured count-in. Starting during playback
   begins at the transport position after the input is ready. Changing
   track selection during a take keeps its Audio destinations fixed.
3. To finish while playback continues, press Record again. To finish and
   stop or pause, use Stop or Pause. The app prepares the new clips before
   publishing them together. Undo reverses the whole take.

For a mixed take, explicitly arm the Audio and MIDI tracks before Record.
The selected-track MIDI fallback can still play live during an audio-only
take. It becomes a recording destination when you explicitly arm that
MIDI track. See [MIDI recording](midi-recording.md).

## Save and recover

Save places recorded WAVs in the project folder, including takes recorded
before the first Save. On restart, the [recovery dialog](../projects/recovery.md)
can offer an interrupted take. Recovery keeps complete audio frames through
the latest usable boundary. A failed input or clock reports the interruption
and keeps available captured material.

Software monitoring, loop takes and punch recording are pending features.
Hardware and driver timing vary. Check alignment against a known signal on
your device pair before relying on a timing-critical take.
