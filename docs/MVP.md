# Desktop DAW MVP

Status: Scope baseline for later specification definitions.

First implementation specification: [Desktop DAW MVP Iteration 1](specs/mvp-iteration-1.md).

## Documentation standard

Use a combination of ASD-STE100 principles and Plain Language (PL) for project documentation, specifications, and user instructions.

- Use short, direct sentences and active voice.
- Use one consistent term for each concept. Define technical terms and abbreviations on first use.
- Use familiar words. Include technical detail when the reader needs it to understand or perform a task.
- State requirements, defaults, limits, and expected results explicitly. Separate confirmed requirements from proposals and open decisions.
- Use numbered steps for procedures and tables for comparisons or structured requirements.
- Remove filler, promotional language, and ambiguous wording.
- Apply these principles as a practical writing standard; do not claim formal ASD-STE100 compliance without a compliance review.

## Objective

Build a small multitrack audio editor for macOS, Linux, and Windows. A user must be able to import recordings, arrange clips, adjust the mix, save and reopen a project, and export a stereo WAV file.

Use Rust with egui for the desktop interface. Keep the audio engine independent of the interface. Future targets are iPadOS and Android; mobile delivery is outside the MVP.

Provide a headless command-line mode from the start, using the same project model and audio engine as the desktop interface.

## Required features

| ID | Area | Requirement |
| --- | --- | --- |
| MVP-01 | Import | Import WAV files and display a waveform for each clip. |
| MVP-02 | Tracks | Allow stereo audio tracks with no fixed track-count limit. Add and delete tracks. Provide gain, pan, mute, and solo controls per track. |
| MVP-03 | Clips | Move, trim, split, and delete clips without modifying the source recording. Clips on the same track must not overlap; clips on different tracks can play simultaneously. |
| MVP-04 | Timeline | Provide a simple timeline with a track list on the left and a toolbar above the timeline containing playback controls. Provide horizontal zoom and scrolling. |
| MVP-05 | Transport | Play, pause, stop, seek, and loop a selected region. |
| MVP-06 | Mixing | Mix tracks. Provide a master gain control and peak meters. |
| MVP-07 | Output | Use the system's default audio output device. Display audio initialization errors. If the device disconnects, stop playback safely and display a clear error. |
| MVP-08 | Projects | Save and open projects stored as folders containing manifest files and an assets directory. Reference external source audio files during the MVP. Open projects with missing sources normally, show a warning, and display affected clips as empty placeholders. |
| MVP-10 | Export | Export the arrangement as a stereo WAV file. |
| MVP-11 | Sample rate | Use a fixed project sample rate of 48 kHz. Convert imported audio to 48 kHz, or reject unsupported rates with a clear message. |
| MVP-12 | Clip boundaries | Apply short fades at clip boundaries to prevent clicks. |
| MVP-13 | Headless mode | Load and validate an existing project and export its arrangement as WAV from the command line, without a graphical session or audio output device. |

Undo/redo is outside the MVP. Requirement ID MVP-09 is retired; the remaining IDs are unchanged.

## MVP technology stack

Use stable Rust, edition 2024, and Cargo. Select compatible released crate versions during implementation and commit Cargo.lock for reproducible application builds.

