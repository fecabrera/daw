# Drive

The Effects catalog in Timeline lists **Drive** alongside 3-band EQ. Single-click selects a catalog name; double-click appends a new independent instance to the selected audio/MIDI track or Master. Right-click an installed header to remove it. Both effect types share the catalog appearance, unit panel, header power, saved horizontal collapse, and compact lane control.

Single-click an installed effect unit to select it. Shift-click toggles its selection. A primary click in the empty effects lane clears all unit selections. Selected units reuse the selected Tracks border. Selection is UI-only, independent of track/clip and catalog selection, and does not mark the project modified. It survives view changes and reordering; removed IDs are pruned and project replacement clears it. Drag an effect header to move the selected units within its selected chain. Dragging an already selected instance retains the selection; dragging an unselected instance selects it. Show an insertion line and scroll at the lane edges. Commit the order only on release inside the lane; Escape, focus loss, a blocked editing environment, or an outside drop cancels. Preserve stable IDs, settings, enable/collapse states, and compatible DSP history. Same-position drops leave the project clean. Power and knob gestures retain their behavior, and header double-click still collapses the unit. Move selected units as a group in their existing chain order, even when selection clicks occurred in another order or units were separated. Leave unselected units in their existing order.

Drive has two 90-point strips with bottom labels **Drive** and **Color**. Drive contains Volume, Gain, and Tone, from top to bottom. Color contains Shape, Strength, and Frequency. Compact mode retains the header and both strip labels. There are no separate strip power controls. The header power bypasses the entire unit. All controls reuse the shared knob, row geometry, padding, and theme. Shape uses four standard 10-by-6-point diagrams around a cyclic knob; its tooltip and accessibility value give the full shape name. Color Off disables Strength/Frequency controls without changing their saved values.

| Control | Range | Default | Behavior |
| --- | --- | --- | --- |
| Volume | Exact silence to +12 dB | 0 dB | Output level; reuses the standard gain scale and finite silence value. |
| Gain | 0–100% | 20% | Soft-clipping intensity. Zero is clean; full gain provides approximately 36 dB of drive. |
| Tone | 0–100% | 50% | Post-clipping high shelf at 2.5 kHz; −12 to +12 dB, neutral at center. |
| Color shape | Off, Mid scoop, Mid hump, Mid hump + bass cut | Off | Pre-clipping voicing. Off leaves saturation active. |
| Color strength | 0–100% | 50% | Depth of the selected curve. Zero leaves a flat color response. |
| Color frequency | 250 Hz–2 kHz, logarithmic | 750 Hz | Mid center and linked bass-cut position. |

Mid scoop and Mid hump use a broad bell (Q 0.65) with up to −12/+12 dB depth. Mid hump + bass cut combines the hump with a low shelf at half the Color frequency, with up to −18 dB of bass attenuation. Strength scales both depths together. Frequency moves the center and linked shelf boundary. Off and zero Strength share flat voicing. Flat voicing with moderate Gain targets transparent-style drive; Mid hump + bass cut targets a tighter, mid-focused drive. This is a configurable drive, not an exact model of any named pedal.

Processing order is clip/instrument output → each serial effect → track gain/pan and audibility → Master. Within Drive: Color → symmetric normalized tanh soft clipping → Tone → Volume. Low-level input receives increasing gain as Gain rises, while the clipper approaches a smooth bounded output. Gain zero passes the colored signal directly to Tone. Tone and Volume remain active with Color Off.

`daw-unit-core` owns version-1 `daw.drive` settings. The independent `daw-drive` crate implements the portable stereo `EffectUnit`, with private FunDSP filters and independent channel/shape history. Engine state is prepared off the callback and retained by compatible track/effect identity. Every numeric control, shape selection, and header bypass ramps over 5 ms at the project rate. Frequency ramps logarithmically and Volume ramps in amplitude, including exact silence. Inactive color branches and bypassed units keep processing. Display-only edits do not restart ramps, and unrelated controls retain their in-flight ramps. Processing, retarget, and reset do not allocate, block, or perform I/O.

Save/reopen, Save As, track duplication, and processing-rate changes retain settings and both presentation states. Duplicate tracks receive new effect IDs. Invalid parameters or duplicate IDs reject the whole edit. Future Drive versions preserve opaque data and use the existing missing-effect bypass/warning. Playback, random-access warming, and offline export share the same engine. Seek and Stop reset filter state; pause retains it, and transport loops and clip gaps continue processing.

Drive adds no buffering or alignment delay. The clipper runs at the project rate without oversampling; strong drive on high-frequency material can produce aliasing. Higher project processing rates reduce this limitation. Export retains the arranged duration under the current shared effects policy. Automatic tails, beat-lane effects, and pedal-specific clipping models remain outside this change.

FunDSP and its existing transitive dependencies retain the notices in [EQ dependency licenses](../crates/equalizer/assets/licenses), which the macOS package already includes. Native audio-device listening and Windows/Linux appearance require separate checks from automated DSP and UI tests.

Master supports this unit through the same Timeline catalog and controls. Its independent saved chain processes the summed track bus before Master gain and output meters; see [Master effects](modular-audio-units.md#master-effects-implementation--2026-10-14).
