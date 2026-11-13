# Multiband EQ

For controls and use, see the [user guide](../../docs/effects/multiband-eq.md).

## Accepted behavior and layout

Multiband EQ is a separate effect registered once in `effects_catalog::ENTRIES` under EQ & Filters. It uses the existing track, layer, Main, and Master effect chains, catalog gestures, unit selection, header reordering, collapse, compact mode, and `Edit::SetEffects` history. Existing `daw.eq3` settings and processing remain unchanged.

Each band has a stable UUID, power, Mode, Frequency, Gain, and Q. There is no fixed band limit. Empty band lists are valid. Defaults are one enabled Bell band at 1 kHz, 0 dB, Q = 1/√2, with unit power on and Output Gain at 0 dB. Band Gain spans −24…+24 dB, Frequency 20…20,000 Hz, and Q 0.1…10. Output Gain uses the shared silence…+12 dB control. Band power bypasses its filter; it does not cut a frequency range.

Each band occupies one 90-point strip, following the Oscillator layout. Mode, Frequency, Gain, and Q run vertically. Frequency, Gain and Q use 42-point knob faces with the shared display scale, text, value arcs and modulation markers. Three-point vertical gaps keep all four controls and the standard centered Power/“Band N” footer inside the existing expanded lane height. Scaling changes painting and allocated bounds while retaining shared drag, keyboard, reset, focus, and accessibility behavior. Mode uses `menus::Dropdown` with Small-text popup items for all ten `EqBandMode` choices, a full-content-width truncated caption and the shared 5-point control corners. Center its standard-height button within the existing 42-point top slot so the three knobs and footer retain their positions. The DAW and standalone editors share this renderer; retain mode parameter IDs and host gesture bindings. Gain is disabled for all Low/High pass orders and Notch while retaining its value. Right-click a band or its controls for Enable band/Disable band, checked Solo, and Remove band. Reuse `menus::band_context_menu` with Multiband Compressor, retaining EQ’s ability to remove its last band. The shared Oscillator Plus strip follows the bands: an 18-point circular control with the shared panel margin on both sides, for 38 points total at the current margin. The final audio strip is Out, with the normal output gain knob; shared modulation connection strips follow it when open. Oscillator and Multiband EQ use the same `panels::unit_strip` and `panels::unit_add_strip` containers, padding, widths, and add-strip interaction. Band footers use `rows::power_label`; Out uses `rows::unit_label`. Dividers and compact controls retain the shared panel design.

Band addition and removal edit cloned settings through the existing effect-chain transaction. Undo/redo, failed preparation, and cancellation follow session history and worker publication. Removing a band removes only its targeted routes. Duplication assigns fresh effect, band, and route IDs and remaps each copied route to its copied band.

