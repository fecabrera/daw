# Basic production workflows: implementation roadmap

Status: implementation and acceptance roadmap, updated 2026-11-16. Initial
recovery, fades/crossfades, live MIDI/recording, audio sidechain modulation and
clip/track Bounce are implemented. Audio capture, automation and the remaining
feature extensions are pending. Native acceptance remains a separate release
gate; implementation status does not close it.

Existing topic contracts remain authoritative. Proposed work still needs its
listed design gates resolved before implementation. Assign calendar estimates
after device prototypes and acceptance scope are measured.

For the user-facing sequence, see [Features and roadmap](../../docs/roadmap.md).

## Goals and dependencies

Deliver autosave/recovery, editable fades/crossfades, live MIDI and MIDI
recording, audio recording/monitoring, timeline automation and audio sidechain
modulation. Prefer a usable
vertical slice at each gate over adding all controls before their engine paths.

| Stage | Deliverable | Feature status | Required predecessors |
| --- | --- | --- | --- |
| 0 | Storage, timing and interaction contracts | Recorded for delivered slices; capture/automation design gates remain open | None |
| 1 | Autosave and crash recovery | Implemented | Stage 0 storage contract |
| 2A | Editable audio clip fades | Initial scope implemented; presets remain pending | Stage 0 fade contract |
| 2B | Adjacent audio clip crossfades | Initial scope implemented; broader transitions remain pending | Stage 2A and transition-model decision |
| 3A | Live MIDI input | Implemented | Stage 0 event-clock contract |
| 3B | MIDI recording | Implemented; performance extensions remain pending | Stages 1 and 3A |
| 4A | Audio capture and monitoring prototype | Pending feature work | Stage 0 capture clock/device contract |
| 4B | Recoverable audio recording | Pending feature work | Stages 1 and 4A |
| 5A | Automation model and read playback | Pending feature work | Stage 0 parameter/timing contract |
| 5B | Automation lanes and write recording | Pending feature work | Stage 5A; reuse Stage 3B gesture capture |
| 6A | Audio detector and shared sidechain provider | Implemented | Shared modulation and audio/control dependency contract |
| 6B | Sidechain sources and mapping controls | Implemented | Stage 6A |
| 7 | Integrated native acceptance | Pending verification, not a feature implementation | All shipping slices |

Stage numbers retain the proposed user-facing sequence, not a claim that all
numbered work is unfinished or that every native gate is closed. The next
unimplemented core feature stages are 4 and 5. Stage 6 is already implemented
and does not depend on Stage 5. Device feasibility prototypes may run earlier.
Native acceptance for delivered slices remains open in parallel; required
predecessor acceptance still gates dependent releases.

## Existing foundations and ownership

Reuse [session history](../editing/undo-redo.md), the revision-aware background
save path in `crates/ui/src/regular_save.rs`, project-owned media and
[source collection](../projects/source-bundling.md), and
[background audio preparation](background-audio-preparation.md).

Keep project data and validated transactions in `core`; storage, recording
finalization and recovery snapshots in `project`; waveform/media preparation in
`media`; event scheduling, fades, monitoring mix and automation evaluation in
`engine`; native audio stream ownership in `output`; and controls in `ui`.
Keep OS permissions and native adapters in `apps/desktop`. Review whether a
separate portable MIDI adapter crate is justified during Stage 0. No engine,
project or CLI crate may acquire a UI or device dependency.

Callbacks must not allocate, block, perform file I/O or retire final owners.
Use bounded queues and prepared buffers. Writers, resampling preparation,
waveforms, serialization and file publication run on workers. Queue overflow
must have an explicit outcome; never silently drop capture data or note-offs.

## Stage 0: freeze the contracts

The delivered slices have focused contracts in [recovery](../projects/autosave-recovery.md),
[fades](../editing/fades.md), [live MIDI](../audio/live-midi.md),
[MIDI recording](../editing/midi-recording.md) and
[sidechains](../modulation/audio-sidechain-plan.md). This does not close the
capture clock/drift, monitoring/routing or automation design gates for Stages 4
and 5. Before those features proceed, complete the applicable specifications and
feasibility probes below; retain the established contracts for delivered work.
Confirm:

- Recovery snapshot identity, revision, cadence, retention, unnamed-project
  ownership and behavior after explicit Discard. Recovery must not mark manual
  Save complete or overwrite its checkpoint.
