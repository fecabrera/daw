# Timeline

The left toolbar lists **Effects** and **Chords**. **Clip settings** uses the toolbar at the right of the timeline area, from the playback toolbar down to the effects lane.

Timeline arranges audio and MIDI clips on tracks. Use Editor for MIDI notes, Instrument for MIDI sounds, and Mixer for track levels. Its horizontal scrollbar floats over the content, like the effects lane’s scrollbar; hover near the bottom of the timeline to reveal and expand it.

Selected tracks use background highlights without a selection outline. Each selected track is highlighted while its musical grid and clips remain visible. Selected track headers use a raised shade; unselected headers retain their normal theme fill.

Audio track headers show a waveform icon; MIDI track headers show a keyboard icon. Master shows a boom box icon. Their names use the same spacing as group headers. Track gain and pan, and Master gain, use compact knobs without text-entry fields. Compact tracks use one row: icon, name, Mute, Solo, then meters. Pan and Gain are hidden in this compact mode, including Master gain. In the expanded layout, the second row places Mute and Solo at the left; Pan then Gain sit at the right, immediately before the level meters. Master uses one row, with its gain knob on the right before the meters when track headers are expanded. Level meter rails align with the top and bottom padding of the other header controls. Hover or drag a knob to see its value in the tooltip. Master's meters align with the track meters at the right of the control column. Drag an audio or MIDI icon, track name, or blank header area to move the track. Release applies the move; Escape or focus loss cancels it.

The Tracks header’s Add and Duplicate buttons use the same rounded, borderless style as M/S buttons. Their actions and spacing stay the same.

Click the arrow beside a beat track to show or hide its subtracks. They start collapsed. Subtracks appear inside the parent, with the same indentation and selection style as track groups. Each row has Mute, Solo, Pan, Gain and level meters shared with Editor and Instrument. Subtrack names use the same hoverable name buttons as other tracks. A single click selects; double-click the name to rename it. Click a subtrack header to select it; Shift-click selects several. Escape clears subtrack selection. The child timeline lanes stay empty; arrange clips on the parent and edit beat hits in Editor. The project saves each beat folder’s open/closed state. Expanding or collapsing it supports Undo/Redo, like track groups.

Touching clips have a small visual gap. At a crossfade, the gap follows the owning clip’s fade endpoint. Audio, MIDI and beat clips have no selection border. Selected clips use a lighter shade of their color family in the header. Headers have rounded top corners and square bottom corners. The same style applies in the track overview and audio editor.

The musical grid uses a subtle overlay in active audio, MIDI and beat clip
bodies, below their waveforms and notes. Clip backgrounds remain opaque, and
their colors and headers stay unchanged.
These overlay lines use the dark palette's colors in every theme so they remain
visible against colored clips.

**M** and **S** share one joined button group with a divider. They remain independent: either or both can be enabled. The same group appears in Editor, Instrument, Beat Editor, the Tracks panel and Mixer.

Each L/R meter bar has a held-peak marker. Click either bar or the gap between them to reset both held peaks and the clipping warning. This applies to tracks, groups, Returns and Master. Held levels are shared with Mixer.

Use the **Track height** slider between Fit all clips and horizontal zoom to choose **Compact**, **Regular**, or **Large**. A divider separates these sliders from the buttons. Regular is the default. Compact uses one row; Regular uses two. Large adds a middle row containing the track’s send-level knobs, with the name above and M/S, Pan and Gain below. Each row uses the same vertical slot height. Headers keep the same spacing whether they appear first, in the middle, or last. Scroll the sends row horizontally when its knobs do not fit. The row remains empty for tracks without sends. The expand/collapse button switches between Compact and Regular. Master stays on one row, and Editor’s track overview always uses Regular. Save the project to retain Timeline track size, horizontal zoom, and horizontal/vertical scroll positions. Changing these display settings does not change audio or add an Undo step.

