# Inspect tracks

```sh
daw-cli --project examples/demo track.list
daw-cli --project examples/demo track.list --name "Bass synth"
daw-cli --project examples/demo track.show --track "name:Bass synth"
```

`track.list` shows the saved order, including collapsed Group descendants and
Return tracks. The name filter returns every exact, case-sensitive match.
`track.show` requires one match. Use `--track id:UUID` to select a track by ID
when names are repeated. Master is the output bus and has no track ID.

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
daw-cli --project ./Song track.add --type audio --name "New audio"
daw-cli --project ./Song track.add --type midi --name "New MIDI"
daw-cli --project ./Song track.add --type beat --name "New beat"
```

Supply both `--name` and `--type`. The command appends an empty root track
and saves its new ID and default settings. MIDI tracks start with a sine-wave
Oscillator. Duplicate names are allowed. A new name must contain text and no
control characters. Track commands work with an existing saved project.

## Rename a track

```sh
daw-cli --project ./Song track.rename --track "name:New MIDI" --name "Lead"
```

A rename preserves the track ID, clips and routing. Setting the current name
again succeeds without writing the manifest. List tracks first if you need an
ID to disambiguate a name. Use `track.rename --help` for the options.

The legacy `tracks [NAME] [--project FOLDER]` command still defaults to the
current folder and shows all name matches. Add with
`tracks --add NAME --type TYPE [--project FOLDER]` in legacy scripts.

[CLI commands](README.md)
