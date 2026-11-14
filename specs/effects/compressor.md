# Compressor

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/effects/compressor.md).

Double-click **Compressor** in Timeline or Instrument’s Effects catalog to append an independent instance to the selected audio/MIDI track or Master. It uses the same unit header, power/bypass switch, selection outline, header reordering, removal menu, compact lane control, and saved horizontal collapse as the other effects.

## Controls

A 270-point Curve strip precedes three 90-point control strips. The control strips have three aligned rows. Their footers read **Comp**, **Response**, and **Out**. Compact mode retains the header and control footers and hides Curve. There are no separate strip power switches.

| Comp | Response | Out |
| --- | --- | --- |
| Threshold | Attack | Level |
| Ratio | Release | Dry/Wet |
| Curve | Knee | Peak/RMS |

| Control | Range | Default |
| --- | --- | --- |
| Threshold | −60–0 dBFS | −18 dBFS |
| Ratio | 1:1–20:1 | 4:1 |
| Curve | −100%–+100%, bipolar | 0% |
| Detection | Peak, RMS | Peak |
| Attack | 0.1–200 ms, logarithmic | 10 ms |
| Release | 10–2000 ms, logarithmic | 100 ms |
| Knee | 0–24 dB | 6 dB |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |

Peak/RMS uses the shared bounded two-way knob, matching Auto-filter's Hz/Note: upper-corner labels, accent color on the selected label, secondary color on the other label, and one neutral arc between the options. There are no markers, accent value arc, or cyclic wrapping. All editable controls reuse shared drag, Shift fine adjustment, arrow keys, tooltips, accessibility, and double-click reset.

