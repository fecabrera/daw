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

## Timeline file-drop preview

Dragging a WAV over the timeline shows a translucent clip at the cursor's sample position on the hovered track. The preview initially shows the file name and a loading message, then the actual duration and waveform prepared on a background worker. Empty timeline space previews the next track row and creates that track only on drop. Preview and import share the destination and position, including zoom and scroll. Decoded samples and peaks are reused on import. Hover cancellation clears the cache without changing the project; invalid files and overlaps show red outlines and failed drops retain the project. Drops outside the timeline are ignored. Menu imports retain selected-track/playhead placement. Background operations and dialogs block file drops.

Workspace tests, including all 32 UI tests, and strict workspace Clippy pass. File-event tests inspect preview outlines and waveform shapes, verify cursor tracking at non-default zoom/scroll, assert prepared samples are reused, and cover empty projects, direct drops, cancellation, overlaps, invalid files, and ignored drops outside the timeline. Project import/save/export regressions pass. The macOS debug bundle is rebuilt. Native cursor polling adapters account for missing file-drag motion events on macOS, Windows, and X11. Native Finder drag verification and Windows/X11 compilation and drag checks remain open. Wayland file hover/drop remains limited by the current winit backend, which does not emit those events.

## File drag over track controls

File drags over the track controls now target the hovered row at frame zero. Preview and import use that same position, regardless of horizontal scroll. Empty space in the track column follows the existing new-track behavior at frame zero. The preview stays clipped to the timeline. Toolbar, ruler, Master, and out-of-window drops remain ignored.

All 33 UI tests and strict Clippy checks for UI and desktop pass. File-event tests verify the preview outline and imported position after moving from the timeline into the controls at two zoom/scroll settings. Direct drops over the controls also import at zero with a scrolled timeline. Tests cover ignored drops on the toolbar, ruler, Master, and outside the window. The macOS debug bundle is rebuilt. Native Finder drag verification remains open.

## Selection monitor styling

The bottom-right selection endpoints now use the shared transport monitor renderer. Both positions have semibold text and fixed-width digit slots, with default-colored digits and darker periods and dash. The Selection label retains the standard UI font. All 33 UI tests and strict Clippy checks for UI and desktop pass. The macOS debug bundle is rebuilt.

## Tempo monitor dragging

The BPM monitor supports primary-button vertical dragging: up increases tempo and down decreases it by 1 BPM per point, or 0.1 BPM with Shift. Shift can change during the drag. Changes use incremental pointer motion, round to hundredths, and clamp at 0.01 BPM. Horizontal motion and stationary frames do not change tempo. Double-click numeric editing remains available. Background jobs disable dragging.

All 35 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests verify both directions, Shift transitions, stable stationary frames, the positive minimum, disabled dragging during a job, and unchanged clip placement and duration. Existing double-click, Enter, Escape, and focus-loss tests pass. The macOS debug bundle is rebuilt.

## Bar and beat monitor dragging

The toolbar's bar.beat monitor retains the shared transport styling and now has separate vertical drag targets for bars and beats. Each point moves by one whole bar or beat, with upward motion seeking forward. Shift slows movement tenfold while accumulating partial motion until a whole unit is reached. Beats carry and borrow across bars. Seeking uses project tempo and preserves position within the beat to sample precision. Backward movement clamps at frame zero; reversing direction then advances immediately. The existing audio seek path updates playback. Release, window focus loss, disabled controls, and new/open/close project actions clear the drag state.

All 38 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests verify both drag targets, carry and borrow, fractional tempo, preserved beat phase, retained drag units when crossing sections, stationary and horizontal motion, accumulated fine motion, start clamping and reversal, and disabled dragging during a job. Beat conversion tests cover fractional boundaries and sample precision. Clip placement and duration remain unchanged. The macOS debug bundle is rebuilt.

## Selection endpoint dragging

Both endpoints of the selection monitor now share the toolbar monitor's drag handler. Each endpoint has separate bar and beat targets, with whole-unit vertical adjustment, carry and borrow, accumulated Shift motion, and preserved beat phase. The shared renderer supplies digit group bounds for all four targets without changing its styling. Endpoints clamp to keep the range ordered; enabled loops retain at least one sample of duration. Changes mark the project modified and sync playback while preserving the loop toggle and playhead. Release, focus loss, project reset/load, and disabled controls clear the drag state. Background jobs disable editing.

All 41 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests cover all four targets at fractional tempo, stationary and horizontal motion, endpoint independence, Shift accumulation, creation of a selection with Loop disabled, clamping with Loop enabled and disabled, immediate reversal at limits, and blocked adjustment during a job. Existing toolbar drag and tempo editing tests pass after sharing the handler. Clip positions and durations remain unchanged. The macOS debug bundle is rebuilt.

## Time monitor dragging

The time monitor has separate vertical drag targets for hours, minutes, and seconds. Each target includes its suffix; the seconds target also includes the decimal fraction. Up seeks forward and down seeks backward by whole units, with automatic carry and borrow. Shift slows adjustment tenfold, accumulating motion until a whole unit is reached. Fractional seconds remain exact in sample frames. The shared monitor drag handler now supports musical beats and exact frame increments. Seeking clamps at frame zero and saturates at the maximum frame, with immediate reversal at limits. Disabled controls, release, focus loss, and new/open/close project actions clear the drag state.

