# Beat subtracks in Mixer — 2026-11-16

Mixer now expands beat parents into child subtrack strips using the same strip
renderer and group hierarchy presentation as ordinary project tracks. This
supersedes the former parent-only Mixer scope. See the
[implementation contract](../views/mixer.md#beat-subtrack-strips) and
[user guide](../../docs/views/mixer.md#beat-tracks).

## Automated checks

- Seven focused beat-Mixer tests passed. They cover nested color gutters,
  saved row order and destinations, aligned output controls, lane-only fader/M/S
  edits, shared effects selection, parent-scope restoration, saved collapse,
  serialization and Undo/Redo, removal/restoration without stale strips,
  multi-selection shortcuts and narrow overflow with pinned Return/Master banks.
- Existing Mixer tests: 28 passed after extracting the shared renderer.
- Final `cargo test -p daw-ui --lib --offline`: 1,045 passed, 3 ignored.
  The earlier parent-only Mixer shortcut assertion now checks selected-lane
  scope and explicit restoration of parent scope; other shortcut guards remain.
- `cargo test --workspace --offline`: 3,548 passed, 22 ignored.
- `cargo test --workspace --release --offline`: 3,548 passed, 22 ignored.
  Both full profiles used native macOS Audio Unit registration outside the sandbox.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- Local documentation link targets and `git diff --check`: passed.
- `sh tools/package_macos.sh debug --local`: passed, including app signature
  checks. Updated bundle: `target/debug/DAW.app`.

The implementation reuses existing beat mix transactions, layer-meter handles,
owner selection, effect-chain targeting and name/context controls. It introduces
no project tracks, audio routes, persisted fields or callback work. Temporary
preview source was removed from desktop examples before workspace checks.

## Native observation

Used an isolated macOS eframe app with the public DAW UI, shared fonts/icons,
default preferences and no persisted egui memory. Created a temporary unsaved
beat track and two empty subtracks; no user project was opened.

Inspected the Mixer screenshot for the parent disclosure, connected color
gutters, child icon/name headers, parent output labels, aligned faders/meters/M/S
and pinned Master. Toggled the first child's Mute: its parent and sibling stayed
off. Collapsed and expanded the parent: both child strips returned and the
first child's Mute remained on. Closed the preview without saving. The ignored
preview source/bundle remain under work.

Live audio and meter movement were not exercised. Meter identity and signal
processing paths are unchanged; these observations cover presentation and
control isolation. Native Windows/Linux, other display scales, keyboard rename
and live-device behavior remain unverified. Automated geometry tests cover
Dark, Light and Mist plus 640×480 and 1280×800 layouts; they do not establish
native results on those other platforms.
