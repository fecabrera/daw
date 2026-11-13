# Clip and track Bounce validation

Date: 2026-11-12.

Implementation contract: [Bounce](../editing/bounce.md). User procedures:
[Bounce clips and tracks](../../docs/editing/bounce.md).

## Behavior covered

Synthetic fixtures cover selected whole clips and full Audio/MIDI/Beat tracks;
per-source output grouping, gaps, sibling placement, groups, monitoring, residual
clips and disabled-only targets; clip fades, repeat, gain, channels, stretch and
pitch; track, layer and Output effects; Oscillator, Sampler, Granulizer and Noise;
pre/post sends at silent faders and hard pan; external sidechain context, gates,
latency, residual/provider aggregation and flattened re-bounce lineage.

Storage checks cover atomic rejection, one Undo/Redo step, generated-file lifetime,
save/reopen, history-only Save As and source-kind validation. Cancellation checks
cover frame-zero prefixes, unpublished results, late cancellation and stale results.
A disk-backed main/raw/pre-send fixture records zero allocations and deallocations
while processing callback samples. Baked-source ownership changes reject direct
DSP inheritance, and prepared transport matches exact prefix replay.

Egui checks cover success/cancel auto-dismissal, retained errors, exact shortcuts,
extra modifiers/repeats, disabled shifted-command fallthrough, guarded window close
when the receiver is gone, and editing guards through cleanup. The post-bounce
workspace no longer waits for every key to be released: only held-key events are
suppressed, while the workspace re-enables on the next frame. A held Cancel pointer
retains the guard through its release. Fresh key presses after release remain
available, including release/press in the same frame.

The ten-second tail ceiling and terminal fade are expected behavior and do not
produce notices. Float32 Bounce and export preserve samples above 0 dBFS without a
notice. Integer WAV and lossy export retain actual-clipping warnings. The updated
export workflow test checks preserved float samples and retained integer/lossy
warnings. These policies supersede earlier headroom/tail-notice expectations;
historical validation records remain unchanged.

## Executed checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Passed after the final test additions. |
| `cargo test --workspace --offline` | Passed with debug assertions and overflow checks enabled. |
| `cargo test --workspace --release --offline` | Passed separately. |
| `cargo test -p daw-ui --offline` | Passed after the float32-notice change and guarded-window-close addition. |
| `cargo test -p daw-project --release --offline` | Passed after the float32-notice change. |
| `cargo test -p daw-project --test workflow export_settings_keep_overwrite_protection_and_float_headroom --offline` | Passed. |
| `cargo test -p daw-project --test bounce --offline` | Nineteen tests passed; the subsequently added Sampler/Granulizer/Noise fixture passed separately. |
| `cargo test -p daw-ui bounce --offline` | Nine focused tests passed after the final input-suppression adjustment. |
| `cargo test -p daw-project --test bounce --release --offline` | Passed: twenty project Bounce tests. |
| `cargo test -p daw-ui --release bounce --offline` | Passed: nine Bounce UI tests. |
| `cargo build -p daw-cli --offline` and `python3 tools/check_cli.py` | Passed with an isolated temporary audio cache. CLI coverage includes compound audio, collection, codecs, groups, missing sources and overwrite protection. |
| `python3 tools/build_desktop.py debug macos` | Native Apple Silicon build passed using the existing pinned ONNX Runtime cache. The final rebuild includes the last input-suppression adjustment. |
| `git diff --check` and local documentation links | Passed; 649 local link targets checked before final record updates. |

Baseline workspace runs and subsequent affected suites passed. The tail-compaction
follow-up below also passed full debug and release workspace runs with the final
notice, window-close and held-key refinements included.

The first explicit-target native build failed because offline mode prevented the
ONNX build helper from locating its prebuilt library for that target directory.
The retry used `ORT_LIB_PATH` pointing to the same pinned static library already
used by the host build. It succeeded without changing dependencies or downloading
binaries. This was a build-environment issue, not an ONNX inference change.

## Remaining checks

No native UI/device/listening acceptance or Windows/Linux build was performed.
The user's native report showed disabled UI after Bounce. Source inspection and
egui tests identified the all-keys-released guard. The correction has no claimed
native retest. Live device queue pressure, device loss,
publication during Play/Stop/seek, long-job cancellation, both themes and audible
latency/tail checks still need native acceptance. Large-budget and filesystem-fault
matrices are not established by the small generated fixtures.

The existing app bundle was not replaced. Packaging requires a clean committed
tracked tree; this task leaves implementation changes uncommitted. The native
executable build does not establish package/source-archive acceptance. No private
recordings or generated audio were added to the repository. No Rubber Band
implementation source, tests or comments were inspected.

## Tail compaction follow-up

The original conservative budget could leave the complete ten-second allowance
in a file after its signal had decayed. Bounce now renders the complete allowed
window, then compacts only the suffix after the requested geometry. The final cut
uses all main and routing leaves, each with a floor 160 dB below its own peak and
no higher than -160 dBFS. It includes a 5 ms guard/fade outside the requested body.
The leaves retain one clock; no input recording is modified. This supersedes the
original behavior of retaining all conservative padding. It does not introduce
energy-based early termination or truncate a later echo because of an earlier gap.

Complete project tests passed in debug and release. New fixtures verify a late,
very quiet routing-only echo, synchronized leaf lengths, unchanged body samples,
exact-silence compaction, cancellation and shortened integration output. The nine
Bounce UI regressions, workspace/all-target Clippy, isolated-cache CLI smoke,
formatting and the native Apple Silicon executable build passed. Full workspace
debug (`cargo test --workspace --offline`) and release
(`cargo test --workspace --release --offline`) follow-up runs passed separately.
Native listening and UI checks remain open.

## Read-ahead test scheduling follow-up — 2026-11-12

The callback allocation test previously counted 5,000 calls rather than 5,000
completed frames. An asynchronous read-ahead miss returns temporary silence and
keeps the playhead still. A fast test loop could finish before the disk worker
ran, then fail its nonzero-peak assertion.

The test now retries reported buffering with a 15-second deadline. It sleeps for
one millisecond only in the test harness, with heap counting disabled. Every
callback attempt, including buffering, must still allocate and deallocate
nothing. Buffering must leave the playhead unchanged. Every completed frame and
playhead position must match an independent resident-audio renderer. Read errors
and timeouts fail the test. This changes no production preparation, callback or
read-ahead behavior.

All 20 bounce tests passed. The repaired test passed 100 debug and 100 release repetitions with
eight concurrent test processes in each mode. Broader check results are recorded in the
[validation index](README.md).
