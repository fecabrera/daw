# Live MIDI input

Open Application Settings and choose a MIDI input and channel in the MIDI section.
Choose All channels to accept all 16 MIDI channels. Off disconnects the input.
The device selection belongs to the app, not the project.

Select a MIDI track to play its instrument. Enable **R** on one or more MIDI
tracks to play those tracks together. R sits in its own button group after M/S
on the same row. When any track has R enabled, track selection does not change
the input targets. When no track has R enabled, the selected MIDI track receives
input. Its R text has the active color with the regular button background.
Selecting an audio, beat, group or Return track gives no fallback MIDI target.
R and track selection do not modify the project or enter Undo history.

Input uses each track's instrument, effects, sends and mix controls. It works
while stopped and alongside arrangement playback. Moving the playhead from the arrangement ruler or the Timeline editor’s audio or MIDI ruler while stopped or paused does not audition the arrangement, including with Auto-filter enabled. Live keyboard input remains available. Note velocity and sustain
pedal affect playback. Repeated presses of the same key release in press order.

New Sampler pitches prepare in the background. A target whose exact pitch is
not ready stays silent for that press. Press the key again after preparation;
held notes do not start late. Recording still captures the performance.

Stop, loss of window focus, device disconnection, and queue pressure release live
voices. **Transport > Panic** also releases live voices and ends an active take.
After a device failure, select the input again in Settings. The app does not
silently connect a different device with the same name.

See [MIDI recording](../editing/midi-recording.md) for count-in and capture.
