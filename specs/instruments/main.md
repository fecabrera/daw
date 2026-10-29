# Instrument Output bus

Implementation requirements and design records. For user instructions, see the [user guide](../../docs/instruments/main.md).

Output (formerly displayed as Main) is the fixed output bus of one MIDI instrument. It receives the stereo sum of all regular layers. It is separate from the project Master.

Processing order:

`Layer sources → layer Amplitude → layer effects → layer gain/pan → stereo layer sum → Output effects → Output gain/pan → track effects → track gain/pan → project mix`

Output also hosts shared Envelope, LFO, and Macro components. These are modulation sources; adding them does not change audio or create connections. Use the existing destination strips on layer Amplitude, Auto-filter, or Drive to connect them.

## Layout and interaction

Output stays at the bottom of the Instrument workspace, above the track effects lane when that lane is visible. Only regular layers scroll vertically. Output has the shared Layers column width. Its shared `icons::header_icon` Merge artwork, rotated 90° clockwise with its arrow pointing right, and name occupy the first row inside that left panel. Only the compact gain knob occupies the second row; Output has no pan knob or numeric fields. Components have an independent horizontal scrollbar to its right. Output itself cannot be removed, reordered, or renamed. Regular layer reorder gestures stay inside the upper viewport.

Single-click selects a component with the shared effect-unit outline. Shift-click toggles additional units on Output. With its component lane focused, Delete or Backspace removes the selected units together and retains their settings for catalog restoration. Drag a selected header to move or copy that group within Output or to a regular layer; incompatible groups are rejected as a whole. Drag component headers to reorder them on Output or place transferred units at the shared insertion line, including when Output is empty. Output saves this presentation order independently of regular layers; it does not alter processing order. See [component selection and reordering](../editing/components.md#selecting-and-reordering-component-units).

Output starts compact at the standard 68-point track-row height. Its right-toolbar button expands it to the shared 244-point unit height or collapses it again. Compact mode retains component headers and footers. Unit power and horizontal collapse remain independent of the row's compact state. All these states save with the project. Layer and Output selection outlines meet all four edges without an inset and paint after neutral dividers and row fills, so no side is covered. Output owns the separator above its row and shares the preceding layer's bottom pixel. Its component shells omit their separate top edge and share their bottom separator with the adjoining effects lane or status panel. Header fills cover the reserved top frame space so no background gap appears; control positions and unit heights stay fixed. The fixed Layers header separator paints after scrolling rows.

Output has its own right toolbar inside its lane, outside the horizontal component scrollbar. It reuses `lane_toolbar::bounds` and `show`: a 38-point rail, top-aligned 24-point borderless button, 16-point icon and seven-point inset, with the effects lane’s background, dividers and interaction behavior. Component viewports, unit insertion and catalog drop bounds stop at the toolbar’s left edge. Both regular layer and Output buttons stay at the top when expanded or collapsed. The shared toolbar respects `SHARED_TOP_BORDER_TAG`: Output’s row owner alone paints its top separator on the preceding scroll region’s bottom pixel; the toolbar must not add a second line inside the row. Regular layers use a down arrow to expand and an up arrow to collapse. Output reverses these directions: up to expand above its pinned position, down to collapse.

Click Output to make it the component catalog target. Double-click or Enter adds selected Envelope, LFO, and Macro entries through the same actions used by regular layers. Drag components onto Output's label or body to target it directly, including when Output is empty or compact. Catalog additions, drag eligibility, and unsupported-entry tooltips use the same [component registration](../editing/components.md#registering-instrument-components). Registering an Output installation function automatically permits that component's catalog drops; there is no separate drop allowlist. Selected groups retain catalog order. Output ignores Oscillator and Amplitude entries; those can still be dragged onto regular layers. Drops containing only unsupported entries show an invalid target and do not edit the project. Jobs, dialogs, focus loss, disabled editing, Escape, and invalidated snapshots use the existing catalog guards and cancellation rules.

Each added Envelope is independent and starts in Legato mode. Output offers only Retrigger and Legato because its output is shared. LFO offers its existing Transport, Retrigger, and Legato choices; all remain shared. Removing components retains their settings and identities. Re-adding restores the first removed instance of that type. No placeholder cards appear for missing components.

## Controls and modulation

Gain reuses the regular layer compact knob, shared tooltip, adjustment/reset behavior, and DSP coefficients. It defaults to 0 dB and spans silence through +12 dB. Removing the pan knob does not remove or reset the saved pan setting: retain validation, serialization, processing, and existing −1 to +1 bounds for project compatibility. Pan defaults to center. Center preserves unity in each stereo channel. Live changes ramp over 5 ms while sources, notes, envelopes, and LFO phases continue. Output has no independent mute or solo control.

The existing [shared modulation contract](../modulation/contract.md) discovers sources as `Output · Envelope N` and `Output · LFO N`, using Output's stable owner ID and each component's stable unit ID. They can feed compatible destinations anywhere in the same instrument, including its track effects. There is no source clock or route evaluator specific to Output. The host adds these instances to the same prepared shared controls, publishes them once per sample, and fans out cached values. Existing envelope release ownership and continuous-source lifetime rules remain in effect. Gain/pan are ordinary mix controls, not new modulation destinations.

## Saved data and compatibility

The user-facing name is `InstrumentBusSettings::DISPLAY_NAME` (`Output`); shared source discovery uses the same name for Envelope, LFO, and Macro prefixes. Stable IDs and the saved `main` field remain unchanged; no migration or audio processing change is required.

Instrument version 3 stores `main` separately from `layers`. Output contains its owner ID, gain, pan, compact state, component order, effects, and Envelope/LFO/Macro settings. Missing effects default to an empty chain. Missing component order uses the legacy layout. Missing `main` in older projects creates a neutral, compact bus with no components; original layer settings and sound remain unchanged. Saving records its new identity. Explicit null or malformed data fails instead of selecting legacy defaults. Validation rejects duplicate owner/source IDs, per-note Output envelopes, invalid gain/pan, and invalid component settings. Existing core validation checks all references and applies instrument edits atomically.

Save/reopen, Save As, track duplication, view/track switches, sample-rate changes, and normal layer reordering retain Output settings and source references. Output does not own notes or audio clips and does not change arranged/export duration. Live playback and export use the same post-layer bus processing. Storage and DSP preparation occur before publication; sampling and compatible retargeting remain allocation-free.

See [validation records](../validation/README.md) for completed automated checks and remaining native UI, device, and platform checks.

Output also accepts an inline effect chain after its shared components. It processes the combined layer output before Output gain/pan. Catalog additions target Output when its row is selected; click the bottom effects lane to target track effects. Effects can move or copy between Output and regular layers with the shared group drag, preview, and modifier controls. Save/open retains all chains and routes. See [layer effects](../instruments/layer-effects.md).
