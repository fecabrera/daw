# Features and roadmap

DAW provides audio and MIDI arrangement, layered instruments, effects, mixing, and stereo export. The project format is experimental. See the [user guide](README.md) for instructions and the main [README](../README.md) for platform setup.

## Available features

- Arrange audio and MIDI clips in Timeline. Move, copy, trim, loop, and stretch clips with grid snapping.
- Create MIDI patterns and edit notes in Editor's piano roll.
- Build MIDI instruments from Oscillator, Sampler, Envelope, LFO, Amplitude, and Macro components.
- Process tracks and Master with 3-band EQ, Drive, Auto-filter, Compressor, OTT, and Limiter.
- Mix tracks with gain, pan, mute, and solo. Use Main to balance the layers of an instrument.
- Save projects and export stereo WAV or MP3. Validate or render projects from the command line.

## Current limits

Projects reference external audio recordings. Saving does not bundle those files. Missing recordings remain visible and play as silence. Keep recordings available when moving a project to another computer.

Undo/redo is not available. Sampler plays at the original pitch; root note is descriptive only. Export ends at the arranged project duration and does not append effect tails. Mobile delivery is outside the current release scope.

## Planned work

Planned capabilities include a dedicated audio editor, beat tracks and a step editor, effects for individual beat lanes, more effects, and custom instrument routing. These features are not available yet.
