# Settings sidebar — 2026-11-16

The Settings dialog now uses five sidebar groups and shows only the selected
page. See [the specification](../views/application-settings.md) and
[user guide](../../docs/projects/README.md#application-settings).

## Automated checks

- Full UI suite: 1,032 passed, 3 ignored in both debug and release.
- New interaction checks select all five pages, verify other groups' controls
  are absent, retain the selected page, and confirm navigation leaves preferences
  unchanged in Dark, Light and Mist.
- Geometry checks keep every sidebar item and Close visible at 640×480 and
  900×700. Existing MIDI split and audio-device tests now navigate to their pages.
- Full workspace debug/release: 3,535 passed and 22 ignored in each profile.
- Strict workspace Clippy, formatting, local links and `git diff --check`: passed.
- Local macOS debug bundle rebuilt and passed signature verification.

## Native observation

Used an isolated macOS eframe preview with the same Settings implementation,
shared theme, bundled fonts/icons and default preferences. The existing DAW
project and its preferences remained untouched.

Verified Appearance, MIDI and Playback navigation through native accessibility
state. Inspected Dark-theme screenshots for the sidebar selection, exclusive
content, divider, aligned fields, full-width title bar and persistent Close
button. Closed the preview afterward. The preview source and bundle are ignored
scratch artifacts in `work/`, not product files.

Native Windows/Linux and other desktop sizes remain unverified. The native
preview did not initialize audio/MIDI devices or validate separation models;
existing automated control tests cover action wiring.

## Divider and sidebar spacing follow-up

The footer divider spans the dialog width. The category divider connects the
title bar to the footer divider. The sidebar now uses `ControlPanel::catalog_body`,
the same container as the Editor and Instrument Tracks lists, for its outer
padding and row spacing. An isolated native Dark-theme preview confirmed the
compact rows, edge insets and connected dividers; the preview was then closed.

- Targeted Settings tests: 27 passed.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `sh tools/package_macos.sh debug --local`: passed, including signature checks.

An initial workspace run failed because a temporary native-preview example was
removed after Cargo discovered it. The example is absent from the final tree;
workspace checks were restarted after preview cleanup.

- Final `cargo test --workspace --offline`: 3,535 passed, 22 ignored.
- Final `cargo test --workspace --release --offline`: 3,535 passed, 22 ignored.
- Local documentation link targets and `git diff --check`: passed.

Both full test runs used native macOS Audio Unit registration outside the
sandbox. Native Windows/Linux layout checks remain pending.
