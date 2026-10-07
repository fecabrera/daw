# Iteration 1 validation

Date: 2026-10-07.

The prototype implements the iteration 1 feature scope. Automated checks and a macOS smoke test pass. Full acceptance on the reference platforms remains open.

## Test environment

| Item | Observed value |
| --- | --- |
| OS | macOS 27.0.1, build 26A434 |
| Hardware | Mac16,7, Apple M4 Pro |
| RAM | 48 GiB |
| Rust | 1.97.0, pinned in rust-toolchain.toml |
| Graphics | wgpu with the native Metal backend enabled |
| Audio output | MacBook Pro Speakers through CPAL/CoreAudio |
| Negotiated output | 48,000 Hz; requested fixed buffer of 512 frames |
| Project used for native smoke test | Four generated stereo tracks, 12 seconds each |

The buffer setting is not a measured end-to-end latency. Actual callback sizes, underrun counters, audible continuity, and play/seek latency were not measured. The 48 GiB machine does not establish the 8 GB requirement.

## Completed checks

- Workspace build, formatting, and strict Clippy pass.
- Twenty-nine Rust tests pass. They cover project constraints, WAV encodings and resampling, transactional clip edits, source preservation, save/reopen and Save As, failed saves, missing and shortened sources, corrupt sources, mixing, clipping, fades, pause/resume and Stop return position, live control ramps, track meters, callback resampling, plan retirement, pointer-driven clip/loop edits, the fixed Master block during scrolling and resizing, and file shortcuts with unsaved-project and background-operation guards. The native adapter owns shortcuts and uses the same guarded file actions.
- `python3 tools/check_cli.py` passes. It checks WAV headers, repeated export bytes, missing-source warnings, the four-track limit, overwrite behavior, and exit codes. Display environment variables are empty during this check.
- A separate CLI build succeeds. Its dependency graph excludes egui, eframe, wgpu, rfd, and CPAL.
- The native macOS window opens the demo through the folder dialog. Track blocks, muted blue stereo clips, light waveforms, ruler, and timeline lanes display correctly. Add track is disabled at four tracks.
- The revised shared style is checked in the native four-track demo. Charcoal panels, thin borders, blue values and selections, Outfit, and the dark native title bar display correctly. The existing layout is retained. All three UI interaction tests and strict Clippy pass with the revised style.
- Compact 96-point rows are checked in the rebuilt native demo. All four tracks fit, Gain and Pan share a row, track blocks align with their lanes, and adjacent tracks have no vertical gaps. Clip movement, cross-track movement, trimming, and loop tests still pass.
- The horizontal timeline scrollbar replaces the seconds slider. Native track clicks move the ruler and lanes while the track controls stay fixed. Returning to the start restores the clips, and zooming in reduces the thumb width. UI tests and strict Clippy pass after the change.
- The track section uses the toolbar background through the full workspace height, with a vertical divider against the darker timeline. The timeline grid and playhead extend below the tracks. Its scrollbar sits inside the viewport's bottom edge. Native checks confirm the appearance with an empty project and the four-track demo; scrolling moves the timeline while the track section stays fixed. The three UI interaction tests and strict Clippy pass.
- Workspace outer padding and section spacing are removed. In the rebuilt native demo, track boxes fill the track column, the ruler meets the toolbar, and track rows meet their timeline lanes without gaps. Internal control padding is retained. UI interaction tests and strict Clippy pass.
- Mute and Solo use the same outlined buttons as the toolbar. Native checks confirm inactive outlines and blue active states, including both toggles enabled together. Temporary preview edits were discarded. UI tests and strict Clippy pass.
- The macOS toolbar shares the native title-bar area, with reserved space for the window buttons. A separate preview confirms the layout with the four-track demo and opens the project dialog from a mouse click in the first toolbar row. UI tests, strict Clippy, and the debug bundle build pass. Window dragging uses the native viewport command; manual drag checks remain open.
- Transport time uses `00h00m00.00s` in a fixed-width field with Outfit digits in equal-width slots. Unit letters and punctuation retain their normal widths. Native playback and Stop confirm that the value advances while adjacent controls keep their positions. The later previews confirm the current font and spacing. Playback toolbar items have no separators or dividers.
- Each track has L/R level and clipping monitors beside its name, using the shared master monitor component. Native playback in a temporary project confirms red track warnings while reduced master gain keeps the output below clipping. Stop returns levels to zero and retains the warnings. Clicking the left channel clears its warning while the right remains red. Engine and output tests verify gain, pan, mute/solo, channel independence, and stable meter identity across track changes. Workspace tests, strict Clippy, and the debug bundle build pass.
- Native playback advances the playhead and stereo meters. Pause holds the position. Stop returns to the playback start. Mute and Solo can both remain enabled.
- Cancel in the unsaved-project prompt retains the current project. Native Save As completes with external sources preserved.
- Native desktop export and CLI export of the saved test project have identical bytes. SHA-256: `bb8f7b1ec69a26e62ba7699f6f426fe64c31da4d3d41527317ae1dc1cbd147bf`.

