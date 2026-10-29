# Projects and export

Projects store the arrangement, instruments, effects, and settings. Normal imports reference external audio files. Keep those recordings available when sharing or moving a project. Included recordings, when present, travel inside the project folder. Use source collection to include external recordings.

## Save and open

First Save and Save As ask for a project name and a parent folder. They create a new named project folder. Regular Save uses the current location. It writes the project state from when you pressed Save in the background. The workspace stays visible and editable, and the status bar shows Saving project. Changes made during the write remain marked as unsaved; save again to include them. File menu actions are unavailable until the write finishes. Save As keeps external recordings as references and copies already-included recordings into the new project. It also copies project-owned sources retained only by Undo/Redo. Generated [stems](../audio/stem-separation.md) are copied into the new project folder, including temporary stems created before the first save.

When leaving a modified project, choose Save, Discard, or Cancel. A cancelled or failed operation retains the current project. Missing recordings remain visible and play as silence. Files that became shorter supply silence for unavailable sections.

If Save As fails because an included recording or generated stem is missing, restore that file and retry the same project name and location. A failed Save As removes its empty new project folders and retains unrelated contents.

Saving fails if a directory occupies the `project.json` destination. The directory and its contents remain intact. Move that directory out of the project folder or choose another save location, then retry.

## Collect project sources

For headless collection, save and close the desktop project, then use the [collect-sources CLI command](../cli/collect-sources.md). It collects saved assets without desktop Undo/Redo history.

Collect sources before sharing or moving a project:

1. Save the project first if it has no project folder yet.
2. Choose **File > Collect project sources**, in the same File menu group as **Project Settings**.
3. Watch the status bar for checking, copying and saving progress. Collection starts immediately and also saves current changes, without a confirmation prompt.
4. Wait for **Project sources collected and saved** in the status bar. Any warnings appear there too. A dialog opens only if an error occurs. Restore missing or changed originals before retrying.

Collection copies complete original WAV, MP3 and AAC files without trimming, resampling or re-encoding. It includes unused assets, Sampler/Granulizer inputs and sources retained by the active Undo/Redo history. Original files stay unchanged. Undo/Redo history itself is not stored in the project.

Temporary copies need additional space; free space is not measured. When no new copies are needed, collection still saves the project.

Editing and file actions remain blocked while collection runs. They become available again automatically after completion or cancellation. Playback can continue from prepared audio. Use **Cancel** in the status bar to stop collection before saving. After saving succeeds, cancellation does not undo collection; the status bar shows completion and any warnings. Errors stay in a dialog until you close it.

Move the whole project folder, including `assets/` and `stems/`, rather than only `project.json`. Later imports stay external until you collect again. Regular Save does not collect them automatically. Save As copies media already included in the project and protects sources retained only by the active Undo/Redo history.

A failure before saving retains the previous project and playback. If the report says sources were saved but active state was retained, close the report and reopen the saved destination. If commit could not be confirmed, keep all destination files and the candidate manifest named in the report. Recovery is not automatic. Collection does not remove unused media or crash leftovers.

## Project Settings

Use File > Project Settings to change the name, processing rate, output bit depth, buffer size, or [time-stretch engine](../audio/time-stretch.md). Apply accepts valid changes; Cancel or Escape discards the draft.

Each project saves its own audio quality. Processing rates are 44.1, 48, 88.2, 96, 176.4, and 192 kHz; the initial default is 48 kHz. Output bit depth selects an integer or float device-stream format; the initial default is 32-bit float. Buffer sizes are powers of two from 32 to 4096 samples; the default is 512. Smaller buffers can reduce response time but need more frequent processing. Displayed buffer duration is not a measured end-to-end output latency.

Audio settings changes prepare the required audio before replacing the session. Success stops playback so you can restart it with the new settings. Failure or cancellation retains the previous project and playback.

## Application Settings

Open Settings from the app-name menu on macOS or File on Windows/Linux. Use Command+, on macOS or Ctrl+, on Windows/Linux.

Settings groups controls into four categories:

- **Appearance**: titlebar style where supported, theme, and accent color.
- **Playback**: input/output devices, audio quality defaults, and seek cache profile.
- **MIDI**: how notes are divided when splitting clips.
- **Stem Separation**: local model selection and validation.

Choose **Dark**, **Light**, or **Mist** under **Appearance > Theme**. Light uses near-white backgrounds, white panels, and pale grey separators. Mist uses cool grey backgrounds and pale panels. All themes keep your selected accent and clip colors.

On Windows, **Appearance > Titlebar > Custom** combines the menu, project title, and view buttons in the titlebar. A continuous divider separates this row from playback controls in restored and maximized windows. **Native** is the default titlebar style.

Changes apply on this computer and are retained across restarts. Close retains your choices. These preferences are separate from Project Settings.

## Audio devices

The main **Settings** window exposes **Output device** and **Input device** under **Playback**. Select an output device and, if needed, save an input-device choice. **System default** uses the operating system's default device when the next stream opens. **Refresh devices** updates the list. An unavailable saved device is marked in the list. Output selection tests the replacement before saving it. Failure keeps the previous device and shows an error in Settings; success stops playback and keeps the playhead. Input selection saves immediately without interrupting playback.

Input selection is saved for future recording. The app does not yet record or monitor input.

Open **Audio quality defaults…** to choose concrete values for new projects: sample rate, output bit depth and buffer size. Initial values are **48 kHz, 32-bit float and 512 samples**. Apply saves these defaults; Cancel or Escape discards the draft, with Escape closing an open dropdown first. Applying defaults leaves the current project and playback unchanged. You can set defaults without a connected audio device.

