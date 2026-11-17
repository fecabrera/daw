# Arrangement markers and timing maps — 2026-11-16

Implementation contract: [timing maps](../audio/timing-maps.md).

## Automated coverage

- Core: legacy constant representation, persistence round trips, event identity and
  value validation, atomic rollback, piecewise frame/quarter conversion, partial
  signature bars, tempo retiming, source-range slices and MIDI split bounds.
- Engine: note-on/off and actual duration across a tempo boundary, seek
  reconstruction, integrated LFO phase, and the existing playback/export,
  seek/loop, parallel-render and callback allocation matrices.
- Project: capture spanning tempo changes, actual take bounds, save/reopen and
  supported-rate conversion. The new capture test exposed a scalar-tempo
  exclusive-end check; the forward bound now uses the compiled map.
- UI: pinned lane placement, marker drag preview/release/cancel and Undo,
  map-aware frame geometry and active-event toolbar editing. Existing ruler,
  clip, group, beat, Return-bank and effects tests use their actual pane bounds.

## Executed commands

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Passed on the final source. |
| `cargo test -p daw-ui --lib --offline` | Passed: 1,047 tests, 3 ignored. |
| `cargo test --workspace --offline` | Passed. The first run exposed the take-bound defect above. |
| `cargo test --workspace --release --offline` | Passed on the final source, including timing saturation and the ruler/Markers divider. Superseded duplicate run stopped. |
| `cargo build -p daw-cli --offline` | Passed. |
| `python3 tools/check_cli.py` | Passed: deterministic rendering, codecs, source collection, groups, missing sources, overwrite and exit codes. |
| `sh tools/package_macos.sh debug --local` | Passed; ad-hoc signature verified. An initial offline-environment invocation failed to link ONNX Runtime. Normal packaging used the existing local dependency cache. |

Debug tests retain the workspace's debug assertions and overflow checks. Native
Audio Unit tests run outside the sandbox because they need macOS host services.
No existing media, fixtures or user projects were overwritten.

## Ruler boundary follow-up

Added the full-width shared border above the first metadata row after native
feedback identified the missing ruler/Markers boundary. It covers both the lane
header and timeline, including collapsed or hidden-lane configurations. The
existing arrangement interaction test now checks the exact separator span.
`cargo test -p daw-ui --lib tests::arrangement --offline` passed both tests. The
macOS bundle was rebuilt and its signature verified after the change.

Callback timing queries also use numeric saturation at extreme bounds; the
allocation test covers saturated note origins and metronome boundaries.

## Native observation

Opened the rebuilt macOS app with an empty Untitled project. Observed Markers,
Tempo and Signature between the ruler and track viewport. Lane backgrounds match
the Timeline grid; the initial signature uses the shared unit-header block fill.
Created a default Section marker through the lane header menu, observed its named
block, and used Command+Z to restore the clean project. No project was saved.

The saved physical MIDI input was unavailable at startup. Dismissed that notice;
no device preferences were changed. Physical MIDI capture, count-in alignment and
listening comparisons were not established by this check.

## Remaining acceptance

- Native rename/value entry, drag/Shift/cancel/focus, coincident marker menus,
  visibility and short/narrow layouts across themes and display scales.
- Listening and hardware checks across tempo/signature boundaries, seeks, loops,
  stopped live audition, count-in and recorded takes.
- Native export/Bounce listening comparisons and long-session behavior.
- Windows/Linux UI, device and platform checks.

Tempo ramps, whole-section move/copy and Timeline automation remain feature work;
these native checks are separate from that backlog.

## Effects-header alignment follow-up

The first boundary used the next row's top pixel, one logical point below the
Effects catalog's bottom separator. It now reuses `ControlPanel::pinned_row_divider`
to share the preceding header's bottom pixel. The arrangement test compares the
actual Effects header and ruler bounds and verifies the full separator span.
Focused arrangement tests, the full UI suite (1,047 passed, 3 ignored), formatting,
strict UI clippy and the rebuilt macOS bundle/signature checks passed. The earlier full workspace results above cover
the feature baseline; this follow-up changes only separator painting.

