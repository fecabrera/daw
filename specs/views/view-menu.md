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
| Effects Lane | All |
| Returns | Mixer |

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
