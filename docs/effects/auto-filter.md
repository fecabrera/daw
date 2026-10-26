# Auto-filter

Auto-filter combines a stereo filter with a repeating frequency sweep. Add it to an audio/MIDI track or Master from EQ & Filters in the Effects catalog.

[Filter](filter.md) provides the same Cutoff and Out controls with a static cutoff.

## Controls

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

Low-pass keeps lower frequencies, High-pass keeps higher frequencies, Band-pass keeps a range around Frequency, and Notch removes a range around it. Resonance changes the emphasis or width around that frequency.

Strength sets the internal LFO sweep around Frequency. One octave sweeps from half to twice the center frequency. Hz mode uses a fixed speed; Note mode follows tempo. Sync phase shifts both channels' cycle position. Stereo phase shifts the right channel relative to the left.

The internal sweep follows the timing of the audio arriving at Auto-filter, including processing delay from earlier Saturator or Limiter effects. This also applies across loop boundaries and on instrument Output.

The LFO footer switch stops the internal sweep while keeping the static filter active. Header power bypasses the complete unit. Dry/Wet blends the original and filtered signals; Level adjusts the blend's output.

## External modulation

Frequency, Resonance, Level, Dry/Wet, LFO strength, LFO sync phase, and LFO stereo phase accept shared Envelope, LFO, and Macro sources from the same MIDI instrument, including Output. Per note Envelopes are unavailable for this track effect. Use the secondary-color header dot to open [connections](../modulation/README.md).

LFO strength accepts signed amounts from −6 to +6 octaves. Both phase targets accept −180° to +180°. Offsets add to the base controls, then stay within their control limits. Sync phase moves both channels; Stereo phase moves the right channel relative to the left. The LFO switch stops the sweep while retaining these connections. Their markers appear inactive while the LFO is off.

External Frequency modulation combines with the internal sweep. The range arc shows possible movement, and two live Frequency dots show the stereo values. The base pointer stays unchanged. Audio tracks and Master can use the internal LFO without an external source.

Filter ringing is included within the arranged export range; export does not append an extra tail.

The output strip uses the [standard effect output meter and held-peak indicator](output-meter.md). Click its rail to reset only the held peak.

This effect also has a [standalone CLAP/VST3 plugin](plugins.md).
