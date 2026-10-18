# Time stretching

Time stretching changes audio duration while retaining pitch. Timeline clips and Sampler use the project's selected stretch engine.

## Stretch a clip

1. Select the clip or group in Timeline or Editor's clip strip.
2. Hold Option on macOS or Ctrl on Windows/Linux and drag an edge to the target duration.
3. Use Shift to bypass snapping if needed.
4. Release to prepare the stretched audio. Escape cancels an active gesture.

Stretching supports durations from one eighth to eight times the source duration. Original recordings stay unchanged. Preparation failure or cancellation keeps the previous arrangement and audio.

Ordinary waveform-edge dragging trims content. Ordinary header-edge dragging repeats it. Use the modifier when the intended result is a change of duration rather than a trim or repeat.

## Choose an engine

Open File > Project Settings and choose Time stretch engine.

| Engine | Use |
| --- | --- |
| Rubber Band | Default for new and existing projects. |
| Integrated (experimental) | Optional engine for evaluation. Quality and performance still need broader listening checks. |

Apply prepares the audio before changing the project. A successful engine change stops playback; press Play to audition it. Cancel or a preparation error retains the old engine and playback. If a saved engine is unavailable, the application reports an error instead of silently substituting another one.

The engine choice saves with the project and applies to audio stretching, including [Sampler duration and note pitch](../instruments/sampler.md). It is not a per-clip choice. Engine changes retain clip positions, source selection, and stretch ratios.

## Current limits

Timeline clips have no pitch control. Sampler changes pitch relative to its root note, across the full MIDI range. The experimental engine has automated checks, but broader listening, performance, and platform acceptance remain incomplete. Use representative audio to judge it for a project.

A separate command-line comparison tool can process WAV files with the Integrated engine and Rubber Band reference engines. See its [tool guide](../../crates/timestretch-cli/README.md) for commands and reports.

## Offline pitch changes in the standalone tool

The separate tool can raise or lower a WAV file's pitch while retaining its duration. Add `--pitch-semitones` to `process` or `compare`. Positive values raise pitch; negative values lower it. Values from −127 to +127 are supported, including fractional semitones.

```sh
timestretch process --input input.wav --engine timestretch --ratio 1 \
  --pitch-semitones -12 --output lower.wav --report lower.json
```

Use `--ratio` to change duration at the same time. A pitch offset of 0 keeps the existing pitch-preserving behavior. Pitch changes are experimental and do not preserve vocal formants. Strong upward shifts can reduce high-frequency content. The tool writes a separate output file and retains the original recording. Timeline has no pitch control. Sampler uses MIDI notes relative to its root note to choose pitch within the same range.
