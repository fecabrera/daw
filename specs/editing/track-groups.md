# Track groups as processing buses

## Accepted behavior

Groups are nested stereo buses with the existing track controls, meters, effects lane, effect processors, and unit drag behavior. Sources and clips stay on ordinary audio/MIDI tracks. Timeline supports track membership changes through the existing release-only track drag gesture. Group movement includes its complete subtree. Ungroup retains children; deletion removes the subtree. Edits are undoable.

Shared `panels::GroupPanel` geometry indents children by 8 logical points per
nesting level, capped at three levels (24 points). Keep right edges aligned.
Track headers, beat subtrack rows and other nested panels using this helper
share the same spacing; toolbar padding does not determine this indentation.

Timeline track headers have a 16-point leading background strip in the track's
resolved base clip-body color, including nested headers. Beat subtrack headers
inherit their parent track color. Expanded Group and Beat parents continue
their color below the header as an 8-point gutter through visible descendants.
Use the remaining actual indentation width at the three-level cap, so deeper
groups do not paint over child headers. Clip gutters to the scrolling viewport
and preserve continuation when an ancestor header is virtualized offscreen.
Reserve the 16-point strip before the existing seven-point left content inset.
Keep icons, disclosure arrows, labels and controls outside the strip. Preserve
the right meter inset, selection fills, row heights and project data. Edge-drop depth uses
the same shared eight-point step as the painted hierarchy.

## Model and transactions

`core::TrackKind::Group` identifies buses. `Track::group_id` stores the optional stable parent ID; `group_collapsed` stores saved disclosure state for Group and Beat tracks. New beat tracks start collapsed; beat subtracks remain instruments, not project-track children. Missing fields default to no parent and expanded. The existing experimental manifest stores both fields; legacy projects remain flat.

`Project::tracks` is preorder: parents precede their children and each subtree is contiguous. Validation rejects missing or non-group parents, cycles, split subtrees, group clips/instruments, and collapse on non-groups. General ID and effect validation still applies.

`Project::add_group_track` clones the model, appends a default empty root track, assigns Group kind and the next group name, validates and publishes atomically. It preserves existing rows, selection-independent membership and IDs; failure retains the original project. Add group is separate from grouping selected tracks. `Project::add_track_in_group` stages the standard audio/MIDI/beat/group constructor and a move to the parent subtree’s end, validates and publishes once. Group-context creation preserves all surviving row identities/order and has no intermediate root edit or extra undo step. Reject non-group or missing targets atomically.

`Edit::GroupTracks`, `UngroupTrack`, `MoveTrack`, and `SetGroupCollapsed` use `Project::edit`/`edited`. Grouping filters selected descendants, extracts complete selected subtrees in project order, and creates a bus under their nearest common ancestor. Invalid input rejects the entire edit. `MoveTrack::index` is a global insertion index after removing the moving subtree. `can_move_track` validates previews without mutation. Same-position drops do not commit. `ReorderTrack` retains its flat-track behavior and uses the same move transaction within a parent.

Delete and Duplicate use subtree roots to avoid editing selected descendants twice. Duplicate follows existing empty-track-copy behavior: clips are empty, track/effect/sampler identities are fresh, internal parent IDs are remapped, and instrument-local and route identities retain their existing owner scope. Ungroup promotes direct children to the removed bus's parent and discards only the bus settings and chain.

`visible_tracks` hides descendants of collapsed groups without affecting processing. `track_mix_audible` prepares local sibling gates: descendant solos open the ancestor path; each sibling set applies shared `mix_audible` precedence. Effective `Project::audible` also checks ancestors. The renderer uses local gates so each bus applies its own mix stage exactly once.

## Shared rendering and clocks

`engine::buses` prepares child indices, roots, reverse-preorder group indices, and stereo scratch outside the callback. Leaves reuse `TrackProcessor`: source/instrument → track effect chain → sibling alignment → gain/pan. Groups sum direct child outputs in project order, run `EffectChain`, apply their sibling alignment delay, and apply existing stereo balance/gain/audibility ramps. Master sums only roots. `sample_buses` is shared by callback playback, offline export, and cache preparation. Muted branches continue processing. Group effects use existing meter binding and bypass lifecycle traversal.