Curve contains a vertical gain-reduction monitor to the right of the plot, with a click-resettable held maximum. It reports the compression applied before Dry/Wet and output Level. Its scale covers 0–24 dB, filling downward from zero at the top; its numerical reading retains larger values. Zero means no reduction. The meter reads a prepared atomic handle from the actual processor, independent of track/master output meters. Current reduction displays zero when the transport is stopped/paused or there is no output device. Whole-unit off immediately clears the fill, peak line and numerical reading and suspends atomic publication through the [shared effect-meter lifecycle](../architecture/modular-audio-units.md#effect-meter-lifecycle). The panel masks queued current and held readings through `meters::unit_reading` and clears a held peak while power changes are pending. Detector and audio processing continue with their existing history and ramps; re-enabling resumes publication. Meter updates never modify saved settings.

`unit-core::PeakMeter` implements the shared `MeterStorage` contract: an instantaneous scalar cell plus a resettable atomic maximum in nonnegative unit-native values. Compressor uses it through `UnitMeter`, publishing each processed reduction so peaks between UI refreshes and while the panel is hidden/compact/collapsed are retained. Nonnegative finite float bits preserve numeric order for `fetch_max`; invalid readings and negative zero become positive zero. `MeterStorage::initialize` updates only the current reading when binding a prepared processor, so preparation and seek reconstruction cannot raise the held peak. `MeterStorage::clear` clears both values on bypass or disabled attachment, while ordinary zero publication retains the held maximum. Meter clones retain the existing detached-handle policy. Handles are allocated/bound on the control thread and retained until renderer retirement. Existing scalar compressor/renderer attachment APIs remain available through a control-thread adapter; effect discovery, stable-ID retention and binding traverse tracks, Master and instrument layers through the same shared path.

The shared `meters::gain_reduction_vertical` widget paints one horizontal 1-point line in `palette.secondary` across its 8-point-wide vertical rail. Map the held reduction onto the existing 0–24 dB scale and keep the stroke inside the rail at either endpoint. Paint no line at zero. Keep the accent fill and shared interaction/peak behavior, with a GR label above and numeric reading below the rail. Show one decimal below 100 dB and whole dB at larger readings to fit the column. The tooltip includes the actual held peak, including values above 24 dB. A primary click anywhere in the meter's normal hit area resets only the held atomic; no `Response::changed` or project edit is emitted. Guard disabled UI and window focus; secondary clicks do not reset. Expose the reset action and current reading through accessibility. Active processing can immediately capture a new peak after a click.

Held readings survive pause, stop, seek and compatible renderer edits/reordering through the stable meter handle. They are temporary monitoring state, not project or DSP-history state. New unit instances and recreated output devices start with empty holds; removal discards the monitor binding. The UI passes current transport-masked reduction and the shared peak handle together using `meters::GainReduction`; it does not simulate a peak from UI polling. Publication and reset perform no allocation, locking, waiting or file I/O in the callback.

## Processing

`daw-compressor` is a portable stereo effect with no DAW, UI, or device dependency. Its detector uses private FunDSP meter state. It adds no latency and has no output tail after input silence.

This is a feed-forward downward compressor. Peak uses the larger instantaneous absolute channel level. RMS keeps an exponential mean-square detector per channel, with a fixed 10 ms half-time, and uses the larger channel RMS level. Both detector histories run continuously. A shared gain-reduction envelope scales both channels equally, preserving their relative balance.

Threshold and Ratio define the static compression curve. Knee is a width centered on Threshold: zero gives a hard knee; positive values join the uncompressed and compressed slopes with a continuous quadratic curve. Attack applies when the required reduction increases; Release applies when it decreases. Both are one-pole time constants on gain reduction in dB, reaching approximately 63% of a step over one configured time constant. They are response speeds rather than fixed input delays. Ratio 1:1 gives no compression after existing reduction has released.

Dry/Wet blends the original and compressed signals linearly. Level scales the complete blend. Header off bypasses compression, Dry/Wet, and Level and passes the unscaled dry input. Detector processing continues while bypassed so re-enabling retains valid history. Mode, bypass, and parameter changes transition over 5 ms; timing controls interpolate logarithmically. Compatible edits, presentation changes, and reordering retain detector/envelope history and unrelated ramps by instance ID. Stop and seek reset state; pause retains it; loops and gaps keep processing history. Audio and MIDI track effects remain before track gain/pan, with the same processor used for live playback, export, and random-access rendering.

Processing, reset, and compatible retargeting do not allocate, perform I/O, or acquire locks. Meter handles are attached on a control thread and are retired with their renderer. Non-finite input samples become zero; extreme finite output is bounded to the finite float range. RMS detector input is bounded separately to prevent square overflow.

State uses type ID `daw.compressor`, version 1. Its optional `curve` field defaults to zero for older projects. Core validation rejects non-finite/out-of-range controls atomically. Settings, IDs, and presentation survive save/open, Save As, and project rate changes. Track copies receive new effect IDs. Unknown future versions retain their opaque payload and bypass with the shared unavailable-effect warning. Lookahead, automatic makeup gain, external sidechain routing, and detector high-pass controls are outside this version.

Master supports this unit through the same Timeline catalog and controls. Its independent saved chain processes the summed track bus before Master gain and output meters; see [Master effects](../architecture/modular-audio-units.md#master-effects-implementation--2026-10-14).

## Modulation — 2026-10-17

Compressor consumes shared control outputs through the [portable source and destination contract](../modulation/contract.md). Sources can be Retrigger/Legato Envelopes, LFOs, and Macro strips in its own MIDI instrument, including Main. Per note and foreign instrument sources fail core validation. Audio tracks and Master can retain unassigned connections but cannot reference instrument providers. Disconnect a connected Envelope from track effects before changing it to Per note. Disabled, removed, unavailable, or unassigned sources contribute neutral offsets through the common resolver.

| Destination | Signed Amount range | Application and final bounds |
| --- | --- | --- |
| Threshold | −60 to +60 dB | Add to Threshold; clamp to −60–0 dBFS. |
| Ratio | −19 to +19 ratio units | Add to the ratio's numerator, then clamp to 1:1–20:1. An offset of +2 changes 4:1 to 6:1. |
| Attack | −6 to +6 octaves | Multiply duration by `2^offset`; clamp to 0.1–200 ms. |
| Release | −6 to +6 octaves | Multiply duration by `2^offset`; clamp to 10–2000 ms. |
| Knee | −24 to +24 dB | Add to Knee width; clamp to 0–24 dB. |
| Level | −24 to +24 dB | Add to output level, capped at +12 dB. Exact base silence stays silent. |
| Dry/Wet | −1 to +1 | Add to the wet fraction; clamp to 0–1. |
| Curve | −200 to +200 percentage points | Add normalized offset; clamp to −1…+1. |

An octave here is a ratio of durations: +1 doubles the time, and −1 halves it. Amounts multiply the cached source value and weight. Same-target routes sum before the common 1 ms offset smoother and final bounds; total Level offsets are bounded to ±96 dB before gain conversion. Base controls, amounts/power, source enable, and bypass retain their existing 5 ms ramps. Ratio retains its base slope ramp and receives modulation in ratio units; Attack/Release offsets add in log-time space. Nonfinite standalone offsets contribute neutral targets. Zero effective offsets retain the existing unmodulated DSP path and arithmetic.

Peak/RMS and unit power remain discrete controls. The gain-reduction meter reports actual compression before Dry/Wet and Level, including the effect of modulated Threshold, Ratio, Knee, Curve and timing; its click action only resets the held peak. Bypass passes unscaled dry audio while both detectors, the reduction envelope, and modulation continue advancing. Modulation changes no base settings and adds no lookahead or latency.

The secondary-color header dot opens the shared Source/Destination/Amount strips on the right. Reuse connection Plus, Power/Mod N footers, context-menu removal, and strip reordering. Regular controls and the gain-reduction meter remain visible. Compact mode retains unit/connection footers and hides controls and the meter; horizontal collapse hides the body. The eight continuous parameter knobs reuse shared status dots, correlated-source range arcs, modulation action tooltips, click-to-open controls, and actual processed-value markers. Pointers and readouts retain base values. Normal and Amount controls derive from the same definitions in `parameter_knobs.rs`, including time scales, ratio steps, gain marks, Shift adjustment, and neutral-zero Amount reset.

Saved Compressor version 1 adds optional `modulation_open` and `modulations`, defaulting closed/empty for older projects. Visibility edits use the normal effect transaction and mark the project modified. Save/reopen, Save As, effect reorder, track duplication, processing-rate changes, and provider moves retain settings, source identities, and routes. Effect drag copies receive fresh route IDs. Invalid parameters, amounts, route IDs/targets, or source references reject the complete edit. Compatible renderer updates preserve detector/reduction history, in-flight base ramps, and route smoothing by stable ID. Seek/Stop reset state; Pause and loops retain existing transport behavior.

The host prepares route vectors and atomic snapshot handles before publication. `process_modulation` consumes cached providers once per destination sample; Compressor does not look up or advance a source. `ModulationSmoother` applies its offsets. The eight-reading snapshot carries Threshold dBFS, Ratio, Attack/Release ms, Knee dB, Level dB, Dry/Wet and Curve, in that order. Curve is appended so the original seven offsets retain their indices. Multiband Compressor keeps its original seven controls and readings; its internal Curve remains zero. No UI clock simulates modulation. Sampling, reset, snapshots, and compatible retargeting allocate and lock nothing in the callback. External sidechains, audio-derived modulation outputs, cross-track routes, and new detector controls remain outside this change.

## Standard output monitoring

The Out strip alone adds the [standard effect output meter](../architecture/modular-audio-units.md#standard-effect-out-strip-meter), including the shared held sample-peak indicator and independent click reset. Keep control positions, strip widths, unit height, and existing reduction/band monitors unchanged. Hide the output rail in compact and collapsed modes. Meter final stereo audio through `LevelMonitor<3>::sample_output`; use separate prepared handles from reduction and signed band telemetry. Bind through tracks, Master and instrument layers, retain handles by stable effect ID, detach DSP clones, and clear all monitors on unit bypass. Monitoring must not allocate, lock, change audio, or modify saved state.

## Plugin delivery

This unit also has independent CLAP/VST3 previews. The [shared build contract](../architecture/all-effect-plugin-builds.md) defines identity, state, automation, timing, meters, packaging, CI, and acceptance limits.

## Interactive transfer curve — 2026-11-13

The shared unit-ui body adds a 270-point Curve section with the same background,
secondary grid/text, axis margins, unnumbered EQ-style handle, selection accent
and footer alignment as the other graph units. Input and output axes span
−72…+12 dBFS linearly. The lower bound includes the minimum threshold's
full 24 dB knee. The configured curve uses Threshold, Ratio, Knee and Curve before
Dry/Wet and Level. Extract the existing soft-knee reduction formula unchanged
into `unit-core::compressor_gain_reduction_db`; the DSP and UI call the same
arithmetic at Curve 0. Nonzero Curve uses the shared shaped continuation described below; it adds no latency.

Place the handle at the knee midpoint: input equals Threshold and output equals
Threshold minus `(1 - 1/Ratio) * Knee / 8`. A hard knee puts it at the corner
(Threshold, Threshold). Vertical ratio gestures still change Ratio when Knee is
zero; the handle stays at that corner. Keep the knee midpoint within the graph
for every valid control bound. Horizontal movement adds input dB to Threshold,
clamped to −60…0. Vertical movement adjusts log Ratio, clamped to 1…20: up lowers
Ratio and opens the curve. Read modifiers on every frame: Command/Ctrl locks
Threshold (Ratio only); Option/Alt locks Ratio (Threshold only). Both modifiers
lock both values. Always consume both pointer deltas, including locked axes, so
pressing or releasing a modifier mid-drag cannot apply earlier motion or jump.
Shift reduces only future movement tenfold. Press and
modifier-only frames retain values. Tooltip/accessibility identify both controls.

Keep a single two-parameter draft, independent of project state. DAW knob readouts
follow the draft; coalesced graph-preview plans use the existing background worker.
Release commits one effect transaction and one Undo/Redo step for both controls.
Escape, focus loss, disabled/compact UI, skipped frames or invalidated settings
cancel; external changes retain their values. No-op releases are not project edits.
Native bodies emit balanced host Threshold and Ratio gestures with per-parameter
changed flags. Retry cancellation rollback after bounded queue rejection. Host
undo grouping is outside the editor contract. Keep the existing three control
section indices, bindings, peak resets and strip positions relative to each other.
Standalone viewport width becomes 540 points; height remains unchanged.

The small live point follows actual Peak/RMS detector input on the configured
static curve. It does not simulate the attack/release envelope or modulated output;
the existing gain-reduction meter and processed knob markers retain those roles.
A prepared `LevelMonitor<3>` publishes the linked detector amplitude in cell 0
with instant attack and shared 300 ms release; cells 1/2 stay zero. It uses shared
UnitMeter enable/clear/clone/retirement behavior and prepared non-publishing
attachment. Reset clears it, compatible processor inheritance retains its history,
and preparation cannot overwrite the active handle. Sample processing and monitor
publication cannot allocate, lock, wait or retire final owners.

DAW discovery reuses stable effect group `BandMeter<3>` bindings across all effect
owners. The output getter masks paused/stopped readings; power masking hides stale
values immediately. Plugin telemetry reuses the previously unused group in the
existing 13-value message: value 4 carries detector amplitude (with its existing
held cell at 10). The meter message length stays unchanged. Parameter and state changes are specified below.
Old senders supply zero and show no point; old editors ignore these unused cells.
The shared `BandAudition::clear_live_preview` stop hook defaults to no-op; Compressor
clears only its curve cells. CLAP invokes it on serialized processing stop; VST3
invokes it before a stopped meter response. Independent output/reduction holds
remain intact. The point is hidden below −60 dBFS and clamped at the visible +12 dB
limit. Monitoring does not enter PCM, export settings or saved project data.

### GR inside Curve

Keep Curve's 270-point width and the 540-point standalone viewport. Reserve the
rightmost 28 points of the chart for the GR label, vertical rail and current
reading; narrow the plot by 24 points. Align the rail's top/bottom with the plot.
Reuse shared `GainReduction` masking, accessibility, tooltip and click reset and
`vertical_peak_marker` for held reduction, clamped inside the rail at either end.
The meter has no parameter binding and never marks a response or project changed.
Use `Bounds.graph.reduction_meter` for native reset and bypass adapters.
`Bounds.controls[2][2]` belongs to Detection in Out. Compact/collapsed views hide
the graph and its meter together.


## Post-knee Curve control — 2026-11-13

Curve occupies Comp's third row. Detection moves to Out's third row. Both DAW
and native editors call the same body. Curve uses the shared bipolar percentage
knob with Exp/Log endpoints, a zero reset and shared modulation indicators.

`compressor_shaped_gain_reduction_db` owns the audio/graph formula. Let
`s = 1 - 1/Ratio`, `m = 1 - s`, `E = Threshold + Knee/2`,
`d = input_db - E`, and `a = abs(Curve)/12`. At or below E, retain the existing
soft-knee formula. Curve zero returns that formula directly, preserving its
floating-point arithmetic. Ratio 1:1 remains unity.

Above E, reduction starts at `s*Knee/2`. For negative Curve, add
`d - m*expm1(a*d)/a`. Stop the exponential at `L = -ln(m)/a` and retain constant
reduction `s*Knee/2 + (-ln(m)-s)/a` beyond L. This caps the output slope at 1.
For positive Curve, add `d - m*ln1p(a*d)/a`. Evaluate the continuation in f64
and bound reduction to nonnegative values. Both shapes retain value and tangent
at the knee edge and monotonic output; neither amplifies above the input.
The handle stays at the knee midpoint, independent of Curve.

Append Curve to the base-ramp array at index 9 and to the shared offset/snapshot
arrays at index 7. Curve uses the existing 5 ms base ramp and 1 ms offset smoother.
Compatible changes retain detectors, reduction history and unrelated in-flight
ramps. No callback allocation, lock or I/O is added. The neutral path skips the
nonlinear continuation.

The native parameter ID is 109; Detection keeps 108 and all earlier IDs stay
unchanged. Append Curve to the fixed state payload. The plugin envelope is v2
(96 bytes); accept the known v1 88-byte envelope with Curve zero. Reject malformed,
future or out-of-range states before publication. Project settings remain v1.
The meter wire format stays at 13 values. This change does not add a Curve
control or modulation destination to Multiband Compressor.
