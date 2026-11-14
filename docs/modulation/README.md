# Modulation

Modulation changes a parameter over time without changing its base setting. Envelope, LFO, Macro, and [audio sidechains](audio-sidechain.md) supply control values. A receiving parameter is called a destination.

## Make a connection

1. Add an Envelope or LFO to its matching [instrument modulation lane](../instruments/modulation-lanes.md). Add Macro to a regular layer or Output.
2. Open the receiving unit's modulation controls with the secondary-color dot in its header.
3. Click Plus to add a connection.
4. Select a Source and Destination, then turn Amount up or down.
5. Use the connection's power button to disable it, or right-click to remove it.

New connections are enabled but have no source and zero Amount. They do not affect the sound until assigned and adjusted. Connections and their visible layout save with the project.

## Choose sources and amounts

The Source dropdown starts with None, followed by a divider. Select LFO or Envelope to choose a provider from its instrument-wide lane. Regular layer and Output submenus contain their Macros. Groups with no compatible sources are hidden. The selected value includes the group name so repeated component names remain clear. Source and Destination selectors fill the space inside their strip.

Envelope, LFO and Macro sources stay within one MIDI instrument. Output sources can reach compatible layer, layer-effect, and track-effect parameters. Audio tracks and the project Master do not have instrument providers, but their effects can receive audio sidechains from tracks, groups and Returns. These appear after a second divider in Source.

Per note Envelopes control individual voices. Shared Retrigger/Legato Envelopes, LFOs, and Macros can feed shared destinations such as layer and track effects. Incompatible sources are unavailable in the picker. Self-connections and feedback loops are not allowed.

Positive Amount moves with the source; negative Amount reverses its direction. A unipolar source runs from zero to positive values. A bipolar LFO moves above and below zero. Several connections can affect one parameter, within its available range. At Amplitude, continuous sources such as LFO and Macro change level without controlling note release.

Amount uses the destination's units. Pitch uses semitones, level uses dB, and frequency or time can use octaves: +1 doubles the value, and −1 halves it. Double-click Amount to return it to zero. Shift gives finer adjustment.

## Read the indicators

Assigned knobs have a status dot. Filled means active; hollow means inactive. Click the dot to open connections and inspect their sources, amounts and state. The outer range arc shows possible movement, and live dots show processed values during playback. The main pointer and readout keep the base setting.

## Destination examples

This table gives examples, not a complete control list. Open a unit's guide for
all supported destinations and their limits. The unit's Destination dropdown
shows the controls available on that instance.

| Unit | Parameters |
| --- | --- |
| [Oscillator](../instruments/oscillator.md) | Pitch, Gain, Cents and Width for each source; unit Pitch, Level and Cents |
| [Sampler](../instruments/sampler.md) | Gain |
| [Granulizer](../instruments/granulizer.md) | Gain and continuous granular controls |
| [Noise](../instruments/noise.md) | Color, Width, Level |
| [Amplitude](../instruments/amplitude.md) | Gain shaping |
| [Envelope](../instruments/envelope.md) | Attack/Decay/Release time, Attack/Decay note fraction, Sustain, and curve strengths |
| [LFO](../instruments/lfo.md) | Depth, Rate |
| [Macro](../instruments/macro.md) | Value for each strip |
| [3-band EQ](../effects/equalizer.md) | Low/Mid/High Gain; Low/High Frequency and Q |
| [Drive](../effects/drive.md) | Volume, Gain, Tone, Color strength, Color frequency |
| [Bitcrusher](../effects/bitcrusher.md) | Drive, Resolution, Downsampling, Level, Dry/Wet; Resolution and Downsampling retain whole steps |
| [Filter](../effects/filter.md), [Auto-filter](../effects/auto-filter.md) | Frequency, Resonance, Level, Dry/Wet |
| [Compressor](../effects/compressor.md) | Threshold, Ratio, Attack, Release, Knee, Level, Dry/Wet |
| [Limiter](../effects/limiter.md) | Input, Release, Lookahead, Ceiling |
| [OTT](../effects/ott.md) | Low/High crossover; Low/Mid/High Depth, Input, Output, Attack, Release; Level, Dry/Wet |
| [Direction Mixer](../effects/direction-mixer.md) | Direction, Spread |
| [Stereo Spread](../effects/stereo-spread.md) | Lower/Upper Intensity, Lower/Upper Frequency |

Unit power, discrete mode/shape choices, and read-only meters are not destinations. See the [component guides](../README.md#instrument-components) and [effect guides](../README.md#effects) for parameter behavior and limits. Other compatible effects include [Multiband EQ](../effects/multiband-eq.md), [Multiband Compressor](../effects/multiband-compressor.md), [Drive v2](../effects/drive-v2.md), [Saturator](../effects/saturator.md), [Harmonic Lift](../effects/harmonic-lift.md), [Exciter](../effects/exciter.md), [Chorus](../effects/chorus.md), [Plate Reverb](../effects/plate-reverb.md), [Ambient Reverb](../effects/ambient-reverb.md), and [Shimmer Reverb](../effects/shimmer-reverb.md).