Use the **Return tracks** arrow and **Master track** boom box buttons in the
right toolbar to show or hide the pinned Return rows and Master. The buttons sit
at the bottom, with Returns above Master. Both start enabled. Timeline and
Mixer remember separate choices for the app session. You can also use **View >
Returns** and **View > Master**. Hidden tracks keep processing audio, and their
selection stays unchanged.

The Tracks header’s Add track (Plus) button opens a dropdown with **Add audio track**, **Add MIDI track**, and **Add beat track**. Choose an item to create and select a new empty track. Opening or dismissing the menu does not add a track.

The Add track and Duplicate track buttons stay available when another pane or non-text control has focus. Duplicate still requires a selected track. Text entry, jobs, dialogs, active gestures, and loss of window focus retain their guards.

Press M to toggle Mute and S to toggle Solo on each selected track. These shortcuts work from any focused pane or control in Timeline, except while you enter text. Dialogs, blocking jobs and active drags still disable them. Release a dragged control before using M/S. Holding a key does not toggle repeatedly.

When beat subtracks are selected in an expanded beat track, M/S toggles each selected subtrack instead of the parent. The group change uses one undo step. Click the beat parent header to return M/S control to the whole track. Mixer shortcuts continue to control whole tracks.

Rename (Command/Ctrl+R), Group (Command/Ctrl+G), and Ungroup (Command/Ctrl+Shift+G) also work when another pane or non-text control has focus in Timeline. Text entry, active drags, dialogs and blocking jobs retain their guards. Selection requirements still apply.

Press Escape to clear selected clips or a selected clip time range first. Tracks and Master stay selected. If no clips or clip time range are selected, Escape clears track or Master selection. Text entry, open menus and active gestures take precedence. Holding Escape does not advance through these steps.

The Return and Master toggles stay at the bottom of the upper right toolbar.
Opening the effects lane places them just above its toolbar. Closing the lane
returns them to the bottom of the full-height toolbar.

## Add chords

1. Open **Chords** with the piano button in the left toolbar.
2. Expand a note folder, or choose a root note in the header to show only that note’s chords. **All notes** restores all 12 roots. Sharp names also cover their flat equivalents.
3. Drag a chord onto a MIDI track. Drop below the tracks to create a new MIDI track. A drop on a track header starts at frame zero.
4. Follow the preview of the chord’s name, notes, and one-bar bounds. Covered clips preview their remaining ranges. Release to apply the placement. Hold Shift to bypass grid snapping; Escape or loss of window focus cancels.

Each chord is one 4/4 bar at the project tempo. Its notes sound together for the full bar, with roots from C4 through B4 and velocity 100. The catalog includes major, minor, diminished, augmented, sus2, sus4, major sixth, minor sixth, dominant seventh, major seventh, minor seventh, and half-diminished seventh chords. Edit the resulting notes in Editor.

Select a MIDI track, then click a chord or select it with Up/Down to hear it through that track’s instrument and effects. A new selection replaces the previous preview. The preview lasts one bar, up to eight seconds, with up to one second for release and effect tails. Track, group, and Master mix controls apply; other tracks’ Solo settings do not block the preview. Empty instruments and muted tracks remain silent. Audition does not insert notes, move the playhead, or change the project. Stop or Escape cancels it; changing tracks, leaving Chords, or losing window focus also stops it. Sample-based instruments may need a short preparation delay.

Double-click a chord to insert it at the playhead on the selected MIDI track. Enter inserts the selected chords there in catalog order, one after another. These actions require a selected MIDI track and keep the playhead in place. Shift-click selects several chords; dragging one selected chord places the whole selection in order.

With catalog focus, Up/Down navigates visible rows. Shift+Up/Down extends or reduces the chord selection. Enter opens or closes a focused note folder, or inserts selected chords when a chord row has focus. Escape clears catalog selection and follows Timeline’s normal track/clip selection behavior.

Chords use the normal clip overwrite rules and Undo/Redo. Audio and group tracks reject chord drops. Catalog browsing and filtering do not change the project; inserted chords save as ordinary MIDI patterns. A new MIDI track starts with an empty instrument, so add a sound in Instrument before playback.

## Import and arrange

