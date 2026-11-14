# Tooltip acceptance and source inventory

The [canonical tooltip contract](../views/tooltips.md) defines presentation. This
file records coverage; it does not repeat format rules. The [user guide](../../docs/views/tooltips.md)
contains operating instructions. Executed results belong in the
[validation record](../validation/README.md).

## Producer and caller closure

The implementation audited explicit, disabled, rich and native producers plus
callers of icons, ControlPanel headers/catalogs, shared power/Solo/reorder helpers,
Knob wrappers, Dropdown, numeric/transport fields and elided identities. Shared
parameter renderers inherit the canonical formatter. Caller-added explanatory
hover was removed. A static scan of recognized icon/power helper arguments found
no remaining instructional or multiline names. Native/OS-owned caption hints
remain outside first-party content ownership.

The ignored implementation ledger `work/tooltips-implementation-20261114/inventory-final.json`
records each matched file, disposition, current explicit expressions and hashes.
Lexical matches include tests and comments; they are not a runtime widget count.
Implicit identity elision and inherited helpers were checked separately.

| Source | Disposition |
| --- | --- |
| [apps/desktop/src/macos_menu.rs](../../apps/desktop/src/macos_menu.rs) | Updated |
| [apps/desktop/src/windows_titlebar.rs](../../apps/desktop/src/windows_titlebar.rs) | OS-owned caption hints; no first-party producer |
| [crates/filter-editor/src/hover_parity_tests.rs](../../crates/filter-editor/src/hover_parity_tests.rs) | Regression coverage |
| [crates/multiband-compressor-editor/src/body.rs](../../crates/multiband-compressor-editor/src/body.rs) | Updated |
| [crates/multiband-eq-editor/src/body.rs](../../crates/multiband-eq-editor/src/body.rs) | Updated |
| [crates/ui/src/audio_preparation.rs](../../crates/ui/src/audio_preparation.rs) | Updated |
| [crates/ui/src/audio_sidechain.rs](../../crates/ui/src/audio_sidechain.rs) | Updated |
| [crates/ui/src/beat_editor.rs](../../crates/ui/src/beat_editor.rs) | Updated |
| [crates/ui/src/beat_editor/timeline.rs](../../crates/ui/src/beat_editor/timeline.rs) | Updated |
| [crates/ui/src/catalog.rs](../../crates/ui/src/catalog.rs) | Updated |
| [crates/ui/src/chords_catalog.rs](../../crates/ui/src/chords_catalog.rs) | Updated |
| [crates/ui/src/clip_fades.rs](../../crates/ui/src/clip_fades.rs) | Updated |
| [crates/ui/src/clip_lane.rs](../../crates/ui/src/clip_lane.rs) | Updated |
| [crates/ui/src/clip_menu.rs](../../crates/ui/src/clip_menu.rs) | Updated |
| [crates/ui/src/clip_settings.rs](../../crates/ui/src/clip_settings.rs) | Updated |
| [crates/ui/src/diagnostic_details.rs](../../crates/ui/src/diagnostic_details.rs) | Updated |
| [crates/ui/src/editor_transients.rs](../../crates/ui/src/editor_transients.rs) | Updated |
| [crates/ui/src/editor_viewport.rs](../../crates/ui/src/editor_viewport.rs) | Updated |
| [crates/ui/src/envelope_panel.rs](../../crates/ui/src/envelope_panel.rs) | Updated |
| [crates/ui/src/exciter_panel.rs](../../crates/ui/src/exciter_panel.rs) | Updated |
| [crates/ui/src/faders.rs](../../crates/ui/src/faders.rs) | Updated |
| [crates/ui/src/icons.rs](../../crates/ui/src/icons.rs) | Updated |
| [crates/ui/src/instrument_catalog.rs](../../crates/ui/src/instrument_catalog.rs) | Updated |
| [crates/ui/src/instrument_component_drag.rs](../../crates/ui/src/instrument_component_drag.rs) | Updated |
| [crates/ui/src/instrument_main.rs](../../crates/ui/src/instrument_main.rs) | Already concise or inherited renderer |
| [crates/ui/src/instrument_panel.rs](../../crates/ui/src/instrument_panel.rs) | Updated |
| [crates/ui/src/lfo_panel.rs](../../crates/ui/src/lfo_panel.rs) | Updated |
| [crates/ui/src/lib.rs](../../crates/ui/src/lib.rs) | Updated |
| [crates/ui/src/macro_panel.rs](../../crates/ui/src/macro_panel.rs) | Updated |
| [crates/ui/src/meters.rs](../../crates/ui/src/meters.rs) | Updated |
| [crates/ui/src/midi_editor.rs](../../crates/ui/src/midi_editor.rs) | Updated |
| [crates/ui/src/midi_editor_panel.rs](../../crates/ui/src/midi_editor_panel.rs) | Updated |
| [crates/ui/src/mix_controls.rs](../../crates/ui/src/mix_controls.rs) | Already concise or inherited renderer |
| [crates/ui/src/mixer.rs](../../crates/ui/src/mixer.rs) | Updated |
| [crates/ui/src/modulation_panel.rs](../../crates/ui/src/modulation_panel.rs) | Updated |
| [crates/ui/src/multiband_compressor_panel.rs](../../crates/ui/src/multiband_compressor_panel.rs) | Updated |
| [crates/ui/src/multiband_eq_panel.rs](../../crates/ui/src/multiband_eq_panel.rs) | Updated |
| [crates/ui/src/noise_panel.rs](../../crates/ui/src/noise_panel.rs) | Updated |
| [crates/ui/src/oscillator_controls.rs](../../crates/ui/src/oscillator_controls.rs) | Updated |
| [crates/ui/src/oscillator_preview.rs](../../crates/ui/src/oscillator_preview.rs) | Updated |
| [crates/ui/src/panels.rs](../../crates/ui/src/panels.rs) | Updated |
| [crates/ui/src/sampler_panel.rs](../../crates/ui/src/sampler_panel.rs) | Updated |
| [crates/ui/src/settings.rs](../../crates/ui/src/settings.rs) | Updated |
| [crates/ui/src/time_signature_control.rs](../../crates/ui/src/time_signature_control.rs) | Updated |
| [crates/ui/src/titlebar.rs](../../crates/ui/src/titlebar.rs) | Already concise or inherited renderer |
| [crates/ui/src/toolbars.rs](../../crates/ui/src/toolbars.rs) | Updated |
| [crates/ui/src/track_sends.rs](../../crates/ui/src/track_sends.rs) | Updated |
| [crates/ui/src/transport_controls.rs](../../crates/ui/src/transport_controls.rs) | Updated |
| [crates/unit-ui/src/ambient_reverb.rs](../../crates/unit-ui/src/ambient_reverb.rs) | Already concise or inherited renderer |
| [crates/unit-ui/src/auto_filter.rs](../../crates/unit-ui/src/auto_filter.rs) | Updated |
| [crates/unit-ui/src/compressor_curve.rs](../../crates/unit-ui/src/compressor_curve.rs) | Updated |
| [crates/unit-ui/src/compressor_graph.rs](../../crates/unit-ui/src/compressor_graph.rs) | Updated |
| [crates/unit-ui/src/direction_mixer.rs](../../crates/unit-ui/src/direction_mixer.rs) | Updated |
| [crates/unit-ui/src/drive_v2.rs](../../crates/unit-ui/src/drive_v2.rs) | Updated |
| [crates/unit-ui/src/eq_graph.rs](../../crates/unit-ui/src/eq_graph.rs) | Updated |
| [crates/unit-ui/src/equalizer.rs](../../crates/unit-ui/src/equalizer.rs) | Updated |
| [crates/unit-ui/src/exciter.rs](../../crates/unit-ui/src/exciter.rs) | Updated |
| [crates/unit-ui/src/harmonic_lift.rs](../../crates/unit-ui/src/harmonic_lift.rs) | Updated |
| [crates/unit-ui/src/knobs.rs](../../crates/unit-ui/src/knobs.rs) | Updated |
| [crates/unit-ui/src/layout.rs](../../crates/unit-ui/src/layout.rs) | Already concise or inherited renderer |
| [crates/unit-ui/src/lfo_controls.rs](../../crates/unit-ui/src/lfo_controls.rs) | Updated |
| [crates/unit-ui/src/lib.rs](../../crates/unit-ui/src/lib.rs) | Updated |
| [crates/unit-ui/src/limiter.rs](../../crates/unit-ui/src/limiter.rs) | Updated |
| [crates/unit-ui/src/loudness.rs](../../crates/unit-ui/src/loudness.rs) | Updated |
| [crates/unit-ui/src/menus.rs](../../crates/unit-ui/src/menus.rs) | Updated |
| [crates/unit-ui/src/meters.rs](../../crates/unit-ui/src/meters.rs) | Updated |
| [crates/unit-ui/src/multiband_compressor.rs](../../crates/unit-ui/src/multiband_compressor.rs) | Updated |
| [crates/unit-ui/src/multiband_eq.rs](../../crates/unit-ui/src/multiband_eq.rs) | Updated |
| [crates/unit-ui/src/ott.rs](../../crates/unit-ui/src/ott.rs) | Updated |
| [crates/unit-ui/src/plate_reverb.rs](../../crates/unit-ui/src/plate_reverb.rs) | Already concise or inherited renderer |
| [crates/unit-ui/src/reverb_controls.rs](../../crates/unit-ui/src/reverb_controls.rs) | Updated |
| [crates/unit-ui/src/rows.rs](../../crates/unit-ui/src/rows.rs) | Updated |
| [crates/unit-ui/src/shimmer_reverb.rs](../../crates/unit-ui/src/shimmer_reverb.rs) | Updated |
| [crates/unit-ui/src/stereo_spread.rs](../../crates/unit-ui/src/stereo_spread.rs) | Updated |
| [crates/unit-ui/src/strip_reorder.rs](../../crates/unit-ui/src/strip_reorder.rs) | Updated |
| [crates/unit-ui/src/tooltips.rs](../../crates/unit-ui/src/tooltips.rs) | Updated |

## Regression coverage

- Shared Knob hover/drag lifecycle, long values, focus/disabled guards and current
  values retain existing checks. Contextual send hover independently checks the
  unchanged accessible parameter identity and keyboard editing.
- Shared editor hover tests exercise all 20 effect editor crates through their
  registered controls, three themes and two display scales. They compare actual
  hover text with the shared body, require visible hover coverage, and reject
  host gestures or settings changes. Existing idle mesh/gesture tests remain.
- Modulation tests retain correlated-route, inactive, numeric-range, silence and
  live-value checks while asserting action-only marker hover.
- Details checks cover complete retained errors, full memory report content,
  explicit activation, Close/Escape, existing-modal/unfocused guards, unchanged
  project/dirty state and no worker creation.
- Existing dropdown, meter and transport tests retain geometry, input, resets,
  telemetry, shortcuts and dialog/job guards.

## Native acceptance limits

Manually inspect app views, dialogs/settings/status, elided names and source
paths, graphs and nested selectors on macOS, Windows and Linux. Check native
menu hover, screen-reader identities, themes/scales, dragging/focus transitions
and actual CLAP/VST3 host windows. Headless tests and successful builds do not
establish these checks. Do not claim listening or device acceptance from a text
change.
