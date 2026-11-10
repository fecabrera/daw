# Harmonic Lift validation

## Scope

Native nine-strip effect, six fixed harmonic branches, linked maximizer, aligned Normal/HQ paths, built-in placement and standalone CLAP/VST3 integration. See the [implementation specification](../effects/harmonic-lift.md).

## Branch monitoring — 2026-11-10

Six shared band rails now display processed branch contributions with separate
held peaks in the DAW and standalone editors. They use existing strip margins
and scoped monitor handles. The Maximizer reduction rail and output monitors
remain separate. Current checks and native limits are recorded in the
[workspace validation log](README.md#harmonic-lift-band-level-meters--2026-11-10).

## Automated checks — 2026-11-03

- Workspace compile check passed on macOS arm64.
- All 15 packaging tests passed, including discovery of both product targets and the CI entry.
- All eight portable DSP checks passed, including shared targeted modulation and reduced folded Drive harmonics in HQ, and smooth activation at zero Push on hot input.
- All six shared editor tests and all three typed plugin contract tests passed. These cover geometry, control bindings, theme/scale mesh parity, gestures, queue bounds, state corruption and meter bypass. A separate dropdown test passed for both Normal → HQ and HQ → Normal.
- All 15 native format adapter/binary tests passed against the macOS CLAP/VST3 libraries, including parameter timing, six rates, irregular/in-place blocks, state recall and latency queries.
- All three engine integration tests passed with the final DSP: live/export/seek and block-size agreement at six rates, compatible quality updates, targeted route updates, modulation snapshots, and callback allocation/retirement guards.
- CLI smoke checks passed: codecs, repeatability, source collection, missing sources, group buses, overwrite and exit codes.
- Both release plugin libraries and signed macOS preview archives were built. The macOS debug desktop bundle was refreshed. Strict workspace/all-target Clippy and formatting passed. Full workspace tests completed: 2223 passed, 17 ignored, no failures across 282 suites. The final DSP, engine and standalone checks were also rerun after the Push transition adjustment.

## Native limits

No Windows or Linux execution, external-host listening test, native accessibility audit, or SDK validator result is claimed. Automated shared-body mesh comparisons do not establish native window attachment or display scaling. Normal keeps the HQ path warm for seamless automation and is not a CPU-saving mode.