1. Add an audio or MIDI track from the Track menu or the Tracks header.
2. Use Clip > Import clip for WAV, MP3, AAC, or Opus audio, or drop a supported audio file on an audio track. Dropping into empty Timeline space creates a new audio track.
3. Drag a clip header to move it. Hold Option on macOS or Ctrl on Windows/Linux to copy it.
4. Release to apply the change. Escape or focus loss cancels an active gesture.

Moves and copies take priority over destination clips. Covered neighbors are trimmed, split, or removed. The preview shows those changes before release. Shift bypasses grid snapping.

Audio clips can move or copy only to audio tracks; MIDI clips can move or copy only to MIDI tracks. This applies over both track headers and timeline lanes. An incompatible destination shows the invalid-action cursor. Releasing there rejects the entire selected group without an error dialog. Continue dragging to a compatible track to complete the move or copy.

Group buses cannot hold clips: dragging onto a bus shows an invalid-action cursor, and releasing there keeps all dragged clips unchanged. Drop clips onto child tracks.

Audio file drags over MIDI tracks or group buses show the invalid-action cursor. Drops there are ignored without an error dialog. Drop audio files onto audio tracks or empty Timeline space instead.

## Clip menu

Use [Normalize gain](../editing/normalize-gain.md) to set each selected audio clip’s peak or loudness level before effects.

Open **Clip** in the macOS menu bar or the Windows/Linux application menu. Its actions have three groups, separated by lines:

- **Import clip**, **Add MIDI pattern**.
- **Split at playhead**, **Merge**, **Bounce clips**, **Stem separation**, **Normalize gain**.
- **Edit clip**, **Disable clip**, **Delete clip**.

Choose **Add MIDI pattern** with a MIDI track selected to place a new pattern at the playhead. It follows the same placement rules as other new clips: covered clips are trimmed, split, or removed. Undo restores the previous arrangement. Import clip accepts WAV, MP3, AAC-LC, and Opus audio. Use Command+Shift+I on macOS or Ctrl+Shift+I on Windows/Linux to import a clip.

Edit, Disable and Stem separation use the primary selected clip. Disable changes to **Enable clip** for a disabled clip. Delete removes all selected clips. Split acts on selected clips that cross the playhead. Unavailable actions are disabled; Stem separation requires an audio clip in Timeline and a validated model.