Native drag automation did not establish clip movement. Separate egui input tests verify header movement, movement between tracks, trimming, and loop selection. Manual interaction on each reference platform is still required.

## Acceptance coverage

| Test | Evidence | Remaining validation |
| --- | --- | --- |
| AT-01: import | Six WAV format/channel combinations, resampling, stereo waveform display | Native mono waveform and import interactions on all platforms |
| AT-02: track capacity | Automated checks for creation, save/reopen, mixing, meters, export, and Add track beyond four tracks | Native playback and performance checks with larger projects on reference platforms |
| AT-03: edits | Transactional core edits and egui movement/trim tests | Native move, trim, split, delete, and overlap feedback |
| AT-04: mixing | Pan/balance, solo precedence, gain targets, fades, headroom, saturated export | Native clipping indicator and audible transition checks |
| AT-05: transport | Sample-counter tests, loop wrap/end behavior, native play/stop | Audible seek/loop continuity and editing during playback |
| AT-06: persistence | Automated save/reopen, Save As and failure; native Save As and Cancel | Native failed-save prompt and close behavior |
| AT-07: missing/changed sources | Preserved ranges, silence, warning, export and reopen tests | Native warning and placeholder inspection |
| AT-08: export | Repeatability, desktop/CLI byte parity, overwrite and empty export | Native overwrite cancellation and failure handling |
| AT-09: headless | Separate CLI build, dependency graph, CLI smoke check | Run on Windows and Linux without a graphical session |
| AT-10: device failures | Error handling is implemented | Physical disconnection and initialization failure |
| AT-11: interface | macOS layout and native folder/export dialogs | Windows 11 and Manjaro GNOME |
| AT-12: reference workload | Not completed | Full workload on reference hardware, including an 8 GB environment |

## Reference workload procedure

1. Load four five-minute stereo sources. This is 20 minutes of decoded audio, approximately 460.8 MB of PCM.
2. Select a loop and play all four tracks for 30 minutes. Scroll, zoom, move clips, and change gain during playback.
3. Record the OS build, hardware, RAM, audio device/backend, negotiated rate, and actual callback sizes. On Manjaro, also record package and GNOME versions.
4. Check for unintended gaps and clicks. Record underruns where the backend exposes a counter. Record unavailable counters as unavailable.
5. Measure command-to-audible play and seek response. The target is at most 50 ms on the reference output setup.
6. Repeat in an 8 GB environment or document a controlled memory test. Record results separately for each platform.

## Desktop dependency upgrade

The desktop uses egui/eframe 0.36.2 and wgpu 30.0.1. The minimum supported Rust version is 1.95; the pinned build toolchain remains 1.97.0. The app entry point and panel composition use the new `Ui` API. File-drop handling and the UI test harness use the updated APIs. All 20 workspace tests and strict Clippy pass. The CLI dependency graph still excludes GUI and audio-device libraries. The old `block` 0.1.6 dependency is no longer present.

The upgraded native macOS preview opens the temporary four-track project and retains the merged title bar, compact tracks, waveforms, and timeline layout. Playback advances the playhead and channel monitors. Stop returns levels to zero while clipping warnings remain, and zoom changes the timeline scale. A pointer-driven UI test verifies scrollbar dragging changes the timeline offset without changing track layout or project data. Windows and Linux native validation remains open.

The Lucide toolbar preview confirms Play, Square, Plus, and Minus icons with no visible labels. The accessibility tree exposes Play, Stop, Zoom in, and Zoom out names and marks Play disabled for an empty project. Tooltips use the same names. All 20 workspace tests and strict Clippy pass. The debug bundle includes the icon and wrapper licenses. Icons use embedded SVG data and the desktop SVG loader; the CLI dependency graph excludes them.

