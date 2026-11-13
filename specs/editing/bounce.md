# Clip and track bounce

Status: implemented. Native UI, device and platform acceptance is recorded
separately from automated checks in [Bounce validation](../validation/bounce.md).
User procedures belong in the [guide](../../docs/editing/bounce.md).

## Commands and transaction

`daw-core::BounceRequest` selects whole clip IDs or track IDs. `bounce_targets`
resolves Audio/MIDI/Beat roots in saved source order, reports named exclusions,
retains disabled geometric extents, and rejects stale IDs or an empty eligible
selection. Track mode ignores clip/range/loop bounds; clip mode requires whole
clips in the UI. Disabled-only targets produce warned silence. Target track
monitoring does not suppress capture; authored clip, layer, source and bypass
states remain effective. Original monitoring gates still govern external controls.

`Edit::Bounce` installs every `BounceOutput` as one validated transaction. Originals
retain identity and settings; only requested clip mute flags change. Each result
is a fresh sibling immediately after its source, with the same parent/color and
local Mute/Solo. It has fresh track/clip/asset/send IDs, a unique name, neutral
processing and one merged clip. The output keeps internal gaps and starts at the
first requested clip, without stored leading silence. Invalid or incomplete output
leaves the model unchanged. One history record covers every result and mute change.

## Rendered source capability

`Asset.rendered_audio` version 1 requires `Source.kind = rendered_audio` and project
storage. This required discriminator makes older readers reject sound-critical
content instead of silently applying another edge fade or dropping routing audio.
Ordinary legacy assets omit the field. The main file and every auxiliary leaf are
project-owned stereo float32 WAV files on the same source clock. Validation checks
version, identity, format, length and leaf dependencies. `RenderedAudio::dependencies`
enumerates the pre-send leaf and every provider's raw/Post leaves. A Post leaf may
be the main asset; raw/pre-send leaves cannot depend on another rendered bundle.

The output delegates outer envelopes with validated `inherit_source` edges.
Explicit cuts or fade edits resume the existing boundary-envelope behavior. Main
source extraction for compounds, Sampler/Granulizer, Normalize and separation does
not attach routing associations to an unrelated source consumer.

Auxiliary readers copy the main clip's edit clock and controls, substituting only
its source asset. Shared transform requests prepare every leaf for stretch/pitch
changes. Trims, splits, moves, repeats, channel edits and rate changes therefore
retain one clock. Copying a clip duplicates its contribution through the new clip
identity. Re-bounce captures and flattens earlier provider feeds; disabling the old
clips removes their old contribution.

