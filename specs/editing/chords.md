# Chord catalog

Timeline has an exclusive Chords catalog tab in its left toolbar, using the shared Lucide Piano icon, toggle shell, ControlPanel, Catalog, selection focus, folder rows, keyboard navigation, and drag threshold helpers. Effects, Clip settings, and Chords share the catalog column. Instrument retains its existing component catalog. Catalog visibility, root filter, selection, and folder expansion are UI state.

The catalog has 12 root folders, C through B with sharp spellings. The header filter defaults to All notes. Selecting a root exposes and opens only that folder; filtering clears catalog selection while retaining entry identities and other folder expansion choices. Filtering does not alter track scale options. Catalog header actions retain their own hit areas, separate from blank-header focus handling.

`crates/core/src/chords.rs` owns portable chord definitions, one-bar note generation, and `Project::add_midi_chords`. Chord roots are pitch classes 0–11, voiced at MIDI 60–71 (C4–B4). Every note starts at tick zero, lasts 3840 ticks (four beats), and has velocity 100. Generated notes, assets, clips, and new tracks receive independent IDs. Chords contain ordinary embedded MIDI patterns and require no project-format extension.

| Type | Semitone intervals |
| --- | --- |
| Major | 0, 4, 7 |
| Minor | 0, 3, 7 |
| Diminished | 0, 3, 6 |
| Augmented | 0, 4, 8 |
| Sus2 | 0, 2, 7 |
| Sus4 | 0, 5, 7 |
| Sixth | 0, 4, 7, 9 |
| Minor sixth | 0, 3, 7, 9 |
| Seventh | 0, 4, 7, 10 |
| Major seventh | 0, 4, 7, 11 |
| Minor seventh | 0, 3, 7, 10 |
| Half-diminished seventh | 0, 3, 6, 10 |

`midi_clip` shares asset/clip construction with normal empty MIDI pattern creation. Chord insertion clones the project, validates roots and target type, creates an empty default MIDI instrument when no target exists, and places the complete selected group through `Edit::OverwriteClips`. Place group chords sequentially in visible catalog order. Each length uses the project sample rate and tempo through the existing MIDI tick-to-frame conversion. Reject empty selections, missing or non-MIDI targets, invalid roots, and timeline overflow without publishing partial tracks, assets, or clips. Existing source patterns remain immutable when covered neighbors are trimmed or split.

`crates/ui/src/chords_catalog.rs` owns browsing and the drag lifecycle. Arm from the shared catalog press origin; select the captured group when the pointer crosses the standard drag threshold, including motion and release in one frame. Existing MIDI lanes or headers accept drops. Header drops use frame zero; timeline drops use its independent scroll/zoom and shared `placement_start` grid/edge snapping. Shift bypasses snapping. Space below the last visible track creates one MIDI track; audio/group lanes, rulers, side panels, and outside drops reject the entire group.

Cache a candidate project by snapped target and start frame during a drag. Preview both the incoming patterns and affected neighbors with the shared clip painter, MIDI note rendering, neighbor masking, and placement preview geometry. Show the normal invalid cursor when a candidate cannot be placed. Previewing, cancellation, and invalid drops leave project data, dirty state, history, playback, and track/clip selection unchanged. Cancel on Escape, focus loss, project/revision changes, view/tab changes, disabled editing, or a conflicting gesture. Commit only on release, using the same candidate shown in the preview.

Double-click inserts that chord at the playhead on the selected MIDI track. Enter inserts selected chords there in catalog order. Neither action creates a track or changes the playhead. Without a selected MIDI track they make no edit; dragging remains available to create a track. Shared catalog keyboard behavior retains Up/Down, Shift range selection, Enter folder toggling, Escape, focus guards, and held-key-repeat handling.

Publication goes through the existing history and `prepare_audio_edit` path. Prepare required Sampler pitches and stretches on the worker before publishing the project and complete clip selection together. Failures or cancellation retain the original project and prepared audio. Other views and editing commands remain guarded while a chord drag owns the pointer. Chord audition uses the independent prepared-buffer path described below.

Validation covers definitions and timing at all processing rates, distinct identities, overlap/source preservation, rejected edits, keyboard/filter/drag interactions, preview versus release, Shift snapping, cancellation, history, save/reopen, and deterministic export. See [validation history](../validation/README.md).

## Audition

A primary click or Up/Down movement onto a selected chord row auditions that chord through the currently selected MIDI track. The shared Catalog output reports the focused entry separately from the selected group. Folder movement, filtering, deselection, and drag gestures do not trigger audio. Double-click and Enter retain their insertion behavior. Without a selected MIDI track, selection remains available without audition.

`Session::audition_chord` runs on a worker. It clones the session, retains the selected track and its ancestor buses, clears arrangement clips, and inserts one temporary chord at zero. The shared Sampler/stretch preparation path supplies required pitches. The normal Renderer provides instrument layers, modulation, effects, track/group mix, and Master processing. Retain local mute; ignore other tracks' solo gates for the isolated preview. No temporary notes, assets, prepared pitches, transport changes, or history enter the live session.

Render a one-bar gate, capped at eight seconds, followed by one second for release and effect tails. Fade the first and last five milliseconds. The processing-rate buffer is bounded to nine seconds at every supported rate. The UI permits one worker and one replaceable pending request. New requests cancel stale work. Only a result matching the current project, revision, and selected track can reach output. Stop, Escape, window focus loss, leaving the Chords tab/view, blocked editing, track changes, and project edits cancel the preview. Worker and device errors use normal UI error reporting.

Output mixes prepared stereo buffers before the existing device-rate converter, independently of the arrangement renderer and transport. The bounded queue carries at most two buffers. Generation checks discard stale queued buffers and fade active cancellation over five milliseconds. A separate bounded retirement queue returns all buffer owners to the control thread; callback processing neither allocates nor frees buffers. Natural completion and queue saturation remain bounded. Preview audio reaches the main output meter; per-track and unit meters continue to describe arrangement playback.