The latency planner computes each subtree latency bottom-up: a group adds its serial chain latency to the longest child subtree. Each child receives the difference from that sibling maximum; roots align to the maximum root latency. A group's effect input clock subtracts the longest child latency with the shared loop-aware `delayed_frame`; serial effects subtract their own preceding latency as usual. Master subtracts the complete root-path latency. Bypass retains fixed processor latency.

Routing changes invalidate compatible DSP inheritance, even at zero latency, so native output reconstructs exact history on its existing preparation worker. Compatible controls retain shared processor history and 5 ms gain/pan ramps. Collapse is excluded from processing-plan cache comparison. Routing topology, processing settings, and prepared media still determine checkpoint compatibility.

Two-worker cache preparation runs independent leaf tracks in parallel. Group processors retain their ownership without advancing on leaf workers. The coordinator restores all processors, then runs dependent groups and Master per frame through the serial shared bus stage. Cancellation never publishes partial state; serial fallback uses the same processing order. Renderer memory accounting includes routing metadata, scratch, and hierarchical alignment buffers. Callback sampling and compatible retargeting allocate nothing.

## UI reuse and limits

Add audio track, Add MIDI track and Duplicate track use the shared `TrackAction` guards with `Context::text_edit_focused`, rather than the general egui keyboard-input flag. Non-text control focus, including another pane or the Tracks header buttons themselves, must not disable these actions. Retain text editing, valid selection/view, window focus, job/dialog, disabled UI and active-gesture guards. Generic egui control drags also block track creation and duplication until release. Header buttons and menus share the same readiness and release-only dispatch.

The Tracks header Plus button opens a button-anchored dropdown through `menus::Dropdown::show_from_button`. Keep its icon-button geometry and use Add track for its tooltip/accessibility label. The dropdown and empty-area context menu call one `add_track_menu` body with Add audio track, Add MIDI track, Add beat track and Add group, followed by a separated Add subtrack in Plus when a beat is selected/displayed. Blank-space contexts omit Add subtrack. Group contexts expose the four creation choices for children of that group. Opening, hovering or dismissal must not construct a track or record history. Dispatch a selected choice through `perform_track_action` on release, retaining the shared readiness guards. Disabled triggers close an open dropdown.

Keep the native macOS and egui Track menus in this grouped order: Add audio track/Add MIDI track/Add beat track; Add group/Add subtrack; Rename track/Duplicate track; Group selected tracks/Ungroup tracks; Mute/Solo; Delete track. `TrackAction::ALL` supplies the order, and `starts_menu_group` supplies separator boundaries to both adapters. Track context menus use Rename/Duplicate; Group/Ungroup; Delete, with separators only between groups. Preserve action guards, shortcuts, dynamic selection labels and release-only dispatch. Native menu tags and selector lookup both use the same current `ALL` order.

Group/Ungroup use Command+G / Command+Shift+G (Ctrl equivalents off macOS) through shared `TrackAction::shortcut`/`matches_shortcut`. Text fields, modal jobs/dialogs, active drags, focus loss, and held-key repeats keep their existing guards. Native dispatch consumes the forwarded egui event once. Group/Ungroup use `TrackAction`, including native macOS menu dispatch, context menus, selection guards, and history. Timeline and Mixer use `visible_tracks`, shared names, `mix_controls`, existing faders/knobs/meters, and shared selection background fills without selection outlines. Audio, MIDI, Group, and Master headings align left in Timeline. Mixer centers the icon and visible name as one group; name editors retain left-aligned text. Group arrows use a borderless disclosure button with the catalog folder chevron painter, standard icon size, secondary idle color, hover/focus text color, tooltip, and expanded-state accessibility. Keep the 28×22-point hit area in Timeline and use the compact icon width below in Mixer; isolate collapse clicks from name dragging. Audio, MIDI, and Master headers use Audio Lines, Keyboard Music, and Boom Box through the shared header artwork painter. Timeline retains the 28-point icon slot, default artwork colors and shared 6-point gap before each name, including nested tracks. Mixer uses the compact geometry and contrast colors below. Timeline type icons start the existing guarded track drag handler; Mixer icons identify the track type without introducing reordering there. Reuse name editing, selection, previews, and cancellation. Reserve the full content-column width inside shared `meters::stereo_panel` before laying out controls, so short Master headers cannot shrink the column and pull the meters left. Track, group, nested-track, and Master meters share the control column's right inset. Timeline track drag reuses name/blank-header input without a visible grip, insertion-line painting, edge scrolling, cancellation, and release-only commit; core validates parent and insertion boundaries.

