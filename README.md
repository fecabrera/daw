# Desktop DAW

A Rust desktop audio editor with no fixed track-count limit, external WAV sources, and headless WAV/MP3 export. The project format is experimental.

## Run

Install the prerequisites for your platform under [Build](#build), then run from the repository root:

```sh
cargo run -p daw-desktop
```

Create a short project with generated audio and open it:

```sh
python3 tools/create_demo.py
cargo run -p daw-desktop -- examples/demo
```

## Build

Use the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml). Run build commands from the repository root. Builds require the platform's native development tools.

Rubber Band is built from bundled source. Building requires a C++ compiler and Clang/libclang for its Rust bindings; the packaged app needs no separate Rubber Band installation. Third-party license and source notices are in `crates/media/assets/licenses` and the macOS bundle.

The desktop uses egui/eframe 0.36.2 and wgpu 30. The minimum supported Rust version is 1.95; the pinned build toolchain is 1.97.0.

If `work/cargo-home` contains the local dependency cache, set `CARGO_HOME` to that folder before running Cargo. In a POSIX shell, prefix Cargo commands with `CARGO_HOME="$PWD/work/cargo-home"`. This cache is optional.

### Windows

These steps target Windows x64 with the MSVC Rust toolchain. Install the following software before building:

| Software | Required components and purpose |
| --- | --- |
| [Rustup](https://rust-lang.org/tools/install/) | Installs Rust and Cargo. Select the MSVC toolchain. The repository pins the build toolchain in [rust-toolchain.toml](rust-toolchain.toml). |
| [Visual Studio Build Tools](https://learn.microsoft.com/en-us/cpp/overview/acquire-msvc) | Select **Desktop development with C++**, including MSVC x64/x86 build tools and a Windows SDK. These provide `cl.exe`, `link.exe`, headers, and libraries. You can also add this workload to an existing Visual Studio installation. |
| [LLVM](https://github.com/llvm/llvm-project/releases/latest) | Download the **Windows x64 installer** from the official release page. It must include Clang and `libclang.dll`, which bindgen uses to generate the Rubber Band Rust bindings. Install LLVM separately from Rustup. |

[Git for Windows](https://git-scm.com/downloads/win) is needed if you clone the repository. [Python 3](https://www.python.org/downloads/windows/) is needed only for the demo generator and CLI smoke checks. Rubber Band and LAME sources are bundled, so you do not need separate installations of these audio libraries.

#### Configure libclang

1. Install LLVM and check that `libclang.dll` exists, usually at `C:\Program Files\LLVM\bin\libclang.dll`.
2. Open Windows **Edit environment variables for your account**. Add a user variable named `LIBCLANG_PATH` with value `C:\Program Files\LLVM\bin`. If LLVM is installed elsewhere, use the folder that contains the DLL. Set the folder, not the DLL file.
3. Fully close and reopen your terminal and IDE so they receive the new environment variable.

See the [bindgen requirements](https://rust-lang.github.io/rust-bindgen/requirements.html#windows) for LLVM installation details. Setting `LIBCLANG_PATH` does not install the DLL.

#### Build from a terminal

1. Open **x64 Native Tools Command Prompt for Visual Studio**. This configures the compiler, linker, and Windows SDK environment.
2. Change to the repository root.
3. Install the pinned Rust toolchain, then build and run:

   ```bat
   rustup show
   cargo build -p daw-desktop
   cargo run -p daw-desktop
   ```

The debug executable is `target\debug\daw-desktop.exe`. For an optimized build, run `cargo build --release -p daw-desktop`; the executable is `target\release\daw-desktop.exe`. Build the CLI with `cargo build -p daw-cli`.

If you prefer to set libclang only for the current shell, use `set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"` in Command Prompt, or `$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'` in PowerShell, before running Cargo.

#### Troubleshoot build errors

| Error | Check and fix |
| --- | --- |
| `linker link.exe not found` | In the developer command prompt, run `where.exe link` and `where.exe cl`. If either is missing, add or repair the C++ workload through Visual Studio Installer. Adding only the linker folder to `PATH` does not configure SDK headers and libraries. |
| `rubberband-sys`: `Unable to find libclang` | Check that `libclang.dll` exists and that `LIBCLANG_PATH` points to its containing folder. Install Windows x64 LLVM if the DLL is missing. Restart the build application after changing Windows environment variables. |

The `CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG` suggestion adds backtrace detail; it does not fix missing build tools or libclang. Windows build, native UI, and audio-device verification remain separate from these setup instructions.

### macOS

Install Rust through Rustup and the native C++ development tools with Clang/libclang. Build and run with:

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

Create an app bundle with `sh tools/package_macos.sh debug`. Open `target/debug/DAW.app`, then select a project folder with File > Open. Use `release` instead of `debug` for an optimized build. The local bundle is unsigned.

### Linux

Install Rust through Rustup, a C++ compiler, Clang/libclang, and ALSA development libraries. Ensure the XDG Desktop Portal service is available for file dialogs. Package names depend on your distribution.

```sh
cargo build -p daw-desktop
cargo run -p daw-desktop
```

## Edit

The UI uses bundled Outfit, including the timeline ruler and transport time display. The time display uses semibold at 13 points for all characters. Digits use the default text color and equal-width slots. The `h`, `m`, `s`, and `.` characters use a slightly darker gray and their normal widths. Drag the hours, minutes, or seconds section vertically to seek by that whole unit. The seconds target includes its decimal portion. Up moves forward; down moves backward, carrying or borrowing across units. Hold Shift for tenfold slower adjustment. Fractional seconds are preserved, and backward dragging clamps at the project start. The font works offline. Its SIL Open Font License is included in the source tree and macOS bundle; `daw-desktop --font-license` also prints it. Include this license with desktop distributions.

Transport and zoom use Lucide icons through egui-lucide. The Play button changes to Pause during playback. Play/Pause and Stop have icon-only buttons with tooltips and accessibility names. Horizontal zoom uses Lucide's Move Horizontal icon beside a compact slider anchored to the toolbar's right edge. Icons are bundled and work offline. Include the licenses in crates/ui/assets/icons with distributions; the macOS packaging script copies them into the bundle.

The fixed layout uses thin borders, compact controls, and a configurable theme and accent. Dark uses charcoal surfaces. Light uses `#E9E9E9` for the timeline and `#F9F9F9` for panels, toolbars, menus, and dialogs, with dark text and matching borders. Purple 500 (`#9C27B0`) is the default. The shared runtime accent in `crates/ui/src/theme.rs` controls accent strokes, knob arcs, playheads, and selection fills. Selected controls and ruler ranges derive their tints from that value. Audio clips inherit their track's color and use contrasting text and waveforms.

Open Settings from the app-name menu on macOS or File on Windows/Linux. Use Command+, on macOS or Ctrl+, on Windows/Linux. The Theme dropdown offers Dark (default) and Light. The Accent color dropdown offers all 19 [Material Design 500 colors](https://mui.com/material-ui/customization/color/#color-palette), from Red through Blue Grey. Each option shows a color swatch on the left and its name without the shade number. Selecting a theme or accent updates the UI immediately. Open dropdown text and arrows retain the chosen accent color. Close keeps the choice; Escape closes an open dropdown first, then the dialog. The desktop saves these preferences in its application storage on autosave and normal shutdown, then restores it on launch. It applies across projects and does not mark a project modified. Other file, editing, and playback controls are disabled while Settings is open.

Every track stores a default color for its clips. New tracks cycle through the [Material Design 400 shades](https://mui.com/material-ui/customization/color/#color-palette), in this order: Deep Orange, Orange, Amber, Yellow, Lime, Light Green, Green, Teal, Cyan, Light Blue, Blue, Indigo, Deep Purple, Purple, Pink, Red. Track 17 starts the sequence again. Assignment uses the track's position when it is added; adding or deleting tracks preserves existing colors. Projects with missing track colors receive this sequence by display position when loading. Saved colors are preserved, including the original blue palette.

A clip inherits its containing track's color unless it has its own saved RGB override. Moving an inheriting clip to another track adopts that track's color; explicit overrides are preserved. Clip headers, channel fills, text, and waveforms resolve through one palette shared with move/trim and file-drop previews. A drop that creates a track previews the next track's palette color. Core `SetTrackColor` and `SetClipColor` edits support a future color picker; `default_track_color(index)` supplies a track default and `None` restores clip inheritance. Track panels retain their neutral background. Missing-source clips retain their gray warning appearance.

The track section shares the toolbar background and has a vertical divider. The darker timeline fills the remaining workspace, with its scrollbar inside the bottom edge.

Adjacent track blocks share a single 1-point horizontal divider, with no gaps or doubled border strokes.

The ruler's bottom divider extends across the full workspace, including below the Tracks header. It remains visible when the project has no tracks.

The ruler and grid use bars and beats at the project tempo in 4/4. Bars and beats start at 1. The bottom-right selection monitor supports the same bar and beat drags on each endpoint, including beat carry and Shift for slower adjustment. Endpoints clamp to keep the range ordered; an enabled loop retains a nonzero duration. Bar numbers appear at normal zoom; closer zoom shows bar.beat labels, such as 2.3. Bar lines are stronger than beat lines; finer subdivisions appear as space permits. In the ruler, bar lines extend to its top, beat lines stop at the bottom of the loop band, and subdivision ticks remain short. The toolbar adds a bar.beat monitor beside the time display, starting at 0001.01, with the same semibold font, digit widths, and text colors. Drag the bar digits vertically to seek by whole bars, or the beat digits to seek by whole beats. Beats carry forward or backward across bars. Hold Shift for tenfold slower adjustment. Backward dragging clamps at the project start. Tempo changes update the scale and monitor, and move clip starts to retain their beat positions. Clip lengths and source audio remain unchanged.

The selection range appears at the right edge of the bottom status bar. It shows the ruler selection's start and end in bar.beat format, such as `0002.01–0004.03`, at the project tempo in 4/4. It uses the same format as the playback toolbar's bar.beat monitor, with bars and beats counted from 1. The playback toolbar also shows the playhead time.

Master uses a compact 68-point outlined block with two rows: its name and gain. It stays fixed at the bottom of the track column, above the status bar. Two vertical L/R output bars fit its inner height at the right edge; their red clipping markers can be clicked to clear. It has no Mute or Solo buttons and is separate from the audio tracks. Audio tracks scroll above it.

The project title is `[project name] - DAW`, with ` *` after the project name for unsaved changes. Application-drawn titles use one 13-point Outfit label. The native window title uses the same text; Windows displays it with the system title-bar font.

The application header, playback toolbar, and Tracks header use one reusable toolbar component. They share the panel background, 8-point padding and horizontal spacing, 22-point control height, and themed buttons. Their controls and spacers remain specific to each section.

Rows and toolbars use a shared layout that centers items vertically within the tallest control's height. This includes track and Master controls, the status bar, and dialog button rows.

On macOS, the first toolbar row shares the title-bar area with the native window buttons. Drag the window from the combined title or unused space in that row. A thin border across the full window width separates this header from the playback toolbar. Both rows have uniform 8-point padding, with 8-point horizontal spacing between controls. A single 1-point divider joins the toolbar to the tracks and timeline. Linux uses a standard native title bar.

Windows uses the standard native title bar with the project name, unsaved marker, and native Minimize, Maximize/Restore, and Close buttons. Windows handles title-bar dragging, maximizing, and edge resizing. File and Edit occupy the first application toolbar row below the title bar; playback controls occupy the next row. The project name appears only in the native title bar. Closing retains the unsaved-change prompt and is blocked during background jobs.

The native macOS menu bar contains File and Edit menus. Windows shows both menus below the native title bar; Linux shows them in the first toolbar row. File contains New project, Open, Import WAV, Save, Save as, Export, and Close project, grouped with separators. Close project is the last item, separated from Export. Use Command on macOS or Ctrl on Windows/Linux: N for New, O for Open, Shift+I for Import, S for Save, Shift+S for Save as, Shift+E for Export, and W for Close project. Closing stops playback, releases the project, and returns to an empty Untitled workspace while the app remains open. Unsaved changes offer Save, Discard, and Cancel. These actions are disabled during background operations or pending confirmation/error dialogs. Export is disabled for an empty timeline. Edit contains Copy and Paste with Command+C/Command+V on macOS or Ctrl+C/Ctrl+V on Windows/Linux. Copy requires a selected clip; Paste requires a copied clip and a remaining track. Clip menu actions are disabled while editing text, dragging, or when the window has no focus.

Project naming, export options, export status, unsaved changes, export replacement, and error messages use one reusable application dialog component. It provides a centered title, consistent padding, bounded width, wrapped message text, and a shared button row. Each caller handles its own actions. File and folder pickers use native dialogs through `rfd`.

- The playback toolbar's tempo monitor starts at `120bpm` and uses the time monitor's semibold font, fixed-width digit slots, and darker suffix. Drag up to increase tempo or down to decrease it, by 1 BPM per point. Hold Shift for 0.1 BPM per point. Dragging clamps at 0.01 BPM and rounds to hundredths. Double-click to replace it with an input box. Enter commits a positive finite value; Esc cancels. Focus loss cancels the draft, as with track names. Tempo saves with the project. Invalid values retain the previous tempo. Changing tempo keeps clip starts at the same beat positions, rounded to the nearest sample frame. It preserves clip lengths, source offsets, repeats, and stretch ratios. A change that would make clips overlap on the same track or exceed timeline bounds is rejected; the tempo and all clips retain their last valid state. The loop selection also keeps both endpoints at their beat positions, including when Loop is disabled. Its endpoints round up to sample frames as with ruler snapping. Invalid selection bounds reject the whole tempo change. The playhead frame stays unchanged.
- Add tracks with the Lucide Plus button at the right of the Tracks header. A flexible spacer separates it from the Tracks label. The Tracks header uses the playback toolbar's 8-point padding on all sides; the ruler matches its 38-point height. Select a track, then use Import WAV to import at the playhead. Dragging a WAV into the timeline shows a translucent preview at the cursor, with its duration and waveform once preparation finishes. Dropping on a track imports at that position; dropping in empty timeline space creates a new track. Red previews indicate unsupported files or invalid ranges. Drops trim overlapping neighbors, remove fully covered clips, and split a surrounding clip into two pieces. Dragging over the track controls clamps the preview and drop to frame zero on that row, including when the timeline is scrolled. Drops outside the track workspace do not import.
- Select one clip with a click. Shift-click (or Command-click on macOS / Ctrl-click on Windows/Linux) adds or removes a clip. Click and hold in empty timeline space, then drag a box to select all clips it touches across tracks; hold Shift to add unselected clips and remove selected clips. Clips outside the box keep their selection state. Escape or focus loss cancels the box. A plain click replaces the selection; clicking empty space clears it.
- Drag the middle of a selected clip's name strip to move the full selection. Relative timing and track spacing stay fixed. Every clip has a preview; one invalid placement rejects the whole move. A translucent preview shows its name and waveform at the drop position while the original clip is dimmed. Drop onto another track to change tracks. Hover below the last track to preview a new row; dropping there creates a track. For a selected group, create enough tracks to preserve its track gaps. The preview uses the new tracks' default colors, and drag duplication works there too. Tracks are created only on a successful drop. Moves stop at the timeline start; dragging farther left keeps the preview and dropped clip at bar 1, beat 1. Moves take priority over existing destination clips: trim their covered edge, remove fully covered clips, or split a surrounding clip into left and right pieces. Previews show all affected ranges; changes commit together on mouse release.
- Drag either edge of a selected clip's name strip to loop the full selection. Apply the same snapped edge movement to each clip, retaining its own repeat length and phase. Clamp the group at timeline bounds and each clip's minimum length. Preview all results, including neighbor trims and removals. Changes commit together only on mouse release. Hold Shift to bypass snapping; Escape or focus loss cancels. The clip's current trimmed length becomes the repeat length; later resizes retain that base. Both edges show a repeated waveform preview with repeat boundaries. Left extensions preserve the audio's existing timing and stop at frame zero. Partial repeats are allowed above the base length; inward header resizing stops at that length. A clip already shorter than its base after a split or body trim retains its current length as the minimum. Loops save with the project and use the same source range for playback and export, without copying audio.
- Drag either edge below the name strip to trim with the same translucent preview. Normal clips can extend to the available source audio and timeline start. Looped clips can shorten within their current visible range. Returning a loop to its base length restores the original trimmed source range and normal trimming, whether you shrink from the same header edge or the opposite edge. Either body edge can then extend back into the original source. Other ranges that still contain a repeat boundary retain their repeat base. Ordinary single-clip trims stop at one sample. With multiple clips selected, either waveform edge trims the whole selection by the same snapped amount. Ordinary group trims reject shortening that would remove any clip. Expansion uses the same requested amount and clamps each clip independently at its source and timeline limits. A clip at its limit does not stop other clips expanding. Extending into another clip trims its overlapping edge to make room. Fully covered clips are removed on release. Hold Shift to bypass snapping; Escape or focus loss cancels.
- Start a waveform drag on the dividing line between two contiguous clips to move their shared boundary: trim one and extend the other, keeping the pair's outer endpoints fixed. Clamp movement to both clips' available source audio and remove either clip when fully covered. Starting more than 3 logical points inside either clip keeps ordinary trimming. The gesture is chosen at the initial press and does not change when the cursor crosses the line. Snap to the grid and source boundaries; Shift bypasses snapping. Preview both clips and commit only on release; Escape or focus loss cancels. If both are selected, treat them as one boundary pair. Other selected clips use the same requested delta with ordinary trim limits; reject the full edit if any of those clips would become empty. Header looping and modifier stretching retain their existing behavior.
- Trim extensions and header looping can trim selected or unselected neighbors on the same track. Right-edge expansions give earlier clips priority; left-edge expansions give later clips priority. Trim only the covered edge, preserving the remaining source audio and loop phase. Remove a clip if its entire resulting range is covered. Previews show all affected ranges; the project and playback keep their original ranges until mouse release. Escape or focus loss cancels the whole edit. Surviving clips keep their IDs, source references, stretch settings, and colors; remove deleted clips from the selection. Menu imports, paste, and drag duplication still reject overlap.
- Right-click a clip to split at the playhead or delete it. Right-click a track block to delete that track.
- Hold Option on macOS or Ctrl on Windows/Linux when starting a drag from either clip edge to stretch all selected clips without changing pitch. This works on the header and waveform edges; an ordinary edge drag still loops or trims. Each opposite edge stays fixed. The dragged edge snaps once, and its duration ratio applies to every selected clip, rounded to sample frames. Waveform previews show all proposed durations; hold Shift to bypass snapping. The common ratio clamps at every clip's timeline limits and between 1/8 and 8 times its original source duration, including existing stretching. Stretching takes priority over overlapping clips: trim their covered edge and remove fully covered clips. For selected clips that overlap each other, earlier clips take priority on right-edge drags and later clips on left-edge drags. Previews show neighbor trims and removals. Escape or focus loss cancels the drag. On release, Rubber Band R3/Finer processes the original source on a worker; the stretch, neighbor trims, and removals apply together only after all buffers are ready. The project and playback retain their original ranges until release and successful processing. Processing failure keeps all original clips and prepared audio. Playback and export share the cached result. Original sources remain available for later trimming, and stretch settings survive looping, splitting, copying, saving, and reopening.
- Hold Option on macOS or Ctrl on Windows/Linux while dragging a selected clip name strip to duplicate the full selection. The originals stay in place; each copy preserves its name, trim, loop, and color and shares the source audio. The preview uses normal move snapping; hold Shift as well to bypass it. Changing the copy modifier during a drag updates the preview and drop action. Overlap, including overlap with the original, is rejected.
- Select clips and press Command+C on macOS or Ctrl+C on Windows/Linux to copy the selection. Command+V or Ctrl+V pastes it with the earliest clip at the playhead and the top copied track on the selected track. Relative timing and track gaps are preserved; missing destination tracks are added. Paste selects all new clips. Deletion removes all selected clips. Copies preserve the name, trim, loop base and phase, and color override while sharing the source audio. Repeated pastes create distinct clips; overlap is rejected. Text fields keep their normal copy/paste behavior. The clip clipboard clears when a project is opened, closed, or replaced.
- Click anywhere in a track panel to select that track, including its padding, controls, and meters. Controls retain their normal actions; selecting a track keeps the current clip selection. Drag the left Lucide grip or a track name up or down to reorder the whole track. The insertion line shows the drop position; holding near the top or bottom scrolls the list. Release to commit, or press Esc to cancel. Track colors, clips, and controls stay with their track, and the master stays pinned. Double-click a track name to edit it in place. Enter saves the name; Esc restores the previous name. Clicking elsewhere or losing focus cancels the draft and restores the name.
- Each track is 68 points tall, with its name above one control row: M, S, gain knob, gain input, pan knob, pan input. Stereo meters stay at the right edge. Adjust gain and pan with the shared knobs or numeric inputs. Gain knobs mark 0 dB at the top; pan marks center. Drag up or right to increase, hold Shift for fine adjustment, use arrow keys for small steps, or double-click to reset. Gain dragging covers -60 to +12 dB; numeric track gain entry retains its existing finite-value validation. Pan ranges from -1 to 1. Commit numeric inputs with Enter or focus loss; Escape cancels an input change.
- Each track has two vertical L/R level bars at the right edge. Left is the first bar; Right is the second. Levels fill from the bottom; silent bars are dark. Red marks a channel that exceeded 0 dBFS before master gain. Click that channel to clear the warning. Hover to read its current peak level.
- Use Play/Pause or Space to start, pause, and resume. Stop returns to where playback started, including after pause/resume or a seek. Click or drag the ruler's lower tick strip to position the playhead, including dragging its marker. Drag empty space in the upper bar/beat-label strip to select a loop range. Drag the selection's left or right handle to resize it, or its body to move it while preserving its length. Moving left clamps at the track start. Enable looping with the Repeat icon button. The button is highlighted while looping is enabled.
- Clip moves, file drops, trims, playhead positioning, and loop-selection creation, resizing, and movement snap within 6 points of a target. Visible bars take priority, then beats, then visible subdivisions, then source or clip boundaries. Moves and file drops compare both clip edges at each grid priority and use the nearest match, preserving length; fallback targets are other clips on the destination track. A loading file preview snaps only its start until its duration is known. Drops made during loading use the decoded duration and the Shift state at release. Clip header looping prioritizes the trimmed base length and its repetitions before the grid. Hold Shift to bypass snapping, including during a drag. Source bounds, the timeline start, and minimum lengths still apply. Preview and release use the same range.
- Use the zoom slider and horizontal timeline scrollbar. Swipe horizontally with two fingers over the timeline, ruler, or scrollbar to scroll left and right. Scrolling stops at the timeline bounds and leaves the track controls fixed. Pinch with two fingers over the timeline, ruler, or scrollbar to zoom around the pointer. Spread to zoom in; pinch inward to zoom out. The point under the pointer stays fixed where timeline bounds permit. Drag the zoom slider left to zoom out or right to zoom in. Clips on one track cannot overlap. There is no undo/redo.
- On the first Save, enter the project name, then choose a parent location in the system folder picker. The app creates `<location>/<project name>/project.json` and an `assets` folder. Save As uses the same flow, starting with the current name, and switches to the new named folder after success. Regular Save writes to the current folder. Invalid names and existing destination folders are rejected. Cancelling or failing leaves the current project and unsaved changes intact. Source WAV files remain external; moving the project alone does not include them.

Mono/stereo 16-bit and 24-bit PCM WAV and 32-bit float WAV are supported. Import converts to 48 kHz. Export opens a format dialog, then a native destination chooser. Choose WAV with 16-bit PCM, 24-bit PCM (default), or 32-bit IEEE float, or MP3 with MPEG Layer III (LAME) and 128, 192 (default), 256, or 320 kbps constant bitrate. Output is stereo at 48 kHz. Integer WAV uses deterministic triangular dither; float WAV preserves headroom without dither. The app remembers the last accepted options for the current app session. Cancel leaves the project unchanged. MP3 uses a bundled encoder; no separate FFmpeg or LAME installation is needed. Its license and source notices are included in the macOS bundle.

Once the destination is chosen and replacement is confirmed if needed, an Export status dialog shows live rendering/encoding progress, file finalization, destination, format, and warnings. It stays open with a completion notice or export error until Close or Esc. Long reports scroll. Export diagnostics appear in this dialog; editing and new file actions stay blocked while it is open. Progress reaches 100% only after the finished file is published.

Missing sources show empty clips and warnings. Playback and export use silence for those clips. Solo overrides mute. Device output, integer WAV, and MP3 clamp clipping; float WAV preserves headroom and reports levels above 0 dBFS. Lower master gain if the clipping indicator appears.

## Headless commands

```sh
cargo run -p daw-cli -- validate --project examples/demo
cargo run -p daw-cli -- render --project examples/demo --output mix.wav
cargo run -p daw-cli -- render --project examples/demo --output mix.wav --overwrite
cargo run -p daw-cli -- render --project examples/demo --output mix.wav --codec float32
cargo run -p daw-cli -- render --project examples/demo --output mix.mp3 --format mp3 --bitrate 320
```

The CLI infers WAV or MP3 from the output extension unless `--format wav|mp3` is supplied. WAV accepts `--codec pcm16|pcm24|float32`; MP3 accepts `--bitrate 128|192|256|320`. Incompatible options and mismatched format extensions are rejected. The CLI has no graphics or audio-device dependencies. Missing sources are warnings and do not fail export. Invalid projects, unsupported audio, and processing errors return exit status 1; invalid arguments return 2.

## Checks

Repository contributors and coding agents should follow [AGENTS.md](AGENTS.md) for architecture, development, documentation, and validation rules.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 tools/check_cli.py
```

See [the scope baseline](docs/MVP.md), [the implementation specification](specs/mvp-iteration-1.md), and [validation results](docs/validation.md). Real-device and cross-platform results must be recorded separately from automated tests.
