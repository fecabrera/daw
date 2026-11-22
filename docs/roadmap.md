# Features and roadmap

DAW provides audio, MIDI and beat arrangement, layered instruments, effects, mixing, and stereo or mono export. The project format is experimental. See the [user guide](README.md) for instructions and the main [README](../README.md) for platform setup.

## Available features

- Arrange audio, MIDI and beat clips in Timeline. Move, copy, trim, loop and stretch clips with grid snapping. Shape audio [fades and adjacent crossfades](views/timeline.md#fades-and-crossfades) with handles or Clip settings.
- Build [beat patterns](views/beat-editor.md) with independently configured subtrack instruments. Mix the parent and expandable subtracks in [Mixer](views/mixer.md#beat-tracks).
- Create MIDI patterns and edit notes in the Timeline editor’s piano roll.
- Play and [record MIDI](editing/midi-recording.md) with multiple R-enabled tracks, selected-track fallback, count-in and sustain capture.
- Build MIDI instruments from Oscillator, Sampler, Granulizer, Noise, Envelope, LFO, Amplitude and Macro components.
- Use [audio sidechain modulation](modulation/audio-sidechain.md) from tracks, groups and Returns, with detector and 0–1 mapping controls beside each connection.
- Process instruments, tracks and Master with the [built-in effects](README.md#effects), including EQ, filters, dynamics, distortion, stereo imaging and reverbs.
- Mix tracks and nested [group buses](editing/track-groups.md) with gain, pan, mute, solo, shared effects and sends to Returns. Use Output to balance instrument layers.
- [Bounce selected clips or complete tracks](editing/bounce.md) to new audio tracks while retaining disabled originals.
- Save projects, collect source media, and use [private autosave and recovery](projects/recovery.md) for saved and unnamed projects.
- Export stereo WAV, MP3 or AAC. Inspect tracks, validate projects or render audio with the [CLI](cli/README.md).

## Current limits

Normal imports reference external audio recordings. Save does not automatically bundle them. Use **Collect project sources** to copy complete original media into a saved project. Included-source Save As also protects active Undo/Redo sources. Missing recordings remain visible and play as silence. Keep externally referenced recordings available, or collect them before moving the complete project folder.

[Undo and redo](editing/undo-redo.md) retain up to 100 project edits for the current session. Sampler supports root-relative playback across the full MIDI note range. [Export](projects/README.md#export) can append enabled effect tails after the selected range, up to ten seconds. Bypassed effects add no tail. The completion report warns when a longer tail is truncated. Mobile delivery is outside the current release scope.

## Core milestone status

“Implemented” means the listed feature is available. It does not mean all native
acceptance checks have passed. Those checks are listed separately below. The
original milestone numbers retain the proposed delivery order; they are not
release dates.

| Order | Milestone | Feature status |
| --- | --- | --- |
| 1 | Autosave and recovery | Implemented: private recovery snapshots, startup recovery and durable MIDI takes. Manual Save remains explicit. |
| 2 | Editable fades and crossfades | Initial scope implemented: fade handles, midpoint curves, adjacent crossfades and group editing. Curve presets and broader transitions remain pending. |
| 3 | Live MIDI and MIDI recording | Implemented: multiple R-enabled tracks, selected-track fallback, count-in, sustain-to-duration capture and recoverable whole takes. Performance extensions remain pending. |
| 4 | Audio recording and monitoring | Pending feature work. Saving an input-device preference does not provide capture or monitoring. |
| 5 | Timeline automation | Pending feature work. Instrument modulation is already available and remains separate. |
| 6 | Audio sidechain modulation | Implemented: track/group/Return sources, detector and mapping controls, and shared playback/export behavior. |

## Planned work

The next unimplemented core workflows are audio recording/monitoring and timeline
automation. Acceptance checks for implemented features remain open in parallel.
Each dependent milestone must still pass its required checks before it ships.

### 4. Capture audio

Add explicit mono/stereo input channels, input meters and monitoring that starts
off. Add recording, take finalization and timing alignment against the playing
arrangement. Keep original takes, support Undo/Redo and recovery, and report
device loss, disk errors and capture gaps. Punch recording, loop takes and take
comping remain later extensions.

### 5. Automate the mix

Start with drawn volume and pan lanes, then continuous effect parameters.
Playback, seeking, looping and export must read the same curves. Add recording
of control movements after curve playback is stable. Keep automation separate
from instrument modulation, with a defined rule for combining both.

### Extensions to implemented milestones

- **Audio transitions:** dedicated curve presets, including equal-power presets, and broader transition types. Current crossfades require touching audio clips with matching channel layouts; non-looped owners need source handles.
- **MIDI capture:** loop overdub, punch recording, controller/sustain lanes, MIDI Polyphonic Expression (MPE), MIDI file import/export and MIDI clock synchronization. Current recording stores sustain as note duration.

### Lower-priority milestones

These retain their proposed order after the core workflows. Partial delivery is
listed explicitly so completed Bounce work is not counted as pending.

| Order | Milestone | Remaining feature work |
| --- | --- | --- |
| 7 | Arrangement markers | Implemented: named section blocks, one-bar creation, edge resizing, editing/navigation, persistence and history. Moving or copying complete sections remains later work. |
| 8 | MIDI performance tools | Quantize, swing, humanization and controller/sustain editing. |
| 9 | Bounce, freeze and stem export | Clip and complete-track Bounce are implemented. Explicit time-range Bounce, reversible instrument freeze and batch stem export remain pending. |
| 10 | Tempo and time-signature maps | Step maps and Timeline lanes are implemented. Tempo ramps remain pending. |
| 11 | Third-party plugin hosting | External instruments/effects, saved plugin state and automation. Existing standalone DAW effect plugins do not provide hosting. |

Deliver markers before section editing, note timing tools before controller
lanes, and one hosted plugin format before expanding format support. Retain
Undo/Redo, save/reopen and playback/export consistency where applicable.

Advanced beat editing, punch recording, take comping, additional effects and
custom instrument routing remain a later backlog without an assigned order.

## Native acceptance checks still open

These are verification tasks for available features, not missing feature
implementations. Completed automated checks and limited native observations do
not establish behavior on every supported platform or device.

| Available workflow | Native acceptance still open |
| --- | --- |
| Autosave and recovery | Recovery after unexpected exits, storage failures, missing sources and concurrent app instances on supported platforms. |
| Fades and crossfades | Appearance, pointer gestures and listening for clicks, dips or excessive gain at joins. |
| Live MIDI and recording | Physical keyboard/sustain behavior, latency, device loss/reconnection, long sessions and audible instrument/effect transitions. |
| Audio sidechain modulation | Source selection and mapping controls, readable meters, audible ducking and playback/export alignment. |
| Clip and track Bounce | Native UI operation and listening checks for the rendered result. |
| Markers and timing maps | Lane gestures, short/narrow layouts, audible tempo transitions, count-in/capture with hardware, and cross-platform playback/export checks. |

Cross-platform native acceptance remains open, including Windows/Linux device
and UI behavior. Capture and automation will need their own native acceptance
checks after their feature work is implemented. Recent Settings, modulation-menu
and Mixer changes also retain native UI coverage limits; these do not make those
features pending implementations.

Technical dependencies, detailed acceptance gates and links to executed evidence
are in the [implementation roadmap](../specs/architecture/production-workflows-roadmap.md).
