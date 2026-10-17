# Instrument Main bus

Main is the fixed output bus of one MIDI instrument. It receives the stereo sum of all regular layers. It is separate from the project Master.

Processing order:

`Layer sources → layer Amplitude and gain/pan → stereo layer sum → Main gain/pan → track effects → track gain/pan → project mix`

Main also hosts shared Envelope, LFO, and Macro components. These are modulation sources; adding them does not change audio or create connections. Use the existing destination strips on layer Amplitude, Auto-filter, or Drive to connect them.

## Layout and interaction

Main stays at the bottom of the Instrument workspace, above the track effects lane when that lane is visible. Only regular layers scroll vertically. Main has the shared Layers column width. Its name and expand/collapse button occupy the first row inside that left panel; gain/pan knobs and numeric fields occupy the second row. Components have an independent horizontal scrollbar to its right. Main itself cannot be removed, reordered, or renamed. Regular layer reorder gestures stay inside the upper viewport.

Single-click selects a component with the shared effect-unit outline. Shift-click toggles additional units on Main. Drag a selected header to move or copy that group within Main or to a regular layer; incompatible groups are rejected as a whole. Drag component headers to reorder them on Main or place transferred units at the shared insertion line, including when Main is empty. Main saves this presentation order independently of regular layers; it does not alter processing order. See [component selection and reordering](component-editing.md#selecting-and-reordering-component-units).

Main starts compact at the standard 68-point track-row height. Its header button expands it to the shared 244-point unit height or collapses it again. Compact mode retains component headers and footers. Unit power and horizontal collapse remain independent of the row's compact state. All these states save with the project. Layer and Main selection outlines meet all four edges without an inset and paint after neutral dividers and row fills, so no side is covered. Main owns the separator above its row and shares the preceding layer's bottom pixel. Its component shells omit their separate top edge and share their bottom separator with the adjoining effects lane or status panel. Header fills cover the reserved top frame space so no background gap appears; control positions and unit heights stay fixed. The fixed Layers header separator paints after scrolling rows.

Layer expand/collapse icons match the small text size of the M/S buttons and retain their standard button dimensions. Regular layers use a down arrow to expand and an up arrow to collapse. Main reverses these directions: up to expand above its pinned position, down to collapse.

Click Main to make it the component catalog target. Double-click or Enter adds selected Envelope, LFO, and Macro entries through the same actions used by regular layers. Drag components onto Main's label or body to target it directly, including when Main is empty or compact. Catalog additions, drag eligibility, and unsupported-entry tooltips use the same [component registration](component-editing.md#registering-instrument-components). Registering a Main installation function automatically permits that component's catalog drops; there is no separate drop allowlist. Selected groups retain catalog order. Main ignores Oscillator and Amplitude entries; those can still be dragged onto regular layers. Drops containing only unsupported entries show an invalid target and do not edit the project. Jobs, dialogs, focus loss, disabled editing, Escape, and invalidated snapshots use the existing catalog guards and cancellation rules.

Each added Envelope is independent and starts in Legato mode. Main offers only Retrigger and Legato because its output is shared. LFO offers its existing Transport, Retrigger, and Legato choices; all remain shared. Removing components retains their settings and identities. Re-adding restores the first removed instance of that type. No placeholder cards appear for missing components.

## Controls and modulation

Gain and pan reuse the regular layer controls, numeric commit rules, and DSP coefficients. Defaults are 0 dB and center. The gain knob spans silence through +12 dB; numeric values follow existing finite-gain validation. Pan spans −1 to +1. Center preserves unity in each stereo channel. Live changes ramp over 5 ms while sources, notes, envelopes, and LFO phases continue. Main has no independent mute or solo control.

The existing [shared modulation contract](modulation.md) discovers sources as `Main · Envelope N` and `Main · LFO N`, using Main's stable owner ID and each component's stable unit ID. They can feed compatible destinations anywhere in the same instrument, including its track effects. There is no source clock or route evaluator specific to Main. The host adds these instances to the same prepared shared controls, publishes them once per sample, and fans out cached values. Existing envelope release ownership and continuous-source lifetime rules remain in effect. Gain/pan are ordinary mix controls, not new modulation destinations.

## Saved data and compatibility

Instrument version 3 stores `main` separately from `layers`. Main contains its owner ID, gain, pan, compact state, component order, and Envelope/LFO/Macro settings. Missing component order uses the legacy layout. Missing `main` in older projects creates a neutral, compact bus with no components; original layer settings and sound remain unchanged. Saving records its new identity. Explicit null or malformed data fails instead of selecting legacy defaults. Validation rejects duplicate owner/source IDs, per-note Main envelopes, invalid gain/pan, and invalid component settings. Existing core validation checks all references and applies instrument edits atomically.

Save/reopen, Save As, track duplication, view/track switches, sample-rate changes, and normal layer reordering retain Main settings and source references. Main does not own notes or audio clips and does not change arranged/export duration. Live playback and export use the same post-layer bus processing. Storage and DSP preparation occur before publication; sampling and compatible retargeting remain allocation-free.

See [validation records](validation.md) for completed automated checks and remaining native UI, device, and platform checks.
