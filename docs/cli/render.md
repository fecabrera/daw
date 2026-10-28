# Export a project

```sh
daw-cli render --project examples/demo --output mix.wav
daw-cli render --project examples/demo --output mix.wav --codec float32 --overwrite
daw-cli render --project examples/demo --output mix.mp3 --bitrate 320
daw-cli render --project examples/demo --output mix.aac --bitrate 192
```

`--project` and `--output` are required. Choose your own output path. Export uses the saved project mix, including track, group and Master processing. It does not save project changes.

| Option | Values and default |
| --- | --- |
| `--format` | `wav`, `mp3`, `aac`. Inferred from the output extension; otherwise WAV. Known extensions must match the format. |
| `--codec` | WAV only: `pcm16`, `pcm24` (default), `float32`. |
| `--bitrate` | MP3 or AAC only: `128`, `192` (default), `256`, `320` kbps. |
| `--overwrite` | Permit replacement of an existing destination after successful export. Omit to retain an existing file. |

All formats export stereo. WAV uses the project rate. MP3 and AAC convert project rates above 48 kHz to 48 kHz. AAC writes AAC-LC in `.aac` files; `.m4a` and `.mp4` are unsupported. Integer WAV, MP3 and AAC clamp levels above full scale. Float32 WAV retains headroom. Warnings report clipping and missing or shortened sources.

An empty project, invalid media or processing error returns exit code 1. Incompatible format options return exit code 2. Export protects source files and the project manifest. Failure leaves an existing destination intact. Warnings do not fail a completed export.

See [Project export](../projects/README.md#export) for desktop export controls.

[CLI commands](README.md)
