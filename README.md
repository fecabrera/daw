# Desktop DAW

A Rust desktop audio editor with no fixed track-count limit, external WAV sources, and headless WAV export. The project format is experimental.

## Run

Use the Rust toolchain pinned in rust-toolchain.toml. On Linux, install ALSA development libraries and ensure the XDG Desktop Portal service is available for file dialogs. Builds require the platform's native development tools.

The desktop uses egui/eframe 0.36.2 and wgpu 30. The minimum supported Rust version is 1.95; the pinned build toolchain is 1.97.0.

```sh
cargo run -p daw-desktop
```

Create a short project with generated audio and open it:

```sh
python3 tools/create_demo.py
cargo run -p daw-desktop -- examples/demo
```

The implementation session stores downloaded dependencies in work/cargo-home. To reuse that cache, prefix Cargo commands with `CARGO_HOME="$PWD/work/cargo-home"`.

On macOS, create an app bundle with `tools/package_macos.sh debug`. Open `target/debug/DAW.app`, then select a project folder with File > Open. Use `release` instead of `debug` for an optimized build. The local bundle is unsigned.

## Edit

The UI uses bundled Outfit, including the timeline ruler and transport time display. The time display uses semibold at 13 points for all characters. Digits use the default text color and equal-width slots. The `h`, `m`, `s`, and `.` characters use a slightly darker gray and their normal widths. Drag the hours, minutes, or seconds section vertically to seek by that whole unit. The seconds target includes its decimal portion. Up moves forward; down moves backward, carrying or borrowing across units. Hold Shift for tenfold slower adjustment. Fractional seconds are preserved, and backward dragging clamps at the project start. The font works offline. Its SIL Open Font License is included in the source tree and macOS bundle; `daw-desktop --font-license` also prints it. Include this license with desktop distributions.

Transport and zoom use Lucide icons through egui-lucide. The Play button changes to Pause during playback. Play/Pause and Stop have icon-only buttons with tooltips and accessibility names. Horizontal zoom uses Lucide's Move Horizontal icon beside a compact slider anchored to the toolbar's right edge. Icons are bundled and work offline. Include the licenses in crates/ui/assets/icons with distributions; the macOS packaging script copies them into the bundle.

The fixed layout uses charcoal panels, thin borders, compact controls, and purple accents (`#7536A0`). The shared `ACCENT` value in `crates/ui/src/theme.rs` controls accent strokes, knob arcs, playheads, and selection fills. Selected controls and ruler ranges derive their tints from that value. Audio clips inherit their track's color and use contrasting text and waveforms.

