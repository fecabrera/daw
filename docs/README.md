# User guide

Use these guides to arrange audio, build MIDI instruments, and export a mix. For builds, packaging, and platform setup, see the [development guides](development/README.md).

Dropdown and context menus use padded rows and rounded highlights. Unit selectors, including modulation sources, layer submenus and destinations, use smaller text. Their hover background matches unselected view buttons. Selected view and toolbar buttons keep their active background when hovered or pressed. Open a layer submenu to choose its modulation sources. Effects, Components and Chords catalogs use the same row spacing and inset selection highlights.

On Windows and Linux, open the burger menu for File, Edit, Clip, Track, and View. Its categories and submenus use the same design as dropdown menus. Keyboard shortcuts appear beside the actions that support them; unavailable actions appear dimmed.

Right-click an item to open its available actions. Context menus for clips, tracks, unit headers, and editable strips use the same design as dropdown menus. Independent plugin editors use it for their appearance menu and EQ band actions. Escape or a click outside the menu dismisses it without applying an action.

Knob tooltips show only the parameter name on the left and the current value on the right, on one line. They keep a minimum width and expand for longer values. Hover to see the value, or drag a knob to keep the tooltip visible as the value changes, even when the pointer leaves the knob. Releasing ends the drag tooltip. Drag up or right to increase a value, hold Shift for finer adjustment, use arrow keys for small steps, or double-click to reset.

## Views

- [Timeline](views/timeline.md): browse chords, import, arrange, select, trim, loop, split, and copy clips or a time range, and edit clip settings.
- [Instrument](views/instrument.md): build layered sounds and add effects.
- [Editor](views/editor.md): create MIDI patterns, edit notes, and inspect audio clip waveforms.
- [Mixer](views/mixer.md): balance tracks and Master.
- [Beat editor roadmap](views/beat-editor.md): planned step-editing capabilities; not yet available.

## Instrument components

- [Oscillator](instruments/oscillator.md)
- [Granulizer](instruments/granulizer.md)
- [Sampler](instruments/sampler.md)
- [Envelope](instruments/envelope.md)
- [LFO](instruments/lfo.md)
- [Macro](instruments/macro.md)
- [Amplitude](instruments/amplitude.md)
- [Instrument modulation lanes](instruments/modulation-lanes.md)
- [Instrument Output](instruments/main.md)

## Effects

- [Standard unit output meters and held peaks](effects/output-meter.md)
- [Saturator and Bitcrusher CLAP/VST3 previews](effects/distortion-plugins.md)

- [3-band EQ](effects/equalizer.md)
- [Multiband EQ](effects/multiband-eq.md)
- [Drive](effects/drive.md)
- [Drive plugin development builds](effects/drive-plugin.md)
- [Drive v2](effects/drive-v2.md)
- [Drive v2 plugin development builds](effects/drive-v2-plugin.md)
- [Bitcrusher](effects/bitcrusher.md)
- [Saturator](effects/saturator.md)
- [Multiband Exciter](effects/exciter.md)
- [Filter](effects/filter.md)
- [Auto-filter](effects/auto-filter.md)
- [Compressor](effects/compressor.md)
- [OTT](effects/ott.md)
- [Limiter](effects/limiter.md)
- [Direction Mixer](effects/direction-mixer.md)
- [Stereo Spread](effects/stereo-spread.md)
- [Chorus](effects/chorus.md)

## Workflows and limits

- [Modulation](modulation/README.md) and [Oscillator pitch and gain modulation](modulation/oscillator-pitch.md).
- [Track groups](editing/track-groups.md): shared processing buses and nested track organization.
- [Undo and redo](editing/undo-redo.md).
- [Moving and copying units](editing/units.md).
- [Time stretching and offline pitch changes](audio/time-stretch.md).
- [Projects, settings, and export](projects/README.md).
- [Command-line interface](cli/README.md): track inspection, validation, source collection and export.
- [Application Settings](projects/README.md#application-settings): Appearance, Playback, MIDI, and Stem Separation.
- [Audio devices](projects/README.md#audio-devices): output/input selection and output sample rate, bit depth, and buffer size.
- [Seek cache profiles](projects/README.md#seek-cache-profiles): choose background CPU and memory use.
- [Split MIDI notes](projects/README.md#split-midi-notes): choose how a note that crosses the playhead is divided.
- [Features and roadmap](roadmap.md).

## Audio preparation

- [Stem separation](audio/stem-separation.md): local models, four-stem groups, progress, and source retention.

- [Effect plugins](effects/plugins.md): all CLAP/VST3 products, unit slugs, controls, and limits.
