# Features and roadmap

DAW provides audio and MIDI arrangement, layered instruments, effects, mixing, and stereo export. The project format is experimental. See the [user guide](README.md) for instructions and the main [README](../README.md) for platform setup.

## Available features

- Arrange audio and MIDI clips in Timeline. Move, copy, trim, loop, and stretch clips with grid snapping.
- Create MIDI patterns and edit notes in Editor's piano roll.
- Build MIDI instruments from Oscillator, Sampler, Envelope, LFO, Amplitude, and Macro components.
- Process tracks and Master with 3-band EQ, Drive, Bitcrusher, Saturator, Auto-filter, Compressor, OTT, Limiter, Direction Mixer, and Stereo Spread.
- Mix tracks and nested [group buses](editing/track-groups.md) with gain, pan, mute, solo, and shared effects. Use Output to balance the layers of an instrument.
- Save projects and export stereo WAV, MP3, or AAC. Inspect tracks, validate projects or render audio with the [CLI](cli/README.md).

## Current limits

Projects reference external audio recordings. Saving does not bundle those files. Missing recordings remain visible and play as silence. Keep recordings available when moving a project to another computer.

[Undo and redo](editing/undo-redo.md) retain up to 100 project edits for the current session. Sampler supports root-relative playback across the full MIDI note range. Export ends at the arranged project duration and does not append effect tails. Mobile delivery is outside the current release scope.

## Planned work

Planned capabilities include collecting media into project folders for portability, direct waveform editing, beat tracks and a step editor, effects for individual beat lanes, more effects, and custom instrument routing. These features are not available yet.
