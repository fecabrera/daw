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

The engine choice saves with the project and applies to audio stretching, including [Sampler duration](../instruments/sampler.md). It is not a per-clip choice. Engine changes retain clip positions, source selection, and stretch ratios.

## Current limits

Pitch shifting is not available through this workflow. The experimental engine has automated checks, but broader listening, performance, and platform acceptance remain incomplete. Use representative audio to judge it for a project.

A separate command-line comparison tool can process WAV files with the Integrated engine and Rubber Band reference engines. See its [tool guide](../../crates/timestretch-cli/README.md) for commands and reports.
