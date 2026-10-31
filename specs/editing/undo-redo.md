# Project undo and redo

Implemented scope supersedes the earlier MVP exclusion. Project editing uses a bounded session history; no manifest version change is required.

## Ownership and state

`daw-project::SessionHistory` owns the current committed snapshot, undo/redo stacks, an optional pending control gesture, and the saved project checkpoint. Each snapshot includes the project model and its decoded, stretched, and Sampler audio maps. Audio sample/peak storage remains shared through existing `Arc` handles. No file I/O, decoding, stretching, or serialization is needed to record or restore history. History operates on the control thread, outside the audio callback.

Snapshots also retain verified missing-source IDs and preparation warnings. An absent original PCM owner without a verified missing-source outcome remains unresolved. Restoring such a loading-time snapshot requests complete preparation for audio tracks and Sampler inputs before output publication. Reconcile readiness before the history action returns, so a Play request or another audio-dependent action in the same frame cannot consume unresolved media.

Project model types implement structural `PartialEq`. Snapshot comparison excludes the playhead, normalizes external source paths against the snapshot's original folder, and includes all other saved project fields. External/generated paths use absolute normalization. Included recordings retain relative paths and their snapshot folder. Shared source receipts preserve their identity across Save As. Restoring a snapshot retains the live save folder and playhead time, converting that time with rounded integer arithmetic to the restored sample rate and clamping only a playhead beyond timeline bounds. Stable IDs, routes, source offsets, repeat metadata, and prepared audio are restored together.

History retains at most 100 undo entries. Undo and Redo together also have an estimated 128 MiB byte budget: owned snapshot metadata plus PCM/peaks retained only by those history states. Shared audio counts once, and audio also held by current/pending snapshots is excluded from that history charge. Current/pending models and the saved checkpoint are reported separately and are not evicted. New committed edits clear Redo; structural no-ops do not.

After record, group completion, Undo or Redo, enforce the byte limit by removing oldest Undo entries, then the farthest Redo entries if necessary. Retain the nearest redo sequence. Cache each snapshot's metadata estimate; refresh deduplicated audio ownership on changes rather than painting. Dropping entries releases their shared owners outside the callback. Before a blocking job, shared media usage within 256 MiB of its 2 GiB limit can release all optional Undo/Redo entries. Current, pending and saved state remain intact. This does not guarantee admission when essential owners still retain the audio.

The UI reports memory-driven removals once through the existing visible notice, after any blocking job closes. Count-only expiry at 100 entries retains its ordinary behavior. See [memory accounting and limits](../architecture/performance.md#estimated-memory-policy).

## Publication and grouping

The UI initializes a baseline before handling edits. `mark_dirty` records validated changes; `changed` also requests the audio plan update. MIDI edits enter logical history immediately even when their audio is pending. Derived-audio completion refreshes matching snapshots without adding a history entry, replacing the saved checkpoint, or clearing Redo. Audio failure/cancellation retains those logical edits; invalid core edits and failed transactional source/settings jobs do not record a state. Explicit seeks update save revisions but bypass edit history. Live playback movement never records history.

Completed worker edits publish before a new pointer press begins its own group. A pointer press in an available editing environment begins a group. Successful control updates become its pending snapshot. Release or focus loss closes the group; only its final changed state enters history. Returning to the original state preserves Redo. Release-only clip/unit/group edits retain their existing atomic transactions. Discrete commands and keyboard edits each commit a step.

Undo/redo publishes the retained session, clears stale control drafts and unit selection, prunes track/clip selections, and requests the existing asynchronous renderer synchronization. A restored sample-rate or buffer-size change retires output, clears the rate-dependent clipboard, and leaves playback stopped, as the existing settings path does. Other edits retain the output and its transport intent. Matching prepared buffers are restored where available. A restored MIDI revision whose complete bank is unavailable requests background preparation while the previous playable revision remains active. No partial bank is sent to Output.

## Saving and lifecycle

The exact successful save snapshot becomes the saved checkpoint, including when editing or undo continues during regular Save. Save revisions still protect against stale completion. Explicit seek revisions independently track unsaved navigation. Undo/redo increments the change revision and derives the unsaved marker from the current project checkpoint plus navigation revisions.

Save As keeps history and its new save destination; a changed project name is a metadata edit. New, Close, and successful Open reset history. No history is serialized. External exports and source files are not undone.

On blocking save completion, call shared `SessionHistory::mark_saved` before recording the returned session. It retargets project-owned asset locations in retained snapshots before structural comparison. Relocation alone must not clear Redo or add an Undo entry; a real project-name change still records an edit. Regular Save only marks its exact written checkpoint and must not record or publish that older snapshot over newer edits.

Regular Save and blocking Save As capture `Session::save_snapshot(history)` and return `SavedSession`. Apply its transient receipt to live/current/pending/Undo/Redo owners before releasing it. Keep newer edits, transport, prepared buffers and navigation/save revisions; mark only the exact written checkpoint saved. Included and generated history-only media is copied for Save As even when absent from the current manifest. A save for another project or a failed worker must not update live paths or history. Path publication validates all models first and does not evict/reorder history. See [source bundling Stage 3](../projects/source-bundling-stage3.md).

## Commands

`HistoryAction` supplies labels and exact shortcuts to both UI and native menus. macOS Edit exposes Command+Z and Command+Shift+Z. Windows/Linux Edit exposes Ctrl+Z and Ctrl+Shift+Z. Disabled stacks stay disabled. Shift+Z cannot fall through to Undo, held-key repeats are ignored, and native dispatch removes a duplicate forwarded event.

Project actions respect text editing, window focus, active edit gestures, jobs, and dialogs. Text fields retain egui's text undo. Native menus use the same availability and dispatch methods as the application menu.

## Acceptance

- Restore complete grouped project edits with IDs and routes intact; reject failed edits without consuming history.
- Group continuous pointer edits and preserve Redo on a return to the original state.
- Keep playhead movement and selection outside history; preserve time across restored processing rates.
- Restore imported and prepared audio without source access; keep paths valid after Save As.
- Track clean/dirty state across saving, newer edits, undo, redo, and explicit seeks.
- Test Command/Ctrl shortcuts, exact Shift handling, repeat suppression, text ownership, menu actions, blocking work, and lifecycle reset.
- Use shared renderer synchronization and retain allocation-free audio callback behavior. Native menu/device checks remain separate from egui tests.
