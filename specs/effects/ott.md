# OTT

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/effects/ott.md).

OTT is a three-band upward/downward compressor. Add it from Timeline or Instrument’s Effects catalog by double-click, Enter with a selection, or a library drag. Each instance uses the shared header, whole-unit bypass, selection, group reorder, remove menu, compact mode, and saved collapse behavior.

## Controls

Low, Mid, and High each occupy one strip with two columns and equal inner/outer horizontal spacing. Each has one centered Power/name footer and no divider between its columns. Out has one column. Three control rows preserve the shared unit height.

| Low | Mid | High | Out |
| --- | --- | --- | --- |
| Freq · Depth | Empty · Depth | Freq · Depth | Level |
| Input · Output | Input · Output | Input · Output | Dry/Wet |
| Attack · Release | Attack · Release | Attack · Release | Peak/RMS |

| Control | Range | Default |
| --- | --- | --- |
| Low/Mid crossover | 20 Hz–2 kHz, logarithmic | 250 Hz |
| Mid/High crossover | 200 Hz–20 kHz, logarithmic | 2.5 kHz |
| Band Depth | 0–100% | 100% |
| Band Input | −24–+24 dB | 0 dB |
| Band Output | −24–+24 dB | 0 dB |
| Band Attack | 0.1–200 ms, logarithmic | 10 ms |
| Band Release | 10–2000 ms, logarithmic | 100 ms |
| Band compression | On/off | On |
| Level | Exact silence (−∞) to +12 dB | 0 dB |
| Dry/Wet | 0–100% | 100% |
| Detection | Peak, RMS | Peak |

Low frequency must remain below High frequency. Knob edits clamp the changed crossover to at least 1 Hz from the other crossover. Shared core validation rejects crossed/equal boundaries and non-finite or out-of-range values atomically. Mid derives its range from the two crossovers and has no frequency control.

Depth scales the signed compression gain in dB; 0% removes compression while retaining Input/Output gains. Input precedes detection/compression; Output follows compression. Band off bypasses compression only, keeps the band audible, and retains its Input/Output gains. This differs from the EQ's range-cut power buttons. Whole-unit off bypasses all processing and output gain with unscaled original input.

Peak/RMS reuses the bounded two-way mode knob, upper-corner labels, selected accent text, and neutral arc without markers or wrapping. All editable controls reuse the shared knob's drag, Shift sensitivity, keyboard, double-click reset, tooltip, and accessibility behavior.

