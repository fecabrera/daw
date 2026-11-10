# Harmonic Lift

Harmonic Lift adds harmonics in six overlapping frequency regions, then applies global drive and linked stereo maximization. Add it from **Effects → Distortion → Harmonic Lift**. It is also available as a [CLAP or VST3 preview](plugins.md).

Use **Harmonic Lift** for broad harmonic reinforcement, density and dynamics shaping in one unit.

The panel has nine strips: Input, Low, Low Mid, Mid, High Mid, Presence, High, Maximizer, and Out. The six frequency regions have fixed roles. They cannot be added, removed, or reordered.

## Input and branches

**Wet Input** adjusts only the processed signal, from −60 to +12 dB. The dry signal keeps its original level. Choose **Normal** or **HQ 2×**. HQ processes the nonlinear stages at twice the project rate to reduce aliasing. It can change the high-frequency sound. Both choices keep the same processing delay. Normal does not reduce CPU use in this version.

Each branch has an independent Power button, Frequency, and Gain. All branches start off. Gain runs from 0 to +24 dB; 0 dB is unity gain, so use Power to turn a branch off. The filtered, shaped branches are added to the wet input. They overlap and do not form an equalizer crossover.

| Branch | Frequency range | Reset |
| --- | --- | --- |
| Low | 20–250 Hz | 135 Hz |
| Low Mid | 250–500 Hz | 375 Hz |
| Mid | 500 Hz–2 kHz | 1.25 kHz |
| High Mid | 2–4 kHz | 3 kHz |
| Presence | 4–6 kHz | 5 kHz |
| High | 6–12 kHz | 9 kHz |

The first five branches emphasize a frequency region. High adds shaped high-pass content. The asymmetric shaping adds both even and odd harmonics.

## Maximizer and output

- **Drive** adds global saturation with high-frequency emphasis. At 0%, this stage passes the wet signal through.
- **Push** raises the signal into a linked stereo maximizer. At 0%, maximization is inactive. Higher values can reduce dynamics.
- **Limit Level** adjusts the maximized wet signal from −24 to 0 dB. It does not limit the dry signal or the final output.
- **Level** adjusts the final mixed output, from silence to +12 dB.
- **Dry/Wet** blends the original and processed signals.

The Maximizer rail shows gain reduction. The [output meter](output-meter.md) sits in the gap after the DAW unit. Standalone editors retain the horizontal Out rail. Click a held reading to reset it. Final Level, dry mixing and HQ filtering can produce peaks above Limit Level. Use a final Limiter when you need a controlled output ceiling.

With all branches off, Normal quality, and the remaining controls at their defaults, the signal is unchanged apart from processing delay. Power, parameter and quality edits use short transitions. Bypass keeps delay compensation active and clears the meters.

In the DAW, the continuous controls accept shared modulation. Frequency and Gain routes belong to a specific branch. Quality and branch Power are not modulation destinations. Standalone plugins expose all controls to host automation, but do not carry DAW modulation routes.
