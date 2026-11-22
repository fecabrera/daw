# Transient Shaper

Add **Transient Shaper** from **Dynamics** in the Effects browser. Use it to change the punch and decay of drums, plucks, and other sounds with a distinct onset.

| Control | What it does |
| --- | --- |
| Attack | Boosts or cuts the start of a hit. The range is −12 to +12 dB. |
| Pump | Reduces the signal just after a hit. Raise it to make the onset stand out without adding gain to the onset. The range is 0 to 12 dB of cut. |
| Sustain | Boosts or cuts the body of the sound. The range is −12 to +12 dB. |
| Speed | Controls how quickly the detector reacts. Higher settings give a shorter, sharper response. The default is 50%. |
| Level | Sets the wet output level from silence to +12 dB. The default is 0 dB. |
| Dry/Wet | Blends the original and processed signals. The default is 100% wet. |
| Clip Mode | **None** leaves the shaped signal unclipped. **Soft** rounds peaks, **Hard** limits peaks to full scale, and **Tube** rounds positive and negative peaks differently. Clipping occurs before Level and Dry/Wet. |

Attack, Pump, and Sustain start at 0 dB. Clip Mode starts at **None**. The **Gain preview** shows the effect of the shape and speed settings on example hits. The output meter shows the processed signal.

Open the standard [modulation panel](../modulation/README.md) to assign a source, destination, and amount. For example, assign an audio sidechain from a kick track to Pump to vary the amount of attenuation. The Transient Shaper still detects hits from its own audio input.

The [standalone plugin](plugins.md) has the same sound controls and supports host automation.