Built-in band reordering uses Oscillator's shared `drag` insertion-slot, threshold, preview-line, cancellation, focus and horizontal edge-scroll helpers. Extend the existing `strip_reorder` gesture lifecycle for EQ bands rather than owning another drag loop. Arm on primary press in the band's label or unused strip space; exclude knob and Power faces. Disable band controls during an active drag. Show the insertion line without changing settings or playback, then move the original band only on valid release within the clipped band bank. Preserve UUIDs, all band settings, routes and meter keys. Escape, focus loss, disabled editing, changed band settings, a changed host or a hidden/removed unit cancels the gesture. Outside drops and same-position drops make no project edit. Commit through the existing track/layer/Main/Master effect-chain transaction as one Undo step; prepared DSP inheritance continues by UUID. No saved format, DSP or parameter-ID changes are needed. Standalone CLAP/VST3 editors use the same release-only gesture lifecycle for visual strip order. Their version-2 state stores a validated slot permutation; version-1 state restores the original order. Public automation IDs, their slot bindings and prepared DSP order stay fixed. CLAP stores presentation changes on the control thread; VST3 uses controller state. Native host undo/recall acceptance remains separate from headless gesture/state tests. See [plugin state and ordering](../architecture/all-effect-plugin-builds.md#multiband-eq-prepared-bank).

## Portable processing

`unit-core::MultibandEqSettings` owns version 1 state under `daw.multiband-eq`. It rejects nil/duplicate band IDs, nonfinite or out-of-range values, unsupported parameters, missing targets, and incorrectly targeted Output Gain routes. Future state versions retain their unknown payload through `EffectSettings::Missing`.

`equalizer::MultibandEq` owns the portable zero-latency stereo DSP. It has no UI, DAW, media, or device dependency. It reuses the existing private FunDSP dependency and its packaged license notices. Ten warm stereo mode paths per band implement Bell, Low/High shelf, Low/High pass, Notch, High pass ×4, Low pass ×4, High pass ×8 and Low pass ×8. The ×4 modes use two serial two-pole SVF sections (four poles total, 24 dB/octave). At default Q the section Qs are 0.5411961 and 1.306563, giving a fourth-order Butterworth response; the Q control scales both sections proportionally. The ×8 modes use four serial two-pole sections (eight poles total, 48 dB/octave), with default section Qs 0.5097956, 0.6013449, 0.8999762 and 2.5629154. For both orders, section k uses Q × √2 / (2 cos((2k + 1)π / (2 × order))), starting at k = 0. Existing two-pole modes retain their original processing. Bands run in serial order with independent channel state. The Output gain follows the last band; whole-unit bypass returns the original input.

Settled whole-unit bypass returns the exact dry sample, including signed zero. Warm mode paths bound only f32 representation overflow at each SVF section boundary, before another section can receive infinity. Response accumulation and crossfades use wider intermediates; zero-weight modes cannot poison the sum. Later finite samples retain finite filter histories after extreme finite input.

All mode filter states continue processing while inactive. Power and mode changes crossfade filter responses over 5 ms. Base parameters, output gain, and whole-unit bypass ramp over 5 ms at each project rate. Frequency clamps below Nyquist in the DSP. Gain affects only Bell and shelf modes. Output modulation preserves the shared silence floor.

Construction allocates filter bands and route storage on the control/worker thread. Compatible prepared updates transfer histories by stable band UUID. Added bands use prepared state; removed band owners remain with the retired renderer. Live updates do not allocate, lock, perform I/O, or release final buffer owners on the callback. More bands increase CPU and prepared storage; no fixed application limit is imposed.

## Band output meters

Each expanded band reuses `meters::unit_monitor_rail` and `meters::band_level_rail` above its first knob. The rail remains four points high within the existing padding, without moving controls or changing lane height. The shared scale is −60…0 dBFS. Enabled levels use track-meter green through 0 dBFS and red above it, independently of the accent; overload readings retain their value in the tooltip. Disabled bands use the common secondary-grey fill, including overload and transition readings. Whole-unit power masks stale readings through `meters::unit_reading`. Compact/collapsed units hide rails, and the native host masks readings while stopped or paused.

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

## Temporary band Solo

Reuse `rows::band_solo` beside the bottom band power label, matching Multiband
Compressor. Compact 90-point EQ footers use B1, B2, etc. to fit the power and S
buttons without changing lane height. The accessibility labels retain Band N.
In expanded mode, anchor the band Power/label/Solo row to the standard footer
position, aligned with Response and Out. The packed control rows must not move
the footer upward. Apply the same shared layout in the DAW and standalone editors.
The live-only binding and offline-mode rules follow [Multiband Compressor](multiband-compressor.md#temporary-solo).

Solo uses separate EQ and rejection histories. Bell/Notch reject a notch at the
effective Frequency/Q; low-frequency modes reject a high-pass response, and
high-frequency modes reject a low-pass response. Apply the selected bands' EQ
responses, then subtract the serial complementary rejection result to audition
the union. Band power bypasses its EQ response without disabling isolation.
Unselected EQ responses do not enter the audition path. Output gain still applies.
Use effective modulated Frequency/Q and the existing mode/power weights. Fade
selection and whole audition over 5 ms. Normal serial histories continue running.
No Solo handles enter project state, plugin parameters, presets, export processors
or detached clones. Preserve the existing fixed-slot suspension when Solo is idle.

## Held band output peaks

Bind each band's `LevelMonitor` to a prepared `BandMeter<3>` peak handle by its
stable effect/band key. Retain the raw-current attachment API for existing users.
Capture held peaks from actual samples, independent of the 300 ms current-level
release. Use `meters::band_output_level` for shared marker painting, readout,
accessibility, click reset, unit bypass masking and disabled-band colors. Keep
rail geometry unchanged and exclude meter presses from strip reordering. Holds
remain monitor state, outside presets and project/history edits.

Native buses and CLAP share the prepared handles. VST3 appends 64 held values to
the previous 77-value telemetry, for 141 values, and reuses the shared per-band
level reset requests. Resetting one band must leave other bands and Out unchanged;
unit bypass clears all band readings and holds. Preparation, inheritance and
current-level release must not create held peaks.

## Response graph and live spectrum

The shared `unit-ui::eq_graph` body precedes the band strips in expanded mode. Its 270-point section contains a 172-point graph across the three standard control rows and a centered Response footer. Use only the standard eight-point item gap before the 18-point footer: the content totals 198 points and the padded section totals 218 points, matching the band/Add/Out strips. Do not add another explicit footer gap. Compact/collapsed units hide it. Reuse theme colors, fonts, dividers and lane geometry in the DAW and standalone editors. Frequency spans 20 Hz–20 kHz logarithmically; response spans −24…+24 dB. Draw individual responses, the combined curve, a subdued −90…0 dBFS output spectrum, and numbered draggable points. Keep the existing band/Add/Out section indices; graph bounds are separate.

The combined curve and unselected band points use `palette.secondary`, matching unselected unit-header text. Selected enabled points retain the accent color. Disabled bands and units use `palette.secondary`.

`equalizer::band_response` derives a portable `FrequencyResponse` from the actual private FunDSP SVF coefficients. Test it against the backend transfer response for every mode, Q range and supported sample rate. Effective modulation snapshots drive curves outside active edits; active drafts show their proposed base settings. The graph does not alter DSP or persist display state.

Normal point drags change logarithmic Frequency and, for gain-using modes, Gain. Command on macOS or Ctrl on Windows/Linux selects Q-only dragging. Read the modifier every frame so pressing or releasing it switches modes within the same pointer gesture. At each switch, retain the current draft and restart displacement accumulation from those values. Modifier-only frames must not change values; subsequent movement changes only the selected mode. Right/up increases Q; left/down decreases it, exponentially within 0.1…10. Shift reduces sensitivity tenfold. Accumulate displacement from the current mode segment's starting values to avoid drift. Retain the full gesture's original values for cancellation. In the DAW, publish all Frequency, Gain and Q changes atomically on release as one history transaction, independent of the number of mode switches. One Undo restores all three values; one Redo reapplies them. Escape, focus loss, hidden/disabled UI or invalidated target settings cancels. Preserve stable band IDs across interactions.

The DAW keeps held drafts outside Session. Clone the render plan, validate the changed effect chain through its ordinary project edit, then send the transient plan through the existing coalesced audio-preparation worker. Preserve saved state and history until release, which commits one ordinary effect-chain transaction. Cancellation publishes the current committed plan. Standalone editors update bounded host parameter gestures live and restore starting values on cancellation, with balanced Begin/Value/End events and fixed slot automation IDs. Both hosts paint through the same body. The standalone viewport grows by 270 points to 488×218 at its default band count; overflow still scrolls.

`SpectrumMeter` and `SpectrumReading` in unit-core contain 64 logarithmic bins and sample-rate metadata. The portable EQ analyzer packs L+iR into a prepared radix-2 FFT, preserving opposite-phase stereo energy. Hann windows use 4096 samples at 44.1/48 kHz, 8192 at 88.2/96 kHz, and 16384 at 176.4/192 kHz. Precompute tables and buffers on preparation threads. Spread FFT and bin work over input samples, capped at eight work items per sample. Publish complete frames with 300 ms release; never expose partially collected bins. Monitoring is display-only, allocation-free, and adds no audio latency.

The shared graph interpolates spectrum presentation without changing analyzer
cadence, the 64-bin telemetry layout, saved state or audio. Keep per-graph,
ephemeral dB values in egui state. Follow new bins with a 35 ms exponential time
constant using elapsed UI time, independent of frame rate and duplicate layout
passes. Initialize directly from the first reading. Clear display history on
missing/all-zero readings, bypass or hiding; reset it on rate changes or a gap
in graph frames. Stop/paused masking must clear immediately rather than leave a
smoothed ghost spectrum. Request 16 ms redraws while the display is active.

Draw a shape-preserving cubic curve in logarithmic frequency/dB space. Use zero
slopes at extrema, harmonic-mean slopes between monotonic segments and clamp each
segment within its adjacent displayed bin values. Sample once per logical horizontal
point, with 64–1024 segments. This removes angular joins without claiming extra
frequency resolution or introducing peaks beyond the telemetry. Keep the existing
fill, stroke, plot bounds, response curves and band interaction geometry. DAW,
CLAP and VST3 editors call this same graph renderer.

Reuse `UnitMeter` enable/clear behavior and prepared handle ownership. Attach prepared renderers without clearing the last visible frame at the same sample rate. Compatible updates transfer analyzer history and retarget handles without final-owner retirement on the callback. Bind after worker preroll, retain handles by effect ID, and mask stopped/paused readings. Test repeated band edits through both the portable analyzer and renderer at all six rates; preparation and inheritance must retain a nonzero published frame. Track, layer, Main and Master scopes use the same binding path.

CLAP shares the prepared spectrum handle. VST3 retains slots 0…140 and appends sample rate at 141 and 64 magnitudes at 142…205, for 206 telemetry values. This changes no saved state or automation IDs. Test transport, bypass clearing, unchanged PCM, and allocation-free callbacks and inheritance.

## Ten-choice plugin Mode range

Append High pass ×4 and Low pass ×4 as plain mode values 6 and 7, then
High pass ×8 and Low pass ×8 as values 8 and 9. Retain the
original six values, parameter IDs, defaults and saved-state layout. The checked
manifest and all 64 band Mode descriptors now have range 0–9 and nine steps.
Existing project and plugin preset mode values retain their meaning. Normalized
CLAP/VST3 host automation uses the expanded range, so automation recorded against
the former six-choice range can select different modes. Native host automation
migration/acceptance is not claimed.

The shared graph response supports up to four serial biquad sections and sums their
dB responses before display clamping. It uses the same section Qs as the DSP.
Selector and band-context popups have separate identities; retain temporary
Solo and Remove-band actions on the selector. Standalone control capture records
the dropdown response so a menu selection emits a bounded edit for its Mode
parameter.
