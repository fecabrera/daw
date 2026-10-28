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

Integrated improves separation between nearby bass notes when stretching or transposing. Bass-heavy drum mixtures use a shorter analysis window to limit attack smearing. Bass-heavy audio can take longer to prepare. Drum attack protection remains active, and stereo channels retain their relationship. The engine remains experimental.

Projects saved with Integrated revisions 2 or 3 upgrade automatically when opened and rebuild their prepared audio. A warning identifies the upgrade. The sound can change. Opening leaves the saved file unchanged; Save stores the new version. Original recordings stay unchanged.

The engine choice saves with the project and applies to audio clip stretching and transpose, including [Sampler duration and note pitch](../instruments/sampler.md). It is not a per-clip choice. Engine changes retain clip positions, source selection, and stretch ratios.

## Current limits

Audio clips support constant transpose in Clip settings. Live pitch automation is not available. Sampler changes pitch relative to its root note, across the full MIDI range. The experimental engine has automated checks, but broader listening, performance, and platform acceptance remain incomplete. Use representative audio to judge it for a project.

Preparation time and memory use depend on the original recording's length, including when a clip uses only a trimmed part of it. Matching clip requests share prepared audio.

Use the separate [command-line tool](../cli/timestretch.md) to compare engines or change the duration and pitch of a WAV file.
