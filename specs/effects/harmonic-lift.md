# Harmonic Lift

## Scope and ownership

Harmonic Lift is a native harmonic enhancer and linked stereo maximizer. It includes built-in track, group, layer and master placement, plus standalone CLAP/VST3 products. It uses project processing rates from 44.1 to 192 kHz. The user controls are in the [guide](../../docs/effects/harmonic-lift.md).

`unit-core::HarmonicLiftSettings` owns version 1 settings under `daw.harmonic-lift`, including six stable branch UUIDs. `harmonic-lift` owns portable stereo DSP. `filter::svf` owns shared trapezoidal SVF primitives with private FunDSP coefficients. `engine` constructs the unit for live and offline rendering. `unit-ui::harmonic_lift` owns the headerless body; `ui` adds the shell, transactions and targeted modulation. The new plugin and editor crates reuse the existing bounded state, format adapters, gestures and meter transport. No new external dependency is added.

The six branch roles are fixed. Copies receive new branch and route identities, and remap targeted routes. Unknown future settings versions remain missing, bypassed units with their payload intact.

## Native processing

1. Tap original stereo input for the aligned dry and bypass paths.
2. Apply Wet Input to the wet path.
3. Add six parallel shaped branches to the wet input. First five use raw SVF band-pass outputs at Q 0.58. High uses high-pass at Q 0.58. Branch frequency limits follow the guide. This is not a complementary crossover bank.
4. Shape each branch with `tanh(x)` for positive samples and `x / (1 + abs(x)^0.4466)^2.23914` otherwise. Apply a 10 Hz high-pass at Q 1/√2, branch Gain and the smoothed Power contribution.
5. Global Drive uses inverse ±3 dB 6.3 kHz shelving filters at Q 0.1, with the intervening curve `(1 + k) x / (1 + k abs(x))`, `k = 3 Drive`. Drive zero bypasses the stage while its filters advance.
6. Maximization uses linked stereo absolute peak detection, a 10 Hz one-pole envelope, and threshold `10^(-1.5 Push)`. Reduction is at most unity, with denominator equal to the larger of current peak and square root of the envelope. Push zero bypasses reduction; entering or leaving zero also crossfades the maximizer stage over 5 ms to avoid a level step on hot input. Apply Limit Level divided by threshold after reduction.
7. HQ processes these stages at twice the sample rate and downsamples. Normal uses a fixed 49-frame delay. Mix aligned dry/wet, apply final Level, then crossfade header bypass with aligned original audio.

The unit sanitizes nonfinite inputs and uses bounded float64 nonlinear processing before finite float32 publication. Limit Level is a wet-stage setting, not a final true-peak ceiling. Output Level and dry mixing can exceed it. FIR ringing can also produce sample overshoot.

All continuous base controls, Power and quality transitions use 5 ms ramps. Frequency ramps operate in log frequency. Shared routes use `RoutedDestination` and `ModulationSmoother`. Branch targets reuse typed Frequency (octaves) and Gain (dB) destinations with UUID targets; global destinations are Wet Input, Drive, Push, Limit Level, Level and Dry/Wet. Final bounds live in portable DSP. No custom route resolver is used.

Both preallocated quality paths remain warm to permit sample-timed standalone automation without resetting filter or FIR history. Normal therefore does not provide the CPU saving of suspending HQ processing. No conversion, allocation, lock, file I/O or owner retirement runs in sample processing or parameter retargeting. Preparation and route changes occur before publication; compatible updates transfer history and route ramps.

## Independent FIR and provenance

The supplied Harmonic Maximizer technical report was used as a behavioral and mathematical reference. Its implementation-specific coefficient archives and proprietary C++ were not imported. This implementation is independently authored and is not a binary/state/preset compatibility layer for that product.