All 43 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests cover each time target, carry and borrow at hour boundaries, expansion from 99 to 100 hours, retained drag units across sections, exact fractional-frame preservation, stationary and horizontal motion, accumulated Shift adjustment, clamping and reversal, and blocked dragging during a job. Frame conversion checks cover both limits. Existing bar/beat and selection drag tests pass after sharing the handler. Clip placement, duration, tempo, and loop state remain unchanged. The macOS debug bundle is rebuilt.

## Ruler selection and playhead bands

The upper bar-label band now displays the loop selection and supports creation in empty space, resizing from either edge, and moving the body. Visible endpoint handles have five-point hit areas. Resizing retains at least one sample of duration; moving preserves duration and clamps within the sample range. The lower tick band supports playhead clicks and continuous dragging, including its visible marker. The ruler playhead line and marker are confined to that band. Each gesture retains its original role when crossing bands. Selection previews sync during dragging and mark the project modified on release. Disabled controls or window focus loss restore the original range and dirty state.

All 46 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests cover creation at project tempo, both edge handles, body movement, zero clamping, sample-valid resize limits, upper/lower role retention, live marker dragging, and cancellation on focus loss or background operations. Shape checks confirm upper-band highlighting and visible handles at different zoom and scroll settings. Loop toggle state, clip positions, and durations are preserved. The macOS debug bundle is rebuilt.

## Clip trim previews and bounds

Both clip trim edges now use the shared move/trim proposal and preview renderer. The original is dimmed while a translucent name and waveform show the proposed start, source offset, and duration. The left edge preserves the right endpoint and clamps to source offset zero, timeline frame zero, and one sample of duration. The right edge preserves start and source offset and clamps to the saved decoded source length, maximum timeline frame, and one sample of duration. Trims remain on the original track. Preview and release share the same range calculation. Overlap previews are red and rejected releases retain the original. Unchanged clamped releases do not mark the project modified.

All 49 UI tests and strict Clippy checks for UI and desktop pass. Pointer tests inspect preview outlines and real waveform shapes for both edges at different zoom and scroll settings, verify previews match committed source ranges, and confirm decoded samples are reused. Bound checks cover source start/end, timeline start, trimming past the opposite edge, and unchanged clamped releases. Both overlap directions retain the original clip. Existing clip movement, ruler, and monitor tests pass. The macOS debug bundle is rebuilt.

## Ruler line heights

Bar lines now extend through both ruler bands to the top of the header. Beat lines extend through the lower tick band and stop at the bottom of the upper loop band. Subdivision ticks retain their short height. All 49 UI tests and strict Clippy checks for UI and desktop pass. The macOS debug bundle is rebuilt.

## Shared accent value

`theme::ACCENT` is the single accent value. Selected control fills and active/inactive ruler selection fills now derive from it by blending with neutral theme surfaces. Accent strokes, knob arcs, playheads, and handles already use that shared value. The accent remains #7536A0; the derived fills retain similar muted shades. All 49 UI tests and strict Clippy checks for UI and desktop pass. The macOS debug bundle is rebuilt.

## Track and clip color support

Every track stores an opaque RGB color for its clips, initialized from the hardcoded #2B5163 default. Older manifests without track colors receive that default; null track colors are rejected. Clips inherit their containing track's color unless they have an explicit RGB override. Track panels keep the neutral background. The default retains the exact existing blue palette. Core edit commands support future color controls. The clip renderer and all drag previews share one resolved palette, with derived header/channel shades and contrasting text and waveform colors. Moves use the destination track's color for inheriting clips and preserve explicit overrides. File-drop previews inherit the target track's color, including their loading placeholder. Missing sources keep their gray warning appearance.

All 74 workspace tests and strict workspace Clippy checks pass. Persistence tests cover save/reopen, older manifests, reset to defaults, invalid RGB values, required track colors, clip overrides, transactional errors, override preservation during move/trim/split, and unchanged audio. Shape tests cover current defaults, dark and light clip overrides, neutral track panels, inherited colors, headers, stereo channel fills, text, waveforms, move previews, committed colors, reset, and missing sources. Pointer tests verify inheritance and overrides across track moves and subsequent track color edits, plus loading/decoded file-drop previews and imported clips. The macOS debug bundle is rebuilt.

## Material 400 track defaults

New tracks cycle through the 16 Material Design 400 colors from Deep Orange to Red, in the requested right-to-left order. Assignment uses the track count before creation and repeats after 16 hues. Existing tracks retain their assigned colors when tracks are added or deleted. Projects without saved track colors receive the palette by display position; saved colors and clip overrides remain intact. The original saved blue retains its existing palette. New-track file-drop previews use the same next-color calculation as import. Track panels and the app accent remain unchanged.

All 76 workspace tests and strict workspace Clippy checks pass. Added checks cover two palette cycles, the first/second/last hues, color stability after deletion, mixed missing/saved/legacy colors, save/reopen, and continuation after loading. File-drop shape checks confirm that first and second new-track previews match their committed colors. Existing inheritance, override, drag, trim, invalid RGB, and audio checks pass. The macOS debug bundle is rebuilt.

## Clip header looping

Dragging either header edge repeats the clip's current trimmed source range. The first resize stores the repeat length; later resizes retain it. Left extensions preserve the audio's existing timeline alignment. Partial repeats are allowed. Header interiors still move clips, and edges below the header still trim. Looped body trims shorten the visible range while retaining the base. Preview and release share the same range, phase, waveform, and overlap rules. Repeat boundaries appear in the waveform. Repeat data saves with the clip; moves and splits preserve it. Playback and export reuse decoded samples and apply the existing short fades at repeat boundaries.

