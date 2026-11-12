# Beat model and shared rendering

Date: 2026-10-31. Status: implemented; native platform acceptance remains open.

See the [requirements](beat-editor.md), [delivery plan](beat-tracks-plan.md) and [user guide](../../docs/views/beat-editor.md).

## Saved ownership and compatibility

`TrackKind::Beat` requires one boxed `Track::beat` container and forbids a separate MIDI `instrument` payload. Audio, MIDI and Group tracks cannot carry that container. Missing optional beat fields retain legacy manifests and their existing serialization. Unknown track types or beat revisions are rejected.

`BeatInstrument` version 1 owns a stable container ID, ordered `BeatSubtrack` values and one fixed `InstrumentBusSettings` Output. It starts empty. Add appends the first available `Subtrack N` name. Each subtrack has its own ID, name, mute/solo, trigger pitch, stop targets and normal `InstrumentSettings`. Its initial source layer is empty. Each instrument owns its own fixed LFO and Envelope lanes.

The subtrack's `main` bus owns its Output lane, canonical gain/pan and strip-collapse state. Its Output panel exposes mute/solo, gain/pan and the shared meter. Its compact Tracks row always mirrors the four mix controls; source layers are added through the selected subtrack’s standard Layers Plus. No second equivalent mix stage exists. The final beat Output stays outside subtrack ordering and cannot be removed or renamed as a subtrack. It may contain effects and local Macros, but no kit-wide LFO/Envelope pair or source layer.

Instrument lookup and source-layer drag capture use stable owner IDs for both ordinary MIDI tracks and nested beat subtracks. `Project::instrument_owners` publishes parent/owner/settings tuples for shared source discovery. Timeline clip-drag navigation searches those owners for Sampler/Granulizer and selects the real parent beat track and the matching subtrack owner; a completed source drop retains the original Timeline clip. Read-only nested presentation snapshots copy only parent metadata and the requested instrument, without cloning siblings, effects or clips. They do not replace the real Project or parent-track selection. Missing-effect load diagnostics use the same complete effect-chain owner traversal as core validation, including beat source layers, subtrack Output and final Output. Opaque settings remain retained and warnings identify their owner. Component/effect transfers reuse existing metadata, scope checks, transactions and graph-copy helpers. Instrument copies assign fresh owner, provider, source, effect and route identities and remap local references. Track duplication follows the existing settings-only behavior: no clips or assets are copied. Stop targets remap to copied siblings.

Project validation checks required/forbidden payloads, owner/effect/sample identities, sample asset/range references and instrument-local modulation. Sibling discovery lists are never merged. Final Output effects can reference its local Macros only. Group collapse changes presentation without changing pattern content or disabling processing.

## Embedded content and edits

`Asset::beat_pattern` holds a `BeatPatternSource` with the same ID as its Asset. It binds a versioned pattern to one beat container ID. The source uses `kind: beat_pattern`, `path_kind: embedded` and Embedded storage; no media path is decoded or copied. Asset payloads cannot contain both beat and MIDI content. `Asset::is_audio` and `pattern_length_ticks` centralize classification and timing.

A pattern has 1–16 bars in 4/4 and Quarter, Eighth, Sixteenth or ThirtySecond resolution. Defaults are one bar and Sixteenth. Coordinates use 960 ticks per beat; the finest slot is 120 ticks. Each enabled `BeatStep` has an ID, subtrack ID, tick and velocity 1–127. Editor enables at velocity 100. Trigger pitch is subtrack-owned, defaults to MIDI 60 and accepts 0–127. Duplicate IDs/positions, nil bindings, invalid values and hits shorter than one project frame are rejected.

Grid edits retain all stored steps. Hits outside the selected length or between visible grid slots are silent; restoring the grid restores their IDs, positions and velocities. Deleted-row content remains dormant and cannot bind to a new row by name or index. Reorder does not rewrite patterns.

`Edit::Beat` applies structural/mix edits through cloned-container and final Project validation. `Edit::ReplaceBeatPattern` creates a fresh embedded source for the active clip. Other clips retain their source IDs and content. The normal history owns model snapshots and shared prepared buffers; no-op UI releases do not publish edits. Invalid grid/source ranges or clip overlaps reject the complete transaction. Rate/tempo changes use the shared tick/frame and bounded source-range helpers; sample inputs inside subtracks rescale with ordinary instruments.