- A shared timestamp-to-project-frame mapping for MIDI events, input audio and
  control gestures. Define event order, Stop/Pause/seek/loop behavior, latency
  compensation and input/output clock drift. Keep clip positions in integer
  frames and note content in the existing musical tick representation.
- Initial armed-track count and supported device combinations. Separate-device
  input/output monitoring needs a clock-drift solution before it is advertised.
- Recording placement, collision and interrupted-take policies. Reuse the shared
  placement transaction; do not add an unvalidated recording-only overlap path.
  Define how rate/device/tempo changes and project switches are guarded mid-take.
- Fade representation, curve evaluation, behavior when fade lengths meet, and
  composition with existing 5 ms anti-click boundaries. Preserve legacy sound
  when the new fields are absent.
- A crossfade model compatible with non-overlapping logical clip ranges. Prefer
  explicit transition metadata and bounded source handles (audio beyond the
  trimmed edge). Do not weaken the overlap invariant implicitly. Specify how
  transitions survive move, trim, split, stretch, looping and source replacement.
- Audio sidechain tap points, mute/solo behavior, detector/mapping bounds,
  shared modulation scope, combined audio/control cycle validation and latency.
  See the [sidechain plan](../modulation/audio-sidechain-plan.md).
- Stable automation destination identities, units, interpolation, time basis,
  base-value precedence and combination with shared modulation. Reuse parameter
  definitions; do not identify targets by panel/list position.

Exit gate: each topic has an agreed initial scope, failure policy, migration
rules and automated/native acceptance matrix. Device/library choices require
current primary-source and license checks at implementation time.

## Stage 1: autosave and crash recovery

**Feature status: implemented.** The [recovery contract](../projects/autosave-recovery.md)
provides private generations, instance leases, startup choices and durable MIDI
takes. There is no pending initial recovery implementation in this roadmap.

Modified committed sessions produce private, versioned snapshots through the
existing worker/revision path. The previous valid generation survives until its
replacement is published. Manual Save and its clean checkpoint remain separate;
uncommitted field drafts are not recovery edits.

Startup supports Recover, Open last saved and Discard recovery, including unnamed
projects. Instance leases isolate concurrent apps. Recovery retains owned media
and history sources, leaves external sources external, and uses bounded retention
and recognized-data cleanup. See the topic contract for cadence, storage limits,
publication/acknowledgment rules and failure behavior.

**Remaining acceptance:** native process-kill and storage-failure checks, corrupt
latest-generation fallback, newer-edit/manual-save races, Discard, unnamed
projects, missing sources, instance isolation and bounded cleanup. Automated
coverage is recorded in [live MIDI/recovery validation](../validation/live-midi-2026-11-15.md);
it does not close native durability gates. Manual saves and unrelated files must
remain intact.

## Stage 2: editable fades and crossfades

**Feature status: initial scope implemented.** See the
[fade/crossfade contract](../editing/fades.md). Initial controls are available;
additional presets and transition types are separate pending feature work.

### 2A: clip fades

Audio clips save fade-in/out lengths and movable midpoint curve controls.
Timeline and the audio Editor provide dedicated handles, previews, release-only
transactions, Escape/focus-loss cancellation and group editing. Clip settings
uses the same values. The shared-boundary trim press region remains separate.
Playback/export use the shared evaluator. The topic contract governs rate/tempo
edits, copy/split/trim/loop/stretch behavior and legacy defaults.

**Pending feature extension:** additional curve presets, including dedicated
equal-power presets.

### 2B: transitions

Fixed-boundary crossfades work between adjacent audio clips with matching channel
layouts, including looped clips. Non-looped owners need source handles; looped
owners retain repeat phase. Transition extent and source bounds are explicit.
Recordings remain unchanged. Stable clip identities and atomic validation retain
transitions through supported edits; required audio prepares before publication.

**Pending feature extensions:** broader transition types, arbitrary same-track
overlapping arrangements and automatic crossfade creation require separate scope.

**Acceptance:** retain curve endpoint/midpoint tests, all project rates,
playback/export parity, source/stretch limits, missing audio, loops, rollback,
Undo/Redo, save/reopen, legacy sound and gesture-conflict coverage. Executed checks
are recorded in [validation history](../validation/README.md). Native appearance,
pointer gestures and listening for clicks, dips or excessive gain remain open.

## Stage 3: live MIDI and MIDI recording