All 85 workspace tests pass, including documentation checks. Added pointer and shape tests cover both header edges at different zoom/scroll positions, crossing into the body during a header drag, repeat previews and committed ranges, trimmed source offsets, left phase alignment, repeat-base retention, body trimming, overlap rejection, one-frame and timeline limits, unchanged clamped releases, and cancellation on focus loss or disabled controls. Waveform tests compare cached and direct sample extrema at wrapped windows, exclude samples outside the base, and cover tiny repeats and shortened sources. Engine tests compare realtime playback and offline rendering against the trimmed source and fades. Project tests cover save/reopen, repeatable WAV export, move/split phase preservation, shared samples, older manifests, and transactional rejection of invalid repeat data and overlaps.

Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Native pointer and audible loop checks remain manual; the new interaction is verified through egui input and shape tests.

## Minimum clip loop length

Header loop resizing now stops at the repeat base length instead of one frame. Normal clips cannot shrink through a header drag; extended loops can shrink to one base-length copy. Both edges use this limit for preview and release while retaining the opposite endpoint and phase alignment. Clips already shorter than the base after a split or body trim retain their current length as the minimum.

All 57 UI tests, formatting, strict workspace Clippy, and diff checks pass. Pointer tests cover both edges above, at, and below the base length, unchanged clamped releases, and matching preview and committed ranges. The macOS debug bundle is rebuilt.

## Clip header repeat separators

Repeat separators now span the header and waveform body. They use a subdued contrasting stroke so they remain visible on both surfaces and follow the clip's resolved color palette. The existing loop preview shape test verifies full-height separators for both header edges at different zoom and scroll positions. That test, formatting, strict UI Clippy, and diff checks pass. The macOS debug bundle is rebuilt.

## Restore source trimming after looping

Boundary resizing now clears the repeat flag when the clip becomes a single contiguous source range. It advances the source offset by the repeat phase while preserving sample mapping and timeline placement. Returning an aligned loop to its base length restores both body edges' ability to extend into the original source. Previously saved contiguous loop ranges also restore before body trimming. Ranges that still cross a repeat boundary retain their repeat data.

All 88 workspace tests pass, including documentation checks. Pointer tests perform the full trim, loop, return-to-base, and source-extension sequence on both header edges, compare every preview with release, clamp both body edges at the original source bounds, and check shared audio storage. They also cover source extension on previously saved base-length loops. Core tests compare every source frame before and after restoration, preserve wrapped ranges, and verify restoration can repeat without shifting the source again.

Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt.

## Source recovery after opposite-edge loop resizing

The previous restoration missed base-length loops with a nonzero phase. Extending from one header edge and shrinking from the other reproduced the failure: the clip stayed looped and the body edge could not recover the source. Returning to exactly one base-length copy now exits looping and restores the original trimmed source range, regardless of phase. Contiguous partial ranges still preserve their sample mapping; other wrapped ranges retain repeat data.

The expanded pointer regression failed before the fix and passes afterward. It covers all four extend/shrink edge combinations and source extension for saved base-length loops with zero and nonzero phases. Core checks cover restoring the original base, preserving contiguous partial and wrapped partial ranges, and repeated restoration. All 88 workspace tests, formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt and the empty running app is restarted with it. A temporary native project opens, but automated drags did not change either the clip or the existing zoom slider, so native pointer behavior remains unverified. The temporary project is closed afterward.

## Clip and loop-selection snapping

Clip body trims and loop-selection creation, resizing, and movement now snap within 6 points. Visible bar lines take priority over beats, visible subdivisions, and source/clip boundaries. Clip header looping instead prioritizes the trimmed repeat base and its multiples before the grid. Grid spacing shares the ruler's tempo and zoom calculation. Shift bypasses snapping immediately during a drag and on release. Source bounds, timeline bounds, and minimum lengths remain enforced. Returning a loop to the trimmed base still restores source extension.

All 94 workspace tests pass. New egui pointer and shape checks cover both clip edges, looping versus trimming priorities, two zoom/scroll settings, changing Shift at a stationary pointer, matching previews and releases, saved repeat-base retention, source recovery, selection creation, selection handles, and moving a selection without changing its duration. Musical-time checks cover visible subdivisions, fallback anchors, bar priority over closer subdivisions, fractional tempos, frame limits, and base repetitions. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Native drag verification remains manual.

## Horizontal trackpad scrolling

Horizontal two-finger swipes now update the timeline offset over its ruler, lanes, empty space, and scrollbar. The viewport consumes egui's processed horizontal delta before painting and uses the scrollbar's shared extent. Offsets clamp at both ends, including when the whole extent fits. Vertical deltas remain available to the track scroll area, so diagonal gestures can scroll both axes. Track controls, Master, toolbars, disabled controls, and unfocused workspaces do not scroll horizontally. Scrolling does not modify the project.

All 66 UI tests pass. New pointer and shape tests cover both directions, two zoom levels, the ruler, clip body, empty space, bounds, same-frame clip painting, scrollbar position retention, diagonal vertical scrolling, fixed track controls and Master, out-of-viewport input, disabled controls, and unchanged clip and transport data. Formatting, strict UI and desktop Clippy, and diff checks pass. The macOS debug bundle is rebuilt. A physical two-finger gesture remains a manual check.

## Clip movement and playhead snapping

Clip moves now compare both edges at each grid priority: bars, beats, then visible subdivisions. The nearest target at the first matching priority sets the translation without changing length, source offset, or repeat data. Other clip boundaries on the destination track provide fallback targets. Timeline bounds and overlap rejection remain enforced. Playhead clicks in the ruler or empty lanes, and dragging the ruler marker, use grid-first snapping with clip boundaries as fallback targets. Shift bypasses snapping immediately during a drag and on release.

