# External plugin hosting integration — 2026-11-17

The desktop and shared renderer now host VST3/CLAP effects and instruments. This
record supersedes the scanner-only implementation status in the earlier
[foundation record](external-plugin-hosting-foundation-2026-11-17.md). It does not
claim complete native acceptance on every shipped platform.

## Scope

The implementation includes default-folder discovery and isolated scan recovery;
stable parameter mappings and common controls; native editor parents and services;
shared parameter modulation; separate raw PCM auxiliary routes; a common live,
offline, seek and bounce block graph; exclusive processing leases; revision-checked
opaque state and Undo/Redo ownership; missing-unit placeholders; strict/explicit
degraded CLI rendering; and embedded isolated scanning with packaged format notices.

The owned fixtures export both formats and include an effect, a polyphonic
instrument, a gain parameter, an auxiliary audio input, fixed three-frame latency,
opaque state and offline-mode negotiation. Native Drive checks compare the actual
loaded binary with this repository's portable DSP. No third-party plugin or user
project was used for these tests.

## Checks

Host: macOS arm64. Rust: workspace-pinned 1.97.0. Cargo commands use `--offline`.
Logs remain under ignored `work/plugin-hosting-validation/`.

| Check | Result |
| --- | --- |
| Full debug workspace, `cargo test --workspace --offline --no-fail-fast` | 3,594 passed, 20 failed, 22 ignored. The 20 failures are the existing AUv2 registration case below. This broad run preceded the final integration refinements. |
| Final host, engine and UI library tests | 1,147 passed, zero failed, three ignored. Includes the final native PCM provenance, grouped catalog and App Settings scan controls. |
| CLI cached catalog tests | Three passed, zero failed. |
| Embedded scanner entry points | Desktop and CLI both passed owned CLAP/VST3 factory enumeration, identity/unload and damaged-module failure checks without a helper. |
| Focused model/host tests | Passed, including disabled PCM cycle rejection, transactional self-route/identity rejection, fresh copy IDs, unknown future payloads, nonfinite controls, state corruption, symlink rejection and immutable-owner pruning. |
| Exact state through history | Passed: stale receipt rejection and exact opaque bytes through Undo/Redo. |
| Native fixture matrix | Passed for both formats at six rates and all eight project buffers (32–4096). Includes polyphonic notes, raw PCM with captured provider feeds, modulation, seek, bounce, degraded rendering, live loops/retriggers, pause/resume, partial FIFO handoff and callback heap guard. |
| Loaded Drive binary comparison | Passed for both formats at six rates, with irregular blocks, native graph parity, exact manual-base state capture, lease exclusivity and retirement. |
| macOS native editor lifecycle | Passed for CLAP and VST3: attach, size, focus, close, reopen and retirement, using real desktop/GPU access. A sandboxed attempt could not attach the editor; the desktop-access run passed. |
| Linux x86_64 host/scanner/fixture cross-check | Passed with the pinned Rust standard library. |
| Windows x86_64 host/scanner/fixture cross-check | Passed with the pinned Rust standard library. |
| Linux X11 and Windows HWND parent source checks | Passed in a narrow cross-compilation harness. This does not establish full desktop or native UI acceptance. |
| Package helper/notice tests | 25 passed. |
| macOS local debug package | Passed after embedded scanning replaced the helper. No helper is present; the complete app signature verifies, and its executable passes owned CLAP/VST3 scans and damaged-module failure checks. |
| Workspace Clippy | Passed with `--all-targets -- -D warnings`; see final log for the source checkpoint. |

The affected-package debug run passed 2,017 tests, with no failures and seven
ignored tests before the last settings/scanner refinements. The full release run
passed 3,595 tests, failed the same 20 AUv2 tests and ignored 22 tests. CLI smoke
checks passed. Native preset rescan, authored base publication, grouped UI
Undo/Redo and exact opaque capture checks passed.

The final fixture rerun also exercises the full supervisor path by spawning the
fixture check executable itself in embedded scanner mode before activating units.
The final example Clippy check passed with warnings denied.

One VST3 graph check timed out at 120 seconds during concurrent builds. A direct
reproduction and a complete rerun passed; its cause remains unconfirmed.

All 20 broad debug failures are
`native::tests::native_audio_component_registration_and_host_calls` in AUv2
export crates. The component lookup returns null. The cause is not established.
No AUv2 code or test was changed to suppress this failure. No full workspace PASS
is claimed.

## Linux GTK launcher follow-up

The GTK compatibility example shares the desktop hosting module and starts the
embedded scanner through the same GTK entry point. The CLAP parameter text buffer
uses the platform's C character type.

`cargo check --locked -p daw-desktop --bin daw-desktop --example gtk_titlebar`
passed on Linux ARM64 with Rust 1.97.0. The x86-64 Docker run stopped before
compilation because the emulator crashed while starting Rust. The GTK window
and plug-in editor were not exercised.

Workspace Clippy and focused desktop and plug-in host tests passed on macOS.
Both workspace runs used `--locked --offline --no-fail-fast -- --skip native_audio_component_registration_and_host_calls`.
The debug run passed before the C character type change. The focused debug tests
and the release workspace run passed with the final change.

## macOS shutdown follow-up

The crash report stopped in `objc_autoreleasePoolPop` during host teardown.
AppKit had drained the pool created in the launch event. Native control calls
and teardown now use pools confined to the current call.

- The host lifetime regression aborted before the fix and passed afterward.
- The signed debug bundle and the release executable returned status 0 on
  macOS Quit with an empty session and a copy of the demo project. Closing the
  empty session's debug window also returned status 0.
- The bundled scanner and the native Drive CLAP and VST3 fixtures passed
  discovery, editor lifecycle, processing, state capture and retirement checks.