Each band has a thin signed gain-change meter in its upper padding. Center is 0 dB, left is reduction, and right is boost. The rail covers ±24 dB; its tooltip/accessibility value retains the actual signed reading, including larger reduction. Enabled bands use accent fill for reduction and the palette's meter color for boost. Disabled bands use the palette's secondary color for either direction, including the remaining meter reading during the band bypass ramp. `gain_change` receives the individual band's enabled state and shares `band_color` with EQ's level rails; only the fill color changes. It reports compression before Input/Output balance, Dry/Wet, and Level; Input affects detection but is not included in the displayed gain. Whole-unit off clears all three readings and suspends atomic publication through the [shared effect-meter lifecycle](../architecture/modular-audio-units.md#effect-meter-lifecycle). The panel masks queued readings through `meters::unit_reading`. Re-enabling resumes publication; detector, crossover, and audio histories and ramps keep their existing behavior. Meters display zero when playback is stopped/paused or no output device exists and do not change saved state. Compact mode retains footers and hides controls/meters.

Each rail holds its greatest reduction and boost independently. `unit-core::BandMeter<3>` stores signed current values plus three reduction and three boost `PeakHold` values through `UnitMeter`; `PeakMeter` for Compressor/Limiter composes the same `PeakHold`. Negative signed readings raise only a reduction hold and positive readings raise only a boost hold, both stored as positive magnitudes. Neutral and invalid readings raise neither. Ordinary zero publication and DSP reset retain holds. Prepared attachment initializes current readings only; detached DSP snapshots and unbound worker reconstruction cannot publish peaks. Bypass clears all three current readings and six holds and suspends publication. An individual band bypass retains its holds and existing compression-ramp monitoring. Pause, stop, seek, compatible edits and reorder retain holds by stable effect identity; new instances and recreated output devices start empty. Holds are not saved or part of DSP history.

`meters::BandGainChange` supplies the shared `GainReduction` view for each rail. Reuse the scalar meters' reset guards, peak-line painting, tooltip and accessibility helpers. Paint two 1-point `palette.secondary` vertical lines across the existing 4-point rail: reduction moves left from center over 0–24 dB, and boost moves right over the same scale. Keep each stroke inside its rail endpoint and omit it at zero; retain the center reference line, signed fills, disabled-band colors and layout. The tooltip reports actual signed gain, held reduction and held boost beyond the display range. A primary click resets both holds for that band, leaving other bands unchanged; current values and settings stay unchanged, with no parameter-change response or project edit. Disabled UI and window focus loss prevent reset; secondary clicks do not reset. Active processing can immediately capture new peaks. Whole-unit off clears/masks all holds even while compact/collapsed or waiting for asynchronous publication.

## Processing

`daw-ott` is a reusable stereo effect behind the portable unit API, with private FunDSP filters and RMS detectors. It implements an OTT-style curve, not an exact emulation of a third-party unit. No dependencies beyond the existing workspace libraries are added.

Three fourth-order Linkwitz–Riley splitters use pairs of Butterworth low/high-pass sections. First split at Low/Mid, then split the upper branch at Mid/High. The low branch passes through the upper crossover's summed low/high response to match its all-pass phase. Summed bands have flat magnitude with neutral gains. The dry path uses that same phase response so partial Dry/Wet does not cancel at the crossover frequencies. Dry/Wet at 0% retains crossover phase but no band gains/compression; whole-unit bypass passes the original samples. Crossovers introduce frequency-dependent phase, with no lookahead or sample delay.

Each band uses a stereo-linked feed-forward detector. Peak takes the larger instantaneous absolute channel level; RMS takes the larger of two private exponential RMS detectors with 10 ms half-time. Both detectors keep running; mode changes blend over 5 ms. One gain envelope per band scales both channels equally.

Fixed curves apply upward 4:1 compression below −36 dBFS and downward 10:1 compression above −18 dBFS. Each threshold has a fixed 6 dB soft knee. Between those knees, gain is unity. Boost is capped at +24 dB and reduction at −60 dB. Digital silence generates no audio; silent detectors release hidden boost to unity. Threshold, ratio, knee, separate upward/downward amounts, and automatic makeup controls are not exposed in this version.

Attack applies when the requested signed gain change grows in magnitude or changes polarity. Release applies when it moves toward unity with the same polarity. Both are one-pole time constants in dB, reaching approximately 63% of a step over the selected time. Depth and the band power blend scale the resulting envelope. Dry/Wet linearly blends phase-matched dry with processed bands; Level scales the complete blend.

Parameters and bypass changes transition over 5 ms. Frequency and timing controls interpolate logarithmically. A crossover edit restarts both frequency ramps together to preserve their ordering during consecutive edits. Compatible plan changes, reordering, and presentation edits retain crossover/detector/envelope history and unrelated ramps by instance ID. Stop and seek reset state; pause retains it; loops/gaps continue processing. Playback, offline export, and random-access rendering use the same sequential processor, before track gain/pan. Crossover filters have a short decay after input ends; there is no delay/reverb tail extension or lookahead.

Processing, reset, and retargeting do not allocate, block, log, or perform I/O. Current and held meter values share a handle prepared on the control thread and retired with the renderer. The existing band discovery and binding path serves tracks, Master, regular layers and Main. Legacy scalar-array attachment APIs remain control-thread adapters; EQ still uses the current-cell array without peak capture. No monitoring handles are constructed or finally released in the callback. Non-finite input becomes zero; pathological filter and detector inputs are bounded separately, and outputs stay within the finite float range.

State uses `daw.ott`, version 1. Settings, presentation, and IDs survive save/open, Save As, and processing-rate changes. Track duplication creates new instance IDs. Future versions retain their opaque state and pass through using the shared missing-effect behavior.

Master supports this unit through the same Timeline catalog and controls. Its independent saved chain processes the summed track bus before Master gain and output meters; see [Master effects](../architecture/modular-audio-units.md#master-effects-implementation--2026-10-14).

## Modulation — 2026-10-17

OTT consumes shared Envelope, LFO, and Macro outputs from its own MIDI instrument, including Main, through the [shared source/destination contract](../modulation/contract.md). Per note and foreign-instrument providers are invalid. Audio tracks and Master may retain unassigned connections but cannot reference instrument providers. OTT publishes no modulation source. Peak/RMS, band power, and unit power remain discrete. Gain-change rails are monitors; their click action resets both held peaks for the corresponding band.

`OttSettings` and the portable `Ott` processor declare the same 19 `ModulationParameter::OTT` targets in Shared scope. Fixed band identities do not use list indices or optional target IDs. Level and Dry/Wet reuse the existing parameter identities. The common typed input frame adds 17 distinct OTT slots. Source discovery/publication, scope checks, route validation/resolution, amount/power ramps, the 1 ms additive offset smoother, route-history retargeting, and destination-owned connection controls remain shared code. OTT owns conversion, final bounds, crossover ordering, and audio processing.

| Targets | Signed Amount per connection | Conversion and final bounds |
| --- | --- | --- |
| Low/Mid crossover | ±6 octaves | Add in log-frequency space; 20–2000 Hz. |
| Mid/High crossover | ±6 octaves | Add in log-frequency space; 200–20,000 Hz, subject to the pair rule below. |
| Low/Mid/High Depth | ±1, displayed as ±100% | Add to each band depth; 0–1. |
| Low/Mid/High Input and Output | ±24 dB | Add to the corresponding gain; −24 to +24 dB. |
| Low/Mid/High Attack | ±6 octaves of duration | Add in log-time space; 0.1–200 ms. |
| Low/Mid/High Release | ±6 octaves of duration | Add in log-time space; 10–2000 ms. |
| Level | ±24 dB | Multiply the ramped base amplitude by the summed dB offset, bounded to ±96 dB before conversion; cap at +12 dB. Exact base silence remains silent. |
| Dry/Wet | ±1, displayed as ±100% | Add to the wet fraction; 0–1. |

Positive octave offsets double frequency or duration per octave; negative offsets halve it. Resolve and smooth all offsets once per destination sample. When either crossover offset is nonzero, bound Low first, then bound High and raise it to at least Low + 1 Hz if necessary. Never swap band identities. Apply this constraint after smoothing, including during changing base ramps. Neutral crossover offsets retain the original crossover arithmetic and valid saved base separation. Modulation changes no base settings. Band compression bypass retains Input/Output modulation; Depth and timing continue advancing while compression is bypassed. Unit bypass retains unscaled dry audio and advancing DSP/modulation histories.

The normal OTT knobs and Amount knobs use `parameter_knobs.rs` definitions. Reuse the native crossover and duration scales, bipolar band dB controls, percentages, and marked output Gain. The header's secondary dot opens the shared Source/Parameter/Amount strips to the right; regular controls and signed gain-change rails remain visible. Plus adds an enabled, unassigned, zero-amount connection; power, removal, reorder, tooltips, range arcs, assignment dots, and live readings reuse existing controls. Compact mode retains footers and hides controls/meters; horizontal collapse hides the body. Crossovers retain their existing manual 1 Hz clamp. Indicators use individual parameter bounds, and their crossover tooltip explains coupled ordering; live readings report the actual constrained values.

Version 1 adds optional `modulations` and `modulation_open`, defaulting empty/closed. Explicit null or malformed values fail. Common settings/core validation rejects unsupported parameters, target IDs, duplicate/nil route IDs, nonfinite/out-of-range amounts, and missing/foreign/per-note source references atomically. Compatible edits retain crossover, detector, gain-envelope, base-ramp, and offset history. Route additions/removals/reordering retain history by stable route identity through the existing prepared renderer transition. Provider moves remap OTT consumers; effect copies receive fresh route IDs while keeping source assignments. Save/reopen, Save As, track duplication, and all six processing-rate changes retain settings and references.

Processed telemetry follows `OTT` order: Low crossover, High crossover; then Depth/Input/Output/Attack/Release for Low, Mid, and High; then Level and Dry/Wet. Values use Hz, normalized fractions, dB, and milliseconds. The shared snapshot capacity is 19; destinations with fewer readings pad unused slots with NaN. Existing five-value accessors remain prefix adapters. Atomic storage and route vectors are prepared before publication and retained by destination identity; sampling, snapshots, reset, and compatible retargeting allocate and lock nothing in the callback. Meter readings include the actual modulated compression depth and detector/timing response before output balance, Dry/Wet, and Level. Playback and export use the same processor; latency, fixed curves, phase matching, band enable semantics, and export duration remain unchanged.

## Standard output monitoring

The Out strip alone adds the [standard effect output meter](../architecture/modular-audio-units.md#standard-effect-out-strip-meter), including the shared held sample-peak indicator and independent click reset. Keep control positions, strip widths, unit height, and existing reduction/band monitors unchanged. Hide the output rail in compact and collapsed modes. Meter final stereo audio through `LevelMonitor<3>::sample_output`; use separate prepared handles from reduction and signed band telemetry. Bind through tracks, Master and instrument layers, retain handles by stable effect ID, detach DSP clones, and clear all monitors on unit bypass. Monitoring must not allocate, lock, change audio, or modify saved state.
