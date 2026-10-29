# Per-layer effects

Implemented scope: regular MIDI instrument layers and Main own independent serial stereo effect chains. Main effects process the layer sum before Main gain/pan; track effects process its output. Layer effects use all registered effect types, including retained missing-unit settings.

## Model and transactions

`unit-core::InstrumentLayerSettings` and `InstrumentBusSettings` save `effects: Vec<EffectInstance>`. A missing field defaults to an empty chain in existing version-3 instruments. Effect settings retain their existing versions, power, collapse, modulation visibility, routes, and stable identities. Inline effects use the layer's existing saved `collapsed` flag for row height.

`Edit::SetLayerEffects` replaces one chain and the existing layer compact state. `Project::edit` validates the full transaction. Reuse `EffectSettings::valid`, project-wide effect ID validation, and the same instrument-source scope checks as track effects. Invalid owners, duplicate effect IDs, invalid settings, missing providers, and Per note providers reject the complete edit. No independent routing validator is added.

`Project::all_effects` includes layer chains for monitoring, missing-unit discovery, and global identity validation. Track duplication assigns fresh effect IDs to its layer chains. Moving a modulation provider remaps layer-effect consumers through the existing component transfer helper. Layer reordering retains effect state by layer identity. Save, Save As, open, live playback, and export use the same saved chains.

## Audio

The processing order is:

1. Per-voice sources and Amplitude, including velocity and note transitions.
2. Stereo voice sum for each layer, with the existing clip boundary fade.
3. That layer's serial effect chain.
4. Layer gain/pan and mute/solo audibility.
5. Parallel layer sum, Main effects, Main gain/pan, track effects, and track mixing.

One processor instance exists per layer bus, not per note. Reuse `engine::effects::EffectChain` for construction, processing, reset, compatible history retention, shared modulation, and meters. `layer_effects.rs` owns bus mixing and prepared alignment delays. The existing no-layer-effect path retains its original voice-summing and transition calculations; per-layer transition samples support the bus path without changing legacy rendering.

Each layer chain receives cached shared instrument sources, including Main, through the existing typed modulation resolver. Per-note aggregation is not added. Base bounds, bypass ramps, route smoothing, telemetry, and meter clearing remain owned by the existing processors and shared helpers.

Limiter and Saturator latency is aligned between parallel layers before summing. Track alignment includes the maximum regular-layer latency plus Main and track chain latency. Main effect clocks follow the parallel layer latency; track-effect clocks follow the total instrument latency. Both use the shared loop-aware delayed-frame helper, so Main retains the correct internal LFO phase at nonzero loop starts. Serial effect clocks include every processor's shared latency report, including bypassed processors. Seek, loop, reset, and worker pre-roll use the existing renderer lifecycle. A changed distribution of latency between layer buses requires worker pre-roll even when the track's total latency is unchanged.

Compatible layer-chain edits retain voices, oscillator phases, route smoothers, effect histories, gain ramps, and alignment buffers. Gain changes ramp for 5 ms while active and apply immediately when idle. Construction, snapshots, buffers, and delay storage are prepared outside the callback; processing and compatible retargeting allocate no memory and do not release final owners on the callback.

## UI

Regular layers and pinned Main share one saved display order for effects and component units in the same horizontal scroll area. Only audio sources remain first. Effects can precede or sit between Macro, LFO, Envelope and Amplitude. Effects process in their relative left-to-right order; moving control components across effects changes presentation only. The existing layer collapse control changes the height of components and effects together. The bottom effects lane continues to show track effects.

The Effects catalog adds to the selected regular layer or Main in Instrument. Clicking the bottom effects lane targets track effects until a layer or Main is selected. Library drag targets resolve from layer geometry and reuse the common insertion preview, release-only commit, and cancellation. Empty layers and a hidden bottom lane remain valid destinations. Previewing does not construct effect instances.

`effects_panel::effect_units` paints installed effects for both the bottom lane and inline layers. The target supplies the chain, source inventory, and edit command. Catalog registration, unit panels, power/collapse, modulation controls, selection borders, Select All, deletion, group reorder/duplication, previews, edge scrolling, and cancellation reuse the same effects code. No effect type allowlist or parallel panel implementation is added. Paint shared unit dividers after inline fills, including the boundary between the final component and first effect, on both regular layers and Main. Component and effect selection use their respective regions of the row; effect keyboard focus stays on its chain after deletion.

Installed effect dragging reorders or duplicates within a chain and moves/copies selected groups between regular layers, Main, and the track effects lane of the same instrument. `core::EffectChainTarget` identifies owners; `Project::effect_chain` shares owner lookup. `Project::transfer_effects` prepares a validated `Edit::Batch` with source removal and destination insertion, preserving source chain order. Moves retain effect and route IDs; copies use `EffectInstance::duplicate` for fresh effect/route IDs and unchanged source assignments. Same-chain drag copies use that helper too.

