# Moving and copying units

Instrument components can move between regular layers in the same instrument. Drag a unit header onto the destination layer's label or component area. The effects lane’s insertion line marks the destination position. Hold Option on macOS, or Ctrl on Windows/Linux, to copy. The modifier can change during the drag; its state at release determines the action. The catalog does not need to be open.

Drag unused space inside an individual oscillator strip to transfer that oscillator. Waveform, pitch, gain, source power, unit power, mode selectors, and modulation controls keep their existing gestures. Double-clicking a unit header still collapses or expands it.

## Destination rules

| Component | Drop behavior |
| --- | --- |
| Oscillator header | Append all sources and their pitch routes to the destination Oscillator unit, in source order. |
| Individual oscillator | Insert that source and only its pitch routes at the source-strip insertion line; a label drop appends it. Other sources and routes stay in place. |
| Envelope or LFO | Place the independent component at the insertion line. Moves update existing modulation connections throughout the instrument and its track effects. Copies start with no consumers. |
| Amplitude | Transfer the stage and its connections to a regular layer without an installed Amplitude stage. An occupied stage rejects the drop. |
| Macro header | Merge all strips and their targeted routes into the destination Macro component; moves update all consumers and copies receive fresh strip/route IDs. |
| Main | Accept shared Envelope, LFO, and Macro components. Reject Per note envelopes, Oscillator, and Amplitude. Moving a component does not change its mode. |

A copy can stay in the source layer where the destination rules permit it. Individual oscillators can also move to an insertion position in their current layer. Unit headers can also reorder components in their current layer; Oscillator stays first. There is one Oscillator unit and one optional Amplitude stage per regular layer; this operation does not add a second unit of either type. An Oscillator merge uses the destination unit's enable, collapse, and modulation-open state. An empty destination takes those settings from the source unit. Individual oscillator settings always travel with the source. Layer gain, pan, mute, solo, name, order, and compact state stay with the layer.

Sources moved out of a layer are removed from its component list. They are not retained as hidden copies. If a legacy source ID already exists in the destination, the moved component receives a new ID and its connections are remapped. Other destination components and their connections remain intact. Removed destination Amplitude settings are replaced only when that slot accepts an explicit transfer.

Copies receive new component and connection IDs, including per-oscillator pitch targets. A copied destination keeps its existing source assignments. Existing consumers of a copied Envelope/LFO keep using the original provider. Base parameters, signed amounts, power, unit collapse, and modulation-open state are preserved. No modulation source or audio processing algorithm is duplicated in the UI.

