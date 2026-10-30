# Instrument-wide modulation lanes

For controls and tasks, see the [user guide](../../docs/instruments/modulation-lanes.md). This contract supersedes the previous placement of Envelope and LFO components on regular layers and Output.

## Ownership and placement

One instrument owns two fixed lanes: LFO and Envelope. Store them separately from audio layers as `InstrumentSettings::lfo_lane` and `envelope_lane`, with stable owner UUIDs. Reuse `InstrumentBusSettings` for component lists, presentation order and compact state. Box these two settings owners to keep `Edit::SetInstrument` small, and include both allocations and their provider/route storage in shared heap accounting.

These are control lanes, not audio buses. The LFO lane permits only LFO settings; Envelope permits only Envelope settings, including Per note mode. Reject effects, Macro controls, non-neutral audio mix fields and duplicate owner/provider IDs. They do not participate in audio summing, layer solo, latency alignment or audio level metering. Regular audio layers and Output retain their existing processing order and mix behavior.

Reuse the Output row implementation for their column labels, horizontally scrolling unit bodies, power actions, component selection and shared right toolbar. The two lanes follow regular layers inside the vertical viewport, in LFO then Envelope order. Output remains pinned. Lane headers expose no name editor, layer actions, M/S, pan, gain or meter. Their toolbar expands downward and collapses upward; compact state is saved independently of unit collapse. Paint shared separators and selection outlines in the same order as regular rows. Scrolling control lanes use `ControlPanel::row_dividers`; only pinned Output uses `pinned_row_divider`. Adjacent rows must share one border pixel.

## Catalog and installed gestures

Register a provider once through `Entry::provider`, with its `ModulationLaneKind` and installation function. This metadata drives double-click/Enter additions, catalog previews, valid releases and hints. Preview must not construct settings or assign UUIDs. Neither regular layers nor Output receive new Envelope/LFO additions. A dedicated lane rejects the entire mixed catalog group if any selected type is unsupported.

Core transfer eligibility also checks the lane kind. A global provider can reorder or copy within its matching lane; it cannot move/copy to regular layers, Output or the other provider lane. Reuse component group transactions, insertion geometry, edge scrolling, release-only commits and cancellation. Preserve unit/route identities on reorder; copies receive fresh identities and retain incoming assignments. Shared selection, Delete/Backspace, retained removal, restoration and Undo/Redo apply unchanged. Effects catalog additions and effect drops cannot target control lanes.

## Modulation and processing

Extend the shared provider discovery and destination enumerators. Source labels group under LFO and Envelope, using lane owner ID and unit UUID. Do not add provider-specific routing loops or sampling in consumers. Global LFOs retain all three restart modes. Global envelopes retain Per note, Retrigger and Legato; moving placement does not convert mode or scope. Pass the existing shared-consumer source mask into the common Envelope body, so Per note remains unavailable while a shared destination uses that provider. Removing that assignment restores Per note availability.

The host prepares voice-local Envelope controls from the shared instrument enumeration, independently of audio-layer storage. Shared controls enumerate the same settings filtered by mode; LFO scheduling also includes the LFO lane. Each provider advances once per voice or shared sample as appropriate. Reuse dependency planning, cached publication, scope/cycle checks, route smoothing, telemetry, seek preparation and compatible state inheritance. Do not change note scheduling, release ownership or DSP arithmetic. Allocation, final-owner retirement, locks and I/O remain prohibited in the callback.

## Saved data and migration

Instrument revision 4 adds both required lane fields. Boxing does not change their JSON shape. Explicit null, missing revision-4 lane fields and malformed settings fail decoding or validation. Revision 1–3 project instruments receive deterministic lane identities scoped by the saved track UUID. The host uses `ModulationLaneKind::legacy_owner_id`, probes existing audio-layer/Output/provided-lane identities, and preserves explicitly saved lane IDs. Identical legacy instruments on different tracks must not acquire interchangeable provider addresses. Inspect the original instrument revision at the track decode boundary before portable decoding upgrades it; reuse the already locked `serde_json` dependency for this inspection.

For older project instruments, first validate the original settings. Then move every installed or retained Envelope/LFO from regular layers and Output into its matching lane. Preserve each type's saved presentation order within each previous owner and visit owners in saved layer order, then Output. Prune former unit-order entries. Retain provider IDs where possible; resolve collisions between former owners from their original scoped identities, independently of display order. Apply the complete old/new source map once to every receiving route, including provider inputs, source/Amplitude/Sampler inputs, layer/Output effects, Macro routes and track effects. Keep route IDs, settings, power, presence, unit collapse and modulation-open state. Never convert Per note to a shared mode.

Migration occurs on project loading before preparing playback, creating history or publishing the session. Reopening revision-4 canonical data makes no changes. The portable legacy layer/Main fields remain readable for direct callers; new catalog installations use only the dedicated lanes. Older clients reject revision 4 rather than discarding the new lane fields. Save As and sample-rate changes retain lane identities and assignments. Deleting or duplicating a regular audio layer does not own or remove the global providers.

## Verification

Cover collision-safe deterministic migration, retained units, presentation order, every receiving route class, scope/mode retention, malformed fields and repeat opens. Compare pre/post-migration samples for all three Envelope modes at all six processing rates, including dependent LFO input routing and callback allocation guards. Exercise every registered catalog entry against both lanes, themes, unit bodies, compact/expanded rows, toolbar exclusions, power, invalid cross-lane drops, release-only reorder, selection, removal and Undo. Keep full project, engine, output, CLI and workspace checks.
