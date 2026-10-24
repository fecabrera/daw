# Oscillator

Oscillator generates tones for MIDI notes. Add it from Sources in the Instrument components catalog to a regular layer. A layer can use Oscillator or Sampler, not both.

## Controls

| Control | Range or choices | Default |
| --- | --- | --- |
| Waveform | Sine, Saw, Square, Triangle | Sine |
| Pitch | −48 to +48 semitones | 0 |
| Gain (each source) | Silence through +12 dB | −12 dB |
| Level (whole unit) | Silence through +12 dB | 0 dB |
| Output Pitch (whole unit) | −48 to +48 semitones | 0 |

The last column, **Out**, contains Level and Pitch. Level controls the combined output of all oscillator sources before Amplitude and layer effects. Sources still sum independently without automatic normalization; adjust Level to balance their combined volume. Each layer saves its own Level.

Use Plus inside the unit, before Out, to add another oscillator. Each source has independent controls and power. Drag unused strip space to reorder sources. Right-click a source or its knob to remove it. Removing the last source leaves the layer silent.

Drag knobs to adjust them, hold Shift for finer movement, or use arrow keys. Double-click restores a default. Waveform diagrams show the available shapes.

Use [pitch, gain, and Level modulation](../modulation/oscillator-pitch.md) for vibrato, pitch contours, or independent oscillator level movement. Use Amplitude with an [Envelope](envelope.md) for level shaping. Turning the Oscillator header off silences all its sources while retaining individual settings.

Select **Level** in the Oscillator connections to modulate the combined output. Amount spans −24 to +24 dB. Envelope, LFO, and Macro sources can control it, including sources in Output. Each note has its own processed Level, limited to +12 dB; a base Level of −∞ stays silent. The knob shows the standard assignment marker, range arc, and live voice extrema. Level modulation does not extend a note; use Amplitude for note release.

Output Pitch transposes every oscillator source together and preserves their tuning intervals. Each source's Pitch remains independent. The output knob sits below Level and saves with the layer. Select **Pitch** in the Oscillator modulation destinations to control all sources; **Osc N · Pitch** controls one source. Amount spans −48 to +48 semitones. Envelope, LFO, and Macro sources can control output Pitch, including shared sources in Output. The standard marker, range arc, and live dots show its modulation. Pitch changes are smoothed and retain oscillator phase.