- Formatting and strict workspace Clippy passed. Both the debug and release
  workspace suites passed with `--locked --offline --no-fail-fast -- --skip native_audio_component_registration_and_host_calls`.

## Linux graph check timeout

The Linux ARM64 container reproduced the CI timeout with the dev build of
`native_graph_check`. CLAP reached the 88.2 kHz, 4,096-frame case before the
120-second timeout. The run used 119 seconds of user CPU time. The debug Drive
CLAP binary was 154 MiB. The graph check fingerprints the bundle during
discovery and before each of its two native loads in all 18 cases.

CI now builds the graph check with `--profile test`, which retains debug
assertions. The graph check logs each rate and buffer size as it begins. With the
same debug plugin binaries, the Linux ARM64 scanner, graph and host checks
passed for both formats in 114 seconds total. The per-graph limit remains 120
seconds. The same sequence passed on macOS arm64. The Linux x86-64 GitHub
runner was outside this verification.

Formatting, strict workspace Clippy and `git diff --check` passed on macOS.
The debug and release runs both passed:

- `cargo test --locked --offline --workspace`
- `cargo test --locked --offline --workspace --release`

## Hosted header button spacing

Adjacent painted header dots have a single 7-point edge-to-edge gap, equal to
the visible inset from the header top to Power. The focused regression
measures the painted edges and checks separate 22-by-17-point hit areas and
action dispatch.

Formatting and workspace Clippy passed on macOS. The spacing change passed
`cargo test --locked --offline --workspace` in debug and
`cargo test --locked --offline --workspace --release` in release. Native visual
verification of the current build was outside these checks.

## Limits

Rust allocator guards observe host and Rust fixture heap operations. They cannot
observe every allocation or lock inside a vendor binary or platform framework.
They do not prove callback deadlines on physical audio hardware.

Cross-compilation checks platform code and ABI types. It does not prove Windows
or Linux native editor operation, HiDPI movement, text input, installed-app loading
policy, audio devices, listening or host/vendor compatibility. Linux editors need
X11/XWayland. The owned macOS editor check does not replace those release checks.

The host rejects unsupported bus layouts and incompatible parameter definitions.
It preserves disconnected port identities rather than guessing a route. Active
vendor code runs in process and can crash or hang the DAW. AU hosting,
cross-architecture bridging and arbitrary vendor redistribution are outside scope.
The existing generated-binding provenance and full release obligation review remain
separate distribution gates.

Unrelated working-tree data, including `tools/__pycache__/`, was retained. No
commit, push or published review was made.

## Controller recall and panel follow-up

The VST3 fixture now returns `kNotImplemented` for controller component-state
synchronization. The full owned fixture matrix passed for VST3 and CLAP at six
rates and all eight project buffers. Corrupt processor state still fails and the
diagnostic identifies the supplied module and format. This includes the grouped
preset check, sidechains, shared modulation, independent seeks, bounce, transport
loops and callback allocation guards.

A separate manual check loaded the installed TDR Nova VST3 shown in the user's
report. Default capture, fresh-instance restore, writable parameter equality and
recapture passed. It ran in a disposable process and did not open or write a DAW
project or install plugin files. This check does not establish native editor,
playback or compatibility for other installed vendors. Normal CI uses owned
fixtures only. The reusable explicit-module check is
`cargo run -p daw-plugin-host --example native_state_check -- FORMAT MODULE`.

The mapped-panel geometry test passed in both light and dark themes. It checks
standard row spacing, knob column alignment, strip width and unchanged edit
revision during painting. This is automated UI layout validation, not native
visual acceptance.

The follow-up host, shared-control and UI library run passed 1,168 tests, with
zero failures and three ignored tests. Clippy passed for the affected host,
fixture, UI, shared-control and engine targets with warnings denied. The desktop
executable rebuilt successfully. The local macOS app package rebuilt and its
complete signature verified. These follow-up checks do not supersede the broad
workspace AUv2 failures or native platform limits listed above.

## Bus arrangement negotiation follow-up

The installed Youlean Loudness Meter 2 VST3 reproduced the reported unsupported
channel layout failure before the change. Initialization now negotiates mono or
stereo while inactive, then re-reads actual buses before allocating buffers.
The same manual check then passed state capture, fresh-instance restore, writable
parameter comparison, four 512-frame stereo processing blocks and recapture at
48 kHz. It used a disposable project-free process. The process also emitted a
macOS notification-tracker diagnostic; the host checks and exit status passed.
No editor, audio device or listening validation is claimed.

The VST3 owned fixture now starts with six-channel buses and changes to stereo
only after accepting the host request. This makes the existing native processing,
state, sidechain and modulation checks exercise real negotiation. Unchanged
controller writes no longer fail state capture; changed values still use the
controller setter and the processor's input parameter queue.

The final native fixture matrix passed for CLAP and VST3 at all six rates and
eight project buffer sizes, including preset notification/state checks. All 18
host library tests passed. Clippy passed for the host, fixtures and engine with
all targets and warnings denied. Both desktop debug locations rebuilt; the local
macOS app package passed complete signature verification. No full workspace
rerun or new native editor/device acceptance is claimed for this follow-up.

## Header action follow-up

Editor and Reload moved from the body to two shared header action slots below
Power. Maximize2 and RefreshCcw use the active modulation dot surface. The title
starts below these slots; action hit areas cannot start header collapse or drag.
Two hosted-panel tests passed, including request queue routing, double-click
isolation, row geometry and both themes. Five shared-panel/view tests passed.
UI Clippy passed for all targets with warnings denied. Both desktop debug build
locations rebuilt and the local macOS app signature verified. No new native UI
visual acceptance or plugin processing checks were needed for this UI change.
