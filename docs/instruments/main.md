# Instrument Output bus

Output combines the audio from all layers of one MIDI instrument. It is separate from the project Master, which combines tracks.

Layer sources pass through Amplitude, layer effects, and layer mix controls. Output applies its gain and pan to the combined layers. The signal then passes through the track Effects Lane and the track mix controls.

## Use Output

Open **Macros** to find the instrument’s Output gain, meters, and Macro controls. The expand/collapse button at the top of its right toolbar shows or hides its controls. The toolbar stays at the right edge while components scroll horizontally. Output cannot be removed, renamed, or reordered.

Use the gain knob at the right of the Output header’s second row to change the combined instrument level. Its default is 0 dB. The stereo meters beside Gain show the combined instrument signal after Output gain and pan, before track effects. They work like track meters: either channel above 0 dBFS lights both clipping indicators, and clicking either rail clears them. For a beat subtrack this row also shows its Pan, Mute, and Solo controls. Gain does not accept modulation.

## Add shared controls

In **Macros**, select the Output Macros row, then add Macro from Modulators in the component catalog. You can also drag compatible installed components onto that row. Output does not accept audio sources, Amplitude, LFOs, or envelopes.

LFOs and envelopes belong to their dedicated [instrument-wide lanes](modulation-lanes.md). Output Macros appear in destination lists with the Output prefix. They can control compatible parameters throughout the same instrument, including its track effects.

Adding a modulator does not create connections. Open a receiving unit's [modulation controls](../modulation/README.md) to assign it. Output settings and layout save with the project.

Use the bottom Effects Lane to add effects to the whole instrument or selected beat subtrack. Open a layer tab to edit its own effects. When you open an older project, the former Output effects move to the start of the corresponding bottom Effects Lane in their saved order.
