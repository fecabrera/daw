# Beat tracks and step editor

Use beat tracks to build rhythms from independently configured instruments. Mixer shows the parent beat track only. You can place that parent inside a [project track group](../editing/track-groups.md).

## Create a beat

1. Choose **Add beat track** from the Track menu, the Tracks panel Plus menu, or its empty-space context menu.
2. Double-click empty space on that track to create a pattern. Double-click the clip to open Editor.
3. Click the **Plus** button in Editor’s **Beat subtracks** header, or choose **Add subtrack** from the Tracks Plus menu or the Track menubar with a beat selected. You can also right-click the beat parent and choose **Add subtrack**. New subtracks start with an empty source layer.
4. Switch to Instrument, open **Tracks**, and select the subtrack. Add Oscillator, Sampler or Granulizer to its layer through the shared component catalog. Each subtrack is an instrument; there is no audio/MIDI type choice.
5. Return to Editor and click steps to enable or disable them. Turn on the headphones button above the step rows to hear each step you enable. Press Play to hear the pattern.

Editor keeps the selected track's clip overview above the step rows. Empty disabled steps fill the visible area outside the selected pattern. Without an active clip, all steps stay visible but disabled, and subtrack controls remain available; select a clip, or double-click empty space in the track overview to create a pattern.

Click the standalone step in the gutter before a subtrack’s pattern steps to
preview that instrument. It works with automatic audition off and without a
selected pattern. It does not add hits, change the pattern, or select the subtrack.
The preview lets the instrument finish its release or one-shot playback. It does
not cut the sound at the step boundary. A new preview or Escape stops it.

## Grid and hit controls

Patterns share one grid across all subtracks. **Length** uses the transport’s musical-position display, with duration counts starting at zero: `0001.00` means one bar and `0000.02` means two beats. It follows trimming, stretching and repetitions. Drag the bar or beat section up or down to adjust it; hold Shift for slower adjustment. Release to apply, or press Escape to cancel. The tooltip shows the exact duration in seconds and samples, including fractions smaller than one beat.

Unlooped sources support up to 16 bars before stretch; stretched and looped clips can show larger lengths. **Subdivision** shows each step’s length relative to one 4/4 bar. With no stretch, the choices are 1/4, 1/8, 1/16 and 1/32 bar. The default is one bar with 1/16-bar steps. Stretching updates these values; for example, doubling a 1/16-bar step makes it 1/8 bar. Trimming or changing pattern length does not change the step duration. Unusual stretch ratios can show a decimal bar value. Trimming or choosing a coarser grid keeps hidden steps silent. Expanding the range or restoring the resolution reveals their original positions and velocities.

An enabled step starts at velocity 100. Right-click it to change velocity from 1 to 127. Right-click a subtrack header to set **Trigger pitch**. Its right-aligned value shows a note such as C4 or D3#. Drag to change it in semitones, or click and type a note; D#3 is also accepted. The range is C-1 to G9 (MIDI 0–127), and the default is C4 (60). Enter accepts a typed note; Escape cancels it. Invalid notes retain the current pitch. Every hit on that subtrack uses this pitch, plus any clip transpose.

Clip Gain also controls retained Output effect tails in a gap. The next unmuted
clip takes over the shared bus gain, including any remaining tails, over up to
5 ms. A following clip or loop boundary can shorten that transition.
Silence applies exactly at the clip start.

Each subtrack has one active voice. A new hit replaces its previous voice. Note-off follows one step later; source trigger mode, Envelope and Amplitude settings determine audible duration. A Sampler in One-shot mode can continue after note-off.

Pattern edits affect only the active clip and create a new source. Other clips keep their previous content. A grid change that would create an invalid source range or overlap another clip is rejected without changing the project. Moving, copying or pasting a beat clip onto a different beat track is currently rejected. Same-track arrangement gestures remain available.

## Extend a beat clip

In Timeline or Editor’s track overview, drag either edge of an unlooped clip’s
body to extend it. Hold Shift to bypass snapping. Release to apply; Escape
cancels. The opposite edge stays fixed. Extending beyond the saved source grows
that clip’s pattern, up to 16 bars, and leaves other clips unchanged. Existing
hits keep their timing. Previously hidden hits can become visible in the added
range. Shortening keeps the pattern content available for later extension.
The clip can end partway through a bar; **Length** follows its visible duration.

