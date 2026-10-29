# Project media source bundling

Status: Stages 0–4 are implemented. Stage 5 native portability/release acceptance remains pending. Contracts: [Stage 0](source-bundling-stage0.md), [Stage 1](source-bundling-stage1.md), [Stage 2](source-bundling-stage2.md), [Stage 3](source-bundling-stage3.md) and [Stage 4](source-bundling-stage4.md).

Date: 2026-10-29.

## Goal and first-version scope

Make a saved project portable by copying its media sources into its project folder, preserving the original files and all editing/audio behavior. Start with an explicit **File > Collect project sources…** action. Normal imports and saves continue to reference external media until the user collects it.

The first version follows these rules:

- Copy full original WAV, MP3 and AAC source files without trimming, resampling or re-encoding. Preserve source metadata and project-rate frame coordinates.
- Include every non-MIDI asset retained by the current project, including unused assets and Sampler/Granulizer inputs without timeline clips. Embedded MIDI patterns already travel in the manifest.
- Preserve generated stems and their existing project-owned lifecycle. Do not rewrite their provenance or move existing `stems/` contents merely to normalize the layout.
- Protect sources retained only by current session history. Collection must not make an Undo or Redo operation depend on an old external location after that source has been collected.
- Deduplicate repeated references to the same resolved file and revision. Different files with the same basename remain distinct. Byte-content deduplication across unrelated files is deferred.
- Store collected recordings under `assets/sources/` with collision-safe, portable names and project-relative manifest paths. Preserve asset, track, clip, layer and unit IDs, names, offsets, loop phases, stretch ratios, colors and settings.
- Publish copied files and the manifest as one controlled operation. Failure or cancellation before commit retains the old manifest, live session, prepared audio and playback.
- Never modify, move or delete original recordings. Do not collect decoded PCM, waveform peaks, prepared stretch/pitch buffers or seek caches.
- Do not prune unused assets, remove bundled files, clear history, or add automatic import collection in this version.

## Current implementation findings

| Existing behavior | Implementation implication |
| --- | --- |
| [`Session::source_path`](../../crates/project/src/lib.rs) resolves absolute paths and paths relative to `Session::folder`. Relative paths can contain `..`. | Internal paths are supported, but a relative path alone does not establish project ownership or portability. |
| `Session::save` creates `assets/`, writes a temporary manifest and replaces `project.json` after success. | Reuse the manifest publication path; stage media before its commit point. |
| [`source_save`](../../crates/project/src/source_save.rs) copies project-owned recordings/stems across protected save roots. | Reuse transient source receipts and shared reservation/cleanup for save and collection publication. |
| [`Source`](../../crates/core/src/lib.rs) retains `kind`, `path` and `path_kind`; Stage 0 adds optional typed `storage` with legacy ownership defaults. | Preserve provenance and path semantics; use explicit ownership for subsequent inventory/copying. |
| [`sampler.rs`](../../crates/project/src/sampler.rs) imports sample files into the asset table. | Inventory assets rather than only scanning timeline clips. |
| [`SessionHistory`](../../crates/project/src/history.rs) retains complete sessions with absolute source paths. save snapshots and receipts retarget project-owned sources. | Retarget all collected source locations across retained snapshots without clearing Redo or replacing newer edits. Stage 3 now retargets all captured owners and enables included-source session I/O. |
| [`regular_save.rs`](../../crates/ui/src/regular_save.rs) saves a snapshot while the live project stays editable. | Keep this behavior; ordinary Save must not silently become a blocking collection operation. |
| [`media::cache`](../../crates/media/src/cache.rs) checks file length/mtime around decoding; [`AudioData`](../../crates/media/src/lib.rs) has no bound content receipt. | Stage 0 supplies streaming SHA-256 receipts and uncached worker revalidation; Stage 2 verifies staged bytes against prepared audio. |