Mixer track-type icons, group disclosure arrows, header labels and Master use the same fixed white foreground on their colored or neutral headers. Keep that color during hover and focus. Use an explicit optional tint in the shared header painter, retaining the same artwork and Timeline/layer geometry and behavior. Mixer header rows measure the truncated label with the same font and wrap policy as its button, then reserve leading space to center the 16-point icon plus 4-point gap and visible text as one group. Name buttons retain left-aligned text next to the icon. Long names stay on one line within the 4-point minimum strip insets. Keep the 4-point icon-to-label gap and vertical padding in a 24-point header. Apply this to audio/MIDI, groups and Master without changing the control spacing below. Retain name-edit, selection and group-collapse interactions.

Timeline insets each complete child control panel, including the name, mix controls, and meters, by 8 logical points per hierarchy level (capped at 24 to retain usable controls). Remove the old name-only indent. Keep all timeline lanes at the fixed control-column boundary. Extend each group selection rectangle through its last visible descendant, using the shared selection painter and unclipped subtree geometry even when its header is virtualized offscreen. Nested row dividers own only their inset left edge; the workspace owns the root left edge and shared right border. The last child and all enclosing groups share the same bottom boundary. Do not add a second root-group left stroke. Group append previews use the full container bottom. Before/after drops use the hovered row's parent and the inset header position to choose ancestor scope, retaining that parent when reordering a child at its bottom edge.

Timeline and Editor track headers and regular Instrument layer headers use the shared 22-point gain/pan knobs centered in 28-point slots. Reuse `mix_controls::header_row` for a common second row: Mute/Solo at the left, Pan then Gain at the right, with a six-point M/S gap and four-point knob-slot gap. Retain the scaled `unit-ui` mappings, fine input, reset, capture and shared hover/drag tooltips. Use Regular 68-point headers in Editor and by default in Timeline; Timeline also has Compact 42-point and Large 100-point headers under the [Timeline display contract](../views/timeline-display.md). Retain layer expanded/collapsed heights and fixed Output placement, and keep selection/drag guards. Layer collapse stays at its first-row right inset. Output has only the right-aligned gain knob, without Pan, Mute or Solo. Timeline Master uses a single 42-point row with icon/name at the left and gain immediately before the meters at the right. Track header rails use the full content height: 28 points in Compact, 54 in Regular and 86 in Large. Master rails use 28 points. Retain seven-point outer meter insets, the six-point gap before meters and four-point stereo channel gap. Scale-free header meters omit the five-point endpoint-label inset; scaled meters retain their label space. Preserve meter mappings, reset, tooltips and clipping indicators. Track/layer/Output/Editor headers have no gain/pan text inputs or numeric draft state. The Editor dropdown fills its first row after the icon to the six-point gap before the meters. Mixer strips and unit/plugin body knobs retain their existing sizes and layout.

The group effects target is the existing `EffectChainTarget::Track(group_id)`. No new processor, panel, catalog registration, or effect transfer implementation is needed. Existing scope validation rejects a child's instrument source on a group effect. Groups do not publish modulation providers and do not host an Instrument view. Audio imports and clip placement reject buses. Custom sends, feedback, sidechains, and pre-fader routing are outside this change.

Installed clips and external files share `placement::drop_bounds` and `clip_drop_track` for row targeting. The drop workspace includes track controls and timeline lanes, bounded by the caller's viewport. Clip drags over a header use that header's track rather than falling back to the source track. Existing frame-delta, snapping, overwrite, source-type, and release-only commit logic then applies to the identified row.

