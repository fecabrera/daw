# Direction Mixer

Implementation requirements and acceptance criteria. For operation, see the [user guide](../../docs/effects/direction-mixer.md).

## Ownership and integration

`crates/unit-core` owns `DirectionMixerSettings`, `DirectionMixerInput`, and the versioned `daw.direction-mixer` effect identity. `crates/direction-mixer` owns portable stereo DSP, base ramps, and final parameter bounds, with no DAW, UI, device, or third-party DSP dependencies. The shared engine constructs it for track and Master chains and invokes the typed modulation entry point. Playback, offline export, random-access rendering, and CLI rendering share that path.

Register Direction Mixer once in `effects_catalog.rs::ENTRIES` under Imaging. Keep earlier registration indices stable. The catalog, selection, folder defaults, keyboard input, group addition/drop, reorder/copy/delete, stable IDs, and transactions retain their shared behavior. Main is a modulation-provider bus, not another audio-effect installation target.

## Controls and state

| Field | Bounds/default | Contract |
| --- | --- | --- |
| `input` | `LeftRight` by default; `MidSide` alternative | Incoming channel interpretation. Output is always L/R. |
| `direction_degrees` | −180–180; 0 | Negative pans the center left; positive pans it right, matching the knob. ±180 exchanges stereo sides. |
| `spread` | 0–2; 1 | Width control before direction: piecewise L/R Mid/Side gains; direct Side multiplier for encoded M/S. |
| `enabled`, `collapsed` | true, false | Independent shared power and presentation states. |
| `modulation_open`, `modulations` | false, empty | Optional serde defaults; explicit null is invalid. |

State version 1 uses the existing `EffectSettings` envelope and fresh `EffectInstance` IDs. Validate finite parameter bounds and shared routes atomically. Malformed or unsupported current data fails; unknown future versions retain their opaque payload as a missing bypassed effect. Empty legacy effect chains remain unchanged. Save/open, Save As, processing-rate changes, track duplication, and effect copy/reorder retain settings; common handlers assign fresh effect IDs, regenerate route IDs for effect drag copies, and remap provider references when required.

Use one standard 90-point strip, including padding, with Input, Direction, and Spread in three shared knob rows and a centered Stereo footer. Reuse `PanelStyle::unit_in_lane`, the shared header power/collapse and modulation dot, full-height connection divider, and compact 68/expanded 244-point lane heights. Input uses the bounded L/R–M/S two-way selector with neutral arc and active accent label. Direction reuses the common angle specification with degree readout, centered zero, and ±180 endpoints. Spread uses a linear 0–2 knob, unity mark, 0.01 keyboard steps, and two-decimal readout. Defaults, reset, pointer/Shift behavior, focus/modal guards, tooltips, and accessibility stay in the shared knob. Parameter and amount definitions belong in `parameter_knobs.rs`. Compact/collapsed modes hide normal controls through the existing shell.

## DSP

For L/R input, form `M = (L + R) / 2` and `S = (L - R) / 2`, then apply `side_gain = min(Spread, 1)` and the following Mid gains. This narrows by reducing Side below unity and widens by reducing Mid above unity. Keep Mid gain at 1 below unity and interpolate linearly between the upper anchors. Keep the control continuous rather than quantizing it. A mono L/R input stays mono and becomes quieter above unity; an anti-phase input is unchanged above unity.

| Spread | Mid gain | Side gain |
| --- | --- | --- |
| 0 | 1 | 0 |
| 1 | 1 | 1 |
| 1.25 | 0.818 | 1 |
| 1.5 | 0.65 | 1 |
| 1.75 | 0.482 | 1 |
| 2 | 0.30 | 1 |

For M/S input, take `M = input[0]` and `S = input[1]` directly, then multiply only `S` by Spread. Retain the accepted direct Side-level control for encoded recordings; do not apply the L/R center attenuation to it. No additional M/S normalization is applied. Blend these complete weighted Mid/Side pairs through the existing input-mode ramp. Let `s = sin(direction)` and `c = cos(direction)`. Use the weighted values in the output matrix without applying Spread again:

```text
left  = M * sqrt(1 - s) + S * c
right = M * sqrt(1 + s) - S * c
```

Use exact coefficients at 0°, ±90°, and ±180°. At 0° this decodes the widened M/S pair; at ±90° Side vanishes and Mid pans fully left/right with equal-power gain; at ±180° Side reverses to exchange L/R without inverting Mid. Direction changes both placement and apparent width. A mono input cannot gain Side content from Spread. Peaks may increase; do not auto-normalize or add a limiter.

