# Instrument and Editor project track browser

## Scope and composition

Instrument adds a List Tree **Tracks** toggle above Instrument components and Effects in the existing left toolbar. It starts closed. Track navigation, components and effects share one 250-point `ControlPanel`; opening one closes the others in Instrument. Editor reuses the same Tracks toggle and browser; Tracks, Effects and Chords share its catalog space. Browser visibility starts closed and is independent in Instrument and Editor. Tree expansion and navigation use the shared state. Other views keep their existing toolbar controls. The shared workspace retains its single top border and bottom effects lane.

[`track_browser.rs`](../../crates/ui/src/track_browser.rs) owns dynamic tree rows, browser-only expansion, cursor and focus. Reuse `ControlPanel::catalog_body` and the common catalog row painter for small text, rounded selected fill, row padding/margins, four-point gaps and keyboard focus appearance. Root audio/MIDI rows use the existing Audio Lines/Keyboard Music icons. Group and beat-parent rows use the shared Chevron Right/Down disclosure without a folder icon. Each nesting level adds one icon slot of label indentation; rows keep full shared hit/selection bounds. Keep dynamic names and stable project/track UUIDs separate from static catalog registration.

## Navigation and state

Read all project tracks in validated parent-before-child order, including unused/empty tracks and groups. Derive visible rows from browser expansion, independently of saved `group_collapsed`. Preserve sibling order and arbitrary nested groups. Groups start closed; beat parents start open; retain choices through panel/view toggles. Reconcile deleted group IDs and clear expansion/cursor/focus when the project identity changes. Do not serialize navigation or create history edits.

Clicking a group or beat-parent chevron toggles its children without changing selection or saved collapse state. Clicking the name selects that project track. Clicking an audio/MIDI row uses shared single-track selection, leaves the view and clip selection unchanged, and retires stale layer/component gestures and targets. MIDI rows show that track's existing instrument. Audio rows keep the existing no-MIDI-instrument presentation and use their shared track effects lane. This extends the historical MIDI-only Instrument effects target; the top MIDI dropdown retains its current filter. Reuse the effects workspace's same-frame target refresh before painting the bottom chain.

In Editor, select the track and its earliest clip through the shared editor-selection helper. An empty track clears clip selection. Highlight the active editor track, including when a retained clip belongs to a different track than the Instrument selection. Group disclosures do not change the editing context. Keep the Editor view and project data unchanged. Its options-panel track header reuses Timeline's `track_name` label and icon, including name editing, truncation and one/two-row geometry, instead of a track dropdown.

Use Up/Down for bounded visible-row navigation, selecting track rows and focusing group rows. Enter selects tracks or toggles groups. Right opens a group; Left closes it or focuses the parent of a child. Retain egui arrow focus filtering through the shared `selection_focus` helper, as catalogs do. Scroll keyboard-focused rows into view. Keyboard handling requires list focus, no text edit, a focused/enabled application, settled gestures and an idle job/dialog path. Track selection and disclosure are disabled under the existing workspace guards. The header Plus menu and blank-panel context menu share `track_creation_menu` for audio, MIDI, beat and empty-group creation in both Instrument and Editor. `TrackAction::AddGroup` appends a validated empty root through `Project::add_group_track`, preserving existing rows and membership. Editor creation uses the shared editor-selection helper to clear the prior clip and show the new empty track. Only the Plus wrapper `add_track_menu` adds Add subtrack for the selected/displayed beat parent; blank-panel menus omit it. Group-row menus use the four creation choices to add inside the clicked group through one staged core change, then expand its browser children and select the new child. Separate Add beat track from Add group through the shared `starts_menu_group` boundary in popup and native menus. `TrackAction::ALL` exposes Add group and Add subtrack in native and egui Track menubars. The latter stays disabled without a beat target. Beat-row context menus include Rename and Add subtrack; capture the clicked parent independently of current/multiple selection. Timeline and Mixer context menus retain explicit beat/group targets, including multi-selection where another track is primary. Opening or dismissing any menu remains clean; commit creation only on release. New subtracks select their real parent and expand its browser children; Editor retains an active clip from that parent. Project-track and group names reuse the shared track-name input on double-click or the full-row Rename menu. Subtrack names reuse their existing inline rename and full-row context menu. All inputs select the full Unicode name, commit on Enter, and cancel on Escape/focus loss or view/panel/project changes. Keep the browser-owned input separate from Editor’s track-header input so only one editor handles the draft. Clamp input indentation to retain usable width in deeply nested groups; never overlap the pinned controls. The browser owns no audio processing.

## Compact output controls and beat navigation

Keep the existing catalog row height, Small name text, margins and four-point row gaps. Reserve a fixed right-aligned control area for Mute, Solo, Pan and Gain. Mute and Solo use 16-point square buttons with 9-point labels; Pan and Gain use the shared knobs scaled to 16 points and four-point gaps. Keep paint clipping in viewport coordinates when scaling knob faces. Names truncate before the controls. Control presses must not select rows, disclose children or arm reorder gestures. Project rows edit their existing mix stage; subtracks use `BeatEdit::SetMix` and their canonical Main bus. Do not add a gain stage.

