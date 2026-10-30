# Moving and copying units

The shared Timeline toolbar places Clip settings, Effects, and Chords from top to bottom in Timeline. In Instrument, it places Instrument components above Effects and uses Lucide KeyboardMusic for the Instrument components button. Keep the effects-lane toggle at the bottom and preserve button identities, icons, selection, and catalog behavior.

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/editing/units.md).

Instrument components can move between regular layers, Main, and the track effects lane of the same instrument. Drag a unit header onto the destination layer's label or component area. The effects lane’s insertion line marks the destination position. Hold Option on macOS, or Ctrl on Windows/Linux, to copy. The modifier can change during the drag; its state at release determines the action. The catalog does not need to be open.

Drag unused space inside an individual oscillator strip to transfer that oscillator. Waveform, pitch, gain, source power, unit power, mode selectors, and modulation controls keep their existing gestures. Double-clicking a unit header still collapses or expands it.

## Destination rules

| Component | Drop behavior |
| --- | --- |
| Oscillator header | Append all sources and their pitch routes to the destination Oscillator unit, in source order. |
| Individual oscillator | Insert that source and only its pitch routes at the source-strip insertion line; a label drop appends it. Other sources and routes stay in place. |
| Envelope or LFO | Reorder or copy the independent component within its matching instrument-wide lane. Reject moves/copies to regular layers, Output or the other provider lane. Copies start with no consumers. |
| Amplitude | Transfer the stage and its connections to a regular layer without an installed Amplitude stage. An occupied stage rejects the drop. |
| Macro header | Merge all strips and their targeted routes into the destination Macro component; moves update all consumers and copies receive fresh strip/route IDs. |
| Output | Accept Macro components and inline effects. Reject Envelope, LFO, Oscillator and Amplitude additions. |

A copy can stay in the source layer where the destination rules permit it. Individual oscillators can also move to an insertion position in their current layer. Unit headers can also reorder components in their current layer; Oscillator stays first. There is one Oscillator unit and one optional Amplitude stage per regular layer; this operation does not add a second unit of either type. An Oscillator merge uses the destination unit's enable, collapse, and modulation-open state. An empty destination takes those settings from the source unit. Individual oscillator settings always travel with the source. Layer gain, pan, mute, solo, name, order, and compact state stay with the layer.

Sources moved out of a layer are removed from its component list. They are not retained as hidden copies. If a legacy source ID already exists in the destination, the moved component receives a new ID and its connections are remapped. Other destination components and their connections remain intact. Removed destination Amplitude settings are replaced only when that slot accepts an explicit transfer.

Copies receive new component and connection IDs, including per-oscillator pitch targets. A copied destination keeps its existing source assignments. Existing consumers of a copied Envelope/LFO keep using the original provider. Base parameters, signed amounts, power, unit collapse, and modulation-open state are preserved. No modulation source or audio processing algorithm is duplicated in the UI.