L/R, Direction 0°, Spread 1 returns the original finite samples exactly. Whole-unit bypass returns the raw input, including encoded M/S input. Sanitize nonfinite input to zero; use f64 intermediate arithmetic and bound extreme finite output to the f32 range. Silence remains exact. There is no latency, history tail, or automatic export extension.

Base Direction, Spread, input format, and bypass use independent 5 ms linear ramps at every project rate. Direction is bounded rather than cyclic: a +180° to −180° edit ramps through the bounded range. Compatible updates preserve current ramps and offset history by effect identity; presentation-only edits do not restart them. Stop/seek reset to current targets, Pause retains state, and loops/gaps retain the shared processing behavior. No processing, reset, or compatible retarget operation allocates, locks, logs, or performs I/O on the callback.

The 0°/±90°/±180° landmarks follow the published [Direction Mixer behavior](https://support.apple.com/en-mide/guide/logicpro/lgcef240d9d7/10.7/mac/11.0). This application uses negative-left/positive-right angles to match its knob, reversing the reference sign convention. The matrix here is independently defined from those landmarks; no compatibility with another processor's internal DSP is claimed.

The corrected L/R Spread anchors come from six user-supplied 48 kHz PCM24 Logic Pro bounces at Direction 0° with Spread 0, 1, 1.25, 1.5, 1.75, and 2. The user confirms the upper endpoint readout is exactly 2.00. The measured gains relative to the unity bounce are recorded above. The former always-unity Mid and direct 0–2 Side multiplier matched none of the upper anchors and is superseded. Linear interpolation through the measured anchors is this application's explicit curve; unmeasured reference settings and encoded M/S output are not claimed equivalent. Reference recordings and machine paths are not repository fixtures. See the [validation record](../validation/README.md#direction-mixer-reference-spread-correction--2026-10-18) for measurement limits.

## Modulation

`ModulationParameter::DIRECTION_MIXER` contains Direction then Spread. Both settings and DSP declare Shared scope through `ModulationDestinationSettings` and `ModulationDestination`. Add the two new typed offset slots after existing slots, preserving their indices. Reuse source discovery/publication, scope checks, route validation/resolution, cached fan-out, route amount/power ramps, `process_modulation`, and `ModulationSmoother<2>` for the shared 1 ms additive smoother. Do not add another source lookup or routing loop.

| Destination | Signed amount | Final application |
| --- | --- | --- |
| Direction | −180° to +180° | Add degrees to the smoothed base; clamp to −180–180. |
| Spread | −2 to +2 | Add control units to the smoothed base; clamp to 0–2, then apply the input-specific audio gain curve. |

Sources are shared Retrigger/Legato Envelope, LFO, and Macro instances from the same MIDI instrument, including Main. Reject foreign and Per note references through the common core rules. Missing, disabled, unavailable, and unassigned runtime sources contribute zero. Input/power remain discrete. Modulation never edits base settings and continues advancing through bypass. Keep route smoothing separate from the base ramps and preserve zero-offset unity arithmetic.

Reuse the Source/Destination/Amount strips, connection Power/Plus/removal/reorder, destination-matched signed amount controls, status dots, correlated range arcs, processed-value dots, and click-to-open behavior. Prepared atomic snapshots report actual Direction and Spread in slots 0–1, with unused slots unavailable. Output discovery, binding, stable-ID retention, and legacy five-reading adapters stay shared. These are parameter snapshots, not audio meters; this unit adds no audio meter.

## Acceptance

Check all six project rates: exact default unity and dry bypass, M/S decoding, width 0/1/2, cardinal angles, negative-left/positive-right base and modulation direction, mono/anti-phase/silence, finite extreme input, bounds, base/mode/bypass ramps, reset, and unchanged compatible ramps. Verify both typed destinations, neutral offsets, shared/local scope, signed route cancellation, all three provider kinds, smoothing, and callback allocation constraints.

Check track and Master chains against independently predecoded audio; live/offline/block/random-access/loop parity; route reorder and compatible replacement; actual snapshots; source validation/remapping; save/reopen/Save As/duplication/rate persistence and invalid-edit rollback. UI checks cover both themes, all installation targets, Imaging folder and display order, knob edits, shared amount scales/reset/Shift, modulation markers and connection opening, power, compact/collapse, and removal. Native appearance, pointer/device timing, and Windows/Linux need separate verification.

For L/R Spread, use different tones on the left and right channels. At Direction 0°, verify all six measured Mid/Side gain pairs plus interpolation within lower and upper segments. Cover track and Master installation at all project rates, offline rendering, and compatible changes during playback after the 5 ms base ramp. Check mono, anti-phase, and nearly identical channels; base/modulated equivalence; unchanged M/S Side gain; and smooth transitions across unity and input modes. Do not treat two identical channels as evidence of stereo width.
