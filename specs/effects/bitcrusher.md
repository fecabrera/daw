# Bitcrusher

Implementation requirements and acceptance criteria. For operation, see the [user guide](../../docs/effects/bitcrusher.md).

## Ownership and integration

`crates/unit-core` owns `BitcrusherSettings`, version-1 `daw.bitcrusher` state, and typed destinations. `crates/bitcrusher` owns quantization, the stereo sample-hold clock, parameter conversion, smoothing, and final bounds. Its only dependency is `unit-core`; it has no DAW, UI, media, device, or external DSP dependency. The host constructs it for the existing serial track/Master paths. Playback, offline export, random-access rendering, and CLI use the same processor.

Append one registration to `effects_catalog.rs::ENTRIES` without changing prior indices. Place it after Drive in Distortion through category metadata. Reuse collapsed folder defaults, catalog selection/navigation/drag behavior, installed selection, copy/reorder/delete, stable IDs, and common transactional edits. Do not create instances during previews. Main supplies modulation rather than another effect installation target.

## Controls and persistence

| Field | Bounds | Default |
| --- | --- | --- |
| `drive_db` | Finite 0–24 dB | 0 |
| `resolution_bits` | Integer `u8`, 1–24 | 24 |
| `downsampling` | Integer `u8`, 1–40 | 1 |
| `level_db` | Finite `SILENT_GAIN_DB` to +12 dB | 0 |
| `dry_wet` | Finite 0–1 | 1 |
| `enabled`, `collapsed` | Independent power/presentation | true, false |
| `modulation_open`, `modulations` | Optional serde defaults; explicit null is invalid | false, empty |

Use the shared `EffectSettings` envelope and fresh `EffectInstance` UUID. Supported malformed data fails; future versions retain opaque payloads as missing bypassed units. Fractional saved Resolution/Downsampling values fail deserialization. Validate final fields/routes atomically through model transactions. Existing projects without the unit remain unchanged. Preserve parameters, routes, visibility, and collapse through save/open, Save As, duplication, reorder/copy, and rate changes. Shared transfer code retains/remaps provider and route identities within the owning instrument.

Reuse `PanelStyle::unit_in_lane` with two 90-point strips: Crush contains Drive, Resolution, Downsampling; Output contains Level and Dry/Wet. Reserve its third row so centered footers align. Use shared margins, full-height dividers, knob rows, power/collapse, modulation dot/body, compact heights, selection, and tooltips/accessibility. No audio meter is added.

Every knob derives from `parameter_knobs::destination`. Drive uses a dB knob with 0.1 dB keyboard steps and reset 0. Resolution and Downsampling use `KnobSpec::stepped(1.0)`, whole-number readouts, keyboard steps of one, and resets 24/1. Shared Shift sensitivity retains quantization. Level/DryWet reuse their existing definitions, amount adapters, and gain/percentage formats. Use the same destination-derived controls for all five modulation amounts; never create a separate generic amount scale.

## DSP

Split finite stereo input into dry and wet paths. Sanitize nonfinite input samples to zero. The wet path is Drive gain, hard clipping to ±1, amplitude quantization, then sample hold. Mix dry and wet linearly, apply final Level, and blend that result with dry through whole-unit power. Level therefore applies to both paths when enabled; whole-unit bypass excludes Level.

For bit depth `b`, define `n = 2^b - 1`. Quantize the driven, clipped sample magnitude to endpoint-inclusive levels:

```text
q(0) = 0
q(x) = sign(x) * (2 * round((abs(x) + 1) * n / 2) / n - 1), x != 0
```

Use f64 intermediates, round halfway magnitudes upward, and preserve sign. Nonzero input selects from `2^b` symmetric levels between −1 and +1; the explicit zero exception preserves digital silence. One-bit nonzero output is ±1. No dither, anti-alias filter, oversampling, alternate clipping mode, or automatic gain compensation is added. Default 24-bit processing still quantizes and clips headroom; it is not an exact dry shortcut. There is no alignment latency or appended export tail.

One `u8` elapsed counter and two f64 held values implement the shared stereo capture clock. Initialize/reset elapsed to `u8::MAX` so the first input captures immediately. Before each sample, capture both processed channels if elapsed is at least the current whole ratio, reset elapsed to zero, then increment it. The held pair drives the wet path until the next capture. At a fixed ratio `N`, captures occur at 0, N, 2N, and so on. A changed ratio keeps elapsed time; shortening a period captures immediately if the elapsed time meets it. Resolution/Drive affect the next capture. Dry audio is never held.

For wet amount `m`, Level amplitude `g`, power weight `e`, and held sample `h`, compute `wet = ((1-m)*dry + m*h)*g`, then `output = (1-e)*dry + e*wet`. Use these weighted sums to retain bounded wet values when dry headroom is extreme; subtractive interpolation can lose the clipped result through cancellation. Settled power-off returns finite dry samples exactly, including subnormal/headroom values. Dry/Wet zero with unity Level is also exact dry. Keep f64 mixing and clamp final sums to the f32 range. The internal hard clip does not clamp the dry path. Quantization/hold can generate discontinuities by design.

