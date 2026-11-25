# Autosave and crash recovery

## Snapshots and ownership

The desktop storage worker captures modified committed sessions every 30 seconds,
with project/revision identity. Snapshot capture retains immutable source owners
and Undo/Redo sources; serialization and media copying run on the worker. Manual
Save and its clean revision remain independent.

Each app instance owns a UUID directory under the platform's private DAW recovery
root and holds an exclusive OS file lease. Startup scans only marked UUID owner
directories and ignores live leases. Hash-checked versioned generations contain
complete manifests, metadata and project-owned media. External references remain
external. Keep three ordinary generations per project; pin pending take bases.
Incomplete generations do not replace prior valid data. Corruption falls back to
the newest valid generation. Ordering is independent of project revision resets.

## Storage admission

Limit each metadata/manifest read to 512 MiB, each take journal to 256 MiB and each
journal record to 4096 bytes. Admission bounds private metadata/journals to
512 MiB and 20 instance workspaces; reserve a conservative take allowance before
capture. Copied source media has no byte quota and can fail with a disk error.
Per-instance leases protect ownership; simultaneous admission across instances
is conservative rather than a global transactional quota.

## Recording journal

Before recording, publish the durable base, create and sync the checksummed
journal, then publish its active header. Journal Start, canonical events and End;
sync at most every 250 ms during capture. A crash can lose the unsynced tail.
Recover only the contiguous valid record prefix. A whole-take checkpoint contains
an applied-take receipt and base generation. Repeated scans/retries use that
receipt and do not duplicate clips. Acknowledgment removes the active pin only
after editor publication. Failed publication leaves a retryable result.

## Recovery and disposal

Recover to a separate marked working copy. Restore a before/after history pair
only for an unpublished take; ordinary snapshots are baselines. Keep source leases
until the working session releases them. Save from a private working copy uses
Save As. Recovery never overwrites the original manual manifest.

Discard writes durable project tombstones and removes only recognized owned data
under an acquired lease. Clean exit cleans acknowledged ordinary recovery data.
Pending takes survive clean shutdown until acknowledged or discarded. Reject
symlink/reparse leaves in owned metadata paths; leave unknown directories untouched.

## Native verification

Required native gates include kill points around sync/publication, disk full,
permissions, missing sources, multi-instance races and platform lease behavior.
Record completed automated and native evidence separately in validation history.
