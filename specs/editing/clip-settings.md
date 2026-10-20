# Clip settings

Clip settings is an occupant of the shared 250-point catalog column in Timeline. It uses `panels::ControlPanel`, its `padded_body` helper, and `field_row` over the shared field layout. The helper owns horizontal padding and available content width. Form and catalog rows share the small-text height plus two standard padding units, with no extra body top gap or inter-row gap. Rows provide the vertical padding, keeping the first text baseline and row spacing aligned with the Effects catalog. The empty-selection prompt uses the same inset and row geometry. This supersedes the Library/Gallery Horizontal-only rail described in the 2026-10-14 toolbar sections. Those sections remain the record of that change.

## Visibility

- The left-rail toggle uses Lucide `FileHeadphone`. Its tooltip is “Show clip settings”.
- Timeline rail order is Library, then Clip settings. Instrument retains Library and Piano without a Clip settings toggle. Gallery Horizontal stays at the bottom.
- Each toggle is a borderless 24-point button. The gap between these buttons is 7 points.
- `clip_settings_visible` defaults to false. The Effects catalog stays the default open panel.
- Opening Clip settings closes the catalogs and clears their selections. Opening a catalog closes Clip settings. All panels can be closed.
- Toggling is UI state and does not mark the project modified.
- Clip settings is available only in Timeline. Leaving Timeline closes it and discards any field draft without a project edit. The shared view-switch path handles menu, button and shortcut changes; rendering also guards the panel by view. Instrument has no toggle or panel, and Editor/Mixer do not show this rail.
- Hiding the panel returns the column width to the workspace.

## Fields

The panel reads the complete selection through `selected_clip_snapshots`, shared with clip group editing. `selected_clip` supplies the primary value; the first snapshot is the fallback if there is no primary. With no selection, it shows “Select a clip” and does not offer editors. Jobs, dialogs, and the existing editing guards disable the fields.

Name, source, and gain are labels. Transpose is a label as well. Labels and values use `TextStyle::Small`, the catalog row size, through `egui::Label` with left alignment and truncation. Read-only values use `palette.secondary`: Source always, and every value while editing is disabled. Editable values use primary text. Source does not open an editor. Its tooltip shows the full path when the label is shortened. Name, gain, and transpose follow the transport tempo control: the second click on that value, within egui’s double-click time and distance, replaces the label with a text field of the same width and selects its text. The click record is cleared when the field opens, so the next single click does not open it. Enter commits. Escape or focus loss restores the saved value and closes the field. A single click does not open the field.

| Field | Interaction |
| --- | --- |
| Name | Double-click to edit. Enter commits a trimmed, nonempty name with no control characters through `Edit::SetClipName`. |
| Source | Read-only file name in the secondary color. MIDI clips show “MIDI pattern”. The tooltip shows the path. |
| Gain | The label shows the value with a dB suffix, for example `0.0 dB`. Drag vertically in 0.1 dB steps. Shift uses 0.01 dB steps. Horizontal motion does not change the value. Double-click edits the number through `faders::format_gain` / `parse_gain`, including −∞. Typed and dragged values clamp to `SILENT_GAIN_DB..=12`. Dragging up from silence starts at −60 dB. |
| Transpose | The label shows the value with a semitones suffix, for example `0 semitones`. Drag vertically in 1 semitone steps. Shift uses 0.1 semitone steps and keeps the unused fraction until it reaches a whole semitone. Horizontal motion does not change the value. Double-click edits a whole number from −48 to +48. Dragging clamps to that range. Typed values outside it are rejected. |

For multiple selected clips, Name and Source are disabled and show the literal “Multiple clips selected”, including when their values match. While idle, Gain and Transpose show their common value or “Multiple values” when they differ. These numeric fields reuse the same label, drag functions, parser, steps, bounds and inline editor as a single clip. Dragging applies a common signed offset to each clip's value: dB for Gain and whole semitones for Transpose. Typed commits still set one absolute value across the selection; mixed fields start their text editor from the primary value.

Use `bounded_delta` for both numeric drags. It restricts the common offset against every participating clip's remaining range, rather than clamping clips independently and losing their differences. Discard unused transpose fractions when group bounds clip a requested step. Subsequent reverse movement starts at the applied value, with no overshoot to unwind. Gain and MIDI-only transpose apply each incremental offset to current values. Audio transpose accumulates its bounded primary preview; release applies that total offset to each clip's unchanged original value before worker preparation. A return to zero offset needs no job or edit.