| Area | Library or technology | Use |
| --- | --- | --- |
| Desktop UI | [egui and eframe](https://github.com/emilk/egui) | Window lifecycle, widgets, input, and custom timeline controls. Use eframe's wgpu renderer. |
| Headless CLI | [clap](https://docs.rs/clap/latest/clap/) | Command parsing, help, project validation, and offline export. |
| Waveforms and timeline | egui Painter with application code | Draw cached waveform peaks, clips, rulers, and meters. Generate waveform peaks on a background worker. |
| Audio output | [cpal](https://github.com/RustAudio/cpal) | Default output device, stream configuration, audio callback, and stream errors. |
| Audio engine | Application code in Rust | Sample-based transport, clip scheduling, gain/pan, mute/solo, fades, metering, and mixing. Reuse the rendering logic for live playback and offline export. |
| Audio decoding | [symphonia](https://github.com/pdeljanov/Symphonia) | Enable WAV and PCM support for the MVP. Wrap it behind the decoder interface; enable more formats after the MVP. |
| WAV export | [hound](https://docs.rs/hound/latest/hound/) | Write WAV exports behind the encoder interface. |
| Sample-rate conversion | [rubato](https://github.com/HEnquist/rubato) | Convert imported audio to 48 kHz on a background worker. |
| Engine communication | [rtrb](https://docs.rs/rtrb/latest/rtrb/) and standard atomics | Preallocated single-producer/single-consumer command queues; atomic meter values and status counters. |
| Background work | Rust standard library threads and channels | Import, waveform generation, project I/O, and export. Keep blocking channels outside the audio callback. |
| Project manifests | [serde](https://serde.rs/) and [serde_json](https://docs.rs/serde_json/latest/serde_json/) | Experimental JSON manifests for project state and asset references. |
| Stable identifiers | [uuid](https://docs.rs/uuid/latest/uuid/) | Persistent project, track, clip, and asset IDs. |
| File dialogs | [rfd](https://github.com/PolyMeilex/rfd) | Source import, project folder selection, and export destination dialogs. Use the XDG Desktop Portal backend on Linux. |
| Errors | [thiserror](https://docs.rs/thiserror/latest/thiserror/) | Typed errors for audio, decoding, project storage, and export. |
| Diagnostics | [tracing](https://docs.rs/tracing/latest/tracing/) and tracing-subscriber | Diagnostics outside the audio callback. Report callback counters/errors through nonblocking communication. |

### Audio backend choices

- macOS: CoreAudio through CPAL.
- Windows: WASAPI through CPAL, using shared output for the MVP.
- Linux: ALSA through CPAL, using the system default device and its PipeWire/PulseAudio bridge where configured. Linux builds require ALSA development libraries.
- Keep the project and mixer at 48 kHz. Prefer a 48 kHz device stream. If the default device requires another rate, use a preallocated Rubato output conversion stage; the project sample rate stays unchanged.
- Keep audio backend details behind an output interface so additional backends can be added later.

### Module boundaries and real-time rules

- Separate project model, audio engine, media import/export, project storage, and desktop UI into modules within a Cargo workspace.
- Provide separate desktop and CLI executable targets backed by shared core crates. The CLI target must build without egui/eframe, wgpu, rfd, or CPAL dependencies; keep graphics and device output in desktop-specific crates.
- Keep egui, manifest serialization, and file dialogs outside the engine's rendering code.
- Preallocate render buffers and queue capacity. Bound command processing per callback and handle queue-full conditions outside the callback.
- Create and retire audio assets on a control/background thread. Avoid final asset destruction and deallocation in the callback.
- Live playback and export use separate render state built from the same project model.
- Use Rust's built-in tests for mixing, transport, import conversion, and project save/reopen behavior. Run cargo fmt, cargo clippy, and cargo test; validate playback on all three desktop platforms.

## Reusable component architecture

- Build features from reusable components with clear responsibilities and interfaces. Prefer composition through Rust structs and traits where implementations need to vary.
- Provide a shared application core for project entities, stable IDs, sample-time types, asset references, and editing commands. Engines and application interfaces use these common definitions.
- Share audio processing components, including gain, pan/balance, fades, metering, and mixing, between live playback and offline rendering.
- Keep shared application types independent of egui, graphics, device backends, and file dialogs. The headless application must use the same core without UI dependencies.

### Shared UI panel core

- All UI panels use a common panel foundation for identity, title/header, content framing, spacing, styling, and visibility.
- Compose panel-specific content, such as the timeline, mixer, and transport, within that foundation. Keep the shared shell separate from each panel's behavior and view state.
- Reuse common controls for gain, pan/balance, mute/solo, meters, and error display instead of implementing separate versions in each panel.
- Panels read application state and submit commands through a common application interface. They must not own duplicate project data or access the audio callback directly.
- Keep the panel foundation within the UI layer. Future themes and mobile layouts can reuse its controls and conventions while providing different layouts and input behavior.
- Introduce shared abstractions when components have actual common behavior; avoid a universal base engine or a panel interface that includes unrelated responsibilities.

## Initial desktop UI

Use the supplied Audacity 4 screenshots as layout references.

- Place the track list on the left and the timeline to its right.
- Align each track's controls with its corresponding timeline lane. Use a simple outlined block containing the track name, a gain input box in dB, a pan/balance input box, and Mute and Solo buttons. Keep track deletion available through a context menu.
- Include a Tracks header and an Add track button. Keep Add track enabled regardless of the track count.
- Place a toolbar above the timeline. Toggle Play to Pause during playback, using the corresponding Lucide icons. Include a separate Stop button.
- Put New project, Open, Import WAV, Save, Save as, and Export WAV in the File menu. Use the native macOS menu bar and an in-window menu on Windows and Linux. Group these actions with separators and provide keyboard shortcuts.
- Display a bars-and-beats ruler and grid above the track lanes using project tempo and 4/4. Show a visible playhead and waveforms within clips. Include a matching bar.beat monitor beside the toolbar time display. Count bars and beats from 1; start the monitor at 0001.01.
- Keep the track list visible during horizontal timeline scrolling. Keep track rows and timeline lanes aligned during vertical scrolling.
- Provide timeline zoom and loop-region controls, plus master gain, peak metering, and clipping status, within the initial layout.
- Place Master in a track-style block fixed at the bottom of the track list, above the status bar. Include master gain and L/R output monitors without Mute or Solo buttons. It is separate from the audio tracks.
- Show the empty track list and timeline before tracks are added. Use the shared UI panel foundation and reusable controls.
- Use simple colored clip blocks with a thin outline, slightly rounded corners, a name strip at the top, and a waveform body. Mono clips show one waveform; stereo clips show two stacked waveforms. See the [clip reference](specs/assets/clip-reference.png) and the implementation specification for details.

## Headless mode

- Minimum MVP commands: validate an existing project folder and render its arrangement to a WAV destination.
- Reuse project loading, source resolution, decoding, mixing, fades, and export logic from the desktop application.
- Do not initialize windows, graphics, file dialogs, or audio devices for headless validation/export.
- Take project and output paths as command-line arguments. Return clear diagnostics and a nonzero exit status on failure; missing or unsupported sources must not be silently skipped.
- Missing sources are warnings, not export failures. Write warnings identifying missing files to standard error, render their clips as silence, and return success if export completes. Unsupported encodings and other processing failures remain errors.
- Report export clipping through diagnostics, using the same final-output clamping as the desktop application.
- Headless operation must work on macOS, Windows, and Linux, including Linux without a graphical session.
- Interactive headless editing, device playback, and remote control are outside the initial headless scope; they can be specified later.

## Mixing behavior

- Use fixed equal-power panning for mono sources in the MVP: approximately -3 dB per channel at center and unity gain on the active channel at either extreme.
- Use fixed stereo balance for stereo sources: preserve both channels at unity gain at center; attenuate the opposite channel toward silence at either extreme without moving its content to the other channel.
- Apply panning according to each clip's source channel layout, using the track's pan/balance position. Preserve source channel metadata when converting audio to the internal representation.
- Solo overrides mute. A track with both solo and mute enabled plays.
- If any track is soloed, play all soloed tracks and silence tracks that are not soloed.
- If no track is soloed, play all tracks that are not muted.
- Use the same mute/solo rules for playback and export.
- Retain floating-point headroom within the mixer. Show a clipping indicator when the master signal exceeds the output range; clamp only at the final device output and integer WAV export. Do not apply automatic normalization or a limiter in the MVP.
- Apply 5 ms linear fades at clip starts and ends. For clips shorter than 10 ms, shorten each fade to half the clip duration so the fades do not overlap.
- Smooth live gain, pan/balance, mute, and solo transitions to avoid clicks.

## Project storage

- Store each project in its own folder with manifest files and an assets directory.
- Manifest files store project state, asset identities, metadata, and source references.
- During the MVP, source audio remains external and is referenced by the manifests.
- Reserve the assets directory for project-owned assets, including future included source audio files. External source audio is not copied into it during the MVP.
- Future source management must support both external references and sources included in the project's assets directory.
- Use an experimental project.json manifest at the project folder root and an assets/ directory for project-owned assets. The manifest schema can change substantially during MVP development.
- Store source paths relative to the project folder where possible; otherwise store absolute paths. Resolve relative paths from the project folder, not the application's working directory.
- When closing the application or opening another project with unsaved changes, prompt to Save, Discard, or Cancel. Continue after Save only if saving succeeds.
- Save by writing a temporary manifest and replacing the previous manifest only after writing succeeds. On failure, preserve the previous manifest and keep the project marked unsaved.
- Revalidate source metadata when opening a project. Warn if a changed source causes saved clip ranges to exceed its current duration; preserve clip data and treat unavailable ranges as silence.

### Initial project.json schema

Use one manifest for the MVP. This is the initial schema, not a stable compatibility contract. Ignore schema_version during loading for now; it is optional and reserved for future use. Validate fields and project constraints against the current implementation. Version checks and migrations are deferred until the format stabilizes.

```json
{
  "schema_version": 1,
  "project_id": "c2e1c40e-e9bd-444e-88c0-a195f18b1b2e",
  "name": "My project",
  "sample_rate_hz": 48000,
  "tempo_bpm": 120.0,
  "master": {
    "gain_db": 0.0
  },
  "assets": [
    {
      "id": "41835009-63d9-4c4b-bb9d-50a5905c2ef6",
      "name": "Guitar.wav",
      "source": {
        "kind": "external",
        "path": "../recordings/Guitar.wav",
        "path_kind": "relative"
      },
      "source_metadata": {
        "sample_rate_hz": 48000,
        "channels": 1,
        "sample_format": "pcm_int",
        "bits_per_sample": 24
      },
      "decoded_frame_count": 14400000
    }
  ],
  "tracks": [
    {
      "id": "114b83e2-f6dc-4ccc-9687-f13ce5ff76a5",
      "name": "Guitar",
      "gain_db": 0.0,
      "pan": 0.0,
      "muted": false,
      "soloed": false,
      "clips": [
        {
          "id": "68a15f7b-dacc-4b91-8e89-49ac29d6af50",
          "asset_id": "41835009-63d9-4c4b-bb9d-50a5905c2ef6",
          "name": "Guitar take",
          "start_frame": 0,
          "source_offset_frame": 0,
          "length_frames": 14400000
        }
      ]
    }
  ],
  "transport": {
    "playhead_frame": 0,
    "loop": {
      "enabled": false,
      "start_frame": 0,
      "end_frame": 14400000
    }
  }
}
```

#### Field rules

- Use UUIDs for project, asset, track, and clip identities. Track array order defines display order.
- Store tempo_bpm as a positive finite BPM value. Default to 120.0 for new projects and manifests without this field. Set it by double-clicking the playback toolbar's tempo monitor. Show `120bpm` with the time monitor's styling and three fixed-width digit slots. Enter commits; Esc or focus loss cancels the draft and restores the monitor. It does not move or stretch audio clips.
- All frame positions, offsets, and lengths use the project's 48 kHz timeline, including source offsets after resampling. One frame contains one sample per channel; five minutes equals 14,400,000 frames.
- Source metadata describes the original file. decoded_frame_count describes its length after conversion to the project sample rate. Use pcm_int or ieee_float for sample_format.
- pan ranges from -1.0 (left) to 1.0 (right); 0.0 is center. Gain values are finite decibel values; mute is a separate boolean.
- source.kind is external for the MVP. Reserve an included variant for future project-owned sources. path_kind distinguishes relative and absolute paths; resolve relative paths from the project folder.
- Clips reference assets by ID. More than one clip can use the same asset without duplicating its source record.
- Preserve asset metadata and clip ranges when sources are missing. Calculate missing-source status when opening the project; do not persist it as a flag.
- Clip ranges use an inclusive start and exclusive end. Require nonnegative integer positions, positive clip lengths, and source_offset_frame + length_frames no greater than decoded_frame_count.
- Permit any track count. Require unique IDs, valid asset references, and no same-track clip overlap. Enabled loops require end_frame greater than start_frame.
- Do not store decoded PCM, waveform caches, output device settings, or active playback state in the manifest. Open projects stopped at the saved playhead position.
- Keep the fixed MVP pan law, fades, export format, and dither policy in the application's MVP defaults. Add explicit settings when they become configurable.
- Do not reject a project based on schema_version. Reject malformed JSON, invalid fields, and unsupported project structures with clear errors. Compatibility with earlier experimental manifests is not guaranteed.
- Reserve assets/ for project-owned files; its internal layout can be specified when source inclusion is implemented.

## Transport and export range

- Disable playback for a project with no clips. Reject export of such a project with a clear message.
- Before replacing an existing export file, require confirmation in the desktop UI or an explicit --overwrite flag in the CLI.
- Play starts at the current playhead. Pause holds that position, and Play resumes from it. Stop returns to the position where playback started, including after pause/resume or a seek during playback. A new playback after Stop or natural completion records a new start position.
- Without looping, playback stops at the end of the last clip.
- When looping is enabled, repeat the selected time range.
- Export from project time zero to the end of the last clip, including leading silence. Ignore transport looping when exporting.

### Missing sources

- Open projects with missing sources normally and show a clear warning identifying the missing files.
- Display affected clips as empty placeholders without waveforms. Preserve their asset references, track positions, source offsets, and durations.
- Missing-source clips produce silence during playback; available sources remain playable and the project remains editable.
- Saving must preserve missing-source references and placeholder clip data.
- Allow export with missing sources. Show a warning identifying the missing files and render their clips as silence, preserving their positions and durations. The desktop warning must not block export; the headless CLI reports the same condition through diagnostics.

## Clip placement

- Do not provide timeline snapping in the MVP. Clip placement, trimming, and selections use sample-based positions without snapping to a grid or clip boundaries.
- Clips on the same track must not overlap. Clips on different tracks may overlap in time.
- Reject imports, moves, and trims that would create overlap; retain the previous valid placement and give clear feedback.
- Show a translucent clip preview while moving, including across tracks. Clamp movement at frame zero. The preview and dropped clip must use the same position.
- Preview WAV files dragged into the timeline at the hovered track and sample position. Load the waveform in the background without changing the project. Reuse prepared audio on drop; empty timeline drops create a track. Cancelled hovers and invalid drops leave the project unchanged.
- Clips may touch at their boundaries. Missing-source placeholders reserve their existing time ranges under the same placement rules.
- Project validation must detect same-track overlap, including in headless mode.

## MVP WAV scope

- Import uncompressed WAV files with 16-bit or 24-bit integer PCM, or 32-bit IEEE floating-point samples.
- Accept mono and stereo sources. Feed mono sources equally to the left and right channels before track gain and pan; preserve stereo channel order. Reject files with more than two channels with a clear error.
- Decode and mix using 32-bit floating-point PCM at the project's fixed 48 kHz sample rate.
- Export stereo WAV with 24-bit integer PCM at 48 kHz using fixed settings.
- Reject unsupported WAV encodings with a clear error. Additional encodings and configurable conversion/export settings follow after the MVP.

### Export quantization and dither

- Apply triangular (TPDF) dither without noise shaping once, immediately before final conversion to 24-bit integer PCM.
- Round to the nearest integer sample and clamp the result to the valid 24-bit integer range.
- Use a deterministic dither seed and identical export processing in the desktop and headless paths so repeated exports of the same project and settings match.
- Dither does not prevent clipping. Retain the agreed clipping diagnostics and final-output clamping.
- Keep these settings fixed in the MVP; user-selectable dither modes and an off option follow in the short-term roadmap.

## Audio architecture requirements

Support for additional audio file formats is planned after the MVP. The initial implementation must support that extension without changes to playback or mixing logic.

### Processing flow

Audio file → decoder → sample-rate and channel conversion → internal audio asset → audio engine.

### Module boundaries

| ID | Component | Requirement |
| --- | --- | --- |
| ARCH-01 | Decoders | Define a common decoder interface for metadata and block-based decoding to floating-point PCM. Implement WAV first. Additional formats must use the same interface. |
| ARCH-02 | Import | Keep decoding and sample-rate/channel conversion separate from playback and mixing. Define handling for mono, stereo, and unsupported channel layouts. |
| ARCH-03 | Audio assets | Store decoded samples separately from clips. Each clip references an asset and a sample range. Moving, trimming, and splitting must not copy the asset's audio samples. |
| ARCH-04 | Audio engine | Process PCM buffers without knowledge of file extensions, codecs, or containers. Keep the engine independent of egui. |
| ARCH-05 | Export | Define an encoder interface separate from the decoder interface. Import support for a format does not imply export support. |
| ARCH-06 | Real-time processing | Keep file access, decoding, allocation, and blocking locks outside the audio callback. Playback must continue when interface rendering stalls. |
| ARCH-07 | Communication | Use bounded queues for interface-to-engine commands. Provide a nonblocking path for meter data from the engine to the interface. |
| ARCH-08 | MVP loading | Decode imported files before playback. Keep asset access separate from the mixer so background streaming through buffered readers can be added later. |
| ARCH-09 | Source information | Preserve the source file path and metadata. Separate asset identity from source location so future projects can use external references or included source files without changes to clip references or mixing logic. |
| ARCH-10 | Timing | Use sample positions internally for precise clip and playback timing. |

A decoder trait and clear module boundaries are sufficient for the MVP. A runtime plugin system is a mid-term goal after the MVP. Keep extension interfaces separate from any future plugin discovery and loading mechanism.

## Output buffer and latency

- Request an output buffer of 512 frames at 48 kHz.
- If that size is unsupported, request the nearest supported size. Use the host default if fixed-size requests fail.
- Handle variable callback sizes; the requested size is not guaranteed by the audio backend.
- A 512-frame buffer at 48 kHz contains approximately 10.67 ms of audio. This is not total output latency; OS buffering, drivers, and hardware add delay.
- Target no more than 50 ms from play/seek command submission to audible change on reference hardware.
- Target no audible dropouts during the agreed test workload. Prioritize stable playback over smaller buffers.
- Add user-selectable buffer sizes and recording/monitoring latency targets with the short-term audio settings.

## First prototype milestone

1. Import two WAV files onto separate tracks.
2. Display their waveforms and move the clips.
3. Play both tracks together with independent gain and mute controls.
4. Seek and zoom while playback continues.
5. Export the mix as a stereo WAV file.
6. Validate and export the same project through the headless CLI.

After this milestone, implement the remaining MVP features.

## Completion criteria

### Reference hardware

Validation environments:

- macOS 27.
- Windows 11.
- Manjaro Linux with GNOME.

Record the exact OS build, Manjaro package versions, and GNOME version in each validation report.

Minimum system memory: 8 GB of RAM on all supported platforms. The MVP must work within this minimum on each reference machine under the agreed test workload.

macOS validation:

- MacBook Air M5.
- MacBook Pro M4 Pro.

Windows and Linux validation:

- Desktop with an AMD Ryzen 7600X CPU.

### Acceptance criteria

- Play and mix four stereo audio tracks simultaneously within the agreed test workload on each reference machine, with a minimum of 8 GB of RAM. Permit projects with more than four tracks in desktop and headless modes. Track capacity depends on available memory; playback capacity also depends on CPU performance and audio buffer settings.
- A user can assemble a short piece, save the project, reopen it, and export the same mix.
- Playback continues without audible dropouts during scrolling, zooming, and editing in the agreed test workload.
- The import, arrangement, playback, save/reopen, and export workflow works on macOS, Linux, and Windows.
- Missing source files, audio initialization failures, and output-device disconnections produce clear errors. Output-device disconnection stops playback safely.
- A project with missing sources opens with a warning and empty placeholder clips that retain their positions and durations. Available audio remains playable.
- Desktop and headless export succeed with missing sources, warn about the missing files, and preserve the export range while rendering affected clips as silence.
- Imports and edits cannot create overlapping clips on the same track.
- Additional decoders can be integrated without changing playback or mixing logic.
- Build and run the CLI without desktop UI or audio-device dependencies. Validate and export a project without a graphical session or audio output device; desktop and CLI exports must match for the same project and export settings.

### Playback test workload

- Load four stereo sources, each five minutes long, on separate tracks: 20 minutes of total decoded audio.
- At 48 kHz with 32-bit floating-point samples, source PCM occupies approximately 460.8 MB, excluding waveform data, working buffers, and other application memory.
- Play all four tracks together in a loop for 30 minutes while scrolling, zooming, moving clips, and adjusting gain.
- Require no unintended audible gaps or clicks and zero reported output underruns where the backend exposes them.
- Run this test on each reference platform with the 8 GB RAM minimum. This is a validation baseline, not a project-duration limit.

## Future features

These horizons describe the proposed order after the MVP, not delivery dates.

Source management is also a future goal; its delivery horizon remains to be defined. Projects must be able to include source files within the project's assets directory. A source manager must let the user control which files remain externally referenced and which are included. Detailed source manager behavior will be defined during specification.

### Short-term

#### Audio Engine

- Additional project sample-rate options.
- Background streaming for large audio files.
- Source buffer files: cache decoded PCM audio on disk and stream it through bounded memory buffers, so playback does not require entire sources in RAM. Generate and read caches outside the audio callback. Cache files are rebuildable from source files and do not replace the original sources.
- Configurable memory budget for decoded audio and streaming buffers. Define defaults and behavior when the budget is reached during specification.
- Recording and monitoring latency targets.

#### Audio Features

- Audio recording.
- Additional import and export formats.
- User-configurable export bit depth and sample format, mono/stereo export, channel mapping/downmix rules, and dither modes including an off option.

#### Musical Timing

- Metronome/click.
- Time signatures.

#### UI

- Light and dark themes.
- Audio settings interface for device selection, buffer-size controls, project sample rate, and audio-memory budget.
- Advanced import/export controls for sample format, bit depth, channel conversion, and dither.
- User-configurable mixing controls after the MVP: pan law, stereo pan/balance modes, clipping protection options, and clip fade lengths and curves.

#### Editing

- Timeline snapping. Snap targets and user controls will be defined during specification.
- Introduce undo/redo progressively, starting with basic track and clip edits.

### Mid-term

- Track grouping with both shared editing controls and mixer buses. Support linked editing of grouped tracks and routing their audio through a group bus for shared mixing controls. Detailed controls and routing behavior will be defined during specification.
- Stem separation.
- Plugin engine.
- MIDI editing and playback.
- Instruments.
- Automation.
- Extend undo/redo coverage to mixing controls and the new editing features introduced after the MVP.

### Long-term

- Time stretching.
- iPadOS and Android delivery.

## Decisions required during specification

These details are not yet fixed:

- Delivery horizon and detailed behavior for the source manager and source inclusion.
- Post-MVP undo/redo rollout: exact coverage per stage, action grouping, and history limits.