## Beat Editor lane-height follow-up

Markers, Tempo and Signature now share Beat Editor's `SUBTRACK_LANE_HEIGHT`
(32 logical points). Tempo hit regions fill the taller row; the short-viewport
Return-bank budget reserves the same shared height. The placement test checks
three shared row heights below the ruler. Beat Editor's visible size is unchanged.

The taller lanes exposed two fixture assumptions: a settings test selected a
clip header behind the dialog, and a group-drag test targeted a row outside its
viewport. The settings test now selects the frontmost field rectangle; the drag
test keeps its destination visible without an Effects lane.

`cargo test -p daw-ui --lib --offline` passed (1,047 passed, 3 ignored).
`cargo fmt --all --check` and strict UI clippy passed. The macOS debug bundle was
rebuilt and its signature verified. No DSP or project-format behavior changed.

## Inline section-name follow-up

New markers now commit immediately as `New section`. Double-clicking a section
replaces its label with the shared inline name input and selects the current
name. Enter commits one marker edit; Escape or focus loss cancels the draft.
Leaving Timeline, hiding the lane, playback or a conflicting editing environment
also cancels it. Tempo and signature value dialogs are unchanged.

The new pointer/keyboard regression covers creation, automatic input focus and
name selection, Unicode typing, Enter, Escape, stable marker identity/frame,
Undo and unchanged tracks. The full UI suite passed (1,048 passed, 3 ignored).
Formatting, strict UI clippy and `git diff --check` passed. The macOS debug
bundle was rebuilt and its signature verified. Native text entry remains an
acceptance check; these results use synthetic egui input.

## Section ranges and signature edges follow-up

Sections now save positive, checked frame lengths. Creation uses one bar from the
active signature and mapped tempo. Earlier marker manifests without a length
resolve to one bar. Both section edges resize the range with the opposite edge
fixed; body drags preserve length. Tempo edits preserve both musical endpoints,
and project-rate edits rescale both endpoints. Section resizing changes no clips.

Signature edges move the shared change between adjacent blocks. The origin stays
at zero, changes cannot cross their neighbors, and the final signature remains
active until another change. Both gestures use the shared grid/Shift bypass,
preview in UI state and commit one transaction on release. Escape, focus loss,
invalidated revision or release outside the lane cancels the gesture.

Five arrangement UI tests passed, covering inline naming, one-bar creation,
section edge resizing and cancellation, Undo, stable identity/name, unchanged
tracks, signature boundaries and the fixed origin. Core tests cover legacy-length
fallback, invalid ranges, atomic rejection and both scalar/mapped tempo changes.
The existing project workflow now also checks section-length persistence and
rescaling at every supported project sample rate.

Formatting, strict workspace clippy and `git diff --check` passed. The macOS debug
bundle was rebuilt and its ad-hoc signature verified. The first workspace debug
attempt hit the sandbox's native Audio Unit registration restriction; native
workspace checks require macOS host services as recorded above. An initial
clippy test-initializer warning was fixed before the passing run.

`cargo test --workspace --offline` passed with macOS host access on the final
source, including all 1,050 UI tests (3 ignored). The section persistence/rate
workflow also passed independently. Native pointer/typing and Windows/Linux
acceptance remain open.

`cargo test --workspace --release --offline` also passed with macOS host access
on the final source, including the section/signature interaction tests and the
storage/rate workflow. The sandboxed release attempt hit the same native Audio
Unit registration restriction before this passing rerun. Debug and release
results are separate; debug assertions remained enabled in the debug run.

## Header selection, clip-style range edits and continuous signatures

Section and signature labels now reuse the unit-header semibold font and shared
selected/unselected text colors. Selection leaves the header fill unchanged and
adds no outline. Plain click selects one block, Shift toggles membership, empty
lane space clears it, and leaving Timeline clears it. Selection does not seek or
edit the project. Context Seek here and previous/next navigation remain available.
Group Delete commits one guarded history transaction and preserves map origins.

