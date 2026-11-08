# Inspect tracks

```sh
daw-cli tracks
daw-cli tracks "Bass synth"
daw-cli tracks --project examples/demo
daw-cli tracks "Bass synth" --project examples/demo
```

Without `--project`, the command reads `project.json` in the current directory. Use `--project` to inspect another project folder.

Without a name, `tracks` lists all audio, MIDI, beat and group tracks in saved order, including children of collapsed groups. Each row shows its position, ID, type, name, parent group ID (or Master), clip count, track effect count, mute and solo settings. An empty project reports **No tracks.** Master is the output bus and is not listed as a track.

A name selects exact, case-sensitive matches. Names that contain spaces need quotes. If several tracks share the name, the command shows every match in saved order, each with its ID. A name with no match returns exit code 1.

Track details include the parent name and ID, saved gain in dB, pan (−1 left, 0 center, +1 right), mute and solo settings, track effect count, clip count and project rate in Hz. MIDI tracks include their instrument layer count. Beat tracks include subtrack names/IDs, source-layer counts, trigger pitches and output gain/pan. Their subtracks are not separate project tracks. Groups include their direct children. Clip details show IDs, asset IDs, gain in dB, mute settings, and start, exclusive end and length in frames and seconds.

Audio clips also show left/right playback enable states, the L/R swap state,
and Normal or Inverted polarity. Fade in and Fade out show the mode
(Automatic, Fade or Crossfade), saved and effective durations in frames and
milliseconds, Shape and Position percentages matching Clip settings. An
unavailable crossfade is marked inactive with the automatic boundary-fade
fallback. Effective duration reflects metadata limits; this command does not
check whether source files are available. MIDI and beat clips omit audio-only
channel, polarity and fade fields.

## Add an empty track

Save and close the desktop project before using this command. It changes the
saved project and has no synchronization with an open desktop session.

```sh
daw-cli tracks --add "New audio" --type audio --project ./Song
daw-cli tracks --add "New MIDI" --type midi --project ./Song
daw-cli tracks --add "New beat" --type beat --project ./Song
```

Both `--add` and `--type` are required together. Types are `audio`, `midi` and
`beat`. The command appends an empty root-level track with a new ID and the
standard defaults, then saves the project. It creates no clips, patterns or
recordings. Duplicate names are allowed. Names must contain text and cannot
contain control characters. You cannot combine `--add` with an inspection name.
Without `--project`, it edits the project in the current directory. A missing
project is an error; this command does not create a new project. Success prints
the added track's name and ID. Failure to load, validate or save returns exit 1;
invalid arguments return exit 2. The operation has no desktop Undo entry.

The following inspection behavior applies when `--add` is absent.

Names appear in quotes. Control characters are escaped so they cannot change the terminal display. Mute and solo are saved controls; they do not describe the combined audible state of parent buses and other tracks.

Inspection reads and validates project metadata only. It does not change the project, open media, prepare audio or report missing sources. Use [validate](validate.md) to check referenced audio.

[CLI commands](README.md)