All 69 UI tests pass. New pointer and shape tests cover left- and right-edge alignment, a right bar taking precedence over a closer left subdivision, same-track and cross-track moves, two zoom/scroll settings, stationary Shift changes, matching preview and release, unchanged source and repeat data, ruler clicks, playhead marker dragging, and empty-lane clicks. Musical-time checks compare both edges by priority and distance, cover anchor fallbacks at either edge, fractional-tempo bars, and maximum frame bounds. Existing overlap, start-clamping, source recovery, and selection tests pass. Formatting, strict UI and desktop Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Native pointer verification remains manual.

## File-drop snapping

File-drop previews and import placement now share the clip-movement snapping calculation. Both edges use bar, beat, and visible subdivision priority, then destination-track clip boundaries. While decoding, only the known start edge snaps; the loading placeholder's artificial width is excluded. Drops made before decoding finishes use the actual duration on the import worker. The worker retains the original pointer position, project tempo, zoom, and Shift state from release, so it does not snap a preview twice. Track-control drops still target frame zero, and Shift allows free placement.

All 72 UI tests pass. New input and shape checks cover either edge, a right bar taking priority over a closer left subdivision, existing and new tracks, two zoom/scroll settings, Shift changes at a stationary pointer, identical preview and committed positions, unchanged duration, decoded-sample reuse, loading previews, and direct drops without a prepared hover. A regression checks that import does not snap an already snapped position a second time. Existing overlap rejection, cancellation, invalid sources, track-control clamping, colors, and clip-movement tests pass. Formatting, strict UI and desktop Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Native file-drag verification remains manual.

## Clip copy and paste

Command+C/Command+V on macOS and Ctrl+C/Ctrl+V on Windows/Linux now copy the selected clip and paste a new clip on the selected track at the playhead. Copy captures the current name, trim, loop base/phase, and color override. Each paste assigns a new ID and shares the asset and decoded samples. A transactional InsertClip edit rejects overlap, invalid source ranges, missing records, duplicate IDs, and timeline overflow without changing the project. Native egui Copy/Paste events use an opaque system-clipboard token for the internal snapshot; command key events are also handled without duplicate actions. External text does not trigger clip insertion. Text fields retain their normal clipboard behavior. Busy operations, dialogs, drags, and focus loss block clip clipboard actions. Project replacement and closure clear the internal snapshot.

All 106 workspace tests pass, including 75 UI tests. Added checks cover actual clip selection, the system clipboard output command, native clipboard events, Command and Control key modifiers, repeated pastes with unique IDs, copying after trimming and looping, deletion of the original after copying, inherited and explicit colors, exact playhead placement, source-sample sharing, external text, text editing, busy/dialog/focus guards, overlap rejection, and project reset. Integration checks cover save/reopen, equivalent export before and after reopening, and transactional rejection of seven invalid insertions. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical shortcut verification on the supported platforms remains manual.

## Clip clipboard menu

The native macOS menu bar now includes Edit with Copy and Paste and Command+C/Command+V equivalents. Windows and Linux place Edit beside File in the first toolbar row with Ctrl+C/Ctrl+V labels. Menu commands use the guarded clip actions. Copy is disabled without a selected clip; Paste is disabled without a current-project clipboard snapshot or a remaining track. Both are disabled during text editing, background operations, dialogs, drags, and loss of window focus. Text inputs retain their existing keyboard clipboard behavior.

All 107 workspace tests pass, including 76 UI tests. A new menu test clicks Copy and Paste, checks the system clipboard output, verifies placement on the selected track at the playhead, and checks shared native dispatch guards during dialogs and text editing. Desktop compilation, formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Native menu interaction and Windows/Linux desktop verification remain manual.

## Clip drag duplication

Holding Option on macOS or Ctrl on Windows/Linux while dragging a clip header now places a copy. The source remains fully visible; the ghost preview shows a copy cursor and validates placement against the original as well as other clips. Both edges use the existing grid-first move snapping, with source-clip boundaries included as fallback targets. Shift bypasses snapping. Changing the copy modifier at a stationary pointer changes the preview and release action. Trim and loop-edge drags retain their existing behavior. The inserted copy receives a new ID, preserves the name, source range, loop base/phase, and color, and shares the asset and decoded samples. Successful insertion selects the copy and its track, marks the project modified, and syncs playback. Failed placement and focus-loss cancellation leave project data unchanged. Duplication does not change the clip clipboard.

All 109 workspace tests pass, including 78 UI tests. Added input tests cover same-track and cross-track duplication, two zoom/scroll settings, dynamic copy-modifier changes, snapped and Shift-unsnapped placement, matching preview and release, unique IDs, loop and color preservation, sample sharing, selection, dirty state, source-boundary snapping, original-overlap rejection, and focus-loss cancellation. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical drag and Windows/Linux desktop verification remain manual.

## Timeline pinch zoom

Two-finger pinches over the timeline, ruler, or horizontal scrollbar now change horizontal zoom. The existing egui-winit integration maps native pinch events to egui Zoom events; the timeline reads the processed zoom delta. Spreading fingers zooms in and pinching inward zooms out. Zoom uses the slider's shared 1–3000 pixels-per-second limits and adjusts scroll to keep the point under the pointer fixed where timeline bounds permit. The ruler, grid, clips, and previews paint using the new scale in that frame. Pinches outside the timeline, during jobs or dialogs, during clip or loop-selection drags, and while unfocused are ignored. Project data, dirty state, and global interface scale remain unchanged.

