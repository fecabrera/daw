# Application Settings

The shared Settings dialog has a left sidebar and a right content panel. List
Appearance, Playback, Time stretch, MIDI and Stem Separation in their existing
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
