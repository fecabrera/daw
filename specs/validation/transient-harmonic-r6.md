# Harmonic-rise regression — Integrated revision 6, 2026-11-08

Status: historical R6 result, superseded by the [revision-7 investigation](transient-lowend-r7.md).
The remaining low-end issue reopened after R6 is now closed: the user listened
to the revision-7 renders and confirmed that distortion and bending are fixed.

A user reported pitch bending on a clean electric-guitar loop after adopting
Combined. In its first ten seconds, revision 5 added four spectral events beyond
the Energy baseline. Their novelty was concentrated in a few harmonics. Those
extra events invoke whole-spectrum phase restoration during expansion and can
disturb sustained notes. This identifies a regression mechanism, not a measured
subjective quality score. The user's exact stretch/transpose settings were not
available during this check.

Revision 6 retains Combined and adds a novelty participation threshold of 16
before refinement. Energy candidates are unchanged. The new synthetic regression
uses quiet 3 kHz picked partials over a sustained bass note. It fails on the
previous implementation at 44.1 kHz and passes with the filter at all six rates.
At 48/96 kHz, its 1.5× and 2× renders match Energy sample-for-sample. The failing
log remains under ignored work storage. Narrowband attacks masked from Energy
may now be missed; this conservative supplement does not separate instruments.

## Real recording and source protection

The complete user-provided guitar recording is 1,035,506 stereo frames at 48 kHz
(21.573 seconds). At ratios 89/120, 3/2 and 2/1, revision 6 yields the same 16
markers and identical PCM hashes as Energy. The first-ten-second comparison also
matches. Compression was already identical in revision 5 because it does not use
attack protection. This proves removal of the added resets, not removal of all
possible pitch artifacts. The separate user listening result is recorded below.

Private renders, hashes and blank listener ratings are under
`work/transient-listening/guitar-r6-full/`; its LISTENING.md gives the protocol.
The six-recording, 18-case Splice pack was also regenerated at
`work/transient-listening/splice-r6/`. All seven source SHA-256 checks match before
and after rendering. No source was written, moved or removed. No private paths,
licensed audio or manifests are committed. Prior R5 packs remain historical.

## User listening confirmation — 2026-11-08

The user listened to the updated audition at their exact stretch settings and
initially confirmed that the pitch bending was gone. The user later reported
remaining low-end distortion and bending on the same recording. The listening
regression is reopened; the earlier confirmation is incomplete. The numeric stretch/transpose settings were not supplied,
and no numeric ratings were given. This single-recording confirmation does not
complete the broader listening acceptance gate or establish native DAW coverage.

## Automated checks

- Portable debug and release all-target suites: pass, including existing pitch,
  stereo, cancellation, waveform attack and resource tests. Two existing extended
  contract tests remain ignored in these runs; their R5 results are historical.
- Synthetic detector benchmark: 126 paired inputs across six rates and three
  gain/layout cases. Combined: 378 true, zero false, zero missed attacks.
  Energy: 324 true, zero false, 54 missed. Mean per-input median detection time
  for one second of audio: 3.593 ms Combined, 0.194 ms Energy (about 18.5×).
  This is this machine's diagnostic timing, not a cross-platform performance gate.
- Core tests; project revision migration, metadata inspection and engine-switch
  tests; app preference migration; standalone tool tests: pass.
- Strict Clippy for affected portable/host crates, rustfmt and diff checks: pass.

Raw benchmark files are `work/transient-detector-benchmark/detection-r6.csv` and
`render-r6.log`. Logs use the `guitar-` prefix under work/transient-listening.
Stretch revision 6 and detector energy-spectral-v5 identify the new output.
Known saved Integrated revisions 2–5 rebuild with a warning on open; source
manifests remain unchanged until Save. App engine preferences also migrate.
Broader listening ratings, native DAW listening and Windows/Linux checks remain open.

Broader listening acceptance was subsequently completed for revision 7 by
[explicit user acceptance](transient-lowend-r7.md#broader-listening-acceptance--2026-11-08).
The R6 statements above remain historical evidence.
