# Beat tracks and step editor

Use beat tracks to build rhythms from independently configured instruments. Mixer shows the parent beat track and expandable subtrack strips. Use the parent for whole-beat controls and each child for its gain, pan, Mute, Solo and output effects. You can place that parent inside a [project track group](../editing/track-groups.md).

## Create a beat

1. Choose **Add beat track** from the Track menu, the Tracks panel Plus menu, or its empty-space context menu.
2. Double-click empty space on that track to create a pattern. Double-click the clip to open the Timeline editor and fit its full source pattern.
3. Click the **Plus** button in the Timeline editor’s **Beat subtracks** header, or choose **Add subtrack** from the Track menu with a beat selected. You can also right-click the beat parent and choose **Add subtrack**. New subtracks start with an empty source layer.
4. Switch to Instrument, open **Tracks**, and select the subtrack. Add Oscillator, Sampler or Granulizer to its layer through the shared component catalog. Each subtrack is an instrument; there is no audio/MIDI type choice.
5. Return to the beat editor pane in Timeline and click steps to enable or disable them. Turn on the headphones button above the step rows to hear each step you enable. Press Play to hear the pattern.

While the pane is open, select a beat clip in the arrangement. The grid shows that complete pattern once, including steps outside the clip’s trimmed
range. Other clips, loop repetitions and empty cells beyond the source are not
shown. Subtrack controls and audition buttons remain available without a clip selected.

## Grid and hit controls

Patterns share one grid across all subtracks. **Clip length** uses the transport’s musical-position display, with duration counts starting at zero: `0001.00` means one bar and `0000.02` means two beats. It follows the project time signature, trimming, stretching and repetitions. Drag the bar or beat section up or down to adjust it; hold Shift for slower adjustment. Release to apply, or press Escape to cancel. The tooltip shows the exact duration in seconds and samples, including fractions smaller than one beat.

Unlooped sources support up to 16 bars before stretch; stretched and looped clips
can have longer visible durations. **Subdivision** shows each source step’s length
relative to the current time signature. In 4/4 the choices are 1/4, 1/8, 1/16 and
1/32 bar. Clip stretch does not change the source grid or these captions. New
sources have a four-quarter-note span with sixteenth-note steps. Changing the
time signature updates beat grouping and fractions without moving hits. Steps
outside a clip trim remain editable in the full source grid, but stay silent
until included in playback. A shorter source length or coarser subdivision keeps
hidden steps stored; expanding the source or restoring the finer subdivision
reveals their original positions and velocities.

An enabled step starts at velocity 100. Right-click it to change velocity from 1 to 127. Drag the value, or click it, type a number and press Enter. The menu stays open while you edit. Right-click a subtrack header to set **Trigger pitch**. Its right-aligned value shows a note such as C4 or D3#. Drag to change it in semitones, or click and type a note; D#3 is also accepted. The range is C-1 to G9 (MIDI 0–127), and the default is C4 (60). Enter accepts a typed note; Escape cancels it. Invalid notes retain the current pitch. Every hit on that subtrack uses this pitch, plus any clip transpose.

Each subtrack has one active voice. A new hit replaces its previous voice. Note-off follows one step later; source trigger mode, Envelope and Amplitude settings determine audible duration. A Sampler in One-shot mode can continue after note-off.

Pattern edits affect only the active clip and create a new source. Other clips keep their previous content. A grid change that would create an invalid source range or overlap another clip is rejected without changing the project. Moving, copying or pasting a beat clip onto a different beat track is currently rejected. Same-track arrangement gestures remain available.

The lower ruler shows source bars starting at 1.1. Fit and zoom apply to the full
source, independently of clip position, trim, stretch and loops. Use the upper
ruler to seek or select a project loop. Wider gaps separate beats according to the
project time signature; steps have no beat-start dots. Colors remain unchanged.

Click the standalone step in the gutter before a subtrack’s pattern steps to
preview that instrument. It works with automatic audition off and without a
selected pattern. It does not add hits, change the pattern, or select the subtrack.
The preview lets the instrument finish its release or one-shot playback. It does
not cut the sound at the step boundary. A new preview or Escape stops it.

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
Timeline arrangement or subtrack headers. It remains available without a selected pattern.

