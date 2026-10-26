# Multiband EQ

For controls and use, see the [user guide](../../docs/effects/multiband-eq.md).

## Accepted behavior and layout

Multiband EQ is a separate effect registered once in `effects_catalog::ENTRIES` under EQ & Filters. It uses the existing track, layer, Main, and Master effect chains, catalog gestures, unit selection, header reordering, collapse, compact mode, and `Edit::SetEffects` history. Existing `daw.eq3` settings and processing remain unchanged.

Each band has a stable UUID, power, Mode, Frequency, Gain, and Q. There is no fixed band limit. Empty band lists are valid. Defaults are one enabled Bell band at 1 kHz, 0 dB, Q = 1/√2, with unit power on and Output Gain at 0 dB. Band Gain spans −24…+24 dB, Frequency 20…20,000 Hz, and Q 0.1…10. Output Gain uses the shared silence…+12 dB control. Band power bypasses its filter; it does not cut a frequency range.

Each band occupies one 90-point strip, following the Oscillator layout. Mode, Frequency, Gain, and Q run vertically. Their 42-point knob faces use the shared knob's display scale, including diagrams, text, value arcs, and modulation markers. Three-point vertical gaps keep all four controls and the standard centered Power/“Band N” footer inside the existing expanded lane height. Scaling changes painting and allocated bounds while retaining shared drag, keyboard, reset, focus, and accessibility behavior. Mode uses `KnobSpec::diagrams` with six filter-response diagrams and the accent highlight on the selected response. Gain is disabled for Low pass, High pass, and Notch while retaining its value. Right-click a knob or footer power button for Remove band. The shared Oscillator Plus strip follows the bands: an 18-point circular control with the shared panel margin on both sides, for 34 points total at the current margin. The final audio strip is Out, with the normal output gain knob; shared modulation connection strips follow it when open. Oscillator and Multiband EQ use the same `panels::unit_strip` and `panels::unit_add_strip` containers, padding, widths, and add-strip interaction. Band footers use `rows::power_label`; Out uses `rows::unit_label`. Dividers and compact controls retain the shared panel design.

Band addition and removal edit cloned settings through the existing effect-chain transaction. Undo/redo, failed preparation, and cancellation follow session history and worker publication. Removing a band removes only its targeted routes. Duplication assigns fresh effect, band, and route IDs and remaps each copied route to its copied band.

## Portable processing

`unit-core::MultibandEqSettings` owns version 1 state under `daw.multiband-eq`. It rejects nil/duplicate band IDs, nonfinite or out-of-range values, unsupported parameters, missing targets, and incorrectly targeted Output Gain routes. Future state versions retain their unknown payload through `EffectSettings::Missing`.

`equalizer::MultibandEq` owns the portable zero-latency stereo DSP. It has no UI, DAW, media, or device dependency. It reuses the existing private FunDSP dependency and its packaged license notices. Six stereo state-variable filters per band implement Bell, Low/High shelf, Low/High pass, and Notch. Bands run in serial order with independent channel state. The Output gain follows the last band; whole-unit bypass returns the original input.

All mode filter states continue processing while inactive. Power and mode changes crossfade filter responses over 5 ms. Base parameters, output gain, and whole-unit bypass ramp over 5 ms at each project rate. Frequency clamps below Nyquist in the DSP. Gain affects only Bell and shelf modes. Output modulation preserves the shared silence floor.

Construction allocates filter bands and route storage on the control/worker thread. Compatible prepared updates transfer histories by stable band UUID. Added bands use prepared state; removed band owners remain with the retired renderer. Live updates do not allocate, lock, perform I/O, or release final buffer owners on the callback. More bands increase CPU and prepared storage; no fixed application limit is imposed.

## Band output meters

Each expanded band reuses `meters::unit_monitor_rail` and `meters::band_level_rail` above its first knob. The rail remains four points high within the existing padding, without moving controls or changing lane height. The shared scale is −60…0 dBFS; overload readings retain their value in the tooltip. Disabled bands use the common secondary-grey fill, including overload and transition readings. Whole-unit power masks stale readings through `meters::unit_reading`. Compact/collapsed units hide rails, and the native host masks readings while stopped or paused.

`unit-core::LevelMonitor` factors the existing 3-band EQ's instant peak attack, 300 ms release, finite-value handling and `UnitMeter` lifecycle into one helper. Both EQ processors use it. Multiband bands monitor their actual stereo stage output, taking the greater absolute channel value before the next band and Out Gain. The 3-band EQ keeps its existing frequency-selective analysis. Monitoring never enters the audio path and adds no latency. Bypass clears readings and suspends publication; reset clears peaks, clone detaches publication handles, and compatible band updates retain handles/history by UUID.

The existing output band-meter preparation, worker bindings and retirement path now accepts `ModulationTarget::Effect` for fixed groups and `EffectBand { effect, band }` for individual bands. This preserves parent scope even if two effects have equal local band IDs. Each individual band uses the first cell of the existing three-cell handle, with two reserved zero cells. Handles are allocated/bound on control workers, retained for matching keys, and pruned after removal; final ownership stays with renderer retirement. There is no fixed band-count or snapshot limit. Callback samples and inheritance allocate nothing.

## Shared modulation

`ModulationParameter::EQ_BAND` contains Gain, Frequency, and Q. Band routes use `target = Some(band.id)`; Output uses the existing Level parameter with no target. Settings use shared mixed-target validation. All destinations are Shared; per-note sources are unavailable.

`RoutedDestination` prepares each typed destination's routes, invokes the standard resolver, and transfers route ramps by route ID and target. `process_modulation_chain` composes repeated destinations without a unit-specific source lookup or routing loop. Band controls use the shared 1 ms additive offset smoother. Gain adds dB, Frequency multiplies by 2^octaves, and Q adds Q units; final bounds stay in portable DSP. Output Gain also uses the shared offset smoother.

The engine publishes each band's three effective parameters with `ModulationTarget::EffectBand`; Output uses the ordinary Effect snapshot. This avoids a fixed snapshot limit on band count. Prepared output handles include every band on track, layer, Main, and Master chains. UI markers filter by stable target UUID and reuse the shared range/indicator and amount-knob definitions.

## Standard output monitoring

Use the [standard effect Out-strip meter](../architecture/modular-audio-units.md#standard-effect-out-strip-meter) for final linked-stereo level, shared held-peak capture/reset, bypass masking, and allocation-free prepared meter binding. Retain audio DSP, latency, saved settings and existing modulation behavior.

## Plugin delivery

This unit also has independent CLAP/VST3 previews. The [shared build contract](../architecture/all-effect-plugin-builds.md) defines identity, state, automation, timing, meters, packaging, CI, and acceptance limits.
