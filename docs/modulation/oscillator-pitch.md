# Oscillator pitch modulation

Use Envelope, LFO, or Macro to change an oscillator's pitch while retaining its base Pitch setting.

## Connect pitch modulation

1. Add Oscillator and a modulation source to a MIDI instrument.
2. Click the secondary-color dot in the Oscillator header.
3. Add a connection with Plus.
4. Select its Source and an Osc N · Pitch destination.
5. Set Amount in semitones. Each connection spans −48 to +48 semitones; zero is neutral.

Positive values raise pitch and negative values lower it. Twelve semitones equal one octave. The amount control steps in semitones; existing fractional values remain supported. Shift slows dragging while retaining those steps.

A Per note Envelope gives each voice its own pitch contour. Retrigger/Legato Envelopes, LFOs, and Macros provide shared movement. Main sources are also available within the same instrument.

Add more connections for other oscillators or combine sources on one Pitch control. Power disables a connection; right-click removes it. Removing an oscillator removes its pitch connections. Reordering oscillators keeps their connections attached.

## Read pitch movement

The range arc shows possible movement. Two live dots show the lowest and highest active voice transpose; they coincide when all voices have the same value. Hover for fractional values and values beyond the visible knob range. The ordinary pointer keeps the base Pitch.

Pitch modulation does not set note length or amplitude. Use [Amplitude and Envelope](../instruments/amplitude.md) for those controls. See [modulation](README.md) for connection editing.