Core conductor edits and clip overwrite/resize share exclusive frame-range
subtraction. Ordinary resizes trim covered edges and remove fully covered blocks
with directional priority. Shared boundaries use the existing clip gesture radii,
keep outer endpoints fixed and can remove a zero-length side. Other selected
blocks use the requested delta independently of the pair clamp. The signature
origin stays at zero with a positive span. UI previews include every affected
neighbor; release still works if the dragged section disappears during preview.

Signature extents remain continuous and derived from change positions. Right-edge
shrinking pulls the following change back instead of leaving a gap. The final
signature fills the remaining timeline and has no finite right handle. Its start
can move. No signature duration field is added to the project format. BPM labels
now use the standard small-text style.

Eight focused UI tests passed, covering header font/colors and Shift/empty-space
selection, group Delete/Undo, selection without seeking, view changes, section
shared-boundary owner removal, ordinary overlap removal, signature continuity,
final-span painting and smaller BPM text. Core regression coverage includes both
resize directions, selected-group priority, independent pair clamps, invalid
selected shortening, protected signature origin and the selected final pair.
CLI build and smoke checks passed. Final workspace checks are recorded below.

The final UI suite passed (1,053 passed, 3 ignored), including the corrected
continuous signature spans and small BPM labels. The focused core range tests,
formatting, strict workspace clippy, CLI build/smoke and `git diff --check` passed.
The final macOS debug bundle was rebuilt and its ad-hoc signature verified.
After the last UI-only batch-order adjustment (delete signature changes before
tempo reprojection), the full UI suite, clippy and bundle checks were repeated.
Native pointer/typing checks and Windows/Linux acceptance remain open.

The native-access debug workspace run passed, including the core range tests,
clip/source mapping matrices, project persistence workflows and UI tests.
Debug assertions and overflow checks remained enabled. The later UI-only
batch-order adjustment was checked by the complete UI suite as noted above.

The final native-access release workspace run also passed, including all 1,053
UI tests (3 ignored), selected-pair signature regression, clip/source mapping and
project workflows. Both debug and release workspace checks are complete. No
native pointer, physical-device or Windows/Linux acceptance is inferred from
these automated results.

## Tempo point interaction follow-up

Empty Tempo-lane double-click and Add actions now insert a point immediately at
the active BPM, with no creation dialog. Existing frames select their existing
point. Interaction uses point-sized hit targets instead of the former wide label
rectangle, so nearby empty lane space remains available for creation.

Tempo points preview vertical value changes through the same BPM adjustment as
the transport: 1 BPM per point, Shift fine adjustment, hundredth rounding and a
positive minimum. The plot range stays fixed during the gesture. Horizontal
movement retains snapping, and the origin retains frame zero. Release commits
one tempo-map edit; vertical release can leave the row. Existing Escape, focus,
job, playback and revision guards cancel the gesture. Click selection and Delete
use stable tempo IDs; origin deletion is protected. Numeric entry stays available
through the context Edit action.

All 11 focused arrangement tests passed. New input coverage includes immediate
creation without a dialog, creation near an existing label, selection/Delete and
Undo without clip deletion, BPM preview without model mutation, vertical and
horizontal motion, Shift fine changes, release outside the row, Escape, fixed
origin position and positive BPM bounds. Strict workspace clippy, formatting and
`git diff --check` passed. The macOS debug bundle was rebuilt and its signature
verified. Full workspace results follow below; native gesture/device checks
remain separate acceptance work.

Both native-access workspace runs passed (debug and release), each including
1,056 UI tests with 3 ignored. A follow-up Delete regression also passed in both
profiles: pressing Delete again after the point disappears cannot delete clips.

