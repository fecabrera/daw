# Project source bundling — Stage 0 contract

Date: 2026-10-29.

Status: storage and verification foundations implemented. Collection, inventory, media copying and desktop controls remain pending in the [staged plan](source-bundling.md).

## Storage model and compatibility

[`SourceStorage`](../../crates/core/src/source_storage.rs) describes ownership independently of `Source.kind` (provenance) and `Source.path_kind` (path representation). `Source.storage` is optional and omitted when absent. The wire values are `external`, `project` and `embedded`.

| Provenance | Omitted storage | Allowed explicit storage | Path contract |
| --- | --- | --- | --- |
| `external` | External reference | `external` or `project` | External references retain absolute/relative resolution, including `..`. Project-owned recordings require `relative` and `assets/sources/<portable filename>`. |
| `generated` | Project-owned | `project` | Preserve existing temporary absolute paths and saved `stems/` paths. Collection must retain generated provenance and lifecycle. |
| `midi_pattern` | Embedded | `embedded` | Retain the existing empty path, `embedded` path kind and pattern validation. |

Deserialization rejects unknown storage values. `Project::validate` rejects conflicting ownership/provenance combinations and invalid included recording paths. Paths inside a project do not imply ownership: an unmarked external source under `assets/` remains an external reference.

This is an additive field in the experimental manifest. Do not change `schema_version`: loaders currently ignore it, so incrementing it cannot enforce compatibility. Omitted or null storage retains legacy inference; serialization omits it rather than rewriting legacy projects. There is no bulk path migration and no change to IDs, frame coordinates or prepared audio. Explicit fields survive round trips.

Older clients ignore this field, can drop it when saving, and treat a collected imported recording as an external reference. They cannot preserve its ownership or guarantee a portable Save As. Editing an included-source project with an older client is unsupported. A future enforced manifest version must introduce an actual loader gate; a version number alone is insufficient.

The foundation can deserialize, validate, inspect and resolve included-source fixtures. No normal import or menu action creates included recordings yet. Session open/save rejects these fixtures with an integration error before media preparation or save output. Keep this gate until Stage 3 can relocate all retained owners: guarding only the current asset table would miss included media restored by Undo/Redo after Save As. Existing external/generated session I/O and history behavior remain unchanged.

## Included-path containment

`daw-core` checks portable syntax without filesystem I/O. `daw-project` checks containment at source resolution:

1. Require the exact `assets/sources/` prefix and one filename. Reject nested paths, `..`, absolute/drive/UNC paths and backslashes.
2. Limit the filename to 255 UTF-8 bytes. Reject control characters, Windows-forbidden characters, leading/trailing spaces, trailing periods and reserved device names, including superscript COM/LPT suffixes. Unicode filenames remain valid.
3. Resolve the project folder and existing ancestors with filesystem canonicalization. The approved source directory must resolve inside the project; a source must resolve inside that directory. Reject external and dangling symlinks. An internal directory alias is allowed only while both containment checks pass.
4. Missing ordinary files or directories retain their safe lexical destination. Path inspection does not create files. Stage 3 session integration must retain the existing missing-source warning/silence behavior; Stage 0 keeps included fixtures gated from session open.

These checks detect static path escapes. They do not lock directories against another process replacing files or symlinks. Stages 1/2 must repeat containment and revision checks before reading and publication, reserve operation-owned destinations and validate staged bytes. Do not treat path validation as a filesystem transaction or an adversarial file-mutation sandbox.

Generated stems retain their existing resolver in this stage. Their inventory/Save As containment and copying checks belong to the common managed-source integration, not a reinterpretation of old stem paths as collected recordings.

## Source revisions and prepared audio

[`SourceRevision`](../../crates/media/src/source_revision.rs) is a runtime value with:

- `byte_length: u64`, counted with checked arithmetic.
- `sha256: [u8; 32]`, over the complete original encoded file, including headers and tags.

The receipt has no path, asset ID, project rate or cached PCM. Moving a file does not change its revision. Different bytes with the same length and mtime have different revisions. SHA-256 is supplied by the already locked `sha2` 0.10.9 dependency; the shared version declaration does not upgrade it. Media [notices](../../crates/media/assets/licenses/SHA256-NOTICE.txt) are copied by the existing native package scripts.

`SourceRevision::read` streams regular-file contents with a 64 KiB buffer, checks cancellation between reads and checks length/mtime before and after reading, including the current path. A digest identifies the bytes actually read. Metadata alone is never a trusted content receipt.

### Import/open and legacy policy

Normal import/open retains the current weak path/rate/length/mtime decode cache. Stage 0 does not add hashing to normal imports, persist receipts, or change `AudioData`/callback ownership. Existing prepared sessions therefore require worker revalidation before collection.

`verify_prepared_source(path, prepared, cancelled)` reads a content revision, bypasses the cache for a fresh decode at the prepared project rate, compares source metadata and every stereo sample bit for bit, then rechecks the content revision. Check cancellation between comparison blocks. Compare original project-rate audio, not a stretched or transposed derivative; peaks do not establish source identity. Use the existing decode memory admission budget. Resource exhaustion fails without replacing or releasing the active session.

This fallback proves audio equivalence to a retained buffer; without a prior trusted receipt it cannot prove which encoded bytes were present at the original import. Do not claim otherwise. If the current original file produces the same metadata and PCM, the full current encoded file may be collected. A known prior content receipt, once introduced, must also match. A mismatch, unavailable buffer or missing source is a preflight failure, not permission to encode PCM as a new original or silently reload the active sound.

