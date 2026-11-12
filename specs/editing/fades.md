# Audio clip fades and crossfades

Initial implementation: dedicated edge handles, midpoint shaping and selected-group
settings in Timeline and the shared audio Editor. For operation, see
[Timeline](../../docs/views/timeline.md#fades-and-crossfades).

## Model and curve

`core::ClipFades` stores two `ClipFade` values on an audio Clip: `length_frames`,
`midpoint`, `midpoint_position` and `crossfade`. Missing/default fields retain the legacy linear
5 ms boundary policy. MIDI/beat clips reject non-default fades. Validate finite
midpoint levels in 0.15–0.85 and positions in 0.15–0.85 and reject unsupported frame bounds through
`Edit::SetClipFades`. Core edits remain atomic.

The shared gain curve is a piecewise cubic Hermite S-curve through `(0, 0)`,
`(midpoint_position, midpoint)` and `(1, 1)`. Both endpoint slopes are zero.
For `x = midpoint_position`, `y = midpoint`, set secants `d0 = y / x` and
`d1 = (1 - y) / (1 - x)`. The shared knot slope is
`min(1.5 * ((1 - x) * d0 + x * d1), 3 * d0, 3 * d1)`. The unconstrained slope
matches second derivatives; the limits keep the cubic control polygons monotone
and prevent overshoot. The two segments share that slope for a smooth join.
Centered controls produce the standard smoothstep `3t² - 2t³`.

Missing midpoint positions default to 0.5. Deserialize legacy coordinates from
the previous 0.01–0.99 range by clamping into the new bounds; reject nonfinite
or previously invalid coordinates. Existing custom fade control points within
the new bounds are retained but now use S-curve interpolation in both rendering and painting;
this intentionally supersedes the previous power curve. Fade-out uses reversed
progress. Custom durations replace outer boundary fades; repeat-seam anti-click
fades remain independent and combine by minimum gain. Zero duration always uses
the original linear anti-click boundary fade.

Effective ordinary fade durations are limited to `length_frames - 1`. Overlapping
in/out envelopes combine by minimum gain. Retain saved
durations across compatible copy, trim, split and placement operations and clamp
their effective values without changing the project during painting. Stretch
and sample-rate changes rescale saved fade lengths with checked arithmetic.

## Fixed-boundary transitions

Crossfades leave all logical clip positions, source offsets and lengths intact.
An outward left edge uses preceding source frames across the preceding clip's
last range. An outward right edge uses following source frames across the next
clip's first range. Adjacent clips must touch, be unmuted audio and have
matching channel layouts. Cap duration at half both clips and the owner's
available stretched-source handles for non-looped owners. A looped owner has no
external source-handle limit: extend its repeat range in either time direction
using the saved phase modulo the repeat length. Preserve the repeat base and
source offset. Apply the existing repeat-seam fade to both contributions,
including extended owner audio. The shared signed source mapping and shifted
repeat phase keep playback and waveform continuations aligned. This includes
one looped clip or two looped clips. No timeline overlap is introduced. Existing
saved crossfades on looped clips become active when the join is eligible; no
manifest schema change is required.

For left-edge transitions, owner weight follows the curve from zero to one;
for right-edge transitions it follows one to zero. The adjacent clip receives
the opposing curve `1 - mirrored.gain(progress)`, where `mirrored` retains the owner time position and uses `1 - midpoint` as its level. Both curves pass through the saved time/gain point. Vertical edits change crossing gain; horizontal edits change crossing time. The gain sum can rise above or fall below unity. This intentionally replaces the previous unity-sum rule for saved crossfades. Apply each clip's gain before track effects/mixing.
Selecting a new crossfade clears the neighbor's corresponding fade. Editing an
ordinary fade at a shared edge clears a competing neighbor-owned crossfade in
the same transaction. If later placement introduces competing saved crossfades,
the earlier clip's outgoing transition takes priority. An unavailable transition
falls back to the original automatic boundary fade while retaining saved data.

`engine::schedule::AudioTrack` prepares transition ranges, envelopes and source
readers outside the callback. Active transitions read both source contributions;
disk-backed extended sources have independent reader state and both readers must
be ready before playback advances. Account for transition/read storage in the
renderer memory estimate. The shared scheduled path covers live playback, seek,
looping, effect processing and export. Random sample rendering uses that path
when transitions are present. No callback allocation or file I/O is added.

## Shared presentation and gestures

Audio headers retain their 22-point height. Reserve the following 8 points for
fade handles before waveform painting. Use 6-point square handles centered in
that strip and a 6-point midpoint circle on the curve. Hit areas are 10 points.
Map full level to the square handle's top edge, 23 points below the clip top
(its center stays at 26 points). Curves attach to the top rather than the center
or waveform boundary. Fill the area
above the sampled curve with a translucent black background mesh behind the waveform; keep header
text and the area below the curve untouched. Paint outlines after fills.
The midpoint can move in both dimensions: horizontal changes normalized position,
vertical changes level. Keep duration fixed; clamp position to 0.15–0.85 and
level to 0.15–0.85. Share these bounds with core edit validation and settings.
Sample the curve through the saved point exactly. Crossfade presentation uses
the neighbor's opposing ramp so its full-level endpoint also meets the
outward handle's top. Reverse displayed time position when mapping to saved owner
settings, but retain the displayed gain level. Both curves meet at that control
point. The new crossing-gain behavior supersedes the previous fixed half-gain crossing.
Duration drag direction is relative to the clip edge: inward selects Fade,
outward selects Crossfade. Clamp unavailable extensions. Paint extended curves
again over the neighbor so later clip fills cannot cover the transition.

`ui::clip_fades` supplies the shared geometry, curve drawing, hit testing,
preview and settings. Capture the fade gesture on press and retain its identity
until release. Keep project data unchanged during handle previews. Release
commits one core edit; Escape, focus loss or blocked editing cancels. Preserve
existing trim, shared-boundary, header-loop and modifier-stretch gestures outside
these hit areas. Before starting a gesture, the platform stretch modifier
(Option on macOS, Ctrl on Windows/Linux) hides and disables fade/crossfade
endpoint and midpoint handles in both views, including Editor's lower waveform.
They must not capture hover or presses while that modifier is held; normal
clip-edge stretch handling takes priority. Curves and waveform gain remain
visible. An already captured fade gesture retains its identity and controls
until release/cancellation, including if the modifier changes mid-drag.
Handle gestures preserve an existing whole-clip selection when the owner is
selected; otherwise select the owner alone. Capture selected audio snapshots on
press and skip MIDI/beat clips. Apply one signed duration delta to each edge,
including each edge's own Fade/Crossfade sign. Apply midpoint level and normalized
position deltas relative to each snapshot, using the dragged clip's duration for
the horizontal delta. Clamp each target independently. Zero-duration midpoints
remain unchanged. Preview neighbor-envelope cleanup as well as target changes.
Publish the ordered SetClipFades edits through one Edit::Batch on release;
resolve each edit against preceding neighbor cleanup so stale opposite edges
cannot be restored. Preserve selection, cancellation and no-op releases. Provide accessible
names and tooltips. Reuse canonical dropdowns and the existing guarded Clip
settings body in both views.

Place a separator before each fade section, with equal spacing above and below.
Center subheaders in the standard settings row height so their text padding
matches the row before the divider. Use secondary-color
Fade in/out subheaders and standard ControlPanel field rows with the same label
width as Name and Gain. Each section has Mode, Duration, Shape and Position rows. Mode fills the value
column and uses Small text for both its caption and popup items. Numeric values reuse Gain’s plain numeric label/font, vertical drag,
Shift fine adjustment and double-click text editing; Enter/focus loss applies
a finite bounded value and Escape cancels.
For an audio selection, show Fade in/out mode, duration in ms, Shape and
Position. Shape maps the displayed point level to −70%–+70%; Position maps its
horizontal fraction to 15%–85%. Zero Shape and 50% Position give a balanced S-curve. Settings and
handle previews read the same model/curve. Disable unavailable Crossfade choices
and shape/position editing at zero duration. Browsing/painting never clamps saved state
into a new edit. For multiple selected audio clips, show one selected audio clip's values and
explain the reference behavior. Settings apply relative duration, level and
position changes to all selected audio clips with independent limits. Mode
changes set the requested mode on each selected audio clip. Skip non-audio clips.
Commit the settings action through the existing batch transaction.

## Verification

Cover curve endpoints/midpoint/monotonicity, legacy behavior, short clips,
validation rollback, source handles and non-overlap, project round trips,
rate/stretch scaling, resident/disk rendering parity, seeks/loops and callback
allocations. Crossfades draw both opposing gain curves over the same time span,
including neighbor overlays. Sample its opposing gain through the same
visual gain-to-height mapping. Keep one shading layer and one set of editable handles.
Curve lines and both marker borders use dark ink. Square and circular markers
have transparent bodies with one-point dark borders, independent of the clip color.
For ordinary fades, a one-point dark vertical guide at the duration handle X runs from the handle
bottom to the clip bottom for nonzero fades, marking the fade-in end/fade-out
start. It is a passive indicator, clips to its host clip and is painted before
markers and selection outlines. Crossfade guides follow the shared neighbor
clipping path so they appear once.

UI checks cover top-attached endpoints, transparent marker bodies and dark borders/curves, above-curve shading, two-axis shape
movement, legacy position migration, inward/outward gestures, inspector edits,
preview-only changes, release, cancellation and waveform geometry. Record actual
checks in [validation history](../validation/README.md). Native appearance,
listening and cross-platform pointer behavior require separate checks.

## Crossfade header and background presentation

Extend the owner header by the active crossfade duration into its neighbor;
shift the neighbor header boundary by the same amount. Apply this in either
direction and during previews, using source-limited active durations. This is
presentation only: retain fixed logical clip ranges. Clip each header slice to
its current host block so saved clip order does not affect the result.

Paint channel backgrounds first, then one translucent black attenuation mesh,
then waveform peaks in their original colors, then curves and markers. Crossfade
shading combines two independent masks: below the neighbor curve and above the
owner's opposing curve. The original side between the curves stays clear;
the extended side receives two composited layers. The upper and lower caps
receive one layer. Put both masks into one mesh and apply that mesh once per
host, so intentional overlap doubles attenuation without duplicate host passes.
Ordinary fades shade only above their curve. Keep
header text outside the attenuation mesh. Draw the owner header extension in
the neighbor's background pass. Dark curve ink and transparent marker bodies
supersede the previous secondary curves and white marker bodies.

Half gain is centered in the waveform area (below the 30-point header/handle
band), while full gain stays at the duration handle top. Map gain `g` to Y as
`bottom - g * (bottom - handle_top) + 2 * (waveform_top - handle_top) * g * (1-g)`.
This smooth correction vanishes at zero/full gain and applies to both curves
and the midpoint marker; it does not change the audio envelope.

Draw both source waveforms in each crossover span. Extend an outgoing owner's
waveform past its logical end using its following source frames; extend an
incoming owner's waveform before its logical start using preceding source
frames. Reuse the normal prepared-audio and original/stretch lookup paths and
source extrema painter. Keep source time mapping unchanged. Scale each visible peak column by its envelope gain. Ordinary fades use the saved fade curve; crossfade continuations use the owner curve and the receiving waveform uses its opposing curve. Resolve active limits and drag previews before painting. This peak display approximates gain within each pixel column; it is not a rendered mix.
Clip continuation painting to the receiving host span and paint it after that
host's peaks and before either curve. Body outlines follow the same visual
boundaries as headers; retain logical project ranges and audio behavior.

Do not add neutral rectangular borders around audio waveform bodies or
crossfade headers, including neighbor header extensions. Retain fade guides,
curve strokes and the single outer selection outline;
MIDI and beat clip outlines retain their existing behavior.

Show dark crossfade curves only while the pointer is inside the visible crossover
span, or while its owner edge is actively dragged. Hide both curves when the
pointer leaves. Keep background masks, waveforms and ordinary-fade guides visible; ordinary fade curves
retain their existing visibility.

Curve strokes are one logical point wide. Paint fade handles and midpoints last
with the lane clip rather than clipping them to their owner body. Repaint adjacent
controls over the current host with a three-point control allowance so a shared
boundary never cuts a control in half, regardless of saved clip order.

At the viewport edge, inset zero-duration squares by their radius so they remain
whole; use the same center for hit testing. Nonzero curve endpoints retain their
frame-derived position. Disposable source-preview IDs do not add live controls.

Complete a selected owner's visual outer outline in the neighbor's foreground
pass, clipped to that host span. Its header-top and body-bottom edges continue
across the crossover to the visual endpoint. Do not stop at the logical edge or
add neutral header/body borders. Use the same preview range and selection stroke
as the owner's pass; retain saved-order independence.

Show fade handles and midpoints only while their visual owner clip is hovered,
or while that owner has an active fade drag. Use the complete visual range,
including its crossover, for hover ownership. Clicking or double-clicking in the
crossover resolves to the visual owner for selection/editor opening, preserving
selection modifiers. Keep shared handle and viewport clipping allowances.

Crossfade guides are hover-only at both crossover edges. Draw each with a
one-point dark stroke from the header bottom (22 points below the clip top,
including the handle band) to the clip bottom. Crossover shading starts at the
same header bottom, leaving no unshaded strip. Use the shared header height
for the header fill, shade boundary and guide tops. Curve endpoints retain
their handle-top attachment at 23 points. Inset each stroke center by half a point toward the overlap to
keep the full stroke inside the host clip. Show the pair only when the crossover
is hovered, including during a drag; dragging outside does not retain guides.

Compound source wrappers can delegate an edge through `inherit_source`. Captured
member clocks retain their envelopes; edits and new cuts use the boundary rules
in [Audio Merge](audio-merge.md).

## Rendered bounce edges

[Rendered bounce sources](bounce.md) also validate `inherit_source` edges. Their
PCM already contains the captured clip envelopes. Full-source loops of the main
file and routing leaves must not apply those edge fades twice. Cropped loops and
explicit new cuts/fade edits retain the ordinary envelope contract.
