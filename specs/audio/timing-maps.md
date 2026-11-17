# Arrangement markers and timing maps

## Model and transactions

`Project::markers` stores named section ranges with stable UUIDs, integer
`start_frame` positions and positive `length_frames`. The optional length field
accepts early marker manifests; a missing length resolves to one bar on the
project clock. New sections always save an explicit length. Range ends are
exclusive and checked for overflow. Names contain visible text, no control characters, and
at most 256 characters. Duplicate names and coincident markers are permitted.
`Project::timing` is version 1 and stores ordered, right-continuous tempo and signature events.
Signature spans are derived from their starts: each ends at the next change,
and the final value extends to the end. They have no independent saved duration.
The combined metadata limit is 4,096 points. Tempo is positive finite
quarter-note BPM. Signature values are positive `u16` values.

An empty event list represents the legacy constant value. The public event
accessors expose a deterministic project-derived origin UUID. Explicit lists
must begin at frame zero. Their origin identity is preserved by edits; the
origin value can change, but its position cannot change and it cannot be deleted.
The scalar tempo/signature fields retain the origin values and the reference
clock for immutable MIDI/beat source frame ranges. New fields use serde defaults;
constant legacy projects retain their existing frame positions and source data.
The current manifest has no top-level version rejection gate. Opening a mapped
project in an older application is unsupported: old readers can ignore map fields.

`SetMarkers`, `SetTempoEvents`, and `SetSignatureEvents` use ordinary atomic core
transactions, session history and persistence. `SetTempo` and `SetTimeSignature`
remain origin-value wrappers. Invalid metadata, overlaps, collapsed event frames
or timing overflow reject the complete candidate. A signature event starts a new
bar; a partial preceding bar counts as one bar. Signature-only edits retain all
clip, note, marker, loop and playhead frames.

Tempo edits capture musical anchors on the old clock, solve the new event
positions, then reproject both section endpoints, later map events, clip starts, MIDI/beat ends
and both saved loop endpoints. Audio durations, source offsets, repeats and
stretch stay absolute. MIDI/beat source offsets and repeat ranges use the origin
tempo reference clock; origin tempo edits rescale these fields. Source note ticks
do not change. The playhead stays at its frame. Sample-rate edits rescale metadata
with checked arithmetic and validate the resulting ordering.

## Prepared clock and audio

`MusicalClock` compiles piecewise frame/quarter prefixes and meter/bar prefixes
on control or worker threads. Queries do not allocate. Frame positions are
nearest-sample conversions; musical snap boundaries round up. The engine holds
compiled clocks outside callback ownership retirement. MIDI source events and
repeat windows retain their reference coordinates; each occurrence maps through
the song clock. Note start and end map separately, so a held note spans tempo
events without a retrigger. Source-range slicing, overwrite, trim, split and
merge share the reference mapping.

The shared portable modulation contexts accept numeric quarter positions and
note/legato origins. LFO, Auto-filter and Chorus use integrated quarter positions
in Note mode and retain their Hz clocks and transition smoothing. Effects on
tracks, layers, Main, groups, Returns and Master receive the prepared clock.
Sampler prepares finite duration/pitch variants for the map's tempos on workers.
Note-on selects an index and retains that duration through later tempo changes.
Bars keeps four quarter notes per bar; Off and Time keep their units.

Metronome beats and accents follow the compiled meter and tempo segments.
Count-in holds the song position and uses the recording start's active meter and
tempo. Capture converts timestamped frame intervals to 960-PPQ ticks off callback.
Maps cannot change during recording. Desktop playback, Bounce and CLI export
share the same renderer and prepared model. Map changes that require source
preparation retain prepare-before-publication semantics; conflicting changes
cannot publish over a newer map. Marker-only changes enter history/save without
rebuilding audio.

## Timeline

Allocate three pinned rows with the shared Beat Editor subtrack lane height
(32 points) after the existing ruler and before the first track: Markers, Tempo,
Signature. Keep the existing control-column width and
frame-linear horizontal geometry. Paint a full-width shared border between the
ruler and the first visible metadata row, including its header. Reuse the pinned-row
divider at the preceding header's bottom pixel so it aligns with the Effects
catalog header separator. Content backgrounds and grids use the ordinary
Timeline painters. Marker and signature blocks use `arrangement_block_fill`: the shared
`unit_separator` surface (`#2B2B2B`) in Dark and the existing unit-header surface
in Light/Mist to retain contrast,
with names/values and no flag decorations. Marker and signature fills start at
the lane top without a background gap; retain the bottom divider inset. Sections
use their saved ranges.
Signature blocks span continuously to the next change; the final block fills
the visible remainder. BPM labels use `TextStyle::Small`. Tempo uses nodes, BPM
labels and a step line. Mapped grid enumeration includes the candidate below a
segment's starting musical position, then checks ownership using rounded frame
coordinates (`segment_start <= frame < next_event`). A bar whose ceil-rounded
frame equals a tempo transition belongs to the following segment and must appear
once with its ruler label. Do not skip it by rounding the musical index upward.
Editor reuses map-aware rulers and note coordinates,
without metadata lanes. Sampler's local source view retains its source clock.

