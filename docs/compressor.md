# Compressor

Double-click **Compressor** in the Effects catalog to append an independent instance to the selected audio or MIDI track. It uses the same unit header, power/bypass switch, selection outline, header reordering, removal menu, compact lane control, and saved horizontal collapse as the other effects.

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

The third Out row is a read-only gain-reduction meter. It reports the compression applied before Dry/Wet and output Level. Its horizontal scale covers 0–24 dB; its numerical reading retains larger values. Zero means no reduction. The meter reads a prepared atomic handle from the actual processor, independent of track/master output meters. It displays zero when the transport is stopped/paused or there is no output device. Meter updates never modify saved settings.

## Processing

`daw-compressor` is a portable stereo effect with no DAW, UI, or device dependency. Its detector uses private FunDSP meter state. It adds no latency and has no output tail after input silence.

This is a feed-forward downward compressor. Peak uses the larger instantaneous absolute channel level. RMS keeps an exponential mean-square detector per channel, with a fixed 10 ms half-time, and uses the larger channel RMS level. Both detector histories run continuously. A shared gain-reduction envelope scales both channels equally, preserving their relative balance.

Threshold and Ratio define the static compression curve. Knee is a width centered on Threshold: zero gives a hard knee; positive values join the uncompressed and compressed slopes with a continuous quadratic curve. Attack applies when the required reduction increases; Release applies when it decreases. Both are one-pole time constants on gain reduction in dB, reaching approximately 63% of a step over one configured time constant. They are response speeds rather than fixed input delays. Ratio 1:1 gives no compression after existing reduction has released.

Dry/Wet blends the original and compressed signals linearly. Level scales the complete blend. Header off bypasses compression, Dry/Wet, and Level and passes the unscaled dry input. Detector processing continues while bypassed so re-enabling retains valid history. Mode, bypass, and parameter changes transition over 5 ms; timing controls interpolate logarithmically. Compatible edits, presentation changes, and reordering retain detector/envelope history and unrelated ramps by instance ID. Stop and seek reset state; pause retains it; loops and gaps keep processing history. Audio and MIDI track effects remain before track gain/pan, with the same processor used for live playback, export, and random-access rendering.

Processing, reset, and compatible retargeting do not allocate, perform I/O, or acquire locks. Meter handles are attached on a control thread and are retired with their renderer. Non-finite input samples become zero; extreme finite output is bounded to the finite float range. RMS detector input is bounded separately to prevent square overflow.

State uses type ID `daw.compressor`, version 1. Core validation rejects non-finite/out-of-range controls atomically. Settings, IDs, and presentation survive save/open, Save As, and project rate changes. Track copies receive new effect IDs. Unknown future versions retain their opaque payload and bypass with the shared unavailable-effect warning. Lookahead, automatic makeup gain, external sidechain routing, and detector high-pass controls are outside this version.