This feature extends the initial external-source-only MVP. Current requirements in [MVP storage](../architecture/mvp-scope.md#project-storage) and [iteration 1](../architecture/iteration-1.md) now describe explicit collection and included-source saves. Historical validation records retain their original scope.

## Planned user behavior

1. Save the project before collection. For the first version, disable Collect on an unsaved project and explain that it must be saved first. Keep the existing first-Save dialog flow unchanged.
2. Collect shows a preflight summary: files already included, files to copy, additional bytes, history-only sources, and any missing or changed sources. It states that successful collection also saves the current project state.
3. The user starts a worker job with byte/file progress and cancellation. Block project edits and competing file jobs during this operation, using existing guards. Playback can continue from the existing prepared buffers.
4. Success saves the new source locations, updates live/history references, and reports the result. Do not add a path-relocation Undo entry or clear Redo. The written snapshot becomes the saved checkpoint.
5. Missing, unreadable or mismatched required sources prevent collection. Report affected files/assets and retain the project. Do not silently omit files or claim a partial result is portable.
6. Later imports remain external until the next collection. Compute collection status from the source inventory; do not persist a flag that claims all future imports are bundled.

Automatic collection, selective source-management UI, a separate portable-copy action, ZIP archives and a CLI collection command are follow-up work. The shared backend must permit those callers without duplicating file or manifest logic.

## Stage sequence

Each stage has an exit gate. A gate records evidence for that stage; it does not imply that later stages or native platform acceptance have passed.

| Stage | Status | Deliverable | Exit gate |
| --- | --- | --- | --- |
| 0 | Complete | Storage, revision and transaction contract | Model/migration rules, failure cases and fixture matrix are explicit. |
| 1 | Complete | Shared source inventory and preflight | All live/history roots are covered and diagnostics are deterministic. |
| 2 | Complete | Transactional collection backend | Copies and manifest publication survive cancellation and injected failures. |
| 3 | Complete | Save, Save As and history integration | Reopen, relocation and history remain independent of originals. |
| 4 | Complete | Desktop command and job UI | Guarded menu dispatch, progress, cancellation and completion use the shared backend. |
| 5 | Pending | Portability acceptance and documentation | Required automated/native evidence and current docs match the delivered feature. |

## Stage 0 — Define storage and source revisions

Implemented contract and compatibility decisions: [Stage 0](source-bundling-stage0.md). Stage 4 now exposes the guarded desktop collection action. Current imports remain external; Stage 3 now enables included-source session I/O.

Work:

1. Separate storage ownership from provenance. An imported recording remains an imported recording after collection; a generated stem remains generated. Do not repurpose `path_kind` to mean ownership.
2. Define a typed storage marker or equivalent model contract in `daw-core`. Legacy external references retain their behavior; legacy generated stems retain their project-owned behavior. Embedded MIDI keeps its existing representation. Decide and document the manifest-version/migration rule before changing serialization.
3. Define included-path validation: project-relative paths, portable separators/names and containment within approved project media directories. Resolve symlinks when checking containment; reject included paths that escape through `..`, absolute paths or an external symlink.
4. Define source revision receipts for import/open and collection. Length and mtime checks around copying detect ordinary concurrent changes, but are not enough to prove that an older decoded buffer matches current disk contents. Specify a content-verification or worker-side revalidation path for legacy/already-open assets without a trustworthy receipt. Reject mismatches instead of silently changing the sound or encoding cached PCM as a replacement original.
5. Define the commit point as successful manifest replacement. Specify pre-commit rollback and post-commit error/durability reporting separately. Once a manifest references new media, cleanup must not delete that media.
6. Freeze naming, destination reservation, retry behavior, cancellation checkpoints, checked byte totals and bounded streaming-buffer requirements.

Exit gate:

- Unit fixtures cover legacy external/generated/MIDI models, included storage, invalid path escapes and conflicting source revisions.
- Defaults and migrations preserve existing sessions. Older-client behavior is documented rather than assumed.
- No application command is exposed yet.

## Stage 1 — Inventory and preflight

Implemented API, ownership, diagnostics and destination rules: [Stage 1](source-bundling-stage1.md). Preflight remains read-only; Stage 4 exposes it through desktop confirmation. A verified source can have separate imported/generated targets when their storage contracts differ; compatible references share a target.

Work:

1. Add focused source-management modules in `daw-project`. Keep filesystem resolution/copy planning out of `daw-core`, UI painting and the audio callback.
2. Build an immutable collection plan from all current audio assets and protected history roots. Include current, saved, pending, Undo and Redo source references as applicable. Do not serialize history into the project.
3. Resolve each snapshot against its own original folder. Group references by resolved file and verified revision while retaining every owning asset ID. Inspect clipboard/retained component references for any additional source ownership; reuse the asset table where they only hold IDs.
4. Identify already-included media and generated stems, new copies, duplicate references, missing/unreadable sources and revision conflicts. History-only media is counted separately in the preview and may require protected files that are absent from the current manifest.
5. Plan collision-safe destinations under `assets/sources/`, independent of duplicate basenames, case differences or source folders. Copy regular-file contents; do not create external symlink/hardlink dependencies.
6. Calculate additional storage and progress totals with checked arithmetic. Treat free-space estimates as advisory checks; actual I/O failures still need safe rollback. Preflight must not create files or modify the project.

Exit gate:

- Tests cover sampler-only/granular sources, unused assets, embedded MIDI, stems, shared source paths, duplicate basenames, and history-only sources.
- Missing files and incompatible revisions produce a complete, actionable report.
- Repeated preflight on the same snapshot produces the same plan and leaves the working tree/session unchanged.

## Stage 2 — Transactional collection backend

Implemented worker API, commit/cleanup rules and failure coverage: [Stage 2](source-bundling-stage2.md). Native Windows and crash/power-loss acceptance remain open. Stage 3 supplies session/history integration; Stage 4 now supplies desktop dispatch.

Work:

1. Execute a validated plan on a worker. Recheck source and project revisions before copying and before publication; reject invalidated plans.
2. Stream bytes into a uniquely reserved staging directory on the project filesystem. Check cancellation between chunks and verify size/content and source stability before accepting a copy. Keep memory bounded independently of total media size.
3. Reserve final filenames without replacing existing or unrelated files. Reuse an existing collected file only after verifying that it is the intended content. Copy repeated references once.
4. Build a candidate manifest and a relocation receipt mapping original asset/source identities and revisions to their new locations. Preserve prepared audio and all editing metadata.
5. Publish complete media before atomically replacing the manifest. Apply the Stage 0 durability contract, including replacing the current Windows backup/rename gap with native atomic manifest replacement. Never publish references to incomplete copies.
6. Before commit, remove only this operation's staging/new files on failure; preserve originals, pre-existing media, unrelated files and the prior manifest. After commit, retain referenced media even if a later notification fails.
7. Define safe crash/retry handling. An interruption before manifest replacement may leave unreferenced complete files, but the old project must remain valid. Do not add automatic garbage collection or guess that an orphan file is safe to delete/reuse.

Exit gate:

- Byte-for-byte tests prove that source files are copied without re-encoding or changing originals.
- Fault injection covers cancellation, source mutation, read/write failure, disk exhaustion, occupied destinations and manifest replacement failure.
- Post-commit failure tests prove that a valid manifest never loses its media through rollback cleanup.

## Stage 3 — Save, Save As and history

Implemented save snapshots, receipt application and acceptance: [Stage 3](source-bundling-stage3.md). Stage 4 now supplies the desktop collection command.

Work:

1. Extend the existing generated-source relocation path into shared project-owned source handling. Keep `stems/` and temporary-stem ownership compatible.
2. Normal Save retains included locations and external references; it does not collect later imports automatically. Preserve revision-aware, editable background saving.
3. Save As copies included media and generated stems to the new project, retaining external references only where collection has not included them. Copy protected history-only media when it is needed to keep that active session's Undo/Redo independent of the old folder.
4. Apply the relocation receipt to live state and every retained source owner. Generalize the current `retarget_generated_sources`/`mark_saved` behavior rather than adding a second history/path cache in the UI.
5. Retain IDs, prepared audio Arcs, selection, transport, control gestures and history ordering. Path relocation alone must not call the ordinary edit-recording path, clear Redo or resynchronize DSP unnecessarily.
6. Release temporary source owners only after all protected references have moved. Do not delete collected files when a clip, asset, unit, history entry or clipboard selection is removed in this version.

Exit gate:

- Collect, Save, Save As and reopen preserve byte-identical deterministic renders.
- Undo/Redo can restore imported/sample/stem content after the original files or old project folder become unavailable.
- Saving after Undo does not restore old external dependencies, and saving an older worker snapshot never overwrites newer edits or clears their dirty state.
- Existing Save As failure cleanup, generated-stem lifetime and regular-save tests continue to pass.

## Stage 4 — Desktop command and job UI

Implemented command, guards and result handling: [Stage 4](source-bundling-stage4.md). Native menu, large-copy/device and cross-platform release acceptance remain Stage 5 work.

Work:

1. Add Collect project sources to the shared File action model, native desktop menus and application-drawn menus. All entry points dispatch the same guarded action.
2. Require a saved project, focus and settled editing state. Disable during ordinary Save, conflicting jobs, dialogs and active gestures. Allow a no-copy result to explain that the project is already collected.
3. Present the Stage 1 summary through shared dialogs, then run the Stage 2 backend through the existing job/progress/cancellation paths. Keep discovery, hashing and copying out of paint/input handlers.
4. Publish successful relocation through Stage 3 and show file/byte totals. Distinguish cancellation, pre-commit failure and a confirmed commit with a later durability/reporting issue.
5. Keep playback on its existing prepared buffers and preserve view/track/clip/unit selection. A failed collection must not stop or replace a working session.

Exit gate:

- egui tests cover enabled/disabled actions, menu dispatch, preflight, progress, cancellation, errors, no-op collection and successful publication.
- A job cannot publish into a different project or overwrite state after a view/project lifecycle change.
- Native menu and large-copy interaction checks are recorded separately from automated UI tests.

## Stage 5 — Portability acceptance and documentation

Acceptance matrix:

| Case | Required result |
| --- | --- |
| Move the collected folder; make original files unavailable | Reopen without missing collected sources; playback/export stays equivalent. |
| Audio clips, sampler-only inputs, Granulizer and generated stems | All media resolves within the moved project; embedded MIDI remains intact. |
| Trimmed, repeated, stretched and transposed content | Original files remain complete; clip/sample mappings and prepared output stay unchanged. |
| Shared source references and duplicate basenames | No redundant copy for the same verified file; distinct sources cannot overwrite each other. |
| Collect again; import new external media; collect again | Existing collected files are retained; only newly required files are copied. |
| Save As followed by removal of the old project folder | New project and retained history use their own included files. |
| Missing/changed media, cancellation and injected I/O failures | No partial manifest or dangling source references; previous project/playback retained before commit. |
| Undo/Redo and history-only source revisions | Paths remain valid; incompatible source versions are reported rather than silently substituted. |
| Windows/macOS/Linux paths and filesystems | Unicode names, case differences, symlinks, destination containment and native rename behavior are verified. |

Implementation validation follows [repository instructions](../../AGENTS.md#validation): affected core/project/media/UI tests, formatting, strict workspace/all-target Clippy, workspace tests, CLI smoke checks when shared project behavior changes, and the macOS desktop rebuild. Use temporary fixtures and fault injection; never move/delete user recordings to test portability. Record native platform/device checks and any unavailable coverage explicitly.

Update [project user guidance](../../docs/projects/README.md), [roadmap](../../docs/roadmap.md), [README](../../README.md), [storage requirements](../architecture/mvp-scope.md#project-storage), relevant iteration/save/history requirements and [validation history](../validation/README.md) as each stage lands. Keep planned behavior separate from available features until the desktop command ships.

The first release gate is a saved project collected, moved and reopened with originals unavailable, including sampler-only and history-only sources, plus passing failure-path and platform evidence. Do not claim portability from successful copying alone.

## Follow-up scope

- A persistent project policy to collect media on import/save.
- A source manager for selective inclusion, relinking and explicit unused-file cleanup.
- Save portable copy without changing the current project location.
- Optional ZIP distribution and a CLI collection command over the same backend.
- Content-based deduplication across different source files.

These items do not block the explicit collection command and are not implicit first-version requirements.