New blank projects copy these values. Opened projects use their saved **Project Settings**, including output bit depth. To change the current project, use **File > Project Settings**. Older projects without a saved output format use 32-bit float; older projects without a buffer value use 512 samples. Application defaults do not replace saved project values.

The output device must support the project's output bit depth. If playback reports an unsupported format, choose another output bit depth in Project Settings or another device in Settings. Device output uses sample-rate conversion when needed and can adapt the hardware buffer size. These adaptations do not rewrite project quality. The driver may mix or convert the stream to a different physical hardware format. Internal processing remains floating point, and export encoding remains separate.

Advice below these controls uses the same smaller grey text as separation model information.

## Seek cache profiles

Open application **Settings** and choose **Playback > Seek cache**. The choice applies on this computer and is retained across restarts. **Balanced** is the default. It is separate from Project Settings and is not saved in the project.

| Profile | Checkpoint memory limit | Background behavior and tradeoff |
| --- | --- | --- |
| Light | 32 MiB | Builds while stopped or paused, with a longer wait after editing. Lower background load; seeks during playback can need more preparation. |
| Balanced | 128 MiB | Builds in small slices, including during playback. Moderate background load and seek coverage. |
| Responsive | 256 MiB | Starts sooner after editing and spends more time building coverage. Higher CPU and memory use; more checkpoints can remain available. |

All profiles preserve exact processing history. Balanced and Responsive can prepare two tracks at a time. Light prepares tracks in sequence to reduce background load. Small workloads and computers with limited processing or memory capacity use sequential preparation automatically. Playback changes and explicit seeks take priority over optional caching. Cache building waits while a pointer gesture is active and yields briefly when callback timing shows playback pressure. Complex projects can use fewer checkpoints to stay within processing-state limits. Working renderer state uses additional memory beyond the checkpoint limit.

Changing profile keeps valid checkpoints where possible and does not stop playback or add an undo step. A smaller profile can discard checkpoints; a larger profile may need more preparation to build denser coverage. A first or distant seek can still take time, especially just after an edit. Use Light to reduce background load, or Responsive when frequent seeks matter and the computer has spare capacity. Parallel preparation helps most when several tracks have substantial processing. One expensive track or a heavy Master chain can limit the benefit.

## Split MIDI notes

Open application Settings and choose **MIDI > Split MIDI notes**. Split into two is the default. A MIDI note that crosses the playhead becomes two notes. Preserve first keeps that note on the clip before the playhead and does not continue it on the new clip. The choice applies on this computer and is kept across restarts. It is not saved in the project. Audio clips are unaffected.

## Memory and large projects

Audio preparation uses a shared 2 GiB budget for decoded samples and working buffers. Long files, high processing rates, and stretched copies need more space. If preparation exceeds the budget, shorten the source, use a lower processing rate, or close an unused project and try again. A failed import or settings change retains the current project and playback. This budget is not a limit on track count or total application memory.

Hover over the processing-rate and buffer-size label in the status bar to see estimated memory use. Undo history and seek checkpoints have separate limits. When audio memory is nearly full, starting a background operation can remove older undo/redo steps; a notice reports this change. The current project stays intact.

Playback preparation also limits retained processing state to an estimated 512 MiB. If it reports that this limit is exceeded, current playback continues. Reduce instrument voices or effects, or retry after pending work finishes. These estimates exclude some system, graphics and temporary memory.

The app prepares exact seek history in the background before playback and between requests. A first or distant seek can still take time if that preparation is incomplete. Current playback continues until the requested position is ready; the app does not substitute approximate effect or modulation history.

Repeated imports of the same unchanged file can share audio storage. They remain separate assets and clips. If a file changes while it is being prepared, retry the operation after the file has finished saving.

## Export

Choose an export destination and format. WAV supports PCM16, PCM24, and float32 at the project rate; PCM24 is the default. MP3 supports 128, 192, 256, and 320 kbps; the default is 192 kbps. MP3 uses 48 kHz when the project rate is higher.

AAC exports stereo AAC-LC in an ADTS `.aac` file. Choose a target bitrate of 128,
192, 256, or 320 kbps; the default is 192 kbps. Frame sizes vary with the audio.
Projects above 48 kHz are converted to 48 kHz before encoding. The dialog and
completion report show the output rate. AAC export uses the bundled encoder.
It does not require FFmpeg. M4A/MP4 export is unavailable.

ADTS does not store the original sample count or gapless trim information. AAC
exports include codec priming and a padded final frame. Reimported AAC clips can
therefore start with a short delay and last slightly longer than the arrangement.
Use WAV when exact sample alignment is required.

Export includes the arranged project duration and does not append effect tails. It follows the same mute, solo, pan, and effect settings as playback. Missing sources produce warnings and silence. Integer WAV, MP3, and AAC clamp clipping; float32 WAV retains headroom and reports levels above 0 dBFS. No automatic normalization is applied.

Progress and completion appear in the status dialog. An existing destination requires replacement confirmation. Cancellation or failure leaves the previous destination intact.

## Command line

Use the [CLI guide](../cli/README.md) to inspect tracks, validate projects, collect sources and export a mix without opening the desktop app.

## Stem-separation models

Choose and validate local model files under **Settings > Stem Separation**. Apple Silicon macOS supports CoreML, CPU, or both; Windows and Linux use CPU. These paths apply on this computer and are not saved in projects. See [Stem separation](../audio/stem-separation.md) for supported models and the separation workflow.
