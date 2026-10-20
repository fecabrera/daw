# Time stretching

Time stretching changes audio duration while retaining pitch. Timeline clips and Sampler use the project's selected stretch engine for duration and pitch changes.

## Stretch a clip

1. Select the clip or group in Timeline or Editor's clip strip.
2. Hold Option on macOS or Ctrl on Windows/Linux and drag an edge to the target duration.
3. Use Shift to bypass snapping if needed.
4. Release to prepare the stretched audio. Escape cancels an active gesture.

Stretching supports durations from one eighth to eight times the source duration. Original recordings stay unchanged. Preparation failure or cancellation keeps the previous arrangement and audio.

Ordinary waveform-edge dragging trims content. Ordinary header-edge dragging repeats it. Use the modifier when the intended result is a change of duration rather than a trim or repeat.

## Transpose an audio clip

Select the clip in Timeline and open **Clip settings**. Drag **Transpose** vertically, or double-click its value and enter a whole number from −48 to +48 semitones. Positive values raise pitch; negative values lower it. Release the drag or press Enter to prepare the change. Escape cancels an active drag or text edit.

Transposing retains the clip duration, trimming, repeats, and original recording. It also works on a stretched clip. The waveform and playback update together after preparation succeeds. Playback keeps the previous audio while preparation runs. Cancel or an error retains the previous settings and audio. Returning to 0 restores the original pitch at the current stretch duration.

Saved audio transpose values now affect playback, including values saved before audio transpose was connected. Missing values remain 0. Large pitch shifts can change timbre or remove high-frequency content; vocal formants are not preserved.

## Choose an engine

Open File > Project Settings and choose Time stretch engine.

| Engine | Use |
| --- | --- |
| Rubber Band | Default for new and existing projects. |
| Integrated (experimental) | Optional engine for evaluation. Quality and performance still need broader listening checks. |

Apply prepares the audio before changing the project. A successful engine change stops playback; press Play to audition it. Cancel or a preparation error retains the old engine and playback. If a saved engine is unavailable, the application reports an error instead of silently substituting another one.

The engine choice saves with the project and applies to audio clip stretching and transpose, including [Sampler duration and note pitch](../instruments/sampler.md). It is not a per-clip choice. Engine changes retain clip positions, source selection, and stretch ratios.

## Current limits

Audio clips support constant transpose in Clip settings. Live pitch automation is not available. Sampler changes pitch relative to its root note, across the full MIDI range. The experimental engine has automated checks, but broader listening, performance, and platform acceptance remain incomplete. Use representative audio to judge it for a project.

Preparation time and memory use depend on the original recording's length, including when a clip uses only a trimmed part of it. Matching clip requests share prepared audio.

A separate command-line comparison tool can process WAV files with the Integrated engine and Rubber Band reference engines. See its [tool guide](../../crates/timestretch-cli/README.md) for commands and reports.

## Offline pitch changes in the standalone tool

The separate tool can raise or lower a WAV file's pitch while retaining its duration. Add `--pitch-semitones` to `process` or `compare`. Positive values raise pitch; negative values lower it. Values from −127 to +127 are supported, including fractional semitones.

```sh
timestretch process --input input.wav --engine timestretch --ratio 1 \
  --pitch-semitones -12 --output lower.wav --report lower.json
```

Use `--ratio` to change duration at the same time. A pitch offset of 0 keeps the existing pitch-preserving behavior. Pitch changes are experimental and do not preserve vocal formants. Strong upward shifts can reduce high-frequency content. The tool writes a separate output file and retains the original recording. Timeline provides constant audio transpose in Clip settings. Sampler uses MIDI notes relative to its root note to choose pitch within the same range.
