# Drive v2

Implementation requirements. User instructions: [Drive v2](../../docs/effects/drive-v2.md).

## Identity and ownership

Register Drive v2 once in `effects_catalog.rs::ENTRIES`, under Distortion, with `EffectInstance::drive_v2` and stable type ID `daw.drive-v2`. Retain `daw.drive`, its version-1 state, constructor, controls, and sound. Version-1 `DriveV2Settings` lives in unit-core; unknown future versions use the existing opaque missing-effect fallback. Common effect settings methods own route access, validation, memory estimates and duplication. Normal core transactions apply to all track, group, layer, Output and Master chains.

The portable `daw-drive::DriveV2` composes the existing Drive Color/clipping/Tone kernel. Backend filter types remain private in daw-drive. No new crate or dependency is required. A private clip-blend kernel keeps the original soft path and existing Drive entry points unchanged.

## Layout

Three standard 90-point strips share the existing effect shell, panel margins, compact height, row spacing and full-height dividers:

1. Drive: bounded two-way Clip mode knob (Soft/Hard), Drive Gain, Drive Tone.
2. Color: three-way Shape, Strength, Frequency; standard shared Power/Color footer.
3. Output: Input Gain, Output Level, Dry/Wet.

Use shared Knob/KnobSpec and destination amount profiles. Shape uses three response diagrams (Mid scoop, Mid hump, Mid hump + bass cut); Off is a separate saved power state. Disable all Color controls while off, retaining values. Clip uses the shared bounded `KnobSpec::two_way_labels` design: Soft/Hard upper-corner labels, active accent color, neutral arc, no markers or accent value arc, Soft reset. Retain standard pointer/keyboard steps, Shift behavior, double-click reset, focus, accessibility, and history. Shared header power, horizontal collapse, compact layout, selection, drag/drop, history and modulation UI remain applicable.

## Processing

Input Gain → split dry/wet. Wet: Color → selected clipping curve → Drive Tone. Mix with the input-gained dry path using Dry/Wet, then apply Output Level. Whole-unit bypass blends back to the original input. Use exact endpoints for dry/wet and bypass; 0% wet retains Input Gain. Soft/Hard selects the distortion curve, not a separate final clipper.

Retain legacy color filters, 250 Hz–2 kHz logarithmic Frequency, 0–100% Strength, normalized Drive Gain and Tone, and approximately 36 dB maximum drive. Color starts off with Mid scoop selected. Gain starts at 20%, Tone/Strength at 50%, Frequency at 750 Hz, input/output at 0 dB, Dry/Wet at 100%, Clip at Soft. Input/output gain use exact silence through +12 dB.

Soft is the original normalized `tanh(x*a)/tanh(a)`, with `a=10^(1.8*Gain)-1`; at `a<1e-4`, pass the colored input. Hard uses `clamp(x*a,-1,1)/min(a,1)` with the same clean case. Blend between curves before Tone over five milliseconds. Retain filter history during mode/power/parameter edits. Inactive Color branches continue processing. All base controls and power transitions use five-millisecond ramps; gain ramps in linear amplitude and Frequency logarithmically. Neutral v2 Input Gain and 100% wet with Soft must reproduce the existing Drive processor exactly for matching Color/Gain/Tone settings and matching Output Level/legacy Volume, including non-unity and silent output levels. Preserve the legacy finishing arithmetic at full wet to avoid floating-point differences. No new latency, oversampling or tail behavior is introduced.

## Shared modulation and lifecycle

Declare Shared destinations in `ModulationParameter::DRIVE_V2`: DriveInput, DriveGain, Tone, ColorStrength, ColorFrequency, Level, DryWet. Append DriveInput at stable typed slot 75 (76 additive slots total); reuse all existing parameter IDs and limits. Input/output amounts span ±24 dB; intensity, tone, strength and mix ±1; Frequency ±3 octaves. Gain bounds match the base controls; exact base silence remains silent. Color power, Shape and Clip are discrete.

Use shared saved route validation, source discovery/publication, cached resolution, typed processing, one-millisecond offset smoothing, five-millisecond route/source ramps, compatible route-history matching, marker/amount controls and prepared snapshots. The outer stage smooths input/output/blend offsets; the existing kernel smooths Gain/Tone/Strength/Frequency offsets once. Do not double-smooth offsets. Seven processed telemetry readings follow the declared order, with input/output in dB and Frequency in Hz. Off Color masks corresponding markers while routing/filter histories continue. No provider is evaluated in the UI.

The existing engine prepares every route and processor outside the callback; reset, sampling and compatible retargeting allocate nothing and do not block, log or perform I/O. Composition uses stack-only temporary legacy settings with empty route storage when retargeting. Header bypass preserves the unprocessed input. Standard shared chain handling supplies reset/seek, loop, snapshot cloning, memory accounting, retirement and export parity.

## Separate plugins

[Drive v2 plugin builds](../architecture/drive-v2-plugin-builds.md) define independent CLAP/VST3 identity, host parameters, state, shared editor, packaging and validation limits over this processor. The original Drive product remains separate.