Every placement is checked against its source's container ID. Cross-beat-track moves, copies, paste and new-track drops are rejected. Same-track arrangement operations retain the normal trim, repeat, split, stretch and priority-placement rules.

## Preparation and processing

`Session::plan` retains the logical project. Beat compilation runs in the existing output preparation worker or offline renderer. `RenderPlan::prepare_beats` is idempotent and creates private processing tracks only: the parent bus, final Output bus and one ordinary instrument runtime per subtrack. Derived clip/asset identities remain deterministic across preparation. Initial device setup creates only a silent, track-free callback plan with the same project clock. It submits the logical plan through the existing preparation worker for first Play, device recreation and first audition. Publication replaces that initial renderer only with complete prepared content and monitoring bindings. Prepared audition audio can play independently while this work runs. Publishing the first stopped project renderer preserves converted audition samples; playing transport changes retain their normal buffer invalidation. Pending Stop/Seek/Play requests retain the shared latest-revision guards. The worker compares logical plans before compilation and reuses its compiled plan across Warm and Plan requests when processing inputs match; processing changes invalidate it. Source layers and subtracks process in stable identity order so presentation reorder cannot change their audio summation.

The lowered final Output is a stereo Group bus even though its instrument payload carries Output controls. Prepare Group/Return balance coefficients from bus kind before inspecting source instrument channel semantics. Center retains unity per stereo channel. Source tracks retain mono equal-power or stereo balance behavior; internal layer/Output normalized Pan is unchanged. Absolute-level tests cover all six rates, mono Oscillator and known stereo Sampler PCM through each bus stage, sends/Returns and Master.

The existing MIDI scheduler maps grid hits into the clip's source, stretch, trim and repeat clock. Beat instruments reserve one voice per subtrack regardless of hit count. Retrigger uses the existing 5 ms voice transition. Note-off follows one grid step; normal Sampler trigger mode, Envelope and Amplitude determine duration. The normal layer effects, provider context, latency alignment, final Output, parent effects and Master stages remain shared.

Directed stop links require distinct existing siblings and cannot form cycles. Local mute/solo filters intents before stop compilation. A muted solo subtrack remains silent and excludes non-solo siblings. All remaining intents issue stops before simultaneous hits are admitted, even if another intent suppresses their own hit. A→B→C therefore suppresses B and C when all hit together. The target fades over 5 ms and clears its active voice, including a one-shot Sampler; bus effects retain their tails. Seeking and looping use the same prepared source-clock stop schedule.

A retrigger during a stop fade starts its shared voice transition from the last
effective faded contribution. Retain the previous stop multiplier separately
from clip boundary fades, apply it to the saved voice/layer transition inputs
once at note-on, then remove the stop multiplier for the new hit. Keep layer and
Output effect histories alive. No callback allocation or new audio path is added.

Sampler requests include every subtrack's trigger pitch and clip transposes. Banks, decoding, peaks and pitch/stretch variants use existing workers and saved caches. Source/configuration edits publish complete prepared sessions; compatible pitch/content edits use existing background preparation. No beat event, stop or callback path allocates, locks, reads files or constructs units.

Clip gain is applied once after the final Output effects and before the parent track effects. The prepared gain envelope retains the preceding unmuted clip gain through gaps and retained bus tails, rather than reverting to unity at the clip end. At a following clip, its gain takes over the complete shared Output bus over up to 5 ms, shortened at a following clip or loop boundary; a silent target is exact at the clip start. A leading loop gap uses the preceding iteration's gain, and a clip crossing the loop start transitions from that gain. The prepared envelope is periodic and stateless; lookup allocates nothing on the callback. Virtual child clips retain timing and transpose with unity gain. Prepared bus range indexes select clip gain without scanning clips or allocating on the callback. Each lookup uses the content clock after child processing, final Output effects and sibling alignment delays. Prepare that accumulated delay with the renderer and use the shared loop-aware clock mapping for playback, seek and export.

