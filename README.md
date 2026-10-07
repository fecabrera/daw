# Desktop DAW

A Rust desktop audio editor with a four-track limit, external WAV sources, and headless WAV export. The project format is experimental.

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

The UI uses bundled Outfit, including the timeline ruler and transport time display. The time display uses semibold at 13 points for all characters. Digits use the default text color and equal-width slots. The `h`, `m`, `s`, and `.` characters use a slightly darker gray and their normal widths. The font works offline. Its SIL Open Font License is included in the source tree and macOS bundle; `daw-desktop --font-license` also prints it. Include this license with desktop distributions.

Transport and zoom use Lucide icons through egui-lucide. The Play button changes to Pause during playback. Play/Pause and Stop have icon-only buttons with tooltips and accessibility names. Horizontal zoom uses Lucide's Move Horizontal icon beside a compact slider anchored to the toolbar's right edge. Icons are bundled and work offline. Include the licenses in crates/ui/assets/icons with distributions; the macOS packaging script copies them into the bundle.

The fixed layout uses charcoal panels, thin borders, compact controls, and purple accents (`#7536A0`). Selected controls and ruler ranges use matching purple tints. Audio clips use muted blue fills and light waveforms. Shared visual settings are in crates/ui/src/theme.rs.

The track section shares the toolbar background and has a vertical divider. The darker timeline fills the remaining workspace, with its scrollbar inside the bottom edge.

Adjacent track blocks share a single 1-point horizontal divider, with no gaps or doubled border strokes.

The ruler's bottom divider extends across the full workspace, including below the Tracks header. It remains visible when the project has no tracks.

The selection range appears at the right edge of the bottom status bar. It shows the ruler selection's start and end in seconds to two decimal places. The playback toolbar shows the playhead time.

Master uses a compact 68-point outlined block with two rows: its name and gain. It stays fixed at the bottom of the track column, above the status bar. Two vertical L/R output bars fit its inner height at the right edge; their red clipping markers can be clicked to clear. It has no Mute or Solo buttons and does not count toward the four-track limit. Audio tracks scroll above it.

The header uses one 13-point Outfit label: `[project name] - DAW`, with ` *` after the project name for unsaved changes. The native window title uses the same text.

The window header, playback toolbar, and Tracks header use one reusable toolbar component. They share the panel background, 8-point padding and horizontal spacing, 22-point control height, and themed buttons. Their controls and spacers remain specific to each section.

Rows and toolbars use a shared layout that centers items vertically within the tallest control's height. This includes track and Master controls, the status bar, and dialog button rows.

On macOS, the first toolbar row shares the title-bar area with the native window buttons. Drag the window from the combined title or unused space in that row. A thin border across the full window width separates this header from the playback toolbar. Both rows have uniform 8-point padding, with 8-point horizontal spacing between controls. A single 1-point divider joins the toolbar to the tracks and timeline. Windows and Linux use standard native title bars.

The native macOS menu bar contains the File menu. Windows and Linux show File in the first toolbar row. It contains New project, Open, Import WAV, Save, Save as, Export WAV, and Close project, grouped with separators. Close project is the last item, separated from Export. Use Command on macOS or Ctrl on Windows/Linux: N for New, O for Open, Shift+I for Import, S for Save, Shift+S for Save as, Shift+E for Export, and W for Close project. Closing stops playback, releases the project, and returns to an empty Untitled workspace while the app remains open. Unsaved changes offer Save, Discard, and Cancel. These actions are disabled during background operations or pending confirmation/error dialogs. Export is disabled for an empty timeline.

Unsaved changes, export replacement, and error messages use one reusable application dialog component. It provides a centered title, consistent padding, bounded width, wrapped message text, and a shared button row. Each caller handles its own actions. File and folder pickers use native dialogs through `rfd`.

- Add up to four tracks with the Lucide Plus button at the right of the Tracks header. A flexible spacer separates it from the Tracks label. The Tracks header uses the playback toolbar's 8-point padding on all sides; the ruler matches its 38-point height. Select a track, then import a WAV at the playhead. Dropping a WAV onto the window imports it into the selected track.
- Drag a clip's name strip to move it. Drop onto another track to change tracks. Drag either edge to trim.
- Right-click a clip to split at the playhead or delete it. Right-click a track block to delete that track.
- Each track is 68 points tall, with its name above one control row: M, S, gain knob, gain input, pan knob, pan input. Stereo meters stay at the right edge. Adjust gain and pan with the shared knobs or numeric inputs. Gain knobs mark 0 dB at the top; pan marks center. Drag up or right to increase, hold Shift for fine adjustment, use arrow keys for small steps, or double-click to reset. Gain dragging covers -60 to +12 dB; numeric track gain entry retains its existing finite-value validation. Pan ranges from -1 to 1. Commit numeric inputs with Enter or focus loss; Escape cancels an input change.
- Each track has two vertical L/R level bars at the right edge. Left is the first bar; Right is the second. Levels fill from the bottom; silent bars are dark. Red marks a channel that exceeded 0 dBFS before master gain. Click that channel to clear the warning. Hover to read its current peak level.
- Use Play/Pause or Space to start, pause, and resume. Stop returns to where playback started, including after pause/resume or a seek. Click the ruler to seek. Drag the ruler's lower tick strip to select a loop range, then enable looping with the Repeat icon button. The upper time-label strip cannot edit the selection. The button is highlighted while looping is enabled.
- Use the zoom slider and horizontal timeline scrollbar. Drag the slider left to zoom out or right to zoom in. Clips on one track cannot overlap. There is no snapping or undo/redo.
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

See [the scope baseline](MVP.md), [the implementation specification](specs/mvp-iteration-1.md), and [validation results](docs/validation.md). Real-device and cross-platform results must be recorded separately from automated tests.
