# Beat model and shared rendering

Date: 2026-10-31. Status: implemented; native platform acceptance remains open.

See the [requirements](beat-editor.md), [delivery plan](beat-tracks-plan.md) and [user guide](../../docs/views/beat-editor.md).

## Saved ownership and compatibility

`TrackKind::Beat` requires one boxed `Track::beat` container and forbids a separate MIDI `instrument` payload. Audio, MIDI and Group tracks cannot carry that container. Missing optional beat fields retain legacy manifests and their existing serialization. Unknown track types or beat revisions are rejected.

`BeatInstrument` version 1 owns a stable container ID, ordered `BeatSubtrack` values and one fixed `InstrumentBusSettings` Output. It starts empty. Add appends the first available `Subtrack N` name. Each subtrack has its own ID, name, mute/solo, trigger pitch, stop targets and normal `InstrumentSettings`. Its initial source layer is empty. Each instrument owns its own fixed LFO and Envelope lanes.

The subtrack's `main` bus owns its nested Output lane, canonical gain/pan and strip-collapse state. Its Output panel exposes mute/solo, gain/pan and the shared meter. The compact header mirrors those output controls only when collapsed; expanded headers show name/disclosure and Add layer. No second equivalent mix stage exists. The final beat Output stays outside subtrack ordering and cannot be removed or renamed as a subtrack. It may contain effects and local Macros, but no kit-wide LFO/Envelope pair or source layer.

Instrument lookup and source-layer drag capture use stable owner IDs for both ordinary MIDI tracks and nested beat subtracks. Read-only nested presentation snapshots copy only parent metadata and the requested instrument, without cloning siblings, effects or clips. They do not replace the real Project or parent-track selection. Component/effect transfers reuse existing metadata, scope checks, transactions and graph-copy helpers. Instrument copies assign fresh owner, provider, source, effect and route identities and remap local references. Track duplication follows the existing settings-only behavior: no clips or assets are copied. Stop targets remap to copied siblings.

Project validation checks required/forbidden payloads, owner/effect/sample identities, sample asset/range references and instrument-local modulation. Sibling discovery lists are never merged. Final Output effects can reference its local Macros only. Group collapse changes presentation without changing pattern content or disabling processing.

## Embedded content and edits

`Asset::beat_pattern` holds a `BeatPatternSource` with the same ID as its Asset. It binds a versioned pattern to one beat container ID. The source uses `kind: beat_pattern`, `path_kind: embedded` and Embedded storage; no media path is decoded or copied. Asset payloads cannot contain both beat and MIDI content. `Asset::is_audio` and `pattern_length_ticks` centralize classification and timing.

A pattern has 1–16 bars in 4/4 and Quarter, Eighth, Sixteenth or ThirtySecond resolution. Defaults are one bar and Sixteenth. Coordinates use 960 ticks per beat; the finest slot is 120 ticks. Each enabled `BeatStep` has an ID, subtrack ID, tick and velocity 1–127. Editor enables at velocity 100. Trigger pitch is subtrack-owned, defaults to MIDI 60 and accepts 0–127. Duplicate IDs/positions, nil bindings, invalid values and hits shorter than one project frame are rejected.

Grid edits retain all stored steps. Hits outside the selected length or between visible grid slots are silent; restoring the grid restores their IDs, positions and velocities. Deleted-row content remains dormant and cannot bind to a new row by name or index. Reorder does not rewrite patterns.

`Edit::Beat` applies structural/mix edits through cloned-container and final Project validation. `Edit::ReplaceBeatPattern` creates a fresh embedded source for the active clip. Other clips retain their source IDs and content. The normal history owns model snapshots and shared prepared buffers; no-op UI releases do not publish edits. Invalid grid/source ranges or clip overlaps reject the complete transaction. Rate/tempo changes use the shared tick/frame and bounded source-range helpers; sample inputs inside subtracks rescale with ordinary instruments.

Every placement is checked against its source's container ID. Cross-beat-track moves, copies, paste and new-track drops are rejected. Same-track arrangement operations retain the normal trim, repeat, split, stretch and priority-placement rules.

## Preparation and processing

