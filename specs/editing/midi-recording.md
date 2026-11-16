# MIDI recording

R is transient UI routing state, outside project history. Multiple arms are
allowed. The fallback uses one primary selected MIDI track only when no valid
arm exists. Headers place R in a separate group after M/S on the same row. The
implicit fallback uses active text color and ordinary background. Explicit arming
uses the normal active button background.

Record uses the shared transport surface and Lucide Circle icon. Count-in uses
the bundled Lucide Numbers SVG after metronome. Its toggle changes the persistent
app preference; Settings stores enabled state, duration (1/2/4 bars), input,
channel and recording offset. Record and Panic share app/native menu actions.

A worker creates a durable base and journal before the output accepts Record.
Count-in uses saved tempo and signature and holds the recording origin. Starting
while playing skips count-in. Pre-start held keys do not enter capture. The output
acknowledges the actual start and authoritative stop/pause/seal/panic boundaries.
No route at the start boundary seals the empty take. Capture may extend beyond
the existing arrangement. Loop recording is rejected.

Route removals close notes on that target; additions accept only later attacks.
Repeated pitches use physical tokens and FIFO releases. Sustain resolves to note
durations. All tracks use one origin. Convert integer project frames to 960-PPQ
notes with checked arithmetic and minimum one-tick duration. Bound a take to
60 minutes and one million canonical events/notes. Close held notes at the final
accepted frame. No synthetic note-on occurs when a target changes.

`Project::recorded_midi_take` constructs all assets and placements on a clone and
uses `Edit::OverwriteClips` once. Invalid targets, overflow or invalid patterns
reject the whole candidate. Each target has distinct note identities. Covered
clips follow the shared trim/split/remove placement rules. Empty takes are no-ops.

The storage worker writes a durable whole-take checkpoint before UI publication.
The UI verifies the base digest and records one history transition. Background
preparation follows the logical-first MIDI history contract. Failure retains
captured data for retry. No storage work runs on the input broker or audio thread.
Any journal error stops accepting later receipts, interrupts audio and retains
the accepted prefix. See [recovery](../projects/autosave-recovery.md).

Loop overdub, punch, controller editing, MPE, MIDI files and clock sync remain
outside this slice. Native keyboard/latency acceptance remains a separate gate.