Output meters and parameter snapshots use the same stable subtrack/unit IDs as normal instruments. The final Output reads its shared stereo bus meter. These processing owners never appear as extra Mixer strips or project tracks. The Tracks browser presents subtracks as virtual children of their real beat parent.

## UI and acceptance

Editor headers and step rows copy presentation metadata without processing
graphs. Beat thumbnails cache overview levels by immutable source, project
revision and ordered subtrack IDs. Coarser levels combine subpixel hits; sorted
start ticks limit traversal and drawing to the visible source interval. Detailed
thumbnails retain the shared MIDI trim, repeat, phase and stretch mapping.
Keep full ordered row indices rather than mapping rows into MIDI's pitch range.
Timeline and Editor resize painters pass staged beat content alongside staged
MIDI/audio sources. Disposable source-growth IDs do not enter the live overview
cache; previews retain hit positions and leave the session unchanged.

Editor and Instrument share subtrack selection, name/context controls, mix widgets and release-only group reorder. One shared context menu owns the full subtrack header rectangle, including controls, meters and padding, in the Editor and Tracks browser. Reuse the canonical compound-control popup opening guard; secondary clicks do not trigger primary control actions or project edits. Selecting a subtrack name, control or header background clears final Output targeting through one selection transition. Drag cancellation covers Escape, focus loss, outside release and model revision changes. Delete requires subtrack-header focus and does not act on text, steps or units. Removing the primary row selects a deterministic neighboring primary; removed IDs are pruned. Selection and source identity survive view changes.

Editor indexes hits by stable subtrack ID and source tick for each immutable asset revision, including hidden hits. Update the index after an edit within the same paint so repeated cells remain consistent. Generate source cells only for the visible time interval; construct row widgets only where they intersect the viewport. Retain complete row rectangles for group insertion and edge scrolling, and keep active controls and rename widgets until normal gesture/focus cleanup completes. Presentation culling does not remove content or alter clip mapping, selection or editing transactions.

Instrument opens one selected owner at a time. A selected subtrack reuses the standard MIDI Layers body with its header/Plus, local provider lanes and Output. The parent shows only final Output. Tracks contains compact child rows with canonical mix controls; parent disclosure only hides browser children. No nested source groups or hidden child drop geometry remain. Component registration controls lane compatibility and sources/providers retain their local scope. See [shared Tracks contract](instrument-tracks.md).

Automated coverage includes legacy/model round trips, isolated sources, transfer rejection, rollback, copied identities, source collection after original media removal, Save As/reopen, CLI validation/inspection/rendering, both view structures, pointer group reorder/delete/cancel, every supported processing rate, allocation-free audio, one-shot stops, live/export/seek parity, bounded voice memory, final Output gain and Macro routing. Native visual, device and Windows/Linux acceptance is recorded separately in validation; automated egui checks do not establish native pointer behavior.

## Beat body-edge source extension

Unlooped beat clips use `Project::pattern_trim_bounds` through the same Timeline
and Editor body-edge drag, snapping, group-preview and release-only resize paths
as MIDI. MIDI keeps its existing tick-domain bounds. Beat bounds remain in sample
frames: retain ordinary fractional-bar trims, allow outward growth beyond the
saved source, clamp to project zero/timeline limits and at most 16 source bars.
Header-edge looping and looped-clip body trimming retain their existing behavior.

`Project::trimmed_clips` delegates beat candidates to `beat_resize` before the
shared `apply_pattern_resizes` neighbor transaction. Grow immutable sources only
when necessary; retain source identity for trims and expansion within an existing
source. Keep other source users unchanged. Copy the original clip metadata,
retain its stretch, keep the opposite endpoint exact, and change only its frame
range, source offset and (when grown) asset ID. Extend the stored grid by whole
bars while retaining the exact dragged frame range. Hidden hits remain stored and
can reappear when the source grows. Inward trims on either edge must leave the
complete stored step vector, source ID and source metadata unchanged. Preserve
step IDs, subtrack IDs, start ticks and velocities, including steps hidden by
source length or subdivision. Expansion within the retained source restores
those steps without creating a new asset. These guarantees survive save/reopen
and undo/redo and apply to the shared Timeline/Editor gesture path. Merge is the
explicit operation that writes visible hits only into a fresh source.

