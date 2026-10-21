# Amplitude

Amplitude controls the level and note-release shaping of one MIDI instrument layer. It follows the layer's Oscillator or Sampler and precedes layer gain/pan.

Level occupies one compact strip. Opening modulation adds connection strips beside it.

## Shape notes

1. In the Instrument components catalog, add Amplitude from Processors and an Envelope from Modulators.
2. Open Amplitude's modulation controls with the secondary-color header dot.
3. Add a connection, select the Envelope, and adjust Amount.
4. Set the Envelope's Attack, Decay, Sustain, and Release for the wanted contour.

A Per note Envelope shapes each voice independently. Shared Envelopes can shape voices together. LFO and Macro also control level, but do not create a note-release tail themselves. Without an active contour connection, the layer uses a short note gate.

Use a connection's power control to disable just that contribution. Turning Amplitude off bypasses its gain and modulation while retaining note gating. Removing it retains settings for restoration from the catalog. Only one Amplitude unit is allowed per regular layer, and Output cannot contain one.

See [Envelope](envelope.md) and [modulation](../modulation/README.md) for timing and connection controls.
