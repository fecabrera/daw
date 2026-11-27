# Instrument track column

## Scope and composition

Instrument uses the full Timeline track headers beside the instrument pages.
The header buttons and scrolling tracks share the 250-point column. Draw one
divider directly below the buttons. Use the Timeline hierarchy gutter for
expanded groups and beat tracks. Pin Master below the scrolling rows.
The left toolbar opens either Instrument components or Effects.
Keep the divider below the instrument tabs visible when no instrument is selected.
Inset the placeholder text 8 points from the top and left of the canvas.

## Navigation and state

Click a project track or subtrack to select it. A MIDI track or beat subtrack
shows its editable instrument. An audio track, group, or beat parent shows
the instrument placeholder and keeps its own bottom Effects Lane.

## Compact output controls and beat navigation

Use the shared Timeline row height and header controls. The transport
vertical slider and compact button update the same setting in both views.
A beat parent's disclosure uses the Timeline state. Selecting a beat
subtrack uses its stable ID and keeps the parent as the beat editor context.
Both views share the vertical offset of the track list. Apply it when entering
either view and clamp it to that view's scroll range. Save the shared offset in
the existing project display field.

## Reordering and group membership

Reuse track header selection, rename, and drag actions. Tracks move on
release with one shared transaction. Drop on the middle of a group to
change membership. Group and beat hierarchy use the same track identities
in Timeline and Instrument.

## Acceptance

Verify that the left toolbar shows both catalog buttons and that opening
a catalog preserves the full track column. Check the divider at the toolbar
boundary and that selecting a track targets its effects in the same frame.
See the [validation history](../validation/README.md) for executed checks.

## Panel action button presentation

The Add and Duplicate buttons in the full Tracks panel and the Add button in
the Timeline beat editor use `icons::panel_button` and the shared
`rows::panel_button_style`.
Reuse the M/S idle unit-content fill, unit-header hover fill and existing active
colors. Use 5-point corners and explicit empty strokes in every interaction state,
without an internal divider. Preserve existing button minimum size (28 by 22
points), icon artwork/size, padding, panel geometry, tooltips, enabled guards,
popup anchoring and release-only actions. Explicitly borderless icon buttons keep their existing
styles; framed buttons follow the shared theme. M/S also uses this shared surface style while retaining its paired outer
corners, separator, text and dimensions.

Mixer + Send buttons reuse the same panel surface style with no divider.
Preserve their sizes, spacing and padding. Use an empty stroke and 5-point
corners. Keep the action popup outside the styled button scope.
All framed dropdowns and send-row destination groups follow the shared
[default control surface](dropdowns.md#default-framed-control-surfaces).

## Layer header typography

Regular layer names and the instrument-wide LFO, Envelope and Output header
labels use `fonts::semibold(rows::NAME_FONT_SIZE)`, matching track-header
names at 13 points. Preserve existing text colors, truncation, header placement,
rename interactions, selection and drag behavior.
