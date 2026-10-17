# Projects and export

Projects store the arrangement, instruments, effects, and settings. Audio recordings remain external files. Keep those recordings available when sharing or moving a project.

## Save and open

First Save and Save As ask for a project name and a parent folder. They create a new named project folder. Regular Save uses the current location. Save As preserves the references to the original recordings; it does not copy them into the new project.

When leaving a modified project, choose Save, Discard, or Cancel. A cancelled or failed operation retains the current project. Missing recordings remain visible and play as silence. Files that became shorter supply silence for unavailable sections.

## Project Settings

Use File > Project Settings to change the name, processing rate, buffer size, or [time-stretch engine](../audio/time-stretch.md). Apply accepts valid changes; Cancel or Escape discards the draft.

Processing rates are 44.1, 48, 88.2, 96, 176.4, and 192 kHz; the default is 48 kHz. Buffer sizes are powers of two from 32 to 4096 samples; the default is 512. Smaller buffers can reduce response time but need more frequent processing. Displayed buffer duration is not a measured end-to-end output latency.

Audio settings changes prepare the required audio before replacing the session. Success stops playback so you can restart it with the new settings. Failure or cancellation retains the previous project and playback.

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
