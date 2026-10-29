# CLI project source collection

Status: implemented as a frontend over the shared Stage 1–3 backend. This extends the original desktop scope; it does not close the [Stage 5 native acceptance gates](source-bundling-stage5.md).

## Command and ownership

```text
daw-cli collect-sources [--project <folder>] [--dry-run]
```

The project defaults to `.`. No positional arguments or confirmation prompt are accepted. [`apps/cli/src/collect_sources.rs`](../../apps/cli/src/collect_sources.rs) owns command reporting only. Load through `Session::open`, capture `source_collection_snapshot(None, 0)`, run read-only preflight, then execute its sealed plan only if every diagnostic is clear. Reuse source verification, target reservations, copying, receipts, commit checks and cleanup without duplicate file/manifest logic. No new dependency or device/window initialization is added.

Run blocking work on the headless command thread. The loaded Session is immutable during execution; use `SourceCollectionSnapshot::is_current` as its guard. Backend filesystem and manifest revision checks remain required. The process exits after disk publication, so it needs no live-session receipt application or renderer replacement.

Inventory includes all saved audio assets, including unused assets and sample inputs. Embedded MIDI needs no file copy. This process has no desktop saved/pending/Undo/Redo roots. Require users to save and close the desktop project first; no cross-process editor lock or synchronization is provided. Opening prepares the sources currently on disk; this command cannot establish the identity of older imported bytes that are no longer available.

## Reporting and failure contract

- Print escaped project/folder names, files to copy, already included files and additional encoded bytes before execution. State that staging needs extra space and free space was not measured.
- Print open warnings and all available preflight diagnostics to standard error, including paths and affected asset names/IDs. A blocked report cannot authorize a partial copy.
- `--dry-run` stops after preflight with no writes. Return 0 only when ready; return 1 on blocked or failed verification.
- For collection, return 0 only after `Committed`, including no-copy saves or commits with warnings. Print actual copied files/bytes and saved confirmation. Never print success for `RecoveryRequired`; return 1 and identify the candidate manifest while instructing retention of media.
- Return 1 for project/media/processing/write failures and 2 for invalid arguments. Validation/render missing-source warning behavior stays unchanged.
- A precommit output failure other than BrokenPipe aborts. A result-output failure after confirmed commit produces a best-effort warning and preserves exit 0. Ignore BrokenPipe as in track inspection.
- No signal handler or graceful Ctrl+C cancellation is added. Abrupt process exit follows the shared commit/orphan contract: retain original media, never delete/reuse unreferenced crash files automatically, and never roll back a confirmed commit.

User instructions: [CLI collection guide](../../docs/cli/collect-sources.md).

## Acceptance

[`apps/cli/tests/collect_sources.rs`](../../apps/cli/tests/collect_sources.rs) runs the real executable without display variables. It checks read-only preflight, default/explicit folders, complete copied bytes, unchanged originals, moved Unicode folders with originals offline, identical exported WAV bytes, zero-copy retries, atomic blocking with a good and missing source, help, invalid arguments, invalid manifests and absent folders. Parser tests cover the command shape. [`tools/check_cli.py`](../../tools/check_cli.py) repeats portability, read-only and failure checks with private copies of the tracked generated demo in the existing native CI smoke step.

Shared backend fault, uncertain-commit and interruption tests remain the evidence for publication failures. CLI tests do not claim native UI/device interaction or execution on other platforms. See [validation history](../validation/README.md) for executed checks.