Right-click a clip for the same last two groups: Split/Merge/Bounce clips/Stem separation/Normalize gain, then Edit/Disable/Delete. Edit, Disable and Stem separation use the clicked clip. Split, Merge, Bounce clips, Normalize gain and Delete use the selected group when the clicked clip belongs to it; otherwise they use that clip. See the requirements for [audio merging](#merge-audio-clips), [MIDI merging](#merge-midi-clips), [beat merging](beat-editor.md#merge-beat-patterns), and [Normalize gain](../editing/normalize-gain.md). MIDI headers retain the separate selected-note Velocity and Delete note controls.

## Select and edit clips

Click a clip to select it. Shift-click toggles group membership. Drag a rectangle in empty space to select intersecting clips; Shift-drag toggles them against the starting selection.

Hold Command on macOS, or Ctrl on Windows and Linux, and drag across a track to select a time range. The pointer uses the same crosshair as creating a loop range. The range can cover part of a clip, several clips, and the silence between them. Drag the range to move its contents. Hold Option on macOS, or Ctrl on Windows and Linux, and drag the range to duplicate it. On Windows and Linux, start that drag inside the range; Ctrl-drag outside the range selects a new range. Copy with Command+C or Ctrl+C, and paste at the playhead. The copy keeps the gaps and any silence before the first clip. Delete or Backspace removes the selected time and leaves the rest of each clip. The same range is available in Editor. Shift bypasses grid snapping. Escape or a click outside the range clears it.

The pointer shows a forbidden symbol over incompatible targets. Releasing there cancels the whole move or copy and retains the clips, selected range and Undo history.

Drag a waveform edge to trim. Drag a header edge to repeat the current range. Hold the copy/stretch modifier while dragging an edge to [stretch duration](../audio/time-stretch.md). Group edits preserve relative timing and apply together. Drag the dividing line between touching waveforms to move their shared boundary while keeping their outer endpoints fixed.

MIDI trims retain hidden notes and complete note lengths. Drag an edge back to
restore them; expansion beyond the source adds empty space. Saving and reopening
retain those hidden notes. **Merge** consolidates only the visible range when
you want a new pattern without the hidden content.

Copy with Command+C on macOS or Ctrl+C on Windows/Linux. Paste with Command+V or Ctrl+V at the playhead on the selected track. Copies keep their source content, trim, repeats, stretch, and color override.

Split the selected clips at the playhead with Clip > Split at playhead, or with Command+T on macOS or Ctrl+T on Windows/Linux. The same command is available in Editor. It is disabled when no clip is selected, when the playhead is not inside a selected clip, and in Mixer and Instrument. If several clips are selected, every selected clip that crosses the playhead is split together. The original clip ends at the playhead and a new clip starts there. Audio keeps its recording, trim, loop, and stretch. Right-click a clip and choose Split at playhead to split that clip, or the whole selection when the clicked clip is selected.

MIDI notes that cross the playhead follow Settings > MIDI > Split MIDI notes. Split into two is the default: the note becomes two notes. Preserve first keeps the part before the playhead on the original clip and does not continue it. The choice is saved on this computer, not in the project. Undo restores both pieces in one step. If a looping MIDI clip needs note rewriting, one split operation can unfold up to 4,096 note occurrences across the selection. A larger operation reports an error and leaves the project unchanged. Shorten the looping clips before retrying. Splitting between notes does not have this limit.

Duplicating into new track space creates fresh track defaults. Group paste also creates fresh tracks when needed and preserves gaps between tracks. A new MIDI track starts with an empty instrument and needs a source before it makes sound. Existing destination tracks keep their settings. Moving MIDI clips to new track space retains their instrument.

## Merge audio clips

Select audio clips on one track and choose **Merge** from the **Clip** menu or a
selected clip's context menu. Press **Command+J** on macOS or **Ctrl+J** on Windows
and Linux for the same command. It works in Timeline and Editor.

Clips can touch or have gaps. The result keeps their outer bounds, silent gaps,
gain, fades, stretch, transpose, channel settings and enabled states. Settings
can differ between clips. Select intervening clips too; Merge cannot cover an
unselected clip. Track effects, mixing and routing remain live. Merged bounced
clips retain their captured Pre sends and sidechain feeds through further merges
and clip edits.

The result keeps the first clip's name and color and becomes the selected clip.
Undo restores the original clips in one step. You can also consolidate one
trimmed or looped audio clip. Merging an unchanged complete result again makes
no edit.

Preparation runs in the background. Failure or cancellation keeps the original
arrangement. The merged clip still needs its original recordings. Use
[Collect sources](../projects/README.md#collect-project-sources) before moving the project
without them. Very complex merges or later operations that need too much
continuous audio report a preparation error; shorten the selection and retry.
Trimming or splitting a merged source can need continuous preparation, where
long silent gaps also count toward the limit.

## Merge MIDI clips

Select two or more MIDI clips on the same track. They can touch or have gaps. Choose **Merge**
from the **Clip** menu or a selected clip's context menu. You can also press
**Command+J** on macOS or **Ctrl+J** on Windows and Linux. This works in Timeline
and Editor. The result spans the first selected clip's start to the last selected
clip's end. Gaps remain silent; later notes keep their timeline positions.
Select any intervening clips too: Merge cannot cover an unselected clip.

You can also select one MIDI clip, including a looped or trimmed clip, and use
**Merge**. This rebuilds only its visible notes in one unlooped pattern. Notes
outside the trimmed range stay in the original source. The clip keeps its start,
length, name and color; its source offset resets to zero. Undo restores the trim
and loop.

The clips must have matching gain, transpose, enabled state and channel settings.
Merge creates one clip with the first clip's name and color. It keeps the visible
notes, including trimmed and repeated sections, in one unlooped pattern. Notes
remain separate at the old clip boundaries. Source patterns stay unchanged.
The merged clip is selected. Undo restores the original clips in one step.

One merge can contain up to 4,096 visible note occurrences. A larger operation
reports an error and leaves the clips unchanged. Shorten the selection before
retrying.

Beat patterns also support **Merge** with the same command and shortcut. See
[Merge beat patterns](beat-editor.md#merge-beat-patterns) for grid and length limits.

## Navigate and play

Play, Stop and Loop are separate standard buttons. They use the standard hover highlight and keep their green, red and gold active colors.

Press Space to play or pause from any view, pane, or focused control. Text fields take precedence: Space inserts a space while you edit text. Holding Space does not toggle playback repeatedly. Dialogs and blocking background jobs still disable the shortcut.

The transport toolbar groups **Metronome**, **time signature**, and **Tempo**, in that order, after a divider. Set the **time signature** between Metronome and Tempo. It starts at **4/4**. Drag the upper number up or down in steps of one to change beats per bar. Drag the lower number through **2, 3, 4, 6, 8, 12, 16** to change the beat note value. Double-click either number, type an integer from **1 to 65,535**, and press Enter. The text field keeps the displayed number’s width; longer input scrolls inside it. Escape or clicking elsewhere cancels text entry. The ruler and grid update during the drag. Release to apply the value to the project; Escape or loss of window focus cancels the drag.

The signature is saved with the project and supports Undo/Redo. It updates bar/beat positions, rulers, grids, snapping, and the metronome. Tempo stays in quarter notes per minute. Clips, notes, the playhead, and the loop range keep their exact timing. Existing pattern lengths and source duration settings keep their original durations.

Use **Metronome**, immediately before the time signature, to hear a click on each
beat during playback. A higher click marks the first beat of each bar. In 6/8, it plays six eighth-note clicks per bar. It
follows the project tempo and playhead through seeks and loops, and fades out
when playback pauses or stops. The button uses the standard idle and hover appearance, with a gold active
background. It starts off and remembers its choice
for the app session. It does not change the saved project. The click is a playback
aid; exports retain the project mix.

The ruler uses the saved time signature, with bars and beats counted from 1. In dark mode, labels outside the loop range use light text. Text on the enabled loop highlight uses dark text for contrast. Click to set the playhead. Drag the selection band to choose a loop range; use Loop to enable it. Changing tempo keeps clip starts and the loop range at their musical positions. Audio clip duration stays unchanged; MIDI and beat timing follow tempo.

At a transport loop boundary, MIDI and beat releases and one-shot samples
continue into the next iteration. Audio clips repeat only the selected loop
region. Track, group and Master effect tails continue across the boundary.

Seeking and editing notes during playback keep the current audio and playhead running while the change is prepared. A seek then fades to the requested position. A note edit takes effect at the current playback position, without pausing or returning to an earlier position. Changes to effect latency use the same continuous playback behavior.

Recently prepared positions use cached sound state for faster seeks. The first seek, a seek outside the cached range, or a seek after a project edit can still require history reconstruction. This can delay when a change becomes audible. Rapid changes replace pending changes. Pause stops the active audio even while a seek or edit is being prepared; ordinary pause and resume retain the current sound state.

Use **Fit all clips**, immediately left of the playback toolbar’s zoom control, to show every clip across all tracks. It fits from the earliest clip start to the latest clip end, including gaps. The button is disabled when there are no clips. It changes only the view.

Horizontal scrolling moves the timeline while track controls stay fixed. Pinch zoom keeps the pointer's time in place where possible. Double-click a MIDI clip to open it in [Editor](editor.md).

## Fades and crossfades

Hold Option on macOS or Ctrl on Windows/Linux before dragging an edge to
stretch. While held, fade and crossfade handles and midpoint controls are hidden
and disabled in Timeline and Editor. A fade drag already in progress keeps its
gesture until release or cancellation.

Audio clips have square fade handles in the strip between the name header and
waveform. Drag the left or right handle inward to create a fade. Drag it outward
to crossfade with the adjacent audio clip. The join stays at the same timeline
position; the blend uses original audio beyond the trimmed edge. Crossfades
require touching clips with matching mono/stereo layouts. Either or both clips
can loop. A looped clip continues its repeat range and phase through the blend,
including its usual loop-seam fades. A non-looped clip needs unused source audio
beyond the edge that extends into its neighbor. Gaps, muted clips and unavailable
source audio prevent a blend.

A circle on each curve controls its shape. Drag it horizontally to change its
position within the fade, or vertically to change its level. Neither changes
the fade duration. The S-curve starts at the top of its square handle. Handles and midpoint controls
have transparent bodies with dark borders, and the curve line is dark. The area
above the curve is darkened to show attenuation; waveform colors stay unchanged. Crossfades show both
opposing curves when you hover over the blend or drag a fade control.
Hover a clip to show its fade handles and midpoint controls. Clicking a
crossover selects the clip whose header and waveform extend across it. The owning clip’s header and source waveform extend into the neighboring clip
through the overlap region. The original side between the curves keeps its normal brightness. The extended
side between the curves receives two layers of shading; the upper and lower
regions receive one layer.
Drag the crossing point vertically to change the gain where the curves meet, or horizontally to change its time. Raising the crossing can increase the transition level; lowering it can reduce it. Both waveforms keep their colors. Their heights follow the fade curves, including while you drag a handle. Ordinary fades also reduce the displayed waveform height toward silence. Audio waveform bodies and crossfade headers have no extra border;
selected clips keep their lighter header color across the full visible crossfade. Clip
timing stays unchanged. For an ordinary fade, a dark vertical guide runs from its duration handle
through the waveform, marking the fade-in end or fade-out start. Crossfades show
a vertical guide at each edge only while the crossover is hovered. Both guides
start directly below the header and reach the waveform bottom. The crossover
shading also meets the header without a gap.
Release commits the change; Escape or loss of window focus cancels. When the dragged clip is selected, these controls edit all selected audio clips.
An unselected clip starts a new selection. Duration drags apply the same time
change to each clip. Midpoint drags apply the same gain and normalized-position
change, preserving relative settings. Each clip stops at its own limits; clips
without a usable adjacent source cannot crossfade. MIDI and beat clips are skipped.
The whole group previews before release and commits as one Undo step. Header looping, waveform trimming and
shared-boundary trimming keep their existing gestures outside the fade handles.

For selected audio clips, a divider separates the fade controls from the
other clip settings, and another divider precedes **Fade out**. Both use muted subheaders, with labels
on the left and values on the right. Each section contains Mode (Fade/Crossfade),
Duration in milliseconds, Shape and Position. The Mode selector fills the value
column. Drag numeric values up or down, hold Shift for finer adjustment, or
double-click to type a value. Shape sets the control point level from −70% to +70%; Position sets
its horizontal location from 15% to 85% between the clip edge and duration handle.
Shape 0% with Position 50% gives a balanced S-curve. These settings and the curve
handles edit the same values. For a group, the settings show one selected audio
clip as the reference. Duration, Shape and Position changes apply relative to
each clip's existing value; Mode applies the chosen mode to the selected audio
clips. Each clip retains its own source and duration limits. Fade duration is bounded by the clip. Crossfade duration is bounded by half
each adjacent clip and, for a non-looped owner, the available source audio. Zero duration
keeps the automatic 5 ms anti-click fade. A crossfade replaces the corresponding
neighbor boundary fade. If editing removes an eligible join, its crossfade
becomes inactive and the automatic boundary fade applies.

Older saved control points outside these ranges are clamped when the project
opens. Fades save with the project, support Undo/Redo, and sound the same in playback
and export. Copies retain their settings; trims and splits retain settings with
durations limited to the surviving clip. Stretch and project-rate changes scale
the saved durations. Loop seams retain their automatic anti-click fades.

## Clip settings

In Timeline, use the headphone-file button in the right toolbar to open Clip settings beside it. The panel can stay open with the Effects or Chords catalog on the left. Closing it gives its space back to the timeline. When the effects lane is hidden, the right toolbar extends to the status bar. Leaving Timeline closes this panel and discards any unconfirmed text. Audio tracks also offer these controls under **Clip options** in [Editor](editor.md).

Audio clips also have **Channels** and **Polarity** rows after Transpose, before
the fade sections. The joined **L** and **R** buttons independently enable the
left and right playback channels; both start enabled. The small **Invert**
checkbox beside them swaps the left and right audio signals before these channel
controls. It starts unchecked. Turning
both channels off silences the clip. Mono recordings still
use these controls for the left and right playback outputs. The separate small **Polarity > Invert** checkbox flips the
polarity of both channels; it starts unchecked. It does not reverse audio in time.
These settings affect playback and export without changing the recording.
For a group, a toggle is on when all selected audio clips have it on. Clicking
an off or mixed toggle enables it for all; clicking an on toggle disables it for
all. MIDI and beat clips are skipped. Changes support Undo/Redo and save with the
project. The waveform continues to show the source audio.

Select a clip to edit its name, gain, and transpose. The source row shows the audio file name, or MIDI pattern for a MIDI clip. These rows use the smaller catalog text, and the values are aligned to the left. The source is dimmed because it cannot be edited. A long name is shortened with an ellipsis; hover the source to see the full path. Gain is shown in dB, from silence (−∞ dB) to +60 dB, for example 0.0 dB. Transpose is shown in semitones, from −48 to +48, for example 0 semitones.

Drag gain up or down in steps of 0.1 dB. Drag transpose up or down in steps of 1 semitone. Hold Shift while dragging for a finer adjustment: 0.01 dB, or 0.1 semitone. A sideways drag does not change the value. Dragging up from silence starts at −60 dB.

Double-click the name, gain, or transpose to type a value, as with the tempo label. Enter saves it. Escape, or clicking away, restores the previous value. With no clip selected, the fields stay unavailable.

Gain changes the clip's level before the track level. Transpose shifts MIDI notes while they play and leaves the written pattern unchanged. A note that would fall below MIDI note 0 or above 127 plays at that nearest note. Audio transpose changes playback pitch while keeping the clip duration and original recording unchanged. It uses the project's selected [stretch engine](../audio/time-stretch.md#choose-an-engine). For audio, dragging previews the value and releasing starts preparation; Escape cancels the drag. Playback continues with the previous audio until preparation succeeds. Cancel or failure keeps the previous value and audio.

If the selection contains audio, Transpose is unavailable while project audio is preparing or paused. After a preparation error, use Retry and wait for completion before transposing audio. Name, Gain and MIDI-only Transpose remain available.

When several clips are selected, Name and Source are disabled and show “Multiple clips selected”, even when they match. Gain stays editable. Transpose follows the audio-readiness guard above. Each shows the shared value, or “Multiple values” if the selected clips differ. Dragging applies the same change to each clip, preserving their differences across tracks. For example, raising clips at 0 dB and −6 dB by 1 dB gives 1 dB and −5 dB. Movement stops for the whole group when any clip reaches a limit. Typed values set the same value on every selected clip. Undo restores the whole group in one step.

While dragging multiple clips, the label shows only the signed change applied so far, for example “+1.00 dB” or “−2 semitones”. This also applies when the clips start with equal values. The change stops increasing at a group limit. Gain shows two decimal places so fine adjustments remain visible. Releasing the drag restores “Multiple values” for differing values, or the shared value when they match.

Silent clips stay silent when selected with audible clips. The audible clips receive the gain change. If all selected clips are silent, dragging up raises them together from −60 dB, using the usual steps.

If any selected clip contains audio, transpose previews for the whole group until release. All selected clips, including MIDI clips, change together after preparation succeeds. Failure or cancellation keeps the previous settings and audio for the whole group. Changing the selection cancels an unconfirmed text edit or transpose preview.

## Track Mixer

Use the sliders button in the right toolbar, or **View > Track Mixer**, to show
the selected track and its parent bus side by side. The parent is the immediate
track group, or Master for a top-level track. The panel follows the current track
when several tracks are selected. Selecting Master shows only its strip. The Track mixer title stays visible above the strips, including when you scroll their controls.

Use the same gain, pan, mute, solo, sends and meters as in [Mixer](mixer.md).
Controls affect the strip shown without changing track selection. Scroll the
panel when its controls do not fit. With no track selected, select a track to
show its controls.

Track Mixer and Clip Settings share the right sidebar. Opening one closes the
other. The left catalog can stay open. Track Mixer starts closed and remembers
its visibility while you switch views during the app session.

## Track groups

Select track headers and choose **Track > Group selected tracks** to create a shared processing bus. Group and Master labels align left. Use the borderless group arrow to hide or show children. Drag tracks into a group, or move the complete group from its name or blank header area. See [Track groups](../editing/track-groups.md) for routing, shared effects, and Ungroup.

## Add effects

Select a track, group, or Master, then use the [Effects catalog](instrument.md#effects). Browse EQ & Filters, Dynamics, Distortion, and Imaging. Double-click an effect, select effects and press Enter, or drag them into the effects lane. Click a folder to hide or show its entries.

Drag an installed effect header from the effects lane onto another track’s header or clip lane to append it to that track’s effects. Audio tracks, MIDI tracks, and groups accept effects. Drop onto the fixed Master block to append effects to Master. Shift-click effects first to move a selected group in chain order. Hold Option on macOS or Ctrl on Windows/Linux to copy instead; the modifier at release determines the action. The target row has an outline during a valid drag. Release to apply the change, or press Escape to cancel.

A drop selects the destination track or Master and the transferred effects. Effects that use modulation sources from another track show a forbidden cursor and reject the whole group without changing either chain. Master cannot use track modulation sources. Remove incompatible source assignments before transferring effects.

## Stem separation

Right-click an audio clip and choose **Stem-separation** to create a group with aligned Drums, Bass, Other, and Vocals tracks. The original stays in place, disabled. Select local models in Settings first. Unsaved projects use temporary stem storage until the first save. See [Stem separation](../audio/stem-separation.md) for setup, progress, cancellation, and file storage.

Right-click a disabled clip and choose **Enable clip** to restore it. **Disable clip** silences only that clip. Its header and content become neutral grey, and its translucent body lets the grid show through in Timeline and Editor. These actions support Undo and Redo.

## Supported imports

Import mono or stereo WAV, MP3, AAC-LC, or Opus clips. AAC-LC can use `.aac` (ADTS),
`.m4a`, or `.mp4` containers. Opus uses Ogg `.opus` or `.ogg` files.
WAV supports PCM16, PCM24, and float32. Audio is converted to the project processing rate, and its waveform is prepared before
the clip is added. The original recording remains unchanged and must remain
available when reopening the project.

HE-AAC, protected audio, other codecs inside MP4 files, and files with more than
two channels are unsupported. Opus supports one mono/stereo stream
with the standard channel mapping (family 0); chained or multiplexed streams
and Ogg Vorbis are unsupported. Recorder files without a final end marker can
load if they end on a complete page. A rejected import leaves the arrangement intact.
M4A/MP4 imports reject metadata above 8 MiB or a single compressed packet above
16 MiB. Malformed table sizes and out-of-file packets return an import error.
Re-encode files that exceed these limits; the failed import leaves the project unchanged.

MP3 encoder delay and padding are removed when the file provides that information.
Opus imports remove encoder delay and end padding and apply the header output gain.
AAC files can retain encoder padding, so their clip length can be slightly longer
than the original uncompressed recording.

Each view retains its own upper left toolbar panel choice for the app session. Opening a panel closes the other upper panels in that view. Switching views restores that view's choice, including a closed panel. The left toolbar stays available when its panels are closed.

## Bounce to audio

Use [Bounce clips or Bounce tracks](../editing/bounce.md) to bake sound to new
audio tracks while retaining disabled originals. The progress dialog closes
automatically on success or cancellation. Editing remains disabled until the
result is ready or cleanup is complete; errors remain open with details.