The native macOS File menu exposes six grouped actions and their system keyboard shortcuts. The window has no duplicate File dropdown or individual file-action toolbar buttons. Export is disabled for an empty timeline. Both File > Open and Command+O open the native project-folder dialog; Cancel returns to the unchanged empty project. Native Command+N opens the unsaved-project prompt; Cancel retains a temporary test track. Automated checks confirm that file shortcuts are blocked during a background operation, Save as takes precedence over Save, and native commands use the same guarded actions without duplicate egui shortcut handling. Windows and Linux retain the in-window File menu; native checks on those platforms remain open. All 23 workspace tests and strict Clippy pass. The macOS debug bundle is rebuilt.

The header divider preview confirms a thin border across the full window width between the title row and playback toolbar. It uses the shared border color and the existing space between rows. All seven UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The toolbar padding preview confirms separate 8-point padding around the header and playback row. Controls have equal space above and below, and the divider still spans the full width. The seven UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The header title preview confirms one 13-point DM Sans label reading `Untitled - DAW`. Adding a temporary track changes both the label and native window title to `Untitled * - DAW`. Discarding that test project restores the clean title. The seven UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The toolbar boundary preview confirms a single 1-point divider across the full width where the toolbar meets the tracks and timeline ruler. The toolbar frame's extra outline is removed. Internal 8-point control padding is retained. The seven UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Play/Pause preview starts a temporary four-track project at `00h00m05.00s`. Playback changes the icon and accessibility name to Pause. Clicking Pause holds the time at `00h00m10.97s` across subsequent observations and restores the Play icon. Stop returns to `00h00m05.00s`. Space starts and pauses playback; resuming with Play and then using Stop also returns to the original 5-second start. Engine tests cover seeks, live plan updates, natural completion, and loops. Callback tests cover pause/resume and Stop at 44.1, 48, and 96 kHz device rates. All 25 workspace tests and strict Clippy pass. The macOS debug bundle is rebuilt. Windows and Linux native checks remain open.

The monospace preview confirms that the transport time uses bundled Hack at 13 points. Characters have equal widths, and the full `00h00m00.00s` value fits the existing 112-point field. The remaining UI uses DM Sans. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt and includes the Hack license.

