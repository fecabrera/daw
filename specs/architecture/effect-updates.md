# Targeted effect updates

Effect-only edits update the active renderer through worker-prepared transactions.
Source schedules, voices, transport and compatible DSP histories remain active.
Full renderer preparation remains the fallback for incompatible edits.

## Ownership and classification

`engine::EffectUpdate` compares the acknowledged effective plan with the desired
project plan. Both are lowered through the existing beat compiler. The compiler
rejects non-effect changes before creating candidate processors. It compares media
allocation identities through `same_processing_plan`, with effect chains and
effect presentation slots removed for this comparison.

| Change | Preparation and publication |
| --- | --- |
| Chain compact state or effect presentation flags | Publish metadata without retargeting DSP. |
| Controls with the same type, band IDs/order and route membership | Record only changed effect slots; retarget those existing processors. |
| Route amount or enable with unchanged source, target and parameter | Retarget shared route ramps and retain bindings. |
| Compatible add/remove/replace/reorder or cross-owner move | Allocate destination slots/maps; construct only new or incompatible processors; move survivors by stable ID. |
| Internal band or route topology | Prepare the affected unit and inherit its compatible band, route and processing history. |
| Non-effect content, context or timing/dependency change | Use full asynchronous renderer preparation and its existing exact-history activation. |

Direct controls cover all 21 concrete effect types. Portable setters retain
their existing bounds, ramps and intentional lifecycle changes. Loudness mode or
power changes retain their analyzer reset. Limiter lookahead retains its prepared
maximum delay. Missing units preserve opaque settings and have no DSP controls.
Repeated destinations use shared `RoutedDestination` route controls; external
routes use the same `ModulationRoute` control ramp.

There is no limit on project or chain size. A structural transaction moving more
than 4096 destination slots uses the full replacement path. This bounds one
atomic relocation, not the number of installed effects. Direct controls are
excluded from this structural admission count. Native callback timing at the
minimum buffer size remains an acceptance check; the slot bound is conservative,
not a measured hardware deadline guarantee.

## Timing and graph compatibility

Each owner retains its full chain latency. Comparing every layer, Output, track
and master chain preserves the existing parallel-layer distribution and all
unchanged group/send/Return alignment. Non-effect graph configuration must match.
Audio-sidechain route identities, source settings, destination owner and delayed
stage must also match. Amount and power do not change that dependency layout.
Equal total project latency alone is insufficient.

This conservative gate can reject changes that a future graph-local preparer
could support. For example, moving latency between layers retains the full
reconstruction fallback even when the longest layer path stays the same.
Reordering units across a sidechain destination stage also falls back when its
required stage delay changes.

## State and metadata

`AudioOutput::plan` is the latest desired logical state used by save/export and
history. `active_plan` and `active_revision` describe acknowledged playback.
Published plan/binding records wait for the callback's processing acknowledgment.
Do not publish new meter maps merely because a command entered the queue: a
cancelled removal followed by Undo must retain the active unit's original handle.

Transactions contain a complete prepared effect metadata plan, owner indices,
changed slot lists, relocation references, new units, settings, channel targets
and monitoring storage. `Renderer::apply_effect_update` swaps this metadata and
the affected layer settings together with the DSP ownership change. It updates
effect lookup, layer processing flags, channel semantics, end bounds and gain
targets. The source schedule and existing alignment buffers remain unchanged.

For `A → B → C` becoming `A → C → B`, surviving processor owners move once.
Their histories stay alive and the existing 5 ms topology transition applies.
For an ordinary control change on B, only B is retargeted. C continues processing
B's changed output; no historical replay or downstream reconstruction occurs.
Harmonic Lift inheritance matches branches by ID before transferring positional
filter/control/monitor state. Variable split networks keep their existing
compatibility rules rather than claiming arbitrary old crossover history.

## Background preparation and revisions