The final UI-only precision fix skips BPM rounding when vertical delta is zero,
so horizontal movement preserves values such as 123.456 BPM exactly. The updated
full debug UI suite passed (1,057 passed, 3 ignored). Its position assertion allows
the existing one-frame musical-snap/reprojection rounding; the BPM assertion
remains exact. Strict workspace clippy, formatting and the rebuilt macOS bundle
passed again after the precision fix. The final release UI result follows below.

The final release UI suite passed too (1,057 passed, 3 ignored), including exact
fractional BPM preservation and the repeated-Delete guard. Final workspace
clippy, formatting and `git diff --check` passed. Full workspace runs cover the
feature baseline; both complete UI suites cover the final UI-only precision fix.
Native pointer/device and Windows/Linux checks remain acceptance work.

## Gridline at rounded tempo boundaries

Reproduced the missing bar 4 gridline and ruler label using 120, 80, 103.14 and
116.46 BPM changes placed at successive bars. A tempo event's rounded frame can
sit just after the exact musical boundary while that boundary's ceil-frame is
the event frame itself. The old segment excluded that sample and the new segment
rounded its first musical index past it, so neither emitted the bar tick.

Mapped tick generation now includes the preceding candidate and assigns segment
ownership by frame coordinates. The regression failed before the fix and passes
after it at every supported rate. It checks bar labels, a boundary at/near the
viewport start, exactly one tick there, and the actual painted bar line.
Strict workspace clippy, formatting and `git diff --check` passed. Native visual
acceptance remains separate; full workspace/package results follow below.

Both native-access workspace runs passed on this fix, debug and release,
including 1,058 UI tests (3 ignored) in each profile. Formatting, strict workspace
clippy and `git diff --check` passed. The macOS debug bundle was rebuilt and its
ad-hoc signature verified. Native visual and Windows/Linux acceptance remain
open; these results establish automated tick generation and painting behavior.

## Whole-number BPM drag steps

Tempo points and the transport now share `TempoValueDrag`, which accumulates
subpixel pointer motion but emits whole-number BPM values. Both use 1 BPM steps
with or without Shift, with a drag minimum of 1 BPM. Shift retains timeline snap
bypass for horizontal point movement. Zero vertical motion preserves an existing
numeric value exactly; numeric entry remains independent of drag quantization.
The transport stores temporary gesture state under the control's stable widget
ID, and each tempo-point gesture owns the same state type.

Twenty focused tests passed, including both controls, Shift behavior, positive
bounds, horizontal value preservation and fractional pointer accumulation. The
old fractional-step assertions were updated to the requested whole steps.
Formatting, strict workspace clippy and `git diff --check` passed. Full workspace
and packaging results follow below; native pointer acceptance remains separate.

### Shift sensitivity clarification

Shift now reduces pointer sensitivity tenfold in both transport and Tempo lane,
while the shared drag accumulator still emits whole BPM values. This is slower
movement between the same 1 BPM steps, not fractional output steps. Gesture
residuals survive modifier changes. Focused tests cover Shift motion, switching
back to normal sensitivity, subpixel accumulation, stationary fractional values
and positive bounds. All 20 focused tests and strict workspace clippy passed.
The clarified macOS debug bundle was rebuilt and its signature verified.

Both final native-access workspace runs passed, debug and release, including
1,059 UI tests (3 ignored) in each profile. The clarified Shift behavior is
covered in the transport and lane tests. Formatting, strict workspace clippy,
`git diff --check`, and the rebuilt bundle/signature checks passed. Native
pointer/device and Windows/Linux acceptance remain separate pending checks.

## Tempo point drag direction tolerance

Tempo point gestures now choose the dominant axis on the first recognized drag
sample. They ignore perpendicular motion below 12 logical points
from the latest axis-lock position. A switch consumes that tolerance, applies
only the excess motion and rebases the anchor; ignored movement is not replayed
into time or BPM. Stationary frames do not change direction. The initial sample
is processed immediately so a rapid second sample cannot replace the starting
axis before its first effect is captured. Origin points remain vertical-only.
Filtered coordinates feed both timeline snapping and the existing whole-BPM,
Shift-sensitive accumulator, including horizontal release bounds.

