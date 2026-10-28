# Inspect tracks

```sh
daw-cli tracks
daw-cli tracks "Bass synth"
daw-cli tracks --project examples/demo
daw-cli tracks "Bass synth" --project examples/demo
```

Without `--project`, the command reads `project.json` in the current directory. Use `--project` to inspect another project folder.

Without a name, `tracks` lists all audio, MIDI and group tracks in saved order, including children of collapsed groups. Each row shows its position, ID, type, name, parent group ID (or Master), clip count, track effect count, mute and solo settings. An empty project reports **No tracks.** Master is the output bus and is not listed as a track.

A name selects exact, case-sensitive matches. Names that contain spaces need quotes. If several tracks share the name, the command shows every match in saved order, each with its ID. A name with no match returns exit code 1.

Track details include the parent name and ID, saved gain in dB, pan (−1 left, 0 center, +1 right), mute and solo settings, track effect count, clip count and project rate in Hz. MIDI tracks include their instrument layer count. Groups include their direct children. Clip details show IDs, asset IDs, mute settings, and start, exclusive end and length in frames and seconds.

Names appear in quotes. Control characters are escaped so they cannot change the terminal display. Mute and solo are saved controls; they do not describe the combined audible state of parent buses and other tracks.

Inspection reads and validates project metadata only. It does not change the project, open media, prepare audio or report missing sources. Use [validate](validate.md) to check referenced audio.

[CLI commands](README.md)
