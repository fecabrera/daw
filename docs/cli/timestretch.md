# Time-stretch and pitch tool

A separate command-line comparison tool can process WAV files with the Integrated engine and Rubber Band reference engines. See the [standalone tool reference](../../crates/timestretch-cli/README.md) for build options, reports, comparison scripts and limits.

## Process or compare WAV files

Build with `cargo build -p timestretch-cli --release`. The executable is `target/release/timestretch` (`timestretch.exe` on Windows). It runs without a DAW project, window or audio device.

```sh
timestretch process --input input.wav --engine timestretch --ratio 3/2 \
  --output longer.wav --report longer.json
timestretch compare --input input.wav --ratio 3/2 --sample-rate 48000 \
  --output-dir comparison
```

`process` uses one engine: `timestretch` (Integrated), `rubberband-r2` or `rubberband-r3`. `compare` runs all three and saves their audio, reports and a summary. Reference engines are included in the default build; an Integrated-only build cannot run comparisons.

Inputs support mono or stereo PCM16, PCM24 and float32 WAV. Output uses float32 WAV and keeps the input channel count and headroom. `--ratio` is output duration divided by input duration; use a fraction or decimal from 1/8 to 8. `--sample-rate` accepts 44100, 48000 (default), 88200, 96000, 176400 or 192000 Hz.

Use a new or empty comparison directory. For `process`, add `--overwrite` to permit replacement of output and report files after successful preparation. The input stays unchanged. Ctrl+C cancels a comparison. A failed comparison retains its logs and failure summary. Exit codes are 0 for success, 1 for processing or file errors, and 2 for invalid arguments.

Reports include timing, frame counts and diagnostics. A successful run does not establish audio quality; retain the output for listening. Use `timestretch --help` or `timestretch <command> --help` for the full option list.

## Offline pitch changes in the standalone tool

The separate tool can raise or lower a WAV file's pitch while retaining its duration. Add `--pitch-semitones` to `process` or `compare`. Positive values raise pitch; negative values lower it. Values from −127 to +127 are supported, including fractional semitones.

```sh
timestretch process --input input.wav --engine timestretch --ratio 1 \
  --pitch-semitones -12 --output lower.wav --report lower.json
```

Use `--ratio` to change duration at the same time. A pitch offset of 0 keeps the existing pitch-preserving behavior. Pitch changes are experimental and do not preserve vocal formants. Strong upward shifts can reduce high-frequency content. The tool writes a separate output file and retains the original recording. Timeline provides constant audio transpose in Clip settings. Sampler uses MIDI notes relative to its root note to choose pitch within the same range.

[CLI commands](README.md)