Silence has no finite dB difference. With audible and silent clips selected, gain drags retain silent values and use the primary audible gain, or the first audible gain when the primary is silent, as the drag reference. Only audible clips participate in the gain bound calculation and receive the common offset. All-silent selections retain the existing upward-drag admission at −60 dB through the shared gain control.

During a drag with multiple clips selected, show only the signed cumulative applied change: `+1.00 dB` or `−2 semitones`. Apply this display to both equal and differing starting values, including audio previews. Reuse the shared field label and numeric typography without the “Multiple values” prefix or parentheses. Gain uses two decimal places to expose Shift steps; Transpose uses whole semitones. The shared UI-only `DragChange` records field, primary ID, ordered selection IDs and cumulative offset. Feed it only the bounded change actually applied, including preview increments for audio. It never reports discarded pointer overshoot. Reuse the field label and layout; the complete readout must fit the available width. Clear it on release, cancellation, focus/selection changes, disabled editing, hiding the panel or leaving Timeline. Idle fields restore `Multiple values` when mixed or the shared numeric value when equal.

Shared gain/transpose edit builders accept a value mapping, omit unchanged clips and emit the existing typed core edits. Relative drags and absolute text commits use these same builders. Submit the complete field change through one `Edit::Batch` and the existing `DawUi::edit` path. A typed value equal to the primary must still update other selected clips that differ. An unchanged selection stays clean and creates no history entry. The existing pointer gesture history groups drag updates into one undo step; a text commit is one transaction. Unselected clips retain their settings.

The open text field stays in UI state. Closing the panel or changing the primary or selection membership discards it without saving. Store primary ID and ordered selected IDs on the field draft and audio transpose preview so a changed group cannot receive an old edit. If any selected clip is audio, transpose previews the entire group in UI state until release, then uses the shared audio worker once for the full batch. A zero-offset release leaves the selection unchanged. Escape, focus loss, selection changes, hiding the panel or leaving Timeline discards that preview. MIDI-only selections retain their existing live transpose drag updates.

## Playback

`Clip::gain_db` and `Clip::transpose` are saved on the clip. Zero values are omitted from JSON. Missing values load as zero.

Audio and MIDI playback multiply by the clip’s linear gain before track or Main gain. `RenderPlan::sample_at` and `AudioTrack::sample` apply it on the audio paths. `MidiTrackRuntime::sample_in_region` applies it to the instrument output.

MIDI transpose is added at note-on and clamped to MIDI 0–127. The pattern is not rewritten. `clip_pitch` is the shared conversion.

Audio transpose uses the project-selected Rubber Band R3 or Integrated engine through `PreparedStretch::with_pitch_cancellable`, the same path used by Sampler. Prepare from the original decoded asset with the clip's reduced stretch ratio (unity when absent) and signed semitone offset. Pitch never changes the prepared source length or clip duration; retain source offsets, repeat lengths/phases, start, gain, IDs and colors. No DSP algorithm or engine revision changes.

`daw-media::ClipAudioKey` identifies each asset/ratio/transpose request; `PreparedStretch` additionally checks source allocation, processing rate, engine/revision and pitch revision. Equal requests share buffers. Different pitches cannot alias. `clip_audio` is the common lookup for Session and RenderPlan, serving the prepared samples and peaks together. Missing/stale transformed audio never falls back to the original pitch. Unstretched zero-pitch clips use the original allocation directly.

`project::clip_audio` owns preparation and admission detection. Existing `prepare_stretches_cancellable` callers also prepare pitched, unstretched audio and prune unused variants. The shared `ui::audio_edits` transaction covers clip transforms and existing Sampler edits. It prepares a candidate Session on the existing job worker, then publishes project, all clip/Sampler buffers and peaks together through `Job::PreparedEdit`. Mixed audio/MIDI group changes publish together, including when one audio request fails. Equal requests continue to share prepared buffers. Cancellation, failure and dropped receivers retain the current session and playback. Compatible output publication uses the existing renderer preparation/retirement path; the callback only reads prepared PCM and performs no pitch work or heap operations.

Undo/redo retains complete prepared snapshots. Open, Save As, rate/engine changes, paste and stretch preparation use the same cache. No manifest migration is required: `Clip::transpose` already defaults to zero. Previously saved nonzero audio values now become audible. The original recordings stay unchanged; there is no pitch automation or formant preservation.

## Validation

Reject an empty or control-character name, a non-finite gain outside silence to +12 dB, and a transpose outside −48..=48. A rejected edit leaves the project unchanged.
