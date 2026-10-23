# Filter

Filter shapes stereo audio with a static cutoff. Add it from **EQ & Filters** in
the Effects catalog to an audio/MIDI track, Master, or a MIDI instrument layer
or Output. It has the same Cutoff and Out controls as Auto-filter.

| Section | Control | Range or choices | Default |
| --- | --- | --- | --- |
| Cutoff | Shape | Low-pass, High-pass, Band-pass, Notch | Low-pass |
| Cutoff | Frequency | 20 Hz–20 kHz | 1 kHz |
| Cutoff | Resonance | Q 0.1–10 | Q 0.707 |
| Out | Level | Silence (−∞) to +12 dB | 0 dB |
| Out | Dry/Wet | 0–100% | 100% |

Low-pass keeps lower frequencies. High-pass keeps higher frequencies. Band-pass
keeps a range around Frequency. Notch removes a range around it. Resonance changes
the emphasis or width around that frequency. At 44.1 kHz, the highest processed
cutoff is about 19.85 kHz.

Dry/Wet blends the original and filtered audio. Level adjusts that blend.
Header power bypasses the unit. Changes transition smoothly. Double-click a knob
to reset it. Compact mode keeps the Cutoff and Out labels; double-click the header
to collapse the unit horizontally.

Frequency, Resonance, Level, and Dry/Wet accept shared Envelope, LFO, and Macro
sources from the same MIDI instrument, including Output. Use the secondary-color
header dot to open [modulation connections](../modulation/README.md). Per note
sources are unavailable for this effect. Audio tracks and Master have no
instrument modulation sources. Filter has no internal LFO controls or sweep.

Filtering adds no processing latency. Ringing stays within the arranged export
range; export does not append an extra tail. Power, controls, connections and
collapse state are saved with the project. Undo and Redo apply to changes.
