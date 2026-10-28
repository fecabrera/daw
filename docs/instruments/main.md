# Instrument Output bus

Output combines the audio from all layers of one MIDI instrument. It is separate from the project Master, which combines tracks.

Layer sources pass through Amplitude, layer effects, and layer mix controls. Output then processes the combined layer output through its effects and gain/pan, followed by track effects and track gain/pan.

## Use Output

Output stays at the bottom of Instrument while regular layers scroll. Its expand/collapse button shows or hides its controls. Output cannot be removed, renamed, or reordered.

Use Gain to change the combined instrument level. Its default is 0 dB. Output has no Pan, Mute, or Solo control. Gain does not accept modulation.

## Add shared controls

Select Output, then add Envelope, LFO, or Macro from Modulators in the component catalog. You can also drag compatible installed components onto Output. Output does not accept audio sources, Amplitude, or Per note Envelopes.

New Output Envelopes start in Legato mode; Retrigger is also available. Output sources appear in destination lists with the Output prefix. They can control compatible parameters throughout the same instrument, including its track effects.

Adding a modulator does not create connections. Open a receiving unit's [modulation controls](../modulation/README.md) to assign it. Output settings and layout save with the project.

Output effects appear after its components and process the combined layer output before Output gain/pan. Select Output to add effects from the Effects catalog. Drag effect headers between Output, regular layers, and the track effects lane to move them; hold Option on macOS or Ctrl on Windows/Linux to copy. Click the bottom effects lane to add track effects.
