# Beat editor roadmap

Beat tracks and the beat editor are planned. They are not available in the current application. Use [MIDI patterns](editor.md) or arrange audio clips in Timeline to build rhythms today.

## Planned workflow

**Add beat track** will join Add audio track and Add MIDI track in the menubar/application Track menu, the Tracks panel Plus dropdown and track-creation context menus.

Editor will keep the beat track's clip overview above a lower step editor. The lower editor will show one row per beat subtrack, with a common pattern length and step resolution. Every subtrack will be an instrument. Choose Sampler, Oscillator or other supported sources inside its layers. Both Editor and Instrument will let you add, select one or several, delete, rename and reorder subtracks, and adjust the same gain and pan controls.

In Instrument, each subtrack's named row will act as its output layer. Expand Kick to show Layer 1, LFO and Envelope; Snare can have Layer 1, Layer 2 and its own LFO and Envelope. Each group's modulators will be independent. Collapsing hides its children while keeping the named output row visible. One overall Output lane will follow all subtrack groups and combine their audio. Mixer will control the parent beat track.

The planned step editor includes on/off steps, velocity, a fixed trigger pitch for each subtrack, and lane-stop connections such as a closed hi-hat stopping an open hi-hat. Patterns default to one bar at 1/16 resolution. These are planned capabilities, not current controls. See the [features and roadmap](../roadmap.md) for current availability.