**Feature status: implemented.** See [live MIDI](../audio/live-midi.md),
[recording](../editing/midi-recording.md), [recovery](../projects/autosave-recovery.md)
and [executed validation](../validation/live-midi-2026-11-15.md).

### 3A: live input

Machine-local device/channel selection supports multiple R-enabled MIDI targets.
With none armed, input follows the primary selected MIDI track. Timestamped
note-on/off, velocity and sustain pass through bounded engine events independently
of painting. Live input remains separate from chord/mouse audition. Stop, target
changes, focus loss and Panic use the existing release rules; overflow has an
independent panic path. Workers prepare exact Sampler pitches; unready attacks
are suppressed rather than transformed in the callback or replayed late.

### 3B: record patterns

Record, configurable count-in and arming use the shared transport actions.
Capture resolves sustain into note durations and publishes a validated whole take
through the existing preparation/history path. Whole takes use one history
transition and durable recovery checkpoints. Failure retains retryable captured
data and leaves the original project intact. Loop recording is rejected in the
current slice.

**Pending feature extensions:** loop overdub, punch, MIDI file exchange,
controller/sustain lanes, MIDI Polyphonic Expression (MPE) and MIDI clock sync.
These are not unresolved parts of the implemented initial recording slice.

**Remaining acceptance:** physical keyboard/sustain timing and latency on each
supported platform, unplug/reconnect, audio-device loss, long sessions and native
listening through layer/Main/send chains, voice stealing and prepared-pitch or
transport transitions. Native recovery fault points and storage pressure also
remain open. Retain automated event ordering, frame/tick conversion, note-boundary,
count-in, overflow, collision, history, save/reopen and recovery regressions;
synthetic events do not establish physical-device results.

## Stage 4: audio recording and monitoring

**Feature status: pending.** The saved input-device preference is a foundation;
it does not implement audio capture, input meters or monitoring.

### 4A: capture feasibility and monitoring

Extend the selected input-device path to explicit mono/stereo channel pairs and
input meters. Start monitoring Off; make it an explicit action to avoid feedback.
Reuse the engine's track/bus processing for monitored audio. Define whether the
user hears effects while the stored take remains dry, and test that routing.
Use a preallocated capture ring and a worker writer. Keep input resampling and
clock-drift handling explicit for the supported device configurations.

### 4B: recording lifecycle

Record to uniquely owned temporary files. Track valid written frames and capture
gaps. Finalize WAV headers, decode/prepare peaks and register project-owned media
on workers before atomic take publication. Reuse source ownership for Save As,
collection and history. Support unnamed projects and keep Undo-retained takes.
Recover valid interrupted material without advertising unwritten frames.

Exit gate: all project rates, mono/stereo mapping, alignment against an existing
click, long capture/drift, buffer pressure, disk full, permissions, device loss,
cancel/stop, crash recovery, Save As and Undo/Redo pass. Define a measured timing
error budget in Stage 0. Report actual latency and verified device combinations.
Defer comping, punch recording and loop takes.

## Stage 5: timeline automation

**Feature status: pending.** Existing instrument modulation and sidechains do not
provide authored timeline automation lanes or write recording.

### 5A: model and playback

Start with track/Master volume and track pan, then register continuous effect
parameters. Store lanes and points by stable owner/destination IDs. Preserve
unsupported destinations without applying them. Use validated batch edits,
legacy defaults and bounded worker-prepared evaluation data. Evaluate in the
shared engine for playback, seek, loops and export. Define how automation and
existing modulation combine before applying each destination's final bounds.

### 5B: editing and write modes

Add lane show/hide, point creation/move/delete, interpolation and time-range
selection through shared interaction helpers. Deliver Read first, then Touch
and Latch with explicit takeover/return behavior. Capture control gestures with
engine-clock timestamps; commit one take to history after recording stops.
Define trimming/replacing existing curves and failure rollback before Write
ships. Defer discrete parameters and external plugin automation.

Exit gate: block-size independence, seek/loop/export parity, base/modulation
precedence, limits, destination deletion/copy, history, migrations, save/reopen,
write-mode boundaries and callback allocation checks pass.

## Stage 6: audio sidechain modulation

**Feature status: implemented; native acceptance open.** The
[audio sidechain contract](../modulation/audio-sidechain-plan.md) owns detector,
routing and UI behavior. The Source dropdown contains None, existing modulation
source groups, and Tracks/Groups/Returns with the established dividers. Selecting
an audio source reveals detector and normalized mapping controls in that
connection. Existing sources, signed amounts, typed destinations, connection
identities, power, reorder and telemetry remain shared.