All 25 focused tempo tests passed. New coverage checks tolerance on both axes,
switching in both directions without a jump, initial large diagonal movement,
stationary frames, origin behavior, model-free previews and Undo. Existing
fractional-value preservation and Shift/whole-step tests also passed. Strict
workspace clippy, formatting and `git diff --check` passed. The macOS debug
bundle was rebuilt and its ad-hoc signature verified. Full workspace results
follow below; native gesture feel remains a separate acceptance check.

Both final native-access workspace runs passed, debug and release, including
1,064 UI tests (3 ignored) in each profile. Formatting, strict workspace clippy,
`git diff --check` and the rebuilt bundle/signature checks passed. Native pointer
feel and Windows/Linux acceptance remain pending and are not inferred from
these synthetic input and automated workspace checks.

## Marker/signature block top edge and reference fill

Marker and signature fills now start exactly at the lane top, removing the
one-point background strip above them. The bottom inset remains reserved for
the shared row divider. The supplied flat sample contains RGB (43, 43, 43),
`#2B2B2B`. Both block types use that color through the existing neutral unit
surface in Dark mode. Light/Mist retain their existing contrasting header fills
through the app's shared `arrangement_block_fill` helper. Text weight and
selection behavior retain the unit-header contract.

Updated geometry checks assert the marker top edge and bottom divider extent,
the signature top edge across resize/Undo, and the exact supplied Dark color.
All 14 focused arrangement tests passed. Formatting, strict workspace clippy
and `git diff --check` passed. The macOS debug bundle was rebuilt and its ad-hoc
signature verified. Full workspace results follow below. Native visual checks
remain separate acceptance work.

Both final native-access workspace runs passed, debug and release, including
1,064 UI tests (3 ignored) in each profile. Formatting, strict workspace clippy,
`git diff --check` and the rebuilt bundle/signature checks passed. Native visual
and Windows/Linux acceptance remain pending.

## Compact lane-header Add buttons (2026-11-17)

Markers, Tempo and Signature now have right-aligned compact Plus buttons. They
reuse the shared panel-button surface with an 18-point square footprint and
12-point icon, centered in each row with the existing right inset. Standard
panel buttons retain their geometry. The collapse hit region excludes the Add
button, and hidden/collapsed lanes do not display an ambiguous Add action.

Buttons and header context Add share one playhead action. Sections use New
section and one bar; tempo/signature events inherit the active value immediately
without opening a creation dialog. Existing tempo/signature events at the frame
are selected instead of duplicated. All commands retain edit/job/playback guards
and ordinary history transactions.

All 16 focused arrangement tests passed. New coverage clicks all three controls,
checks their compact geometry, exact playhead placement, active 90 BPM and 3/4
inheritance, one-step Undo, duplicate event handling, no collapse or creation
dialog, and disabled controls during a conflicting job. Formatting, strict
workspace clippy and `git diff --check` passed. The macOS debug bundle was rebuilt
and its ad-hoc signature verified. Full workspace results follow below; native
pointer and cross-platform acceptance remain separate.

Both final native-access workspace runs passed, debug and release, including
1,066 UI tests (3 ignored) in each profile. Formatting, strict workspace clippy,
`git diff --check` and the rebuilt bundle/signature checks passed. Native visual,
pointer/device and Windows/Linux acceptance remain separate pending checks.

## Playhead across arrangement lanes (2026-11-17)

The Timeline playhead line starts at the ruler bottom and draws over the
arrangement lanes and tracks. The egui regression checks its endpoints and its
paint order over a marker fill.

On macOS arm64, the arrangement-filtered UI run passed all 21 tests.
Formatting, strict workspace Clippy and `git diff --check` passed.

- `cargo test --locked --offline --workspace` passed with macOS host access.
- `cargo test --locked --offline --workspace --release` passed with macOS host
  access.

Each workspace run passed 1,075 UI tests with 3 ignored. The sandboxed runs
stopped at Audio Unit registration. Native visual checks were not run.
