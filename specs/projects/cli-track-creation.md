# CLI track creation

`daw-cli tracks --add NAME --type audio|midi|beat [--project FOLDER]` appends
an empty root-level track to an existing saved project. The folder defaults to
`.`. Require both options together and reject a positional inspection name.
Reject blank names and control characters at argument parsing. Duplicate names
are allowed, consistent with track inspection.

Keep argument and output handling in the CLI. Open the validated document
through Session::open_document and into_editor_session without audio preparation.
Use the shared core add_track/add_midi_track/add_beat_track constructors for
fresh IDs and standard settings, assign the requested name, validate and save
through Session::save. Reuse source ownership, atomic manifest publication and
storage failure handling; do not add a CLI-only manifest writer. Existing save
validation may reject unavailable project-owned sources. Do not create a missing
project. The resulting track has no clips or patterns. Existing session state
and sources follow the standard save path. No audio stream or window is opened.

Require users to save and close the desktop project first. There is no
cross-process synchronization or desktop Undo history. Print escaped name and
ID only after save succeeds. Postcommit stdout errors are warnings, not failed
creation, to avoid inviting duplicate retries. Ignore BrokenPipe. Return 0 for
saved creation, 1 for load/validation/storage failure and 2 for invalid arguments.
Keep tracks listing and exact-name inspection unchanged.

Acceptance covers all three types through the real executable, persisted
names/types/IDs/defaults, existing-track preservation, duplicate names, invalid
argument rollback and missing-project failure. See the [user guide](../../docs/cli/tracks.md).
