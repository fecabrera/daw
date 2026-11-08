# Features and roadmap

DAW provides audio, MIDI and beat arrangement, layered instruments, effects, mixing, and stereo export. The project format is experimental. See the [user guide](README.md) for instructions and the main [README](../README.md) for platform setup.

## Available features

- Arrange audio and MIDI clips in Timeline. Move, copy, trim, loop, and stretch clips with grid snapping. Shape audio [fades and adjacent crossfades](views/timeline.md#fades-and-crossfades) with handles or Clip settings.
- Build [beat patterns](views/beat-editor.md) with independently configured subtrack instruments and shared mix controls.
- Create MIDI patterns and edit notes in Editor's piano roll.
- Build MIDI instruments from Oscillator, Sampler, Envelope, LFO, Amplitude, and Macro components.
- Use [audio sidechain modulation](modulation/audio-sidechain.md) from tracks, groups and Returns, with detector and 0–1 mapping controls beside each connection.
- Process tracks and Master with 3-band EQ, Drive, Bitcrusher, Saturator, Auto-filter, Compressor, OTT, Limiter, Direction Mixer, and Stereo Spread.
- Mix tracks and nested [group buses](editing/track-groups.md) with gain, pan, mute, solo, and shared effects. Use Output to balance the layers of an instrument.
- Save projects and export stereo WAV, MP3, or AAC. Inspect tracks, validate projects or render audio with the [CLI](cli/README.md).

## Current limits

Normal imports reference external audio recordings. Save does not automatically bundle them. Use **Collect project sources** to copy complete original media into a saved project. Included-source Save As also protects active Undo/Redo sources. Missing recordings remain visible and play as silence. Keep externally referenced recordings available, or collect them before moving the complete project folder.

[Undo and redo](editing/undo-redo.md) retain up to 100 project edits for the current session. Sampler supports root-relative playback across the full MIDI note range. [Export](projects/README.md#export) appends enabled effect tails after the arrangement, up to ten seconds. Bypassed effects add no tail. The completion report warns when a longer tail is truncated. Mobile delivery is outside the current release scope.

## Planned work

The proposed priority order below covers the next basic production workflows.
The initial fade/crossfade controls and audio sidechain modulation are implemented;
native sidechain acceptance checks remain open. The other milestones remain
planned. Each milestone must pass its acceptance
checks before the next dependent milestone ships. This is a delivery proposal,
not a release-date commitment.

| Order | Milestone | What you will be able to do |
| --- | --- | --- |
| 1 | Autosave and recovery | Recover recent edits in saved and unnamed projects after an unexpected exit, without replacing the last manual save. |
| 2 | Editable fades and crossfades | Shape audio clip entrances and endings, and blend adjacent audio clips without changing their recordings. |
| 3 | Live MIDI and MIDI recording | Play built-in instruments from a MIDI keyboard, then record notes, velocity and sustain into editable patterns. |
| 4 | Audio recording and monitoring | Arm audio tracks, choose input channels, monitor input and record mono or stereo takes alongside the arrangement. |
| 5 | Timeline automation | Draw volume, pan and continuous effect-parameter changes, then record control movements during playback. |
| 6 | Audio sidechain modulation | Use tracks, groups and Returns as modulation sources, with controls that map their audio to values from 0 to 1. |

### 1. Protect work

Add background recovery snapshots, recovery choices at startup and a bounded
retention policy. Manual Save remains explicit. Recovery must retain available
project-owned audio and report missing external recordings. Completed recordings
must become recoverable before their temporary files can be removed.

### 2. Finish audio transitions — initial controls implemented

Fade-in/out handles, adjustable midpoint curves and fixed-boundary adjacent
crossfades are implemented for audio clips. Clip settings uses the same values.
Selected audio clips support group fade and crossfade editing.
Crossfades require touching clips and matching channel layouts. Either or both
clips can loop; non-looped owners need source handles. Dedicated equal-power presets and broader transition types remain
planned. Native appearance and listening acceptance remain open.

### 3. Capture MIDI performances

Start with device selection, live playing and reliable note release on Stop,
disconnect and track changes. Follow with track arming, a configurable count-in
and recording notes, velocity and sustain. Recorded patterns must remain editable
and survive save, reopen and recovery. Loop overdub, controller editing and
expression features follow later.

### 4. Capture audio

Start with explicit mono/stereo input selection, input meters and monitoring
that is off by default. Add recording, take finalization and timing alignment
against the playing arrangement. Keep original takes, support Undo/Redo, and
report device loss, disk errors and capture gaps. Punch recording and take
comping follow later.

### 5. Automate the mix

Start with drawn volume and pan lanes, then continuous effect parameters.
Playback, seeking, looping and export must read the same curves. Add recording
of control movements after curve playback is stable. Keep automation separate
from instrument modulation, with a defined rule for combining both.

### 6. Modulate from other audio

Implemented; see [Audio sidechain modulation](modulation/audio-sidechain.md).

Extend the existing modulation Source dropdown with **Tracks**, **Groups** and
**Returns**. Show **None** first, then the existing modulation sources, then the
sidechain sources, with a divider between each section. Keep the existing
modulation amount and destination controls.
Selecting an audio source reveals its sidechain controls in that connection.

Choose what to measure: Peak or RMS level, the stereo signal or a selected
channel, and an optional frequency range. Set the input levels that map to 0 and
1, then shape the response with attack/release, curve and invert controls. Show
the detected level and mapped value so the result is clear. For example, a kick
track could reduce a bass track's gain, or a vocal could move a filter cutoff.
The same detector and mapping must run during playback and export.

### Lower-priority milestones

These follow the six core milestones above. The proposed order starts with
smaller arrangement improvements and leaves plugin hosting last because it adds
a larger compatibility and platform-testing scope.

| Order | Milestone | What you will be able to do |
| --- | --- | --- |
| 7 | Arrangement markers | Name song sections, navigate between them and keep markers with the saved project. |
| 8 | MIDI performance tools | Quantize existing notes, apply swing and humanization, and edit controller and sustain data. |
| 9 | Bounce, freeze and stem export | Render selected clips or tracks, temporarily freeze instruments and export separate tracks in one operation. |
| 10 | Tempo and time-signature maps | Place tempo and signature changes at song positions while preserving the intended musical and audio timing. |
| 11 | Third-party plugin hosting | Load external instruments and effects, save their state and automate their parameters. |

Deliver these in smaller steps: markers before section editing; note timing tools
before controller lanes; selected-range rendering before freeze and batch export;
explicit tempo changes before tempo ramps; and one plugin format before expanding
format support. Each step must retain Undo/Redo, save/reopen and playback/export
consistency where applicable.

Advanced beat editing, punch recording, take comping, new effects and custom
instrument routing remain a later backlog. Their detailed scope and order are
not assigned by this roadmap.

Technical dependencies, scope decisions and acceptance gates are in the
[implementation roadmap](../specs/architecture/production-workflows-roadmap.md).
