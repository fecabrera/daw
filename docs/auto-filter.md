# Auto-filter

Double-click **Auto-filter** in Timeline or Instrument’s Effects catalog to append an independent instance to the selected audio/MIDI track or Master. It uses the shared unit panel, header power, saved horizontal collapse, compact lane control, selection border, header reordering, and removal menu. Changes use the shared core validation and project save format.

## Controls

Three strips have three aligned control rows: Cutoff, LFO, and Out. Cutoff and Out each use the standard 90-point width. LFO is one wider strip with two control columns, no internal divider, and one centered standard Power + LFO footer. The gap between its columns equals their left and right padding, matching the control inset in the single-column strips. The other footers read Cutoff and Out. The third Out row is empty. Compact mode retains these three footers.

| Section | Control | Range or choices | Default |
| --- | --- | --- | --- |
| Cutoff | Shape | Low-pass, High-pass, Band-pass, Notch | Low-pass |
| Cutoff | Frequency | 20 Hz–20 kHz, logarithmic | 1 kHz |
| Cutoff | Resonance | Q 0.1–10, logarithmic | Q 0.707 |
| LFO, left | Wave | Sine, Triangle, Saw up, Saw down, Square | Sine |
| LFO, left | Rate mode | Hz, Note | Note |
| LFO, left | Rate | 0.01–20 Hz; or 4 bars, 2 bars, 1 bar, 1/2 through 1/64 with straight, dotted (D), and triplet (T) forms | 1/4 note; retained Hz value 2 Hz |
| LFO, right | Strength | 0–6 octaves | 1 octave |
| LFO, right | Sync phase | −180° to +180° | 0° |
| LFO, right | Stereo phase | −180° to +180° | 0° |
| Out | Level | Exact silence (−∞) to +12 dB | 0 dB |
| Out | Dry/Wet | 0–100% | 100% |

Filter Shape and LFO Wave use the shared diagram selectors. Their diagrams show the four filter responses and five waveforms; the active choice uses the accent color. Names remain in tooltips and accessibility. Rate mode uses a bounded two-position knob with Hz at the upper left and Note at the upper right. Its pointer turns toward the selected label and never wraps. The selected mode label uses the accent color; the other choice uses secondary text color. These labels represent choices, not minimum/maximum values. Keep a neutral arc between the choices, with no markers or accent value arc. Retain the knob face and pointer.

The LFO footer switch stops modulation and leaves the static filter active at Frequency and Resonance. Its parameters remain editable and saved. Header power bypasses the whole unit. Both switches default to on. Rate mode changes retain the separate Hz and note values. Both rate knobs are bounded and show minimum/maximum labels without wrapping. Note rate has nine markers for straight durations only (4 bars through 1/64). Dotted and triplet choices remain selectable without markers. Strength has reference marks at each whole octave from 0 through 6, while retaining continuous adjustment and 0.1-octave keyboard steps. Note positions run from slowest (dotted 4 bars, labeled “4b D”) to fastest (triplet 1/64, labeled “1/64 T”), retaining all 27 durations and the 1/4-note reset. Knobs reuse shared dragging, Shift fine adjustment, arrow keys, double-click reset, bounds, tooltips, and accessibility.

## Processing

`daw-auto-filter` is a portable stereo effect with no UI or DAW dependencies. It uses FunDSP's Simper state-variable coefficients with independent channel integrators. Low-pass and High-pass have 12 dB/octave asymptotic slopes; Band-pass is normalized to unity at its center and Q controls bandwidth. Notch removes the center frequency. Q also controls Low/High-pass resonance. Processing adds no latency.

The bipolar LFO sweeps around the Frequency control: `cutoff = frequency × 2^(Strength × LFO)`. A 1-octave sweep spans half to twice the center frequency. DSP clamps cutoff to 20 Hz–the smaller of 20 kHz and 45% of the processing rate. This leaves a safe margin below Nyquist without changing saved controls.

Note mode derives cycles from project sample frames and tempo in 4/4. Hz mode derives cycles from project seconds and ignores tempo. Sync phase offsets both channels from that transport position. Stereo phase offsets the right channel relative to the left: positive advances the right waveform, negative delays it. ±180° are equivalent opposite-phase endpoints. Sine and Triangle start at their rising zero crossing; Saw up starts at −1, Saw down at +1, and Square at +1. A 1 ms modulation smoother softens waveform edges; the phase controls describe the target waveform before that smoothing.

