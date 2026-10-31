# Beat tracks and step editor

Use beat tracks to build rhythms from independently configured instruments. Mixer shows the parent beat track only.

## Create a beat

1. Choose **Add beat track** from the Track menu, the Tracks panel Plus menu, or its empty-space context menu.
2. Double-click empty space on that track to create a pattern. Double-click the clip to open Editor.
3. Click **Add beat subtrack** in Editor, or the Plus button in Instrument’s **Subtracks** header. New subtracks start with an empty source layer.
4. Switch to Instrument and add Oscillator, Sampler or Granulizer to that subtrack's layer through the shared component catalog. Each subtrack is an instrument; there is no audio/MIDI type choice.
5. Return to Editor and click steps to enable or disable them. Press Play to hear the pattern.

Editor keeps the selected track's clip overview above the step rows. Without an active clip, empty steps stay visible but disabled, and subtrack controls remain available; select a clip, or double-click empty space in the track overview to create a pattern.

## Grid and hit controls

Patterns share one grid across all subtracks. Choose 1–16 bars and 1/4, 1/8, 1/16 or 1/32 resolution. The default is one bar at 1/16. Shortening the pattern or choosing a coarser grid keeps hidden steps silent. Expanding the grid restores their original positions and velocities.

An enabled step starts at velocity 100. Right-click it to change velocity from 1 to 127. Right-click a subtrack name to set its trigger pitch from MIDI 0 to 127; the default is 60. Every hit on that subtrack uses this pitch, plus any clip transpose.

Each subtrack has one active voice. A new hit replaces its previous voice. Note-off follows one step later; source trigger mode, Envelope and Amplitude settings determine audible duration. A Sampler in One-shot mode can continue after note-off.

Pattern edits affect only the active clip and create a new source. Other clips keep their previous content. A grid change that would create an invalid source range or overlap another clip is rejected without changing the project. Moving, copying or pasting a beat clip onto a different beat track is currently rejected. Same-track arrangement gestures remain available.

## Manage subtracks

Editor and Instrument share the same names, order, selection and mix controls.

- Click a name to select one subtrack; Shift-click names to select a group.
- Double-click a name, or use its context menu, to rename it. Enter accepts; Escape cancels.
- Drag a selected name to reorder the selected subtracks together. Release inside the list to apply the move. Escape, focus loss or an outside release cancels it.
- Use the name's context menu to delete selected subtracks. Delete/Backspace also removes them while a subtrack name has keyboard focus. Text, step and unit controls retain their own keyboard behavior.
- Use Mute, Solo, Pan and Gain in either view. Solo applies within the beat instrument; parent track and group controls still apply to the complete output.

Deleting a subtrack removes its stop links. Its old pattern steps remain dormant and do not transfer to a newly added subtrack. Track duplication copies processing settings into fresh subtrack identities and starts with no clips, like audio and MIDI track duplication.

## Instrument groups and Output

The Instrument page uses the same header, Plus button, left column, dividers, lane toolbars and pinned Output layout as a MIDI instrument. Each expanded subtrack has a compact header with its name, chevron and **Add layer** button. Its source layers, LFO, Envelope and Output appear below, without a separate Layers header. Use the Output panel for mute, solo, pan, gain and its level meter. LFO and Envelope lanes accept only their matching provider units. Each subtrack's modulation sources stay local to that instrument.

Collapsing a group hides all its child lanes, including Output, and retains audio processing. The compact header also shows the output controls while collapsed, and hides Add layer. Each nested Output lane contains that subtrack's output units and controls. Its Gain and the group-header Gain control the same level. One fixed **Output** follows all groups and combines them before the parent track's effects and mix controls. It supports the shared output body, effects and local Macros. It cannot be renamed, deleted or reordered as a subtrack.

## Stop another subtrack

Right-click a subtrack name and open **Stops**. Select the subtracks that its hits must stop. For example, make Closed hat stop Open hat.

Stops use a 5 ms fade, including for one-shot samples. If both subtracks have a step at the same time, the stopping subtrack wins. Self-links and cycles are rejected. Muted or excluded non-solo subtracks issue no trigger or stop. With simultaneous A→B→C hits, B and C are suppressed; all enabled stop intents apply before hits are admitted.

Beat settings and embedded patterns save with the project. Sampler recordings follow the normal external/included source rules; use **Collect project sources** before moving a project that references external recordings. The CLI can inspect beat tracks, validate, collect sources and render them through the same engine.

Use the lower editor's topbar to add subtracks, set the pattern's bar count,
and choose its step resolution. Each compact lane starts with mute, solo, the subtrack name, pan, and gain,
followed by its steps. The ruler above the steps follows the same
zoom and scroll position as the pattern.

In Instrument, each subtrack has a chevron beside its name. Click it to hide or
show its nested source layers, LFO and Envelope lanes. The subtrack Output lane is hidden while collapsed. New groups start expanded.
Collapsing a group keeps its instruments playing. The output strip's toolbar
button controls that strip separately.
