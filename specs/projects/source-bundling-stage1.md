# Project source bundling — Stage 1 inventory and preflight

Date: 2026-10-29.

Status: shared snapshot, inventory and preflight APIs implemented. The [Stage 2 copying backend](source-bundling-stage2.md) is implemented; [Stage 4](source-bundling-stage4.md) now exposes the desktop command. [Stage 3](source-bundling-stage3.md) enables included-source session open/save and history relocation. See the [staged plan](source-bundling.md) and [Stage 0 contract](source-bundling-stage0.md).

## Ownership and entry points

[`daw-project::source_collection`](../../crates/project/src/source_collection/mod.rs) owns collection snapshots, source owners, reports and sealed plans. [Preflight](../../crates/project/src/source_collection/preflight.rs) resolves/verifies sources; [target planning](../../crates/project/src/source_collection/targets.rs) assigns compatible destinations; [path checks](../../crates/project/src/source_collection/paths.rs) validate output trees. History exposes borrowed roots internally; it does not maintain another source cache or change edit ordering.

| API | Contract |
| --- | --- |
| `Session::source_collection_snapshot(history, project_revision)` | Capture the live session and optional same-project history without file I/O. Preserve the complete live project, including its playhead. Clone metadata and share PCM, caches and temporary-stem owners. Validate captured project models and project identities. |
| `SourceCollectionSnapshot::preflight(available_bytes, cancelled)` | Worker-only file resolution, hashing, uncached audio comparison, destination checks and totals. Return all source diagnostics without creating/reserving files. |
| `SourceCollectionSnapshot::is_current(live, history, revision)` | Check the caller's edit/job revision, complete project metadata, save folders, history-root order/presence, saved comparison metadata and original PCM pointer/rate/metadata identity. Ignore live playhead movement. No file I/O; file revisions need separate executor checks. |
| `CollectionPreflight::diagnostics()` | Sorted typed issues with paths and every affected owner index. Owner records retain scope, asset ID/name, original source, original folder and prepared audio. |
| `CollectionPreflight::verified_sources()` | Only source groups whose owners and destinations passed. A blocked report can contain verified groups; they do not authorize partial collection. |
| `CollectionPreflight::into_plan()` | Produce `SourceCollectionPlan` only when no diagnostics remain. Plan fields are private; accessors return immutable views. |
| `SourceCollectionPlan` | Retain the captured owners, project/folder/revision, manifest content receipt, verified source receipts, proposed targets and totals for Stage 2. The staging-directory accessor returns a proposal, not a reservation. |

The host supplies its `u64` edit/job revision. Capture once for a job, retain the snapshot through preflight, and use `is_current` before execution/publication. Folder values come from the normal absolute-folder Session save/open lifecycle. Perform capture, stale checks and preflight outside painting and the audio callback. Stage 4 adds desktop dispatch; the later [CLI frontend](source-bundling-cli.md) reuses this API without desktop history.

## Inventory roots

| Root | Media treatment |
| --- | --- |
| Live current session | Include every non-MIDI asset, including unused assets and Sampler/Granulizer inputs with no timeline clips. |
| History current | Retain its own source references and buffers even when it differs from live state. |
| Pending group | Capture without finishing the group or clearing Redo. |
| Undo and Redo | Retain every restorable session. Scope indices follow bounded history storage order; they are stable only for the captured job. |
| Saved comparison model | Retain metadata for later retargeting/checks. It owns no PCM and does not require copies of otherwise unreachable files. |
| Embedded MIDI | Count references but do not resolve or copy files. Patterns remain in the manifest. |

Resolve each reference through its owning session and original folder. History already normalizes external/generated paths to absolute paths; using the live folder as their original base would break Save As and history-only assets. A snapshot retains temporary-stem guards, so releasing optional history during a pending worker cannot delete its source files. Final owners retire with the snapshot/plan outside the callback.

The [clip clipboard](../../crates/ui/src/lib.rs) contains project/track IDs and `Clip` values, including asset IDs. It has no independent file or PCM owner. [Component drag state](../../crates/ui/src/instrument_component_drag.rs) retains instrument settings and Sampler asset IDs, not an independent source path/buffer. Current asset-table coverage supplies their media; Stage 4 must require settled gestures. No UI-specific source traversal or duplicate clipboard source cache is added.

## Verification and conflicts

1. Require a saved project folder. Read/validate its manifest without preparing media, check its project ID and retain its SHA-256 content revision. Unsaved projects, unreadable/invalid manifests, foreign project/history state and manifest mutation while loading are operation errors.
2. Resolve every audio owner and group readable regular files by canonical path. Symlink aliases for external inputs resolve to one source. Different canonical paths remain distinct even when their bytes match; hardlink/content deduplication across paths is not implemented.
3. For every distinct original PCM owner/rate/metadata, use `daw-media::check_prepared_source`. Its typed `PreparedSourceCheck` returns a content revision and audio-match result. The existing `verify_prepared_source` error-returning API remains compatible. Never parse error strings to decide whether audio matches.
4. Prepared audio must use its owning project's rate. Compatible history rates verify independently against the same original encoded bytes. Shared PCM/rate/metadata references reuse one check within the source group while retaining all owner identities.
5. Missing/unreadable/non-regular files, missing prepared audio, decode/resource failures and mismatched audio produce per-owner diagnostics. If one path satisfies some retained audio versions but not others, report a revision conflict with all owners. A content-revision change between checks also blocks the group. Continue inspecting other sources.
6. Choose a canonical extension from the verified codec/container through `daw-media::source_file_extension`. WAV remains `wav` despite a misleading original suffix; MP3 and ADTS AAC use `mp3`/`aac`; ISO MP4 uses `mp4`, including AAC `.m4a` input. Asset display names and original bytes stay unchanged. Recheck the source digest after planning its targets.

