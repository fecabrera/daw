# Compressor

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/effects/compressor.md).

Double-click **Compressor** in Timeline or Instrument’s Effects catalog to append an independent instance to the selected audio/MIDI track or Master. It uses the same unit header, power/bypass switch, selection outline, header reordering, removal menu, compact lane control, and saved horizontal collapse as the other effects.

## Controls

Three 90-point strips have three aligned rows. Their footers read **Comp**, **Response**, and **Out**. Compact mode retains the header and these footers. There are no separate strip power switches.

| Comp | Response | Out |
| --- | --- | --- |
| Threshold | Attack | Level |
| Ratio | Release | Dry/Wet |
| Peak/RMS | Knee | Gain-reduction meter |

| Control | Range | Default |
| --- | --- | --- |
| Threshold | −60–0 dBFS | −18 dBFS |
| Ratio | 1:1–20:1 | 4:1 |
| Detection | Peak, RMS | Peak |
| Attack | 0.1–200 ms, logarithmic | 10 ms |
| Release | 10–2000 ms, logarithmic | 100 ms |
| Knee | 0–24 dB | 6 dB |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |

Peak/RMS uses the shared bounded two-way knob, matching Auto-filter's Hz/Note: upper-corner labels, accent color on the selected label, secondary color on the other label, and one neutral arc between the options. There are no markers, accent value arc, or cyclic wrapping. All editable controls reuse shared drag, Shift fine adjustment, arrow keys, tooltips, accessibility, and double-click reset.