LFO components carry their incoming Depth/Rate connections and modulation-panel open state when moved or copied. Copies receive fresh incoming route IDs while retaining source assignments. Moving any provider also updates LFO consumers, including those on Main. Self/feedback cycles remain invalid under the [shared dependency rules](../modulation/contract.md#providers-that-also-consume-modulation).

## Selecting and reordering component units

Single-click a component to select it. It uses the same outline, click detection, and pane-focus helpers as an installed effect. A plain click replaces the selection. Shift-click toggles membership within the current lane. Shift-click in another lane starts a selection in that lane. Clicking empty component space or moving focus outside its lane clears it. Selection is UI state; it does not mark the project modified or save with the project. Layer and catalog selection remain independent. Controls keep their own gestures and focus. The outline covers the complete unit, including its visible modulation controls, and clips to its lane while scrolling.

Drag a selected unit header to move the selected group in its existing display order, independent of click order. Dragging an unselected unit moves only that unit and replaces the selection when the drag starts. Hold Option on macOS or Ctrl on Windows/Linux to duplicate the group; the modifier at release determines the action. Reorder within a regular layer or Main, or transfer the group to another compatible lane. A collapsed unit uses its vertical title strip. Audio sources stay first; control components and inline effects can appear in any order after the source. Dragging another unit to the left of Oscillator places it immediately after Oscillator. Main has no audio source unit and permits effects before or between its control components. The same insertion-line renderer, pointer threshold, focus restoration, edge scrolling, and release-only commit rules serve effects and components. Selected units keep their outlines during the drag; there is no separate row outline for installed-component drops. Same-position drops show no insertion line and make no edit.

Transfers between lanes, including empty Main, use this same insertion preview. Drop in a component area to choose its position, or on a layer label to append it. An incoming Oscillator unit remains first and merges its sources into the destination source unit. A Macro transfer merges its strips into the destination Macro unit at the indicated unit position. Existing type, mode, identity, and connection rules still apply. After release, copies select the resulting units and retain their originals; moves select the group on its new owner. If the group includes Oscillator, same-lane moves keep it first while moving the other selected units. Source transfers/copies merge oscillators into the destination source unit and keep that unit first. Invalid destinations show the standard forbidden cursor and make no edit. Every selected unit must be compatible: one unsupported unit rejects the complete group. Output rejects a group containing Oscillator, Amplitude, Envelope or LFO. An occupied Amplitude slot also rejects the complete group, including same-lane copies that contain Amplitude. A Macro group transfer retains the existing singleton-module merge rules.

Each regular layer, Output and fixed modulation lane saves a `component_order` list in instrument revision 4. Envelope/LFO entries use stable unit IDs; Oscillator, Macro, and Amplitude use their owner’s singleton unit identity. Missing lists default to the previous canonical order. Partial lists append missing units in canonical order, with sources first. Retained removed units keep their position when restored; physically transferred providers lose their old owner's entry. New providers append after the saved entries. Validation rejects duplicate, unknown, stale, individual-oscillator-strip, and misplaced-source entries. Invalid insertion indices fail as one project transaction.

This order changes presentation only. Provider vectors, source-strip order, parameter values, routes, audio topology, phases, and held notes retain their existing behavior. Save/reopen, Save As, track duplication, rate changes, layer reorder, and view switches retain the layout. Component identities and order policy live in `unit-core`; core transactions and the UI use those shared definitions.

## Select All in installed-unit lanes

Command+A on macOS or Ctrl+A on Windows/Linux selects every installed unit in the focused component or effects lane. Include horizontally scrolled-out, disabled, and collapsed units. Limit component selection to one regular layer or Main; limit effect selection to the active track/Master chain. Use the full installed inventory, not the clipped visible rectangles. Empty lane clicks retain pane keyboard focus, so the command also works without a current selection. Create the keyboard target through `unit_selection::keyboard_focus`, covering the lane with `Sense::focusable_noninteractive`. Keep child control click/drag gestures intact. Do not use a zero-area target: egui can surrender its focus when ownership checks read the response after a blank-space click. Resolve the clicked owner before handling shortcuts, including a click release and Select All in the same frame. An empty lane remains a clean no-op.

Effects and components share `unit_selection::shortcuts` for Delete/Backspace and Select All key handling, and `unit_selection::select_all` for selection replacement. Reuse pane ownership, text editing, pointer/drag, enabled UI, window focus, job/dialog, and name-edit guards. Require the exact platform command modifier and ignore held-key repeats and extra modifiers. Consume blocked/repeated unit shortcuts while the pane owns them; leave text editors' keys intact. Preserve control focus, track/clip selections, unit order, and project data. Do not expand the selection into other lanes or catalogs. Select All must not dirty the project or publish an audio edit. Existing group movement/copy/removal consumes the resulting selection unchanged, with sources first.

## Context-menu power actions

Installed effect and component header menus place Enable/Disable first. If the clicked unit is selected, target the selected units in that owner lane; otherwise target only the clicked unit. Disable when all targets are enabled; Enable when any target is disabled. Apply one common state on menu-item release. Preserve settings, identities, order, collapse state, routes, and other lanes. Opening or dismissing a menu makes no edit. Header power controls retain their single-unit behavior.

`unit_power` shares the label/action and typed settings access. Component scopes pass transient group state through egui stack tags so nested panels retain their existing menus. Apply requests after panel rendering through the enclosing instrument edit. Effects use the existing chain edit; inline effects defer requests until the complete owner chain is available. Preserve unit selection while a popup is open. Retain existing editing, drag, job/dialog, focus, and disabled-UI guards. Missing effects expose an unavailable power item without changing opaque saved state. Save and Undo/Redo reuse the existing enabled fields and transactions.

## Removing selected component units

With the component lane focused, press Delete or Backspace to remove every selected unit through one `Edit::SetInstrument`. This works on regular layers and Main, with compact rows and collapsed units. Surviving units, layer/track/clip selection, and clips stay unchanged. The lane keeps keyboard focus after removal, including when empty. Held-key repeats and additional deletion presses with no selected units do not remove clips or mark the project modified.

Envelope, LFO, Macro, and Amplitude retain their settings, identities, connections, and saved unit positions when removed. Re-add them through the catalog to restore them. Removing the Oscillator unit discards its sources and their pitch routes, as individual source removal does. Main itself is never removed by this action.

Effects and components share `unit_selection::shortcuts` for key handling and pane-focus guards. Text editors keep Delete/Backspace for editing. Extra Alt/Ctrl/Command modifiers, pointer gestures, drags, jobs, dialogs, disabled UI, and window/pane focus loss block unit removal. Blocked keys and held-key repeats cannot fall through to clip deletion while the unit pane owns them. Portable owner settings validate the full removal group before changing any member; invalid or empty groups make no edit.

## Reordering strips

Drag unused space inside an oscillator strip to reorder sources within the Oscillator unit. The same gesture can insert that source between oscillators in another layer. The insertion line marks its position; dropping on a layer label appends it. Source UUIDs and pitch routes stay unchanged during reordering. Source controls and routes remain attached by ID when the displayed Osc N numbers change.

Drag unused space inside a modulation strip to reorder connections within that destination. Amplitude, Auto-filter, Drive, Oscillator, and LFO all use this shared control. Source assignments, target IDs, amounts, power, and route IDs stay attached to the connection when the displayed Mod N numbers change. Reordering does not create a copy. The copy modifier remains available for oscillator transfers and installed effect headers. Strip edge scrolling, release-only commit, cancellation, and clean no-op drops follow the same rules as other unit drags. Source order and route order save with the project.

## Effects

Hold the same modifier while dragging an installed effect header to copy it at the insertion line. If the dragged effect belongs to the installed-unit selection, copy the selected group in chain order. Keep the originals in place and select the copies after release. Copies preserve settings and source assignments, with fresh effect and route IDs. Ordinary dragging retains group reordering. This works in track, Master, and inline layer effect lanes, including compact lanes and collapsed units. Effect headers also move or duplicate selected groups between regular layers, Main, and the track effects lane of the same instrument; see [per-layer effects](../instruments/layer-effects.md).

## Timeline effect drops

Installed effects from the bottom effects lane can move or copy onto visible Timeline track rows. Both a track header and its clip lane target that track’s effect chain. Audio, MIDI, and Group tracks accept these drops, including empty chains. Master can be the source or destination; drop onto its fixed Timeline block to append to its chain. A row drop appends the selected group in its source chain order. Moves within the same chain append/reorder; a move that leaves the order unchanged makes no edit. Option on macOS or Ctrl on Windows/Linux selects copying at release, with fresh effect and route IDs through `EffectInstance::duplicate`.

`timeline_workspace` resolves individual visible row rectangles after scrolling and clipping. Group container outlines do not determine ownership: a child row targets that child, not its group. The clipped fixed Master block resolves to `EffectChainTarget::Master` through the same drag path as track rows. The ruler, scrollbars, clipped rows, and empty space are not targets. A valid row uses `panels::selection_border`; an incompatible modulation source shows `NotAllowed` without an error dialog. Reject the complete group when any effect references a provider unavailable in the destination instrument. Master rejects all assigned instrument providers. Keep assignments intact; do not silently disconnect or remap foreign providers.

Reuse installed-effect activation, selection capture, copy modifiers, shared vertical edge scrolling, cancellation guards, and the atomic core transfer transaction. Hover does not construct units, edit the project, or switch the selected track. Track controls remain disabled during the drag. On a valid release, apply one history edit, refresh the bottom lane snapshot, select the destination track or Master and transferred effects, and focus that lane. Reuse `select_track` and `select_master` for target selection. Escape, focus loss, disabled editing, jobs/dialogs, view/owner changes, invalidated source order, and outside releases cancel through the existing drag path. Ordinary catalog additions and inline layer transfers retain their existing targets.

## Transactions and cancellation

Individual oscillator-strip transfers use `Edit::TransferInstrumentComponent` in `daw-core`. Component groups use `Project::component_group_transfer`, which reuses the individual transfer/ref-remapping rules, resolves destination IDs once, validates the complete result, and prepares one `Edit::Batch` for instrument and track-effect settings. No intermediate group state is published. Same-lane reorder changes only presentation metadata. `Project::can_transfer_components` checks every member without allocating or creating IDs; individual strips retain `can_transfer_component`. Copies keep existing source assignments, including assignments to providers also copied in the group. Effect copies use the target chain transaction: `Edit::SetEffects`, `Edit::SetMasterEffects`, or `Edit::SetLayerEffects`. Cross-layer and cross-track effect groups use `Project::transfer_effects` to prepare and validate both chains as one batch. `Project::can_transfer_effects` checks chain ownership, selected IDs, and destination source scope without cloning the project or generating copy IDs. Project validation and the transfer preflight share the same provider-scope predicate.

The UI shares `instrument_component_drag.rs` for catalog and installed-component drag activation, layer targets, edge scrolling, and cancellation. Effects and installed components reuse `unit_selection.rs` for Shift toggling and lane-order group capture, and `drag.rs::group_insertion` for group insertion and clean no-op detection. They also use the same insertion-line and scrolling primitives. Modulation destinations use `strip_reorder.rs` for one shared temporary gesture. All three use `drag.rs` for insertion positions, preview lines, thresholds, focus, and edge scrolling. Oscillator and effect copies use the existing platform copy modifier. Previewing does not change project data or create copies. Commit only on a valid primary-button release, including a fast motion and release in one frame. Escape, focus loss, jobs/dialogs, disabled editing, changed instrument snapshots, track/view/project replacement, and invalid/outside drops cancel. No-op and cancelled gestures leave the project clean.

Saved transfers use the existing project fields. Save/reopen and Save As preserve the result. Renderer preparation and publication use the existing project-change path; live playback and offline export share the resulting model. Instrument component transfers remain within one instrument. Installed effects also support Timeline track-row drops as described above. Selections spanning multiple lanes and a second Amplitude stage remain unsupported. Automated checks and native platform limits are recorded in [validation](../validation/README.md).

## Registering instrument components

Register each catalog component once in `instrument_catalog.rs::ENTRIES`, with its stable type ID, display name, category and installation function. `Entry::layer` makes a component available to regular layers; `.with_main(add_to_main)` adds Output support. `Entry::provider` registers an Envelope or LFO with its `ModulationLaneKind` and lane installation function. Installation functions update the existing project settings and retain each component's restoration rules.

The catalog list, double-click/Enter additions, drag eligibility, and unsupported-target tooltips read this registration. No component type allowlist belongs in the drag handler. A new registration automatically participates in catalog drops for its supported destinations, including empty and compact rows. Unknown IDs and unsupported targets remain invalid. Mixed selections add only compatible entries on regular layers and Output, in catalog order. A dedicated modulation lane rejects the complete catalog group if it contains an unsupported type.

Eligibility checks inspect the registered functions without calling them. They must not create units, assign IDs, or change settings during preview. Only a valid release or explicit catalog addition invokes installation. Installed component moves and copies continue to use core `Project::can_transfer_component` and the shared transaction; a new transferable component needs its core editing behavior, not another UI drop filter. Portable DSP construction remains in the host.

## Instrument catalog folders

The component catalog groups registered entries in this order:

| Category | Components, in display order |
| --- | --- |
| Sources | Oscillator, Sampler, Granulizer |
| Modulators | Envelope, LFO, Macro |
| Processors | Amplitude |

`Entry::layer` requires a `Category`. `instrument_catalog.rs` defines the category order and labels; membership comes from each registration, without another component allowlist. Entry indices and type IDs remain stable. `catalog.rs` owns optional folders, visible-row order, selection, focus, keyboard input, and scroll state. Effects reuses these folders through its own category metadata; see [Effects catalog folders](#effects-catalog-folders). `ControlPanel` shares row geometry, text, selection painting, and theme colors. Effects, Components and Chords reuse the [canonical list spacing](../views/dropdowns.md#catalog-panels), including inset row highlights, 4-point inner and outer padding and 4-point gaps; folder rows add the standard Lucide chevron and Folder/Folder Open icons. Component names are indented beneath folders. Stable widget IDs depend on folder/entry identity rather than visible position.

All folders start collapsed through the shared `Catalog::with_folders` default. Expansion is application UI state and survives catalog/view switches. It is not serialized with a project and never marks it modified. Folder rows cannot add components, arm component drags, or become members of a component selection. Collapsing a folder removes its children from selection, including the Shift-range baseline. Selections in other folders remain. Both group-drag capture and keyboard additions use visible component order and exclude closed folders.

Up/Down traverses visible folder and component rows without wrapping. Folder focus is separate from component selection. Unmodified movement onto a folder clears selection; Shift movement extends or shrinks ranges over component rows only. Enter toggles the focused folder once per press, Left closes it, and Right opens it. Left on a component closes its parent folder and moves focus there. Enter on a component retains the shared once-per-press addition. Escape and pane-focus rules retain their existing behavior. Mouse folder activation commits on release; existing focus, modal, disabled-UI, and active-drag guards apply. Folder controls remain separate from empty-space deselection.

Use the shared vertical scroll area below the fixed catalog header. Clip row interaction to the viewport, keep the panel border continuous, and scroll keyboard destinations into view. Retain selection when its focused row scrolls offscreen through the shared `SelectionFocus` vertical-scroll mode. Keep scroll positions independent between the two catalogs. Hidden rows have no active drop/press targets. Existing component eligibility, layer/Main compatibility, source conflicts, transactions, and release-only drag commits remain unchanged.

Acceptance covers both themes, category order, collapse/expand, hidden selection pruning, cross-folder selection/drag order, bounded keyboard navigation, Shift ranges, repeated-key guards, folder-state retention, small-height scrolling, and unchanged project data while browsing. Existing component addition, Main-target, cancellation, and Effects interaction tests must pass.

## Effects catalog folders

The Effects catalog uses the same `Catalog::with_folders`, `ControlPanel` folder/entry rows, icons, selection/focus, keyboard navigation, and scrolling as the Instrument catalog. Keep category membership in each `effects_catalog.rs::ENTRIES` registration. Each entry has a stable type ID, name, category, and constructor. Category order belongs to the Effects adapter; do not add a separate type allowlist to painting, keyboard, or drag handlers.

| Category | Effects, in display order |
| --- | --- |
| EQ & Filters | 3-band EQ, Filter, Auto-filter |
| Dynamics | Compressor, OTT, Limiter |
| Distortion | Drive, Bitcrusher, Saturator |
| Imaging | Direction Mixer, Stereo Spread |

Entry indices and effect type IDs remain stable. Visible folder order determines Shift ranges, selected-type capture, Enter additions, and library group drops. Both addition paths instantiate through `effects_catalog::instantiate` in captured order; do not restore registration-array order when building a selected group. Construct fresh instances only for an explicit addition or a valid release. Preview and cancellation create no effect instances or project edits.

All categories start collapsed through the same shared default. Expansion and scroll positions remain independent UI state for the two catalogs; retain them through catalog and Timeline/Instrument view switches, without project serialization or a modified flag. Reuse the folder keyboard and hidden-selection pruning contract above. A folder cannot become an effect selection, start an effect drag, or add an instance. Collapsing one prunes its child selections and Shift baseline; other folders retain their selected entries.

Retain track/Master target dispatch, Instrument's selected MIDI target, installed-chain order/selection, insertion previews, edge scrolling, release-only commit, and all existing focus, text, disabled/modal, active-drag, target, chain-change, and cancellation guards. Scrolling preserves selection when the focused row leaves the viewport; the catalog header and final right divider stay fixed. Hidden children have no active hit regions.

Acceptance covers both themes and Timeline track, Timeline Master, and Instrument targets: folder order, collapse/expand and keyboard behavior, hidden selection pruning, independent folder retention, cross-folder Enter/drop order and fresh IDs, clean previews/cancellation, focus/modal/disabled/drag guards, short-height scrolling, and retained existing unit behavior. Automated validation belongs in [validation history](../validation/README.md).

## Panel borders

Timeline track/Master, regular layer, and Main selection outlines meet all four edges without an inset. They paint after neutral dividers and row fills so no side of the stroke is covered. The fixed Layers header separator paints after scrolling content. Main owns one top separator on the preceding layer's bottom pixel. Its components omit their separate top and bottom unit edges, and header fills cover the reserved top frame space without moving controls; the adjacent effects lane or status panel owns that separator. These borders share the existing panel and theme helpers in both themes and at both row heights.

All regular-layer component rows use the same 1-point overlap with the Layers column, including empty rows and layers containing only Macro. Do not infer spacing from a component-type allowlist. The Layers column owns its continuous right divider. Regular layers, modulation lanes and Output tag their unit scopes with `SHARED_LEFT_BORDER_TAG`; `PanelStyle::unit` honors that tag for expanded and collapsed units, even when top and bottom borders remain enabled. Each preceding unit owns the next shared vertical edge. Omit only the duplicate left stroke and retain frame geometry, other edges, and selection outlines. Check every registered component as the first unit on its supported owner in both themes, compact/expanded rows, and collapsed/expanded units.


Macro headers use these same installed-component gestures. The module moves all its strips, saved names, and targeted input routes, merging into a populated destination. Moves remap all consumers; copies use fresh strip/route IDs and matching targets while retaining source assignments. Both regular layers and Main accept Macro modules. See [Macro editing](../instruments/macro.md#editing-and-saved-state).

## Sampler source ownership — 2026-10-17

The component catalog also exposes [Sampler](../instruments/sampler.md). Each regular layer accepts one Oscillator component or one Sampler component. Oscillator retains multiple internal strips. Sampler stays first, cannot enter Main, and rejects source conflicts during catalog previews, additions, and installed moves/copies. Source headers provide an explicit Replace source action. Existing group selection, insertion, copy IDs, removal keys, and cancellation apply. Sampler copies share immutable audio and receive new sampler IDs. Sampler removal discards its source settings and prunes its saved order entry; other retained components keep their restoration behavior.

## Granulizer sample-source slot

[Granulizer](../instruments/granulizer.md) uses the same sample-input component slot as Sampler, with independent granular settings and catalog identity. Either source excludes Oscillator and the other sample mode; both stay first and cannot enter Main. Existing source moves, copies, removal, selection and drop transactions apply.

## Components and inline effects — 2026-10-24

Only sources are pinned before other units. Regular layers and Output can interleave effects with control components through the saved mixed order in [layer effects](../instruments/layer-effects.md#mixed-inline-order--2026-10-24). Effect entries are presentation references, not catalog components or inputs to the component-transfer API. Keep effect and component selections separate while resolving insertion from their combined geometry. Relative effect order remains the serial chain order; component placement remains presentation-only.
