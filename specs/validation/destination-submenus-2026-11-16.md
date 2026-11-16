# Modulation destination submenus — 2026-11-16

The shared DAW Destination menu groups removable strips for Oscillator, Macro,
Multiband EQ, Multiband Compressor and Exciter. Unit-wide destinations precede
one divider. Fixed Harmonic Lift targets remain flat. Group identity uses strip
UUIDs; short leaves and qualified selected captions use explicit UI metadata.
The backing target inventory retains existing new-connection defaults.
See the [contract](../modulation/contract.md#destination-strip-submenus) and
[user guide](../../docs/modulation/README.md).

## Automated checks

- `cargo test -p daw-ui --lib modulation_panel --offline`: 20 passed.
- Final `cargo test -p daw-ui --lib --offline`: 1,038 passed, 3 ignored.
- `cargo test --workspace --offline`: 3,541 passed, 22 ignored.
- `cargo test --workspace --release --offline`: 3,541 passed, 22 ignored.
  Native macOS Audio Unit registration was available outside the sandbox.
- The tests cover all five actual destination inventories, exact Oscillator
  root/leaf order and supported strip counts, Compressor presentation versus
  signal order, duplicate Unicode Macro names containing the label separator,
  absent retained controls, target preservation after reorder/add/remove and
  serialization, amount bounds and target-specific source eligibility.
- Actual pointer interaction checks verify read-only parent browsing, leaf
  assignment, full popup closure and qualified selected captions in Dark,
  Light and Mist. Existing full-app Oscillator interactions now choose child
  leaves instead of the previous flat labels. Existing Macro cycle/history and
  Source/audio-sidechain interaction coverage passed in the full UI run.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `sh tools/package_macos.sh debug --local`: passed, including app signature
  verification. Updated bundle: `target/debug/DAW.app`.
- Local documentation link targets and `git diff --check`: passed.

Initial compile checks caught an unhashable parameter salt and a test theme API
mismatch; both were corrected. The prior Oscillator interaction fixture also
needed to distinguish strip-footer text from identical popup-parent text and
child leaves from root Pitch/Cents. The final full UI run passed.

## Native observation

Used a separate macOS eframe preview with the public DAW UI, bundled fonts/icons,
default preferences and no persisted egui memory. Created an unsaved MIDI track
and two oscillators without opening a user project. Checked the first connection
still targets Osc 1 · Pitch. Inspected the root Level/Pitch/Cents order, divider,
Osc 1/Osc 2 parents, Small text, arrows and leaf Pitch/Gain/Cents/Width order.
The child popup opened left at the screen edge. Selecting Gain under Osc 2
closed the menus and produced the qualified Osc 2 · Gain selected value.
Closed the isolated preview without saving. Its source/bundle remain ignored
scratch files in work; the temporary desktop example was removed before final
workspace checks.

Native keyboard/focus traversal, other receiving units, large strip counts,
other scales/themes and Windows/Linux behavior remain unverified. Automated
menu checks do not establish these native results. No listening or MIDI-device
check was performed; audio processing and persisted routing formats are unchanged.
