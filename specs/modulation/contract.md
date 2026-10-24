# Shared modulation contract

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/modulation/README.md).

Implemented on 2026-10-15. The portable contract and runtime code live in `crates/unit-core/src/modulation`. They have no DAW, UI, device, or backend dependencies. Envelope, LFO, and Macro provide control outputs. Oscillator, Sampler, Amplitude, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, Bitcrusher, Envelope, LFO, and Macro consume parameter inputs through this contract.

A modulation output carries a normalized control value. A modulation input is a destination parameter that receives that value through a signed amount. Audio ports remain separate. A unit can declare both kinds of modulation port.

## Shared code and ownership

| Component | Responsibility |
| --- | --- |
| `ModulationSource` | Declares output name/range/behavior and produces the next control sample using `ModulationSourceContext`. Envelope, LFO, and Macro implement it. |
| `ModulationSourceSettings` | Describes saved provider identity, scope, presence, and enable state. Envelope, LFO, and Macro instances implement it. |
| `ModulationOutput` | Advances a provider once, normalizes its value, applies the common source-enable weight, and publishes a cached `ModulationValue`. |
| `ModulationSourceId` | Identifies a provider by stable layer and unit IDs, without naming its concrete unit type. |
| `ModulationDestinationSettings` | Declares supported parameters and scope; exposes saved routes and uses common route validation. |
| `ModulationDestination` | Declares the same parameters/scope as its settings and consumes typed `ModulationInputs`. Its audio type can be mono or stereo. |
| `ModulationSourceIndex` | Prepared source-slot lookup with generation identity; pass local/shared indices through `ModulationContext` and rebuild after publication order changes. |
| `ModulationRoute` | Resolves a cached source and smooths route amount/power changes. |
| `ModulationTargetId` | Identifies a repeated destination instance independently of its display position. |
| `ModulationInputs` | Filters the optional instance identity and sums supported routes by parameter identity and resolves the amplitude gate/release rules. Declaration order does not determine a parameter's meaning. |
| `ModulationSmoother` | Applies the same 1 ms smoothing to Oscillator, Sampler, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, Bitcrusher, Envelope, LFO, and Macro offsets, with common nonfinite-input handling. |
| `process_modulation` | Standard destination entry point: checks source scope, resolves routes, then invokes the unit. Playback and export use it for all destinations. |

`UnitDescriptor::provides_modulation()` and `consumes_modulation()` inspect the declared modulation ports. These capabilities are independent of `is_source`, which identifies audio generators. Envelope, LFO, and Macro declare modulation outputs. Oscillator, Sampler, Amplitude, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, Bitcrusher, Envelope, LFO, and Macro declare modulation inputs. Envelope, LFO, and Macro declare both input and output ports.

`InstrumentSettings::modulation_sources()` enumerates providers through their settings contract. The source selector, labels, range indicators, and core reference validation use this metadata instead of assuming that every provider is an Envelope. Retained removed providers remain available to reference validation; the picker lists only present providers. Disabled providers remain listed and produce a neutral contribution after their enable ramp.

The host still owns concrete unit construction, note scheduling, voice identity, and storage. The DAW registers Envelope and [LFO](../instruments/lfo.md) and [Macro](../instruments/macro.md) providers in regular layers and the fixed [instrument Main bus](../instruments/main.md). Main permits shared sources only. This contract does not add a graph editor, audio sidechain, cross-track connection, or multiple outputs per provider.

## Signal and scope rules

