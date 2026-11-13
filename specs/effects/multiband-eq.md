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

The footer always reads `Response`. There is no bass collection state, collection
repaint or resolution transition. DAW output masks the spectrum while
stopped/paused. Plugin musical transport Stop does not imply a processing reset:
hosts can continue callbacks while stopped.

The combined curve and unselected band points use `palette.secondary`, matching unselected unit-header text. Selected enabled points retain the accent color. Disabled bands and units use `palette.secondary`.

`equalizer::band_response` derives a portable `FrequencyResponse` from the actual private FunDSP SVF coefficients. Test it against the backend transfer response for every mode, Q range and supported sample rate. Effective modulation snapshots drive curves outside active edits; active drafts show their proposed base settings. The graph does not alter DSP or persist display state.

Normal point drags change logarithmic Frequency and, for gain-using modes, Gain. Command on macOS or Ctrl on Windows/Linux selects Q-only dragging. Read the modifier every frame so pressing or releasing it switches modes within the same pointer gesture. At each switch, retain the current draft and restart displacement accumulation from those values. Modifier-only frames must not change values; subsequent movement changes only the selected mode. Right/up increases Q; left/down decreases it, exponentially within 0.1…10. Shift reduces sensitivity tenfold. Accumulate displacement from the current mode segment's starting values to avoid drift. Retain the full gesture's original values for cancellation. In the DAW, publish all Frequency, Gain and Q changes atomically on release as one history transaction, independent of the number of mode switches. One Undo restores all three values; one Redo reapplies them. Escape, focus loss, hidden/disabled UI or invalidated target settings cancels. Preserve stable band IDs across interactions.

The DAW keeps held drafts outside Session. Clone the render plan, validate the changed effect chain through its ordinary project edit, then send the transient plan through the existing coalesced audio-preparation worker. Preserve saved state and history until release, which commits one ordinary effect-chain transaction. Cancellation publishes the current committed plan. Standalone editors update bounded host parameter gestures live and restore starting values on cancellation, with balanced Begin/Value/End events and fixed slot automation IDs. Both hosts paint through the same body. The standalone viewport grows by 270 points to 488×218 at its default band count; overflow still scrolls.

`SpectrumMeter` publishes coherent frames with 512 logarithmic measured cells
from 20 Hz to 20 kHz, sample rate, readiness, reset generation and an exclusive
observation-end frame clock. A single full-range Hann branch packs L+iR into a
prepared radix-2 FFT. Sum positive/negative coefficient powers to preserve mean
stereo energy, including opposite-phase audio. Analysis follows final output,
never enters PCM, and adds no audio latency.

| Project rate | FFT / hop | Coefficient spacing | Observation |
| --- | --- | --- | --- |
| 44.1 / 48 kHz | 4096 / 1024 | 10.767 / 11.719 Hz | 93 / 85 ms |
| 88.2 / 96 kHz | 8192 / 2048 | 10.767 / 11.719 Hz | 93 / 85 ms |
| 176.4 / 192 kHz | 16384 / 4096 | 10.767 / 11.719 Hz | 93 / 85 ms |

Use this branch at every frequency for the whole processing lifetime. This
supersedes the decimated high-resolution bass branch, provisional preview,
collection footer and resolution blend. No bass FIR, decimator, separate FFT or
cross-band power blending remains. Prepare windows, bit reversal, twiddles, maps
and buffers outside processing; no new thread is required.

A cell spans geometric midpoints to adjacent published centers. Pool the maximum
calibrated coefficient power inside each cell; interpolate adjacent powers only
when it contains no coefficient center. This preserves off-center peaks without
claiming finer measured resolution. Close bass tones can merge. Mapping validates
at most 26 contributions per cell, with an aggregate cap of 2400.

Service oldest ring copies before overwrite. Incremental copies, butterflies and
mapped contributions share an 80-item budget per source frame. A complete
publication combines 512 bins and uses fewer than 528 atomic stores. The prepared
analyzer, including inline storage, is less than 600 KiB. Live, prepared and retired
renderers can each own an analyzer until ordinary worker retirement completes.
Reset invalidates counters/jobs without clearing FFT-sized buffers. Missed analysis
deadlines invalidate analysis and restart capture; they never alter PCM.

The first full-range publication is available within 125 ms of processed audio
after activation/reset, including observation and computation but excluding
transport/polling and painting. After warmed silence, target onset within 3 dB
in 150 ms, including ordinary display easing. Amplitude release uses the actual
hop and the shared 300 ms time constant. The signal gate requires at least 26 dB
fall within 1.8 seconds. Keep resolution constant after startup.

For schema-2 compatibility, current producers publish `SPECTRUM_UPPER_READY` for
the full-range branch, its clock in `upper_end_frame`, and zero `bass_end_frame`.
The legacy `SPECTRUM_BASS_READY` bit and bass-clock fields remain supported by
receivers; no current producer sets that bit. Do not change the wire layout,
plugin identities, project schema or parameter state.

The sole serialized active producer writes a sequentially consistent atomic
versioned frame. Readers make at most two attempts. Each UI/control consumer
retains its last complete frame on ordinary contention and discards it on clear
epoch changes. Clear invalidates caches before payload publication. Prepared
attachments and prepared enable changes never write a live handle. Activation
invalidates stale rate/source data; compatible inheritance transfers complete
ring, pending jobs, release and readiness history, and retargets handles
without final-owner retirement on the callback. Track, layer, beat, Main/Output
and Master scopes retain existing effect discovery and bindings.

The shared graph keeps ephemeral per-graph dB state, with 35 ms UI-time easing
and 16 ms repaint requests. Duplicate layout passes do not advance easing.
All frequencies use the same ordinary easing throughout playback. There is no
readiness-triggered blend, delayed fine target or retained outgoing contour.
Missing/all-zero readings, bypass, hiding and stopped/paused masking clear
immediately. Rate/generation changes and skipped graph frames reset display
history. Silent ready frames must not request collection repaints.

Draw a shape-preserving cubic contour bounded by adjacent displayed values.
Include every measured center and an interpolated midpoint. At limited logical
width, retain the minimum and maximum per column in original order, using
64–511 columns and at most 1025 points. This preserves visible peaks and valleys
without claiming extra measured resolution. Peaks narrower than a display pixel
can still coincide. Keep graph fills, bounds, controls and interaction unchanged.
DAW, CLAP and VST3 call the same renderer.

CLAP shares prepared frames. VST3 uses distinct schema-2 request/reading IDs and
an explicit 668-value count. Slots 0–141 retain scalar/band meters and rate;
142–653 contain magnitudes; 654 schema 2; 655 branch validity; 656–659 generation;
660–663 upper clock; 664–667 bass clock. Each u64 uses four exact 16-bit float
words. Validate a complete frame before mutating any received meter. Unsupported
peers use the legacy 206-value message with its spectrum masked. New peers mask
legacy spectrum rather than reinterpret old bins. See [wire compatibility](../architecture/all-effect-plugin-builds.md#spectrum-wire-compatibility).
Parameter IDs, identities, saved state, latency and Undo remain unchanged.

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
