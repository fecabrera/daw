# Dropdowns and catalog list spacing

This is the canonical dropdown presentation for the DAW and independent plugin editors. It supersedes selector-specific popup styling. Collapsed selectors retain their existing size, captions, tooltips and enabled behavior.

## Shared dropdowns

Use `unit-ui::menus::Dropdown` for all selectors. It wraps egui's ComboBox, retaining its interaction, popup scrolling, dismissal and accessibility. The DAW imports the module as `menus`; portable unit panels use it directly. Keep concrete choices and edits in their existing owners.

- Use regular Button text in popup items, even inside a control group that uses Small text. Selected captions can retain compact text.
- Apply 4-point inner padding around labels and submenu arrows, 4-point outer popup margins, and 4-point vertical gaps between items.
- Give root and child popup contents a minimum width of 140 points. Longer labels expand the popup; keep labels on one line and each row's hit area and highlight at full content width. Hover must not wrap labels or resize rows.
- Use rounded, borderless row highlights and theme secondary text for inactive rows. Hover, press, selection and open submenus use the same accent-tinted fill and normal text color over the whole row, including text, padding and arrow. Disabled choices retain egui's disabled behavior.
- Use the shared 16-point Lucide ChevronRight for submenu arrows, tinted with the row text color. Retain egui's submenu hover, focus and dismissal behavior.
- Use `menus::separator`: the divider occupies only its 1-point line height. The shared 4-point item gap supplies the space above and below it; add no divider padding.

`menus::popup_style` and `menus::layout` also serve nested submenus. Scope these changes to popup contents; do not change button padding or text globally.

## Windows and Linux application menu

The burger menu and its File, Edit, Clip, Track, and View submenus reuse `menus::popup_style` and `menus::layout`. Use the same regular text, full-row rounded highlights, minimum popup width, padding, item gaps, and Lucide ChevronRight arrows as dropdown menus. Action groups use `menus::separator` without added divider padding. View choices use `menus::checked_button`, which retains the active-view checkmark inside a shared selectable button row. Preserve existing menu order, shortcuts, checked views, enabled guards, submenu navigation, dismissal, and release-only actions. Keep toolbar controls and native macOS menus unchanged.

## Catalog panels

Effects, Components and Chords share `Catalog` and `ControlPanel::catalog_body` / `catalog_row`:

- Reuse `menus::ITEM_PADDING`, `OUTER_MARGIN` and `ITEM_SPACING`: 4-point row padding, outer list margins and gaps between visible rows.
- Apply outer margins once around the scrollable list. Selected row highlights fit inside these margins; keep the fixed header and panel border unchanged.
- Keep the existing Small text, category indentation, disclosure/folder icons, selected fill, hover text colors and rounded highlights. Center labels and icons vertically; reserve the icon height plus inner padding for every row.
- Keep form fields outside the catalog helper. Their existing row height and baseline rules remain independent.

Stable row IDs, selection, keyboard navigation, visible folder order, scrolling, drag previews and release-only edits retain the [catalog contract](../editing/components.md#instrument-catalog-folders). Spacing changes must not create units or project edits.

## Acceptance

Check both egui base themes and retain context-local palette and accent colors. Cover regular popup text inside Small control groups, long labels, minimum/full row widths, stable hover over text and padding, disabled entries, 4-point item gaps, unpadded dividers, and catalog outer margins and selection geometry. Retain existing source grouping/move tests and catalog keyboard, folder, drag, scroll and guard tests. Automated egui checks do not replace native pointer or platform checks.