## Smoothing and runtime

Use six shared `ModulationRamp` values: Drive dB, Resolution, Downsampling, Level amplitude, Dry/Wet, whole-unit power. Base changes ramp over 5 ms at every processing rate. Unchanged/presentation edits do not restart ramps. Resolution and Downsampling round after base ramps and offsets, so intermediate effective settings remain whole values. Preserve capture phase, held audio, in-flight ramps, and the shared offset smoother on compatible renderer updates. Do not interpolate held samples or fractional bit depths/ratios.

`ModulationSmoother<5>` supplies shared 1 ms additive smoothing and nonfinite-neutral handling. Add Drive dB then clamp 0–24. Add Resolution/Downsampling offsets, round nearest with halfway values away from zero, then clamp 1–24/1–40. Multiply the ramped Level amplitude by `10^(clamp(offset,-96,96)/20)`, cap it at +12 dB amplitude, and retain exact base silence. Add Dry/Wet offsets and clamp 0–1.

Continue control ramps, offsets, and capture timing through bypass. Stop/seek clear held samples, reset ramps to current targets, clear offsets, and capture the next input immediately. Pause retains state. Track mute, gaps, and loop boundaries follow existing effect-chain processing. The unit has no note-release ownership.

Construction and route storage stay on the control thread. Sampling, reset, live retargeting, telemetry, and transport must not allocate, lock, log, perform file I/O, or retire heap owners in the callback.

## Shared modulation

`ModulationParameter::BITCRUSHER` is Drive, Resolution, Downsampling, Level, Dry/Wet. Settings and processor implement the shared destination traits in Shared scope. Add typed Drive/Resolution/Downsampling slots 53–55 after the existing 53 slots, for 56 total. Reuse existing Level and DryWet slots. Preserve earlier identities/indices.

| Destination | Signed amount | Final native reading |
| --- | --- | --- |
| Drive | ±24 dB | 0–24 dB |
| Resolution | ±23 bits | Whole 1–24 bits |
| Downsampling | ±39 ratio units | Whole 1–40× |
| Level | ±24 dB | Actual gain in dB or exact silence sentinel |
| Dry/Wet | ±1 normalized | 0–1 |

Reuse metadata/discovery/publication, same-instrument scope checks, cached provider fan-out, typed route validation/resolution, additive sums, route/source ramps, and identity-based history retargeting. Support shared Retrigger/Legato Envelope, LFO, and Macro sources, including Main. Reject Per note and foreign references. Missing/unavailable/unassigned/disabled sources contribute zero. Do not add a consumer-specific source lookup or routing loop.

Reuse Source/Destination/Amount strips, destination-derived signed native amount mappings, connection power/reorder/remove, range arcs, and moving dots. Resolution/Downsampling amount controls also step by one; stored fractional amounts can remain valid and are rounded only after summation/smoothing in DSP. Their amount adapters use native origin 1 and direction +1. Drive amounts use native origin 0 and direction +1; Level/DryWet retain their common mappings. Range endpoints use final rounded/clamped rules. Effective snapshots expose five values in BITCRUSHER order; the remaining slots of the existing 19-value capacity stay NaN. UI must not run another processor or provider for telemetry.

## Acceptance and provenance

The implementation is independently written from the agreed design and conventional gain, quantization, sample-hold, and mixing equations. Apple's public [Bitcrusher description](https://support.apple.com/en-gb/guide/logicpro/lgcef1a8f27c/mac) informed the earlier control discussion and sample-rate ratio meaning. It does not specify this quantizer or transition law. No exact Logic audio equivalence is claimed. No external source code or private media is a fixture.

Check all six rates and 1–24 depths: silence, endpoint clipping, known low-bit levels, symmetry, quantization error, Drive-before-clip and gain-after-blend, every 1–40 hold period, independent channels on one clock, intentional aliasing, reset, finite headroom, nonfinite-neutral input/offsets, exact bypass/dry mix, and base silence. Verify whole modulated settings, signed sums/cancellation, final bounds, base/modulated agreement, same-instrument shared scope, and Per note rejection.

Verify track/Master and mono/stereo/live/export/block/random-access/seek/loop parity, provider kinds, stable capture history through presentation updates, compatible parameter edits, and allocation-free callback paths. Check malformed edits/storage, future missing payloads, save/reopen/Save As/duplication/rate changes, discovery/binding/snapshots, and unused NaN slots. UI checks cover both themes, track/Master/Instrument catalog additions, whole keyboard/pointer steps, resets, shared amount maps, geometry, power/compact/collapse/removal, source markers, and connection opening. Record executed commands/results and native listening/device/platform limits in [validation history](../validation/README.md).
