# Remaining guitar low-end regression — revision 7, 2026-11-08

Status: automated checks pass; **the reported guitar regression is closed and
broader listening acceptance is complete by explicit user acceptance**.
The user listened to the revision-7 renders and confirmed that the low-end
distortion and bending are fixed. The user qualified
the earlier R6 confirmation with remaining low-end distortion and bending on the
same guitar recording. Do not treat the initial confirmation as final acceptance.
Numeric user stretch/transpose settings and artifact timestamps have not yet been
supplied. Tests and auditions below use stated representative settings.

## Diagnosis and implementation

The full guitar has 34.7006% of analyzed linked power at/below 250 Hz, full-source
flatness about 8.19e-8, and no significant broadband-band veto. Revision 6 therefore
selects the 2,048-frame window at 48 kHz despite meaningful low fundamentals.
A generated harmonic-rich F-sharp-minor voicing reproduces a 60.10197-cent error
on its 92.4986 Hz fundamental at ratio 89/120. Upper partials carry most power;
the prior 50% bass threshold misses this case.

Revision 7 selects the existing long window when low-band power is at least
10%, while retaining the broadband-first choice and mixed-noise veto. The guitar
now uses 16,384 frames at 48 kHz. The window-only experiment fixed compression
but lost about 2.405 dB on the generated low fundamental during 1.5× expansion:
attack phase resets also disturbed held notes.

On entering each expansion guard, a separate Hann FFT ending 2 ms before its
source onset establishes prior low-note presence. Bins at/below 250 Hz whose
history power is at least one quarter current power retain accumulated phase;
other bins restore source phase. Keep this mask for that guard. Stable guard
identities prevent reuse when a hop skips a gap. The time map and detector are
unchanged. A centered-window-only history experiment failed a kick waveform
check; the pre-onset window avoids classifying the new kick as a held tone.

Stretch identity is 7, pitch composition remains 2, and audio-analysis detector
identities remain energy-r4 / energy-spectral-v5. Known Integrated project/app
preferences 2–6 migrate with existing rebuild/warning rules. Workspace admission
now reserves 10 MiB and 56 transient bytes per maximum event, including history
scratch and source-onset fields. No new dependencies or source edits are needed.
See the [implementation contract](../audio/timestretch.md#harmonic-rich-low-notes-and-held-phase--revision-7).

## Measurement definition and limits

The generated chord test spans six rates, ratios 89/120, 3/2, 2/1, and combined
17/20 +6 semitones / 5/4 −6 semitones. Fundamentals meet the existing 5-cent and
0.5 dB limits globally and in local windows inside the middle half of output.
Whole-excerpt peaks alone can hide modulation. A local boundary diagnostic at
44.1 kHz / 2× measured 5.034 cents before the steady-state domain was applied;
that log is retained. This interior metric does not certify boundary fidelity.

The old attack waveform metric compared the entire mixed waveform with source
phase. Keeping a held note continuous allows a different phase at the new hit;
forcing it to match source phase conflicts with this change. Its original
mixed-waveform residual at 8× was 0.03360 and is **not reported as passing**.
Attack metric v2 subtracts the known input bed and a pre-attack sine/cosine fit
from output, then retains the same 2% residual limit on percussion alone. It
also requires held-tone level within 0.5 dB. A calibrated phase-offset control
and a later injected attack verify that fitting does not remove attack energy.
Pure-percussion waveform/pre-echo criteria are unchanged. No numeric threshold
was loosened to fit output. Original, window-only, centered-history, local and
corrected metric logs remain under ignored work storage.

## Completed checks and private auditions

- Analysis/stretch debug and release all-target tests pass (41 tests; two extended
  tests skipped in ordinary runs). Both extended release contract tests pass when
  run explicitly, including six-rate pitch/level and short-layout matrices.
- Core/media, revision migration, inspection, engine-switch, app preference,
  standalone tools, doctests and strict Clippy pass. Formatting/diff checks pass.
- The full 21.573-second guitar and six-recording Splice panel were rendered at
  48 kHz. All source SHA-256 checks match before/after rendering. Frame extents
  and finite samples are checked by the tool. Guitar source markers are unchanged
  from R6; rendering changes. No human scores were generated.

Private packs are work/transient-listening/guitar-r7-lowend and splice-r7. A
separate direct R6/R7 comparison, guitar-lowend-r6-r7-current/LISTENING.md,
contains original, R6 and R7 at ratios 89/120, 3/2 and 2/1 (transpose 0), with
optional 250 Hz fourth-order Butterworth low-pass versions. Each ratio set uses
one shared attenuation, without limiting or independent loudness matching. The
filtered files are diagnostic and add their own filter phase/settling. Licensed
audio, source paths and manifests stay private under ignored work storage.

Longer windows can change attack/decay texture. Prior presence is not instrument
separation, and short history/boundaries can miss held notes. The reported guitar
issue is closed by the user listening confirmation below. Broader listening
acceptance is complete by the subsequent explicit user decision. Native DAW,
Windows/Linux, performance and device acceptance retain their separate status.

## User listening confirmation — 2026-11-08

The user listened to the revision-7 renders and confirmed that the low-end
distortion and bending are fixed. This closes the reported guitar regression.
No numeric listener ratings or additional stretch settings were supplied. This
confirmation covers the reported recording and auditioned renders. The subsequent
acceptance decision below completes broader listening acceptance. Native DAW
coverage is not established by this listening confirmation.

## Broader listening acceptance — 2026-11-08

**COMPLETE for Integrated revision 7, by explicit user acceptance.** Following
confirmation that the revision-7 renders fixed the guitar's low-end distortion
and bending, the user instructed: “Mark broader listening acceptance as complete.”
This closes the outstanding broader listening item for this revision and
supersedes the remaining listening-panel requirements as recorded in the
[acceptance plan](../acceptance/timestretch.md#revision-7-acceptance-decision--2026-11-08).

No additional listening sessions, numeric ratings, multi-listener panel results,
full-corpus runs, R2/R3 comparisons or loudness measurements were supplied or
inferred. Existing render/test evidence and source-protection records are
unchanged. Performance, platform, native DAW/device and stable-release acceptance
remain separate gates.
