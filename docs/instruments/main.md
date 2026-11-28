# Instrument Output bus

Output combines the audio from all layers of one MIDI instrument. It is separate from the project Master, which combines tracks.

Layer sources pass through Amplitude, layer effects, and layer mix controls. Output applies its gain and pan to the combined layers. The signal then passes through the track Effects Lane and the track mix controls.

## Edit shared Macros

Open **Macros** to edit Macro controls shared across the instrument. The expand/collapse button at the top of its right toolbar shows or hides its controls. The toolbar stays at the right edge while components scroll horizontally. Output cannot be removed, renamed, or reordered.

Use the track header for track levels and metering. Saved Output gain and pan apply during playback.

## Add shared controls

In **Macros**, select the Output Macros row, then add Macro from Modulators in the component catalog. You can also drag compatible installed components onto that row. Output does not accept audio sources, Amplitude, LFOs, or envelopes.

LFOs and envelopes belong to their dedicated [instrument-wide lanes](modulation-lanes.md). Output Macros appear in destination lists with the Output prefix. They can control compatible parameters throughout the same instrument, including its track effects.

Adding a modulator does not create connections. Open a receiving unit's [modulation controls](../modulation/README.md) to assign it. Output settings and layout save with the project.

Use the bottom Effects Lane to add effects to the whole instrument or selected beat subtrack. Open a layer tab to edit its own effects. When you open an older project, the former Output effects move to the start of the corresponding bottom Effects Lane in their saved order.
