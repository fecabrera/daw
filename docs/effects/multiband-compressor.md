# Multiband Compressor

Add **Multiband Compressor** from Dynamics. It divides stereo audio into adjacent
frequency ranges and compresses each range independently. It starts with three
bands, split at 200 Hz and 2 kHz.

**Split** sets the boundaries between ranges. The lowest range extends to DC;
the highest extends to Nyquist. Boundaries cannot cross in the DAW. Scroll the
Split strip vertically when it contains more controls than fit.

Each band has these controls:

| Control | Range | Default |
| --- | --- | --- |
| Threshold | −60–0 dBFS | −18 dBFS |
| Ratio | 1:1–20:1 | 4:1 |
| Attack | 0.1–200 ms | 10 ms |
| Release | 10–2000 ms | 100 ms |
| Knee | 0–24 dB | 6 dB |
| Makeup | Silence…+12 dB | 0 dB |
| Detection | Peak, RMS | Peak |

These controls follow [Compressor](compressor.md). Channels share gain reduction
to preserve stereo balance. Each band rail shows output level with the shared held
sample-peak marker. Click a rail to reset only that band's held level peak. Holds
remain through pause and stop; unit power off clears them. The reduction
readout shows compression before Makeup. Click the reduction readout to reset
its held peak. Band power bypasses compression and Makeup, keeping the frequency
range audible.

Click **S** to hear only that band's processed range. Multiple S buttons combine
their ranges. S also works when band power is off. Solo excludes the full-range
dry signal, including at 0% Dry/Wet. Solo is temporary: it does not change the
project, presets or undo history. Unit power off clears Solo. DAW export ignores
it. Standalone hosts must declare offline rendering for export to ignore Solo;
real-time recording captures what you hear.

Plus splits the widest range. Right-click anywhere in a band for **Enable band**
or **Disable band**, checked **Solo**, and **Remove band**. Removal merges its
range with a neighbor. At least one band remains in the DAW.
Drag a footer or unused band space to change its visual position. Its frequency
range, settings and connections stay with it. Escape cancels a drag.

**Out** has Level and Dry/Wet. The dry blend follows the crossover phase, which
avoids cancellation against the compressed ranges. Header power passes the
original input without output gain. Crossovers use a fixed 24 dB/oct slope.
There is no lookahead, automatic makeup or external sidechain.

The six continuous band controls and Out controls accept shared modulation from
the same instrument. Open the header's connection dot to add routes. Split and
Peak/RMS remain manual choices.

This effect also has [standalone CLAP/VST3 previews](plugins.md), with up to 64 band
slots and horizontal scrolling. More bands use more CPU and memory.