Every track stores a default color for its clips. New tracks cycle through the [Material Design 400 shades](https://mui.com/material-ui/customization/color/#color-palette), in this order: Deep Orange, Orange, Amber, Yellow, Lime, Light Green, Green, Teal, Cyan, Light Blue, Blue, Indigo, Deep Purple, Purple, Pink, Red. Track 17 starts the sequence again. Assignment uses the track's position when it is added; adding or deleting tracks preserves existing colors. Projects with missing track colors receive this sequence by display position when loading. Saved colors are preserved, including the original blue palette.

A clip inherits its containing track's color unless it has its own saved RGB override. Moving an inheriting clip to another track adopts that track's color; explicit overrides are preserved. Clip headers, channel fills, text, and waveforms resolve through one palette shared with move/trim and file-drop previews. A drop that creates a track previews the next track's palette color. Core `SetTrackColor` and `SetClipColor` edits support a future color picker; `default_track_color(index)` supplies a track default and `None` restores clip inheritance. Track panels retain their neutral background. Missing-source clips retain their gray warning appearance.

The track section shares the toolbar background and has a vertical divider. The darker timeline fills the remaining workspace, with its scrollbar inside the bottom edge.

Adjacent track blocks share a single 1-point horizontal divider, with no gaps or doubled border strokes.

The ruler's bottom divider extends across the full workspace, including below the Tracks header. It remains visible when the project has no tracks.

The ruler and grid use bars and beats at the project tempo in 4/4. Bars and beats start at 1. The bottom-right selection monitor supports the same bar and beat drags on each endpoint, including beat carry and Shift for slower adjustment. Endpoints clamp to keep the range ordered; an enabled loop retains a nonzero duration. Bar numbers appear at normal zoom; closer zoom shows bar.beat labels, such as 2.3. Bar lines are stronger than beat lines; finer subdivisions appear as space permits. In the ruler, bar lines extend to its top, beat lines stop at the bottom of the loop band, and subdivision ticks remain short. The toolbar adds a bar.beat monitor beside the time display, starting at 0001.01, with the same semibold font, digit widths, and text colors. Drag the bar digits vertically to seek by whole bars, or the beat digits to seek by whole beats. Beats carry forward or backward across bars. Hold Shift for tenfold slower adjustment. Backward dragging clamps at the project start. Tempo changes update this scale and monitor without moving or stretching audio.

The selection range appears at the right edge of the bottom status bar. It shows the ruler selection's start and end in bar.beat format, such as `0002.01–0004.03`, at the project tempo in 4/4. It uses the same format as the playback toolbar's bar.beat monitor, with bars and beats counted from 1. The playback toolbar also shows the playhead time.

Master uses a compact 68-point outlined block with two rows: its name and gain. It stays fixed at the bottom of the track column, above the status bar. Two vertical L/R output bars fit its inner height at the right edge; their red clipping markers can be clicked to clear. It has no Mute or Solo buttons and is separate from the audio tracks. Audio tracks scroll above it.

The header uses one 13-point Outfit label: `[project name] - DAW`, with ` *` after the project name for unsaved changes. The native window title uses the same text.

The window header, playback toolbar, and Tracks header use one reusable toolbar component. They share the panel background, 8-point padding and horizontal spacing, 22-point control height, and themed buttons. Their controls and spacers remain specific to each section.

Rows and toolbars use a shared layout that centers items vertically within the tallest control's height. This includes track and Master controls, the status bar, and dialog button rows.

On macOS, the first toolbar row shares the title-bar area with the native window buttons. Drag the window from the combined title or unused space in that row. A thin border across the full window width separates this header from the playback toolbar. Both rows have uniform 8-point padding, with 8-point horizontal spacing between controls. A single 1-point divider joins the toolbar to the tracks and timeline. Windows and Linux use standard native title bars.

The native macOS menu bar contains the File menu. Windows and Linux show File in the first toolbar row. It contains New project, Open, Import WAV, Save, Save as, Export WAV, and Close project, grouped with separators. Close project is the last item, separated from Export. Use Command on macOS or Ctrl on Windows/Linux: N for New, O for Open, Shift+I for Import, S for Save, Shift+S for Save as, Shift+E for Export, and W for Close project. Closing stops playback, releases the project, and returns to an empty Untitled workspace while the app remains open. Unsaved changes offer Save, Discard, and Cancel. These actions are disabled during background operations or pending confirmation/error dialogs. Export is disabled for an empty timeline.

Unsaved changes, export replacement, and error messages use one reusable application dialog component. It provides a centered title, consistent padding, bounded width, wrapped message text, and a shared button row. Each caller handles its own actions. File and folder pickers use native dialogs through `rfd`.

- The playback toolbar's tempo monitor starts at `120bpm` and uses the time monitor's semibold font, fixed-width digit slots, and darker suffix. Drag up to increase tempo or down to decrease it, by 1 BPM per point. Hold Shift for 0.1 BPM per point. Dragging clamps at 0.01 BPM and rounds to hundredths. Double-click to replace it with an input box. Enter commits a positive finite value; Esc cancels. Focus loss cancels the draft, as with track names. Tempo saves with the project. Invalid values retain the previous tempo. Changing tempo does not move or stretch audio clips.
- Add tracks with the Lucide Plus button at the right of the Tracks header. A flexible spacer separates it from the Tracks label. The Tracks header uses the playback toolbar's 8-point padding on all sides; the ruler matches its 38-point height. Select a track, then use Import WAV to import at the playhead. Dragging a WAV into the timeline shows a translucent preview at the cursor, with its duration and waveform once preparation finishes. Dropping on a track imports at that position; dropping in empty timeline space creates a new track. Red previews indicate unsupported files or overlapping placements. Dragging over the track controls clamps the preview and drop to frame zero on that row, including when the timeline is scrolled. Drops outside the track workspace do not import.
- Drag the middle of a clip's name strip to move it. A translucent preview shows its name and waveform at the drop position while the original clip is dimmed. Drop onto another track to change tracks. Moves stop at the timeline start; dragging farther left keeps the preview and dropped clip at bar 1, beat 1. Overlapping placements show a red outline and are rejected on drop.
- Drag either edge of the name strip to loop the clip. The clip's current trimmed length becomes the repeat length; later resizes retain that base. Both edges show a repeated waveform preview with repeat boundaries. Left extensions preserve the audio's existing timing and stop at frame zero. Partial repeats are allowed above the base length; inward header resizing stops at that length. A clip already shorter than its base after a split or body trim retains its current length as the minimum. Loops save with the project and use the same source range for playback and export, without copying audio.
- Drag either edge below the name strip to trim with the same translucent preview. Normal clips can extend to the available source audio and timeline start. Looped clips can shorten within their current visible range. Returning a loop to its base length restores the original trimmed source range and normal trimming, whether you shrink from the same header edge or the opposite edge. Either body edge can then extend back into the original source. Other ranges that still contain a repeat boundary retain their repeat base. All trims stop at one sample. Extending into another clip shows red and is rejected on release.
- Right-click a clip to split at the playhead or delete it. Right-click a track block to delete that track.
- Double-click a track name to edit it in place. Enter saves the name; Esc restores the previous name. Clicking elsewhere or losing focus cancels the draft and restores the name.
- Each track is 68 points tall, with its name above one control row: M, S, gain knob, gain input, pan knob, pan input. Stereo meters stay at the right edge. Adjust gain and pan with the shared knobs or numeric inputs. Gain knobs mark 0 dB at the top; pan marks center. Drag up or right to increase, hold Shift for fine adjustment, use arrow keys for small steps, or double-click to reset. Gain dragging covers -60 to +12 dB; numeric track gain entry retains its existing finite-value validation. Pan ranges from -1 to 1. Commit numeric inputs with Enter or focus loss; Escape cancels an input change.
- Each track has two vertical L/R level bars at the right edge. Left is the first bar; Right is the second. Levels fill from the bottom; silent bars are dark. Red marks a channel that exceeded 0 dBFS before master gain. Click that channel to clear the warning. Hover to read its current peak level.
- Use Play/Pause or Space to start, pause, and resume. Stop returns to where playback started, including after pause/resume or a seek. Click or drag the ruler's lower tick strip to position the playhead, including dragging its marker. Drag empty space in the upper bar/beat-label strip to select a loop range. Drag the selection's left or right handle to resize it, or its body to move it while preserving its length. Moving left clamps at the track start. Enable looping with the Repeat icon button. The button is highlighted while looping is enabled.
- Clip moves, file drops, trims, playhead positioning, and loop-selection creation, resizing, and movement snap within 6 points of a target. Visible bars take priority, then beats, then visible subdivisions, then source or clip boundaries. Moves and file drops compare both clip edges at each grid priority and use the nearest match, preserving length; fallback targets are other clips on the destination track. A loading file preview snaps only its start until its duration is known. Drops made during loading use the decoded duration and the Shift state at release. Clip header looping prioritizes the trimmed base length and its repetitions before the grid. Hold Shift to bypass snapping, including during a drag. Source bounds, the timeline start, and minimum lengths still apply. Preview and release use the same range.
- Use the zoom slider and horizontal timeline scrollbar. Swipe horizontally with two fingers over the timeline, ruler, or scrollbar to scroll left and right. Scrolling stops at the timeline bounds and leaves the track controls fixed. Drag the zoom slider left to zoom out or right to zoom in. Clips on one track cannot overlap. There is no undo/redo.
- Save to a project folder. Source WAV files remain external; moving the project alone does not include them.

Mono/stereo 16-bit and 24-bit PCM WAV and 32-bit float WAV are supported. Import converts to 48 kHz. Export writes stereo 24-bit WAV with deterministic triangular dither.

Missing sources show empty clips and warnings. Playback and export use silence for those clips. Solo overrides mute. Final output clamps clipping; lower master gain if the clipping indicator appears.

## Headless commands

```sh
cargo run -p daw-cli -- validate --project examples/demo
cargo run -p daw-cli -- render --project examples/demo --output mix.wav
cargo run -p daw-cli -- render --project examples/demo --output mix.wav --overwrite
```

The CLI has no graphics or audio-device dependencies. Missing sources are warnings and do not fail export. Invalid projects, unsupported audio, and processing errors return exit status 1; invalid arguments return 2.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 tools/check_cli.py
```

See [the scope baseline](docs/MVP.md), [the implementation specification](specs/mvp-iteration-1.md), and [validation results](docs/validation.md). Real-device and cross-platform results must be recorded separately from automated tests.
