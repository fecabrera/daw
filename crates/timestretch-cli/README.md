# Standalone time stretch comparison

Build from the repository with `cargo build -p timestretch-cli --release`. The resulting binary is `target/release/timestretch` (`.exe` on Windows). The tool depends on no DAW crate, project, configuration, graphics, or audio device.

The default build includes Rubber Band R2 and R3 references. It needs the C++ compiler, CMake, and libclang required by the binding. The portable engine does not. Build with `--no-default-features` for an Integrated-only tool; `compare` then fails explicitly because references are unavailable.

```sh
timestretch compare --input input.wav --ratio 3/2 --sample-rate 48000 --output-dir comparison
timestretch process --input input.wav --engine timestretch --ratio 1.5 \
  --output output.wav --report output.json
python3 /path/to/scripts/compare.py --binary /path/to/timestretch \
  --input input.wav --ratio 3/2 --output-dir comparison
python3 /path/to/scripts/benchmark.py --binary /path/to/timestretch \
  --input input.wav --ratios 1/2 1 3/2 2 8 --sample-rates 48000 96000 \
  --warmups 1 --runs 5 --timeout-seconds 600 --output-dir benchmark
```

`compare` processes the same WAV with `timestretch`, `rubberband-r2`, and `rubberband-r3`. It verifies SHA-256 prepared-input identities, engine versions, output lengths, and output file hashes. Each engine runs in an isolated child with a timeout. Ctrl+C cancels and terminates the active comparison child. Failures retain logs and a failed summary. Scripts accept an explicit prebuilt binary and run from any directory without building or downloading dependencies.

Inputs: mono/stereo PCM16, PCM24, or float32 WAV. Processing rates: 44100, 48000 (default), 88200, 96000, 176400, 192000. Ratio is output/input duration, a fraction or decimal between 1/8 and 8. Outputs are float32 WAV with original channel count and preserved headroom. Conversion is identical for all engines. No normalization, limiting, pan, effects, or clip fades are applied.

Use a new or empty comparison/benchmark directory. `process --overwrite` permits replacing output/report files after successful staging, never the input or fixture manifest. Canonical and hard-link aliases are rejected. The success summary is published last. Multiple destination files do not form an atomic filesystem transaction; a partial publication reports the audio path already published. Exit codes: 0 complete processing, 1 processing/I/O/incomplete comparison, 2 invalid arguments.

## Reports and metrics

Reports contain source/prepared/output SHA-256 identities, exact ratio, channels/rate/frame counts, engine revision/options/native versions, build identity, host information, diagnostics, and separate decode/conversion/engine/encoding/measurement timings. Peak RSS is explicitly unavailable in this version; do not interpret it as zero or as engine scratch usage. Benchmark rotates engine order, discards warmups, retains runs/failures, and emits JSON/CSV median/min/max/MAD summaries. R3 is the fixed comparison baseline. File processing success is not a quality acceptance result.

`--fixture-manifest` accepts a JSON object with `schema_version: 1`, `metric_profile: "synthetic-v1"`, and `prepared_sha256` matching the hash from a process report. Include `generator` and `seed` metadata when generating fixtures. Supported measurements:

- `frequency_hz`: known sine frequency. Pitch uses interpolated rising zero crossings in the middle half; require at least 20 cycles. Level compares source/output RMS there. `tone-v1` is also accepted for tone-only manifests.
- `channel_factor`: expected R/L multiplier. Report normalized residual power in dB; exact zero is the string `"-inf"`.
- `stereo_phase_radians`: expected R-minus-L phase for the supplied sine frequency, measured with sine/cosine projections.
- `events`: ordered, non-overlapping `{ "start_frame": ..., "end_frame": ... }` regions, one expected event per region, in prepared-input frames. Use a forward 1 ms mean-square envelope and its first 10% peak crossing; width is the cumulative-energy 10–90% span, ties earliest. Regions map by duration ratio. Missing events or invalid regions fail measurement.

Metrics report their pass/fail flags. Unspecified metrics remain unassessed. Arbitrary files get diagnostics, not invented pitch/transient acceptance. Keep raw outputs for listening. Full corpus performance and listening acceptance remain separate work.

## Reuse and licensing

Keep sibling `timestretch` and `timestretch-cli` directories when extracting these packages. Their manifests specify portable dependencies and have no workspace inheritance. Build with `cargo build --manifest-path /path/to/timestretch-cli/Cargo.toml --release`; scripts and the installed binary have no repository-relative runtime paths.

Original tool/adapters are BSD-2-Clause. Reference-enabled distributions include GPL-2.0-or-later Rubber Band dependencies; ship [notices](licenses) with the binary and retain the applicable obligations. The reference option does not change the standalone core's dependency graph. Reference adapters use public APIs; no Rubber Band implementation material is incorporated in the new engine. See the engine's [provenance record](../timestretch/PROVENANCE.md).

## Processing revisions

`process` and `compare` use Integrated revision 2 and Rubber Band revision 1 by default. For a reproducible earlier Integrated render, pass `process --engine timestretch --revision 1`. Integrated accepts revisions 1 and 2; Rubber Band accepts revision 1. Invalid combinations fail before processing or publication. Each report records the actual revision. Engine output is raw: no loudness matching, normalization, or limiting.
