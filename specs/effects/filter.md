# Filter

For user instructions, see [Filter](../../docs/effects/filter.md).

## State and registration

Register Filter once in `effects_catalog::ENTRIES` under EQ & Filters with type
ID `daw.filter`. `unit-core::FilterSettings` stores version 1, enabled, collapsed,
modulation visibility, Shape, Frequency, Q, Level, Dry/Wet and modulation routes.
It stores no internal LFO fields. Defaults match Auto-filter's Cutoff/Out controls:
Low-pass, 1000 Hz, Q 1/√2, 0 dB, 100% wet, enabled, both presentation flags false
and an empty route list. Validate frequency 20–20,000 Hz, Q 0.1–10, Level from
`SILENT_GAIN_DB` through +12 dB and wet 0–1, all finite. Reject invalid settings or
routes atomically through the existing track/layer/Master effect transactions.
Preserve unknown future versions as opaque unavailable effects. Copies receive
new effect and route IDs; moves and compatible updates preserve identity.

## Shared processing

`daw-filter` is the portable stereo processor. Its FunDSP Simper coefficients,
independent channel integrators, four response weights, logarithmic frequency
ramp, Q/Level/wet/bypass ramps, modulation smoother and telemetry are shared by
Filter and Auto-filter. Auto-filter composes this processor with its existing
LFO and transport clock. The static processor owns no LFO or transport state.
The `process_with_cutoff_offsets` composition entry point accepts Auto-filter's
smoothed per-channel octave sweep; ordinary Filter processing supplies zero.
FunDSP types remain private. No UI, DAW, device or media dependency is introduced.

Keep Auto-filter's existing audio arithmetic and behavior. Low/High-pass slopes
are 12 dB/octave; Band-pass is normalized at its center, and Notch rejects the
center. Clamp processed cutoff to 20 Hz through `min(20 kHz, rate × 0.45)`.
Dry/Wet blends dry and filtered input linearly; Level follows that blend. Header
off passes unscaled dry input while retaining processing history. Parameter,
shape and bypass changes ramp over 5 ms. External offsets use the common 1 ms
smoother. Processing adds zero latency. Reset clears integrators, snaps ramps to
their targets and resets modulation smoothing. Pause, gaps, loops, seeks,
compatible publication and reordering follow the shared effect lifecycle.

Filter implements the [shared modulation contract](../modulation/contract.md)
with the existing `ModulationParameter::FILTER` list and Shared scope: Frequency
(±6 octaves), Resonance (±10 Q), Level (±24 dB), Dry/Wet (±1). Reuse typed route
resolution, source inventory, source removal/duplication handling, route identity
retargeting and destination smoothing. Final bounds remain in the portable unit.
Use the existing five-value cutoff L/R, Q, Level and wet snapshot layout. Filter's
Frequency UI shows one processed marker; no internal-sweep range is added.
Construction and preparation occur outside the callback. Processing, reset,
compatible retargeting and route updates allocate nothing and perform no I/O.

## Presentation and hosts

Reuse Auto-filter's Cutoff/Out rows, response diagrams, parameter knobs and amount
adapters through the shared panel renderer. Omit the entire middle LFO section.
Each remaining section is one standard 90-point strip, with three aligned rows,
equal height and a full-height shared divider. Cutoff contains Shape/Frequency/Q;
Out contains Level/Dry-Wet and an empty third row. Footers read Cutoff and Out.
Compact mode retains these labels. Reuse header power, collapse, modulation dot,
connection strips, selection, reorder/copy/delete and focus guards. A temporary
Auto-filter control view adapts common fields without cloning routes; its LFO
fields are never displayed, persisted or passed to Filter processing.

Use the same `EffectChain` runtime on tracks, Master, regular instrument layers
and Output. Include the new type in prepared-state fingerprints and memory
accounting. Reuse playback/export/seek/loop processing and existing publication
transactions. All chains retain their placement, source-scope and identity rules.

## Verification

Check all project rates and shapes; static parity with LFO-disabled Auto-filter;
old/new Auto-filter audio and telemetry parity; live retarget/reset and callback
allocation; shared Envelope/LFO/Macro modulation with playback/export/block/seek/
loop parity; state/route versions, invalid edits, duplication and save/reopen;
track/Master/layer/Output placement; both-theme two-strip controls and compact mode.
Record native interaction, listening and Windows/Linux checks separately in
[validation history](../validation/README.md).

## Standard output monitoring

Use the [standard effect Out-strip meter](../architecture/modular-audio-units.md#standard-effect-out-strip-meter) for final linked-stereo level, shared held-peak capture/reset, bypass masking, and allocation-free prepared meter binding. Retain audio DSP, latency, saved settings and existing modulation behavior.
