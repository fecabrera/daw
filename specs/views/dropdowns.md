# Dropdowns and catalog list spacing

This is the canonical dropdown presentation for the DAW and independent plugin editors. It supersedes selector-specific popup styling. Collapsed selectors retain their existing size, captions, tooltips and enabled behavior.

## Default framed control surfaces

The shared `unit-ui` theme is the default for all framed buttons, joined control
groups and collapsed dropdowns in the DAW and independent plugin editors. Use
the unit content background (`unit_separator`) for idle fill, the neutral unit
header background for hover fill, and 5-point corners. Draw no control stroke
in idle, hover, pressed, selected, open or disabled states. Retain existing
active/selected fills, foreground colors, dimensions, padding and interaction.
Text inputs retain their input fill, text, caret and selection feedback, with
5-point corners and no idle, hover or focused border/outline. The caret remains
1 point; the selection stroke has zero width without changing its color.
DAW numeric gain and held-peak fields share an explicit input-colored frame
with 5-point corners, empty stroke and the existing 4-by-2-point text margins.
The held-peak field remains read-only and retains click-to-reset monitoring.

Joined groups have no inset frame. Round only the outside corners of the first
and last segments, keep the shared inner edge square, and draw a full-height
1-point separator in the unit-header background color. Each separator reserves
its own 1-point layout gap; it must not overlap either control. Group width is
the sum of segment widths plus 1 point per separator. Keep segment dimensions
and group heights unchanged. This also applies to joined input/dropdown groups,
including root/sharp, duration/mode, Unison/Detune and send destinations.
Source-specific controls remain owned by the DAW, while the default
surface styling lives in the shared theme.

Other frameless toolbar and view toggles retain their transparent idle
surfaces and existing hover/semantic active colors. Hover, pressed and active
backgrounds use 5-point corners through the shared DAW toggle radius. Catalog
rows retain 2-point rounding. Circular power buttons and knob faces retain their
prior surfaces. Popup rows retain the canonical style below, including 2-point
corners. Workspace/panel separator geometry is separate
from control strokes and retains its existing borders.

### Transport buttons

Play, Stop, Loop and Metronome are independent standard 30-by-24-point buttons
with 16-by-16-point icons and 5-point corners on all sides. Use the standard
7-point toolbar gap between Play, Stop and Loop. Do not draw a shared frame or
internal dividers. Idle background/foreground and hover background/foreground
use the standard control palette. Preserve semantic active fills: Play is green
(51, 133, 85), Stop is red (197, 78, 88), and Loop and Metronome use `LOOP_COLOR`,
all with white active artwork. Retain action guards, focus, keyboard shortcuts,
return-to-origin behavior and the surrounding toolbar height.

## Shared dropdowns

Use `unit-ui::menus::Dropdown` for all selectors. It wraps egui's ComboBox, retaining its interaction, popup scrolling, dismissal and accessibility. The DAW imports the module as `menus`; portable unit panels use it directly. Keep concrete choices and edits in their existing owners.

Custom icon-button action dropdowns use `Dropdown::show_from_button`. Retain the trigger’s geometry, tooltip and accessibility name; anchor the popup to the button with egui’s menu opening, focus and dismissal behavior. Use the same regular popup style and full-row layout as context menus. Disabling the trigger closes its popup. Keep choice lists and action dispatch in their host.

