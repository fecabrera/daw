# Compound audio Merge validation

Date: 2026-11-12.

Implementation contract: [Audio clip Merge](../editing/audio-merge.md).

## Automated coverage

- Source sample parity at all six project rates, ordinary/internal/exterior
  crossfades, repeat phases, muted-neighbor fallback, channel operations and
  mixed mono/stereo members through live track processing.
- Sparse gaps, nested merges, exact source bounds, retained original assets,
  stable result identity, version/serialization/cycle validation, canonical
  no-op, new-cut edge delegation and explicit nesting admission.
- Compiled metadata expansion admission and huge sparse gaps without dense PCM.
  Oversized linearization and cancellation retain original model/audio owners.
- Worker extraction, Normalize, Sampler, transpose and processing-rate changes.
  Transform continuation handles, independent prefix/body frame rounding,
  cropped source windows and generated-stem boundaries.
  Verified missing originals retain warnings/silence; shortened-source
  extraction fails rather than publishing unavailable frames as valid samples.
- Save/Open/Save As, source collection after removal of external originals and
  history restoration with the retained leaf sources.
- Atomic UI publication, result selection, one-step Undo/Redo and failure in
  Timeline and Editor. Existing MIDI/beat Merge menu/shortcut guards remain
  covered. Optional transient analysis retains its worker/cache tests.
- Chorus/compressor state inheritance across transparent Merge and disk-backed
  seeks/loops and warm-reader inheritance, with instrumented zero callback allocations.
- Headless CLI compound loading, inspection, render parity, source collection
  and unsupported-recipe-version rejection, alongside the existing smoke suite.

## Commands

The default-profile and release workspace suites passed, including doctests.
The complete affected-owner suites reran after the final source-state inheritance
change. Focused regressions cover frame bounds, cropped windows and continuation
previews.

| Command | Result |
| --- | --- |
| `cargo test -p daw-engine --test compound_audio -p daw-project --test compound_audio --offline` | Passed. |
| `cargo test -p daw-ui clip_merge --offline` | Passed. |
| `cargo test -p daw-ui editor_transients --offline` | Passed. |
| `cargo test -p daw-engine audio_merge_inherits --offline` | Passed; effect state and callback allocation check. |
| `cargo test -p daw-engine --test streaming compound_disk --offline` | Passed; resident/disk parity, seeks, loops and callback allocations. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Passed. |
| `cargo test --workspace --offline` | Passed, including doctests. |
| `cargo test --workspace --release --offline` | Passed, including doctests. |
| `cargo test -p daw-engine -p daw-output -p daw-project -p daw-ui --offline` | Passed after the final source-state change. |
| `cargo test -p daw-engine -p daw-output -p daw-project -p daw-ui --release --offline` | Passed after the final source-state change. |
| `cargo build -p daw-cli -p daw-desktop --offline` | Passed. |
| `python3 tools/check_cli.py` | Passed, including the compound fixture; repeated with an isolated temporary `DAW_AUDIO_CACHE`. |
| `git diff --check` | Passed. |
| Changed Markdown local file targets | Passed; 342 local file targets. |
| `sh tools/package_macos.sh debug` | Initial attempt blocked by the existing corresponding-source guard: `Commit tracked changes before packaging corresponding source.` No commit or bypass was made. |

No dependencies, lockfile, effect DSP, plugin products or notices changed.
No Rubber Band implementation source was inspected or adapted; preparation
continues through the existing public backend adapters.

## Remaining native acceptance

Native Timeline/Editor menus and gestures, waveform appearance, real-device
playback, Stop during preparation, listening across joins, underruns and platform
filesystem lifetime checks were not run. Windows and Linux native checks remain
for their platform jobs. Automated egui and allocation tests do not establish
these results. No older executable was run; old-reader rejection follows the
strict source-kind validator contract. The desktop binary was built. The initial bundle attempt required committed
source; packaging is checked again after the implementation commit.
