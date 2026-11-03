# Noise

Noise generates White, Pink or Brown noise for MIDI layers and beat subtracks.
Add it from **Sources → Noise** in the Instrument components catalog. Adding it
replaces the layer's existing source. Use another layer to mix Noise with an
Oscillator, Sampler or Granulizer.

## Controls

| Control | Range or choices | Default |
| --- | --- | --- |
| Type | White, Pink, Brown | White |
| Color | −100% darker to +100% brighter | 0% |
| Level | −∞ to +12 dB | −12 dB |
| Width | 0% mono to 100% independent stereo | 0% |
| Power | On/off | On |

The **Noise** strip contains Type, Color and a small spectral preview. White has
an even frequency balance. Pink places more energy at lower frequencies. Brown
has a stronger low-frequency emphasis. The types use a common reference RMS
level; this does not guarantee equal perceived loudness.

Color changes the bass/treble balance around 1 kHz. Its endpoints apply opposite
6 dB changes to the low and high parts of the signal. The preview shows approximate
relative balance from 20 Hz to 20 kHz. It responds to Type and base Color, remains
visible while stopped, and excludes Level, modulation and downstream processing.
It is not a live spectrum analyzer.

The **Out** strip contains Level, Width and the [standard output meter](../effects/output-meter.md).
Width starts with the same signal in both channels and introduces an independent
right-channel stream as it increases. MIDI pitch does not tune the noise. Note
velocity affects its volume.

Use [Amplitude](amplitude.md) and an [Envelope](envelope.md) to shape noise bursts,
percussion and sustained textures. Without an assigned amplitude envelope, Noise
uses the standard short note gate. It does not run as a continuous background
source when no notes are active.

Color, Width and Level accept [modulation](../modulation/README.md) from Envelope,
LFO and Macro. Type remains discrete. Knobs use the standard fine adjustment and
double-click reset. Power retains the settings. Double-click the unit header to
collapse it, or right-click the header to remove it. The compact layer view hides
controls and the preview.

Each note has an independent noise stream. Replaying or exporting the same project
reproduces those streams. Copying a layer or source creates a separate stream.
