# Application Settings

The shared Settings dialog has a left sidebar and a right content panel. List
Appearance, Playback, Time stretch, MIDI, Stem Separation and Plugins in that
order. Highlight the active group with the shared catalog selection style.
Show only that group's controls and advice in the content panel.

Appearance is initially active. Remember navigation in transient application UI
state across dialog reopening and project changes during the same app session.
Do not serialize navigation in AppSettings or projects, mark a project modified,
or add an Undo entry when the active group changes.

Keep existing controls, defaults, preference persistence, native-device actions,
model validation and busy/error guards. A page change must not invoke a hidden
control or change a preference. Changes to actual preferences still apply and
persist through the established app settings path.

Use the same shared catalog body as the Editor and Instrument Tracks panels
for sidebar padding and row spacing.

Keep the sidebar and Close action visible. Add an edge-to-edge divider above the
footer. The divider between the sidebar and settings runs from below the title
bar to that footer divider, through the content padding. Bound dialog dimensions
to the viewport and scroll the active content panel when required. Retain independent
scroll state per group. Reuse shared field alignment, typography, dropdowns,
buttons and the canonical tooltip presentation. The shared dialog's optional
width override must leave other dialogs' defaults unchanged.

Close and Escape retain the existing behavior. Escape dismisses an open popup
before closing Settings. Audio quality defaults remain the existing secondary
dialog opened from Playback.

Validate actual group clicks, exclusive page content, preference isolation,
device selection and MIDI-note split interaction, theme/accent behavior, smaller
windows and persistent Close/sidebar visibility. Separate native observations
from headless geometry/interaction checks.

## Plugins

Keep discovery configuration in this page. Show the resolved standard folders,
saved additional folders with their VST3/CLAP type, and scan status/failures.
Default folders remain included. Additional paths are machine preferences, never
project data. Share them with CLI discovery. Provide Scan folders, Rescan all,
Retry failed and Cancel scan. Run traversal and isolated scan supervision on a
worker; a scan or settings page change must not edit a project or stop playback.

Plugin selection belongs in the existing catalogs. Both Effects and Instrument
components have VST3 and CLAP folders below a divider. Show effects and
instruments in their respective catalogs and preserve their identities on
refresh. Use the shared catalog styling and keyboard navigation. Double-click
or press Enter to add the selected external unit.