All 111 workspace tests pass, including 80 UI tests. Added gesture-event tests cover zoom in and out over the ruler, clip lanes, unused timeline, and scrollbar; pointer anchoring; painting at the new scale; unchanged project data and interface scale; scroll persistence; zoom and extent limits; invalid factors; and location, job, dialog, drag, and focus guards. Existing two-finger scrolling tests pass. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical trackpad verification and Windows/Linux desktop verification remain manual.

## Multiple clip selection

Shift-click toggles clip selection, as does Command-click on macOS or Ctrl-click on Windows/Linux. Click and hold in empty timeline space, then drag a box to select intersecting clips across tracks; Shift adds to the prior selection. Escape or focus loss cancels the box and restores the prior selection. Plain clip clicks replace the selection, and plain empty-space clicks clear it. All selected clips use the existing selection outline. Selection-box gestures do not change project data, playhead position, or loop range.

Copy and Paste through keyboard shortcuts and the Edit menu now act on the full selection. Paste places the earliest clip at the playhead and maps the top copied track to the selected track, preserving relative times and track gaps. It stages default tracks when needed and assigns a new ID to every inserted clip. Header moves and Option-drag on macOS or Ctrl-drag on Windows/Linux preserve group layout, preview every member, and clamp the group to timeline and available-track bounds. Snapping compares all group edges by grid priority before destination boundaries; Shift bypasses snapping. Moving originals are excluded from collision checks, while duplication retains them as occupied ranges and snap targets. Invalid placement makes every preview red and rejects the whole group. Trimming and looping remain local to the dragged edge. Delete removes the whole selection and prunes its IDs.

Batch edits validate the final arrangement as one transaction, allowing a clip to move through another selected clip's previous range. Any error rolls back all placements and any staged paste tracks. Copies preserve names, source offsets, lengths, loop bases/phases, and color overrides while sharing assets and decoded samples. Successful moves and insertions sync playback and mark the project modified; pasted and duplicated groups become selected. Project replacement and closure clear selection and clipboard state.

All 117 workspace tests pass, including 85 UI tests and 16 project integration tests. New input checks cover Shift-click toggling, plain selection replacement, rectangular and additive selection, cancellation, selection outlines, group clipboard snapshots, repeated multi-track pastes, automatic track creation, overflow/overlap rollback, source-sample sharing, group move/duplication previews, Shift overrides, track/timeline clamping, group deletion, and focus-loss cancellation. Group snapping checks cover a companion's higher-priority bar, destination-boundary fallback, and extreme bounds. Integration checks cover final-state batch validation, rollback of nested errors, grouped copies, save/reopen, and equivalent exports. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop interaction on each supported platform remains manual.

## Shift-drag selection toggle

Shift-click-and-hold in empty timeline space now toggles clips touched by the selection box. Unselected clips are added, selected clips are removed, and clips outside the box retain their state. The box compares against the selection captured at the start of the gesture each frame. Keeping the box stationary does not repeatedly toggle clips, and clips leaving the box regain their initial state. Cancellation continues to restore the starting selection. The same toggle rule applies to Command/Ctrl-modified selection boxes.

All 118 workspace tests pass, including 86 UI tests. The new input regression checks mixed add/remove selection across tracks, unchanged clips outside the box, stationary frames, shrinking and expanding the box, repeated gestures, copying the resulting selection, clearing the primary selection when all members are removed, and unchanged project data and dirty state. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop verification remains manual.


## Clip drops into unused timeline space

Moving a clip header below the last track now previews a new destination row. Dropping creates a default track and places the clip at the previewed position. Selected groups map their top track to the first new row and create enough tracks to preserve their track gaps. Move snapping, Shift override, timeline-start clamping, and drag duplication apply to these destinations. Previews use the future tracks' default colors with existing clip overrides. Hovering does not change the project. New tracks and clip placements commit as one validated transaction, so failure or focus-loss cancellation leaves tracks, clips, and selection unchanged. Returning to an existing track uses that destination without adding rows. The ruler, track controls, scrollbar, and space outside the timeline cannot create tracks through a clip drag.

All 121 workspace tests pass, including 89 UI tests. Three new input tests cover single clips and groups, retained track gaps, move and duplication, two zoom/scroll settings, snapped and Shift-unsnapped placement, preview waveform/color/outline, matching committed geometry, default track names and colors, preserved metadata and shared source samples, stationary hovering, timeline-start clamping, returning to existing tracks, excluded drop areas, focus-loss cancellation, and rollback of staged tracks after validation failure. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop interaction remains manual.


## Track panel selection

Primary or secondary presses anywhere in a visible track panel now select that track. This includes borders, padding, names, mute/solo buttons, gain/pan knobs and inputs, and meters. The selection outline updates in the press frame. Pointer events remain available to child controls, and the existing clip selection stays intact. Disabled UI, background jobs, dialogs, foreground layers, and loss of window focus block panel selection; hit testing respects the scrolling UI's clip rectangle.

All 123 workspace tests pass, including 91 UI tests. Two new input checks cover panel locations, immediate selection outlines, unchanged clip selection and dirty state on press, mute/solo toggles, gain/pan dragging, numeric-input focus, disabled UI, focus loss, outside clicks, dialogs, and a foreground layer covering the panel. Existing track-name editing tests pass. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop interaction remains manual.


## Track reordering