For left expansion beyond source zero, prepend whole bars and shift every stored
hit by their tick count. Set the source offset to hide unused leading padding.
Compute padding from the difference between scaled old/new source lengths to
avoid rounding an extra bar into stretched sources. Retain hit IDs, velocity,
subtrack bindings and project timing within two frames of tick/stretch rounding.
Include hidden hits when bounding left padding; never discard them at the
16-bar limit. Validate new assets and the final neighbor edits atomically.
Previews and cancellation do not change project/history/dirty state; release
publishes the complete result through normal preparation and session history.

## Live transport loop tails

A continuous transport wrap releases instrument note gates without resetting their
voices. One-shot sources, envelope releases and bus effects finish across the
boundary; hits and directed stops in the next iteration retain normal priority.
Instrument clip-edge fades must not silence these voices during live looping. Gaps at
the loop start continue the retained voices with their preceding clip gain. A choke already fading at the wrap
continues toward silence and cannot return to full gain. Seek/reset still
reconstructs the destination instead of carrying voices from the old position.
Keep this path allocation-free and use the latency-compensated feed wrap when
processing lookahead is active. This applies to MIDI and beat instruments. Nonlooping clip boundaries retain
their existing behavior. Audio clips repeat only the selected region; existing
track, group and Master effect histories continue through the wrap.

## Merge beat patterns

`Project::merge_pattern_clips` dispatches MIDI and beat sources through their
shared selection validation and final snapshot publication. `merge_beat_clips`
accepts one or more clips on one beat track through the shared eligibility rule.
Single clips consolidate their visible range, including loops and trims; current
bounds do not need to prove prior trimming. This also permits full-length single
patterns and remains consistent after source-edge trims and save/reopen.
Accept gaps while retaining the exact outer frame range and existing hit times.
Gaps count toward the 16-source-bar limit and must preserve the common grid.
Reject overlaps and intervening unselected clips. All sources retain their
beat container binding. Require matching gain, transpose, mute, channels,
subdivision and stretch. Validate clip bounds, source offsets and repeat
length/phase against the common subdivision in source ticks. Convert frame
coordinates to nearest ticks; reject partial gates or off-grid hits.

Reuse Split/MIDI Merge's bounded occurrence mapping for source windows, phases
and stretched repetitions. Construct one embedded `BeatPatternSource` containing
visible hits only. Retain subtrack IDs, velocities and unique step IDs; use fresh
IDs for additional occurrences of shared steps. Do not awaken hidden hits or
rewrite existing assets, lane instruments, stop links or mix settings. Preserve
the earliest clip's ID, name and color. Clear source offset and repeat, retain
the common stretch, and preserve the exact selected frame range. The source uses
the common subdivision so each gate retains its one-step duration.

Round source length up to whole four-quarter-note bars, sufficient to cover the
retained frame range after stretch. Reject results beyond 16 source bars or 4,096
visible hit occurrences. Reject the full operation without project mutation on
error. Publish through existing preparation/history, select only the merged
clip, and undo/redo as one edit. Expose the same Merge command, Command+J/Ctrl+J,
context menus, native dispatch and view/text/gesture guards as MIDI Merge.

Single-clip consolidation uses the same occurrence, grid, length and hit limits
as group Merge. Materialize only the visible source window and repetitions;
retain lane IDs, velocities, subdivision and stretch. Preserve clip identity,
start and length, name/color and mix settings; clear repeat and source offset.
Keep original assets and lane instruments unchanged. The shared menu/shortcut
and preparation/history paths publish one selected clip and restore the original
trim, repeat and source reference with one undo.

### Hidden-step trim regression coverage

`crates/core/tests/beat_trim.rs` checks both-edge trims, serialization/reopen and
restoration at all six project rates with fractional tempo and optional stretch.
Require identical project/source data after restoring the original range,
including source-sharing copies and beat instrument/lane settings. Combined
length/subdivision masking must retain every stored step and reveal the original
IDs/velocities when source length and resolution permit them again.

The paired egui regression drives actual inward trims and expansion in Timeline
and Editor. Check preview-only movement, source identity/content, hidden/revealed
step hit regions, and undo/redo. This verifies the existing retention path;
no new DSP, source format or rendering implementation is required.
