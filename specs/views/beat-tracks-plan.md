# Beat tracks and editor implementation plan

Status: planning only; no beat-track implementation is available yet.

Date: 2026-10-31.

This plan extends the [Beat MVP](beat-editor.md), [source-container architecture](../architecture/modular-audio-units.md#one-source-container-per-track), and [shared modulation contract](../modulation/contract.md). The current four views remain Timeline, Instrument, Editor, and Mixer. The placement decisions below supersede the older Units-view plan and the historical MIDI-only restriction for Instrument.

## Confirmed behavior

- Add **Add beat track** after Add audio track and Add MIDI track in the menubar/application Track menu, the Tracks panel Plus dropdown, and the Tracks panel context menus that offer track creation.
- A beat track owns beat subtracks. A subtrack is a child of its beat instrument, not a project track, track group, or ordinary MIDI instrument layer. Earlier specifications call these children beat lanes.
- Editor keeps the selected beat track's clip overview above its lower beat editor. The lower editor shows one step row per subtrack and an action to add a subtrack.
- Editor and Instrument both support adding, selecting one or several subtracks, deleting, renaming, and reordering them. Both expose the same subtrack gain and pan values. Retain the previously accepted Mute/Solo controls.
- In Instrument, each subtrack is an expandable group whose named row acts as that subtrack's output layer. It contains one or more source layers plus its own LFO and Envelope sublanes. Kick and Snare have separate outputs, settings and modulators.
- One fixed Output lane follows all subtrack groups and sums the whole beat instrument. Do not add another Output child beneath each group: its named group row already serves that role.
- Mixer controls the parent beat track. Beat subtracks do not become extra Mixer strips or ordinary children in the project track browser.
- Every beat subtrack is an instrument, with source units chosen inside its layers. There are no audio/MIDI subtrack types or type chooser. Sampler, Oscillator and other supported sources use the existing instrument model.
- Preserve step/velocity controls, one fixed trigger pitch per subtrack, one active voice per subtrack, the shared pattern grid, lane-stop links, immutable pattern sources and serial routing.

The attached menu images establish where the new track action belongs. The first attachment shows step-sequencer rows; the fourth shows an Add track dropdown. Exact lower-editor row styling remains to be confirmed because the request names the fourth image as its row reference. This does not block the model, commands, group structure, or shared-control plan.

## View composition

| View | Parent beat track | Subtracks |
| --- | --- | --- |
| Timeline | Normal track header, beat clips, existing track effects lane and arrangement gestures. | Pattern content appears inside beat clips; no extra project-track rows. |
| Editor | Existing selected-track overview and clip selection. | One lower-editor row per subtrack, with shared headers/mix controls and step toggles across time. |
| Instrument | Beat track is selectable through the shared track browser and compatible instrument selector. Parent effects retain their existing target. | One expandable group per subtrack. Its named row is its output layer; its children are source layers, LFO and Envelope. One fixed overall Output follows all groups. |
| Mixer | Existing gain, pan, Mute/Solo, meter and track effects behavior. | No separate Mixer strips. |

Instrument group structure:

```text
Beat 1
  ▾ Kick       [M] [S] [Pan] [Gain]   [Kick output controls/units]
      Layer 1
      LFO
      Envelope
  ▾ Snare      [M] [S] [Pan] [Gain]   [Snare output controls/units]
      Layer 1
      Layer 2
      LFO
      Envelope
  [+ Add beat subtrack]
  Output                            [whole beat-instrument output]
```

Each LFO sublane accepts only LFOs. Each Envelope sublane accepts only Envelopes. They are fixed children of their subtrack and cannot be renamed, deleted, moved to a sibling group, or selected as whole subtracks. Their units retain the normal add, select, delete, power, reorder, modulation and compact/expanded behavior. Collapsing a group hides its child rows while retaining its named output row. Group collapse is presentation state; it does not mute audio, disable modulation, change selection identity, or mark a pattern edit.

Each group output processes the sum of its own source layers. Reuse the normal instrument Output body and controls on the named Kick/Snare row; do not create duplicate output settings or another output child. The final Output processes the sum of all subtrack outputs. It stays outside group scrolling in the shared pinned-Output position, cannot be renamed, deleted or reordered as a subtrack, and uses its own selection target.

This does not add a parent-wide LFO/Envelope pair shared by all subtracks. Cross-subtrack modulation is outside the initial scope. Lane-stop connections remain a distinct accepted trigger feature, not a modulation shortcut.

## Shared subtrack interactions

Use one subtrack command/selection model for both views. Store a primary subtrack ID and a set of selected IDs per displayed beat track; keep focused child lane, installed-unit selection, step selection and clip selection separate.

- Plain header click selects one subtrack. Shift-click toggles membership. Clicking a member of a selected group can make it primary without discarding the group before a drag.
- Reuse the current focus, text-entry, dialog/job and active-gesture guards. Delete/Backspace removes selected subtracks only with subtrack-header/list focus. It must not delete a group while editing its name or operating on steps, knobs or selected units.
- Header rename targets the primary subtrack, through the existing inline name editor. Enter accepts; Escape cancels. Labels, truncation and accessibility reuse track/layer names.
- Drag the group header/row handle to reorder the selected subtracks together in their current order. Move the whole Instrument group, including all its child lanes. Commit on a valid release only; Escape, focus loss, outside drops and invalidated state cancel. Same-position drops do not create history.
- Reordering retains subtrack, provider, unit and event identities, assigned sources, steps, routes, settings and compatible runtime state. It changes no step timing. Never address a source or step row by its display index.
- Gain and pan in both views bind to one canonical subtrack bus control. Reuse the track/layer knob definitions, units, bounds, reset, sensitivity, smoothing and keyboard handling. These are the named group's output-layer controls. Do not apply another equivalent gain/pan stage beneath that group.
- Deleting a group is one atomic edit. Remove its processing configuration and affected owned routes/stop links, preserve survivors, and choose a deterministic neighboring primary selection. Deleting the final subtrack leaves an empty beat instrument with an enabled Add action.
- View switching preserves the selected subtracks and active clip. A deleted subtrack is pruned from selection immediately; a worker cannot restore it. If no clip is active, show the subtrack controls with an unavailable step grid and a create/select-pattern action. Do not silently edit a different clip.

Add beat subtrack appears in both views and creates a normal empty instrument group with its initial source layer, local LFO/Envelope lanes and named output. It does not ask for an audio/MIDI type. Choose source units through the shared component catalog or source-replacement action. Freeze naming and insertion defaults in Stage 0. Keep the ordinary project-track Add menu separate from this command.

## Ownership and reusable code

| Owner | Planned responsibility |
| --- | --- |
| `unit-core` and portable units | Reuse source/instrument settings, per-note/continuous source contracts, typed destinations, ramps and meters. Add portable beat-container settings only where they have a DAW-independent responsibility. |
| `core` | Beat track kind/container ownership, stable subtrack identities, pattern/step data, validation, transactional edits, duplication and source isolation. |
| `project` / `media` | Save/open migration, source inventory and collection, history, required sample/pitch preparation, caches and complete-session publication. Beat pattern assets must never be decoded as external audio. |
| `engine` | Prepared step/note schedules, independent subtrack runtimes, scoped provider bindings, trigger/stop handling, latency alignment and parent summation. |
| `output` | Existing bounded prepared-renderer activation, monitoring and retirement. |
| `ui` | Shared subtrack headers, selection/commands, drag previews, Editor step painting, grouped Instrument composition and menu integration. |
| `desktop` / `cli` | Existing native menu adapter and headless inspection/validation/rendering over shared commands and project APIs. |

Reuse these implementations rather than creating parallel versions:

- `TrackAction`, `track_selection.rs` and the native adapter's `TrackAction::ALL` mapping for Add beat track. Consolidate track-creation choices so menubar, Plus and context menus cannot drift.
- `track_browser.rs`, `track_overview.rs`, track header/name/mix controls, `selection_focus.rs`, and existing layer/group selection and release-only drag helpers.
- `panels.rs`, `toolbars.rs`, `lane_toolbar.rs`, shared workspace borders, canonical dropdowns/catalog spacing, shared knobs and unit bodies.
- Existing instrument source/layer panels, per-container LFO/Envelope lanes and Output bodies, catalog registration metadata, component/effect transfer helpers and shared modulation discovery. Extend registration targets centrally; do not put type allowlists in drag handlers.
- Shared sampler/audio preparation, MIDI tick/frame conversion and scheduling primitives, effect-chain processing, bus mixing, latency alignment, meters and renderer history. Do not create a second beat-specific audio pipeline.

Every subtrack is an instrument with source layers. Those layers remain inside its group and do not become beat subtracks or extra step rows. Its LFO and Envelope sublanes belong to that subtrack's instrument scope and can address compatible destinations across its source layers and named output. They do not become a kit-wide pair. A Sampler-based subtrack uses exactly this instrument model and shared sample preparation; it is not an audio-track variant.

## Content and processing contracts

The parent beat track owns one beat instrument. That container owns the ordered subtrack configurations and one overall Output bus. Each subtrack owns its named output layer, source layers, local LFO/Envelope providers and serial effects. Its name and shared mix controls belong to that named output row. Beat pattern assets own immutable grid/step content, addressed by stable row/subtrack identities. Clips own placement and source mappings.

Pattern/step or grid edits create a new asset for the edited clip. Other clips retain their original pattern. Subtrack rename, mix, processing and visual order edits change track-owned configuration and therefore apply to that track's clips; they do not clone or rewrite every pattern. Group expansion and selection are UI/presentation state, not step content.

Provider discovery and route validation must include the containing subtrack scope. A Kick LFO must not appear as the Snare LFO merely because both are named LFO 1. Per-note envelopes follow the subtrack's own triggers; shared envelopes/LFOs advance once for that subtrack's context. Move/rename/reorder must not reconnect routes by index. Duplication creates fresh owned identities and remaps local routes and stop links atomically.

Each subtrack initially has one active voice. Each enabled step sends note-on and note-off one step later. Retrigger replaces the active voice. Source trigger modes, envelopes and Amplitude determine audible duration, including Sampler One-shot/Gate; do not add duration rules based on a subtrack type. Reuse the shared 5 ms ramps/fades and unit lifecycle at every supported rate. Within each subtrack, reuse the existing source-layer mixing and alignment, then process that group's output effects and canonical gain/pan. Align group output latency before summing into the overall Output, then use the parent track's existing effects and gain/pan. Group output, overall Output and track controls each apply at their own level exactly once. Parent/group/Master routing, mute/solo precedence and live/export/seek behavior remain shared.

Required source/pitch preparation runs on workers and uses saved caches. No source decoding, bank construction, allocation, locks or file access belongs on a step trigger or the callback. Reuse editor/prepared revisions for pattern edits where safe; source/configuration transactions retain their existing atomic failure behavior. Optional warming must yield to required work.

## Stages and exit gates

| Stage | Deliverable | Exit gate |
| --- | --- | --- |
| 0 | Freeze data, UI and editing contracts | Remaining decisions below have explicit rules and generated fixtures; no new public menu item yet. |
| 1 | Shared model, edits and persistence | Legacy projects round-trip unchanged; beat models, identities, isolation, transactions and history validate. |
| 2 | Preparation and rendering | Generated beat fixtures play/export/seek correctly through independent subtrack processing with no callback allocation. |
| 3 | Creation and navigation | All three requested Add surfaces dispatch the same command and select the same valid beat track; browsers, groups, Mixer and CLI recognize it. |
| 4 | Lower Editor | Step grid and all subtrack management/mix controls work over the shared model and survive cancellation/history. |
| 5 | Grouped Instrument | Every subtrack has independently scoped source, LFO and Envelope content; both views stay synchronized. |
| 6 | Integrated acceptance and guides | Complete workflows, native checks and current documentation match the delivered feature. |

Stages 3–5 may be developed separately, but do not advertise beat tracks as available or expose an incomplete release workflow before creation, Editor and Instrument work together.

### Stage 0 — Contracts

Specify the stored content discriminator and version/default rules before extending the current `TrackKind`, track instrument fields and `Asset::midi_pattern` model. Audit callers that currently classify every non-MIDI asset as an audio file. Define required/forbidden payload combinations and clear errors for unsupported versions.

Freeze subtrack add/delete/reorder transactions, empty-track defaults, selection fallback, child-lane/group presentation, and exact row styling. Retain a single source container per track and a single canonical subtrack bus gain/pan. Reuse the subtrack instrument's existing Output settings on its named group row, and define the overall beat Output's separate settings/identity. Do not retain an extra per-group Output child. Reuse normal instrument source controls and nested source-layer ownership within that hierarchy; do not add a separate audio-subtrack schema.

Freeze pattern-to-subtrack bindings before coding. Shared sources, missing/deleted subtrack rows, parent-track duplication, and moving/pasting beat clips to another beat track need explicit identity mappings. Do not match rows by name or position, mutate shared pattern assets in place, or silently lose steps on transfer. Decide whether deleted bindings remain dormant immutable content or require new per-clip assets; record the tradeoff.

Specify grid resize/resolution behavior, velocity bounds/default, trigger-pitch default, and simultaneous/conflicting/cyclic lane-stop behavior. Prefer existing MIDI value conventions where applicable, but keep these choices separate from confirmed user requirements. Confirm the supplied row reference. Define deterministic timing at tempo changes, trims, stretches, repeats, loops and nonzero seeks using shared pattern mapping.

### Stage 1 — Core model and storage

Add beat track/source container and immutable pattern models with stable identities. Implement creation, subtrack structural/mix edits and pattern edits through `Project::edit`/validated batches. Validate instrument/source-layer ownership, provider scope, source references, bounds, stop links and container ownership centrally. Include memory estimates and history retention. Support source collection/Save As for sample-only inputs without mistaking beat pattern assets for files. Add legacy round-trip, invalid-data and rollback tests before UI integration.

### Stage 2 — Audio and preparation

Prepare common-grid events in sample time through shared tick/frame helpers, including velocity and fixed subtrack trigger pitch. Construct and retire per-subtrack source/provider/effect state outside the callback. Reuse sampler banks and the instrument/effect runtime; align source layers before each named group output, then align groups before the final beat Output and parent track processing. Cover independent clocks, retrigger replacement, envelope release, lane stops, mute/solo, tails, clipping, meters, loops and exact history reconstruction. Cache keys must include relevant content/configuration without mixing sibling settings.

### Stage 3 — Track creation and navigation

Add one shared Add beat track action, labels and guards to the menubar/application menu, Tracks Plus dropdown and creation context menus. Keep the current audio/MIDI entries and menu grouping. Update native-menu indexing, track icon/formatting, project track browser, group membership, duplicate/delete, compatible instrument selection, selected-track overview, Mixer and CLI output. A beat parent must open the beat editor; its subtracks' source choices must not switch it to the ordinary audio editor or piano roll.

### Stage 4 — Editor

Compose the lower step grid beside shared subtrack headers with aligned vertical scrolling and existing ruler/playhead/grid primitives. Keep pattern length/resolution shared across rows. Add steps and velocity editing, one trigger-pitch control per subtrack, the Add action, selection, rename/delete and group reorder. Reuse common row/mix controls rather than a second state model. Test no active clip, multiple selected clips, empty instruments, row focus versus grid/unit focus, edge scrolling, text entry, jobs and undo/redo. Edit only the explicit active clip's immutable pattern.

### Stage 5 — Instrument

Render each subtrack's named output row as its expandable group header, with source layers and local LFO/Envelope sublanes beneath it. Render one shared final Output outside the groups, using the existing pinned-lane component. Reuse the current lane toolbar, compact controls, borders, source/effect bodies, meters and catalog metadata. Group headers expose the same mix values and selected IDs as Editor. Restrict provider additions to the matching child lane, filter source menus by subtrack scope, and reject invalid drops before constructing instances. Reuse saved source preparation, local route duplication/remapping and unit-selection behavior. Never substitute a single kit-wide modulator lane for the independent child lanes.

### Stage 6 — Acceptance and documentation

Add generated fixtures with Sampler-based and Oscillator-based subtrack instruments, mixed source layers inside an instrument, and groups whose LFO/Envelope names match but UUIDs differ. Verify source replacement preserves subtrack identity and no audio/MIDI type selector is exposed. Verify isolation, fan-out inside each group, group selection/reorder/delete, collapsed groups continuing to process, separate Kick/Snare output processing, exactly one fixed final Output, aliasing of mix controls between views, provider copying, missing sources and worker failure/cancellation. Test pattern sharing, structural edits, duplicate/transfer policies, history, save/reopen/Save As/collection, CLI validation/rendering, all rates and supported buffers, live/export/seek parity and callback allocations.

Compare visual geometry and interaction in Dark/Light/Mist and supported display scales, then check native menus, pointer/keyboard behavior, playback and devices on macOS/Windows/Linux. Record unexecuted native checks as open gates. Update user guides, README/roadmap and validation records only as stages become usable. Planning documents must continue to label the feature unimplemented until then.
