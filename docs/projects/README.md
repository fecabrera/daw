# Projects and export

Projects store the arrangement, instruments, effects, and settings. Audio recordings remain external files. Keep those recordings available when sharing or moving a project.

## Save and open

First Save and Save As ask for a project name and a parent folder. They create a new named project folder. Regular Save uses the current location. It writes the project state from when you pressed Save in the background. The workspace stays visible and editable, and the status bar shows Saving project. Changes made during the write remain marked as unsaved; save again to include them. File menu actions are unavailable until the write finishes. Save As preserves the references to the original recordings; it does not copy them into the new project.

When leaving a modified project, choose Save, Discard, or Cancel. A cancelled or failed operation retains the current project. Missing recordings remain visible and play as silence. Files that became shorter supply silence for unavailable sections.

## Project Settings

Use File > Project Settings to change the name, processing rate, buffer size, or [time-stretch engine](../audio/time-stretch.md). Apply accepts valid changes; Cancel or Escape discards the draft.

Processing rates are 44.1, 48, 88.2, 96, 176.4, and 192 kHz; the default is 48 kHz. Buffer sizes are powers of two from 32 to 4096 samples; the default is 512. Smaller buffers can reduce response time but need more frequent processing. Displayed buffer duration is not a measured end-to-end output latency.

Audio settings changes prepare the required audio before replacing the session. Success stops playback so you can restart it with the new settings. Failure or cancellation retains the previous project and playback.

## Seek cache profiles

Open application **Settings** and choose **Seek cache**. The choice applies on this computer and is retained across restarts. **Balanced** is the default. It is separate from Project Settings and is not saved in the project.

| Profile | Checkpoint memory limit | Background behavior and tradeoff |
| --- | --- | --- |
| Light | 32 MiB | Builds while stopped or paused, with a longer wait after editing. Lower background load; seeks during playback can need more preparation. |
| Balanced | 128 MiB | Builds in small slices, including during playback. Moderate background load and seek coverage. |
| Responsive | 256 MiB | Starts sooner after editing and spends more time building coverage. Higher CPU and memory use; more checkpoints can remain available. |

All profiles preserve exact processing history. Playback changes and explicit seeks take priority over optional caching. Cache building waits while a pointer gesture is active and yields briefly when callback timing shows playback pressure. Complex projects can use fewer checkpoints to stay within processing-state limits. Working renderer state uses additional memory beyond the checkpoint limit.

Changing profile keeps valid checkpoints where possible and does not stop playback or add an undo step. A smaller profile can discard checkpoints; a larger profile may need more preparation to build denser coverage. A first or distant seek can still take time, especially just after an edit. Use Light to reduce background load, or Responsive when frequent seeks matter and the computer has spare capacity.

## Memory and large projects

Audio preparation uses a shared 2 GiB budget for decoded samples and working buffers. Long files, high processing rates, and stretched copies need more space. If preparation exceeds the budget, shorten the source, use a lower processing rate, or close an unused project and try again. A failed import or settings change retains the current project and playback. This budget is not a limit on track count or total application memory.

Hover over the processing-rate and buffer-size label in the status bar to see estimated memory use. Undo history and seek checkpoints have separate limits. When audio memory is nearly full, starting a background operation can remove older undo/redo steps; a notice reports this change. The current project stays intact.

Playback preparation also limits retained processing state to an estimated 512 MiB. If it reports that this limit is exceeded, current playback continues. Reduce instrument voices or effects, or retry after pending work finishes. These estimates exclude some system, graphics and temporary memory.

The app prepares exact seek history in the background before playback and between requests. A first or distant seek can still take time if that preparation is incomplete. Current playback continues until the requested position is ready; the app does not substitute approximate effect or modulation history.

Repeated imports of the same unchanged file can share audio storage. They remain separate assets and clips. If a file changes while it is being prepared, retry the operation after the file has finished saving.

## Export

Choose an export destination and format. WAV supports PCM16, PCM24, and float32 at the project rate; PCM24 is the default. MP3 supports 128, 192, 256, and 320 kbps; the default is 192 kbps. MP3 uses 48 kHz when the project rate is higher.

Export includes the arranged project duration and does not append effect tails. It follows the same mute, solo, pan, and effect settings as playback. Missing sources produce warnings and silence. Integer WAV and MP3 clamp clipping; float32 WAV retains headroom and reports levels above 0 dBFS. No automatic normalization is applied.

Progress and completion appear in the status dialog. An existing destination requires replacement confirmation. Cancellation or failure leaves the previous destination intact.

## Command line

Validate or export without opening a window or audio device. Run these commands from the repository root after building the CLI:

```sh
cargo run -p daw-cli -- validate --project examples/demo
cargo run -p daw-cli -- render --project examples/demo --output /tmp/daw-export.wav
```

Choose your own output path. An existing file needs the explicit `--overwrite` option. The CLI reports warnings and export errors in the terminal. See the main [README](../../README.md#build-and-run) for setup.