Output owns separate latest-request workers for controls and effect topology.
Neither waits for the exact transport preparation worker. Each has at most one
pending request and one ready result; superseded results are discarded off audio.

The control worker can prepare an effective plan for surviving compatible units
while topology preparation is pending. That plan retains the active topology.
Controls for new units and incompatible internal topology wait for the complete
transaction. A partial control publication advances a distinct effective token
but does not acknowledge the uninstalled desired topology.

Every candidate carries its requested revision and acknowledged base token.
The callback checks both before mutation. Any intervening publication invalidates
an old base. Output recompiles the latest desired state against the acknowledged
effective plan; it never retargets an old exact replay result and calls it exact.
This serializable base check protects all controls in a transaction without
maintaining independent, conflicting per-unit command streams.

Every queued owner also receives a unique publication ID, separate from the
desired revision. A control candidate and a topology candidate can share a
desired revision. Rejecting one removes only its own publication record. The
callback acknowledges the exact installed publication, and retirement processing
refreshes that acknowledgment before releasing records. This prevents a stale
sibling from removing the active metadata during a concurrent drain.

Seek/Play/Stop preparation retains its separate transport intent and supersedes
effect candidates through the existing request revision. Stop retains immediate
silencing. A partial effect update cannot falsely complete a transport request.
For unchanged source content, controls can update the audible renderer while an
exact seek or fallback rebuild is pending. These publications are always partial;
they neither move the playhead nor acknowledge the pending full result.
Undo/Redo use normal validated project edits; no DSP state or acknowledgment is
stored in history or manifests. Failure keeps prior playback and reports through
the existing preparation-error path. Later edits or Undo can retry.

## Callback and resource lifecycle

Only checked publication and portable setters run in the callback. Destination
slots, maps, route banks, monitoring lists and processors are allocated on workers.
Reused units keep their handles. New units are bound before publication.

The callback moves old processors, metadata and containers into the consumed
transaction. A separate bounded retirement queue returns them to the control
thread. Queue-full publication is retried; retirement-full publication retains
the complete candidate in a preallocated pending slot. No final Box/Vec/Arc owner
is dropped on the callback's successful, stale or backpressure paths.

Candidate storage participates in the 512 MiB output-state admission policy.
Workers reserve only candidate storage, without reserving the unchanged active
DSP a second time. Publication divides the existing active and candidate leases
between the new active capacity and the retirement envelope using allocation-free
owned accounting. Shared-media owner accounting remains outside audio.

## Exact caches and diagnostics

Every processing edit invalidates exact DSP checkpoints through the existing
strict plan comparison. Optional warming follows the new desired plan and yields
to editing. PCM, peaks and valid prepared media stay shared. Never seed exact
checkpoints from histories accumulated through live edits.

Future seeks/starts reconstruct the captured desired settings through the normal
renderer/cache path. Offline export uses that same saved model and DSP. Continuous
playback retains its actual edit trajectory, which is a different reference from
reconstruction with constant saved settings.

`PerformanceSnapshot` reports applied effect transactions, processors prepared in
applied transactions, retries of complete transactions, and fallback counts for arrangement, latency,
sidechain and publication admission. Superseded preparation is not included in
the applied-processor count. Existing activation/command timing remains available.

Run `cargo run -p daw-engine --release --example effect_updates` for a synthetic
CSV comparison of full preparation/inheritance versus patch preparation/handoff
at 1, 8, 32 and 128 units. It includes Drive chains and a mixed reverb case for
controls and reorders. It does not measure UI, queueing, callback/device response,
audible transitions or native deadline compliance.

## Validation

`output::effect_update_tests` exercises ordinary controls for every effect type,
stable-ID moves, new-only preparation, route ramps, band changes, rapid async
edits, partial control publication, stale bases, retirement saturation, layers,
beat lowering, groups, Returns and sidechain stages under the callback heap guard.
Record executed debug/release, workspace, CLI and native results separately in
[validation history](../validation/README.md).