Double-click empty lane space or use its menu to add. Each expanded lane header
has a right-aligned shared compact Lucide Plus panel button (18-point square,
12-point artwork), vertically centered with an 8-point right inset. Exclude its hit area from the collapse control. The button
and header context Add share `add_arrangement_at_playhead`, use the exact stopped
playhead frame and retain the existing edit/job/playback guards. Marker creation
uses New section and one bar. Tempo/signature additions immediately inherit the
active value without a dialog; an existing event at that frame is selected.
Select the added item and retain metadata keyboard focus. Each successful addition is one Undo step. Marker creation commits immediately with `New section`, a one-bar
length in the active signature and mapped tempo, and no dialog. Both block types
use the shared semibold
unit-header font and `panels::unit_header_text`: secondary text when unselected,
normal text when selected, unchanged fill and no selected outline. Reuse
`unit_selection::select` for plain/Shift clicks and its select-all helper.
Empty lane clicks clear selection. Selection stays in UI state without seeking
or editing the project. Context-menu Seek here and previous/next commands seek.
Keyboard/context Delete removes the selected metadata through one guarded batch,
retaining the tempo/signature origin. Input fields keep their keys.

Both block types use an 8-point resize area. Capture a touching boundary only
within 3 points of the exact edge at the initial press; keep that gesture for
the full drag. Shared-boundary edits keep outer endpoints and remove an empty
side. The signature origin retains at least one frame at zero. Other selected
blocks use the requested ordinary trim delta independently of the pair clamp.
Ordinary resizing retains the opposite endpoint and rejects an empty selected
block. Right-edge edits give earlier blocks priority; left-edge edits give later
blocks priority. Trim covered neighbors, remove fully covered blocks, and retain
both sides with a fresh right-piece identity when required. Core conductor edits
and clips share `frame_ranges::uncovered_ranges`; source mapping remains owned
by the clip adapters. Core section/signature helpers calculate the complete
transaction for preview and release. Signature right-edge shrinking pulls the
following transition back to preserve continuity. The final signature has no
right resize handle; its body/left edge moves the start of the final span. UI snapshots include neighbor changes and
retain pointer ownership even if the dragged block disappears. Body drags move
the selected block group. No section gesture moves clips or source data.

Double-clicking a section, or its Edit action, replaces the label with the shared
inline name input. Select the full current name and focus the input. Typing stays
in UI state; Enter commits one validated `SetMarkers` transaction. Escape, focus
loss, hidden lanes, leaving Timeline or an unavailable editing environment cancels.
Signature value editing and the tempo point's context Edit action retain the
shared dialog. Empty Tempo-lane double-click and Add actions immediately insert
an event at the snapped frame with the currently active BPM, then select/focus
it without opening a dialog. At an occupied frame, select the existing event.
Tempo hit regions cover only the point (12-point square clipped to the lane),
leaving labels and line space available for creation. Selected points use the
shared selected text color. Vertical motion reuses the transport BPM adjustment:
whole-BPM values, 1 BPM per point and a 1 BPM drag minimum. Both controls retain
subpixel movement in shared drag state, so slow motion still advances in 1 BPM
steps. Shift reduces sensitivity tenfold while retaining 1 BPM output steps; it
also retains timeline snap bypass for horizontal point movement.
No vertical motion preserves an existing numeric value exactly. Freeze
the display range during a gesture, preview BPM/labels in UI state and commit
one SetTempoEvents on release. Horizontal movement retains timeline snapping. Tempo points choose the dominant
axis on the first recognized drag sample and ignore perpendicular movement up
to 12 logical points from the latest axis-lock position. Crossing that distance
switches the active axis and applies only the excess displacement, without
replaying ignored movement. Each later switch rebases the physical anchor. Feed
the filtered position into both timeline snapping and BPM adjustment. Process
the initial drag sample immediately, so a fast second sample cannot change the
first chosen axis before it takes effect. Stationary frames never switch axes.
The origin is vertical-only and can change BPM but cannot move or be deleted. Vertical release may
leave the row; horizontal release outside its bounds cancels. Escape, lost window
focus, hidden/collapsed lanes, conflicting jobs, playback or a changed revision
cancel the preview. Delete uses the selected event ID and shared metadata guard.

Block drag previews against
the committed clock and commits once on release. Shift bypasses snapping.
Escape, focus loss, outside release, a changed revision or a conflicting job
cancels the drag. Origin event positions remain fixed. Focused metadata Delete removes
only the metadata event. Coincident markers are available in the context menu.
Previous/next navigation seeks strictly before/after the playhead without wrap.
View > Arrangement lanes controls visibility/collapse. Short viewports show a
collapsed row. Stopped transport tempo/signature controls edit the event active
at the playhead; playback makes these controls read-only.

## Acceptance

Automated checks must cover constant-map compatibility, conversion boundaries,
partial bars, invalid transaction rollback, source ranges across tempo changes,
held notes, seeks/loops, callback allocation freedom, phase integration,
Sampler onset durations, capture alignment, save/reopen, history, sample rates,
UI lane placement, focus and cancellation. Native gesture, listening, MIDI device,
and Windows/Linux checks remain separate from automated acceptance. See the
[validation index](../validation/README.md) for executed evidence.
