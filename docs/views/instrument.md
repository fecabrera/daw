# Instruments and effects

An instrument creates audio from MIDI notes. Effects change audio from a track or the project Master. Use Instrument to build a MIDI instrument, and use the Effects catalog in Timeline or Instrument to add effects.

## Instrument components

| Component | Use |
| --- | --- |
| [Oscillator](../instruments/oscillator.md) | Generate Sine, Saw, Square, or Triangle tones. One Oscillator unit can contain several sources. |
| [Sampler](../instruments/sampler.md) | Play a selected audio region when MIDI notes arrive. |
| [Envelope](../instruments/envelope.md) | Shape a parameter over the life of a note. |
| [LFO](../instruments/lfo.md) | Repeat a control pattern at a fixed or tempo-related rate. |
| [Amplitude](../instruments/amplitude.md) | Set a layer's note level and connect level modulation. |
| [Macro](../instruments/macro.md) | Control several parameters from one knob. |

## Layers

A new MIDI instrument starts with one empty layer. Add a source before expecting sound. A layer can contain Oscillator or Sampler. It cannot contain both. Add components from the Instrument catalog by double-clicking an entry, selecting it and pressing Enter, or dragging it onto a layer.

Use the Layers Plus button to add another layer. Each layer has gain, pan, mute, and solo controls. Solo applies among the layers of that instrument. [Main](../instruments/main.md) controls their combined output and can hold shared modulators.

## Amplitude

[Amplitude](../instruments/amplitude.md) controls the level of notes in a layer. Connect an Envelope to shape attack and release, or an LFO for repeated level changes. Without an active Amplitude contour, notes use a short gate. Turning Amplitude off bypasses its level shaping while retaining note gating.

## Effects

Available effects are [3-band EQ](../effects/equalizer.md), [Drive](../effects/drive.md), [Auto-filter](../effects/auto-filter.md), [Compressor](../effects/compressor.md), [OTT](../effects/ott.md), and [Limiter](../effects/limiter.md). Select the destination track or Master before adding an effect. Effects run in lane order. Master effects process the combined track mix.

The header power control turns a unit on or off. Double-click its header to collapse or expand it. Off units keep their settings and remain editable. See [moving and copying units](../editing/units.md) and [modulation](../modulation/README.md).