Drag activation, insertion previews, edge scrolling, copy modifiers, selection capture, cancellation, and release checks remain in `effects_reorder.rs`. Cross-layer edits stay pending until the instrument finishes drawing, then publish once without overwriting the model with its earlier UI snapshot. Empty destinations, including Main, are valid, including moves toward earlier rows and fast release. Focus and selection follow the destination. Restore the destination focus after all rows have painted; a source row visited later must not replace its pane bounds. Prepare destination chains against the previous layer chains before swapping bus state, matching globally unique effect IDs. Transfers between track and instrument buses use the same history matcher before either chain is swapped. Compatible processor histories move by stable effect ID between layer buses without callback allocation; changed latency placement still requires worker pre-roll.

## Layer bus level meters

Regular layer and Output headers reuse the track `meters::stereo_panel` and `reset_on_click` widgets with `daw-output::StereoMeter`. Keep the existing 250-point label width. Reserve the same 20-point stereo rail column and six-point content gap as tracks; each rail is eight points wide with four points between channels. Fit the rails to the 54-point inner height of the two-row header, with seven-point top/right insets, in both expanded and compact layers. Keep Pan then Gain immediately before the rails; Output retains Gain alone. Keep meters outside the horizontally scrolling units and exclude their press origin from layer reorder gestures. Render silent rails without an output device. Reuse track scale, tooltips, clipping colors and click/reset behavior; add no numeric field.

`engine::layer_effects::LayerEffects` stores display-only bus peaks and the latest processed contribution. `MidiTrackRuntime` records regular layers after voice/clip fades, layer effects, alignment delay and layer gain/pan/audibility, including clip gain. Output records its final stereo sum after its own effects/gain/pan and clip gain, before track effects, track gain, group/track audibility and Master. The legacy no-effect voice sum stays unchanged; separate monitor contributions follow its gain and note transitions. Monitoring must not change audio samples. `Renderer::layer_peaks` returns `((track_id, layer_id), peaks)` and `reset_track_peaks` resets these buffer-local peaks together with tracks. Empty, muted or solo-excluded layers become silent after their existing ramps. Transport silence publishes zero live readings while preserving held peaks and clipping until reset.

`output::layer_meters` prepares a map of shared stereo handles, keyed by track and layer/Output identity, on the control/worker path. Retain handles through reorder and compatible edits; prune removed owners and give copies separate handles. Bind only after worker pre-roll and reset its accumulated peaks before publication. The callback reads renderer peaks and publishes atomics once per buffer, following the same device-converter policy as track meters, without allocation, locks or I/O. Keep old handles alive through renderer retirement and release final owners outside the callback. Clicking either rail clears the shared clipping latch and held peak without changing current samples, levels or project/history. Do not persist telemetry in project data.

## Verification

Core checks cover migration defaults, round-trip settings, transactional validation, source scope, discovery, and duplication. Engine checks compare layer and track placement for every processor, check parallel isolation and limiter alignment, seek reconstruction, telemetry, and allocation-free live updates. UI checks exercise inline placement, catalog addition, power, drag duplication, deletion, and layer targeting in both themes. Project checks cover Save As/reopen, preserved routes and missing units, rendered audio, and WAV export. See [validation](../validation/README.md) for executed results and native limits.

## Mixed inline order — 2026-10-24

`InstrumentComponent::Effect(id)` records inline effect positions in the existing `component_order` field. `visible_units` combines installed components and effects; `visible_components` retains its component-only contract for selection, transfer eligibility and removal. Hidden providers retain their saved slots. Missing legacy order retains components followed by effects. Validation rejects duplicate/stale entries, any source after another unit, and effect entries that do not match a prefix of the serial chain. Effect transfers continue through `Project::transfer_effects`; `position_inline_effects` adds mixed display placement to the same atomic transaction. `SetLayerEffects` prunes removed positions and retargets surviving effect slots to the updated serial order. Layer and track copies remap saved effect identities.

Regular layers and Output paint effects individually through `effects_panel::effect_units`, retaining full-chain group capture, shared unit shells and selection controls. Component and effect drags use the complete ordered row geometry for insertion. Effect drops exclude the source area; component moves clamp after the source. Catalog construction, copies and edits occur on valid release only. Cross-bus transfers retain existing source-scope checks. End-of-frame release cleanup cancels outside drops after every destination has had a chance to consume the gesture. Pane focus distinguishes control selection from effect selection in the same row. Saved order and effect-chain changes remain one history entry; no-op and cancelled drops stay clean.

The renderer keeps its existing source, Amplitude, bus effect and gain/pan stages. Presentation changes alone retain compatible histories and do not change output or allocate on the callback.
