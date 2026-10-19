# Envelope modulation inputs

For controls and tasks, see [Envelope](../../docs/instruments/envelope.md). Existing ADSR, note-relative timing, mode, and source lifecycle remain specified in [MIDI instruments](midi-patterns.md#envelope-unit).

## Model and contract

Envelope is both a Contour source and a typed modulation destination. `EnvelopeInstance` implements `ModulationDestinationSettings` with nine stable parameters in `ModulationParameter::ENVELOPE`. Saved `modulations` and `modulation_open` default to empty/closed without a version change. `EnvelopeSettings` stays Copy and contains base DSP parameters only. Validate route IDs, targets, finite signed amounts, source references, scope, and provider cycles through the shared contract.

The nine typed offsets append slots 56–64 to the common input frame; existing identities and indices stay unchanged. Attack/Decay/Release Time retain saved ±6-octave offsets, displayed as multipliers from ×1/64 to ×64 (×1 at zero). Sustain, three curve strengths, and Attack/Decay Note fractions use ±1 normalized offset. Time and Note routes retain distinct identities and remain selectable in both Timing modes. Unused timing settings stay saved and do not affect the active stage. Release always uses Time. Timing, Mode, and curve shape are not destinations.

`modulation_input_scope` defaults to the declared scope on both shared destination traits. Envelope overrides it from the saved Mode and prepared runtime scope, so the common entry point also rejects local inputs for standalone shared instances. `EnvelopeInstance::mode` supplies destination scope in the instrument graph: Per note accepts local/shared sources; Retrigger/Legato accept shared sources only. Every Envelope, LFO, and Macro provider participates in the same dependency graph. All assigned routes count, including retained, disabled, and zero-amount routes. Source pickers use the same cycle check. No provider can feed itself or create a mixed feedback loop.

## Portable DSP and host processing

`daw-envelope::Envelope` implements `ModulationDestination<Audio = ()>` and declares a modulation input beside NoteEvents. It reuses `ModulationSmoother<9>` for the 1 ms additive offsets and owns bounds, time conversion, and ADSR stage math. Base settings remain unchanged. Time scales by `2^offset` then clamps to 0–10,000 ms; normalized controls add and clamp to 0–1. Zero Time remains zero. Note timing retains its unknown-duration Time fallback.

Duration modulation retains fractional normalized stage position rather than resetting or rounding progress on every input sample. Curve and Sustain modulation update smoothed effective controls without repeatedly re-anchoring the stage. Explicit base edits retain the existing anchored-stage and 5 ms Sustain edit behavior. Idle remains idle; inputs never own note events or release independently of the contour. Reset restores neutral offsets. Unconnected/neutral processing preserves legacy samples exactly.

`Control` prepares routes and calls the common provider input/publication helper. That helper calls `process_modulation`, then samples `ModulationOutput` once. Prepared provider-first index arrays interleave shared Envelopes, LFOs, and Macros. Per-voice Envelope indices use the same dependency planner for local edges after shared values are complete. There is no callback graph traversal, custom source lookup, or route summation loop. A modulation-aware note-on defers zero-duration transitions until the first resolved input, allowing Note Attack modulation to grow from a zero base fraction.

Compatible retargeting retains the Envelope, offset smoother, source weight, route ramps, and stage progress by stable IDs. Route storage, graph order, publication buffers, and replacements are prepared outside the callback. Removed routes return to neutral through the existing smoother. Empty routes with neutral offsets retain the original source-only fast path.

## History and preparation

Uninterrupted playback, offline export, and block rendering evaluate the same history. Native transport preparation replays the project prefix when any Envelope has an assigned, enabled, nonzero input. It then primes existing latency buffers. Replay runs on the existing output preparation worker, with cancellation at buffer boundaries. It must never run in the callback. Completed preparation publishes atomically; cancelled requests publish nothing.

Low-level `Renderer::seek`, hidden note history before a trimmed clip, and direct/backward loop jumps retain the existing analytical note-state reconstruction and reset rules; they do not synthesize earlier input modulation history. Their result can differ from uninterrupted history. Use `Renderer::prepare_transport` for a prepared native seek or `render_offline_at` for exact offline history. Arbitrary callback reconstruction, unbounded callback replay, and a new graph editor are outside this change.

## UI, telemetry, and transfers

Envelope reuses the common header toggle, connection body, Source/Destination labels, amount adapters, source filters, strip reorder, and knob markers. Base and Amount knobs share destination definitions. Mode choices reject incompatible scope changes. Saved open state follows moves, copies, and save/reopen.

`ModulationTarget::Envelope { track, layer, unit }` uses the existing prepared snapshot handles. Nine pairs of native minimum/maximum readings use slots 0–17; slot 18 stays unavailable. Shared Envelopes publish one effective value per pair; local Envelopes aggregate active voices. The UI does not run another Envelope. Shape/Mode/Timing do not occupy telemetry slots.

Envelope component copies get new provider and route IDs, retaining source assignments. Provider moves remap Envelope consumers, including Main, through the same transfer helper as other destinations. Source/mode incompatibility rejects the whole transaction. No source or route identity depends on display order.

## Verification

Check neutral sample equality, every typed target, Note timing from zero, bounds, stage continuity, release completion, source scope, mixed cycles, migration/roundtrip, copied route IDs, provider moves, all three overlap modes, fan-out, block/live/export parity, cancellable worker seeks, callback allocation, UI controls/markers in both themes, and Save As/reopen audio. See [validation](../validation/README.md) for executed results and native limits.

## Time multiplier presentation — 2026-10-19

Envelope Attack/Decay/Release Time amount readouts, knob tooltips, endpoint labels, and assignment-marker tooltips display `2^amount` as a multiplier. The shared `parameter_knobs` amount definition and formatter own this presentation. Sub-unity readouts retain up to six decimal places so ×1/64 remains visible; larger ratios use up to two. Trailing zeros are removed. Native duration readouts remain in milliseconds/seconds, and Note fractions remain percentages.

This supersedes the former `oct` display only. Saved amounts, ±6 bounds, source weighting/polarity, summation, ramps, smoothing, range arcs, processed telemetry, native-scale dragging, keyboard steps, and neutral-zero reset remain unchanged. The existing shared octave adapter still performs interaction mapping; zero displays ×1. No project migration or DSP change is required. Other units retain their current amount units.
