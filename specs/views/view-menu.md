# View menu panels

## Accepted behavior

The View menu lists Timeline, Instrument, Editor and Mixer, followed by a
separator, all toggleable workspace panels, a second separator, and the
full-screen command. macOS retains AppKit's native full-screen command. The
in-app menu sends the equivalent viewport command.

| Panel | Available views |
| --- | --- |
| Tracks | Instrument, Editor |
| Components | Instrument |
| Effects Catalog | All |
| Chords | Timeline, Editor |
| Clip Settings | Timeline |
| Track Mixer | Timeline |
| Effects Lane | All |
| Returns | Mixer |
| Master | Mixer |

Keep unavailable panel entries visible, disabled and unchecked. A supported
panel's checkmark reads the current view's actual visibility, including when a
job temporarily disables its command. Refresh native menu states every update;
do not cache checks when creating the menu. View switches expose that view's
existing preferences. Preserve the existing Clip Settings reset on leaving
Timeline. Catalogs and inspectors may still show their existing empty-selection
placeholders; menu availability matches the corresponding toolbar panel.

## Shared state and dispatch

`daw_ui::ViewPanel` owns stable panel metadata and the view-availability matrix.
`DawUi::view_panel_enabled`, `view_panel_selected` and `toggle_view_panel` are the
shared native/in-app menu contract. Toolbar toggles call the same dispatcher.
Retain existing dialog, preparation and active-gesture guards, catalog selection
cleanup, catalog exclusivity and effects-lane selection cleanup. No panel toggle
changes project history, audio routing or processing.

Native AppKit items use bounded enum tags, no new key equivalents, and the
existing main-thread command queue. Revalidate queued actions against the current
view before toggling. Keep the two panel-section separators before AppKit adds
its full-screen item.

## Validation

Check the availability matrix, checked-state refresh across views, unsupported
and blocked actions, catalog exclusivity, toolbar/menu parity, section ordering
and separators, and full-screen viewport command dispatch. Keep headless texture
delta cleanup and run both debug and release workspace tests. Native macOS menu
appearance and Windows/Linux window behavior require platform checks.

## Timeline Track Mixer

The right Timeline rail has Clip Settings followed by Track Mixer (sliders icon).
Both use the shared View-panel dispatcher and gesture/job/dialog guards. These
inspectors are mutually exclusive; opening Track Mixer cancels unconfirmed clip
settings. Left catalogs remain independent. Track Mixer starts hidden and retains
its choice across view switches in the app session. It is absent outside Timeline.

Reserve two standard Mixer strip widths (120 logical points each) outside the
Timeline canvas and drop bounds. Keep the right rail available when the inspector
is closed. Constrain the inspector to leave the existing minimum Timeline width;
scroll the strips horizontally and vertically when necessary.

Resolve the primary selected track by stable ID each frame. Show that track on
the left and its immediate `group_id` bus on the right, or Master when ungrouped.
Group and Return tracks follow the same rule. Master selection shows Master once,
with no parent. Always show the standard ControlPanel “Track mixer” header and
its bottom divider, including when a track or Master is selected. Place the
scrolling strips below this fixed panel header. Empty selection shows the
“Select a track” placeholder, using the same padding, small secondary text and
form-row baseline as Clip Settings. Selection and topology
changes must update the strips without stale references.

Reuse the [Mixer output-destination label](mixer.md), `mixer_strip` and its headers, numeric fields, faders, meters, send rack and
control updates. Headers use Mixer geometry, colors and text; inspector names are read-only
and must not start Timeline track drags or duplicate Timeline rename fields. Controls edit their own track without switching selection.
The right toolbar owns the single divider between the inspector and toolbar;
Master must not add a second outer strip border in Timeline.
Panel toggles and rendering do not edit project history or audio routing. All
controls respect blocked editing. No new audio processing or callback work is
introduced.

Validate toolbar/menu parity, inspector exclusivity, parent resolution, Master
selection, control edits, blocked input, view switching and canvas separation with
automated egui frames. Native layout, scrolling and meter motion need UI checks.
