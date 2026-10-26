# Saturator

Implemented on 2026-10-19 from the accepted five-control proposal. User instructions are in the [Saturator guide](../../docs/effects/saturator.md).

## Controls and layout

Use two standard 90-point strips under the shared unit header, with connection strips added on the right when modulation is open. User-requested dropdowns take precedence over the usual shape-knob convention.

| Strip | Row 1 | Row 2 | Row 3 | Footer |
| --- | --- | --- | --- | --- |
| Saturation | Shape dropdown | Clip Mode dropdown | Drive knob | Saturation |
| Out | Level knob | Dry/Wet knob | Empty | Out |

Dropdowns reuse the labeled rows, captions, widths, fonts, and interaction used by the shared modulation panel. Knobs reuse destination definitions and the shared Knob widget. Expanded/collapsed and compact presentation, header power, selection outlines, source indicators, and connection controls follow existing effect units.

| Control | Range/options | Default | Meaning |
| --- | --- | --- | --- |
| Shape | Analog Clip, Soft Sine, Bass Shaper, Medium Curve, Hard Curve | Analog Clip | Primary nonlinear transfer curve. |
| Clip Mode | No Clip, Soft Clip, Hard Clip | No Clip | Final clipping on the processed path after the selected Shape. |
| Drive | −36 to +36 dB | 0 dB | Input gain before shaping; negative values reduce saturation. |
| Level | Exact silence to +12 dB | 0 dB | Processed-path gain after the final clip stage. |
| Dry/Wet | 0–100% | 100% | Linear balance of original and processed audio. |

The two selectors remain discrete and unmodulated. Drive, Level, and Dry/Wet accept shared modulation using the common source/destination contract. Amounts are ±36 dB for Drive, ±24 dB for Level, and ±100 percentage points for Dry/Wet. Reuse the native destination profile for each amount knob, existing route sums/ramps, additive smoothing, processed telemetry, markers, and range arcs.

## Sound and signal flow

```text
input ──┬── Drive → Shape → Clip Mode → Level ──┬── Dry/Wet → output
        └──────── original, latency-aligned ───┘
```

- Analog Clip: linear center with a rounded transition into clipping.
- Soft Sine: a wide sine-derived arc for gentle compression, without wavefolding.
- Bass Shaper: a broader, smoother knee tuned for low-frequency material. Use fixed tuning for this control set.
- Medium Curve: earlier cubic compression and stronger third-harmonic content than Soft Sine.
- Hard Curve: actual hard clipping at ±1.2 for stronger upper odd harmonics; distinct from the final Hard Clip stage at ±1.

The independent curves have unity small-signal gain, symmetric positive/negative response, and output headroom above ±1 so post-clipping has a distinct effect. Listening and native-device validation remain separate from the automated harmonic checks.

No Clip bypasses only the final clip stage; the selected Shape still saturates. Soft Clip rounds processed peaks; Hard Clip clamps processed samples at ±1 before Level. Soft Clip should have the same ±1 processed-sample ceiling. Level can subsequently raise the processed signal above that ceiling. Dry/Wet blends after Level, and the dry contribution is not clipped by the processed-path clipper. These are sample-peak controls, not a true-peak limiter. At 0% wet, output is the original audio with the fixed processing delay; Level does not affect it.

Do not add automatic gain compensation: Drive and Level have separate, predictable roles. At 0 dB Drive, a hot input can still saturate. Header power uses the shared click-free bypass and meter lifecycle.

Fixed 4× oversampling runs the nonlinear path through independent 129-tap Blackman-windowed sinc interpolation and decimation filters, cutoff 0.1125 cycles per high-rate sample. Normalize coefficients off the callback. Interpolation is polyphase; decimation emits phase zero. The combined group delay is 128 high-rate samples, or exactly 32 project frames, at every project rate. Dry and bypass use the same fixed 32-frame delay. Engine chain latency adds this delay through the existing track/layer/Main/Master alignment and priming path, including when disabled. The shared effect-chain clock also subtracts this delay before downstream processors, including Auto-filter's internal LFO. It uses the same loop-aware helper as bus alignment. Latency is constant across all controls. The transition band attenuates frequencies near project Nyquist; there is no quality switch.

The fixed history arrays are mirrored so FIR taps read contiguous slices without per-tap modulo. Tap order and arithmetic remain unchanged. The extra storage is 162 stereo f64 frames per processor (2,592 bytes), including cloned checkpoints; it is allocated with the processor, never during callback processing.

## Shared implementation and placement

- The portable `daw-saturator` crate runs behind the existing EffectUnit and typed modulation contracts. Keep DSP dependencies private and reuse shared settings, routes, smoothing, telemetry, and effect construction patterns.
- Register the effect in Distortion through the existing effect catalog metadata. Make it available to audio/MIDI tracks, Master, regular instrument layers, and Main through the same installation paths.
- Reuse effect selection, group shortcuts, move/copy, previews, cross-lane transfers, stable instance/route IDs, removal, and saved presentation. Do not add Saturator-specific drag handling.
- Shared instrument providers, including Main, can drive its continuous parameters wherever the existing scope rules allow. Audio tracks and Master retain the existing provider availability rules.
- Smooth numeric changes and bypass over 5 ms. Crossfade shape and clip-mode changes over 5 ms without a latency change. Preserve compatible DSP/route history and held notes when settings change.
- Prepare filters, oversampling buffers, graph replacements, and telemetry off the callback. Processing, retargeting, and reset must not allocate, block, or perform I/O. Live playback and export share the same processor.
- Persist selectors, numeric controls, routes, enabled state, collapsed state, and modulation-open state through existing versioned effect settings. Reject invalid settings as one transaction and retain unsupported-version data through the shared missing-effect path.