## Manage subtracks

Selected subtracks use the same header and lane colors as selected Timeline
tracks and Instrument layers in Dark, Light and Mist. Each selected subtrack is
highlighted without a selection outline, including when several are selected. Steps
use closely spaced rectangular tiles with small rounded corners. They keep their
existing on/off and hover colors. Dividers separate subtrack rows across both headers and the step grid. The separator below the ruler also spans the headers to the left toolbar. Subtrack Mute and Solo use the shared joined M / S
button group; each button remains independent.

The beat editor and Instrument share subtrack names, order, selection, and mix settings. Timeline shows subtrack mix controls in expandable child rows under the beat parent. Edit steps in the beat editor pane. Projects save each Timeline beat folder’s open or closed state. Instrument’s **Tracks** panel lists subtracks below their beat parent.
Subtrack names use the same hoverable name buttons as other tracks and align to the standard left padding. Hover to see the full name. A single click selects; double-click the name to rename it.
Selecting a subtrack in the beat editor also selects its instrument when you return to
Instrument, including after selecting the final Output.

- Click a name to select one subtrack; Shift-click headers to toggle group membership. In the beat editor, clicking any part of a subtrack header, including its controls or empty space, selects that subtrack. Press Escape to clear subtrack selection in the beat editor or Instrument. Escape first cancels an active rename or drag.
- Right-click anywhere in a subtrack header, including its controls, meters or empty space, to open its context menu. Opening or dismissing the menu does not change controls or pattern content.
- Double-click a name, or use its context menu, to rename it. In the beat editor, double-clicking the empty header area beside the name also starts renaming. Mix controls retain their own double-click actions. The name uses the same text input as track names and starts with the whole name selected. Enter accepts; Escape or moving focus elsewhere cancels.
- Drag a selected name to reorder the selected subtracks together. Release over a subtrack in the same beat track and panel to apply the move. Only the panel where the drag started scrolls at its edges. Moving a subtrack to a different panel, beat track, project group or Master is rejected. Escape, focus loss, closing the originating panel or an outside release cancels it.
- Use the name's context menu to delete selected subtracks. Delete/Backspace also removes them while a subtrack name has keyboard focus. Text, step and unit controls retain their own keyboard behavior.
- Use Mute, Solo, Pan and Gain in the beat editor or Instrument. Solo a
  subtrack to hear it through the beat parent and any enclosing groups. The
  regular track Solo rules exclude unrelated branches and keep sends from
  soloed branches feeding their Returns. Within the beat track, only soloed
  subtracks can play. A muted beat subtrack stays silent when soloed.

Deleting a subtrack removes its stop links. Its old pattern steps remain dormant and do not transfer to a newly added subtrack. Track duplication copies processing settings into fresh subtrack identities and starts with no clips, like audio and MIDI track duplication.

## Instrument layers and Output

Select a subtrack in **Tracks** to open its **Layers** page. It uses the same layout and controls as a MIDI instrument: source layers, local LFO and Envelope lanes, and Output in one scroll area. The Layers Plus button adds a source layer to that subtrack. Each subtrack's modulation sources stay local to its instrument. Selecting another subtrack replaces the visible instrument and its layer selection.

Drag selected source-layer headers to reorder them within the selected subtrack. Hold Option on macOS or Ctrl on Windows/Linux to copy them. Release inside the layer list to apply; Escape, focus loss or an outside release cancels the drag.

Select the beat parent in **Tracks** to show only its final **Output**. This combines the subtracks before the parent track's effects and mix controls. It supports output effects and local Macros. It cannot be renamed, deleted or reordered as a subtrack. Selecting a subtrack returns to that instrument's Layers page.

In Instrument’s **Tracks** panel, the beat parent's chevron hides or shows its subtrack rows. Beat parents start expanded. Hiding rows retains playback and the selected instrument. The Output strip's toolbar controls that strip separately.

