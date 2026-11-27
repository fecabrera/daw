# View menu panels

## Accepted behavior

The View menu lists Timeline, Instrument, and Mixer, followed by a
separator, all toggleable workspace panels, a second separator, and the
full-screen command. macOS retains AppKit's native full-screen command. The
in-app menu sends the equivalent viewport command.

| Panel | Available views |
| --- | --- |
| Components | Instrument |
| Effects Catalog | All |
| Chords | Timeline |
| Clip Settings | Timeline |
| Track Mixer | Timeline, Instrument |
| Effects Lane | All |
| Timeline Editor | Timeline |
| Returns | Timeline, Mixer |
| Master | Timeline, Mixer |

Keep unavailable panel entries visible, disabled and unchecked. A supported
panel's checkmark reads the current view's actual visibility, including when a
job temporarily disables its command. Refresh native menu states every update;
do not cache checks when creating the menu. View switches expose that view's
existing preferences. Preserve the existing Clip Settings reset on leaving
Timeline. Catalogs and inspectors may still show their existing empty-selection
placeholders; menu availability matches the corresponding toolbar panel.

The Effects Lane starts open and shares one visibility setting across Timeline,
Instrument, and Mixer. In Timeline, the editor and Effects Lane share the
lower slot. Opening one hides the other. Each view remembers its own
catalog choice. Instrument’s full Tracks panel is always visible.

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
its choice across view switches in the app session. Instrument has a separate
visibility choice for the same panel.

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

## Timeline Return and Master visibility

Timeline reuses Mixer's Return (`CornerDownLeft`) and Master (`BoomBox`) toolbar
buttons in the existing right toolbar. Stack Returns above Master at the bottom,
using the standard toolbar width, button size and inset. Retain Clip settings and
Track mixer at the top. When the effects lane is open, keep the visibility
buttons at the bottom of the upper rail, above the lower lane's right toolbar.
When closed, place them at the bottom of the full-height upper toolbar. Both buttons use the shared View-panel dispatcher,
selection style, tooltips, accessibility names and editing guards.

Store separate Timeline and Mixer visibility choices in UI state. Default each
to visible and retain them across view switches for the app session. Hidden
Timeline Returns reserve no bank height; hidden Master reserves no pinned row
height. Do not paint or interact with hidden rows, their dividers or drop targets.
Keep selection, project history, audio routing and processing unchanged.

## Instrument right toolbar

Reserve a standard-width right toolbar outside the Instrument canvas and drop
targets. Keep Track Mixer at the top. Its state starts closed and stays
independent of Timeline. Constrain the panel to leave room for lane labels and
the lane-specific collapse buttons.

## Instrument subviews

Show two joined control groups at the top of Instrument. The first group lists
the selected instrument’s layers by stable ID and saved name, followed by its
Add layer button. Round the two outer edges of the group. The second group
contains LFOs, Envelopes, and Macros and stays visible. When the layer group
does not fit, scroll only its named tabs. Pin the Add layer button to the
right edge of the Layer group, beside the modulation group.
Use the toolbar's 8-point padding between the two groups.
At narrow widths, put the Track Mixer below the tab bar when it would cover
the modulation group or leave too little room to scroll the layer tabs.
Place the groups beside the shared Timeline track headers. Disable the
modulation buttons for audio tracks, groups, and beat parents.

- The first layer opens by default. Show the selected layer’s source row, then
  its Effects row, then a gap and its mix controls. The bottom Effects Lane
  continues to show the track or beat-subtrack chain.
- LFOs and Envelopes each open their fixed provider lane. Macros opens the
  instrument Output controls and any groups of layer-owned Macros. Show only
  the matching type in the Instrument components catalog.

Instrument uses the Timeline track header controls, including a pinned Master,
with shared vertical scroll. Both views use the same track height in the
transport toolbar. The horizontal slider and Fit All Clips remain visible and
disabled in Instrument.

Tab switches change presentation state. Use layer tabs for group selection
and reordering and as cross-layer drop targets.

The layer tab context menu targets the clicked layer. Show Duplicate, a
divider, Mute and Solo, another divider, and Delete. Use the platform shortcut
hints for Command/Ctrl+D, M, S, and Command/Ctrl+Delete. Opening the menu
preserves selection. Disable deletion on the final layer. The duplicate
shortcut targets the primary selected layer and uses the normal layer copy
transaction.

Only geometry in the active tab participates in selection and drops. Reuse the
existing lane editors, collapse state, owner IDs and edit transactions. Keep
audio rendering, saved component order and modulation discovery independent of
the active subview.

Draw one full-width bottom divider on the last displayed row in each instrument subview,
including Macros. Adjacent rows share a single divider.
Clip the dividers to the lane scroll area. New rows start expanded. Loading a
project restores its saved row heights.