- Unit selectors use `Dropdown::small_items()` for Small popup text (11 points), including modulation source/destination menus and their layer submenus. Nested unit menus use `menus::small_popup_style`; measure their labels with Small text. Other selectors and application/context menus retain regular Button text (13 points). Selected captions retain their existing size.
- Apply 4-point inner padding around labels and submenu arrows, 4-point outer popup margins, and 4-point vertical gaps between items.
- Give root and child popup contents a minimum width of 140 points. Longer labels expand the popup; keep labels on one line and each row's hit area and highlight at full content width. Hover must not wrap labels or resize rows.
- Use rounded, borderless row highlights and theme secondary text for inactive rows. Hover, press and open submenus use `theme::TOOLBAR_HOVER_FILL`, the same translucent grey as the view buttons. Selected choices retain their accent-tinted fill. Use normal text color over the whole highlighted row, including text, padding and arrow. Disabled choices retain egui's disabled behavior.
- Use the shared 16-point Lucide ChevronRight for submenu arrows, tinted with the row text color. Retain egui's submenu hover, focus and dismissal behavior.
- Use `menus::separator`: the divider occupies only its 1-point line height. The shared 4-point item gap supplies the space above and below it; add no divider padding.

`menus::popup_style`, its Small-text variant `menus::small_popup_style`, and `menus::layout` also serve nested submenus. Scope these changes to popup contents; do not change button padding or text globally.

## Windows and Linux application menu

The burger menu and its File, Edit, Clip, Track, and View submenus reuse `menus::popup_style` and `menus::layout`. Use the same regular text, full-row rounded highlights, minimum popup width, padding, item gaps, and Lucide ChevronRight arrows as dropdown menus. Action groups use `menus::separator` without added divider padding. View choices use `menus::checked_button`, which retains the active-view checkmark inside a shared selectable button row. Preserve existing menu order, shortcuts, checked views, enabled guards, submenu navigation, dismissal, and release-only actions. Keep toolbar controls and native macOS menus unchanged.

## Context menus

All application and independent plugin editor context menus use `menus::ContextMenu`. This component wraps egui's context popup with `popup_style` and `layout`, including the minimum content width, regular text, shared padding and gaps, and full-row rounded highlights. It retains the response's default popup identity, pointer anchor, focus, and dismissal, so existing `Response::context_menu_opened` checks remain valid. Track panels retain their compound-control opening guards through `ContextMenu::open_memory`.

Use the component for clip, MIDI note, track and empty-track areas; effect and instrument headers; oscillator, Macro, and modulation strips; EQ band removal; and plugin appearance menus. Use `menus::separator` for their action groups. Shared clip and track actions use `menus::action_button` to retain a visible gap before right-aligned shortcuts, including when long labels expand the popup. Keep action dispatch, disabled guards, selected targets, note fields, menu ordering, project edits, plugin parameters, and appearance storage in their existing owners. Opening or dismissing a menu must not change project data or send host parameter edits.

## Catalog panels

Effects, Components and Chords share `Catalog` and `ControlPanel::catalog_body` / `catalog_row`. The [Instrument track browser](instrument-tracks.md) reuses the same body and row painter with dynamic track UUIDs, track icons and nested group disclosures; project navigation stays separate from library selection:

- Reuse `menus::ITEM_PADDING`, `OUTER_MARGIN` and `ITEM_SPACING`: 4-point row padding, outer list margins and gaps between visible rows.
- Apply outer margins once around the scrollable list. Selected row highlights fit inside these margins; keep the fixed header and panel border unchanged.
- Keep the existing Small text, category indentation, disclosure/folder icons, selected fill, hover text colors and rounded highlights. Center labels and icons vertically; reserve the icon height plus inner padding for every row.
- Keep form fields outside the catalog helper. Their existing row height and baseline rules remain independent.

Stable row IDs, selection, keyboard navigation, visible folder order, scrolling, drag previews and release-only edits retain the [catalog contract](../editing/components.md#instrument-catalog-folders). Spacing changes must not create units or project edits.

## Acceptance

Check both egui base themes and retain context-local palette and accent colors. Cover regular application popup text inside Small control groups, Small unit popup text at every submenu level, long labels, minimum/full row widths, stable hover over text and padding, disabled entries, 4-point item gaps, unpadded dividers, and catalog outer margins and selection geometry. Retain existing source grouping/move tests and catalog keyboard, folder, drag, scroll and guard tests. Automated egui checks do not replace native pointer or platform checks.
