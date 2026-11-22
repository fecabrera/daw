# Application Settings

The shared Settings surface has a left sidebar and a right content panel. List
Appearance, Playback, Time stretch, MIDI, Recording, Stem Separation and
Plugins in that order. Highlight the active group with the shared catalog selection style.
Show only that group's controls and advice in the content panel.

Appearance is initially active. Remember navigation in transient application UI
state across dialog reopening and project changes during the same app session.
Do not serialize navigation in AppSettings or projects, mark a project modified,
or add an Undo entry when the active group changes.

Keep existing controls, defaults, preference persistence, native-device actions,
model validation and busy/error guards. A page change must not invoke a hidden
control or change a preference. Changes to actual preferences still apply and
persist through the established app settings path.

Use the same shared catalog body as Instrument’s Tracks panel
for sidebar padding and row spacing.

Keep the sidebar and Close action visible. Add an edge-to-edge divider above the
footer. The divider between the sidebar and settings runs from below the title
bar to that footer divider, through the content padding. Bound dialog dimensions
to the viewport and scroll the active content panel when required. Retain independent
scroll state per group. Reuse shared field alignment, typography, dropdowns,
buttons and the canonical tooltip presentation. The shared dialog's optional
width override must leave other dialogs' defaults unchanged.

Show Audio quality defaults inline in Playback, below a separator after the
device and seek cache controls. Use the shared field alignment. Apply saves
validated defaults for new projects. Cancel or closing Settings discards the
draft. Escape dismisses an open popup first, then discards an unsaved quality
draft when Playback is active. With no active draft it closes Settings.

Desktop Settings uses a single native window with regular platform chrome
and a standard close button. macOS WGPU and Windows Glow use a deferred eframe
viewport. Linux uses a GTK transient window. Keep native hosting independent
of the main-window Titlebar preference. Retain the embedded Settings dialog
for hostless use and when the child window fails to start or present.
Retain the session, active group, scroll positions and picker draft on fallback.

Reserve a fixed bottom panel in the native client. Give the Close row the
shared horizontal and vertical padding. Draw its full-width divider at the
top of the footer. Size the sidebar and scrollable page from the remaining
client height so they meet the divider and keep the footer visible as the
window resizes.

Use the same footer in project prompts and reports. Fit the native client
around the content and footer. Keep dialog actions below the divider during
progress and on validation errors. Align Deny at the left and Accept at the
right. Place Cancel immediately to the left of Accept when both are present.
Show only the actions the dialog offers. Use the shared accent color with
white text for Accept, including Close on an acknowledgement dialog.
Cancel and Deny use neutral buttons. Use the right edge for a progress dialog
that only offers Cancel. Place Open last saved in the secondary position in
the recovery footer.

Treat the active native dialog as modal to the main window. The desktop
host keeps the dialog above the owner and blocks owner input. If another
dialog takes over, transfer ownership to the active dialog. Native pickers
retain their own modality. If the desktop host falls back to an embedded
dialog, keep workspace widgets disabled and limit input to the modal layer.

Keep one Settings session and block main-window editing through child
retirement. Repeated Settings requests focus the current session. Native
close and footer Close retire the session through the same state transition.
Keep the Audio quality draft in the Settings session. Errors and model
validation use separate shared native dialogs and take focus when needed.
Return to the same Settings session after dismissal. Queue root close while a Settings picker holds its native
parent, then apply the normal project close guard after the picker resolves.
Discard results from a retired Settings session.

Attach native file and folder choosers to the active Settings window. Use
`FileChooserNative` on GTK and async RFD on eframe. If the eframe Settings
owner is unavailable after a bounded attempt, suspend Settings and use
the verified root window as the chooser owner. Resume the same Settings
session after the chooser returns. Keep the existing validation and commit
paths for received file and directory choices.

Write eframe autosave geometry from the main viewport, including on a
Settings redraw. A watched child reports graphics failure or successful
presentation to root. Contain child surface failures without stopping the
main renderer. The host checks that the child presents within the startup
deadline and uses the embedded surface if it does not.

Validate actual group clicks, exclusive page content, preference isolation,
device selection and MIDI-note split interaction, theme/accent behavior, smaller
windows and persistent Close/sidebar visibility. Separate native observations
from headless geometry/interaction checks.

## MIDI and Recording

Keep Split MIDI notes, MIDI input, MIDI channel and input status on MIDI.
Show the count-in duration and enable toggle, Recording offset, Panic, and
retained MIDI take retry on Recording. Keep both pages on the existing MIDI
preference and recording action paths.

## Accent color

Show eight circular Material 400 swatches in a single row to the right of
the Accent color label. Order them Red, Deep Orange, Amber, Green, Indigo,
Deep Purple, Pink, and Blue Grey. Reduce their spacing and diameter together
when the content narrows. Give the selected swatch a contrasting ring.
Each swatch has a tooltip, an accessible name and selection state, and
supports keyboard activation.

Apply the chosen Material 400 shade immediately as the main accent for all
themed controls. Use Red for new settings. Continue to load any previously saved
accent. When the saved accent is outside the current palette, show its name
on label hover and leave the swatches unselected until the user picks one.

## JSON themes

Load embedded Dark, Light, Mist and Midnight templates and discover user overrides from
the absolute per-user directory supplied by the desktop host. Scan at startup
and when the user activates the refresh-cw icon beside the Theme dropdown. Read files on a worker and publish
one catalog on the UI thread. A reload applies edits to the requested theme
without changing the project, audio configuration, or Undo history.

Version 1 accepts a UTF-8 JSON object with optional UTF-8 byte order mark.
Require `version` (integer 1), `id`, `name`, `base` and `colors`. Custom IDs
begin with `user.` and use a 1–48 character lowercase ASCII slug with letters,
digits and hyphens. Start and end the slug with a letter or digit. Trim names
and require 1–48 characters without control characters. A custom theme can
inherit any original stock theme by its ID in `base`. A stock override uses
the matching stock ID and name. Dark and Light use their matching base. Mist
uses `light` and Midnight uses `dark`. Treat the legacy ID `user.midnight` as
the stock ID `midnight` in theme files and migrate that saved selection to
the built-in choice.

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

Apply a unique valid Dark, Light or Midnight override over its embedded palette. Resolve
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

Join the Theme dropdown and the Refresh themes icon in a shared input group.
Show the directory directly below the Appearance controls on its own row.
Place the square-text path-copy icon and folder-open icon at the row's right edge.
Use the standard toolbar icon size for all three theme actions. Create
the directory if needed before opening it in the desktop
file manager. Show the current fallback and file diagnostics in Appearance.
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