Shared move previews use core `TrackKind::accepts_asset` to reject group buses and audio/MIDI source mismatches, including secondary members of a multi-track clip selection. Core project validation reuses the same asset-type policy. The resolved preview marks the entire group invalid and takes cursor precedence over Move/Copy feedback. Apply this guard to whole clips and selected time-range moves/copies. Release rejects any invalid incoming preview before editing, duplication, or worker preparation; keep selection, project data, and undo history unchanged, without an error dialog. This applies to audio/MIDI moves and copies, root/grouped tracks, and expanded/collapsed buses. Returning to a valid child track restores the existing move/copy gesture. Core placement validation remains authoritative for other failures, which retain normal error reporting.

External audio-file drags reuse `file_target` for both hover feedback and release eligibility. A rejected target sets the forbidden cursor and leaves `file_drop_target` empty, so MIDI-track and group-bus drops never enter the import path or change selection/history. This applies to MIDI-track and group headers and lanes, including grouped MIDI tracks and expanded/collapsed buses, before and after preview decoding and when a drop arrives without an earlier hover. A rejected hover clears an earlier valid target. Returning to an audio track reuses the same prepared preview audio and existing snapping/import behavior. Other unsupported file destinations use the same rejection feedback; Sampler drops retain their separate target path.

## Shared group-panel presentation

Timeline track groups use `panels::GroupPanel`
for depth indentation, control-column bounds and row separators. Its bounds inset
the left edge while keeping the right edge fixed. Expanded group headers use the
child depth for their bottom separator, leaving the gutter open. Both reuse
`ControlPanel::row_left_divider` for nested edges, the shared disclosure button,
and the shared selection-border painter. Keep project hierarchy/edit commands
with their respective owners; presentation helpers do not change project data.

## Regression coverage

- Core: legacy defaults, hierarchy validation, nested/nonadjacent grouping, cross-parent extraction, invalid/cyclic drops and atomic rollback, ungroup/delete/duplicate identities, collapsed visibility, solo path rules.
- Engine: unity parity, nested gain/pan, shared nonlinear processing, nested latency alignment, serial/two-worker exact history under loops, collapse cache compatibility, routing invalidation, callback allocation guard and group meter output.
- Project: save/reopen and render parity, undo/redo, unchanged source recording.
- UI: Group/Ungroup, group effects targets and transfers, collapsed rows in both views, subtree selection, drag destination and cycle checks, existing modal/focus guards; invalid clip bus targets for moves/copies, collapsed groups, atomic multi-clip rejection, and recovery to valid child tracks.

See [validation history](../validation/README.md) for executed checks and platform limits.

## Shared Tracks browser

