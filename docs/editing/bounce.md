# Bounce clips and tracks

Bounce creates audio with the current clip edits, instrument sound, layer effects,
track effects, track gain and pan included. It keeps Group, Return and Master
processing live. Audio, MIDI and Beat parent tracks are supported.

## Bounce selected clips

1. Select whole clips in Timeline or Editor. Clear any time-range selection first.
2. Choose **Clip > Bounce clips**, use the clip context menu, or press Command+B
   on macOS or Ctrl+B on Windows/Linux.
3. Wait for the Bounce progress dialog. Editing is disabled while it runs.

**Bounce clips** follows **Merge**. A separator places **Stem separation** and
**Normalize gain** in the next group in both the Clip menu and clip context menu.

Bounce creates one Audio track for each source track in the selection. Each new
track contains one clip, with the selected clips joined and their gaps retained.
The clip starts at the first selected clip. Unselected clips stay enabled.

## Bounce selected tracks

1. Select one or more Audio, MIDI, Beat parent or Group tracks.
2. Choose **Track > Bounce tracks**, use the track context menu, or press
   Shift+Command+B on macOS or Shift+Ctrl+B on Windows/Linux.
3. Wait for the Bounce progress dialog.

Track Bounce includes each selected track's complete arrangement. Clip selection,
the time range, the playhead and the transport loop do not limit it. A Group
produces one audio track for its full mix, including nested groups and their
processing. If you also select a track inside that Group, Bounce includes it
only in the Group mix. Empty tracks, empty groups and Returns are skipped with
a notice.

## Result and progress

Each new track appears after its source, with the same parent and color. A Group
bounce appears below the entire Group. Its name starts with the source name
followed by **Bounce**. Source clips remain in place and become disabled,
including clips in nested groups. Existing disabled clips stay
disabled. Undo restores the complete operation in one step; Redo restores the same
audio without rendering again.

The new track starts with no instrument or effects, unity gain and centered pan.
It copies the source's Mute, Solo and sends. Bounce captures authored sound even
when the source track is muted or excluded by Solo; disabled clips and disabled
instrument sources still remain silent. You can change the two tracks'
monitoring controls independently after Bounce.

A Group bounce captures the descendants' Mute and Solo mix. If a descendant
is soloed, the new audio track also starts soloed. Any enclosing Group, Returns
and Master continue to process the project live. The selected Group's own sends
are copied as editable sends. Active descendant sends are captured at their
current levels and continue feeding their live Returns through the bounced clip.

The progress dialog shows preparation, rendering and publication. **Cancel**
stops work before publication. Editing remains disabled until generated files
have been cleaned up. Success and cancellation close the dialog automatically.
Warnings appear as notices. Errors keep the dialog open with details and a
**Close** button. An error or cancellation leaves the original arrangement intact.
Cancel is no longer available once the complete result is being published.

## Sound and practical limits

- Bounce includes releases and effect tails, with at most 10 seconds after the
  last requested clip. Sources or effects without a finite tail bound use that
  ceiling. After rendering, Bounce removes the unused ending and keeps a short
  final fade without shortening the requested clips or their internal gaps.
  Late echoes within the window remain included. Routing audio can require a
  longer ending than the visible main waveform. This normal behavior produces
  no warning. Long sparse
  arrangements can require substantial rendering time and storage.
- Output uses stereo float32 WAV at the project sample rate. It retains levels
  above 0 dBFS without a warning; it does not normalize or dither. Playback and
  integer exports can still clip at these levels.
- Pre-fader sends and existing audio sidechains retain captured audio from their
  original signal stages. Do not remove the original track if other tracks still
  use its identity as a sidechain source. Deleting it makes those connections
  unavailable under the usual missing-source rules.
- Captured routing audio follows the bounced clip's position, trims, splits,
  repeats, gain, channel edits, stretching and transpose. Copying that clip also
  copies its contributions to existing sidechains and captured Return feeds.
  Bouncing it again retains those associations.
- Captured routing stages remain snapshots. Later effects on the bounced track
  do not recreate its original pre-fader signal. Its new fader and pan remain live
  for normal output and sends. Sidechain Post applies its monitoring gates;
  sends continue to bypass Mute and obey Solo.
- Bouncing only part of a track can change interactions in shared nonlinear
  effects. Selected and remaining clips then excite separate processors. Use a
  full track bounce when that interaction matters.
- Source recordings remain unchanged. Save or Save As retains the generated
  audio, including files needed by Undo and Redo. Older application versions that
  lack rendered-source support cannot open these projects.
- Jobs that exceed available memory, frame bounds or the WAV file-size limit
  fail without disabling originals. Shorten the requested arrangement and retry.
