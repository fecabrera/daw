# Basic production workflows: implementation roadmap

Status: proposed delivery plan. Existing behavior contracts remain authoritative.
Resolve the listed design gates in topic specifications before implementing each
milestone. Assign calendar estimates after device prototypes and acceptance
scope are measured.

For the user-facing sequence, see [Features and roadmap](../../docs/roadmap.md).

## Goals and dependencies

Deliver autosave/recovery, editable fades/crossfades, live MIDI and MIDI
recording, audio recording/monitoring, timeline automation and audio sidechain
modulation. Prefer a usable
vertical slice at each gate over adding all controls before their engine paths.

| Stage | Deliverable | Required predecessors |
| --- | --- | --- |
| 0 | Freeze storage, timing and interaction contracts | None |
| 1 | Autosave and crash recovery | Stage 0 storage contract |
| 2A | Editable audio clip fades | Stage 0 fade contract |
| 2B | Adjacent audio clip crossfades | Stage 2A and transition-model decision |
| 3A | Live MIDI input | Stage 0 event-clock contract |
| 3B | MIDI recording | Stages 1 and 3A |
| 4A | Audio capture and monitoring prototype | Stage 0 event-clock contract |
| 4B | Recoverable audio recording | Stages 1 and 4A |
| 5A | Automation model and read playback | Stage 0 parameter/timing contract |
| 5B | Automation lanes and write recording | Stage 5A; reuse Stage 3B gesture capture |
| 6A | Audio detector and shared sidechain provider | Stage 0 audio/control dependency contract; shared modulation |
| 6B | Sidechain sources and mapping controls | Stage 6A |
| 7 | Integrated native acceptance | All shipping slices |

Stages ship in the numbered user-facing order. Device feasibility prototypes may
run earlier to expose platform limits before delivery estimates are made.

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

Produce focused topic specifications and small feasibility probes. Confirm:

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

Add bounded, versioned recovery snapshots outside the manually saved manifest.
Use the existing snapshot worker and revision tracking. Retain the previous
valid generation until the next generation is published atomically. Preserve
edits made while a snapshot is writing. Avoid saving uncommitted field drafts.

Offer Recover, Open last saved and Discard recovery at startup. Support unnamed
projects and multiple application instances without snapshot collisions. Keep
recoverable project-owned media alive through retention and cleanup. Do not
silently collect external recordings. Define recovery cadence and storage bounds
in the topic specification; show failures through existing reporting paths.

Exit gate: process-kill/failure injection before and after publication; corrupt
latest snapshot fallback; newer-edit/manual-save races; explicit Discard;
unnamed projects; missing sources; instance isolation; and bounded cleanup all
pass. Recovery leaves manual saves intact and never deletes unrelated files.

## Stage 2: editable fades and crossfades

Initial implementation is described in [Audio clip fades and crossfades](../editing/fades.md). It includes the requested handle/midpoint controls and inspector settings. Broader curve presets and transition support remain planned.

### 2A: clip fades

Add saved fade-in/out lengths and movable control points on S-curves for audio
clips. Additional curve presets remain a later extension. Use dedicated handles in Timeline and the audio Editor, with previews,
release-only transactions, Escape/focus-loss cancellation and group-edit rules.
Keep the existing shared-boundary trim press region distinct from fade handles.
Use one evaluator for playback and export. Define rate/tempo edit behavior,
copy/split/trim/loop/stretch behavior and legacy defaults in the fade contract.

### 2B: transitions

Support adjacent audio clips, including looped clips. Non-looped owners need
sufficient source handles; looped owners continue their saved repeat phase. Show the
transition extent and clamp or reject unavailable material explicitly. Keep
source recordings unchanged. Store transitions by stable clip identity and
validate affected clips as one transaction. Prepare any required audio before
publication. Defer arbitrary same-track overlapping arrangements and automatic
crossfade creation until separately specified.

Exit gate: known-signal curve tests; mono/stereo and every project rate;
playback/export parity; source and stretch limits; missing audio; repeated clips;
transaction rollback; Undo/Redo; save/reopen; legacy sound; and gesture conflicts
pass. Native listening checks cover clicks, dips and excessive gain at joins.

## Stage 3: live MIDI and MIDI recording

### 3A: live input

Add machine-local MIDI device/channel selection and an explicit instrument target.
Schedule timestamped note-on/off, velocity and sustain through bounded engine
events, independent of editor painting. Keep live input distinct from chord and
mouse audition. Define release behavior on Stop, disconnect, target change and
focus loss; provide a panic action that releases held voices. Never leave stuck
notes after queue pressure. Prepare Sampler pitches on workers; define the live
behavior for a pitch that is not ready instead of processing it in the callback.

### 3B: record patterns