## Stop another subtrack

Right-click a subtrack name and open **Stops**. Select the subtracks that its hits must stop. For example, make Closed hat stop Open hat.

A new hit during a stop fade starts from the current faded level. It does not
restore the previous full level. Layer and Output effect tails continue.

Stops use a 5 ms fade, including for one-shot samples. If both subtracks have a step at the same time, the stopping subtrack wins. Self-links and cycles are rejected. Muted or excluded non-solo subtracks issue no trigger or stop. With simultaneous A→B→C hits, B and C are suppressed; all enabled stop intents apply before hits are admitted.

## Merge beat patterns

Select two or more beat clips on the same track, with or without gaps. Choose **Merge** from
the **Clip** menu or a selected clip's context menu. Press **Command+J** on macOS
or **Ctrl+J** on Windows and Linux. Use this shortcut in Timeline.

Gaps remain silent and later hits keep their timeline positions. Select any
intervening clips too: Merge cannot cover an unselected clip. Gaps count toward
the merged pattern's length limit and must preserve the common subdivision.

Select one beat clip, including a looped or trimmed clip, and use **Merge** to rebuild its visible
hits into one unlooped pattern. It keeps the same clip start and length, clears
the source offset and preserves lane assignments, velocities, subdivision and
stretch. Hits outside the trimmed range stay in the original source. Undo
restores the trim and loop. The grid, length and hit limits below also apply to
single clips.

The clips must have matching subdivision, stretch, gain, transpose and enabled
state. Clip bounds, source offsets and loop bounds must align with that
subdivision. Merge creates one unlooped pattern with the first clip's name and
color. It keeps visible hits, lane assignments and velocities, including repeated
sections. Hidden hits stay in the original sources; they do not enter the merged
pattern. Lane instruments, mix controls and stop connections stay unchanged.
The result retains the common stretch and subdivision, so hit duration stays the
same. Undo restores the original clips in one step.

The result supports up to 16 source bars before stretch and 4,096 visible hit
occurrences. Merge rejects a partial or off-grid hit, incompatible settings or an
oversized result and leaves the clips unchanged.

## Trim and restore beat steps

Trimming either clip edge excludes steps outside that range from playback and
the arrangement clip thumbnail. The full source grid still shows and edits those steps.
Their positions, lane assignments and velocities remain stored. Drag the clip
edge back to include them in playback, including after saving and reopening.
Undo/Redo restores the previous clip range without changing the stored steps.

Steps hidden by a shorter source length or coarser subdivision also stay stored.
Expand the range or restore the finer subdivision to reveal them. Use **Merge**
when you want a new pattern that contains only the visible hits.

## Extend a beat clip

In the Timeline arrangement, drag either edge of an unlooped clip’s
body to extend it. Hold Shift to bypass snapping. Release to apply; Escape
cancels. The opposite edge stays fixed. Extending beyond the saved source grows
that clip’s pattern, up to 16 bars, and leaves other clips unchanged. Existing
hits keep their timing. Previously hidden hits can become visible in the added
range. Shortening keeps the pattern content available for later extension.
The clip can end partway through a bar; **Clip length** follows its visible duration.

Drag a clip header’s edge to extend repetitions. Looped clips retain their
existing body-trim behavior. Resize previews include any neighboring clips that
will be trimmed or removed. Undo restores the complete change.

## Playback and effect tails

Clip Gain also controls retained Output effect tails in a gap. The next unmuted
clip takes over the shared bus gain, including any remaining tails, over up to
5 ms. A following clip or loop boundary can shorten that transition.
Silence applies exactly at the clip start.

When a beat clip repeats or the transport loop restarts, beat one-shots and
release tails continue into the next pass. A new hit on the same subtrack
replaces the previous voice. Configured Stops connections still choke their
targets. Seeking starts playback from the new position.

## Save, move or render a beat project

Beat settings and embedded patterns save with the project. Sampler recordings follow the normal external/included source rules; use **Collect project sources** before moving a project that references external recordings. The CLI can inspect beat tracks, validate, collect sources and render them through the same engine.