- `Unipolar` outputs span 0–1; `Bipolar` outputs span −1–1. The shared output code clamps finite samples and replaces nonfinite samples with zero. `ModulationSource::modulation_weight` supplies an optional normalized contribution depth (default 1). Publication multiplies it by the source-enable ramp, keeping zero Depth neutral even for Amplitude. Source metadata also supplies the UI's possible-range bounds.
- The host advances each source exactly once per sample and fans its cached value out to destinations. Reading a route never advances the source. Disabled providers keep advancing; their contribution weight ramps over 5 ms.
- `PerNote` destinations can consume local or shared values. `Shared` destinations accept shared values only. The common entry point excludes local values for shared destinations, including outside the DAW. Core validation also rejects invalid saved connections. Callers must put values in the correct local/shared buffer; the host owns their scope.
- Oscillator Pitch/Gain and unit Pitch/Level, Sampler Gain, and Amplitude are per-note destinations. Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, and Bitcrusher are shared track destinations; LFO Depth/Rate and Macro Value are shared component destinations. Retrigger and Legato envelopes supply shared values; Per note supplies voice-local values. LFOs supply shared values in every restart mode; Macro strips supply shared continuous values. All current connections stay in the same MIDI instrument. Audio tracks and Master have no instrument provider.
- Routes start unassigned, enabled, and at zero amount. Amounts must be finite and within the target's bound. Route IDs must be unique within the destination. Unsupported targets or invalid source references fail project validation as one transaction.
- Additive routes sum `source × amount × source weight` in each parameter's units. Missing, disabled, unassigned, or invalid runtime values contribute zero. Multiple routes to the same parameter add, including signed cancellation. Route amount/power changes ramp over 5 ms. Reset takes the current targets without a ramp.
- Amplitude receives a resolved multiplicative factor. With no effective route it follows the note gate. Full-depth per-note envelopes preserve the existing ADSR arithmetic. Continuous sources form a separate multiplicative gain group after contours; bipolar continuous values normalize to 0–1 for this factor. Shared tails belong only to the final released voices; older voices cannot revive. Partial depth blends with the gate and reaches silence at release end. Only `Contour` values with matching release ownership can extend a note. `Continuous` values cannot revive or extend voices. These rules are shared resolver code, not provider-specific destination logic.
- Oscillator, Sampler, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, Bitcrusher, LFO, and Macro apply the common 1 ms offset smoother before unit-specific bounds. Amplitude retains its note-gate, level, and bypass behavior without adding an offset smoother to its ADSR. Each unit owns its parameter conversion, final bounds, bypass, and audio DSP. Base settings never change when modulation runs.