Add arming, Record and a configurable count-in through shared transport actions.
Capture notes, velocity and sustain; resolve sustain into playable note durations
until controller lanes exist. Publish validated patterns as one undoable take
through existing MIDI preparation/history paths. Preserve the original project
on failed publication and retain recoverable captured data where possible.
Defer loop overdub, punch, MIDI file exchange, continuous-controller editing, MIDI Polyphonic Expression and MIDI clock sync.

Exit gate: deterministic event ordering and frame/tick conversion, held-note
boundaries, sustain, count-in, disconnect/overflow, pending sample pitches,
collision policy, Undo/Redo, save/reopen and recovery pass. Test physical MIDI
keyboards and latency on each supported platform; synthetic events are insufficient.

## Stage 4: audio recording and monitoring

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

Extend the current modulation Source dropdown with Tracks, Groups and Returns.
Order its sections as None, divider, existing modulation sources, divider,
sidechain sources.
Selecting an audio source reveals detector and normalized mapping controls in the
same connection strip. Preserve existing sources, signed amount controls, typed
destinations, strip identities, power, reorder and telemetry.

Deliver the detector/provider and routing validation first, then the shared UI.
The [audio sidechain plan](../modulation/audio-sidechain-plan.md) defines proposed
Peak/RMS detection, channel selection, optional frequency filtering, input-level
bounds, curve/inversion and attack/release. Prepare combined audio/control
ordering and alignment before callback publication. Never resolve a cross-track
source or advance a detector inside a destination's processing loop.

This milestone reuses the shared modulation contract; it does not require
Stage 5 automation to function. Both features need explicit composition rules
when they control the same destination.

Exit gate: detector/mapping reference tests, routing/cycle and latency tests,
playback/seek/loop/export parity, mute/solo rules, missing/deleted source handling,
identity/history/persistence, callback allocation checks and native source-picker
interaction pass. Use the same processing path for desktop and headless export.

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

## Lower-priority delivery sequence

The remaining basic features follow the core milestones. This is a proposed
order; it does not assign release dates. Each item needs a topic specification
before implementation, including defaults, bounds, cancellation, migrations and
failure behavior. Retain existing behavior until its replacement is specified.

| User milestone | Work package | Main dependencies |
| --- | --- | --- |
| 7 | Arrangement markers | Shared timeline interaction, history and persistence |
| 8 | MIDI performance tools | Stage 3 recording; Stage 5 lane editing for controller lanes |
| 9 | Bounce, freeze and batch stem export | Shared renderer/export, source ownership and Stage 1 recovery |
| 10 | Tempo and time-signature maps | Shared musical clock; Stage 5 automation time-basis contract |
| 11 | Third-party plugin hosting | Stage 3 live input, Stage 5 automation and stable render/event contracts |

### Arrangement markers

Start with named point markers, add/rename/move/delete, previous/next navigation
and save/reopen. Use stable identities and shared history transactions. Define
whether markers follow absolute frames or musical positions, including behavior
under the existing global tempo/rate edits. Defer moving or duplicating complete
song sections until clip, automation and transition edits can form one atomic
transaction.

Exit gate: snapping and Shift bypass, cancellation, bounds, selection/focus
isolation, Undo/Redo, tempo/rate edits and persistence pass. Markers must not
change audio rendering.

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

Start with rendering selected clips or a time range to a new project-owned audio
asset. Then add track rendering and reversible instrument freeze. Preserve the
original clips, instrument state, routing and automation for unfreeze. Specify
the render point, effect tails, latency alignment and treatment of sends/Returns
before implementing each action. Reuse worker rendering and transactional source
publication; never render in UI painting or overwrite source media.

Add batch stem export with explicit track selection, shared start/end bounds,
tail policy, file naming, destination collision handling and cancellation.
Define whether stems include shared returns and Master processing; do not imply
that separately exported stems always sum to the stereo mix.

Exit gate: render/live parity at the specified render point, aligned stems,
clipping/tail diagnostics, failed/cancelled publication, source retention,
Undo/Redo, unfreeze, Save As and recovery pass. CLI and desktop must share render
and validation behavior when both expose the operation.

### Tempo and time-signature maps

Add explicit tempo/signature events before considering tempo ramps. Define a
shared piecewise musical-time conversion used by rulers, snapping, scheduling,
count-in, metronome, automation and export. Preserve integer frame clip ranges
and the existing MIDI tick content. Specify which objects follow musical time
and which retain absolute time; document the treatment of audio clips, loops,
recorded takes, fade transitions and markers.

Migrate a legacy project's single tempo/signature into an equivalent constant
map without changing its sound or positions. Specify new map-edit transactions
and their overlap/overflow rollback rules before changing `Edit::SetTempo`
behavior. Validate the full affected arrangement atomically.

Exit gate: boundary rounding and conversion tests, changes inside notes/clips,
seek/loop/metronome/export parity, recording alignment, cancellation, history,
rate changes and legacy round trips pass.

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
