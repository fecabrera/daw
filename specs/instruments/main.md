# Instrument Output bus

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/instruments/main.md).

Output (formerly displayed as Main) is the fixed output bus of one MIDI instrument. It receives the stereo sum of all regular layers. It is separate from the project Master.

Processing order:

`Layer sources → layer Amplitude → layer effects → layer gain/pan → stereo layer sum → Output gain/pan → track effects → track gain/pan → project mix`

Output hosts shared Macro controls. Envelope and LFO components belong to the fixed [instrument-wide modulation lanes](modulation-lanes.md). These are modulation sources; adding them does not change audio or create connections. Use the existing destination strips on layer Amplitude, Auto-filter, or Drive to connect them.

## Layout and interaction

In Instrument, a MIDI track or beat subtrack shows its Output controls on the Macros row in Modulation, below LFO and Envelope. The Effects tab shows the regular layer chains. The beat parent uses the standard empty Layers pane. Its saved Output gain, pan, and Macro settings stay in the processing graph. The bottom Effects Lane shows the selected track or beat subtrack chain.

Output uses the shared Layers column width. Its Merge icon and name occupy the first header row. The compact gain knob and shared stereo meter occupy the second row. A beat subtrack also exposes its canonical mute, solo and pan there. The rails span the 54-point inner header height and reuse the [layer meter contract](layer-effects.md#layer-bus-level-meters). The Output meter measures before the track or subtrack effect chain. Output stays fixed while regular layers can be reordered within the shared viewport.

Click a Macro component to select it. With its lane focused, Delete or Backspace removes selected components and retains their settings for catalog restoration. Use the existing component drag and copy gestures for compatible destinations. See [component selection and reordering](../editing/components.md#selecting-and-reordering-component-units).

New Output rows start expanded at the shared 244-point unit height. The right-toolbar button toggles between expanded and the standard 68-point compact height. Compact mode retains component headers and footers. Unit power and horizontal collapse remain independent of the row's compact state. All these states save with the project. Layer and Output selection outlines meet all four edges without an inset and paint after neutral dividers and row fills, so no side is covered. Output owns the separator above its row and shares the preceding layer's bottom pixel. Its component shells use the shared row border and omit their own bottom border. Header fills cover the reserved top frame space so no background gap appears; control positions and unit heights stay fixed. The fixed Layers header separator paints after scrolling rows.

Output has its own right toolbar inside its lane, outside the horizontal component scrollbar. It reuses `lane_toolbar::bounds` and `show`: a 38-point rail, top-aligned 24-point borderless button, 16-point icon and seven-point inset, with the effects lane’s background, dividers and interaction behavior. Component viewports, unit insertion and catalog drop bounds stop at the toolbar’s left edge. Both regular layer and Output buttons stay at the top when expanded or collapsed. The shared toolbar respects `SHARED_TOP_BORDER_TAG`: Output’s row owner paints one top separator on the shared row boundary. Output and regular layers use a down arrow to expand and an up arrow to collapse.

Click the Macros row in Modulation to make Output the component catalog target. Double-click or Enter adds selected Macro entries through the same actions used by regular layers. Drag components onto Output's label or body to target it directly, including when Output is empty or compact. Catalog additions, drag eligibility, and unsupported-entry tooltips use the same [component registration](../editing/components.md#registering-instrument-components). Registering an Output installation function automatically permits that component's catalog drops; there is no separate drop allowlist. Selected groups retain catalog order. Output ignores Oscillator, Amplitude, Envelope and LFO entries; those can still be dragged onto regular layers. Drops containing only unsupported entries show an invalid target and do not edit the project. Jobs, dialogs, focus loss, disabled editing, Escape, and invalidated snapshots use the existing catalog guards and cancellation rules.

The Output row accepts Macro components. Removing a Macro retains its settings and identities; re-adding restores its retained strips. Envelope and LFO catalog additions target their dedicated lanes, which preserve the original modes and assignments during older-project migration. No placeholder cards appear for missing components.

## Controls and modulation

Gain reuses the regular layer compact knob, shared tooltip, adjustment/reset behavior, and DSP coefficients. It defaults to 0 dB and spans silence through +12 dB. Removing the pan knob does not remove or reset the saved pan setting: retain validation, serialization, processing, and existing −1 to +1 bounds for project compatibility. Pan defaults to center. Center preserves unity in each stereo channel. Live changes ramp over 5 ms while sources, notes, envelopes, and LFO phases continue. Output has no independent mute or solo control.

The existing [shared modulation contract](../modulation/contract.md) discovers Macro sources with the `Output` prefix, using Output's stable owner ID and each component's stable unit ID. They can feed compatible destinations anywhere in the same instrument, including its track effects. There is no source clock or route evaluator specific to Output. The host adds these instances to the same prepared shared controls, publishes them once per sample, and fans out cached values. Existing envelope release ownership and continuous-source lifetime rules remain in effect. Gain/pan are ordinary mix controls, not new modulation destinations.

## Saved data and compatibility

The user-facing name is `InstrumentBusSettings::DISPLAY_NAME` (`Output`); shared source discovery uses it for Macro prefixes. The Output bus ID and saved `main` field remain unchanged during provider-lane migration.

Instrument revision 4 retains `main` separately from `layers` and adds the fixed modulation lane owners. Output contains its owner ID, gain, pan, compact state, component order and Macro settings. The legacy effects field remains readable for project migration. Legacy Envelope/LFO fields remain readable; older project sources migrate to their dedicated lanes with all receiving references. See [migration](modulation-lanes.md#saved-data-and-migration). Missing effects default to an empty chain. Missing component order uses the legacy layout. Missing `main` in older projects creates a neutral, expanded bus with no components; original layer settings and sound remain unchanged. Saving records its new identity. Explicit null or malformed data fails instead of selecting legacy defaults. Validation rejects duplicate owner/source IDs, per-note Output envelopes, invalid gain/pan, and invalid component settings. Existing core validation checks all references and applies instrument edits atomically.

Save/reopen, Save As, track duplication, view/track switches, sample-rate changes, and normal layer reordering retain Output settings and source references. Output does not own notes or audio clips and does not change arranged/export duration. Live playback and export use the same post-layer bus processing. Storage and DSP preparation occur before publication; sampling and compatible retargeting remain allocation-free.

See [validation records](../validation/README.md) for completed automated checks and remaining native UI, device, and platform checks.

When loading a saved project, prepend any former Output effects to the selected owner’s track or beat subtrack chain in their saved order. Preserve effect settings, identities and modulation routes. See [layer effects](../instruments/layer-effects.md).
