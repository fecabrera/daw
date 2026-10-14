# OTT

OTT is a three-band upward/downward compressor. Add it from the Effects catalog by double-click, Enter with a selection, or a library drag. Each instance uses the shared header, whole-unit bypass, selection, group reorder, remove menu, compact mode, and saved collapse behavior.

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

Each band has a thin signed gain-change meter in its upper padding. Center is 0 dB, left is reduction, and right is boost. The rail covers ±24 dB; its tooltip/accessibility value retains the actual signed reading, including larger reduction. It reports compression before Input/Output balance, Dry/Wet, and Level; Input affects detection but is not included in the displayed gain. Off reads zero. Meters display zero when playback is stopped/paused or no output device exists and do not change saved state. Compact mode retains footers and hides controls/meters.

## Processing

`daw-ott` is a reusable stereo effect behind the portable unit API, with private FunDSP filters and RMS detectors. It implements an OTT-style curve, not an exact emulation of a third-party unit. No dependencies beyond the existing workspace libraries are added.

Three fourth-order Linkwitz–Riley splitters use pairs of Butterworth low/high-pass sections. First split at Low/Mid, then split the upper branch at Mid/High. The low branch passes through the upper crossover's summed low/high response to match its all-pass phase. Summed bands have flat magnitude with neutral gains. The dry path uses that same phase response so partial Dry/Wet does not cancel at the crossover frequencies. Dry/Wet at 0% retains crossover phase but no band gains/compression; whole-unit bypass passes the original samples. Crossovers introduce frequency-dependent phase, with no lookahead or sample delay.

Each band uses a stereo-linked feed-forward detector. Peak takes the larger instantaneous absolute channel level; RMS takes the larger of two private exponential RMS detectors with 10 ms half-time. Both detectors keep running; mode changes blend over 5 ms. One gain envelope per band scales both channels equally.

Fixed curves apply upward 4:1 compression below −36 dBFS and downward 10:1 compression above −18 dBFS. Each threshold has a fixed 6 dB soft knee. Between those knees, gain is unity. Boost is capped at +24 dB and reduction at −60 dB. Digital silence generates no audio; silent detectors release hidden boost to unity. Threshold, ratio, knee, separate upward/downward amounts, and automatic makeup controls are not exposed in this version.

Attack applies when the requested signed gain change grows in magnitude or changes polarity. Release applies when it moves toward unity with the same polarity. Both are one-pole time constants in dB, reaching approximately 63% of a step over the selected time. Depth and the band power blend scale the resulting envelope. Dry/Wet linearly blends phase-matched dry with processed bands; Level scales the complete blend.

Parameters and bypass changes transition over 5 ms. Frequency and timing controls interpolate logarithmically. A crossover edit restarts both frequency ramps together to preserve their ordering during consecutive edits. Compatible plan changes, reordering, and presentation edits retain crossover/detector/envelope history and unrelated ramps by instance ID. Stop and seek reset state; pause retains it; loops/gaps continue processing. Playback, offline export, and random-access rendering use the same sequential processor, before track gain/pan. Crossover filters have a short decay after input ends; there is no delay/reverb tail extension or lookahead.

Processing, reset, and retargeting do not allocate, block, log, or perform I/O. Three atomic meter values per instance share a handle prepared on the control thread and retired with the renderer. Non-finite input becomes zero; pathological filter and detector inputs are bounded separately, and outputs stay within the finite float range.

State uses `daw.ott`, version 1. Settings, presentation, and IDs survive save/open, Save As, and processing-rate changes. Track duplication creates new instance IDs. Future versions retain their opaque state and pass through using the shared missing-effect behavior.
