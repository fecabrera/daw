# Modulation

Modulation changes a parameter over time without changing its base setting. Envelope, LFO, and Macro components supply control values. A receiving parameter is called a destination.

## Make a connection

1. Add an Envelope, LFO, or Macro to a MIDI instrument layer or Main.
2. Open the receiving unit's modulation controls with the secondary-color dot in its header.
3. Click Plus to add a connection.
4. Select Source and Parameter, then turn Amount up or down.
5. Use the connection's power button to disable it, or right-click to remove it.

New connections are enabled but have no source and zero Amount. They do not affect the sound until assigned and adjusted. Connections and their visible layout save with the project.

## Choose sources and amounts

Sources stay within one MIDI instrument. Main sources can reach compatible layer, layer-effect, and track-effect parameters. Audio tracks and the project Master do not have instrument modulation sources.

Per note Envelopes control individual voices. Shared Retrigger/Legato Envelopes, LFOs, and Macros can feed shared destinations such as layer and track effects. Incompatible sources are unavailable in the picker. Self-connections and feedback loops are not allowed.

Positive Amount moves with the source; negative Amount reverses its direction. A unipolar source runs from zero to positive values. A bipolar LFO moves above and below zero. Several connections can affect one parameter, within its available range. At Amplitude, continuous sources such as LFO and Macro change level without controlling note release.

Amount uses the destination's units. Pitch uses semitones, level uses dB, and frequency or time can use octaves: +1 doubles the value, and −1 halves it. Double-click Amount to return it to zero. Shift gives finer adjustment.

## Read the indicators

Assigned knobs have a status dot. Filled means active; hollow means inactive. Hover to see sources, amounts, and inactive reasons. Click the dot to open connections. The outer range arc shows possible movement, and live dots show processed values during playback. The main pointer and readout keep the base setting.

## Available destinations

| Unit | Parameters |
| --- | --- |
| Oscillator | Pitch for each source |
| Sampler | Gain |
| Amplitude | Gain shaping |
| Envelope | Attack/Decay/Release time, Attack/Decay note fraction, Sustain, and curve strengths |
| LFO | Depth, Rate |
| Macro | Value for each strip |
| 3-band EQ | Low/Mid/High Gain; Low/High Frequency and Q |
| Drive | Volume, Gain, Tone, Color strength, Color frequency |
| Bitcrusher | Drive, Resolution, Downsampling, Level, Dry/Wet; Resolution and Downsampling retain whole steps |
| Auto-filter | Frequency, Resonance, Level, Dry/Wet |
| Compressor | Threshold, Ratio, Attack, Release, Knee, Level, Dry/Wet |
| Limiter | Input, Release, Lookahead, Ceiling |
| OTT | Low/High crossover; Low/Mid/High Depth, Input, Output, Attack, Release; Level, Dry/Wet |
| Direction Mixer | Direction, Spread |
| Stereo Spread | Lower/Upper Intensity, Lower/Upper Frequency |

Unit power, discrete mode/shape choices, and read-only meters are not destinations. See the individual [component and effect guides](../views/instrument.md) for parameter behavior and limits.