Drag a track name vertically to reorder the whole track. An accent insertion line spans the panel and timeline; release within the track workspace commits the move. Track order stays unchanged during the preview. Holding near the viewport's top or bottom scrolls the list, allowing moves across tracks outside the initial view. The master remains pinned. Escape, focus loss, disabled controls, jobs, dialogs, and outside releases cancel. Dropping at the current position does not mark the project modified. Double-click name editing, other controls, clip selection, and clipboard snapshots retain their behavior. Row widget identities now follow track IDs instead of display positions. The transactional ReorderTrack edit preserves track/clip IDs, names, colors, controls, source references, and clip ranges; successful moves sync playback. Array order persists through project save/reopen.

All 127 workspace tests pass, including 94 UI tests and 17 project integration tests. Three new UI checks cover insertion previews, six upward/downward moves, panel and lane destinations, unchanged previews, selection and clipboard preservation, no-op and single-track drops, Escape/focus/job/dialog/outside cancellation, and automatic scrolling down and back up through twelve tracks with a pinned master. A new integration check verifies metadata and color preservation across six tracks, save/reopen order, equivalent audio export, upward and downward moves, and transactional rejection of missing tracks, invalid positions, and a failed batch. Existing panel controls and track-name editing tests pass. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop interaction remains manual.


## WAV and MP3 export options

File → Export and its shortcut open the shared application dialog before the native destination chooser. Choose WAV with 16-bit PCM, default 24-bit PCM, or 32-bit IEEE float; choose MP3 with MPEG Layer III through bundled LAME and CBR 128/192/256/320 kbps (default 192). Output remains stereo at 48 kHz. The last accepted options are retained for the app session; cancelling options leaves the project unchanged. Pending overwrite confirmation retains the selected encoder settings. Export options and overwrite prompts block background editing. Integer WAV keeps deterministic TPDF dither; float WAV preserves headroom without dither or clamping. MP3 uses bounded streaming encoding, clamps normalized input, and finalizes encoder delay/padding metadata for gapless decoding. Source files, project manifests, and completed destinations remain protected; failed encoding removes temporary files. CLI format, codec, and bitrate options use the same backend, infer the format from the extension, and reject incompatible arguments with exit code 2.

All 133 workspace tests pass, including 97 UI tests, 19 project integration tests, and one CLI test. New checks cover format/codec/bitrate switching, cancellation, background jobs and blocked editing, retained overwrite settings, every codec and bitrate, exact stereo 48 kHz WAV formats and duration, float headroom, clipping diagnostics, deterministic bytes, source/manifest protection, failure rollback, and temporary-file cleanup. Independent Symphonia MP3 decoding verifies exact gapless sample counts, leading silence, and bounded lossy error for all four bitrates. The CLI smoke check passes for WAV/MP3 options, repeated bytes, protected existing destinations, incompatible arguments, missing sources, and eight-track projects. Formatting, strict workspace Clippy, and diff checks pass. The rebuilt macOS debug bundle contains the encoder licenses and source/build notice; its dynamic dependencies contain no external LAME or FFmpeg library. Physical native destination-dialog interaction and Windows/Linux builds remain unverified.


## Export status dialog

Once an export destination and any overwrite confirmation are accepted, the shared Export status dialog opens immediately. The export worker reports encoded timeline frames through a shared atomic counter. Rendering/encoding progress stays below 100% while the file is being finalized; success reaches 100% only after publication. The report includes destination, format/codec/bitrate, source and clipping warnings, a completion notice, or an export error. Worker disconnection is reported in the same dialog. Long paths and diagnostics scroll while Close stays outside the scrolling area. The report remains open after success/failure until Close or Escape; a running report cannot be dismissed. Background editing, clip/file actions, and playback shortcuts remain blocked while the report is open. Export diagnostics no longer require the bottom notices area or a separate generic error window.

All 136 workspace tests pass, including 99 UI tests and 20 project integration tests. New checks verify live progress, the finalization stage, completion and warnings, blocked background controls and file shortcuts, ignored Escape while running, Close and Escape after completion/failure, long scrollable reports, error and worker-disconnection routing, retained settings/destination, and actual background WAV/MP3 exports. The integration progress check verifies monotonic bounded frame updates through the final partial block and byte-identical exports with and without progress callbacks for both formats. Formatting and strict workspace Clippy pass. The macOS debug bundle is rebuilt. Physical native-dialog interaction and Windows/Linux builds remain unverified.


## Named first Save and Save As

The first Save and every Save As now open the shared application dialog with the current project name selected for replacement. Continue or Enter with a valid name opens the system folder picker for the parent location. Saving creates a new child folder using the trimmed name, with project.json and assets inside. The manifest name and current project folder change only after the background save succeeds. Regular Save continues to use the current folder. Existing folders or files at the destination are rejected instead of replaced. The shared backend validates blank names, path separators, unsupported characters, control characters, reserved device names, and length. Save As preserves source identity, decoded samples, and the original project manifest while recalculating relative source paths. Failure rolls back the session and removes only empty newly created directories. Cancelling the name dialog or native picker cancels any pending close/open/new operation and retains the current project and unsaved state. A pending close proceeds only after successful saving. Editing and other file/clip actions are blocked while naming.

All 142 workspace tests pass, including 103 UI tests and 22 project integration tests. New checks cover first Save and Save As shortcuts on new/existing projects, selected input text, Unicode names, Enter/Continue/Cancel/Escape, invalid-name feedback, disabled background controls, both cancellation points, actual background saves, regular Save after Save As, deferred session updates, save-before-close success/failure/cancellation, child-folder creation, saved names, existing-folder/file protection, failed-save directory cleanup, retained source references and shared samples, equivalent exports, and reopening both original and copied projects. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical native-picker interaction and Windows/Linux builds remain unverified.


