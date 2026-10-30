# Instrument modulation lanes

Each MIDI instrument has an **LFO** lane and an **Envelope** lane. They belong to the whole instrument and appear after its regular layers, before Output. They scroll vertically with the layers. Output stays pinned at the bottom.

Select a lane, then add its component from Modulators in the Instrument components catalog. Double-click an entry, select it and press Enter, or drag it onto the lane's label or body. The LFO lane accepts only LFOs. The Envelope lane accepts only envelopes. A mixed catalog selection containing an unsupported type makes no addition to these lanes. Regular layers and Output no longer accept these two component types.

Both lanes start compact. Use the button at the top of each lane's right toolbar to expand or collapse its row. Controls scroll horizontally while the toolbar stays fixed. Unit power and unit collapse remain independent of the row layout. Modulation lanes have no audio source, effects, mix controls, or level meter. They cannot be renamed, removed, or reordered.

Drag unit headers to reorder units within their lane. Hold Option on macOS or Ctrl on Windows/Linux to copy. Units cannot move to a different kind of lane. Click to select one unit, Shift-click to select several, and press Delete or Backspace to remove them. Undo restores the removal. Re-adding a removed unit restores its retained settings and assignments.

Use a receiving unit's [modulation controls](../modulation/README.md) to assign a source. Pickers group these sources under LFO and Envelope. Adding a source creates no connections. LFOs produce shared signals. Envelopes keep Per note, Retrigger, and Legato modes; Per note remains specific to each playing voice. Existing scope and cycle checks still apply throughout the same instrument.

Older projects move existing LFOs and envelopes from regular layers and Output into their matching lanes when opened. Their settings, assignments, and sound are retained. Lane layout, unit order, and controls save with the project. Projects saved with these lanes require the current application version.
