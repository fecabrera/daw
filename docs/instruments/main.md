# Instrument Main bus

Main combines the audio from all layers of one MIDI instrument. It is separate from the project Master, which combines tracks.

Layer sources pass through Amplitude, layer effects, and layer mix controls. Main then processes the combined layer output through its effects and gain/pan, followed by track effects and track gain/pan.

## Use Main

Main stays at the bottom of Instrument while regular layers scroll. Its expand/collapse button shows or hides its controls. Main cannot be removed, renamed, or reordered.

Use Gain to change the combined instrument level and Pan to position it. Defaults are 0 dB and center. Main has no independent mute or solo control. Its gain and pan do not accept modulation.

## Add shared controls

Select Main, then add Envelope, LFO, or Macro from Modulators in the component catalog. You can also drag compatible installed components onto Main. Main does not accept audio sources, Amplitude, or Per note Envelopes.

New Main Envelopes start in Legato mode; Retrigger is also available. Main sources appear in destination lists with the Main prefix. They can control compatible parameters throughout the same instrument, including its track effects.

Adding a modulator does not create connections. Open a receiving unit's [modulation controls](../modulation/README.md) to assign it. Main settings and layout save with the project.

Main effects appear after its components and process the combined layer output before Main gain/pan. Select Main to add effects from the Effects catalog. Drag effect headers between Main, regular layers, and the track effects lane to move them; hold Option on macOS or Ctrl on Windows/Linux to copy. Click the bottom effects lane to add track effects.
