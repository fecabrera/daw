# Time stretching

Time stretching changes audio duration while retaining pitch. Timeline clips and Sampler use the project's selected stretch engine for duration and pitch changes.

## Stretch a clip

1. Select the clip or group in the Timeline arrangement or editor pane.
2. Hold Option on macOS or Ctrl on Windows/Linux and drag an edge to the target duration. Holding the modifier hides and disables fade/crossfade handles and midpoint controls so they cannot intercept the stretch gesture.
3. Use Shift to bypass snapping if needed.
4. Release to prepare the stretched audio. Escape cancels an active gesture.

Stretching supports durations from one eighth to eight times the source duration. Saved fade durations scale with the clip duration; their curve shapes stay the same. Original recordings stay unchanged. Preparation failure or cancellation keeps the previous arrangement and audio.

Wait for project audio to finish before stretching audio clips or a selection containing both audio and MIDI clips. Releasing such a stretch while preparation is running, paused or failed keeps the previous arrangement. If preparation is paused or failed, use Retry first. MIDI-only stretching remains available.

Ordinary waveform-edge dragging trims content. Ordinary header-edge dragging repeats it. Use the modifier when the intended result is a change of duration rather than a trim or repeat.

Press or release the modifier during an edge drag to switch modes immediately.
The preview updates at the current pointer position, even without further pointer
movement. Releasing the modifier restores trim for a waveform edge or loop for a
header edge. The modifier state at mouse release selects the action. A drag that
starts as a shared-boundary trim keeps that paired trim gesture.

## Transpose an audio clip

Select the clip in Timeline and open **Clip settings**. Drag **Transpose** vertically, or double-click its value and enter a whole number from −48 to +48 semitones. Positive values raise pitch; negative values lower it. Release the drag or press Enter to prepare the change. Escape cancels an active drag or text edit.

Transposing retains the clip duration, trimming, repeats, and original recording. It also works on a stretched clip. The waveform and playback update together after preparation succeeds. Playback keeps the previous audio while preparation runs. Cancel or an error retains the previous settings and audio. Returning to 0 restores the original pitch at the current stretch duration.

Saved audio transpose values now affect playback, including values saved before audio transpose was connected. Missing values remain 0. Large pitch shifts can change timbre or remove high-frequency content; vocal formants are not preserved.

Audio clips support constant transpose in Clip settings. Live pitch automation is not available. Sampler changes pitch relative to its root note, across the full MIDI range. Broader listening, performance, and platform checks remain open for revision 8.

## Choose an engine

Open File > Project Settings and choose Time stretch engine.

| Engine | Use |
| --- | --- |
| Rubber Band | Default for new and existing projects. |
| Integrated (experimental) | Revision 8. Use representative audio to audition it for your project. |

Apply prepares the audio before changing the project. A successful engine change stops playback; press Play to audition it. Cancel or a preparation error retains the old engine and playback. If a saved engine is unavailable, the application reports an error instead of silently substituting another one.

Integrated revision 8 improves how the engine stretches gradual pitch
changes and sustained sounds. It also preserves the timing of gradual fade-ins.
Long stretches can still change the texture of low notes and layered loops.

Projects saved with Integrated revisions 2 through 7 rebuild their prepared audio
when opened. The **Time stretch upgrade** dialog appears after preparation.
The sound can change. Use **Save** to store revision 8.

Set **Settings > Time stretch > Default engine** to choose the engine
for new projects. Rubber Band is the initial default. Changing this preference
does not change the current project; opened projects keep their saved engine.

The engine choice saves with the project and applies to audio clip stretching and transpose, including [Sampler duration and note pitch](../instruments/sampler.md). It is not a per-clip choice. Engine changes retain clip positions, source selection, and stretch ratios.

Use the separate [command-line tool](../cli/timestretch.md) to compare engines or change the duration and pitch of a WAV file.

## Current limits

Preparation time and memory use depend on the original recording's length, including when a clip uses only a trimmed part of it. Matching clip requests share prepared audio.
