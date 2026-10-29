# Project source bundling — Stage 4 desktop command

Date: 2026-10-29.

Status: desktop action, preflight confirmation, bounded progress, cancellation and receipt publication implemented. [Stage 5](source-bundling-stage5.md) still owns full native portability and release acceptance.

## Command and guards

`FileAction::CollectSources` supplies the shared **Collect project sources…** label and dispatch for the application File menu and native macOS File menu. It has no keyboard shortcut. Existing File shortcuts remain unchanged; optional shortcut metadata also feeds native key equivalents and menu labels. Both menus reuse the shared disabled-action explanation for unsaved projects.

[`collection.rs`](../../crates/ui/src/collection.rs) owns the desktop workflow. Require a saved Session folder, an idle file-job path, a focused/enabled application, no text edit, settled track/clip/layer/unit gestures and no conflicting dialog/model validation. Unsaved projects show a disabled-item explanation. Confirming a review cannot overwrite another job receiver or bypass an active error/dialog. File actions, project editing, view actions and window-close requests stay blocked through review and the final report. Existing prepared playback can continue.

Before capture, release optional history under the existing memory-pressure policy and initialize history. Capture the complete immutable source snapshot once with project/job and navigation revisions. The shared prepared-job dispatcher starts the worker without evicting history after capture. Discovery, hashing, decoding, copying and manifest I/O remain in `daw-project` workers; frame handling only compares metadata, reads progress and paints dialogs.

## Review and progress

Preflight uses the Stage 1 API with cancellation and no asserted free-space measurement. The shared dialog shows the project folder, files to copy, files already included, additional encoded bytes and history-only sources. It states that collection also saves current project state, keeps originals unchanged and needs additional temporary space. No-copy plans explain that no new files are required; confirmation still saves the project.

Blocked reports expose each source path, diagnostic and affected asset names, with no collection action. Missing or conflicting sources cannot become a partial collection. Close or Escape discards a review without modifying files or project state.

After **Collect and save**, execute the sealed plan on a worker. A bounded single-slot progress channel uses nonblocking `try_send`; intermediate updates can be dropped. The report displays discovery/copy/verification/publication phases, file and byte counts, cancellation and warnings through the existing `job_report::Report`. It reaches 100% only after confirmed commit and successful active-state publication. Generic status-bar Cancel retains the collection receiver until the worker acknowledges cancellation, so a late commit result cannot be discarded.

## Lifecycle and completion

Each operation has a unique ticket and an independent atomic lease. Frame polling invalidates the lease on project/model/history, edit revision, explicit navigation revision or view changes. The worker queries that lease before copying and publication. Status destruction cancels and invalidates its own worker; tickets prevent another status from accepting its result. Playback-only playhead updates do not invalidate the snapshot.

| Result | Desktop behavior |
| --- | --- |
| Cancel/error before commit | Retain live project, prepared audio, dirty state and old manifest; let the backend clean only operation-owned files. |
| Confirmed commit | Call Stage 3 `CollectionOutcome::apply`, mark the captured navigation checkpoint saved and derive dirty state from history/navigation. Do not replace the Session or request renderer synchronization. |
| Confirmed commit with late cancellation/notification/durability warnings | Apply the receipt and show completion with warnings. Do not report ordinary cancellation or roll back media. |
| Confirmed disk commit after lifecycle invalidation | Retain active state and show recovery guidance to reopen the saved destination. Never publish into a different project. |
| Unconfirmed manifest commit | Retain media/candidate files, show a distinct recovery failure and include the candidate path. Do not apply the receipt or claim portability. |

Selection and pane focus remain UI state. Freeze installed-unit/catalog focus clearing while the collection modal owns the same project/view. After dismissal, restore its prior pane through the shared scheduled-focus helper. Track, clip, layer and installed-unit identities, prepared buffers, transport and pending/Undo/Redo order remain intact. Native menu or view lifecycle invalidation does not restore old selection into another project/view.

## Validation and limits

[`collection/tests.rs`](../../crates/ui/src/collection/tests.rs) covers saved/unsaved readiness; focus/text/gesture/job/dialog guards; application-menu input dispatch; summary/diagnostics; cancellation in discovery, review and worker completion; no-copy collection; actual collection/reopen; byte/file progress; clean/dirty state; held receiver cancellation; late commit warnings/cancellation; stale project/view/navigation; foreign committed outcomes; window-close guards; and effect/component selection and refocus. Existing UI suites must continue to pass. Shared backend fault/recovery tests remain the filesystem evidence for unconfirmed commit handling.

See [validation history](../validation/README.md) for executed results. Automated egui input and macOS compilation do not establish native menu interaction, device continuity, large-copy responsiveness, native Windows/Linux paths, reparse points, network filesystems or crash/power-loss durability. Those checks remain explicit Stage 5 gates; no archive/CLI collection, automatic import collection, relinking or garbage collection is added.