Peak/RMS detection, channel selection, optional frequency filtering, input-level
bounds, curve/inversion and attack/release are implemented. Preparation resolves
combined audio/control ordering and alignment before callback publication. Source
lookup and detector advancement stay outside destination processing loops.
Desktop and headless export use the same processing path.

This milestone does not require Stage 5 automation. Their combination still
needs an explicit composition rule when automation is implemented.

**Acceptance:** retain detector/mapping references, routing/cycle/latency tests,
playback/seek/loop/export parity, mute/Solo, missing/deleted sources, identity,
history/persistence and callback-allocation coverage in
[validation history](../validation/README.md). Native source selection, mapping,
telemetry, audible ducking and playback/export alignment on macOS, Windows and
Linux remain open. This is verification work, not an unimplemented detector or UI.

## Stage 7: core release gate

For each delivered slice, update its user guide, topic specification and
[validation history](../validation/README.md). Run the required formatting,
Clippy, debug and release workspace checks; run CLI checks when shared
rendering/storage changes affect them. Keep debug assertions enabled. Verify
legacy projects and export parity. Run native capture, MIDI, timing, monitoring,
recovery and gesture checks on macOS, Windows and Linux. Automated frames do not
establish physical device or native UI behavior.

Close the milestone only when required checks pass or explicitly narrow the
advertised scope. Apply the same release gate to each lower-priority milestone.
No additional effect units are required for the six core milestones.

## Outstanding native acceptance for delivered features

Keep these gates separate from pending implementation. Automated tests, native
builds and limited UI observations are evidence only for the cases actually run.
Do not mark a gate complete from a passing synthetic test or package build.