The 2× FIR pair is generated during preparation from normalized Kaiser-windowed sinc filters: 119-tap interpolation (β 8.6) and 79-tap decimation (β 7.5), half-band cutoff. Their combined delay is `(59 + 39) / 2 = 49` host frames. Decimation retains the even phase. Dry, Normal and bypass use the same 49-frame delay in every mode. This corrects the reference's wet/dry one-frame mismatch. The native design also replaces zero-gain branch switching with explicit Power, uses standard gain/output controls, and smooths edits.

## Presentation and monitoring

The nine fixed strips use the shared 90-point width, standard 52-point knobs, strip height, margins, cards and footer alignment. Quality uses `Dropdown::small_items()`. Each branch has Frequency and Gain plus footer Power, with a four-point horizontal level rail in its existing top margin. Input and Out have no independent footer Power. Maximizer retains Drive, Push and Limit Level with its top reduction rail. The DAW uses the stereo gap output meter; standalone Out retains its horizontal output rail. DAW selection, collapse, bypass and modulation use existing shells.

`UnitMeter<PeakMeter>` publishes smoothed reduction with 10 ms attack and 300 ms release. `LevelMonitor<3>` publishes final output. Bypass immediately clears and masks both meters while audio completes its ramp. Meter clicks do not change the project or start drags.

Six independent `LevelMonitor<3>` instances publish branch contribution levels
after asymmetric shaping, 10 Hz high-pass, Gain and smoothed Power, before global
Drive, Push, Limit Level, final Level and Dry/Wet. Core captures the larger
absolute stereo contribution for each branch. Normal uses the native-rate peak;
HQ uses the maximum across its two internal frames. Blend these peaks with the
current quality crossfade. These are internal tap readings before output-path
latency alignment, not a reconstruction or analysis of the final output.

Reuse shared instant attack, 300 ms release, held sample peaks, bypass masking
and secondary-grey inactive-band colors. A branch Power transition fades its
contribution to zero; its held peak remains resettable. Unit bypass clears all
branch readings/holds and stops publication. Clones detach handles; compatible
inheritance preserves release history and moves prepared handles without
allocation or final-owner retirement in the callback.

The DAW discovers branches through `EffectSettings::band_ids`, scopes handles by
effect and branch UUID, and binds them with the existing individual-band meter
path. `unit-ui::harmonic_lift` paints all six through `band_output_level`, so the
DAW output-rail suppression policy does not hide them. Hide branch rails in
compact/collapsed views without moving controls or changing unit dimensions.

## Standalone contract

`plugins/harmonic-lift/unit.toml` is the checked manifest and identity source. Automation IDs 100–125 cover Bypass, Wet Input, Quality, six ordered Power/Frequency/Gain triplets, Drive, Push, Limit Level, Level and Dry/Wet. Deterministic local branch IDs bind these fixed roles. Binary state uses `DAWHARML`, envelope version 1, settings version 1, and 26 bounded float64 values. It contains no DAW routes, paths, project IDs or DSP history.

Both formats report 49 frames latency at every rate and a conservative four-second tail. They reuse the shared native editor/window and narrow format sources via concrete dependency aliases. The packager discovers the manifest, and CI builds the product on macOS, Windows and Linux. Native host attachment and SDK validator acceptance remain separate platform checks.

Meter transport retains the existing 13-value common prefix and appends six
current/held branch pairs at indices `13 + 2*i` and `14 + 2*i`, for 25 values.
Reset bits 5–10 select the six independent branch holds; existing output,
reduction and common band reset bits retain their meanings. CLAP shares prepared
handles; VST3 uses the existing bounded control-thread telemetry messages.
Editor readouts send reset masks only, with no automation gesture or state edit.
Parameter IDs, state bytes and viewport dimensions stay unchanged.

## Validation

Automated checks cover impulse alignment, neutral/dry/bypass behavior at six rates, asymmetric harmonics, linked limiting, final headroom, quality transitions, nonfinite input recovery, meter bypass, saved state, copy identities, live/export/seek agreement, callback allocation/retirement, plugin parameter mappings and binary fixtures, ABI behavior, shared editor parity and package discovery. Current results and native limits are recorded in the [validation log](../validation/harmonic-lift.md).