## Half-beat ruler ticks

Visible half-beat ruler ticks are now 8 points tall, compared with 5 points for finer subdivisions. Whole-beat and bar tick heights, grid positions, zoom visibility, and snapping use the existing behavior. All seven existing ruler tests pass, along with formatting, strict workspace Clippy, and diff checks. The macOS debug bundle is rebuilt.


## Track grip handles

Each audio track has an unframed 16-point Lucide Grip at the far left of its name row, centered in a 22-point hit target. The balanced header keeps the name centered in its existing content column and retains control-row and meter positions. The handle has a Reorder track accessibility label, drag tooltip, and grab/grabbing cursors. It starts the shared track reorder gesture, including insertion preview, auto-scroll, release, and cancellation. Existing name dragging and double-click editing remain available.

All 104 UI tests pass. The new input check covers handle selection, reorder preview and release, Escape cancellation, centered name geometry, preserved clip selection, and blocked dragging during background jobs. Existing track reordering, control interactions, name editing, and meter layout checks pass. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop interaction remains unverified.

## Clip stretching

Option on macOS and Ctrl on Windows/Linux select pitch-preserving stretching when a drag starts from either clip edge, including header edges. Plain header and waveform edge gestures retain looping and trimming. Stretching fixes the opposite edge, previews the waveform at the proposed duration, uses grid-first snapping, and supports Shift to bypass snapping. It clamps at the timeline start and to ratios from 1/8 through 8. Overlapping releases are rejected. Escape now cancels the shared clip drag handler; focus loss also cancels. Processing runs after release on a worker and applies the staged edit only after success.

Rubber Band 4.0.0 is bundled through rubberband 0.1.5 / rubberband-sys 0.1.4, using R3/Finer, offline mode, linked stereo channels, and no internal worker threads. The original asset is retained. Reduced integer ratios are saved on clips; offsets and repeat ranges use stretched-frame coordinates. Session caches hold one full-source buffer per asset and ratio, shared by waveform painting, live playback, and export. Preparing a new stretch retains only buffers still used by the project. Reopening rebuilds these buffers. Playback does no stretch processing or buffer allocation. Missing and shortened sources retain existing silence/warning behavior. Bundled library licenses and source notices accompany the macOS app; building the bindings needs a C++ compiler and Clang/libclang.

All 149 workspace tests pass, including 107 UI tests and 23 project integration tests. New checks cover both edges at both header/body heights, platform modifiers, snapped and free previews, background commits, source sharing, extension to original source bounds after stretching, ratio/start clamps, overlap, Escape cancellation, processing failure rollback, ratio/offset/phase scaling, and return to the original ratio. A 440 Hz stereo signal keeps its pitch and opposite-channel relationship at half and one-and-a-half duration. Integration checks cover trims, loops, splits, copies, cache reuse, retained source-file bytes, old optional-field defaults, saved/reopened settings, equivalent live/offline samples, and byte-identical exports before/after reopen. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical desktop gestures and Windows/Linux builds remain unverified.

## Windows custom title bar

Windows now uses a custom themed title bar instead of the native title bar. The first row contains the project title, unsaved marker, and Minimize, Maximize/Restore, and Close controls. File and Edit occupy the second row; playback controls occupy the third. All rows use the shared toolbar component. A thin border outlines the window. Title-area dragging and double-clicking issue native move and maximize/restore commands. Border handles issue resize commands for all edges and corners, except when maximized or fullscreen. Close uses the existing unsaved-change and background-job guards. The previous Windows single-row title/menu presentation is superseded. macOS and Linux retain their existing window presentation.

Checks run on Windows x64 with the pinned Rust toolchain and LLVM/libclang configured:

| Command | Result |
| --- | --- |
| `cargo build -p daw-desktop` | Passed; Windows debug executable built. |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo test --workspace --no-fail-fast` | 152 passed; two existing UI tests failed on Windows. All five new header tests and all project integration tests passed. |
| `git diff --check` | Passed. |

New egui input tests cover separate title/menu/playback rows, title and unsaved-marker updates, minimize/maximize/restore/close commands, title dragging and double-clicking, all eight resize directions, suppressed resizing when maximized, and close cancellation for unsaved changes or active jobs. File-drop tests now use timeline geometry instead of a fixed vertical position, so they work with the additional Windows row.

The unchanged revision was also tested from an ignored temporary copy. It reproduced `export_jobs_use_selected_settings_and_overwrite_retains_them` (a source path is not absolute on Windows) and `save_before_close_waits_for_named_save_and_cancels_on_failure_or_cancellation` (a fixture path cannot be found). These failures remain outside this header change. Logs are in `work/windows-header-baseline-tests.log` and `work/windows-header-workspace-tests.log`.

Native window gestures, Windows scaling and snapping, file-dialog interaction, and real audio-device playback were not checked. Automated egui tests verify viewport commands, not their native execution. macOS and Linux builds were not run for this change.

## Standard Windows title bar restored

The standard Windows title bar supersedes the custom Windows header described above. Windows draws the project title, unsaved marker, and native caption buttons and handles window moving and resizing. File and Edit remain in the first application toolbar row, with playback controls below. The project title is not repeated in the menu row. Custom caption drawing and border resize handles were removed. Native close requests retain the existing unsaved-change prompt and active-job guard.

On Windows x64, `cargo build -p daw-desktop`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check` passed. `cargo test --workspace --no-fail-fast` completed with 149 passing tests and the same two baseline UI failures recorded above. Updated egui checks verify native title commands after a project rename, the unsaved marker, no duplicate project label in the menu row, menu/playback ordering, and cancellation of close requests for unsaved changes or active jobs. The test log is `work/windows-native-title-tests.log`.