The third Out row is a gain-reduction monitor with a click-resettable held maximum. It reports the compression applied before Dry/Wet and output Level. Its horizontal scale covers 0–24 dB; its numerical reading retains larger values. Zero means no reduction. The meter reads a prepared atomic handle from the actual processor, independent of track/master output meters. Current reduction displays zero when the transport is stopped/paused or there is no output device. Whole-unit off immediately clears the fill, peak line and numerical reading and suspends atomic publication through the [shared effect-meter lifecycle](../architecture/modular-audio-units.md#effect-meter-lifecycle). The panel masks queued current and held readings through `meters::unit_reading` and clears a held peak while power changes are pending. Detector and audio processing continue with their existing history and ramps; re-enabling resumes publication. Meter updates never modify saved settings.

`unit-core::PeakMeter` implements the shared `MeterStorage` contract: an instantaneous scalar cell plus a resettable atomic maximum in nonnegative unit-native values. Compressor uses it through `UnitMeter`, publishing each processed reduction so peaks between UI refreshes and while the panel is hidden/compact/collapsed are retained. Nonnegative finite float bits preserve numeric order for `fetch_max`; invalid readings and negative zero become positive zero. `MeterStorage::initialize` updates only the current reading when binding a prepared processor, so preparation and seek reconstruction cannot raise the held peak. `MeterStorage::clear` clears both values on bypass or disabled attachment, while ordinary zero publication retains the held maximum. Meter clones retain the existing detached-handle policy. Handles are allocated/bound on the control thread and retained until renderer retirement. Existing scalar compressor/renderer attachment APIs remain available through a control-thread adapter; effect discovery, stable-ID retention and binding traverse tracks, Master and instrument layers through the same shared path.

The shared `meters::gain_reduction` widget paints one vertical 1-point line in `palette.secondary` across the existing 8-point rail. Map the held reduction onto the existing 0–24 dB scale and keep the stroke inside the rail at either endpoint. Paint no line at zero. The current accent fill and numeric reading keep their existing geometry and behavior. The tooltip includes the actual held peak, including values above 24 dB. A primary click anywhere in the meter's normal hit area resets only the held atomic; no `Response::changed` or project edit is emitted. Guard disabled UI and window focus; secondary clicks do not reset. Expose the reset action and current reading through accessibility. Active processing can immediately capture a new peak after a click.

Held readings survive pause, stop, seek and compatible renderer edits/reordering through the stable meter handle. They are temporary monitoring state, not project or DSP-history state. New unit instances and recreated output devices start with empty holds; removal discards the monitor binding. The UI passes current transport-masked reduction and the shared peak handle together using `meters::GainReduction`; it does not simulate a peak from UI polling. Publication and reset perform no allocation, locking, waiting or file I/O in the callback.

## Processing

`daw-compressor` is a portable stereo effect with no DAW, UI, or device dependency. Its detector uses private FunDSP meter state. It adds no latency and has no output tail after input silence.

This is a feed-forward downward compressor. Peak uses the larger instantaneous absolute channel level. RMS keeps an exponential mean-square detector per channel, with a fixed 10 ms half-time, and uses the larger channel RMS level. Both detector histories run continuously. A shared gain-reduction envelope scales both channels equally, preserving their relative balance.

Threshold and Ratio define the static compression curve. Knee is a width centered on Threshold: zero gives a hard knee; positive values join the uncompressed and compressed slopes with a continuous quadratic curve. Attack applies when the required reduction increases; Release applies when it decreases. Both are one-pole time constants on gain reduction in dB, reaching approximately 63% of a step over one configured time constant. They are response speeds rather than fixed input delays. Ratio 1:1 gives no compression after existing reduction has released.

Dry/Wet blends the original and compressed signals linearly. Level scales the complete blend. Header off bypasses compression, Dry/Wet, and Level and passes the unscaled dry input. Detector processing continues while bypassed so re-enabling retains valid history. Mode, bypass, and parameter changes transition over 5 ms; timing controls interpolate logarithmically. Compatible edits, presentation changes, and reordering retain detector/envelope history and unrelated ramps by instance ID. Stop and seek reset state; pause retains it; loops and gaps keep processing history. Audio and MIDI track effects remain before track gain/pan, with the same processor used for live playback, export, and random-access rendering.

Processing, reset, and compatible retargeting do not allocate, perform I/O, or acquire locks. Meter handles are attached on a control thread and are retired with their renderer. Non-finite input samples become zero; extreme finite output is bounded to the finite float range. RMS detector input is bounded separately to prevent square overflow.

State uses type ID `daw.compressor`, version 1. Core validation rejects non-finite/out-of-range controls atomically. Settings, IDs, and presentation survive save/open, Save As, and project rate changes. Track copies receive new effect IDs. Unknown future versions retain their opaque payload and bypass with the shared unavailable-effect warning. Lookahead, automatic makeup gain, external sidechain routing, and detector high-pass controls are outside this version.

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

An octave here is a ratio of durations: +1 doubles the time, and −1 halves it. Amounts multiply the cached source value and weight. Same-target routes sum before the common 1 ms offset smoother and final bounds; total Level offsets are bounded to ±96 dB before gain conversion. Base controls, amounts/power, source enable, and bypass retain their existing 5 ms ramps. Ratio retains its base slope ramp and receives modulation in ratio units; Attack/Release offsets add in log-time space. Nonfinite standalone offsets contribute neutral targets. Zero effective offsets retain the existing unmodulated DSP path and arithmetic.

Peak/RMS and unit power remain discrete controls. The gain-reduction meter reports actual compression before Dry/Wet and Level, including the effect of modulated Threshold, Ratio, Knee, and timing; its click action only resets the held peak. Bypass passes unscaled dry audio while both detectors, the reduction envelope, and modulation continue advancing. Modulation changes no base settings and adds no lookahead or latency.

The secondary-color header dot opens the shared Source/Destination/Amount strips on the right. Reuse connection Plus, Power/Mod N footers, context-menu removal, and strip reordering. Regular controls and the gain-reduction meter remain visible. Compact mode retains unit/connection footers and hides controls and the meter; horizontal collapse hides the body. The seven parameter knobs reuse shared status dots, correlated-source range arcs, source/amount tooltips, click-to-open controls, and actual processed-value markers. Pointers and readouts retain base values. Normal and Amount controls derive from the same definitions in `parameter_knobs.rs`, including time scales, ratio steps, gain marks, Shift adjustment, and neutral-zero Amount reset.

Saved Compressor version 1 adds optional `modulation_open` and `modulations`, defaulting closed/empty for older projects. Visibility edits use the normal effect transaction and mark the project modified. Save/reopen, Save As, effect reorder, track duplication, processing-rate changes, and provider moves retain settings, source identities, and routes. Effect drag copies receive fresh route IDs. Invalid parameters, amounts, route IDs/targets, or source references reject the complete edit. Compatible renderer updates preserve detector/reduction history, in-flight base ramps, and route smoothing by stable ID. Seek/Stop reset state; Pause and loops retain existing transport behavior.

The host prepares route vectors and atomic snapshot handles before publication. `process_modulation` consumes cached providers once per destination sample; Compressor does not look up or advance a source. `ModulationSmoother` applies its offsets. The existing seven-reading snapshot carries Threshold dBFS, Ratio, Attack/Release ms, Knee dB, Level dB, and Dry/Wet, in that order. No UI clock simulates modulation. Sampling, reset, snapshots, and compatible retargeting allocate and lock nothing in the callback. External sidechains, audio-derived modulation outputs, cross-track routes, and new detector controls remain outside this change.