The DM Sans time preview replaces Hack in the time display with 13-point DM Sans. Each glyph is centered in a slot sized for the widest character in `0123456789hms.`. Slot and field widths stay constant as the displayed value changes. The native screenshot confirms that the complete time fits and that thin dividers on both sides span the full playback toolbar height, including its padding. Accessibility still exposes the complete time as one label. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The digit time preview keeps equal-width slots only for digits. The `h`, `m`, `s`, and `.` characters use their normal DM Sans widths. The native screenshot confirms the tighter spacing, complete time value, and both full-height dividers. The field retains a fixed width of at least 112 points. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The time padding preview confirms equal 8-point padding inside both time-display dividers and 8-point spacing to the adjacent Stop and Loop controls. The fixed field width is now derived from the glyph advances and padding, removing the unused space on its right. Digits keep equal-width slots; unit letters and punctuation keep their normal widths. The full-height dividers remain at the field edges. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The playback spacing preview confirms that all dividers between playback toolbar items are removed, including those around the time display and zoom controls. The existing time-display padding, control spacing, and horizontal section borders remain. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Repeat preview confirms that the Loop checkbox is replaced with a Lucide Repeat icon button immediately after Stop. It uses the shared icon button size and styling with no visible label. In a temporary project with a valid loop range, clicking it enables looping, highlights the button, and exposes an accessible on state. Clicking again restores the off state. Empty-range validation and background-operation disabling are retained. All seven UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The track button labels preview confirms visible `M` and `S` labels in equally sized outlined buttons. Both retain their Mute and Solo tooltips and full accessible names with on/off states. Native clicks enable both toggles on a temporary track. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The ruler bands preview confirms a charcoal ruler with separate time-label and tick bands, `0:00`, `0:05`, and `0:10` major labels, shorter minor ticks, and a muted blue selection highlight confined to the lower band. The native preview opens a temporary project with a stored loop range. Automated pointer tests confirm that lower-band drags edit the selection and reject empty ranges; drags starting in the upper band cannot edit it, including when they cross into the lower band. Upper-band clicks still seek. All 26 workspace tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The ruler now uses the same shared background color as the timeline (`#121212`). The separate ruler background constant is removed. The selection highlight and band borders remain. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The ruler selection uses a softer dark blue-gray (`#232E32`) when Loop is disabled. Enabling Loop restores the stronger highlight (`#45636F`). The range and interaction behavior are unchanged. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Master preview confirms a 96-point outlined block fixed at the bottom of the track column, above the status bar. Gain and L/R monitors are removed from the playback toolbar and placed in this block. It has no Mute or Solo buttons. The native empty project shows zero levels; adding a track retains the Master block at the bottom. An automated layout test checks vertical scrolling and window resizing, confirms that Master stays fixed, and preserves the four-track limit. All 27 workspace tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The time weight preview confirms white semibold digits and light gray regular-weight unit letters and decimal point. DM Sans uses its bundled variable font with weight 600 for digits. Digit slots remain equal-width; other characters retain their normal widths. The complete time remains one accessible label. All nine UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The revised time colors preview confirms semibold weight for all time characters. Digits now use the default text color (`#C7C9CC`); unit letters and the decimal point use a slightly darker gray (`#AAAEB3`). Digit slots remain fixed, and unit characters retain their normal widths. All nine UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The vertical meters preview confirms two narrow bars at the right edge of every audio track and Master. Bars use the full 82-point inner height and fill from the bottom. Native playback in the existing meter test project shows live green levels and red clipping states. Stop clears levels to dark bars while retaining red markers at the top. Clicking the left bar clears only its marker; the right marker remains. L/R accessibility names, numeric values, and peak tooltips are retained. Track controls still fit within 96-point rows aligned with their timeline lanes, and Master stays fixed at the bottom. All nine UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The track dividers preview confirms one shared 1-point line between adjacent track blocks, replacing stacked border strokes. Three temporary native track rows retain their 96-point height, internal padding, vertical meters, and alignment with timeline lanes. The selected track retains its blue outline. All nine UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Master divider preview confirms that Master has no bottom border. The status bar supplies the single thin divider at their shared boundary, with no gap. Master's top and side borders, control padding, 96-point height, and bottom-fixed position remain. All nine UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Close project preview confirms the new last item in the native File menu. Command+W opens the unsaved-changes prompt; Cancel retains the temporary track and dirty title. File > Close project followed by Discard clears that track, returns to Ready and Untitled, and keeps the app window open. The menu adapter adds a separator before Close project. Automated checks cover the shared close shortcut, blocked actions during background work, retaining unsaved project data, and closing only after successful save completion. Closing clears the session, selections, meter/output state, and timeline scroll. All 29 workspace tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt. Windows and Linux native checks remain open.

