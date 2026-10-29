# Project source bundling — Stage 3 save and history integration

Date: 2026-10-29.

Status: included-source open/Save/Save As, collection receipt application and protected-history saves implemented. [Stage 4](source-bundling-stage4.md) now exposes the desktop collection command. Native platform and crash/durability acceptance remain separate.

## Save snapshots and publication

[`Session::save_snapshot`](../../crates/project/src/source_save.rs) captures the live Session and optional current/pending/Undo/Redo roots without file I/O. Clones share prepared audio and temporary-stem owners. The saved dirty-comparison model is not a restorable audio owner and does not add media copies.

Run `SaveSnapshot::save` or `save_new` on a worker. The resulting `SavedSession` holds the exact written Session and a transient relocation receipt. Desktop ordinary Save, first Save and Save As all capture protected history through this API. Headless callers without history can continue using `Session::save` and `save_new`. Callers that retain history must supply it when capturing a save, especially for Save As.

The worker handles project-owned imported recordings and generated stems through one path. External recordings remain references. Same-folder Save retains included paths and existing missing-source warnings without decoding those files again. It does not collect subsequent imports. Save As reserves independent regular files under `assets/sources/` or grouped `stems/` directories. Canonical source identity and storage class deduplicate compatible copies across current and history-only assets. Generated filenames/provenance remain intact; new group directories use UUIDs.

Save binds its store to the canonical physical destination before reserving assets. Root and ancestor aliases remain supported; entries below that approved root still pass the ordinary-component/reparse checks. Filesystem reads/writes use the physical root. A confirmed result retains the caller's folder spelling only if it still resolves to that root; an alias change keeps the physical folder with a warning. Missing owned paths resolve through their nearest existing ancestor before same-folder reuse checks. Ordinary Save keeps missing-media references through root/ancestor aliases; Save As still rejects missing media that requires copying.

Copies use a bounded 64 KiB transfer buffer, exact length checks, source/destination SHA-256 verification, file sync and uncached comparison with every distinct retained prepared version. A per-save cache keys verified destination paths and encoded revisions, then matches processing rate, metadata and PCM Arc identity. Shared owners reuse a successful comparison; different PCM owners or revisions still require independent verification. Recheck source and destination content revisions before publication. A missing or changed source needed for copying prevents Save As. The worker validates relocated models before publishing the manifest through the shared native replacement helper. It reuses Stage 2 bounded handle retention, conservative cleanup and directory-sync rules. An uncertain replacement retains media and the candidate manifest for recovery rather than deleting possibly referenced files. It does not automatically collect, prune, reuse orphan files or delete old project media.

`SavedSession::apply(live, history)` merges only captured source owners that still match asset ID, original provenance/storage, resolved path and PCM/rate/metadata identity. It prepares and validates all live/history models before changing any of them. Newer edits, audio maps, processing caches and playhead remain intact. Uncaptured included owners cannot be silently rebased to a different project folder. A different project is rejected. The receipt is discarded after application; no UI path cache is added.

The exact written, normalized model becomes the saved checkpoint. Path publication does not finish a pending gesture, add an Undo entry, clear Redo, reorder history or evict entries. Normal edit and preparation-pressure boundaries still enforce history limits. A real Save As name change remains a metadata edit. Older ordinary-save completions cannot publish their arrangement over newer edits or clear their unsaved state. Blocking Save As accepts only the saved name and source locations; it retains the live playhead, selections and prepared audio, and reports new save warnings through existing notices.

## Included paths and history

The Stage 0 session I/O gate is removed. Open resolves included media within `assets/sources/`, with portable syntax and ordinary-component checks. Reject internal and external symlink/reparse aliases; their names need not survive moving the project. Missing ordinary files retain the existing warning, empty-waveform and silence behavior.

History normalizes external/generated references to absolute paths using pure path resolution. Included recordings retain their required relative paths and the matching snapshot folder. Save/collection receipts relocate all protected owners before history restore adopts the new live folder. Prepared PCM, stretch/pitch/Sampler buffers, source/clip identities and mappings stay shared and unchanged. Temporary-stem owners retire only after their references move; collected media is never owned by temporary-directory cleanup.

`SessionHistory::mark_saved` and `Session::retarget_project_sources` support existing current-session save callers. They validate changes before publication and preserve newer edits. They cannot copy an uncaptured history-only source: use a history-aware save snapshot for Save As. Marking an incomplete included-source Save As fails rather than assigning its history a wrong relative-path base.

## Collection receipt application

[`CollectionOutcome::apply`](../../crates/project/src/source_collection/execute.rs) accepts only a confirmed commit. It checks the captured project/job revision, live/history models, owner identities and root ordering through `SourceCollectionSnapshot::is_current`. Playback-only playhead changes remain allowed. Recovery-required, foreign and stale outcomes do not mutate live/history state.

The executor prevalidates retained relocated models before filesystem commit. Apply uses the same pure relocation path as save completion; it performs no file I/O, decoding or DSP preparation. Keep the host's edit/job exclusion active through apply. Preserve UI selections and control state; path relocation does not request renderer synchronization. Stage 4 connects this API to guarded menu/job dispatch and surfaces commit/recovery warnings.

## Acceptance and limits

[Source-bundling integration tests](../../crates/project/tests/source_bundling.rs) cover collection and Save As with original files and the old folder removed; imported and generated Redo-only media; pending and Undo owners; dirty checkpoints; stable PCM/cache Arcs and navigation; stale/foreign receipts; uncaptured owners; missing included media; later external imports; failed destinations and changed encoded bytes. Sampler and Granulizer inputs are covered without timeline clips. Collected MP3/AAC/MP4 sources retain their full encoded bytes through Save As; output extensions come from verified codec/container metadata. Trimmed, repeated, stretched and pitched clips retain their mappings, prepared buffers, deterministic render and byte-identical WAV export across collected Save As/reopen, using both saved stretch engines.

Existing generated-stem lifetime, Save As failure cleanup, editable background-save and UI selection/history tests remain required. See [validation history](../validation/README.md) for executed checks. Automated local filesystem tests do not establish native Windows/Linux, device/UI, network filesystem, process-interruption or power-loss acceptance. Stage 4 exposes the desktop command; the complete portability release gate remains pending.
