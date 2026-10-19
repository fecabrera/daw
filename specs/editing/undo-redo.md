# Project undo and redo

Implemented scope supersedes the earlier MVP exclusion. Project editing uses a bounded session history; no manifest version change is required.

## Ownership and state

`daw-project::SessionHistory` owns the current committed snapshot, undo/redo stacks, an optional pending control gesture, and the saved project checkpoint. Each snapshot includes the project model and its decoded, stretched, and Sampler audio maps. Audio sample/peak storage remains shared through existing `Arc` handles. No file I/O, decoding, stretching, or serialization is needed to record or restore history. History operates on the control thread, outside the audio callback.

Project model types implement structural `PartialEq`. Snapshot comparison excludes the playhead, normalizes external source paths against the snapshot's original folder, and includes all other saved project fields. Normalized absolute paths preserve source identity across Save As. Restoring a snapshot retains the live save folder and playhead time, converting that time with rounded integer arithmetic to the restored sample rate and clamping only a playhead beyond timeline bounds. Stable IDs, routes, source offsets, repeat metadata, and prepared audio are restored together.

History retains at most 100 undo entries, plus current state and a pending gesture. Redo uses the same retained states. New committed edits clear Redo; structural no-ops do not. Metadata is cloned, and audio is shared. The count limit does not impose a separate byte limit; history can retain old prepared buffers within the existing audio resource budget.

## Publication and grouping

The UI initializes a baseline before handling edits. `mark_dirty` records successful changes from both direct edits and worker publication; `changed` also requests the existing audio plan update. Failed or cancelled background work does not record a state. Explicit seeks update save revisions but bypass edit history. Live playback movement never records history.

Completed worker edits publish before a new pointer press begins its own group. A pointer press in an available editing environment begins a group. Successful control updates become its pending snapshot. Release or focus loss closes the group; only its final changed state enters history. Returning to the original state preserves Redo. Release-only clip/unit/group edits retain their existing atomic transactions. Discrete commands and keyboard edits each commit a step.

Undo/redo publishes the retained session, clears stale control drafts and unit selection, prunes track/clip selections, and requests the existing asynchronous renderer synchronization. A restored sample-rate or buffer-size change retires output, clears the rate-dependent clipboard, and leaves playback stopped, as the existing settings path does. Other edits retain the output and its transport intent. Prepared buffers are restored rather than rebuilt.

## Saving and lifecycle

The exact successful save snapshot becomes the saved checkpoint, including when editing or undo continues during regular Save. Save revisions still protect against stale completion. Explicit seek revisions independently track unsaved navigation. Undo/redo increments the change revision and derives the unsaved marker from the current project checkpoint plus navigation revisions.

Save As keeps history and its new save destination; a changed project name is a metadata edit. New, Close, and successful Open reset history. No history is serialized. External exports and source files are not undone.

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
