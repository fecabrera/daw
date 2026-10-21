# Oscillator pitch and gain modulation

Use Envelope, LFO, or Macro to change an oscillator's pitch or gain while retaining its base knob settings.

## Connect pitch modulation

1. Add Oscillator and a modulation source to a MIDI instrument.
2. Click the secondary-color dot in the Oscillator header.
3. Add a connection with Plus.
4. Select its Source and an Osc N · Pitch destination.
5. Set Amount in semitones. Each connection spans −48 to +48 semitones; zero is neutral.

Positive values raise pitch and negative values lower it. Twelve semitones equal one octave. The amount control steps in semitones; existing fractional values remain supported. Shift slows dragging while retaining those steps.

A Per note Envelope gives each voice its own pitch contour. Retrigger/Legato Envelopes, LFOs, and Macros provide shared movement. Output sources are also available within the same instrument.

Add more connections for other oscillators or combine sources on one Pitch control. Power disables a connection; right-click removes it. Removing an oscillator removes its pitch and gain connections. Reordering oscillators keeps their connections attached.

## Read pitch movement

The range arc shows possible movement. Two live dots show the lowest and highest active voice transpose; they coincide when all voices have the same value. Hover for fractional values and values beyond the visible knob range. The ordinary pointer keeps the base Pitch.

Pitch modulation does not set note length or amplitude. Use [Amplitude and Envelope](../instruments/amplitude.md) for those controls. See [modulation](README.md) for connection editing.

## Connect gain modulation

Open the same Oscillator modulation controls and select **Osc N · Gain** as Destination. Set Amount from −24 to +24 dB. Positive amounts raise gain; negative amounts lower it. Zero is neutral. The amount knob uses the Gain knob’s steps and drag behavior.

A Per note Envelope shapes each note independently. LFO and Macro sources, including Output sources, can move several oscillator gains together. Add separate connections for each oscillator or combine several sources on one Gain target.

Gain modulation retains the base Gain setting. The processed gain is limited to +12 dB. A Gain setting of −∞ and a muted oscillator remain silent. Disabling a connection returns smoothly to the base gain. Gain modulation does not change note length; use Amplitude to control note release.

Assigned Gain knobs show the shared marker and possible-range arc. Click the marker to open connections. Live dots show the lowest and highest active voice gain, including gain/mute smoothing, before velocity, Amplitude, and layer mixing. Connections and their open state are saved with the project and follow oscillators when reordered, moved, or duplicated.