The ruler label background preview confirms that the upper 22-point time-label band uses the toolbar's shared panel color (`#1E1E1E`). The lower tick band retains the timeline background (`#121212`). Band borders, selection highlighting, and ruler interactions remain. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Add track spacer preview confirms a Lucide Plus button with no visible text, aligned to the right of the Tracks header. A flexible spacer separates the left-aligned Tracks label from the button. The shared icon button retains its Add track tooltip and accessible name. Native clicks create four temporary tracks; the button then becomes disabled. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The workspace divider preview confirms a single 1-point line below the ruler and Tracks header across the full workspace width. It remains visible in an empty project and after adding a temporary track. The ruler's separate bottom stroke and the first track's top stroke are replaced by this shared border. Track geometry and ruler interactions remain. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The first-track outline preview confirms a complete blue outline when the first track is selected. The shared divider above it uses the selection accent; the timeline segment stays neutral. Selecting the second track restores the neutral divider above the first. The shared line remains 1 point thick. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The shared dialog preview confirms that Unsaved changes and Error use the same centered frame, title presentation, 8-point content padding and spacing, message layout, and button row. Cancel retains the temporary track and dirty title; Discard closes the temporary project. Opening a temporary invalid manifest shows Error, and OK dismisses it. Export replacement also uses this component. Callers retain their action handling, and unsaved-change actions remain disabled during a background job. Native file and folder pickers remain in `rfd`. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The dialog title preview confirms that the shared title uses the standard body text style: 13-point DM Sans, matching the message and buttons. The native Unsaved changes preview shows the updated title size. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The compact Master preview confirms two control rows in a 62-point block: name and gain. Its L/R meters use the 48-point inner height. Red meter markers retain clipping status and click-to-clear behavior without adding a third text row. Master remains fixed above the status bar with a single thin divider. The scrolling and resize test uses a smaller initial window so four audio tracks still overflow the viewport. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The selection status preview confirms that the selection range appears at the right edge of the bottom bar with a Selection label. The playback toolbar retains the playhead display and no longer shows the range. The range reads the same loop-selection start/end values as before, formatted in seconds to two decimal places. Native automated drag attempts did not change the selection; existing automated ruler interaction tests pass. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Tracks header padding preview confirms the playback toolbar's shared 8-point padding on all sides, around a 22-point control row. The Tracks label stays left, and the Plus button stays right with a flexible spacer. The header and ruler both use a 38-point height to retain one aligned bottom divider. The ruler keeps its 20-point selection band and uses an 18-point label band. Native inspection confirms the padding and alignment. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The ruler font preview confirms smaller 11-point DM Sans time labels, centered in the existing label band. Native inspection shows readable `0:00`, `0:05`, and `0:10` labels. Formatting and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The shared toolbars preview confirms that the window header, playback toolbar, and Tracks header use one reusable component for panel background, 8-point padding and horizontal spacing, and 22-point control height. Icon buttons use the same control-height constant. Native inspection confirms matching styling and section alignment. Four temporary tracks can be added; Add track then becomes disabled. The native window-button clearance, Tracks spacer, and section dividers remain. All 11 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The gain and pan knobs preview confirms a shared 28-point control with dark faces, blue value arcs, light pointers, and fixed top marks for 0 dB and centered pan. Track knobs and numeric fields fit the existing 96-point rows beside the vertical meters. Master remains two rows tall and uses a 68-point block with 54-point meters. Native keyboard checks update track gain to 0.1 dB and pan to 0.01; numeric gain entry updates the knob to -6 dB. Automated pointer tests cover incremental dragging, reduced sensitivity with Shift, and double-click reset to unity gain. Native double-click automation did not trigger reset. All 14 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Outfit preview confirms the bundled font throughout the application UI, including the header, toolbars, track controls, Master, ruler, status bar, and shared Unsaved changes dialog. Regular text uses explicit weight 400; the transport display uses weight 600 with fixed digit slots and natural-width units. Existing font sizes and control layouts remain. The bundled license and `--font-license` output match the official Outfit OFL file, and the macOS bundle no longer contains the former DM Sans license. All 14 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The single control-row preview confirms the track name above M, S, gain knob, gain input, pan knob, and pan input in that order. The row fits within the existing track column beside both vertical meters, with no visible Gain or Pan labels. Numeric inputs retain tooltips and existing validation. Track height remains 96 points. Native macOS inspection confirms the layout. All 14 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The Master gain label is removed. The gain knob and numeric control remain, with a Gain (dB) tooltip on the numeric control. Strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

The zoom slider preview confirms a right-anchored toolbar control with the bundled Lucide Move Horizontal icon on its left. The 90-point slider has a round handle and no visible label or value. It uses a logarithmic range of 1 to 3000 pixels per second. Native clicks changed zoom from 70 to 55 and then 1200, updating the ruler scale without marking the project modified. Native drag automation did not change the value. All 14 UI tests pass; strict Clippy checks pass after the anchoring change. The macOS debug bundle is rebuilt. The native preview also confirms Master has no visible gain label.

The zoom slider width is reduced from 90 to 64 points, with a 4-point rail and a smaller round handle. Its size is scoped to the zoom control; the shared toolbar retains its existing padding and height. Strict Clippy checks pass for the UI and desktop packages. The macOS debug bundle is rebuilt.

The centered track-row preview confirms that M, S, gain knob, gain input, pan knob, and pan input share a vertical center. The row allocates the full 28-point knob height before placing controls. Native macOS inspection confirms the alignment and the compact right-anchored zoom slider. Strict Clippy checks pass for the UI and desktop packages. The macOS debug bundle is rebuilt.

The shared row-centering preview confirms explicit vertical centering in the window header, playback and Tracks toolbars, track name and control rows, Master controls, bottom status bar, and dialog actions. The shared row layout reserves the tallest item height before placement; dialog actions retain wrapping. Native macOS inspection confirms the main UI and Unsaved changes dialog. All 14 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

Clip names now use 11-point Outfit instead of 13-point text. Names are vertically centered in the existing 22-point header strip, retaining 7-point left padding and clipping to the clip bounds. Strict Clippy checks pass for the UI and desktop packages. The macOS debug bundle is rebuilt.