LFO components carry their incoming Depth/Rate connections and modulation-panel open state when moved or copied. Copies receive fresh incoming route IDs while retaining source assignments. Moving any provider also updates LFO consumers, including those on Main. Self/feedback cycles remain invalid under the [shared dependency rules](modulation.md#providers-that-also-consume-modulation).

## Selecting and reordering component units

Single-click a component to select it. It uses the same outline, click detection, and pane-focus helpers as an installed effect. Selecting another component replaces the selection. Clicking empty component space or moving focus outside its lane clears it. Selection is UI state; it does not mark the project modified or save with the project. Layer and catalog selection remain independent. Controls keep their own gestures and focus. The outline covers the complete unit, including its visible modulation controls, and clips to its lane while scrolling.

Drag a unit header to reorder it within a regular layer or Main. A collapsed unit uses its vertical title strip. Oscillator is an audio source and stays first; other component units can appear in any order after it. Dragging another unit to the left of Oscillator places it immediately after Oscillator. Main has no audio source unit and permits any component order. The same insertion-line renderer, pointer threshold, focus restoration, edge scrolling, and release-only commit rules serve effects and components. The selected unit keeps its outline during the drag; there is no separate row outline for installed-component drops. Same-position drops show no insertion line and make no edit.

Transfers between lanes, including empty Main, use this same insertion preview. Drop in a component area to choose its position, or on a layer label to append it. An incoming Oscillator unit remains first and merges its sources into the destination source unit. A Macro transfer merges its strips into the destination Macro unit at the indicated unit position. Existing type, mode, identity, and connection rules still apply. Copies select the resulting component; moves select it on its new owner. Invalid destinations show the standard forbidden cursor and make no edit.

Each regular layer and Main save a `component_order` list in instrument version 3. Envelope/LFO entries use stable unit IDs; Oscillator, Macro, and Amplitude use their owner’s singleton unit identity. Missing lists default to the previous canonical order. Partial lists append missing units in canonical order, with sources first. Retained removed units keep their position when restored; physically transferred providers lose their old owner's entry. New providers append after the saved entries. Validation rejects duplicate, unknown, stale, individual-oscillator-strip, and misplaced-source entries. Invalid insertion indices fail as one project transaction.

This order changes presentation only. Provider vectors, source-strip order, parameter values, routes, audio topology, phases, and held notes retain their existing behavior. Save/reopen, Save As, track duplication, rate changes, layer reorder, and view switches retain the layout. Component identities and order policy live in `unit-core`; core transactions and the UI use those shared definitions.

## Reordering strips

Drag unused space inside an oscillator strip to reorder sources within the Oscillator unit. The same gesture can insert that source between oscillators in another layer. The insertion line marks its position; dropping on a layer label appends it. Source UUIDs and pitch routes stay unchanged during reordering. Source controls and routes remain attached by ID when the displayed Osc N numbers change.

Drag unused space inside a modulation strip to reorder connections within that destination. Amplitude, Auto-filter, Drive, Oscillator, and LFO all use this shared control. Source assignments, target IDs, amounts, power, and route IDs stay attached to the connection when the displayed Mod N numbers change. Reordering does not create a copy. The copy modifier remains available for oscillator transfers and installed effect headers. Strip edge scrolling, release-only commit, cancellation, and clean no-op drops follow the same rules as other unit drags. Source order and route order save with the project.

## Effects

Hold the same modifier while dragging an installed effect header to copy it at the insertion line. If the dragged effect belongs to the installed-unit selection, copy the selected group in chain order. Keep the originals in place and select the copies after release. Copies preserve settings and source assignments, with fresh effect and route IDs. Ordinary dragging retains group reordering. This works in track and Master effect lanes, including compact lanes and collapsed units.

## Transactions and cancellation

Instrument transfers use `Edit::TransferInstrumentComponent` in `daw-core`. One project transaction moves/copies settings, remaps targets and source references, and validates the final project. `Project::can_transfer_component` provides allocation-free drop eligibility; it creates no IDs. Effect copies use the existing `Edit::SetEffects` or `Edit::SetMasterEffects` transaction.

The UI shares `instrument_component_drag.rs` for catalog and installed-component drag activation, layer targets, edge scrolling, and cancellation. Effects and installed components use the same insertion-line and scrolling primitives. Modulation destinations use `strip_reorder.rs` for one shared temporary gesture. All three use `drag.rs` for insertion positions, preview lines, thresholds, focus, and edge scrolling. Oscillator and effect copies use the existing platform copy modifier. Previewing does not change project data or create copies. Commit only on a valid primary-button release, including a fast motion and release in one frame. Escape, focus loss, jobs/dialogs, disabled editing, changed instrument snapshots, track/view/project replacement, and invalid/outside drops cancel. No-op and cancelled gestures leave the project clean.

Saved transfers use the existing project fields. Save/reopen and Save As preserve the result. Renderer preparation and publication use the existing project-change path; live playback and offline export share the resulting model. This feature does not add cross-track transfers, component multi-selection, or a second Amplitude stage. Automated checks and native platform limits are recorded in [validation](validation.md).

## Registering instrument components

Register each catalog component once in `instrument_catalog.rs::ENTRIES`, with its stable type ID, display name, and layer installation function. `Entry::layer` makes it available to regular layers. Add `.with_main(add_to_main)` when the component also supports Main. Installation functions update the existing project settings and retain each component's restoration rules.

The catalog list, double-click/Enter additions, drag eligibility, and unsupported-target tooltips read this registration. No component type allowlist belongs in the drag handler. A new registration automatically participates in catalog drops for its supported destinations, including empty and compact rows. Unknown IDs and unsupported targets remain invalid. Mixed selections add only compatible entries, in catalog order.

Eligibility checks inspect the registered functions without calling them. They must not create units, assign IDs, or change settings during preview. Only a valid release or explicit catalog addition invokes installation. Installed component moves and copies continue to use core `Project::can_transfer_component` and the shared transaction; a new transferable component needs its core editing behavior, not another UI drop filter. Portable DSP construction remains in the host.

## Panel borders

Timeline track/Master, regular layer, and Main selection outlines meet all four edges without an inset. They paint after neutral dividers and row fills so no side of the stroke is covered. The fixed Layers header separator paints after scrolling content. Main owns one top separator on the preceding layer's bottom pixel. Its components omit their separate top and bottom unit edges, and header fills cover the reserved top frame space without moving controls; the adjacent effects lane or status panel owns that separator. These borders share the existing panel and theme helpers in both themes and at both row heights.


Macro headers use these same installed-component gestures. The module moves all its strips, saved names, and targeted input routes, merging into a populated destination. Moves remap all consumers; copies use fresh strip/route IDs and matching targets while retaining source assignments. Both regular layers and Main accept Macro modules. See [Macro editing](macro.md#editing-and-saved-state).
