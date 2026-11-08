# Revision-5 private Splice audition preparation — 2026-11-08

Historical revision-5 pack. Use the [revision-6 report](transient-harmonic-r6.md)
and its regenerated packs for the current detector.

Status: **awaiting human listening**. No listener scores or subjective quality
judgment have been generated. This is a prepared two-detector comparison, not
a passed listening-acceptance gate.

## Scope and source protection

The user authorized read-only use of their Splice library. Six existing recordings
were read; all excerpts/renders, private paths, source hashes and mappings are in
ignored work storage. No source was modified, renamed or deleted. SHA-256 checks
before and after preparation match for all six files. Audio is not committed or
redistributed. Earlier public samples and intermediate R5 packs were not used for
this final corpus; `splice-r5-current` records the final R5 detector result, superseded by R6.

## Panel

| Private case ID | Material | Source excerpt | Output channels/rate |
| --- | --- | --- | --- |
| drums-resampled | drums resampled | 10.0 s | 2 / 48 kHz |
| drums-clean | drums clean | 10.0 s | 2 / 48 kHz |
| bass-guitar | bass guitar | 10.0 s | 2 / 48 kHz |
| pluck | pluck | 10.0 s | 2 / 48 kHz |
| pad | pad | 10.0 s | 2 / 48 kHz |
| mixed | mixed | 10.0 s | 2 / 48 kHz |

Ratios are 3/4, 3/2 and 2/1: 18 cases, 36 raw detector renders. Each case has
A/B audition WAVs plus the original reference. The old Energy detector is
`energy-r4`; the revision-5 default Combined detector is `energy-spectral-v4`.
No pitch shift is applied in this panel.

Automated checks completed:

- Every render is finite and has the exact requested number of frames.
- All six compression pairs have identical PCM hashes, as expected because
  compression uses no attack protection map.
- Input hashes still match after rendering.
- Tool integration tests verify source content/mtime preservation, refusal to
  overwrite a pack or write into a source directory, hidden mappings and blank ratings.

The tool records onset counts/timestamps and render time, but there are no
annotated real-recording onset labels. Additional detections are not counted as
correct detections. Timings from this preparation are diagnostic, not controlled
performance measurements.

## Audition protocol

1. Open the [listening instructions](../../work/transient-listening/splice-r5-current/LISTENING.md) and use only `audition/` while scoring.
2. Compare A and B with the original; keep `private/` hidden until scores are complete.
3. Record overall and applicable transient, tonal, vocal, stereo and boundary
   impairment from 1 (unusable) to 5 (no objectionable artifact), plus comments.
4. Use [the blank score sheet](../../work/transient-listening/splice-r5-current/ratings.csv), with a separate copy per listener. Obtain three independent listeners before unblinding.

One common attenuation per A/B/original set prevents peak clipping. There is no
limiter, per-file RMS normalization or claim of LUFS matching.

Limits: six rather than twelve recordings, no dedicated vocal case, only 48 kHz,
no R2/R3 reference renders, no human scores and no formal loudness matching.
This cannot pass the full three-engine listening gate. Windows/Linux and native
DAW listening/gesture checks remain open.

## Reproduction

Use a private corpus JSON array with id/path/rights/start_seconds; do not commit
the manifest or licensed media. The local manifest contains private absolute
paths and remains under work/.

```sh
cargo run -p timestretch-cli --release --no-default-features --locked --bin transient_listen -- \
  --manifest work/transient-listening/splice-manifest.json \
  --output-dir work/transient-listening/new-r5-audition --seconds 10
```

Use a new output directory. The final private report and key are under
`work/transient-listening/splice-r5-current/private/`; they are intentionally
not linked as audition material to avoid revealing A/B labels.