The accent color is now purple #7536A0 (RGB 117, 54, 160), sampled from the latest supplied swatch. Shared controls, numeric values, knob arcs, selection outlines, and playheads use this accent. Selected control fills and enabled/disabled ruler ranges use matching muted purple tints. Strict Clippy checks pass for the UI and desktop packages. The macOS debug bundle is rebuilt.

The matching Master input preview confirms that Master gain uses the same numeric input component as track gain and pan: 36-point width, 11-point Outfit, left-aligned accent text, shared input styling, and 4-point knob spacing. Native entry commits -6 dB and updates the knob. Master edits commit with Enter or focus loss, cancel with Escape, retain the -60 to +12 dB range, and reject non-finite values. A regression test covers commit, Escape cancellation, range clamping, invalid input, and project-close reset. All 15 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

Track gain, track pan, and Master gain boxes now use the default UI text color through the shared numeric input component. Strict Clippy checks pass for the UI and desktop packages. The macOS debug bundle is rebuilt.

Track and clip selections now share the same 2-point purple stroke and 2-point rounded corners, drawn inside their bounds. Selected tracks receive a complete outline; neutral 1-point shared dividers remain. Native macOS inspection confirms the first track has a visible top selection edge and retains its default semibold name color. All 15 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

Audio tracks now use a 68-point block with exactly two control rows: name, then M/S, gain knob and input, and pan knob and input. The vertical meters use the 54-point inner height, matching Master. Timeline lanes use the same reduced height. Native macOS inspection confirms both rows fit with the existing padding and complete selection outline. The scrolling test uses a smaller viewport so four compact tracks overflow. All 15 UI tests and strict Clippy checks for the UI and desktop packages pass. The macOS debug bundle is rebuilt.

## Track capacity

The four-track cap is removed from track creation, project validation, imports, and the Add track button. The status bar no longer reports a maximum. Current requirements specify no fixed track-count limit; earlier four-track rejection checks above describe the previous implementation.

All 35 workspace tests and strict Clippy pass. Core validation accepts 32 tracks. An eight-track workflow imports, saves, reopens, mixes, and exports identical WAV bytes before and after reopening. Output tests verify eight-track playback and stereo meters, then a live update to nine tracks. Pointer-driven UI checks add eight tracks and retain the fixed Master block while scrolling and resizing. The headless CLI smoke check validates and exports an eight-track project. The macOS debug bundle is rebuilt. Larger-project native playback and performance checks on the reference platforms remain open.

## Inline track naming

Double-clicking a track name replaces its label with a centered single-line input using the same 13-point Outfit font and default text color. The current name is selected. Enter commits and restores the label; Escape discards the draft and restores the label. Focus loss retains the input and draft. A changed name marks the project modified; an unchanged name or cancellation does not. Track deletion and project replacement clear the draft.

All 18 UI tests and strict Clippy checks for the UI and desktop packages pass. Pointer-driven tests verify single-click selection, double-click editing, Unicode text entry, Enter, Escape, unchanged commits, draft retention after focus loss, and cleanup on track deletion and project close. The macOS debug bundle is rebuilt. The native preview opens and adds a temporary track, but subsequent automated clicks produce no visible changes, including clicks on Add track and Mute. Native automation therefore does not establish the rename interaction; a manual native check remains open.

## Project tempo input

The playback toolbar has a BPM box after the time monitor, using the shared numeric input style, spacing, and vertical centering. New projects start at 120.0 BPM. Positive finite values commit on Enter or focus loss; Escape restores the previous tempo. Invalid input restores the previous value and shows an error. Tempo saves as tempo_bpm; older manifests without the field load at 120.0. Changing tempo does not move or stretch audio.

All 40 workspace tests, strict Clippy, and the headless CLI smoke check pass. Tests cover tempo validation, save/reopen, older manifests, invalid input, Enter, Escape, focus loss, unchanged values, and reset/load behavior. The macOS debug bundle is rebuilt. Native inspection confirms the box styling and toolbar alignment. Native entry commits 135.5 BPM and marks the project modified; a visible 90.0 draft returns to 135.5 after Escape.

## Musical ruler and transport monitor

