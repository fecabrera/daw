# Project time signature

## Accepted behavior

Place a `numerator/denominator` selector between Metronome and Tempo in every view. The shared timing section follows a standard vertical toolbar separator and orders Metronome, time signature, then Tempo. Reuse the bar.beat monitor painter: 13-point semibold Outfit, fixed-width digit slots, darker slash, shared toolbar padding and spacing. Each number is a separate vertical drag and double-click target with a tooltip and accessibility name.

Numerator drags change by one per four logical points. Denominator drags traverse 2, 3, 4, 6, 8, 12, 16. Double-click opens that number's text field at the unchanged display-slot width. Use the slot's full width as TextEdit's desired outer width, with one-point horizontal and two-point vertical margins. Do not subtract padding again or expand the monitor, slash, other number or following Tempo control. A one-digit value fits without clipping. Longer input scrolls inside the field and remains editable through 65535. Keep the actual number in the monitor's accessibility label.

Enter accepts any positive integer through 65535, including denominators outside the drag list. Reject zero, overflow and non-integer input with the shared error path. Escape, focus loss or disabled editing cancels; clicking away cancels text entry. A drag previews the number, rulers, musical positions, grids and snapping through a UI-only effective signature. Commit the saved project value once on release; cancellation restores the original display without an edit. Use the committed active renderer signature for metronome playback. A same-value release or text commit stays clean. Apply the existing text/gesture/job/dialog guards.

Store `Project::time_signature` as `TimeSignature { numerator: u16, denominator: u16 }`. Both values must be positive. Serde defaults a missing field to 4/4 for older manifests. `Edit::SetTimeSignature` validates and changes only this field through the existing transaction. Use session save/history and ordinary renderer publication. Retain tempo, clip starts/lengths, source offsets/repeats/stretch, MIDI tick data, playhead and loop endpoints exactly.

## Musical clock

Tempo remains quarter notes per minute. One displayed beat lasts `sample_rate * 60 / tempo * 4 / denominator` frames; a bar contains numerator beats. Anchor bar 1, beat 1 at frame zero. Use the signature for transport and selection positions, duration displays, position dragging, Timeline/Editor/Sampler rulers, grids and snapping. Beat cells retain their quarter-note source clock; resolution captions express the unchanged step as a fraction of the current bar. Existing pattern spans and Sampler Bars duration values retain their four-quarter-note source meaning; changing meter does not stretch or rewrite source content.

Keep frame/pixel coordinates unchanged. Add exact bar ticks to bounded power-of-two subdivision ticks, so coarse zoom still represents odd meters. Bar snap spacing is a whole power-of-two multiple of the numerator. Keep label spacing at least 80 points, omitting a non-bar label too close to the next bar. Position labels count from one; duration labels count completed bars/beats from zero. Round snapped and dragged frame boundaries up.

The [metronome](metronome.md) uses one denominator-note click per beat and accents every numerator beats. Thus 6/8 has six eighth-note clicks, with a single bar-start accent. Read the active renderer's signature, preserve 5 ms transitions on changes, and keep callback processing allocation-free. Preserve the current click level and offline/export audio.

## Acceptance

Check legacy default, save/reopen, invalid transaction rollback, Undo/Redo and exact retention of audio/MIDI/transport timing. Compare renders before/after a meter edit. Test 3/4, 6/8, 7/8 and non-power-of-two denominators across supported rates and zooms, bar grid/snap boundaries, fixed frame geometry, and bar label spacing. Test independent drag/text targets, bounds, invalid input, release-only commits, cancellation, toolbar placement and guards in both themes. Verify callback click grouping and preserve existing allocation/device-conversion tests. Native gestures, audible click balance and non-macOS platform checks remain separate acceptance checks.
