# Oscillator

Oscillator generates tones for MIDI notes. Add it from Sources in the Instrument components catalog to a regular layer. A layer can use Oscillator or Sampler, not both.

## Controls

| Control | Range or choices | Default |
| --- | --- | --- |
| Waveform | Sine, Saw, Square, Triangle | Sine |
| Pitch | −48 to +48 semitones | 0 |
| Gain (each source) | Silence through +12 dB | −12 dB |
| Level (whole unit) | Silence through +12 dB | 0 dB |

The last column, **Out**, contains Level. It controls the combined output of all oscillator sources before Amplitude and layer effects. Sources still sum independently without automatic normalization; adjust Level to balance their combined volume. Each layer saves its own Level.

Use Plus inside the unit, before Out, to add another oscillator. Each source has independent controls and power. Drag unused strip space to reorder sources. Right-click a source or its knob to remove it. Removing the last source leaves the layer silent.

Drag knobs to adjust them, hold Shift for finer movement, or use arrow keys. Double-click restores a default. Waveform diagrams show the available shapes.

Use [pitch and gain modulation](../modulation/oscillator-pitch.md) for vibrato, pitch contours, or independent oscillator level movement. Use Amplitude with an [Envelope](envelope.md) for level shaping. Turning the Oscillator header off silences all its sources while retaining individual settings.