`Session::plan` retains the logical project. Beat compilation runs in the existing output preparation worker or offline renderer. `RenderPlan::prepare_beats` is idempotent and creates private processing tracks only: the parent bus, final Output bus and one ordinary instrument runtime per subtrack. Derived clip/asset identities remain deterministic across preparation. Source layers and subtracks process in stable identity order so presentation reorder cannot change their audio summation.

The existing MIDI scheduler maps grid hits into the clip's source, stretch, trim and repeat clock. Beat instruments reserve one voice per subtrack regardless of hit count. Retrigger uses the existing 5 ms voice transition. Note-off follows one grid step; normal Sampler trigger mode, Envelope and Amplitude determine duration. The normal layer effects, provider context, latency alignment, final Output, parent effects and Master stages remain shared.

Directed stop links require distinct existing siblings and cannot form cycles. Local mute/solo filters intents before stop compilation. A muted solo subtrack remains silent and excludes non-solo siblings. All remaining intents issue stops before simultaneous hits are admitted, even if another intent suppresses their own hit. A→B→C therefore suppresses B and C when all hit together. The target fades over 5 ms and clears its active voice, including a one-shot Sampler; bus effects retain their tails. Seeking and looping use the same prepared source-clock stop schedule.

Sampler requests include every subtrack's trigger pitch and clip transposes. Banks, decoding, peaks and pitch/stretch variants use existing workers and saved caches. Source/configuration edits publish complete prepared sessions; compatible pitch/content edits use existing background preparation. No beat event, stop or callback path allocates, locks, reads files or constructs units.

Clip gain is applied once after the final Output effects and before the parent track effects. Virtual child clips retain timing and transpose with unity gain. Prepared bus range indexes select clip gain without scanning clips or allocating on the callback. Each lookup uses the content clock after child processing, final Output effects and sibling alignment delays. Prepare that accumulated delay with the renderer and use the shared loop-aware clock mapping for playback, seek and export.

Output meters and parameter snapshots use the same stable subtrack/unit IDs as normal instruments. The final Output reads its shared stereo bus meter. These processing tracks never appear as extra Mixer strips or project-browser entries.

## UI and acceptance

Editor and Instrument share subtrack selection, name/context controls, mix widgets and release-only group reorder. One shared context menu owns the full subtrack header rectangle, including controls, meters and padding, in both expanded and collapsed groups. Reuse the canonical compound-control popup opening guard; secondary clicks do not trigger primary control actions or project edits. Selecting a subtrack name, control or header background clears final Output targeting through one selection transition. Drag cancellation covers Escape, focus loss, outside release and model revision changes. Delete requires subtrack-header focus and does not act on text, steps or units. Removing the primary row selects a deterministic neighboring primary; removed IDs are pruned. Selection and source identity survive view changes.

Editor indexes hits by stable subtrack ID and source tick for each immutable asset revision, including hidden hits. Update the index after an edit within the same paint so repeated cells remain consistent. Generate source cells only for the visible time interval; construct row widgets only where they intersect the viewport. Retain complete row rectangles for group insertion and edge scrolling, and keep active controls and rename widgets until normal gesture/focus cleanup completes. Presentation culling does not remove content or alter clip mapping, selection or editing transactions.

Instrument composes a compact one-row group header, followed by source layers, local provider lanes and a separate Output child using the MIDI instrument body. The shared track-group chevron controls child visibility as independent UI state, initially expanded. All child lanes, including the subtrack Output, hide on group collapse. Compact header controls retain access to the same bus. There is no nested Layers title row. Child lane headers use the shared track-panel indent; unit bodies and column dividers remain aligned. The group selection border encloses its children. All lane rails use the shared toolbar. One final Output remains pinned outside group scrolling. Component catalog registration still controls which units can enter each lane; source/provider menus stay local.

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
can reappear when the source grows.

For left expansion beyond source zero, prepend whole bars and shift every stored
hit by their tick count. Set the source offset to hide unused leading padding.
Compute padding from the difference between scaled old/new source lengths to
avoid rounding an extra bar into stretched sources. Retain hit IDs, velocity,
subtrack bindings and project timing within two frames of tick/stretch rounding.
Include hidden hits when bounding left padding; never discard them at the
16-bar limit. Validate new assets and the final neighbor edits atomically.
Previews and cancellation do not change project/history/dirty state; release
publishes the complete result through normal preparation and session history.
