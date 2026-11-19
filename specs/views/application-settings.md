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

## JSON themes

Load embedded Dark, Light and Mist templates and discover user overrides from
the absolute per-user directory supplied by the desktop host. Scan at startup
and when the user selects Reload themes. Read files on a worker and publish
one catalog on the UI thread. A reload applies edits to the requested theme
without changing the project, audio configuration, or Undo history.

Version 1 accepts a UTF-8 JSON object with optional UTF-8 byte order mark.
Require `version` (integer 1), `id`, `name`, `base` and `colors`. Custom IDs
begin with `user.` and use a 1–48 character lowercase ASCII slug with letters,
digits and hyphens. Start and end the slug with a letter or digit. Trim names
and require 1–48 characters without control characters. A custom theme can
inherit the original Dark, Light or Mist with `base` set to `dark`, `light`,
or `mist`. A stock override uses the matching stock ID and name. Dark and Light
use their matching base. Mist uses `light`.

The `colors` object accepts any subset of `background`, `panel`, `input`,
`border`, `divider`, `text`, `time_unit`, `secondary`, `grid` and
`grid_subdivision`, and `button_background`. Values use opaque `#RRGGBB` sRGB colors. Reject unknown or
duplicate keys, nulls, invalid types and unsupported versions. Compare the
resolved foregrounds to background, panel and input. Require 4.5:1 contrast
for primary text and 3:1 for secondary text. Keep accent, clip and track colors,
semantic colors, fonts and control geometry in their existing owners.

Use `button_background` for the idle fill of standard framed buttons, joined
toggles and transport buttons. Each stock JSON defines its own value. Omitted
values inherit that role through the normal palette overlays. Hover and
active states, borders, unit headers and unit shells use their own colors.

Apply a unique valid Dark or Light override over its embedded palette. Resolve
Mist by applying the embedded Mist colors over effective Light, then applying
a unique valid Mist override. If inherited Mist has insufficient contrast and
no valid Mist override fixes it, use original stock Mist and report the issue.
Custom themes inherit original stock palettes. Keep the stock defaults and
theme preference. Persist an optional custom ID separately from the stock
fallback. Retain an unavailable custom ID so repair restores it on reload.

Scan at most 512 direct entries. Accept regular files with an ASCII
case-insensitive JSON extension. Skip directories and symlinks. Reject files
larger than 32 KiB. A missing folder is empty. A folder-wide scan error keeps
the previous catalog, while a bad file is reported and excluded. Reject all
valid files with the same ID and show their paths. Sort stock entries first,
then custom entries by case-insensitive name and ID.

Create theme overrides makes the themes directory and writes only missing
stock files. Copy stock JSON returns the original template for the current
base. Show the directory, current fallback and file diagnostics in Appearance.
Apply the resolved palette to egui style, shared painting and native window
appearance. Keep Dark and Mist Mixer header text white. Use Light palette text
for Light Mixer headers. Standalone plugin editors retain their appearance API.

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
