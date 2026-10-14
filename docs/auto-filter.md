# Auto-filter

Double-click **Auto-filter** in Timeline's Effects catalog to append an independent instance to the selected audio or MIDI track. It uses the shared unit panel, header power, saved horizontal collapse, compact lane control, selection border, header reordering, and removal menu. Changes use the shared core validation and project save format.

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
