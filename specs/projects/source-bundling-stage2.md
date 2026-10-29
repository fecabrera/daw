# Project source bundling — Stage 2 collection backend

Date: 2026-10-29.

Status: worker execution, relocation receipts and transactional publication implemented. [Stage 3](source-bundling-stage3.md) supplies session/history relocation and enables included-source session I/O. [Stage 4](source-bundling-stage4.md) now exposes the desktop command. This backend alone does not establish project portability.

## Worker API and ownership

[`SourceCollectionPlan::execute`](../../crates/project/src/source_collection/execute.rs) consumes a verified plan through an immutable borrow. It accepts a current-state guard, a cancellation query and a fallible progress callback. Run it on a worker while the host excludes project edits and competing file jobs. The guard must check the captured project/job revision and ownership; `SourceCollectionSnapshot::is_current` supplies the full session/history comparison. Playback can continue from prepared audio.

The executor checks the manifest content revision, source revisions and original path resolution before copying and before publication. It never edits the live Session or history. The returned `CollectionOutcome` retains the snapshot, prepared buffers and temporary-stem guards. Stage 3 must apply a committed receipt before releasing those owners.

`RelocationReceipt` records the project ID/revision/folder and each owner's scope, asset ID, original source/folder, verified content revision and final source. The candidate manifest changes only current assets' source locations/storage. IDs, names, frame mappings, settings and captured playhead stay unchanged. History-only sources receive protected files and receipt entries without being added to the current manifest. Imported recordings become explicitly project-owned; generated stems retain provenance and their separate `stems/` layout.

## Copy and reservation sequence

1. Reserve a private `assets/.collect-<UUID>` directory exclusively. Retry occupied proposals with fresh UUIDs. Unix staging directories use mode 0700.
2. Stream each unique encoded source into private staging with a 64 KiB buffer. Check cancellation between chunks, exact length, source metadata stability and SHA-256. Flush and sync completed files.
3. Decode the private staged file without the weak decode cache. Compare it against every distinct prepared PCM/rate/metadata owner. Stage already-included sources too: this binds verified private bytes to prepared audio before reuse.
4. Reserve final files with `create_new`. Recheck ordinary components, case collisions and the project filesystem. Retry occupied imported names or generated group directories without replacing existing entries. Protect existing manifest source paths even when their files are missing.
5. Copy staged bytes into independent final regular files, then flush, sync and verify their revisions. Do not create hardlinks or symlinks. Compatible repeated references share one final file; imported/generated storage classes can require separate targets from one staged source. Remove each private proof through its pinned handle after all its targets pass verification, before staging the next source.
6. Build and validate the candidate project and complete receipt before publication. Write, flush and sync a uniquely reserved temporary manifest beside `project.json`.
7. Sync directories where supported. Recheck original sources, final files, candidate content, file identities, manifest revision and host guard. Replace the manifest only after all media is complete.

Progress reports staging bytes separately from final-copy bytes and file totals. The conservative peak-media bound includes all unique staged sources plus new final copies; actual staging retains only the current source proof. Manifest bytes, filesystem allocation overhead and decoding memory are additional costs; preflight free-space estimates remain advisory. Actual write failures abort safely before commit.

## Commit, cleanup and recovery

Successful manifest replacement is the commit point. Before it, an error or cancellation retains the old manifest and returns an error after cleanup. Cleanup tracks filesystem identities and removes only entries created by this operation. It never recursively deletes directories or deletes original/pre-existing media. Changed entries, unrelated files and nonempty directories remain with warnings. Root replacement prevents cleanup against the replacement tree.

The shared store retains at most 32 entry handles plus its pinned canonical root; temporary source/manifest files and the private staging directory stay pinned while active. Completed destination files and ordinary created directories enter bounded eviction queues. Close older pins before further reservations exceed the budget, while retaining identity metadata for publication checks. A released file/directory ID can be recycled, so rollback must not delete a released reservation merely because reopening reports the same ID. Retain it with a cleanup warning. Still-pinned entries retain ordinary identity-checked rollback; never recurse or delete a reused private pathname. Successful commits preserve final media and remove private staging without warnings about released final pins.

After commit, rollback is disabled before notifications and other fallible work. Late cancellation, cleanup failure, directory-sync failure or progress delivery failure returns a committed outcome with warnings; referenced media remains.

If replacement reports an error, the executor checks the ordinary manifest target's content. Confirmed candidate bytes produce a committed result; confirmed old bytes produce a pre-commit error. An unconfirmed result returns `RecoveryRequired`, retaining final media, staging and the candidate manifest path. Its receipt/candidate must not be applied as confirmed state. No automatic deletion, orphan reuse, recovery overwrite or garbage collection is added. A later attempt reserves fresh names.

A process interruption can leave complete, unreferenced files. This is preferable to deleting media referenced by an uncertain commit. Content checks and identity checks detect ordinary concurrent changes; they do not lock all external files or make the project folder a security sandbox. The caller must serialize application writes.

## Native manifest replacement and durability

[`file_publish`](../../crates/project/src/file_publish.rs) is shared by collection, ordinary Save and export. It requires distinct ordinary files with the same canonical parent. Unix uses rename. Windows uses `SetFileInformationByHandle(FileRenameInfoEx)` with replacement and POSIX semantics. Existing delete-sharing readers retain the old data; new opens see the replacement. A target handle requires delete sharing before replacement, so a nonsharing reader still blocks publication. See the [Microsoft rename flags](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information). Only an unsupported information class or filesystem permits fallback to `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`, without cross-volume copying. Permission and sharing failures return errors. The old manifest is never moved aside first. Neither API establishes universal filesystem or power-loss guarantees.

`ReplaceFileW` is not used: its documented failure states can change the original names. See [Microsoft replacement failure behavior](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew). Native Windows tests cover successful replacement and a sharing violation that retains both files. They require Windows CI execution; macOS results do not validate them.

Files are synced before commit. Unix containing-directory sync is attempted before/after commit; unsupported directory sync produces a durability warning. Windows file sync and native replacement do not supply a directory-flush guarantee; the outcome reports that limit. Network filesystems, power interruption, process termination and native reparse-point behavior remain acceptance work. Never claim crash durability from the injected-failure tests alone.

## Automated acceptance

[Execution tests](../../crates/project/src/source_collection/execute/tests.rs) cover byte-identical WAV/MP3/AAC/MP4 copies, complete originals despite trimmed clips, shared references, history-only owners, generated groups and independent imported/generated copies. They also cover stale jobs/manifests, source mutation, occupied reservations, already-included reuse, arithmetic overflow, cancellation across phases, read/write/flush/sync failures, disk exhaustion, rollback identity checks, unrelated staging contents, confirmed commit after a reported replacement error, uncertain commit recovery and post-commit failures.

[Publication tests](../../crates/project/src/file_publish.rs) check replacement without backup artifacts, an open old-file reader, a pinned candidate with Unicode names, first publication, invalid destinations, Unix symlink rejection and Windows sharing failure. Results and platform limits are recorded in [validation history](../validation/README.md). Stage 3 adds local reopen, Save As, history relocation and render-equivalence tests with originals unavailable; native release acceptance remains separate.
