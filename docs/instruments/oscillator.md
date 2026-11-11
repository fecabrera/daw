# Oscillator

Oscillator generates tones for MIDI notes. Add it from Sources in the Instrument components catalog to a regular layer. A layer can use Oscillator or Sampler, not both.

## Controls

| Control | Range or choices | Default |
| --- | --- | --- |
| Waveform dropdown | Sine, Saw, Square, Triangle | Sine |
| Pitch | −48 to +48 semitones | 0 |
| Cents (each source and Out) | −100 to +100 cents | 0 |
| Unison | Off, 2, 4, 8 voices | Off |
| Detune | 0–100 cents | 10 cents |
| Width | 0–100% | 100% |
| Gain (each source) | Silence through +12 dB | −12 dB |
| Level (whole unit) | Silence through +12 dB | 0 dB |
| Output Pitch (whole unit) | −48 to +48 semitones | 0 |

The last column, **Out**, contains Level, Pitch, and Cents. Level controls the combined output of all oscillator sources before Amplitude and layer effects. Sources still sum independently without automatic normalization; adjust Level to balance their combined volume. Each layer saves its own Level.

The gap after the unit holds its [stereo output meter and held-peak indicators](../effects/output-meter.md). It measures the sum across notes after source gains, unit Level and note velocity, before Amplitude and layer processing. Click the rail to reset only the held peak. The meter stays visible in compact/collapsed views; unit power-off clears it.

Use Plus inside the unit, before Out, to add another oscillator. Each source has independent controls and power. Drag unused strip space to reorder sources. The insertion line stays within the oscillator unit’s height. Right-click a source or its knob to remove it. Removing the last source leaves the layer silent.

Each source shows one cycle of its selected waveform above the Waveform dropdown. Choose Sine, Saw, Square or Triangle from that dropdown. The graphic stays visible while stopped or playing. Pitch changes the cycle duration, so it does not change the number of cycles shown. The preview shows the source shape at a fixed height; it excludes Gain, modulation, note phases, other sources, Amplitude and effects. A powered-off source keeps its shape in grey.

Unison and Detune share one input group. Choose the voice count on the left; drag or type Detune on the right. Detune spreads the voices from below to above the source pitch by the selected cents amount. Width spreads them between the stereo channels; zero centers all voices. Detune and Width are disabled when Unison is Off. Unison voices share the source Gain and are averaged before other sources are summed.

Cents adds fine tuning to Pitch. There are 100 cents in one semitone. Out Cents tunes all oscillator sources together.

Drag Pitch, Cents, Width, and Gain knobs to adjust them, hold Shift for finer movement, or use arrow keys. Double-click restores a default.

Use [pitch, Cents, Width, gain, and Level modulation](../modulation/oscillator-pitch.md) for vibrato, pitch contours, or independent oscillator level movement. Use Amplitude with an [Envelope](envelope.md) for level shaping. Turning the Oscillator header off silences all its sources while retaining individual settings.

Select **Level** in the Oscillator connections to modulate the combined output. Amount spans −24 to +24 dB. Envelope, LFO, and Macro sources can control it, including sources in Output. Each note has its own processed Level, limited to +12 dB; a base Level of −∞ stays silent. The knob shows the standard assignment marker, range arc, and live voice extrema. Level modulation does not extend a note; use Amplitude for note release.

Output Pitch transposes every oscillator source together and preserves their tuning intervals. Each source's Pitch remains independent. The output knob sits below Level and saves with the layer. Select **Pitch** in the Oscillator modulation destinations to control all sources; **Osc N · Pitch** controls one source. Amount spans −48 to +48 semitones. Envelope, LFO, and Macro sources can control output Pitch, including shared sources in Output. The standard marker, range arc, and live dots show its modulation. Pitch changes are smoothed and retain oscillator phase.