For final collection, verify the private staged file against its expected revision **and** prepared audio before publication. Rechecking an externally mutable source before/after decoding alone cannot certify that an intervening decode saw the same bytes. Retain immutable prepared owners through the operation. A future optimization may bind a trusted digest to a frozen file decoded during import/open; it must establish that binding before skipping revalidation.

### Inventory identity and conflicts

Stage 1 source records must retain the project/job revision, owning snapshot/folder, asset IDs, resolved path, content revision and prepared-audio owner. Group repeated references only when resolved file identity and verified revision agree. Do not deduplicate unrelated paths solely from matching digests in the first version.

The saved dirty-comparison `Project` is metadata, not a restorable audio owner. Account for its references during metadata retargeting, but do not require copying an otherwise unreachable source solely because it remains in that comparison model. Current assets, including unused assets, and restorable current/pending/Undo/Redo sessions retain their full collection requirements.

Two retained owners that require different revisions at one current external path are a conflict unless each required original is independently available and verified. Report all affected owners. Never retarget older history to newer bytes merely because the path or asset ID matches. History-only sources participate in the same checks; history remains unsaved session state.

## Frozen copying and transaction rules

The following rules specify Stage 2. They are not an implemented copying API in Stage 0.

### Naming, reservation and retry

- Stage under `assets/.collect-<operation UUID>/` on the destination filesystem. Reserve the directory exclusively; do not reuse an existing directory.
- Require ordinary directories/files below the canonical project root for staging and final collected-media paths. Preflight must reject destination or included-media symlinks/reparse points, including internal aliases: an absolute symlink into the current folder can break when that folder moves. Do not rewrite or delete user symlinks. The resolver's containment check alone does not certify portability.
- Final imported media names use `assets/sources/<source UUID>.<extension>`, with a new UUID v4 and a lowercase supported extension (`wav`, `mp3`, `aac`, `m4a`, `mp4`). Derive the extension from the accepted source/container; never use arbitrary source basenames as destination paths. Keep original asset display names.
- Reserve final files exclusively with create-new/no-replace semantics. An existing filename is a collision; generate a new UUID and retry. Never use overwrite-capable rename as the reservation mechanism. Existing collected files may be reused only through a validated source record and matching digest.
- A retry creates a new operation directory and revalidates the immutable plan. Keep no automatic orphan-reuse or garbage-collection rule. Crash leftovers are unreferenced files, not evidence that collection committed.
- Copy regular-file bytes, never symlinks or external hardlinks. Keep transfer buffers at 64 KiB, checked `u64` byte/file totals, and cancellation checks before/after each read/write chunk, between files, before final verification and immediately before commit. Reject counter overflow. Disk-space estimates are advisory; actual writes must handle exhaustion.

### Publication and cleanup

| Phase | Required behavior on failure/cancellation |
| --- | --- |
| Read-only preflight | No files, manifest, session or history change. |
| Staging and verification | Remove only this operation's staged files; retain originals and existing project media. |
| Final media preparation | Complete, verify, flush and sync exclusively reserved final files before a manifest can reference them. Until manifest commit, cleanup may remove only files proven to belong to this operation. |
| Manifest commit | Write/flush/sync a uniquely reserved temporary manifest, then atomically replace `project.json` using the shared platform replacement path. Successful replacement is the commit point. |
| After commit | The manifest and new media are authoritative. Apply the relocation receipt even if cancellation arrives now. Do not report an ordinary cancellation/rollback or delete referenced media. Report later notification/durability failures as committed with a warning. |

Retain a cleanup guard that is disarmed immediately after successful replacement, before any fallible notification. Filesystem commit and live/history publication must use a prevalidated relocation receipt so UI publication cannot become an independent partial transaction. No edit or conflicting job may invalidate that receipt during the collection commit.

On Unix, sync containing directories after creating/publishing media and after manifest replacement when supported; failures before commit abort, while failures after replacement report a committed result with uncertain crash durability. The current Windows `replace_file` helper renames the old manifest to a backup before renaming the new one into place. That leaves a crash window with no `project.json` and does **not** meet the collection contract. Stage 2 must replace this shared path with native atomic replacement for existing manifests and no-replace publication where appropriate, with native failure tests. Retain file flushing and record remaining Windows directory/power-loss durability limits in acceptance.

No platform may claim multi-file atomicity or unconditional power-loss safety. A crash before commit leaves the previous manifest valid; a crash after commit must never trigger rollback cleanup on reopen. Stage 0 freezes this requirement; it does not change the current save replacement helper.

## Fixtures and later acceptance

Implemented checks cover:

- Legacy external/generated/MIDI ownership and unchanged omission of the storage field.
- Explicit included storage round trips, invalid ownership, unknown wire values and portable path rejection.
- Included manifest validation, metadata round trips/inspection, path resolution and session open/save gates that preserve existing files.
- Contained/missing paths plus external directory, file and dangling symlinks on Unix.
- SHA-256 known bytes, relocation-independent revisions, same-length/same-mtime changed audio, cache bypass, project-rate comparison, metadata mismatch, cancellation, concurrent mutation and missing/non-regular sources.

Stage 2 must add deterministic fault injection for read/write/flush/sync failures, occupied destinations, disk exhaustion, cancellation at each commit boundary, manifest replacement failure and post-commit notification failure. Stage 3 adds collected Save As and complete history-owner relocation. Native Windows reparse-point/rename checks and platform durability acceptance remain open. Stage 0 does not establish project portability.

Completed development checks and remaining native limits are recorded in [validation history](../validation/README.md).