Audio Merge retains this capability through the version 2 companion recipes
described in [Audio Merge](audio-merge.md#prepared-source-evaluation). Version 1
remains the file-bundle contract. Main extraction stays independent of auxiliary
routing. Compiled activity selects the historical Pre override only for captured
members; ordinary mixed members retain current track effects and mono/stereo pan.
Captured stereo leaves retain their baked original pan.

## Shared capture and routing

`daw-engine::BounceCapture` owns independent reference and isolated renderers.
Both use the existing schedules, TrackProcessor, MIDI/Beat source containers,
EffectChain, Buses, modulation and latency alignment. They replay from frame zero
without transport looping or live audition. The isolated target uses only the
requested excitation. It consumes the full reference graph's route-local external
control publications at the same destination stage clocks. Unrelated audio is
processed for reference context but never included in the output. Beat parents
resolve through the standard private Group/Output/lane lowering.

Capture delays each tap to the common content clock. Main PCM is post-effect,
post-pan and post-gain, before monitoring gates. Raw PCM preserves pre-sidechain
channel values; panned pre-gain PCM preserves Pre sends, including silent faders
and hard pan. Never derive these taps by inverse gain/pan. Original gain and pan
are applied exactly once. Group, Return and Master processing remains downstream.

Sends are copied with fresh IDs and their existing level/tap/power. Captured Pre
readers replace the historic pre-gain stage; current output pan remains live.
Post sends use the baked main through ordinary current track processing. Both
modes bypass Mute and obey Solo. Old-ID sidechains sum residual live audio and
active captured contributions before one follower. Pre is raw; Post applies the
contributing output's current fader/pan and monitoring gates. A new output ID is
also a normal provider. Captured provider stages are historical snapshots;
subsequent output effects do not reconstruct them.

The combined core graph includes contribution-to-destination dependencies, even
for disabled routes/clips, and rejects feedback. Missing original provider IDs
remain missing; no alias resurrects a deleted source. Prepared readers and delays
are allocated on workers, included in memory estimates and streaming preflight,
and reset without allocation. Rendering with captured sources uses the serial
preparation path. Callback sampling never decodes, performs file I/O, waits or
allocates. Renderer retirement retains the established off-callback path.

Partial nonlinear/shared processing cannot guarantee a null against the original
joint excitation: `F(selected + residual)` need not equal `F(selected) + F(residual)`.
Clip mode warns when enabled residual content remains.

## Bounds, files and ownership

`offline_tail_cap` supplies the existing ten-second ceiling to export and Bounce.
Ordinary export otherwise keeps its prior tail policy. Bounce bounds actual
note-envelope releases, including modulated release limits, prepared sample
continuations, relevant layer/Output/track effect budgets and the short gate
release. Granular activity and effects without a finite tail contract use the cap.
It never uses the voice-slot reservation maximum or an energy-based early stop.
No Group/Return/Master tail is baked. A capped result receives a baked 5 ms
terminal attenuation on every retained tap, inside the ceiling. The cap and fade
are expected behavior and do not produce a warning or notice.

After the full tail window has rendered, `bounce_tail::compact` scans its complete
suffix backwards for every main/auxiliary leaf. The per-leaf amplitude floor is
`min(peak, 1) * 1e-8`: 160 dB below a quiet signal's peak, never above -160 dBFS.
This keeps quiet sources and routing-only tails independent of main gain/pan.
All leaves use the latest significant frame plus a 5 ms guard, clamped to the
rendered end and never before the requested geometric end. Only the retained tail
receives the new terminal fade; the requested body is never faded twice. Bounded
float32 re-encoding replaces only unpublished job-owned files, then final decode
and peaks use the shortened common clock. Cancellation/failure keeps publication
atomic. This is post-render suffix compaction, not energy-based early stopping;
late echoes after quiet gaps remain eligible throughout the full window.

`daw-project::Session::bounce` serially captures bounded blocks, encodes float32,
checks finite samples and source errors, retains float32 headroom without a notice,
decodes/prepares peaks and transforms,
and verifies a complete playable renderer before returning `PreparedBounce`.
Preflight checks integer arithmetic and the stereo float WAV size limit. Shared
AudioReservation covers PCM, temporary blocks and known renderer storage; decode
uses the existing admitted resident/disk-backed representation. Exact frame-zero
prefixes remain cancellable. No fixed track count or silent rate reduction applies.

The shared generated-media directory owner is reused by separation and Bounce.
Session/history/save snapshots retain it. Failure, cancellation and dropped
unpublished results remove only job-owned files. Explicit worker cleanup reports
filesystem failures. Every main/auxiliary file participates in source ownership,
protection, relocation, collection and save receipts. Redo reuses exact retained
media. Save As includes history-only generated assets.

## UI and publication lifecycle

Clip > Bounce clips uses Command/Ctrl+B. Track > Bounce tracks uses
Shift+Command/Ctrl+B. Context menus and native macOS selectors use the same guarded
actions and metadata. Shifted B is matched and consumed first, even when
ineligible; extra modifiers/repeats are rejected. Focus, text editing, gestures,
file hover, required media work, saves and dialogs guard dispatch.

A Bounce-owned status guard is installed before dispatch. It guards every editing,
file and native-menu entry independently of `self.job`. Editing remains locked
through rendering, candidate preparation, publication or cancellation cleanup.
The worker prepares history admission too. Before acceptance the host checks
cancellation and the exact source model, excluding current transport/display.
The control-thread acceptance point serializes Cancel with publication.

With output present, the host submits the candidate through full worker transport
reconstruction while retaining the old editor Session/history. It must not inherit
original effect tails already captured in the new file. Rendered-source plans
require exact prefix history for playback preparation too. Queue pressure defers activation through the existing output
path. Completion requires the matching processing-publication receipt and current
plan acknowledgement, including after transport changes. Without output, worker
reader verification suffices; no first audible sample is required. Latest transport
intent survives model installation. Preparation errors restore the old plan/model
and clean unpublished media. Only complete publication replaces Session/history.

Running/Cancelling/Publishing/cleanup use shared `job_report::Report` visuals.
Cancel is absent after acceptance and during cleanup. Success and acknowledged
cancellation remove Bounce status automatically. Warnings become nonmodal notices.
Failures retain one Failed dialog with details/Close; generic errors do not duplicate
it. Export/separation manual-Close behavior remains unchanged. A completion-frame guard prevents terminal input from reaching underlying edits.
Held keys are then suppressed individually until release; they do not keep the
workspace disabled. Only a pointer button already held at dismissal retains the
workspace guard through its release frame. New gestures are available on the next
frame. Focus returns to the previous control.

## Acceptance

Cover selection grouping, skipped roots, exact original mute sets, stale/invalid
transactions, source edits/gaps, mono/stereo pan and headroom, track/layer/Beat DSP,
external sidechain context and latency, sends at silence, provider aggregation,
copy/re-bounce/missing identities, graph cycles, shared transformations, tail bounds,
resource failure, worker cleanup, one Undo/Redo, save/reopen and history-only Save As.
UI checks cover exact shortcuts, all guards, success/cancel dismissal after the
correct acknowledgement, retained failures and terminal input. Keep native/menu,
device, listening, Windows and Linux evidence distinct from headless checks.
