# Instrument project track browser

## Scope and composition

Instrument adds a List Tree **Tracks** toggle above Instrument components and Effects in the existing left toolbar. It starts closed. Track navigation, components and effects share one 250-point `ControlPanel`; opening one closes the others in Instrument. Other views keep their existing toolbar controls and visibility choices. The shared workspace retains its single top border and bottom effects lane.

[`track_browser.rs`](../../crates/ui/src/track_browser.rs) owns dynamic tree rows, browser-only expansion, cursor and focus. Reuse `ControlPanel::catalog_body` and the common catalog row painter for small text, rounded selected fill, row padding/margins, four-point gaps and keyboard focus appearance. Root audio/MIDI rows use the existing Audio Lines/Keyboard Music icons. Group rows use the shared Chevron Right/Down disclosure without a folder icon. Each nesting level adds one icon slot of label indentation; rows keep full shared hit/selection bounds. Keep dynamic names and stable project/track UUIDs separate from static catalog registration.

## Navigation and state

Read all project tracks in validated parent-before-child order, including unused/empty tracks and groups. Derive visible rows from browser expansion, independently of saved `group_collapsed`. Preserve sibling order and arbitrary nested groups. Groups start closed; retain choices through panel/view toggles. Reconcile deleted group IDs and clear expansion/cursor/focus when the project identity changes. Do not serialize navigation or create history edits.

Clicking a group toggles its children; it does not change selected tracks or saved collapse state. Clicking an audio/MIDI row uses shared single-track selection, leaves the view and clip selection unchanged, and retires stale layer/component gestures and targets. MIDI rows show that track's existing instrument. Audio rows keep the existing no-MIDI-instrument presentation and use their shared track effects lane. This extends the historical MIDI-only Instrument effects target; the top MIDI dropdown retains its current filter. Reuse the effects workspace's same-frame target refresh before painting the bottom chain.

Use Up/Down for bounded visible-row navigation, selecting track rows and focusing group rows. Enter selects tracks or toggles groups. Right opens a group; Left closes it or focuses the parent of a child. Retain egui arrow focus filtering through the shared `selection_focus` helper, as catalogs do. Scroll keyboard-focused rows into view. Keyboard handling requires list focus, no text edit, a focused/enabled application, settled gestures and an idle job/dialog path. Track selection and disclosure are disabled under the existing workspace guards. No track creation, rename, reorder, drag/drop or audio processing is added here.

## Acceptance

[`track_browser/tests.rs`](../../crates/ui/src/track_browser/tests.rs) covers saved order, all track kinds, nested groups, independent saved collapse state, toolbar/panel switching, same-frame MIDI/audio effects targets, unchanged project/clip state, bounded keyboard navigation, rename/removal, disabled input and shared row geometry in Dark/Light/Mist at 1×/1.5× scale. Existing catalog paint/focus and workspace suites remain required. Native app/device and Windows/Linux checks are separate from automated egui tests; executed results belong in [validation history](../validation/README.md).