The ruler and timeline grid use bars and beats at project tempo in fixed 4/4. Bars and beats count from 1. Normal zoom labels bars; closer zoom labels bar.beat. Ruler ticks and grid lines share one tempo-aware generator with stronger bar lines, lighter beat lines, and adaptive power-of-two subdivisions. A bar.beat monitor starts at 0001.01 beside the time monitor. Both monitors share 13-point semibold Outfit, equal-width digits, default text color, darker punctuation, padding, and vertical centering. Clip positions and durations remain sample-based.

All 22 UI tests and strict Clippy checks for UI and desktop pass. Tests cover exact beat/bar boundaries, fractional tempo, scroll and zoom, bounded tick density at extreme valid tempos, musical ruler seeking, loop selection, and unchanged clip data after tempo edits. The macOS debug bundle is rebuilt. Native inspection confirms aligned ruler/grid positions and matching monitor styling in a temporary demo. Seeking to bar 3 at 120 BPM shows 4.00 seconds and 0003.01. Playback and loop wrap update both monitors; Pause holds them; Stop returns them to the 4-second playback start. Native tempo-edit automation did not establish a tempo change in this preview; tempo-dependent scale and monitor updates are verified by automated tests.

## Tempo monitor and inline editing

Tempo now displays as 120bpm beside the transport monitors. It shares their 13-point semibold Outfit, equal-width digit slots, default digit color, darker units, padding, and vertical centering. Three slots keep one-, two-, and three-digit integer tempos at the same width. Fractional and longer tempos expand without rounding. Double-click replaces the monitor with a centered input of the same width and selects its numeric value. Enter commits; Escape cancels; focus loss retains the draft. Project replacement clears the draft.

All 23 UI tests and strict Clippy checks for UI and desktop pass. Pointer and keyboard tests cover single-click versus double-click, selected-value replacement, valid and invalid commits, Escape, unchanged values, draft retention after focus loss, project reset/load, and stable widths across digit values. The macOS debug bundle is rebuilt. Native inspection confirms the 120bpm monitor and inline input styling. Native entry commits 135.5bpm, changes the ruler scale, and marks the project modified. A visible 90 draft is confirmed; native Escape automation did not establish cancellation in this preview. Cancellation is verified by the pointer/keyboard UI test.

## Centered gain and pan values

The shared numeric input now centers text for track gain, track pan, and Master gain. All 23 UI tests pass, including the existing gain editing checks. The macOS debug bundle is rebuilt.

## Cancel editable labels on focus loss

Track name and tempo editors now use one shared action handler. Escape, widget focus loss, or window focus loss cancels the draft and restores the label or monitor without changing the project. Enter commits even though a single-line input also loses focus on Enter. Disabled inline editors cancel their drafts.

All 24 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests verify clicking elsewhere cancels each editor without modifying its stored value or dirty state. Keyboard and window-focus tests cover Tab and app deactivation for both editors. Existing Enter, Escape, unchanged-value, and invalid-tempo checks pass. The macOS debug bundle is rebuilt.

## Musical selection monitor

The bottom-right Selection monitor now displays both endpoints with the existing bar.beat formatter, such as 0002.01–0004.03. It uses the project tempo in 4/4, counts bars and beats from 1, and updates with ruler selection or tempo changes. Its placement and standard UI style remain unchanged.

All 24 UI tests and strict Clippy checks for UI and desktop pass. Existing formatter tests cover beat/bar boundaries and fractional tempo; ruler tests cover selection changes at project tempo. The macOS debug bundle is rebuilt.

## Clip move preview and timeline start clamp

Moving a clip now dims the original and draws a translucent copy of its name and waveform at the proposed position, including moves between tracks. The overlay uses the shared clip renderer and is clipped to the scrolling timeline viewport. Valid moves use the accent outline; overlapping or overflowing placements use red. Preview and drop share pointer-to-frame conversion and track targeting. Release is handled after all lane positions are current. Leftward movement clamps at frame zero while preserving source offset and duration; trimming retains its existing bounds checks. A window focus loss or disabled workspace clears the drag.

All 27 UI tests and strict Clippy checks for UI and desktop pass. Pointer-driven tests inspect painted preview outlines, confirm the project remains unchanged during dragging, and compare previews with dropped positions at different zoom and scroll values. Same-track and cross-track leftward moves clamp to zero. Overlap previews are red and rejected drops retain both original clips without marking the project modified. Existing movement and trim checks pass. The macOS debug bundle is rebuilt.