See [Sampler Gain modulation](../instruments/sampler.md#gain-modulation--2026-10-17), [Oscillator pitch and gain modulation](oscillator-pitch.md), [Envelope components](../instruments/midi-patterns.md#envelope-components), [Auto-filter](../effects/auto-filter.md), [Drive modulation](../effects/drive.md#envelope-modulation), [EQ modulation](../effects/equalizer.md#modulation--2026-10-17), and [Compressor modulation](../effects/compressor.md#modulation--2026-10-17), and [Limiter modulation](../effects/limiter.md#modulation--2026-10-17) for controls, units, bounds, and existing lifecycle behavior.

`ModulationSourceContext` supplies absolute project frame, tempo, and optional prepared note and legato origins. Origins can be negative for notes preceding a trimmed clip. Envelope ignores this context and keeps its note lifecycle. LFO uses it for deterministic clocks. The host prepares timing data and freezes calls while paused; sources never read a device or UI clock. Published values carry their current range and behavior, so resolution and indicators use the same contract.

## Adding a source

1. Implement `ModulationSource` in the portable unit crate. Declare its output range and behavior (`Contour` or `Continuous`) and implement allocation-free sample production. Override `modulation_output()` on both runtime and settings when the range depends on saved settings. Declare `PortKind::Modulation` in the unit's output ports.
2. Implement `ModulationSourceSettings` for its saved settings, using the same output descriptor. Supply a stable unit ID, scope, presence, and enable state.
3. Register its saved settings in the host's `modulation_sources()` enumeration and prepare its concrete runtime/lifecycle on the control thread. Publish samples through `ModulationOutput`; reuse cached values for all destinations. New provider types require this host registration but do not require a new picker or route evaluator.
4. Test range/enable behavior, scope, fan-out, state inheritance, serialization, and callback allocation. Note-driven providers also need note-off, overlap, seek, and loop tests. Continuous providers must also prove they cannot extend a note.

## Adding a destination

1. Implement `ModulationDestinationSettings`. Declare supported `ModulationParameter` targets and destination scope. Use its shared `valid_modulations()` check in settings validation. New parameter kinds also need labels, amount limits, units, and typed input storage in `unit-core`.
2. Implement `ModulationDestination` in the portable unit crate. Take parameters and scope from the settings declaration. Read the typed input frame, convert parameter units, and apply the unit's bounds. Add a modulation input port to its descriptor. Reuse `ModulationSmoother` for additive continuous offsets.
3. Have the host prepare routes and call `process_modulation` for each sample. Reuse the destination-owned UI strips with the same parameter/scope declarations and the existing indicator helpers. Do not add another source lookup, route-summing loop, or smoothing implementation.
4. Test neutral equivalence, all supported parameters, bounds, invalid inputs, smoothing, compatible updates, and callback allocation. Live playback and export must use the same path.

The older scalar Amplitude API and positional Auto-filter/Drive offset methods remain low-level adapters for existing standalone callers. DAW rendering uses the typed entry point. The offset adapters use the same smoother and DSP as the typed interface.

## Note-relative provider timing

Envelope remains a `ModulationSource`. The host passes an optional full note duration through `Envelope::note_on_with_duration` in processing-rate frames; the existing `note_on()` uses the fixed-time fallback when duration is unknown. Note lengths are prepared with the event schedule, including stretch, off the audio thread. The portable Envelope scales only Attack and Decay; destinations consume the same cached normalized signal through `process_modulation`. No destination needs note-length logic or a separate route type. Per note, Retrigger, and Legato retain their existing scope, fan-out, release ownership, and reconstruction rules. See [Envelope timing and overlap behavior](../instruments/midi-patterns.md#envelope-unit).

## Saved data and real-time constraints

Connections, amounts, enable state, and existing `modulation_open` fields keep their saved defaults. Repeated destinations can add an optional route `target` UUID; ordinary destinations omit it. Oscillator saves `oscillator_modulations` and `oscillator_modulation_open` on its layer, with empty/closed legacy defaults. The Rust identity is now `ModulationSourceId { layer, unit }`. JSON continues writing the historical `envelope` key for that unit ID and also accepts `unit` when reading. This retains existing projects without a version change; the key does not constrain the provider type.

Prepare source instances, route vectors, cached-value buffers, and replacement state before publication. Runtime sampling, resolution, smoothing, and retargeting must not allocate, lock, log, wait, or perform file I/O. Compatible replacements retain stable source/route identities and DSP history; the host retires old storage outside the callback. Seek/reset and failure handling retain the existing host rules.

## Repeated destinations

Oscillator uses the same destination interface with `Pitch` in semitones, `OscillatorGain` in dB, and `PerNote` scope. Its layer implements `ModulationDestinationSettings` for the module's routes; portable instances report their saved ID through `ModulationDestination::modulation_target()`. Ordinary destinations retain the default `None`. `valid_targeted_modulations` validates repeated targets; `prepare_target_routes` prepares each instance's route vector on the control thread. `process_modulation` invokes the common resolver with that destination ID. A mismatched ID is never interpreted as another list position.

Every expanded modulation strip labels its two dropdowns Source and Destination. The labels use the shared 11-point secondary text above each selector, within the existing control rows; compact strips keep their footer-only layout. Envelope Mode reuses the same labeled row and dropdown width, including its secondary caption color and spacing. The UI's `targeted_body` accepts parameter/ID/label choices. The original `body` is an adapter with no instance IDs, so all destinations still use one connection widget. Parameter indicators use the same marker builder with the relevant instance's routes. `retarget_modulation_routes` moves route history by route and target IDs; Oscillator, Sampler, Amplitude, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, and LFO use this helper during compatible renderer updates. Each route vector is processed once per destination sample. Do not pass one shared mutable ramp vector successively to multiple instances.

`Oscillator::from_settings` installs its instance identity. `SourceUnit::set_frequency` sets its unmodulated frequency; `process_modulation` generates the next mono sample (there is no input audio). Standalone callers using `Oscillator::new` retain an unscoped destination and the unmodulated `SourceUnit::next_sample` API. Gain uses the same stable target and shared typed entry point, with signed dB offsets and processed extrema. See [pitch/gain behavior, limits, and migration](oscillator-pitch.md).

## Destination-matched amount knobs

All destination-owned modulation amount strips use `parameter_knobs.rs`. Its `destination` function is the normal knob definition used by Sampler, Amplitude, Auto-filter, Drive, Saturator, 3-band EQ, Compressor, Limiter, OTT, Direction Mixer, Stereo Spread, and Bitcrusher; Pitch returns the same `knobs::pitch_spec` used by oscillators. Its `amount` function derives the signed amount control from that definition. New targets must register both their destination definition and their amount-unit mapping here. Do not create an unrelated generic amount knob in a unit panel.

Amounts retain their existing stored units, bounds, and routing meaning. Zero remains neutral and double-click resets to zero. Endpoint labels and readouts show amount units (Envelope Time shows the corresponding positive multiplier), so a Frequency amount never pretends to be an absolute Hz setting. Idle rendering does not round saved fractional values.

| Target | Saved amount range | Shared style and interaction |
| --- | --- | --- |
| Envelope Attack / Decay / Release Time | ±6 logarithmic octaves; displayed ×1/64 to ×64 | Reuse the shared octave adapter and native time scale without changing interaction. Display `2^amount` in readouts, tooltips, and assignment markers; zero/reset displays ×1. |
| Saturator Drive | ±36 dB | Reuse the centered −36..+36 dB input knob, 6 dB reference marks, 0.1 dB steps, and neutral-zero reset. |
| Limiter Input / Ceiling | ±24 dB | Reuse the native linear dB scales and 0.1 dB steps; Input retains its 6 dB reference marks. |
| Limiter Release | ±6 octaves of duration | Reuse the native logarithmic duration scale through the shared octave adapter, with 1/24-octave keyboard steps. |
| Limiter Lookahead | ±10 ms | Reuse the native linear millisecond scale and 0.1 ms steps, with neutral zero. |
| Compressor Threshold / Knee | ±60 / ±24 dB | Reuse their native linear dB scales and 0.1 dB steps, mirrored around neutral zero. |
| Compressor Ratio | ±19 ratio units | Reuse the 1:1–20:1 linear scale and 0.1 steps, with magnitude mapped from its 1:1 origin. |
| Compressor Attack / Release | ±6 octaves of duration | Use the shared octave adapter over the native logarithmic time scales; a native key step is 1/24 octave. |
| EQ band Gain | ±24 dB | Reuse the centered −24 to +24 dB destination scale, 6 dB reference marks, 0.1 dB steps, and neutral-zero reset. |
| EQ Low/High Frequency / Q | ±6 octaves / ±10 Q | Reuse the native logarithmic frequency/Q definitions and shared octave/signed-Q amount adapters. |
| Pitch | ±48 semitones | Same one-semitone steps and octave marks as Pitch. Existing fractional values remain visible until edited. |
| Frequency / Color frequency | ±6 / ±3 octaves | Derive the octave coordinate from the native logarithmic frequency scale. The native frequency keyboard operation becomes 1/24 octave per key press. |
| Resonance | ±10 Q | Mirror the native logarithmic Q curve around neutral; zero magnitude starts at the native 0.1 Q minimum. Reuse the native multiplicative keyboard operation. |
| Oscillator Gain / Sampler Gain / Level / Volume | ±24 dB | Mirror the native gain attenuation curve and gain reference marks, retaining 0.1 dB keyboard steps. Bounds stay finite rather than adding an infinity amount. |
| Amplitude | ±1 envelope depth | Reuse the gain curve in depth coordinates, displayed as ±100%. A magnitude `d` maps to native attenuation `20 × log10(1 − d)`; full depth maps to exact silence on that control scale. This is a UI mapping, not a change to envelope routing. |
| Dry/Wet / Drive Gain / Tone / Color strength / LFO Depth | ±1 normalized offset | Reuse the native percentage profile and one-percentage-point keyboard steps. Display signed percentages and ±100 endpoint labels. |

The adapter in `knobs/amount.rs` uses the destination's scale and keyboard function. It adjusts normalized drag travel so the same pointer distance covers the same native-scale distance, including the shared tenfold Shift slowdown. Native gain marks are mapped and mirrored into the signed amount domain, omitting duplicate and out-of-range marks. The base `KnobSpec::value` applies inherited discrete steps after amount conversion as well, so bounded stepped destinations such as Bitcrusher Resolution/Downsampling retain whole amount steps. Idle fractional saved amounts remain untouched until interaction. Painting, focus, disabled behavior, drag accumulation, bounds, keyboard input, reset, fonts, and theme colors remain in the base `Knob` widget.

These are UI definitions. The portable modulation protocol, source publication, signed amounts, smoothing, validation, saved project fields, and audio processing remain unchanged.

## Component transfers

[Moving and copying components](../editing/components.md) uses the same stable source/target identities. Provider moves update all instrument and track-effect references in one core transaction, assigning a new provider ID only if the destination already owns that ID. Destination moves carry their routes. Destination copies receive new route/target IDs and retain source assignments; provider copies receive new IDs without taking over existing consumers. The shared modulation resolver and DSP are unchanged.

All destination-owned modulation strips also use the same reorder gesture. Reordering changes list order only; route IDs, source assignments, target IDs, amounts, power, and compatible DSP history remain attached to their connection. See [strip reordering](../editing/components.md#reordering-strips).

## Providers that also consume modulation

LFO uses both shared interfaces. The host first calls `process_modulation` with its prepared route vector, then publishes the resulting source sample through `ModulationOutput`. The input step consumes no audio (`Audio = ()`); it prepares smoothed control offsets for the next source sample. Destinations keep reading cached values and never advance a provider.

`modulation_dependency_order` prepares a provider-first order on the control thread. `modulation_would_cycle` applies the same edge definition to source pickers. All assigned edges count, including disabled, zero-amount, and retained connections; edits cannot enable a hidden cycle. Project validation checks sources in the same instrument: Per note Envelope destinations accept local/shared sources; shared providers accept shared sources only. Prepared provider-first indices interleave shared Envelopes, LFOs, and Macros; local Envelopes follow their own prepared dependency order after shared publication. Audio destinations then consume the completed cache. Ordering, route vectors, telemetry, and cache storage are prepared before publication. There is no callback graph traversal, allocation, lock, or recursive source sampling.

LFO Depth uses a normalized ±1 offset; Rate uses ±6 octaves per connection in both Hz and Note modes. The source's output behavior remains `Continuous`. UI parameter definitions, amount adapters, source selectors, strip reordering, indicators, enable ramps, and compatible route retargeting remain shared. LFO routes travel with component moves/copies, copied route IDs are fresh, and provider moves update LFO consumers as well as audio destinations. See [LFO timing and rate-history limits](../instruments/lfo.md#depth-and-rate-modulation).


## Multiple Macro strips

[Macro](../instruments/macro.md) groups several stable-ID shared providers in one component. `MacroModuleSettings` reuses targeted routes and the shared destination interface for `MacroValue`. Each strip publishes a unipolar continuous signal. `provider_destinations()` enumerates both LFO and Macro nodes. The graph and source picker filter incoming edges by each target ID, so independent strips do not acquire one another's connections. `targeted_body_with_sources` reuses the common connection controls with target-specific valid source choices; ordinary destinations remain adapters over that function. Macro uses the shared 1 ms offset smoother and exported `ModulationRamp` for its 5 ms base-value transition.

The host's `continuous_modulation.rs` constructs concrete LFO/Macro processors and shares their route processing, output publication, reset, and identity-based retargeting. Each destination still calls the portable `process_modulation` entry point; no consumer looks up or advances a Macro directly. Macro Value amounts retain signed ±100% limits and the Value knob's percentage scale/steps. Module and strip power use the existing neutral source contribution weight.

Macro strip names are published through the optional `ModulationSourceSettings::modulation_name` metadata and `ModulationSourceInfo::display_name`. Missing names use the provider type and ordinal. Shared source selectors and Macro Value destination labels show the saved name; route IDs and DSP remain independent of display names.

## Three-band EQ destination

`EqSettings` and `Equalizer` declare the same seven parameters through `ModulationDestinationSettings` and `ModulationDestination`. The `EQUALIZER` parameter list uses fixed band identities: Low Gain/Frequency/Q, Mid Gain, High Gain/Frequency/Q. The common typed input frame has separate slots for these identities; no route targets a band by display position. Source publication, scope checks, route sums and ramps, the offset smoother, and identity-based route retargeting stay shared. EQ owns its shelf/cut coefficient updates and final gain/frequency/Q bounds.

All destination snapshots now reserve 19 processed readings; unused slots are marked unavailable. EQ uses its first seven; the existing five-reading engine/output accessors remain adapters over the same snapshot. Handles and publication storage are prepared before callback use, retained by stable destination identity, and read without advancing a provider or DSP. See [EQ modulation](../effects/equalizer.md#modulation--2026-10-17) for controls, units, persistence, and monitoring.

## Compressor destination

`CompressorSettings` and `Compressor` declare the same seven shared targets in `COMPRESSOR`: Threshold, Ratio, Attack, Release, Knee, Level, Dry/Wet. Level and Dry/Wet reuse the existing typed parameters and amount definitions. Five distinct parameter identities add threshold, ratio, duration, and knee inputs to the common stack-owned resolver frame. Compressor owns its final bounds, slope/log-time/gain conversions, detector/envelope DSP, and processed-value reporting. Shared source publication, route resolution/ramping, offset smoothing, UI strips and indicators, history retargeting, and processed-value snapshots remain common code. See [Compressor modulation](../effects/compressor.md#modulation--2026-10-17) for units, ranges, persistence, and callback constraints.


## Sampler Gain destination

`SamplerSettings` and `Sampler` declare one typed `SamplerGain` parameter in PerNote scope. The shared input frame owns its additive dB slot. Sampler reuses cached publication, scope checks, route validation/resolution, the 1 ms offset smoother, 5 ms route/source ramps, and identity-based route retargeting. The portable unit owns final gain conversion, the +12 dB ceiling, exact base silence, and existing source/power/boundary behavior. Gain does not own note release.

The UI reuses the same connection body, header dot, Gain-derived signed amount knob, and indicators. Existing Sampler snapshots keep active position in slot 0 and publish lowest/highest active gain dB in slots 1/2, excluding velocity, boundary fades, and Amplitude. See [Sampler Gain modulation](../instruments/sampler.md#gain-modulation--2026-10-17) for persistence, bounds, and compatible updates.


## Limiter destination

`LimiterSettings` and `Limiter` expose Input, Release, Lookahead, and Ceiling through the same destination traits in Shared scope. Four typed slots in the common input frame keep dB, octave, and millisecond offsets distinct. Cached source publication, scope validation, signed route sums/ramps, the additive smoother, connection strips, amount adapters, indicators, and route retargeting remain shared code. Limiter owns final parameter bounds, peak-tree queries, linked stereo/M/S protection, and gain-reduction monitoring. Mode/Routing remain discrete; Lookahead modulation does not change fixed latency. Snapshots publish four processed values in LIMITER order, with unused slots NaN. See [Limiter modulation](../effects/limiter.md#modulation--2026-10-17) for saved defaults, units, and existing ceiling/reconstruction limits.


## OTT destination

`OttSettings` and `Ott` declare 19 shared targets in `OTT`: two crossovers, five controls per fixed band, Level, and Dry/Wet. Level/Dry/Wet reuse existing identities; 17 new parameter slots extend the common stack-owned input frame. OTT uses the common resolver, smoother, source metadata, route-history retention, destination-matched amount adapters, connection strips, and indicators. The portable processor owns final bounds and constrained crossover ordering. Snapshot capacity is now 19; existing destinations retain their prefix layouts and pad unused readings with NaN. See [OTT modulation](../effects/ott.md#modulation--2026-10-17) for units, order, persistence, bypass, and meter behavior.

## Direction Mixer destinations

[Direction Mixer](../effects/direction-mixer.md#modulation) uses Shared Direction and Spread destinations through the common contract. Amounts are degrees (±180) and spread control units (±2). Apply the portable input-specific Spread gain curve after the shared additive offsets and final bounds; an L/R value above unity reduces Mid rather than increasing Side. Input format stays discrete. Typed offset storage appends two slots after the existing 47, preserving prior identities and indices. Base controls and signed Amount controls share destination specifications; range and processed-value indicators use the same final bounds as DSP. Prepared snapshots expose the two effective native values without advancing processing.


## Stereo Spread destinations

[Stereo Spread](../effects/stereo-spread.md#shared-modulation) declares four Shared destinations: Lower/Upper Intensity and Lower/Upper Frequency. Intensity amounts are ±100 percentage points; the bipolar base ranges are −100% to +100% with zero defaults. Frequency amounts are ±6 octaves. Add four typed slots after the existing 49, for 53 total, preserving earlier indices. Reuse source metadata/discovery/publication, route validation and cached resolution, destination-derived amount controls, the 1 ms offset smoother, prepared snapshots, and history retargeting. The portable processor owns constrained frequencies and band DSP; Order stays discrete. Snapshots publish actual Intensities then Frequencies in the declared order, with remaining readings unavailable.


## Bitcrusher destinations

[Bitcrusher](../effects/bitcrusher.md#shared-modulation) declares five Shared destinations: Drive, Resolution, Downsampling, Level, and Dry/Wet. Three typed targets append slots 53–55 for 56 total; Level/DryWet reuse their existing identities. Amounts are ±24 dB, ±23 bits, ±39 ratio steps, ±24 dB, and ±100 percentage points. Resolve all routes through the same cached source/destination entry point, sums/ramps, scope checks, and offset smoother. The processor owns final bounds, rounding Resolution/Downsampling to whole values, quantization, and capture timing. Reuse destination-derived native amount adapters, connection controls, rounded range indicators, prepared five-value snapshots with unused NaNs, and stable-ID retargeting. Base fields use integer Resolution/Downsampling; route amounts may remain fractional before final DSP rounding.

## Prepared lookup and history matching

Hosts publish values with matching `ModulationSourceIndex` generations. Shared routes cache validated local/shared slots and missing-source results. Local precedence, dynamic range/behavior, contour release ownership, scope checks, and smoothers stay in the common resolver. Legacy slice-only calls retain checked cached hits. Compatible history retargeting uses preallocated route scratch links for hashed identity matching, with a bounded direct path for small lists; incoming summation order stays unchanged. See [performance requirements](../architecture/performance.md#modulation-and-inheritance).

## Envelope destinations

[Envelope inputs](../instruments/envelope.md) add nine stable typed slots (56–64) for Time ADSR durations, Sustain, curve strengths, and Note Attack/Decay fractions. Envelope uses both shared interfaces and the common provider input/publication helper, route resolver, offset smoother, graph ordering, scope/cycle checks, controls, amount adapters, history retargeting, and prepared telemetry. Note scheduling remains in the host; parameter bounds and fractional stage progress remain in the portable unit. Native history replay runs on the existing cancellable preparation worker.

## Saturator destination

`SaturatorSettings` and the portable `Saturator` declare Shared `SATURATOR` inputs: SaturatorDrive, Level, DryWet. Drive appends typed slot 66; Level and blend reuse the existing typed slots. All routes reuse provider metadata/publication, source scope validation, cached resolution, route ramps, the additive smoother, history retargeting, destination-derived amount controls, markers, and processed snapshots. The portable processor owns final drive/gain/blend bounds and the fixed-latency oversampled DSP. Shape/Clip selectors remain discrete. See [Saturator](../effects/saturator.md).

Filter uses the existing Frequency/Resonance/Level/Dry-Wet destination list and Shared scope, with the same routing, validation, amount controls and smoothing as Auto-filter. It has no internal LFO. See [Filter](../effects/filter.md).

## Granulizer destinations

[Granulizer](../instruments/granulizer.md) extends the shared sample-source facade with eight granular destinations, while plain Sampler still validates Gain only. Both use the same PerNote contract, prepared route vector, additive smoother, source lookup and route-history matching. Granular snapshots add effective values and active grain positions through the existing prepared telemetry channel.

## Oscillator unit Level destination

Oscillator Level reuses the typed Level slot and PerNote scope after summing sources. Unit Pitch reuses the typed Pitch slot to transpose all sources before summing. Their untargeted routes share the layer-owned vector with instance-targeted Pitch/Gain routes. `valid_mixed_modulations` checks parameter/target ownership and unique route IDs across both groups. Prepared route vectors keep one advancement per destination/sample. The portable output stage owns final gain bounds and uses the same offset smoother; the host retains prepared routing, compatible history, and processed extrema. See [Oscillator Level](../instruments/midi-patterns.md#oscillator-level-modulation--2026-10-23).

## Drive v2 destination

[Drive v2](../effects/drive-v2.md) declares seven Shared inputs through the common settings/runtime contract. DriveInput appends additive slot 75; six other inputs reuse existing typed identities. Input/output offsets use the shared Gain amount profile, final +12 dB bound, and exact base silence. The processor composes the original Color/clipping/Tone kernel with input gain, dry/wet, output level and unit bypass. Reuse cached publication, source scope checks, typed resolution, one-millisecond offset smoothing, route-history matching and prepared seven-value snapshots. Discrete Color power, Shape and Clip have no routes.
