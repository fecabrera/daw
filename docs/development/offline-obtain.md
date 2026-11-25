# Compare offline beat tracking

Run the experimental file comparison from the workspace root.

```sh
cargo run --locked --release -p daw-project --example source_beat_compare -- --input PATH --rate 48000
```

The command prepares one file and returns one JSON record with the production
Ellis-based source BPM, the standard offline OBTAIN result, and preparation
and analysis times in nanoseconds. Both detectors use percussion ambiguity
recovery. OBTAIN also uses targeted tempo selection. Beat positions are
zero-based frames at the reported prepared sample rate. Divide frames by that
rate for seconds. The result lists the
analyzed regions and any tracked runs. The BPM nearest to 100, when a
supported octave alternative exists, is a separate summary field.

Omit `--input` to send one UTF-8 file path per line on standard input and
receive one JSON record per line. Batch output uses input-order IDs and
records per-file errors. Add `--repeats 9` for nine timed analysis calls
after the first call. The detectors share one prepared file and rotate call
order. With the standard two detectors, a single-file error exits with
status 1 and invalid arguments exit with status 2. A musical abstention
appears in a successful record.

## Compare experimental variants

```sh
cargo run --locked --release -p daw-project --example source_beat_compare -- --experiment combined --repeats 9 < paths.txt
```

The `--experiment` option adds a third result with its own status and
timings. The variants share the same prepared input.

| Variant | Behavior |
| --- | --- |
| `legacy` | Run the original offline OBTAIN for a paired comparison. |
| `reuse` | Reuse phase seeds and landmark weights in the original tracker. |
| `onsets` | Add a conditional search using balanced spectral band flux to the original tracker. |
| `rivals` | Add comparison of competing tempo contours to the original tracker. |
| `combined` | Combine the experimental onset search and competing contour checks. |
| `selection` | Run targeted OBTAIN tempo selection before percussion recovery. |

Run `--profile` in a separate pass for read and transform counts, stage
times, context candidates, and the selected period. Profile periods are
in 100 Hz bins. Onset time includes prepared reads. Decode time includes
cumulative recurrence. Use runs without `--profile` for timing comparisons.

## Compare percussion ambiguity recovery

```sh
cargo run --locked --release -p daw-project --example source_beat_compare -- --percussion-recovery --repeats 9 < paths.txt
```

This comparison keeps the benchmark's four result keys. `baseline` and
`obtain` report the pre-recovery detectors. `ellis_recovery` and
`obtain_recovery` report the current defaults. All four runs use the same
prepared input.

See the [offline tracker specification](../../specs/audio/offline-obtain.md)
for supported bounds, status meanings and validation.
