# Shared modulation contract

Implemented on 2026-10-15. The portable contract and runtime code live in `crates/unit-core/src/modulation`. They have no DAW, UI, device, or backend dependencies. Envelope provides a control output. Amplitude, Auto-filter, and Drive consume parameter inputs through this contract.

A modulation output carries a normalized control value. A modulation input is a destination parameter that receives that value through a signed amount. Audio ports remain separate. A unit can declare both kinds of modulation port.

## Shared code and ownership

| Component | Responsibility |
| --- | --- |
| `ModulationSource` | Declares output name/range and produces the next control sample. Envelope implements it. |
| `ModulationSourceSettings` | Describes saved provider identity, scope, presence, and enable state. Envelope instances implement it. |
| `ModulationOutput` | Advances a provider once, normalizes its value, applies the common source-enable weight, and publishes a cached `ModulationValue`. |
| `ModulationSourceId` | Identifies a provider by stable layer and unit IDs, without naming its concrete unit type. |
| `ModulationDestinationSettings` | Declares supported parameters and scope; exposes saved routes and uses common route validation. |
| `ModulationDestination` | Declares the same parameters/scope as its settings and consumes typed `ModulationInputs`. Its audio type can be mono or stereo. |
| `ModulationRoute` | Resolves a cached source and smooths route amount/power changes. |
| `ModulationInputs` | Sums supported routes by parameter identity and resolves the amplitude gate/release rules. Declaration order does not determine a parameter's meaning. |
| `ModulationSmoother` | Applies the same 1 ms smoothing to Auto-filter and Drive offsets, with common nonfinite-input handling. |
| `process_modulation` | Standard destination entry point: checks source scope, resolves routes, then invokes the unit. Playback and export use it for all three destinations. |

`UnitDescriptor::provides_modulation()` and `consumes_modulation()` inspect the declared modulation ports. These capabilities are independent of `is_source`, which identifies audio generators. Envelope declares a modulation output. Amplitude, Auto-filter, and Drive declare modulation inputs.

`InstrumentSettings::modulation_sources()` enumerates providers through their settings contract. The source selector, labels, range indicators, and core reference validation use this metadata instead of assuming that every provider is an Envelope. Retained removed providers remain available to reference validation; the picker lists only present providers. Disabled providers remain listed and produce a neutral contribution after their enable ramp.

The host still owns concrete unit construction, note scheduling, voice identity, and storage. The current DAW registers Envelope as its only routable provider. This contract does not add an LFO unit, graph editor, audio sidechain, cross-track connection, or multiple outputs per provider.

## Signal and scope rules

- `Unipolar` outputs span 0–1; `Bipolar` outputs span −1–1. The shared output code clamps finite samples and replaces nonfinite samples with zero. Source metadata also supplies the UI's possible-range bounds.
- The host advances each source exactly once per sample and fans its cached value out to destinations. Reading a route never advances the source. Disabled providers keep advancing; their contribution weight ramps over 5 ms.
- `PerNote` destinations can consume local or shared values. `Shared` destinations accept shared values only. The common entry point excludes local values for shared destinations, including outside the DAW. Core validation also rejects invalid saved connections. Callers must put values in the correct local/shared buffer; the host owns their scope.
- Amplitude is a per-note destination. Auto-filter and Drive are shared track destinations. Retrigger and Legato envelopes supply shared values; Per note supplies voice-local values. All current connections stay in the same MIDI instrument. Audio tracks and Master have no instrument provider.
- Routes start unassigned, enabled, and at zero amount. Amounts must be finite and within the target's bound. Route IDs must be unique within the destination. Unsupported targets or invalid source references fail project validation as one transaction.
- Additive routes sum `source × amount × source weight` in each parameter's units. Missing, disabled, unassigned, or invalid runtime values contribute zero. Multiple routes to the same parameter add, including signed cancellation. Route amount/power changes ramp over 5 ms. Reset takes the current targets without a ramp.
- Amplitude receives a resolved multiplicative factor. With no effective route it follows the note gate. Full-depth per-note envelopes preserve the existing ADSR arithmetic. Shared tails belong only to the final released voices; older voices cannot revive. Partial depth blends with the gate and reaches silence at release end. These rules are shared resolver code, not Envelope-specific destination logic.
- Auto-filter and Drive apply the common 1 ms offset smoother before unit-specific bounds. Amplitude retains its note-gate, level, and bypass behavior without adding an offset smoother to its ADSR. Each unit owns its parameter conversion, final bounds, bypass, and audio DSP. Base settings never change when modulation runs.

See [Envelope components](midi-patterns.md#envelope-components), [Auto-filter](auto-filter.md), and [Drive modulation](drive.md#envelope-modulation) for controls, units, bounds, and existing lifecycle behavior.

## Adding a source

1. Implement `ModulationSource` in the portable unit crate. Declare its normalized output range and implement allocation-free sample production. Declare `PortKind::Modulation` in the unit's output ports.
2. Implement `ModulationSourceSettings` for its saved settings, using the same output descriptor. Supply a stable unit ID, scope, presence, and enable state.
3. Register its saved settings in the host's `modulation_sources()` enumeration and prepare its concrete runtime/lifecycle on the control thread. Publish samples through `ModulationOutput`; reuse cached values for all destinations. New provider types require this host registration but do not require a new picker or route evaluator.
4. Test range/enable behavior, scope, fan-out, state inheritance, serialization, and callback allocation. Note-driven providers also need note-off, overlap, seek, and loop tests.

## Adding a destination

1. Implement `ModulationDestinationSettings`. Declare supported `ModulationParameter` targets and destination scope. Use its shared `valid_modulations()` check in settings validation. New parameter kinds also need labels, amount limits, units, and typed input storage in `unit-core`.
2. Implement `ModulationDestination` in the portable unit crate. Take parameters and scope from the settings declaration. Read the typed input frame, convert parameter units, and apply the unit's bounds. Add a modulation input port to its descriptor. Reuse `ModulationSmoother` for additive continuous offsets.
3. Have the host prepare routes and call `process_modulation` for each sample. Reuse the destination-owned UI strips with the same parameter/scope declarations and the existing indicator helpers. Do not add another source lookup, route-summing loop, or smoothing implementation.
4. Test neutral equivalence, all supported parameters, bounds, invalid inputs, smoothing, compatible updates, and callback allocation. Live playback and export must use the same path.

The older scalar Amplitude API and positional Auto-filter/Drive offset methods remain low-level adapters for existing standalone callers. DAW rendering uses the typed entry point. The offset adapters use the same smoother and DSP as the typed interface.

## Saved data and real-time constraints

Connections, amounts, enable state, and `modulation_open` keep their existing saved format and defaults. The Rust identity is now `ModulationSourceId { layer, unit }`. JSON continues writing the historical `envelope` key for that unit ID and also accepts `unit` when reading. This retains existing projects without a version change; the key does not constrain the provider type.

Prepare source instances, route vectors, cached-value buffers, and replacement state before publication. Runtime sampling, resolution, smoothing, and retargeting must not allocate, lock, log, wait, or perform file I/O. Compatible replacements retain stable source/route identities and DSP history; the host retires old storage outside the callback. Seek/reset and failure handling retain the existing host rules.