Live parameter changes, filter/wave choices, LFO power, output gain, Dry/Wet, and whole-unit bypass transition over 5 ms. Frequency smoothing uses logarithmic space. Rate or tempo changes take the shorter circular path to the new transport phase over 5 ms. Seeks, starts at a saved position, and loop wraps use the destination transport phase. Filter history continues across loops and gaps; seek/Stop reset it, pause retains it. Compatible live updates retain integrators and in-flight transitions by instance ID. Reordering keeps those histories. Standalone callers can provide frame/tempo through `process_at`; the shared `EffectUnit::process_sample` fallback advances at 120 BPM.

Dry/Wet blends dry and filtered samples linearly, then Level scales the blend. Header off passes the unscaled dry signal. Bypassed processing continues so re-enabling has valid filter history. Playback, sequential export, and random-access rendering use the same processor and host frame/tempo. Export retains the existing arranged duration; no extra resonant tail is appended.

Saved state uses type ID `daw.auto-filter`, version 1. Core rejects invalid finite ranges, note-rate indices, and versions atomically. IDs and all controls/presentation states survive save/open and rate changes; track duplication creates new IDs. Unknown/future-version effect payloads remain opaque and bypassed with the existing unavailable-effect warning. Processing, reset, and compatible retargeting do not allocate or perform I/O.

Master supports this unit through the same Timeline catalog and controls. Its independent saved chain processes the summed track bus before Master gain and output meters; see [Master effects](modular-audio-units.md#master-effects-implementation--2026-10-14).

## External envelope modulation

The header's 10-point icon-free dot toggles the modulation strips on the right while keeping regular controls visible. It reuses the enable control's painting and 22-point click target, but uses the theme's secondary color. Filled means expanded; hollow means collapsed. Hover and keyboard focus show a circular halo. Each strip has Source, Parameter, a signed Amount knob, and a Power/Mod N footer. The final Plus strip adds a connection; right-click a strip to remove it. New connections have no source, zero amount, and power on. Expansion and connections are saved with each effect. Toggling expansion marks the project modified; missing `modulation_open` fields default closed. The shared strip renderer and header interaction guard are also used by Amplitude.

Sources are shared Envelope instances (Retrigger or Legato) in this effect's MIDI instrument. Per-note sources require a per-voice destination and are not available here. Audio tracks and Master have no instrument-envelope sources. Disabled/removed sources and disabled connections contribute neutral offsets; Envelope state continues advancing while disabled.

Supported targets are Frequency (±6 octaves), Resonance (±10 Q), Level (±24 dB), and Dry/Wet (±1). Sum contributions before applying existing target bounds. External Frequency offsets add to the independent internal LFO in log-frequency space. Smooth external offsets over 1 ms and live amount/power edits over 5 ms. The public `process_modulated_at` API accepts four offsets without DAW dependencies. Offline playback/export and sequential transport processing use the same prepared envelope values.

Saved Auto-filter version 1 adds an optional `modulations` list, defaulting empty for old projects. Each connection stores its ID, layer/envelope source reference, target parameter, amount, and power. Shared validation rejects invalid or foreign sources and incompatible per-note routing before publishing an edit. See [Envelope components](midi-patterns.md#envelope-components).

## Parameter modulation indicators

Frequency, Resonance, output Level, and Dry/Wet use the shared knob indicators described in [MIDI modulation indicators](midi-patterns.md#envelope-components). Assigned parameters have a secondary-color status dot: filled for active modulation and hollow for inactive connections. Hover lists source names, amounts, inactive reasons, range, and processed values. Click opens the saved modulation controls without changing the base value. The thin outer arc shows the possible modulation range, including signed envelope amounts and the internal bipolar LFO on Frequency. Frequency's range also respects the processing rate's cutoff limit.

Moving dots show actual processed values after ramps, external smoothing, and bounds. Frequency has left/right markers for its stereo LFO. The ordinary pointer, accent arc, and numeric readout retain the base value. Live markers appear during playback and use callback block snapshots; there is no separate UI LFO clock. Audio tracks and Master can show internal LFO modulation without an envelope source. Unit/source/route power and zero amounts determine active status. These indicators and their live state do not edit the project.
