# Autosave and recovery

An interrupted [audio or mixed take](../editing/audio-recording.md#save-and-recover) can appear in the recovery dialog. The recovered clip ends at the last verified audio frame.

## Automatic snapshots

The desktop app creates private recovery snapshots of modified projects about
every 30 seconds. It retains three ordinary generations per project and instance.
This does not update the manually saved project or clear its unsaved indicator.
Use Save for a permanent project checkpoint.

Recovery uses bounded metadata and journal storage. Empty workspaces use no
recovery slots.

## Recover or discard a project

After an interrupted session, the app offers Recover, Open last saved when a
saved location exists, or Discard recovery. Recovery opens a separate working
copy. Save asks for a permanent destination. The original saved project remains
unchanged. Recovery can restore unnamed projects. It skips corrupt generations
and offers the latest valid one. Another running instance's data stays private.

Project-owned sources stay with recovery copies. External recordings remain
external; restore missing files before retrying. Recovery cannot replace a deleted
external recording. Discard removes only recognized private recovery data. It does
not delete manually saved projects or source recordings.

## Recover a MIDI take

MIDI recording protects its base project before capture starts. It writes the
accepted events to a journal and checkpoints the complete take before publishing
it in the editor. Recovery of an unpublished take restores one Undo step for the
whole take. Ordinary recovered snapshots start a new history session.

The app refuses a new take
when it cannot reserve recovery capacity. Disk and permission failures appear as
errors. Retained interrupted takes remain protected until publication or explicit
discard. Recovery is a safeguard; keep separate backups of important projects.