| Delivered slice | Native checks still open | Contract / evidence |
| --- | --- | --- |
| Stage 1 recovery | Kill points around journal/base/checkpoint sync, publication and acknowledgment; disk full/permissions; missing sources; multi-instance admission and platform leases/durability | [Recovery contract](../projects/autosave-recovery.md), [MIDI/recovery validation](../validation/live-midi-2026-11-15.md) |
| Stage 2 fades/crossfades | Appearance/scaling, pointer gestures and listening for clicks, dips or excessive gain at joins | [Fade verification](../editing/fades.md#verification), [validation history](../validation/README.md) |
| Stage 3 live MIDI/recording | Physical keys/sustain, latency calibration, device loss/reconnect, long sessions, voice/processing and prepared-pitch/transport transitions | [Remaining gates and follow-ups](../validation/live-midi-2026-11-15.md#remaining-gates) |
| Stage 6 audio sidechains | Source-picker/mapping interaction in every theme, readable telemetry, audible ducking and playback/export alignment across platforms | [Sidechain acceptance](../modulation/audio-sidechain-plan.md#delivery-and-acceptance) |
| Milestone 9 clip/track Bounce | Native UI and listening at the documented render point, including tail behavior | [Bounce validation](../validation/bounce.md) |

Windows/Linux native device/UI and durability coverage remains open where listed
in each record. Future capture/automation gates depend on their feature work and
are not presented as acceptance-only gaps in delivered features. Recent
[Settings](../validation/settings-sidebar-2026-11-16.md),
[modulation submenus](../validation/destination-submenus-2026-11-16.md) and
[Mixer/gutter changes](../validation/beat-mixer-2026-11-16.md) are implemented;
their native UI limits remain in the linked records.

## Lower-priority delivery sequence

The remaining basic features follow the core milestones. Clip and complete-track
Bounce are already implemented within milestone 9. The proposed order for the
remaining work does not assign release dates. Each item needs a topic specification
before implementation, including defaults, bounds, cancellation, migrations and
failure behavior. Retain existing behavior until its replacement is specified.

| User milestone | Remaining feature work | Main dependencies |
| --- | --- | --- |
| 7 | Whole-section editing; section markers implemented | Shared timeline interaction, history and persistence |
| 8 | MIDI performance tools | Stage 3 recording; Stage 5 lane editing for controller lanes |
| 9 | Explicit time-range Bounce, freeze and batch stem export; clip/track Bounce implemented | Shared renderer/export, source ownership and Stage 1 recovery |
| 10 | Tempo ramps; step tempo/signature maps implemented | Shared musical clock; Stage 5 automation time-basis contract |
| 11 | Third-party plugin hosting | Stage 3 live input, Stage 5 automation and stable render/event contracts |

### Arrangement markers

Section marker creation with a one-bar range, inline renaming, unit-header selection, clip-style edge resizing/overlap resolution,
dragging, deletion, navigation, persistence and history are implemented. See [the timing-map contract](../audio/timing-maps.md).
Markers retain musical anchors during tempo edits and scale during rate edits.
Whole-section move/copy remains pending feature work.

Native acceptance remains open for pointer/focus behavior, short/narrow layouts,
Undo/Redo and navigation on supported platforms. Automated checks are recorded
separately in the [validation index](../validation/README.md).

### MIDI performance tools

Deliver quantize first, then swing and bounded humanization with deterministic
preview results. Define grid, strength, note-start/end behavior, timing/velocity
bounds and handling at clip limits. Preview and commit must use the same result;
commit a selected group as one history edit. Retain unmodified notes and shared
pattern ownership rules.

Add controller and sustain lanes after the event model supports stored controller
events. Define their interaction with sustain already resolved into recorded
note lengths, seeking, looping and instrument parameter automation. MIDI file
import/export can follow through the same validated event model; keep it
separate from live device support.

Exit gate: repeatable previews, no-op/cancel behavior, note ordering and bounds,
shared-pattern isolation, pending Sampler preparation, history and persistence
pass. Controller playback must match seek/loop/export behavior and release held
notes reliably.

### Bounce, freeze and batch stem export

**Implemented:** [Bounce clips and complete tracks](../editing/bounce.md) creates
project-owned audio through worker rendering and atomic publication, retains
disabled originals and supports one-step Undo/Redo. Its guide/contract defines
the render point, sends/Returns, tails, cancellation and storage ownership.
See [executed Bounce checks](../validation/bounce.md); native UI/listening remains
an acceptance gap, not pending clip/track rendering.

**Pending feature work:** explicit time-range Bounce and reversible instrument
freeze. Current Bounce requires whole clips or complete source tracks. Preserve the
original clips, instrument state, routing and automation for unfreeze. Specify
the render point, effect tails, latency alignment and treatment of sends/Returns
before implementing each action. Reuse worker rendering and transactional source
publication; never render in UI painting or overwrite source media.

Batch stem export also remains pending. Add explicit track selection, shared start/end bounds,
tail policy, file naming, destination collision handling and cancellation.
Define whether stems include shared returns and Master processing; do not imply
that separately exported stems always sum to the stereo mix.

Exit gate: render/live parity at the specified render point, aligned stems,
clipping/tail diagnostics, failed/cancelled publication, source retention,
Undo/Redo, unfreeze, Save As and recovery pass. CLI and desktop must share render
and validation behavior when both expose the operation.

### Tempo and time-signature maps

Explicit step maps, a shared compiled musical clock, Timeline lanes, source
mapping, renderer/effect timing, metronome/count-in and recording conversion are
implemented under [the timing-map contract](../audio/timing-maps.md). Legacy
constant maps retain their source/reference clock. Tempo ramps remain pending
feature work. Timeline automation remains its separate milestone.

Native acceptance remains open for audible boundary/seek/loop behavior, hardware
capture alignment, export comparisons and platform-specific gestures. Do not
count these checks as unimplemented map features. Record executed automated and
native checks separately in the [validation index](../validation/README.md).

### Third-party plugin hosting

Begin with a feasibility and license review using current SDK documentation.
Select the first format explicitly; existing independent CLAP/VST3 products do
not establish host support. Define scanning, compatibility diagnostics, missing
plugin state preservation, editor lifetime, state capture, process failures and
latency compensation. Decide the isolation policy before loading untrusted
plugins into production sessions.

Deliver one format with instruments/effects and state restoration, then add
stable automation mapping and offline render/freeze support. Reuse shared
parameter/event scheduling while retaining SDK and native window ownership in
host adapters. Add another format only after the first meets its acceptance
matrix. Keep the CLI usable without a display or audio device, and specify its
behavior when a project requires an unavailable plugin or editor-only service.

Exit gate: representative third-party instruments and effects on all supported
platforms, state/automation round trips, editor scaling/lifetime, variable
latency, offline rendering, scan/load failures and missing-plugin recovery pass.
Record callback guarantees and third-party limitations explicitly.

Advanced beat editing, punch/loop recording, take comping, additional effects
and custom instrument routing remain outside this ordered sequence until their
scope and dependencies are specified.