## Reference and scope

The reference is [Ableton Live 12 Saturator](https://www.ableton.com/en/live-manual/12/live-audio-effect-reference/#saturator): its primary curve, input drive, separate post-clip stage, output level, and blend informed the accepted proposal. Ableton's Bass Shaper exposes an additional threshold control. Fixed Bass Shaper tuning retains the five accepted controls. This design does not claim identical algorithms or sound.

## Validation contract

Validate distinct shapes, continuity/symmetry, low-frequency behavior, high-drive finite output, final clip ceilings, and dry/processed routing. Measure oversampling alias suppression, latency, channel isolation, and CPU cost at all six project rates. Check dry/bypass alignment, blend endpoints, numeric smoothing, selector transitions, reset/seek/loop behavior, compatible updates, and zero callback allocations.

Check modulation bounds and source scope, saved-route round trips, Save As/reopen, duplicate IDs, layer/Main/track/Master placement, drag/copy previews, and native/headless controls in both themes. Update the user guide only when the effect is implemented. Completed automated checks and remaining native limits are recorded in [validation history](../validation/README.md).

## Transfer functions and runtime details

For signed input `x`, the rounded-knee function is linear to magnitude `k`, then `sign(x) × (k + (c−k) × tanh((abs(x)−k)/(c−k)))`.

| Shape | Independent transfer function |
| --- | --- |
| Analog Clip | Rounded knee with `k=0.75`, `c=1.5`. |
| Soft Sine | `3 × sin(clamp(x/3, −π/2, π/2))`; no folding. |
| Bass Shaper | `x / sqrt(1 + (x/1.8)^2)`; fixed broad knee. |
| Medium Curve | `y − y^3/(3 × 1.65^2)`, with `y=clamp(x, −1.65, 1.65)`; ceiling ±1.1 and zero slope at the plateau. |
| Hard Curve | `clamp(x, −1.2, 1.2)`; linear center and abrupt knee. |

Soft Clip applies a rounded knee with `k=0.5`, `c=1`. Hard Clip clamps to ±1. After decimation, apply a sample-peak guard at ±1 once the No Clip transition weight reaches zero, preventing reconstruction overshoot from violating the post-clip sample ceiling. Output Level follows this guard; dry/bypass remain independent of that ceiling. During selector transitions, crossfade the old/new nonlinear contributions with shared ramps. Normalize their weights each frame to avoid gain dips after rapid retargets with different remaining ramp lengths. All branches share the same FIR clock/history.

Drive dB, output amplitude, blend, bypass, and selector weights use the existing 5 ms `ModulationRamp`; offset resolution uses the shared 1 ms `ModulationSmoother<3>`. Compatible retargeting preserves FIR history, delay cursors, route ramps, and in-flight transitions. Presentation-only edits do not restart ramps. Nonfinite input samples become zero. Use f64 internal sums and clamp the final finite conversion to f32 range. No callback allocation or final-owner retirement is introduced.

Version-1 `daw.saturator` settings retain all controls, routes, enable/collapse/open state. `ModulationParameter::SATURATOR` declares shared Drive/Level/DryWet. The distinct Drive slot appends index 66; existing indices remain unchanged. Level and Dry/Wet reuse their existing typed identities. Engine snapshots expose Drive dB, Level dB, and mix in slots 0–2. Registration appends the new type to the Effects catalog without changing existing entry indices. No drag allowlist or Saturator-specific transfer code is added.

## Curve separation retune — 2026-10-23

The previous Soft Sine and Medium Curve were close approximations over modest instrument levels. Level-matched user probes at +10 dB Drive differed by only about 0.4% RMS. Retain Analog Clip as the default reference; widen Soft Sine, widen Bass Shaper's rational knee while retaining its low-frequency harmonic advantage, lower Medium Curve's cubic knee, and replace Hard Curve's rounded knee with actual clipping. All five remain odd, monotonic, continuous, bounded, and unity gain for small signals. Keep No Clip headroom, fixed latency, oversampling, modulation, crossfades, and routing unchanged. Settings remain version 1 with the same identities and controls; existing non-default shapes use the retuned algorithms and can change sound/level. There is no automatic gain compensation.

A processed coherent sine-triad regression at +10 dB Drive requires more than 2.5% gain-matched RMS residual between every pair at all six project rates, with more than 8% between Soft Sine and Medium Curve. This is a waveform-separation guard, not a claim of perceptual loudness matching or a substitute for listening. Separate harmonic checks require Medium Curve's third harmonic to exceed Soft Sine's by four times and Hard Curve to retain upper odd harmonics. Retain continuity, symmetry, small-signal gain, Bass Shaper bass-harmonic, alias suppression, timing, transition, and live/export parity checks.

## Standard output monitoring

Use the [standard effect Out-strip meter](../architecture/modular-audio-units.md#standard-effect-out-strip-meter) for final linked-stereo level, shared held-peak capture/reset, bypass masking, and allocation-free prepared meter binding. Retain audio DSP, latency, saved settings and existing modulation behavior.