Drag a clip header’s edge to extend repetitions. Looped clips retain their
existing body-trim behavior. Resize previews include any neighboring clips that
will be trimmed or removed. Undo restores the complete change.

## Manage subtracks

Editor and Instrument share the same names, order, selection and mix controls. The **Tracks** panel in both views lists subtracks below their beat parent. Its right-pinned Mute, Solo, Pan and Gain controls change the corresponding track or subtrack without selecting it.
Selecting a subtrack in Editor also selects its instrument when you return to
Instrument, including after selecting the final Output.

- Click a name to select one subtrack; Shift-click headers to toggle group membership. In Editor, clicking any part of a subtrack header, including its controls or empty space, selects that subtrack. Press Escape to clear subtrack selection in Editor or Instrument. Escape first cancels an active rename or drag.
- Right-click anywhere in a subtrack header, including its controls, meters or empty space, to open its context menu. Opening or dismissing the menu does not change controls or pattern content.
- Double-click a name, or use its context menu, to rename it. In Editor, double-clicking the empty header area beside the name also starts renaming. Mix controls retain their own double-click actions. The name uses the same text input as track names and starts with the whole name selected. Enter accepts; Escape or moving focus elsewhere cancels.
- Drag a selected name to reorder the selected subtracks together. Release over a subtrack in the same beat track to apply the move. Moving a subtrack to a different beat track, a project group or Master is rejected. Escape, focus loss or an outside release cancels it.
- Use the name's context menu to delete selected subtracks. Delete/Backspace also removes them while a subtrack name has keyboard focus. Text, step and unit controls retain their own keyboard behavior.
- Use Mute, Solo, Pan and Gain in either view. Solo applies within the beat instrument; parent track and group controls still apply to the complete output.

Deleting a subtrack removes its stop links. Its old pattern steps remain dormant and do not transfer to a newly added subtrack. Track duplication copies processing settings into fresh subtrack identities and starts with no clips, like audio and MIDI track duplication.

## Instrument layers and Output

Select a subtrack in **Tracks** to open its **Layers** page. It uses the same layout and controls as a MIDI instrument: source layers, local LFO and Envelope lanes, and a pinned Output. The Layers Plus button adds a source layer to that subtrack. Each subtrack's modulation sources stay local to its instrument. Selecting another subtrack replaces the visible instrument and its layer selection.

Drag selected source-layer headers to reorder them within the selected subtrack. Hold Option on macOS or Ctrl on Windows/Linux to copy them. Release inside the layer list to apply; Escape, focus loss or an outside release cancels the drag.

Select the beat parent in **Tracks** to show only its final **Output**. This combines the subtracks before the parent track's effects and mix controls. It supports output effects and local Macros. It cannot be renamed, deleted or reordered as a subtrack. Selecting a subtrack returns to that instrument's Layers page.

## Stop another subtrack

Right-click a subtrack name and open **Stops**. Select the subtracks that its hits must stop. For example, make Closed hat stop Open hat.

Stops use a 5 ms fade, including for one-shot samples. If both subtracks have a step at the same time, the stopping subtrack wins. Self-links and cycles are rejected. Muted or excluded non-solo subtracks issue no trigger or stop. With simultaneous A→B→C hits, B and C are suppressed; all enabled stop intents apply before hits are admitted.

Beat settings and embedded patterns save with the project. Sampler recordings follow the normal external/included source rules; use **Collect project sources** before moving a project that references external recordings. The CLI can inspect beat tracks, validate, collect sources and render them through the same engine.

Use the lower editor's topbar to adjust the clip length and step resolution.
Subtrack names and mix controls appear in the left panel, with a right-pinned
Plus button in the **Beat subtracks** header. Headers and steps scroll together.
Each header shows the name, followed by mute, solo, pan, gain and the shared
stereo level meter pinned right. The meter reads that subtrack's output and shares
its clipping indication with Instrument. Click either bar to reset its held peak
and clipping warning. The ruler above the steps follows the same
zoom and scroll position as the pattern. The gutter between the left panel and
the steps contains the same headphones audition control as the MIDI editor.
Audition is off by default. Removing a step or clicking a disabled step is silent.

Use the scrollbar below the steps to move through time without moving the
track overview or subtrack headers. It remains available without a selected pattern.

In **Tracks**, the beat parent's chevron hides or shows its subtrack rows. Beat parents start expanded. Hiding rows retains playback and the selected instrument. The Output strip's toolbar controls that strip separately.
