# Project source bundling — Stage 5 acceptance

Date: 2026-10-29.

Status: automated portability and process-interruption coverage added. **Native release acceptance remains open.** macOS checks below do not establish Windows/Linux or device/UI acceptance.

## Acceptance matrix

| ID | Requirement | Automated evidence | Remaining gate |
| --- | --- | --- | --- |
| SB-01 | Move the collected folder with originals unavailable | `moved_collected_project_reopens_mixed_sources_and_restored_history_without_originals`: actual directory rename, original input directory moved offline, full manifest equality, contained media, deterministic render and byte-identical WAV export. | Native Windows/Linux run. |
| SB-02 | Audio, Sampler, Granulizer, generated media and embedded MIDI survive relocation | Same mixed-source test renders each sample instrument in isolation, retains generated provenance and embedded notes, and restores a Redo-only source before moving/reopening. Stage 3 also covers sample inputs with no clips. | Native separation/model/device checks remain separate; fixtures use synthetic generated media. |
| SB-03 | Trims, loops, stretch and pitch retain mappings/output | `trimmed_repeated_stretched_and_pitched_clips_keep_prepared_audio_and_exports` now renames the saved folder before reopening. Both saved engines retain clip metadata, prepared buffer identity before close, deterministic renders and byte-identical export. | Native platform run; no new listening-quality claim. |
| SB-04 | Shared paths deduplicate; distinct Unicode/case basenames stay distinct | `incremental_collection_retains_existing_paths_and_distinguishes_case_and_unicode_basenames`: shared original path, distinct directories with case/Unicode basenames, stable asset IDs, distinct output paths, unchanged originals and a Unicode moved folder. Stage 1 covers destination case collisions and external aliases. | Windows/Linux filesystem execution. |
| SB-05 | Collect again after a new import copies only new media | Same incremental test performs initial collection, one-file incremental collection and a zero-copy collection, then moves/reopens with all original directories offline. Existing included paths remain unchanged. | Native UI interaction. |
| SB-06 | Save As protects current and history-only sources | Stage 3 `collection_and_save_as_protect_redo_only_media_and_keep_render_and_pcm`, including imported/generated variants, removes the old folder before Undo/Redo and reopening. | Windows/Linux run. |
| SB-07 | Failures/cancellation cannot publish incomplete media | Stage 2 deterministic real-I/O fault tests and Stage 4 egui job tests cover cancellation, read/write/flush/sync failure, disk exhaustion, mutation, reservation races, confirmed/unconfirmed commit and late notifications. | Native filesystem faults, network filesystems and device continuity. |
| SB-08 | Abrupt process exit respects manifest commit and orphan policy | `process_interruption_preserves_commit_boundary_and_retry_does_not_reuse_orphans` launches the test executable as a child. Exit before replacement retains the old manifest; retry creates fresh media without deleting/reusing the complete orphan/staging directory. Exit after replacement reopens/renders with the original offline. | Windows/Linux run; power interruption and machine crash remain unverified. |
| SB-09 | Path containment rejects symlinks/reparse aliases | Existing Unix symlink tests; new Windows-only `windows_junctions_cannot_redirect_included_reads_collection_or_save_as` rejects junction-backed open, preflight and Save/Save As while preserving target data and manifest. Native replacement/sharing tests remain in `file_publish`. | Windows junction/replacement execution; other reparse kinds and nonlocal filesystems. |

Project integration cases live in [`source_bundling.rs`](../../crates/project/tests/source_bundling.rs). Interruption/fault tests live in [`execute/tests.rs`](../../crates/project/src/source_collection/execute/tests.rs). UI cases live in [`collection/tests.rs`](../../crates/ui/src/collection/tests.rs). Stage 1's complete owner/conflict coverage remains in [`source_collection/tests.rs`](../../crates/project/src/source_collection/tests.rs).

## Test execution and CI

Run from the repository root with the pinned toolchain and native prerequisites:

```sh
cargo test --locked -p daw-project --test source_bundling
cargo test --locked -p daw-project --lib source_collection
cargo test --locked -p daw-project --lib file_publish::tests
cargo test --locked -p daw-project --lib source_storage::tests
cargo test --locked -p daw-ui --lib collection::tests
```

Then run the repository formatting, strict Clippy and workspace checks. The existing [native platform workflow](../../.github/workflows/platform.yml) already runs these tests through `cargo test --locked --workspace` and uploads `tests.log`. No duplicate test job or CI dependency gate is added. A new native runner log is required before recording a platform as passed; merely compiling or scheduling a test is insufficient.

The Windows-only fixture uses `powershell.exe -NoProfile -NonInteractive` and a constant `New-Item -ItemType Junction` command. Paths are environment data; no execution-policy setting, profile, overwrite flag, elevation or download is used. Junction creation failure fails the test instead of silently skipping coverage. The fixture requires a junction-capable local Windows filesystem. See the [Microsoft New-Item reference](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/new-item?view=powershell-5.1) for the junction item type and path parameter. Other Windows reparse kinds remain separate acceptance work.