Show expanded beat subtracks in saved order, indented under the parent. Keep `selected_track` on the real project parent and use stable subtrack IDs for the active instrument. Selecting a child opens the standard MIDI Layers body, including its Layers header/Plus, local providers and pinned Output. Selecting the parent shows only the final beat Output. No simultaneous nested Instrument groups remain. In Editor, selecting another child of the open beat parent retains the active clip and step grid. Disclosure retains the current owner and does not edit the project.

## Reordering and group membership

Arm name/icon/background drags at the initial primary press; preserve control and rename gestures. Use the shared movement threshold, focus/cancellation primitives and edge scrolling. Keep a normal press armed only in browser state; activate the global track drag after crossing the movement threshold. A selection click must not flash disabled workspace controls. Armed gestures still guard selection/edit shortcuts; leaving the view, hiding the panel or replacing the project cancels them. Preview insertion during the drag and commit only on release. Project rows use `Project::track_subtree`, `can_move_track` and `Edit::MoveTrack`; preserve IDs and entire group subtrees. The middle half of a group row targets its child list, including collapsed groups. Row edges target before/after the subtree; the left gutter selects an outer parent. A release below the final row returns to Master. Reject cycles and invalid destinations.

Subtrack gestures reuse beat multi-selection, group insertion and `BeatEdit::Reorder`. Limit the allowed release area to the original parent's visible subtrack rows, intersected with the viewport. Reject another beat parent, project group, Master or outside drop. Cancel on Escape, focus loss, disabled editing, revision changes or lost gesture ownership. Same-position releases and selection/disclosure stay clean. Do not construct or publish project edits before release.

Only primary-button release completes a primary reorder. Secondary and middle
button releases retain its preview and leave project data and history unchanged.

Record the originating pane and view for each beat reorder. When Tracks and the
step editor display the same parent, only the originating pane paints the preview,
scrolls and completes or cancels its release. Another pane's outside bounds cannot
clear that drag. Shared Escape, focus and revision guards still cancel it.
The Tracks scroll-area prepass must also check the reorder origin. Editor
reorders leave the Tracks scroll position unchanged. Tracks-origin beat reorders
and ordinary track drags retain edge scrolling.
Closing Tracks cancels its own drag. View switching retains the active-gesture
guard and clears beat reorder state when a view change is accepted.

## Acceptance

[`track_browser/tests.rs`](../../crates/ui/src/track_browser/tests.rs) covers saved order, all track kinds, nested groups, independent saved collapse state, toolbar/panel switching, same-frame MIDI/audio effects targets, unchanged project/clip state, bounded keyboard navigation, rename/removal, disabled input and shared row geometry in Dark/Light/Mist at 1×/1.5× scale. Editor cases cover first-clip selection, empty audio tracks, keyboard switching, catalog exclusivity and per-view visibility. Additional cases cover canonical output controls, fixed row size/right alignment and unclipped knob paint in every theme at 1×/1.5×/2×, selected-subtrack Layers parity, parent-only Output, release-only project/group moves and subtrack reorder, rejected cross-beat drops, and cancellation. Existing catalog paint/focus and workspace suites remain required. Native app/device and Windows/Linux checks are separate from automated egui tests; executed results belong in [validation history](../validation/README.md).

## Panel action button presentation

Tracks-browser Add, Layers Add, Editor Beat-subtracks Add, and Timeline Tracks
Add/Duplicate use `icons::panel_button` and the shared `rows::panel_button_style`.
Reuse the M/S idle unit-content fill, unit-header hover fill and existing active
colors. Use 5-point corners and explicit empty strokes in every interaction state,
without an internal divider. Preserve existing button minimum size (28 by 22
points), icon artwork/size, padding, panel geometry, tooltips, enabled guards,
popup anchoring and release-only actions. Explicitly borderless icon buttons keep their existing
styles; framed buttons follow the shared theme. M/S also uses this shared surface style while retaining its paired outer
corners, separator, text and dimensions.

Mixer + Send buttons and the Instrument view's top track selector reuse the same
panel surface style, with no divider. Preserve their previous sizes and spacing.
For + Send, retain its existing button padding while drawing an empty stroke
and 5-point corners. Keep its action popup outside the styled
button scope. The Instrument selector retains canonical `menus::Dropdown`, its
arrow, width, text and guards; restore the original visuals and canonical popup
style inside the menu so only its collapsed surface changes. All framed dropdowns and send-row destination groups now follow the shared
[default control surface](dropdowns.md#default-framed-control-surfaces).

## Layer header typography

Regular layer names and the instrument-wide LFO, Envelope and Output header
labels use `fonts::semibold(rows::NAME_FONT_SIZE)`, matching track-header
names at 13 points. Preserve existing text colors, truncation, header placement,
rename interactions, selection and drag behavior.
