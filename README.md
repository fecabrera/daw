# Desktop DAW

A desktop digital audio workstation for macOS, Windows and Linux, written in Rust. It imports external WAV, MP3, and AAC sources, and shares its rendering engine with a headless command-line interface.

## Current features

- **Timeline:** arrange, trim, loop, stretch, transpose, split, move, copy, and duplicate audio and MIDI clips or a selected time range, with grid snapping, edit previews, clip settings, and a chord catalog.
- **MIDI editor:** create patterns and edit notes in a piano roll, with track controls and optional scale snapping.
- **Instruments:** build layered synthesizers, samplers, and granular instruments, with modulation and a shared Output bus. Browse project tracks and groups from Instrument.
- **Instrument components:** [Oscillator](docs/instruments/oscillator.md), [Sampler](docs/instruments/sampler.md), [Granulizer](docs/instruments/granulizer.md), [Envelope](docs/instruments/envelope.md), [LFO](docs/instruments/lfo.md), [Amplitude](docs/instruments/amplitude.md), and [Macro](docs/instruments/macro.md).
- **Effects:** [3-band EQ](docs/effects/equalizer.md), [Multiband EQ](docs/effects/multiband-eq.md), [Drive](docs/effects/drive.md), [Drive v2](docs/effects/drive-v2.md), [Bitcrusher](docs/effects/bitcrusher.md), [Saturator](docs/effects/saturator.md), [Filter](docs/effects/filter.md), [Auto-filter](docs/effects/auto-filter.md), [Compressor](docs/effects/compressor.md), [OTT](docs/effects/ott.md), [Limiter](docs/effects/limiter.md), [Direction Mixer](docs/effects/direction-mixer.md), [Stereo Spread](docs/effects/stereo-spread.md), and [Chorus](docs/effects/chorus.md).
- **Mixer:** track gain, pan, mute, and solo controls, [track group buses](docs/editing/track-groups.md) with shared effects, and master gain.
- **Playback:** tempo controls, loop playback, stereo level meters, and [audio device settings](docs/projects/README.md#audio-devices) for device selection and new-project quality defaults. Each project saves its own sample rate, output bit depth and buffer size. Input-device selection is saved for future recording.
- **Editing history:** [undo and redo](docs/editing/undo-redo.md) for project edits.
- **Projects:** save and open projects, [collect media sources](docs/projects/README.md#collect-project-sources), and configure audio processing settings.
- **Appearance:** Dark, Light, and Mist themes, accent colors, and colored clips.
- **Export and CLI:** export stereo WAV, MP3, or AAC files; inspect tracks, validate projects, collect sources and render audio with the [CLI](docs/cli/README.md).
- **Standalone audio tools:** experimental offline time stretching and pitch shifting, with three-engine comparisons.

Stem separation supports four or six stems using local HTDemucs models through Charon, with CoreML on Apple Silicon macOS and CPU on supported platforms. Either validated model enables separation; CoreML takes priority, with optional CPU fallback. It creates aligned stem tracks in a group and retains the disabled original clip. See [Stem separation](docs/audio/stem-separation.md) for setup and limits.

## Status and planned work

Audio and MIDI workflows are available in four views: **Timeline**, **Instrument**, **Editor**, and **Mixer**.

The project format is experimental. Normal imports retain external references; included recordings stay inside the project folder. Generated stems use temporary storage before the first save, then stay inside the project folder. Missing audio files remain visible and play as silence.

Planned work includes audio and beat editors, beat tracks, more effects, effects for individual beat lanes, and custom instrument routing. See the [features and roadmap](docs/roadmap.md) for details.

## Documentation

- [User guide](docs/README.md): working with clips, instruments, modulation, and effects.
- [CLI guide](docs/cli/README.md): track inspection, project validation, source collection, export and standalone WAV processing.
- [Development guides](docs/development/README.md): platform setup, build and run steps, packaging, plugin builds, and CI downloads.
- [Features and roadmap](docs/roadmap.md), including current limits and planned capabilities.
- [Time stretching and offline pitch changes](docs/audio/time-stretch.md) and the [standalone comparison tool guide](docs/cli/timestretch.md).
- [Implementation specifications](specs/README.md): architecture, data formats, requirements, and acceptance criteria.
- [Validation records](specs/validation/README.md): completed checks and remaining verification limits.
- [Repository instructions](AGENTS.md): contribution rules and development workflows.

## Licenses

Original code uses the [BSD 2-Clause License](LICENSE). Bundled dependencies have separate terms. Their notices are included with the [media libraries](crates/media/assets/licenses), [fonts](crates/unit-ui/assets/fonts), [icons](crates/ui/assets/icons), [UI dependencies](crates/ui/assets/licenses), and [oscillator dependencies](crates/oscillator/assets/licenses), and [EQ dependencies](crates/equalizer/assets/licenses), and [stem-separation dependencies](crates/separation/assets/licenses).

The [native dependency audit](specs/audio/stem-native-dependencies.md) records platform runtime requirements and license obligations for the combined applications. The original-code BSD license does not replace bundled-library terms or corresponding-source requirements.