The child-process test exits with a known nonzero status at existing filesystem hooks. It bypasses Rust cleanup guards and has a bounded 30-second parent deadline. It proves process-interruption behavior on the tested filesystem, not power-loss durability. It uses only exclusively created synthetic scratch fixtures and never moves user media.

## Native interaction checklist

Use a dedicated synthetic project and retain its original fixture/reference export. Do not use private recordings or an unsaved working project.

Create a separate copy of the tracked generated demo from the repository root. The fixture snippet uses Bash or Git Bash; run Cargo from your configured native build terminal. This does not regenerate or modify `examples/`:

```sh
python - <<'PY_FIXTURE'
from pathlib import Path
import shutil
import tempfile
Path("work").mkdir(exist_ok=True)
root = Path(tempfile.mkdtemp(prefix="source-bundling-native-", dir="work")).resolve()
shutil.copytree("examples/audio", root / "examples/audio")
shutil.copytree("examples/demo", root / "examples/demo")
print(root)
PY_FIXTURE
```

With the printed folder as `<scratch>`, build the CLI and render a reference before testing the native command:

```sh
cargo build --locked -p daw-cli
cargo run --locked -p daw-cli -- validate --project "<scratch>/examples/demo"
cargo run --locked -p daw-cli -- render --project "<scratch>/examples/demo" --output "<scratch>/before.wav"
cargo run --locked -p daw-desktop -- "<scratch>/examples/demo"
```

Use only this separate fixture instance for N-01–04. After collection, close its report and fixture window, rename only its copied `examples/audio` directory to `offline-audio`, and move only its copied `examples/demo` directory to `<scratch>/moved`. Reopen that folder and render `after.wav` with the same CLI options. Require no missing-source warnings and identical `before.wav`/`after.wav` bytes. Never rename the repository's `examples/audio` directory or a working project. The small demo supports menu/relocation checks; it does not satisfy the large-copy case by itself.

| ID | Procedure and pass condition | Current evidence |
| --- | --- | --- |
| N-01 | In each native File menu, unsaved Collect is disabled with its save-first explanation. Save the fixture, dispatch Collect, and verify the preflight/confirm/report flow. Existing Save/Save As/Export shortcuts remain unchanged; Collect has no shortcut. | Open. Native UI automation could not start locally. |
| N-02 | Play a quiet synthetic arrangement during preflight/copy. Confirm that playback/transport continue and selection/controls return after closing the report, with no renderer restart caused solely by relocation. | Open. No device interaction recorded. |
| N-03 | Use multiple distinct synthetic files totaling at least 256 MB within the preparation budget. Check window repaint, progress, cancellation acknowledgement, retry and no overwrites. Additional-byte totals exclude temporary space. | Open. Small automated fixtures do not establish responsiveness. |
| N-04 | Collect; move the entire folder; make fixture originals unavailable; reopen and compare playback/export. Include sample-only inputs, generated sources and a source restored from history before closing. Repeat Save As with the old folder unavailable. | Local automated filesystem coverage passes; native UI/device and Windows/Linux evidence open. |
| N-05 | On native Windows/macOS/Linux filesystems, run SB-01–09. Record filesystem, OS/build, toolchain, commit, command, result and log. Include case behavior, Unicode, aliases, sharing failures and containment. | macOS automated evidence only. Windows/Linux logs absent. |
| N-06 | Verify the documented commit/recovery behavior on additional filesystems and after process/machine interruption. Preserve files when commit is uncertain; do not infer an orphan is safe to delete. | Local process exit and injected errors covered. Machine crash, power loss, network/nonlocal filesystem and native uncertain-commit recovery open. |

## Local evidence and release decision

The local host reports macOS 27.0.1 (build 26A434) and rustc 1.97.0 (2d8144b78, Homebrew). The workspace and test temporary directories report `apfs` through the SDK `statfs` API. Evidence uses base commit `dddee35f36e5da0990318ccb3deb2a660d145153` plus the uncommitted Stage 5 test/document changes. A separate generated-demo copy was CLI-validated and rendered as a native-check reference; that is fixture preparation, not native interaction evidence.

Native app inventory was attempted twice through the computer-use tool. Both attempts failed with `Sky Computer Use native pipe startup failed`; no app/window was controlled and no native menu/device result is claimed. Windows/Linux native runners are not available in this session. Tests are wired into existing CI but were not pushed or dispatched here.

See [validation history](../validation/README.md) for completed commands/results. Stages 0–4 remain implemented. Stage 5 stays **in progress** until required native platform and interaction evidence is recorded. The tests and checklist do not waive that release gate.