Native caption-button interaction, Windows scaling and snapping, and real audio-device playback were not checked. macOS and Linux builds were not run.

## Accent settings

Settings opens from the application-name menu on macOS and File on Windows/Linux, with Command/Ctrl+comma as its shortcut. The shared dialog offers all 19 Material 500 colors, labeled by name with a color swatch on the left of each dropdown item. Purple 500 replaces the previous custom purple as the default. A choice applies immediately to themed controls and custom accent painting through the current egui context. Track and clip colors remain separate. Close retains the choice; Escape closes an open dropdown first, then the dialog. Settings blocks project editing and other file/clip actions. Preferences do not modify the project or its dirty state and survive project changes. Desktop startup reads application storage; eframe autosave and normal shutdown save the preference. Missing or invalid stored values use the default.

All 155 workspace tests pass, including 113 UI tests. New checks cover all color names and left-hand swatch geometry, selection and popup closure, preference round trips and invalid-data fallback, live knob and selected-clip colors, context isolation, modal guards, keyboard and native-dispatch routing, Escape, and retained preferences across project changes. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt and includes the new persistence dependency notices. Physical native-menu interaction, persistence across a native app restart, and Windows/Linux builds remain unverified.


## Light theme

Settings now includes a Theme dropdown with Dark (default) and Light. Light uses the supplied `#E9E9E9` background and `#F9F9F9` panels, toolbars, menus, and dialogs. Shared palettes provide dark text, borders, input/meter backgrounds, grid lines, and warning/error colors. Custom painting follows the active context palette; egui controls use matching visuals. Switching applies immediately, preserves the chosen accent, and sends the native window theme command. Theme and accent persist together in application storage; older accent-only settings default to Dark. Theme changes preserve project data, dirty state, clip colors, typography, and layout.

All 157 workspace tests pass, including 115 UI tests. The new input checks exercise both dropdown choices, accent changes in Light, switching back to Dark, context isolation, dialog fills, unchanged control spacing/fonts, timeline/panel/input/grid colors, track/master text, clip/waveform colors, missing-source text, selection outlines, and unchanged project/geometry. Preference round trips cover both themes with every accent and the older settings schema. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical native-window appearance, persistence across a native restart, and Windows/Linux builds remain unverified.


## Open dropdown accent foregrounds

The shared theme now updates the open widget foreground with the selected accent. Theme and Accent color dropdown labels and arrows retain that color after the pointer is released or leaves the control, instead of reverting to the default purple. Background and outline tints retain the existing behavior.

All 158 workspace tests pass, including 116 UI tests. The regression check opens both Settings dropdowns with Deep Orange and Teal in Dark and Light, checks the rendered label after release and pointer exit, verifies the shared open foreground used by the arrow, and confirms preferences remain unchanged. Formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle is rebuilt. Physical native interaction and Windows/Linux builds remain unverified.


## Tempo changes retain clip beat positions

Typed BPM entry and vertical tempo dragging now use the shared transactional `Edit::SetTempo` command. Each clip start scales by old BPM divided by new BPM, rounded to the nearest sample frame. Clip lengths, source offsets, loop bases/phases, stretch ratios, colors, IDs, and prepared audio stay intact. Final validation rejects same-track overlaps and timeline overflow before publishing any changes. Rejection reports an error and retains the last valid tempo, clips, dirty state, and playback plan. Successful changes mark the project modified and sync playback. Re-entering the current tempo is a no-op. Playhead and loop-selection frame positions stay unchanged. This supersedes the earlier behavior that kept clip starts fixed in seconds.

All 161 workspace tests pass, including 117 UI tests and 25 project integration tests. New checks cover typed entry and dragging, accepted and rejected increases, lower and fractional tempos, sample rounding, cross-track starts, touching clip endpoints, invalid values, start/end overflow, rollback after another track has been staged, preserved clip metadata and prepared samples, unchanged transport frames, live/render parity, saved/reopened positions, equivalent exports, and retained source bytes. Formatting, strict workspace Clippy, diff checks, and the CLI smoke check pass. The macOS debug bundle is rebuilt. Physical desktop interaction, real-device playback during tempo changes, and Windows/Linux builds remain unverified.


## Tempo changes retain loop-selection beats

The transactional tempo edit now scales both loop-selection endpoints by old BPM divided by new BPM, including when Loop is disabled. Endpoints round up to sample frames, matching ruler snapping and keeping fractional beat boundaries in the correct beat on the selection monitor. The Loop toggle and playhead frame stay unchanged. Tempo, clip starts, and selection publish together. Endpoint overflow, a collapsed enabled loop, or clip overlap rejects the entire change and retains the previous selection. This supersedes the fixed loop-selection frames recorded above.

All 162 workspace tests pass, including 117 UI tests and 26 project integration tests. Checks cover lower, higher, and fractional tempos, enabled/disabled and empty selections, preserved beat endpoints and toggle/playhead state, selection monitors after typed and dragged BPM changes, saved/reopened ranges, overlap rollback, endpoint overflow, and an enabled loop collapsing below sample precision. Formatting, strict workspace Clippy, diff checks, and the CLI smoke check pass. The macOS debug bundle is rebuilt. Physical desktop interaction, real-device playback during tempo changes, and Windows/Linux builds remain unverified.