Diagnostics are ordered by path, owner indices and message, then deduplicated. Missing and incompatible sources do not become silently omitted copies. Cancellation or counter overflow returns an operation error without a plan. Uncached verification keeps Stage 0's decode memory admission; resource failure retains all active buffers and state.

Receipts identify observed bytes; they do not lock external files or certify a simultaneous filesystem snapshot. Stages 2/3 must recheck file/manifest revisions, containment and live ownership, and verify private staged bytes against prepared audio before commit.

Plan readiness does not certify the manifest commit path or write permissions. Stage 2 must also require an ordinary manifest target (or a safely absent new target), reject manifest symlinks/reparse points, and apply the native atomic-replacement contract before writing.

## Destination proposals and reuse

UUID v4 proposal IDs are assigned once during snapshot capture. Repeating preflight on that same snapshot, unchanged files and the same capacity estimate produces identical sources, paths, diagnostics and totals. A fresh job snapshot has fresh proposal IDs.

- Imported media proposes `assets/sources/<UUID>.<canonical extension>`. Repeated references share one target. An unmarked external file merely located under `assets/` stays external and needs a collected copy unless the same source already has a verified included owner.
- Verified included recordings inside the current project retain their existing ordinary path. Generated stems already inside the current project's `stems/` retain their location.
- Generated sources outside that tree propose `stems/<group UUID>/<original filename>`. Siblings share a proposed group directory. Case-colliding sibling names use separate UUID directories while retaining filenames and asset names.
- A verified source used by imported and generated assets can require two final targets: imported ownership requires `assets/sources/`, while generated ownership retains `stems/`. Keep one source/receipt with separate compatible target-owner lists. Deduplicate within each target class. Byte totals count both new output files; Stage 2 can stage the original source once before materializing its targets.
- Check every existing component below the canonical project root for ordinary file/directory types, symlinks and Windows reparse attributes. Reject internal aliases as well as external escapes in included/output paths. Do not rewrite/remove user paths.
- Cache directory listings for case-collision checks. Screen both existing and proposed paths for case collisions, independent of source basenames. These checks do not replace native Windows filesystem acceptance.
- Reject occupied final or `assets/.collect-<operation UUID>` proposals. Report the path; do not overwrite, reserve or reuse it. A new preflight job can propose fresh UUIDs. Stage 2 must still reserve exclusively, handle races/retries and never treat these checks as reservations.

The storage-class target distinction refines the initial same-source deduplication rule without changing the Stage 0 storage model or normalizing existing stem layouts.

## Totals and capacity

`CollectionSummary` uses checked `u64` arithmetic for reference/file counts and encoded-byte totals:

- Audio and embedded reference counts cover all captured roots, including repeated history references.
- Source-file, duplicate-reference, copy, already-included and additional-byte totals cover verified groups. They are partial when a report has diagnostics; `is_ready` must accompany their presentation.
- History-only source count means no live Current owner uses that source. History-only copy bytes count new targets needed only by retained roots. Unused live assets still count as current sources.
- `additional_bytes` counts final new encoded files, including separate targets required by different storage classes. It excludes decoded audio, caches, staging/manifest overhead and filesystem allocation overhead. Stage 2 must account for its execution strategy's peak space needs.
- `available_bytes` is an optional caller-provided filesystem estimate. `None` means not measured. `space_appears_sufficient` compares only final media bytes and is advisory; an insufficient estimate does not make a verified plan unsafe or reserve capacity. Actual writes can still fail.

Stage 1 adds no native capacity-query dependency and claims no available-space measurement by default. Future worker adapters can supply an estimate; UI painting must not query the filesystem.

## Acceptance and remaining stages

Automated coverage includes repeated read-only preflight; unused, sampler-only, granular and embedded MIDI assets; duplicate references/basenames and identical bytes at unrelated paths; current/pending/Undo/Redo ownership; saved-only metadata; old-folder resolution; temporary stem lifetime; compatible processing rates; stale revisions/model/history/audio owners; missing, unreadable, non-regular, unprepared and mismatched sources; source mutation/cancellation; included/stem reuse; storage-class targets; occupied/case-colliding destinations; symlinks; checked totals; and MP3/AAC/MP4 container verification.

See [validation history](../validation/README.md) for executed checks. Stage 2 supplies copy/rollback fault injection. Stage 3 adds local portable-reopen tests; native Windows/Linux execution, Windows reparse behavior and release portability acceptance remain pending. This stage does not create files, alter manifests, mark the project dirty, clear history, reload live audio or establish project portability.