Instrument and Editor support project-track reorder and group membership through the [shared browser contract](../views/instrument-tracks.md#reordering-and-group-membership). Move complete group subtrees through the core transaction. Beat subtracks are virtual browser children, not project groups; their drags remain inside the original beat parent.

Dragging a selected project track moves all selected tracks in project order. Selected groups include their complete subtrees; selected descendants move once. Drop in the middle of an existing group row to add the selection to that group, including collapsed groups. Edge drops reorder the selection. The move commits as one undoable transaction on release.

Move previews use `Project::track_move_layout`: one preorder membership pass and
lightweight index/parent ordering, without copying or validating clips, assets or
processing graphs. Both drag surfaces use this topology for eligibility and no-op
detection. `moved_tracks` constructs and validates the full project only for the
release transaction. A stationary plain Timeline click replaces the selection;
arming a drag retains the group until the gesture is resolved.

## Timeline shortcut focus

Timeline Rename, Group and Ungroup use text-edit focus rather than general
keyboard ownership. Other pane and non-text control focus does not block them.
Retain selection requirements, window focus, active drag/pointer, text entry,
dialog and blocking-job guards, repeat suppression and shared native-menu
dispatch. Mixer follows its focus rules below; other views retain their existing rules.
This Timeline change leaves Mute and Solo handling unchanged.

## Mixer shortcut focus

Mixer Rename, Group, Ungroup, Mute and Solo use the text-edit focus guard rather
than general keyboard ownership. Other pane and non-text control focus does not
block their shortcuts or shared native-menu actions. Preserve active drag,
pointer, window-focus, selection, text-entry, dialog and job guards and repeat
suppression. Timeline and Instrument Mute/Solo dispatch remains unchanged.


## Track and layer selection presentation — 2026-11-08

Track, Group, Return, Master, beat-subtrack and instrument-layer selection uses
background fills without accent selection outlines. Remove track/container
selection strokes in Timeline, pinned banks, Mixer and Inspector strips, and
layer/Output/provider header strokes in Instrument and Beat Editor. Retain the
shared selected header and lane theme colors, selection membership, keyboard
focus, gesture handling and all neutral structural dividers. Mixer strip bodies
use selected_track_header_background while their colored header bands keep the
track palette. Track overview headers reuse the same selected track background.

Effect/component unit selection and drag-target feedback retain their own
existing outlines. Keep shared selection_border for those callers; do not mask
or change it globally. Tests must verify background highlights and absence of
track/layer selection strokes across themes, scrolling and multiple selection,
while retaining structural borders and unit interaction coverage.

Compact Timeline and Editor track headers reserve only the Mute/Solo group beside the name and meters. Hide Pan and Gain, including Timeline Master gain, without changing their saved values. Expanded headers retain their mix knobs. Beat subtracks in Timeline follow the same mode. Instrument and lower Beat Editor mix controls retain their existing layout.

## Mixer hierarchy gutters

Keep each mixer header 24 points tall. Indent nested headers downward by 12 points
per group level (half the header height), capped at three displayed levels.
Paint each ancestor's 12-point horizontal band across all descendant strips with
that ancestor's resolved clip-header background color. Continue bands through
nested groups and retain them when the ancestor is horizontally offscreen.
Start strip dividers at the strip's own header, leaving ancestor bands connected.
Nested headers have a 1-point top border matching the side divider's color and
inset; root headers retain the shared workspace top border.
Collapse hides descendant strips and their bands; root tracks, Returns and
Master have no ancestor bands. Keep centered icon/name geometry and existing
header hit behavior. Use fixed white text and icons for every header, independent
of track color, theme, hover and selection. Name editing retains standard input
styling. Preserve colored header fills and neutral Master fill.

Keep output faders, meters and send controls aligned at the bottom. Include the
maximum displayed gutter height in the minimum scrollable content height so
nested headers and destinations cannot overlap the lower controls. Apply the
shared strip presentation to Mixer and Timeline's Track mixer. No project,
selection, routing or saved mix values change during painting.

## Shared Mute/Solo presentation

Track, layer, beat-subtrack, Tracks-browser and mixer M/S buttons use the shared
`mix_controls::mute_solo_with_size` implementation. Keep their existing button sizes,
independent toggles, accessibility labels, tooltips and disabled guards. Draw no
outer or per-button strokes in idle, hover, pressed or selected states. Use the
unit content background (`unit_separator`, not the strip-card surface) as the
group and idle-button background; use the neutral unit-header fill for hovered
buttons. Draw a full-height 1-point separator between M and S in that unit-header
color. Preserve existing selected/pressed background and foreground colors,
and hover foreground feedback. All other framed controls follow the shared
[default control surface](../views/dropdowns.md#default-framed-control-surfaces).

Use a 5-point outer group radius. Buttons fill the group height without an
inset frame. Each separator reserves its own 1-point layout gap. Standard
22-by-22-point buttons form a 45-by-22-point group; browser 16-by-16-point
buttons form a 33-by-16-point group. Round only their outer corners to 5 points; keep their
shared inner edge square. M/S labels use the shared semibold font at the existing
Small text size for standard buttons and 9 points for browser buttons. Explicitly clear each button stroke so hover, focus and
selected style rules cannot restore an outline. Keep the divider outside both clickable button rectangles; reserve the added
point in parent layouts.

### Copy source identity

Regular-track and group duplication retain track-scoped provider references while
refreshing globally unique Sampler and Noise identities. Noise owned routes get
fresh route identities; retain settings, disabled/collapsed state and sidechain
assignments. Beat subtracks retain the shared deep instrument-copy path.
