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


## Trim selected clips as a group

Waveform-edge trims now use the full selection across tracks. Snap the dragged edge once and apply the same frame delta to each clip. Reject the whole shortening edit if any clip would become empty. Expansion clamps each clip independently at its saved source and timeline bounds; the dragged clip reaching its limit does not cap the others. Preview every affected clip, dim the originals, and show rejected ranges in red. Validate final overlaps through the shared project model and commit with `Edit::Batch`. Preserve IDs, selection, source references, loop phases, stretch coordinates, and color overrides. Header looping and modifier stretching remain local to the dragged clip. Shift bypasses snapping. Escape, focus loss, and unchanged clamped releases retain the original project. This supersedes the earlier restriction that kept selected waveform trims local to one clip.

All 166 workspace tests pass, including 119 UI tests and 26 project integration tests. New core checks cover opposite endpoints, empty-range rejection, source/timeline bounds, stretched source lengths, loop phase mapping, and restoration of original source bounds. Pointer-driven group checks cover both edges, grid snapping and Shift bypass, independent source clamps, previews matching release, overlap with selected and unselected clips, rejected shortening at or beyond the shortest clip, Escape cancellation, retained selection, and clean stationary/clamped releases. `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass. Cargo checks use the local cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction and Windows/Linux builds remain unverified.


## Loop selected clips as a group

Header-edge looping now applies one snapped frame delta to all selected clips across tracks. Each clip uses its own current trimmed length as the first repeat base and retains its saved base on later edits. Left resizes shift each repeat phase to preserve the existing audio timing. Clamp the common movement at the group's timeline bounds and each clip's minimum loop length. Prioritize the dragged clip's base multiples before grid snapping; Shift bypasses snapping. Show all repeated previews and dim the originals. Use shared project validation for the final ranges, including collisions between selected clips and with unselected clips. Reject an invalid release without publishing any changes. Successful edits use `Edit::Batch` and retain selection, IDs, source offsets, colors, and stretch data. Escape, focus loss, and unchanged clamped releases preserve the project. This supersedes the earlier restriction that kept selected header looping local to one clip.

All 170 workspace tests pass, including 122 UI tests and 26 project integration tests. New checks cover both header edges, different trimmed and saved repeat bases, phase/sample mapping, preserved stretch data, previews matching commits, repeat snapping and Shift bypass, group minimum and timeline bounds, returning all clips to their original ranges, selected/unselected overlap rejection with red previews and errors, no-op releases, Escape cancellation, focus loss, and retained selection. Existing single-clip looping and group trimming checks also pass. `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction and Windows/Linux builds remain unverified.


## Resize overlaps trim or remove neighboring clips

Date: 2026-10-10.

Waveform trim extensions and header looping now resolve overlaps instead of rejecting them. The shared `Project::resized_clips` calculation and transactional `Edit::ResizeClips` command resize all candidates first, then process right-edge edits in ascending original timeline order and left-edge edits in descending order. Earlier clips win right-edge collisions; later clips win left-edge collisions. Trim the covered edge of selected or unselected neighbors on the same track and remove fully covered ranges. Preserve remaining source sample mapping, loop phases (including a full repeat with nonzero phase), stretched coordinates, IDs, and color overrides. Deleted selection IDs are pruned. Source files and asset records stay intact. Moves, imports, paste, and modifier stretching retain overlap rejection. This supersedes the earlier resize-overlap rejection recorded above.

During dragging, calculate the complete result in a separate project copy. Paint all affected ranges and hide fully covered clips in the preview. Keep project data, dirty state, and playback geometry unchanged until the primary mouse button is released. Commit the resize, all neighbor trims, and all removals together on release. Moving back restores previews from the unchanged original project. Escape, focus loss, disabled controls, and dialogs cancel; no-op releases leave the project clean. Invalid shortening and invalid source/timeline bounds still reject the full transaction.

All 177 workspace tests pass, including 124 UI tests, 13 core tests, and 27 project integration tests. New checks cover both resize directions, partial and complete neighbor coverage, selected clips trimming one another in deterministic order, source mapping for looped and stretched survivors, wrapped full-repeat phases, invalid-command rollback, no-op edits, neighbor previews and commits, selected neighbor removal, unchanged project state during dragging, preview restoration on backward movement, Escape and focus-loss cancellation, selection pruning, saved/reopened ranges, equal renders/exports, shared sample buffers, and unchanged source bytes. Formatting, strict workspace Clippy, diff checks, CLI build, and `python3 tools/check_cli.py` pass. Cargo checks use the local cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction, real-device playback during these gestures, and Windows/Linux builds remain unverified.


## Stretch selected clips as a group

Date: 2026-10-10.

Option-edge dragging on macOS and Ctrl-edge dragging on Windows/Linux now stretch every selected clip across tracks. Header and waveform edges share the same calculation. Snap the dragged edge once and apply its duration ratio to all selected clips, rounded to sample frames. Keep each opposite endpoint fixed and clamp the common ratio at every clip's timeline and accumulated 1/8–8 source-ratio bounds. Preserve IDs, selection, colors, and source references; scale source offsets and repeat bases/phases. Show all previews and reject the full group if any selected or unselected clip would overlap. Shift bypasses snapping. This supersedes the earlier restriction that kept modifier stretching local to one clip.

Keep project data, playback, and prepared audio unchanged during dragging and processing. Release stages the complete group in a separate session and prepares all required Rubber Band buffers on one worker. Publish only after success. Escape or focus loss cancels the drag; a stationary or unchanged clamped release stays clean. Processing failure retains all original clips and existing prepared buffers. Prepared results reuse existing cache entries and original decoded audio; no source file changes.

All 183 workspace tests pass, including 130 UI tests. New checks cover both edges in the header and waveform, shared ratios with different clip lengths, grid snapping and Shift bypass, previews matching committed ranges, unchanged state before release and worker completion, accumulated stretch ratios, repeat bases/phases and source offsets, shared timeline/ratio clamps, sample rounding near the maximum timeline frame, selected and unselected overlap rejection, Escape and focus-loss cancellation, no-op releases, worker failure after preparing another clip, retained prepared buffers, cache reuse, and returning a clip to its original source ratio. Existing single-clip stretch and group trim/loop checks also pass. `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction, real-device playback during group stretching, and Windows/Linux builds remain unverified.


## Stretch overlaps trim or remove neighboring clips

Date: 2026-10-10.

Stretching now resolves overlaps instead of rejecting them. The shared `Project::stretched_clips` calculation and transactional `Edit::StretchClips` command apply the new stretch coordinates, trim intersecting neighbor edges, and remove fully covered clips. Selected clips may trim or remove one another using the existing direction order: earlier clips take priority for right-edge stretches; later clips take priority for left-edge stretches. Retain each survivor's source mapping, stretch ratio, repeat phase, ID, source reference, and color. Prune removed clip IDs from the selection only after publication. This supersedes the stretch-overlap rejection recorded above.

Calculate the entire result in a project copy for the preview. Mask original neighbor ranges and fully covered clips, then show the surviving ranges. Moving back restores the original preview without changing the project. Mouse release stages all stretches, trims, and removals in a separate session. Prepare Rubber Band buffers on the worker and publish the complete result only after success. Project data, playback ranges, dirty state, selection, and prepared audio stay unchanged during dragging and processing. Escape or focus loss cancels the drag; processing failure keeps all original clips and prepared buffers. Source recordings stay unchanged.

All 190 workspace tests pass, including 16 core tests, 133 UI tests, and 28 project integration tests. Checks cover both stretch edges in the header and waveform, partial neighbor trims, full removal, selected clips trimming and removing one another, deterministic priority independent of command order, preserved looped/stretched neighbor samples, invalid-command rollback, previews matching commits, unchanged state before release and processing completion, preview restoration on backward movement, Escape and focus-loss cancellation, worker failure with pending neighbor edits, retained prepared buffers, selection pruning, saved/reopened results, equal renders and exports, cache reuse, and unchanged source bytes. Formatting, strict workspace Clippy, diff checks, CLI build, and `python3 tools/check_cli.py` pass. Cargo checks use the local cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction, real-device playback during these gestures, and Windows/Linux builds remain unverified.


## Moves and file drops overwrite destination ranges

Date: 2026-10-10.

Moves and timeline file drops now take priority over existing destination clips. The shared `Project::placed_clips` calculation and `Edit::OverwriteClips` transaction remove the full moving group from its original tracks before resolving destination overlaps. Trim covered neighbor edges and remove fully covered clips. A placement inside a neighbor retains both surrounding pieces: keep the original ID on the left or sole surviving piece and assign new IDs to additional right pieces. Preserve source sample mapping, loop phases, stretch ratios, colors, and source references. Multiple moved clips preserve their gaps and may split the same neighbor at several positions. Resizes, stretches, moves, and drops share the same range subtraction helper. This supersedes earlier move and file-drop overlap rejection; menu imports, paste, and drag duplication retain their existing overlap validation.

Previews calculate the complete result in a separate project copy. Mask affected original ranges and draw all surviving neighbor pieces alongside the incoming clips. Hovering, moving back, and cancelling never change project data or playback. Commit moves, neighbor edits, and any required new tracks together on mouse release. File drops reuse prepared audio and publish the asset, clip, neighbor edits, and new track only after import succeeds. Invalid sources, ranges, tracks, or incoming groups retain the original project and audio caches. Grid snapping, Shift bypass, frame-zero clamps over track controls, and empty-timeline previews remain supported.

All 200 workspace tests pass, including 20 core tests, 137 UI tests, and 30 project integration tests. New and updated checks cover middle splits, trims on either edge, full coverage, touching boundaries, multiple incoming clips and retained gaps, cross-track moves, preserved looped/stretched neighbor samples, IDs and colors, atomic invalid-group rollback, previews matching committed geometry, snapping and Shift bypass, unchanged state before release and import completion, preview restoration, Escape and focus-loss cancellation, cancelled file hovers, failed imports, prepared audio reuse, saved/reopened fragment IDs, equal renders and exports, and unchanged source bytes. Formatting, strict workspace Clippy, diff checks, CLI build, and `python3 tools/check_cli.py` pass. Cargo checks use the local cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction, real-device playback during these gestures, and Windows/Linux builds remain unverified.

## Shared waveform boundary between contiguous clips

Date: 2026-10-10.

Starting a waveform drag on the dividing line between touching clips now trims one clip while extending the other. The pair retains its outer endpoints and clamps the shared boundary to both source limits in stretched-frame coordinates. Fully covering either member removes it. The hit area is one physical pixel centered on the line; the gesture is captured at the initial press. Starting inside either clip retains ordinary trimming. Header looping and modifier stretching keep their existing behavior. If both clips are selected, treat them as one pair; other selected clips retain the requested trim delta and ordinary source clamps and empty-range rejection.

`Project::trimmed_boundary` and transactional `Edit::TrimBoundary` share preview and release calculations. Preview both ranges and removals without modifying the project. Commit the pair, other selected trims, and any resulting neighbor edits together on release. Shift bypasses snapping. Returning to the original boundary restores the previews. Escape or focus loss cancels; a stationary or fully clamped release stays clean. Retain source references, IDs, stretch metadata, colors, and surviving neighbor sample mapping, including wrapped loop phases. No source files or manifest schema change.

All 212 workspace tests pass, including 25 core tests, 144 UI tests, and 30 project integration tests. New checks cover dragging from either side, both directions, source limits with existing stretching, full coverage and selection pruning, loop phase mapping and restored source bounds, atomic group rejection, previews matching release, grid snapping and Shift bypass, unchanged state during dragging, preview restoration, Escape and focus loss, clean clamped releases, ordinary trims beside the line, unchanged loop/stretch behavior, and precise line interaction at two display scales. `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction, real-device playback during these gestures, and Windows/Linux builds remain unverified.

## Wider shared waveform boundary hit area

Date: 2026-10-10.

The shared boundary gesture now accepts the initial press within 3 logical points on either side of the dividing line, for a total width of 6 points. This supersedes the one-physical-pixel hit area recorded above. The size stays consistent across display scales. The remainder of the ordinary 8-point waveform edge area still trims the individual clip. Hover feedback and gesture capture use the same threshold; header looping, modifier stretching, snapping, previews, source clamps, and release transactions retain their behavior.

All 212 workspace tests pass. Updated pointer checks cover both sides within and outside the wider area at two display scales and confirm ordinary trimming remains available. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical desktop interaction and Windows/Linux builds remain unverified.

## Track multi-selection and mute/solo shortcuts

Date: 2026-10-10.

Press M to toggle mute or S to toggle solo on every selected audio track. Each track reverses its own state, including mixed selections. Apply the group through one validated `Edit::Batch` and sync playback. A plain panel click selects one track; Shift-click or platform Command/Ctrl-click adds or removes a track. Highlight the complete selection without changing clip selection or project data. Preserve selection through saves and reordering, prune deleted tracks, and reset it when opening or closing a project. Master is excluded.

Ignore key repeat and Command/Ctrl/Alt-modified keys. Block shortcuts during text editing, pointer or monitor drags, file dragging, jobs, dialogs, disabled UI, and loss of focus. Track-panel padding near the timeline boundary no longer triggers the timeline's expanded pointer hit area or moves the playhead.

All 217 workspace tests pass, including 26 core tests and 148 UI tests. New checks cover single and multiple selections, mixed mute/solo states, unchanged unselected tracks, modifier selection, immediate outlines, empty selection, key repeat, modified shortcuts, text input, blocked interactions, selection cleanup, save/reorder retention, project replacement, and atomic rollback for invalid track commands. `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass. Cargo checks use the local cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical keyboard interaction, real-device playback, and Windows/Linux builds remain unverified.

## Track menu

Date: 2026-10-10.

Add Track after Edit in the native macOS menu bar and the in-window Windows/Linux menu. Its Mute and Solo items show M and S and toggle each selected track's own state. Menu clicks, native callbacks, and keyboard shortcuts share guarded `TrackAction` dispatch and validated group edits. Disable both items without a selected track or during text editing, drags, jobs, dialogs, or loss of focus. The macOS adapter ignores native key repeat and consumes matching forwarded key events to prevent duplicate toggles.

All 219 workspace tests pass, including 150 UI tests. New menu checks cover mouse activation on release, mixed group states, unchanged unselected tracks, retained selection, shortcut labels, empty selection, disabled UI, and shared callback guards. Existing keyboard checks also pass. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS desktop compiles and the debug bundle is rebuilt at `target/debug/DAW.app`. Native menu interaction, physical keyboard routing, and Windows/Linux builds remain unverified.

## Add track in the Track menu

Date: 2026-10-10.

Add track is the first item in both Track menus, followed by a separator, then Mute and Solo. It has no shortcut and works without a selected track. Menu callbacks and the Tracks header Plus button share the guarded Add action. Append an empty track with its default color, select it, retain clip selection, mark the project modified, and sync playback.

All 220 workspace tests pass, including 151 UI tests. The new menu check covers item order, separator placement, mouse activation on release, adding without selection, preserved existing tracks and clip selection, default color and control states, and selection of the new track. Existing guard and mute/solo checks pass. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Native menu interaction and Windows/Linux builds remain unverified.

## Track context-menu group deletion

Date: 2026-10-10.

Secondary-click anywhere in a track panel to open a context menu. Clicking a selected track preserves the group; clicking an unselected track selects only that track. Show Delete track for one track or Delete selected tracks for a group. Opening or dismissing the menu leaves project data unchanged. Menu-item release removes the selected tracks and their clips through one validated batch. Prune deleted track/clip selections and numeric input state after the panels finish painting, mark the project modified, and sync playback. Retain source files and asset records. Menu opening and deletion respect jobs, dialogs, and focus; deletion also respects text editing and active drags.

All 223 workspace tests pass, including 154 UI tests. New pointer checks cover names, padding, Mute controls, and meters, group preservation, selecting an unselected target, deletion on release, unchanged surviving tracks/assets, retained surviving clip selection, deleted selection/control cleanup, Escape dismissal, blocked jobs, and deleting all tracks with playback sync. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Physical secondary-click interaction, real-device playback, and Windows/Linux builds remain unverified.

## Delete tracks from the menu bar

Date: 2026-10-10.

Add Delete track after Solo, separated by a divider, in both Track menus. Change its label to Delete selected tracks for a group, including the native macOS item. It has no shortcut and requires a selected track. Native callbacks, in-window menu clicks, and context-menu deletion share guarded `TrackAction::Delete` dispatch and the same validated batch, source preservation, and selection/control cleanup.

All 224 workspace tests pass, including 155 UI tests. New checks cover single/group menu labels, item placement and separator, deletion on mouse release, preserved surviving tracks/assets, selection cleanup, dirty state, and playback sync. Existing empty-selection, disabled-UI, text-editing, drag, job, dialog, focus, and context-menu checks also pass. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Native menu interaction and Windows/Linux builds remain unverified.

## Duplicate empty tracks from menus and the header

Date: 2026-10-10.

Add Duplicate track or Duplicate selected tracks before Delete in the context menu and Track menu bar. Add an icon-only Lucide Copy Plus button immediately after the header Plus button. All three controls share guarded `TrackAction::Duplicate` dispatch; disable duplication without selection and during blocked interactions. The button uses the shared styling and single/group tooltip and accessibility label. Duplication has no keyboard shortcut.

Apply a batch of `Edit::DuplicateTrack` commands. Each copy gets a new ID, the same name, color, gain, pan, mute, and solo settings, and no clips. Insert each directly after its original in project order. Select the copies with the first as primary, retain clip selection, mark the project modified, and sync playback. Keep original tracks, clips, and assets unchanged. Context-menu actions publish only on item release after panel painting. Failed validation retains the project and selection.

All 228 workspace tests pass, including 27 core tests and 158 UI tests. New checks cover both menus with single/group selections, empty copies with exact settings, adjacent insertion and noncontiguous source order, distinct IDs on repeated duplication, unchanged originals and assets, retained clip selection, copy selection, release-only changes, disabled header clicks, group header duplication, missing-track/duplicate-ID rollback, and UI failure rollback. Existing guard checks include duplication, and the Add button still works beyond four tracks at its new header position. Formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Native menu/button interaction, real-device playback, and Windows/Linux builds remain unverified.

## macOS titlebar preference (2026-10-11)

- Added a macOS-only Titlebar dropdown in Settings: Custom (default) and Native. Both apply without recreating the window. Native removes the custom header row and shows the project title in the system titlebar. The preference uses desktop application storage; older settings retain Custom.
- Automated checks cover preference round trips and older data, dropdown switching while retaining theme/accent, custom-header removal/restoration, native title and unsaved-marker updates, and preservation of caption/resizing/fullscreen style flags. The dropdown color test now locates the button by position when the popup opens upward.
- Passed `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (231 tests), and `git diff --check`, using the local Cargo cache. Logs: `work/titlebar-workspace.log` and `work/titlebar-clippy.log`.
- Rebuilt `target/debug/DAW.app` with `sh tools/package_macos.sh debug`. Native UI checks verified both titlebar choices, removal/restoration of the custom header, visible native window buttons, and restoration of Native after normal shutdown and relaunch. Restored the original Custom preference after testing.
- Remaining native checks: fullscreen transitions, window dragging/resizing, close guards, and active playback/file dragging while switching. Windows/Linux builds were not run; their titlebar controls and presentation remain unchanged.

## Project Settings dialog (2026-10-11)

- Added File > Project Settings… to native macOS and application File menus. The shared dialog has one Project name field with Apply/Enter and Cancel/Escape. It selects the current name, retains drafts until confirmation, rejects blank/control-character names, and marks only an actual rename as modified. Project folders remain unchanged.
- Automated checks cover draft selection, Unicode names, whitespace normalization, Apply/Enter, Cancel/Escape, invalid names, unchanged names, title/unsaved-marker updates, arrangement/selection preservation, shared menu dispatch and modal guards, and regular Save/reopen in the existing folder.
- Passed `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (235 tests), and `git diff --check`, with the local Cargo cache. Logs: `work/project-settings-workspace.log` and `work/project-settings-clippy.log`.
- Rebuilt `target/debug/DAW.app` with `sh tools/package_macos.sh debug`. Native macOS checks verified the File menu item, dialog layout, selected current name, unchanged title while typing, disabled background controls, Enter commit, and the updated title/unsaved marker. Discarded the temporary rename and restored the empty Untitled workspace.
- Windows/Linux builds and native interactions were not run. Native file-saving checks and rename interaction during active playback remain unverified; Save/reopen is covered by an automated temporary-folder test.

## Track header Mute/Solo controls (2026-10-11)

- Moved M and S to the far right of the track header, immediately left of the full-height stereo meters. Retained the left grip, centered track name, button sizes, tooltips, accessible toggle states, and individual-track actions. Gain and pan knobs/inputs occupy the second row.
- Updated existing interaction checks for the new header and lower-row positions. Alignment checks cover centered names and M/S alignment/order. Track-panel selection, button toggles, gain/pan inputs, grip/name reordering, and shortcuts pass.
- Passed `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (235 tests), and `git diff --check`, with the local Cargo cache. Logs: `work/track-header-controls-workspace.log` and `work/track-header-controls-clippy.log`.
- Rebuilt `target/debug/DAW.app`. Native macOS checks verified the header layout and Mute/Solo toggles on a temporary empty track. Removed the temporary track and restored the empty Untitled workspace. Windows/Linux native UI and live audio checks were not run.

## Timeline and Mixer views — 2026-10-11

- Added Timeline (default) and Mixer modes over the same session. Mixer shows track names at the top, pan and gain controls at the bottom, vertical gain faders with dB scales, stereo meters, and Mute/Solo buttons. Audio strips follow project order and scroll horizontally; Master remains fixed at the right edge.
- Mixer pan uses the existing `knobs.rs` design. `faders.rs` provides the standard gain fader component with themed surfaces, numeric accessibility values, a -60 to +12 dB drag range, and double-click reset. Both views share track/master values, selection, context actions, name editing, and playback sync. View changes retain timeline zoom/scroll and cancel text drafts without changing project data. Hidden timeline geometry cannot accept a file drop in Mixer.
- Six egui interaction tests cover view changes and retained project/selection/viewport state; ordered strips, horizontal scrolling, and pinned/aligned Master during resizing; mute/solo and group shortcuts; shared track/master gain controls; numeric commit/cancel/invalid input and centered readouts; name editing; preserved gain values outside the fader range; modal guards; and titlebar placement.
- `CARGO_HOME="$PWD/work/cargo-home" cargo fmt --all --check`, `cargo test --workspace --offline` (241 tests), and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. The macOS debug bundle was rebuilt with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`.
- Native macOS checks confirmed the mixer layout, shared pan knob, bottom controls, Master at the right edge, mute toggling, and a fader change to -9 dB reflected in Timeline. View controls appeared beside the project title with Custom and in a separate toolbar below the titlebar with Native. Restored the original Custom preference and cleared the temporary unsaved test tracks. Native scrolling/resizing, audible mixer changes, and Windows/Linux native layout remain unverified; scrolling/resizing are covered by egui tests.
- This supersedes the earlier requirement that Native macOS places playback controls directly below the titlebar: the view toolbar now occupies that position. Timeline keeps its existing compact track rows and bottom-pinned Master.

## Fine gain fader adjustment — 2026-10-11

- The shared track/Master gain fader now applies tenfold slower vertical adjustment while Shift is held. Pressing or releasing Shift during a drag retains the current value without a jump. Idle frames and mouse release retain it as well; movement remains within -60 to +12 dB. The tooltip documents Shift.
- Two pointer-input tests cover sensitivity, incremental motion, modifier changes during a drag, idle/release stability, and range limits. The existing mixer interaction test now explicitly releases Shift after group selection before testing ordinary fader clicks.
- Workspace tests pass (243), formatting and strict workspace Clippy pass, and the macOS debug bundle was rebuilt. Commands used the local `work/cargo-home` cache; Cargo checks used `--offline`. Native Shift-drag and Windows/Linux gestures remain unverified.

## Mixer header colors and strip borders — 2026-10-11

- Audio strip name headers use the track's inherited clip header background and contrasting text. Selection does not change these colors; individual clip overrides do not change the strip header. Audio and Master strips retain thin 1-point outer borders, including the top and bottom. Selected audio strips use an inset outline to keep those borders visible.
- The egui paint test covers light/dark themes, selected/unselected strips, light/dark track colors, the legacy clip palette, clip overrides, text contrast, full-width header fills, and retained outer borders on audio/Master strips.
- Passed `CARGO_HOME="$PWD/work/cargo-home" cargo fmt --all --check`, `cargo test --workspace --offline` (244 tests), `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `git diff --check`. Rebuilt the macOS debug bundle with `sh tools/package_macos.sh debug`. Logs: `work/mixer-header-tests.log`, `work/mixer-header-clippy.log`, and `work/mixer-header-package.log`.
- Native macOS checks confirmed the full colored name header and thin strip borders in the rebuilt app. Cleared the temporary empty test track and restored the clean Untitled Timeline view. Windows/Linux native appearance was not checked.

## Compact mixer headers — 2026-10-11

- Reduced mixer name headers from 36 to 24 points high and name text from 13 to 11 points. Names remain centered. Master and track-name inputs use the smaller text; Timeline retains its existing text size. Clip-derived colors, thin strip borders, and bottom control alignment remain intact.
- Updated the existing egui paint check to verify compact header height, text size, horizontal/vertical name centering, and existing theme/selection colors and borders. Existing mixer name-editing and layout checks also pass.
- Passed `CARGO_HOME="$PWD/work/cargo-home" cargo fmt --all --check`, `cargo test --workspace --offline` (244 tests), `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `git diff --check`. Rebuilt the macOS debug bundle with `sh tools/package_macos.sh debug`. Logs: `work/mixer-compact-header-tests.log`, `work/mixer-compact-header-clippy.log`, and `work/mixer-compact-header-package.log`.
- This size change was checked through egui tests; native platform appearance was not checked again.

## Shared mixer/status border — 2026-10-11

- Audio strips and Master now meet the status bar with one shared 1-point line and no gap. Their top/side borders remain; selection uses the top and sides rather than stacking another bottom outline. Timeline retains its existing separator.
- Existing egui checks verify flush audio/Master boundaries and exactly one full-width separator during resizing and horizontal scrolling. Header/theme/selection tests retain checks for thin top/side borders. All 244 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Logs: `work/mixer-bottom-tests.log`, `work/mixer-bottom-clippy.log`, and `work/mixer-bottom-package.log`.
- Rebuilt the macOS debug app and verified the thin bottom line beneath a selected strip and Master. The smaller centered headers from the previous change were also visible. Removed the temporary empty track and restored the clean Untitled Timeline view. Windows/Linux native appearance remains unverified.
- This supersedes the separate bottom strip and status outlines described in the earlier mixer border record.

## Selected mixer bottom outline — 2026-10-11

- Restored the bottom accent edge for selected strips. It paints over the existing shared separator at the same position and 1-point width, retaining a flush boundary without an extra line or gap. Master retains the neutral separator. Selection painting stays clipped to the scrolling track area.
- The existing paint check now verifies the bottom accent line only when selected, in both themes and with multiple track colors. All 244 workspace tests, formatting, strict workspace Clippy, and diff checks pass. The macOS debug bundle was rebuilt. Logs: `work/mixer-selected-bottom-tests.log`, `work/mixer-selected-bottom-clippy.log`, and `work/mixer-selected-bottom-package.log`.
- Native macOS appearance confirms the restored bottom outline and flush status boundary. Cleared the temporary test strip and restored the clean Untitled Timeline view. Windows/Linux native appearance remains unverified. This supersedes the previous record's top-and-sides-only selection outline.

## Equal mixer selection outline widths — 2026-10-11

- The selected strip's bottom edge now uses the shared selection stroke, matching the top and side widths. Its center is inset by half that width so the complete stroke stays inside the strip, flush with the status bar. Master and unselected borders retain their thin neutral line.
- Updated the existing selected/unselected paint check to verify the shared bottom stroke and inside position in both themes. All 244 workspace tests, formatting, strict workspace Clippy, and diff checks pass. Rebuilt the macOS debug app. Logs: `work/mixer-outline-width-tests.log`, `work/mixer-outline-width-clippy.log`, and `work/mixer-outline-width-package.log`.
- Native appearance was not checked again; the open Example project was retained. This supersedes the previous record's 1-point selected bottom edge.

## Single dividers between mixer strips — 2026-10-11

- Adjacent strips now share one 1-point neutral divider. Each strip draws its left divider; only the last exposed audio edge and the outer Master edge also draw a right border. The thicker selected outline, equal bottom stroke, and flush status boundary are retained.
- Updated existing egui paint checks for thin top/left borders and verified exactly one neutral divider at visible adjacent boundaries after horizontal scrolling and window resizing. Selected outlines remain covered in both themes. All 244 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode.
- Rebuilt the macOS debug app. Logs: `work/mixer-divider-tests.log`, `work/mixer-divider-clippy.log`, and `work/mixer-divider-package.log`. Native appearance was not checked again; Windows/Linux native checks remain unverified.

## Shared mixer top border — 2026-10-11

- Tracks and Master now share one 1-point top line beneath the playback toolbar. Mixer omits the separate toolbar separator and individual strip top borders. The colored headers meet the toolbar without a gap; thicker selected outlines remain intact. Timeline retains its existing separator.
- Existing egui paint checks verify a flush toolbar/header boundary and exactly one thin top line, covering selected/unselected strips in both themes. All 244 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode.
- Rebuilt the macOS debug app. Logs: `work/mixer-top-border-tests.log`, `work/mixer-top-border-clippy.log`, and `work/mixer-top-border-package.log`. Native appearance was not checked again; Windows/Linux native checks remain unverified.

## Consistent Timeline/Mixer view buttons — 2026-10-11

- View buttons now use the same font, padding, height, radius, and borderless design in the custom titlebar and separate view toolbar. Selected views retain the runtime accent-tinted background with contrasting text; unselected views use secondary text and a transparent background. Hover/press keeps colors and dimensions stable. Selection accessibility and existing view-switch guards remain.
- The new egui paint/input test covers both titlebar layouts, Timeline/Mixer selection, dark/light themes, Deep Orange/Yellow accents, and idle/hover/press/release. It checks matching sizes, retained selected fills, contrasting/secondary text, no button stroke, and clean no-op clicks on the selected view. Existing switching and modal-guard tests pass.
- All 245 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/view-buttons-tests.log`, `work/view-buttons-clippy.log`, and `work/view-buttons-package.log`.
- Native appearance was not checked again; Windows/Linux native checks remain unverified.

## View-button hover text — 2026-10-11

- Hovered Timeline/Mixer buttons now use the same contrasting text as the selected view. Unselected text returns to secondary when the pointer moves away. Background fills, button dimensions, titlebar consistency, selected states, and action guards remain intact. The style uses local egui foreground states so the rest of the UI is unaffected.
- Extended the existing view-button test to hover the unselected view and move away in both titlebar layouts, dark/light themes, both view modes, and Deep Orange/Yellow accents. It checks rendered text colors, no selected fill on unselected hover, retained backgrounds, and stable geometry.
- All 245 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/view-hover-tests.log`, `work/view-hover-clippy.log`, and `work/view-hover-package.log`.
- Native hover was not checked again; Windows/Linux native checks remain unverified. This supersedes the earlier requirement that unselected view text remains secondary on hover.

## Flush selected mixer right outline — 2026-10-11

- Removed the 1-point gap between a selected strip's right outline and the next strip. The right stroke now reaches the strip boundary; the shared selection width and other edges retain their positions. Painting remains clipped to the scrolling track area.
- Extended the existing header paint test with an adjacent track and checked that the right stroke meets its left boundary in dark/light themes and across track colors. All 245 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/mixer-right-outline-targeted.log`, `work/mixer-right-outline-tests.log`, `work/mixer-right-outline-clippy.log`, and `work/mixer-right-outline-package.log`.
- Native appearance was not checked again; Windows/Linux native checks remain unverified.

## Left-aligned Timeline track names — 2026-10-11

- Timeline track names now start directly after the grip, using the normal row spacing. The name field fills the remaining space before Mute/Solo, which retain their right-side positions. Rename inputs use the same left alignment; Mixer headers retain their centered names.
- Updated the existing grip/reorder test to check the name's left position and unchanged Mute/Solo alignment, including reorder release and cancellation. All 245 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/track-name-left-targeted.log`, `work/track-name-left-tests.log`, `work/track-name-left-clippy.log`, and `work/track-name-left-package.log`.
- Native appearance was not checked; Windows/Linux native checks remain unverified. This supersedes the earlier centered Timeline name requirement.

## Select track clips from empty header space — 2026-10-11

- Double-clicking blank name-field space in Timeline or Mixer selects all clips on that track, replaces the previous clip selection, and makes the earliest clip primary. An empty track clears clip selection. Timeline header gaps use the same selection behavior. Visible name text retains renaming; grip and control actions remain separate. Selection uses the existing action guards and does not modify project data.
- Added egui input tests for both views, replacement of another track's clip selection, single-click preservation, empty tracks, group copy, header spacing, name editing, control isolation, jobs, and settings dialogs. Updated the rename test to click visible name text rather than blank field space.
- All 248 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/track-header-double-click-targeted.log`, `work/track-header-double-click-tests.log`, `work/track-header-double-click-clippy.log`, and `work/track-header-double-click-package.log`.
- Native interaction was not checked; Windows/Linux native checks remain unverified.

## Rename track menus and shortcut — 2026-10-11

- Added Rename track before Duplicate in the Track menu and first in track context menus. Both display Command+R on macOS or Ctrl+R on Windows/Linux and share guarded `TrackAction::Rename` dispatch. Rename requires exactly one selected audio track and opens the existing focused name draft with its text selected. Opening preserves selection and project data; Enter commits, while Escape/focus loss cancels.
- The macOS native item uses the Command modifier and consumes forwarded matching key events. UI shortcuts ignore repeat and extra modifiers and retain the existing text-input, focus, pointer, gesture, job, and dialog guards.
- Added egui tests for shortcut editing and Enter/Escape in Timeline/Mixer, both menus' release actions and shortcut labels, single-selection requirements, repeat, and extra modifiers. Extended blocked-action tests to send Command+R and verify draft isolation. Existing name focus-loss and track action tests pass.
- All 251 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/track-rename-targeted.log`, `work/track-rename-tests.log`, `work/track-rename-clippy.log`, and `work/track-rename-package.log`.
- Native menu and shortcut interaction was not checked; Windows/Linux native checks remain unverified.

## Command symbol in menu shortcut labels — 2026-10-11

- All egui menus now use the shared `toolbars::format_menu_shortcut` formatter. macOS labels show Command as ⌘, such as ⌘R, ⌘S, and Shift+⌘S, without requiring unrelated modifier glyphs. Other modifier names remain text. Windows/Linux retain Ctrl labels. Native macOS menus retain the system's Command symbol. Key bindings and action guards are unchanged.
- Added a font check for the Command glyph and menu paint coverage for File, Edit, Track, and track context menus across macOS/Windows/Linux formatting, both themes, and enabled/disabled states. Existing rename-menu input tests use the shared formatter and pass.
- All 253 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/menu-command-symbol-glyph.log`, `work/menu-command-symbol-targeted.log`, `work/menu-command-symbol-tests.log`, `work/menu-command-symbol-clippy.log`, and `work/menu-command-symbol-package.log`.
- Native appearance was not checked; Windows/Linux native checks remain unverified.

## Command glyph size in menu shortcuts — 2026-10-11

- Added a menu-specific font family that retains Outfit letters and reduces the Command fallback glyph by 20%. All egui menu shortcut labels use `toolbars::menu_shortcut_text`, which retains standard weak-text and disabled styling. Other UI fonts, Windows/Linux labels, native macOS menus, and key bindings remain unchanged.
- Added rendered-glyph checks at 13 and 18 points with 1×, 1.5×, and 2× display scales. Command matches R's height within 1.5 logical points and its bottom alignment within 1 point; standalone Outfit letters retain identical glyphs and layout. Extended menu paint coverage to check the font family across all menus, both themes, and enabled/disabled states.
- All 254 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Formatting, strict workspace Clippy, and diff checks pass. Rebuilt the macOS debug app with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/menu-command-size-targeted.log`, `work/menu-command-size-tests.log`, `work/menu-command-size-clippy.log`, and `work/menu-command-size-package.log`.
- Native appearance was not checked. The running app and its open project were left intact; the rebuilt bundle is on disk.

## Square Mute/Solo buttons and smaller labels — 2026-10-11

- Timeline and Mixer now share 22 × 22-point square Mute/Solo buttons with 11-point Outfit labels. Timeline reserves the narrower pair's width in its header; Mixer keeps the pair centered. Tooltips, accessible names/states, selected styling, and toggle actions remain intact.
- Extended existing Timeline and Mixer paint/input tests to check square bounds and label font size. Updated Timeline click coordinates for the narrower controls. Name alignment, reordering, header selection, and shared mute/solo behavior pass.
- All 254 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. Rebuilt the macOS debug app. Logs: `work/square-track-toggles-tests.log`, `work/square-track-toggles-clippy.log`, and `work/square-track-toggles-package.log`.
- Native appearance was not checked; the running app and open project were left intact.

## View menu — 2026-10-11

- Added View after Track in native macOS and egui menu bars, with Timeline and Mixer items. Only the active view is checked. Native item states update after both menu and toolbar changes. Enabled states and dispatch use the same guarded view-switching methods as the existing buttons; switching preserves project data, selection, timeline scroll/zoom, and cancels text drafts. Selecting the active view is a no-op.
- Added egui paint/input tests for both themes, one correctly positioned check, release-only switching, active-item selection, and unchanged project/selection/scroll/zoom. Shared dispatch tests cover jobs, Settings, and draft cleanup. Existing view-button appearance and interaction tests pass. Enabled AppKit's NSCell feature to use native menu state APIs.
- All 256 workspace tests, formatting, strict workspace Clippy, and diff checks pass using the local Cargo cache in offline mode. The macOS desktop check and debug packaging pass. Logs: `work/view-menu-targeted.log`, `work/view-menu-check.log`, `work/view-menu-tests.log`, `work/view-menu-clippy.log`, and `work/view-menu-package.log`.
- Native menu interaction and Windows/Linux native checks remain unverified. The running app and open project were left intact.


## Project processing rate and buffer settings — 2026-10-11

- File > Project Settings now includes processing sample rate (44.1/48/88.2/96/176.4/192 kHz) and buffer size (32/64/128/256/512/1024/2048/4096 samples), plus estimated buffer duration. Defaults remain 48 kHz and 512 samples. The buffer persists in the manifest and defaults to 512 in older projects. Cancel discards all draft values.
- Apply prepares rate conversion, waveform peaks, and Rubber Band buffers on a worker, then publishes the name, processing settings, and frame coordinates together. Preserve timing to sample precision, contiguous boundaries, source offsets, repeats, stretch ratios, playhead, and enabled/disabled loop selection. Re-decode original sources at the selected rate; unavailable sources with cached PCM convert that cache with a warning. Invalid settings, empty ranges, overflow, or damaged sources preserve the original session. Successful audio changes stop playback and recreate output on the next Play; rate changes clear copied clips with old frame units. Source files stay intact.
- Import, drag previews, ruler snapping, monitors, waveforms, rendering, fades, and smoothing use the selected processing rate. Request the project buffer size from CPAL, clamp to supported limits, and fall back to the device default if necessary. Device-rate conversion uses preallocated buffers. WAV exports use the project rate; MP3 converts project rates above 48 kHz to 48 kHz. Export dialogs show the actual output rate. This supersedes the fixed 48 kHz requirements in earlier validation entries.
- Added model tests for every rate, boundary/timing preservation, unsupported settings, empty clips, and overflow. Workflow tests cover original-source preparation, changed-rate stretching, import, save/reopen, legacy buffer defaults, cached-source fallback, failed conversion, and WAV/MP3 duration. Device-free output tests exercise all 48 combinations of project rate/buffer against a 48 kHz output stream. Engine tests check live/offline samples and 5 ms fades at all rates. egui tests cover dropdown choices, draft cancellation, worker-only publication, and failed Apply retaining name, arrangement, and modified state; ruler tests verify musical positions and snapping at every rate.
- All 266 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Formatting, strict workspace Clippy, CLI build, `python3 tools/check_cli.py`, and diff checks pass. Rebuilt the macOS debug app with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/project-audio-tests.log`, `work/project-audio-dialog-tests.log`, `work/project-audio-format.log`, `work/project-audio-clippy.log`, `work/project-audio-cli-build.log`, `work/project-audio-cli-smoke.log`, and `work/project-audio-package.log`.
- Native dialog interaction, hardware buffer negotiation, latency/dropout behavior, and Windows/Linux native checks remain unverified. The running app and its open project were left intact.

## Combined menu and view toolbar — 2026-10-11

- Windows and Linux now use one application toolbar: File, Edit, Track, and View menus, a vertical separator, then Timeline/Mixer buttons. Playback controls remain below. The project title stays in the native titlebar; Linux no longer repeats it between the menus and view buttons. This supersedes the separate Windows/Linux view toolbar recorded above. macOS titlebar layouts retain their existing behavior.
- Added an egui regression test for menu/button order, shared vertical alignment, one separator, no repeated project title, and release-only switching without modifying the project. It covers both themes, both views, and minimum/default window widths. Existing view controls and their action guards are reused.
- `cargo fmt --all --check` and `git diff --check` passed. Targeted UI tests, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` could not complete on this Windows host. The existing target directory has dependency-loading errors (`E0463`), and Windows Application Control blocks loading `serde_derive` with error 4551. An isolated build in `work/menu-view-target` was also blocked by Application Control before the `parking_lot_core` build script could execute. No policy settings were changed. Logs for the workspace attempts are `work/menu-view-clippy.log` and `work/menu-view-tests.log`.
- The regression test has not run successfully. Native Windows/Linux layout and interaction checks and macOS regression checks remain unverified.

## Optional Windows custom titlebar — 2026-10-11

- Windows Settings now offers Native (default) and Custom titlebars. The preference persists separately from projects and applies without replacing the window. Custom combines the burger menu, project title and unsaved marker, separator, and Timeline/Mixer buttons. The burger reuses File/Edit/Track/View actions. Long titles shorten to reserve the caption controls; playback remains below the header. Native mode retains the combined menu/view toolbar from the preceding entry. macOS and Linux layouts retain their behavior.
- Custom draws Windows-style caption buttons in the app. The desktop adapter uses native Minimize, Maximize/Restore, and Close hit targets, retains window styles, and exposes the title and unused header space for native window dragging. OS-drawn caption controls were not reliable over the eframe GPU client surface in the custom frame preview. Native mode remains available for OS-drawn controls. This replaces the initial custom-frame approach; it does not add a Windows App SDK host.
- Eight focused titlebar tests pass. Coverage includes Settings persistence and switching, menu dispatch, title/control layout in both themes at minimum/default widths, long titles, view changes without project edits, popup/modal drag guards, close prompts, job guards, and caption hover colors. The desktop adapter test passes on a hidden owned Win32 window. It checks client-frame restoration, retained styles, signed hit-test coordinates, caption hits in normal/maximized states, modal drag exclusion, and subclass removal when the window is destroyed.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p daw-desktop`, and `git diff --check` pass on Windows with the pinned toolchain and libclang configured. `cargo test --workspace --no-fail-fast` completes with two existing UI fixture failures: `export_jobs_use_selected_settings_and_overwrite_retains_them` uses a source path that is not absolute on Windows; `save_before_close_waits_for_named_save_and_cancels_on_failure_or_cancellation` cannot find its fixture path. Both also fail in the unchanged baseline (`work/windows-header-baseline-tests.log`). All other workspace tests pass. Final command logs are `work/windows-titlebar-clippy.log`, `work/windows-titlebar-build.log`, and `work/windows-titlebar-tests.log`.
- An isolated preview using the production adapter was checked in both themes, normal and maximized, on this Windows host. Captures show the requested header and visible caption glyphs. Live hit tests return Minimize, Maximize, and Close for the displayed button centers in both window states. Preview files are under `work/windows-titlebar-preview`; no user project or application preferences were changed. Physical mouse dragging/resizing, caption clicks, Snap flyouts, mixed-DPI monitors, and native accessibility activation still need checks. macOS/Linux builds and audio-device checks were not run for this change.

## OS-drawn Windows caption buttons — 2026-10-11

- Replaced the app-drawn caption controls from the preceding entry with Windows' native controls. Windows now uses eframe's Glow/OpenGL renderer and an alpha-enabled framebuffer. The UI clips its header background and content before the caption region. DWM draws the Minimize, Maximize/Restore, and Close controls and handles their hover, hit testing, and actions. No caption icons, hover fills, replacement hit targets, or caption click handlers remain in egui. macOS/Linux retain wgpu. Native mode remains the Windows default; the custom layout and saved preference remain available.
- Preserve the original top client coordinate during maximization. Moving that coordinate prevented DWM from recognizing its own buttons. Offset only the app header past the off-screen resize frame, and refresh the frame after activation, size, DPI, and titlebar changes. Refresh native theme colors when the app theme changes. Reserve the caption width reported by DWM. The renderer dependency notices are under `crates/ui/assets/licenses`; existing packaging copies that directory.
- The desktop frame test and six focused UI titlebar tests pass. UI coverage checks both themes, short/long titles, minimum/default widths, the maximized header offset, view/menu actions, popup/modal drag guards, and an unpainted native caption region. The hidden Win32 test covers frame restoration, retained styles, drag/resize hits, maximized top behavior, modal drag exclusion, and subclass removal. Native button appearance and clicks are checked separately in the live preview.
- Native input checks on an isolated preview verified OS-drawn glyphs, native close hover, and caption hit results in normal/maximized windows. Real Windows mouse input through `SendInput` successfully minimized, maximized, restored, and closed a clean preview. Switching Custom → Native → Custom retained the same HWND. Dark → Light updates the native controls in both window states. A separate preview added a temporary in-memory track, then clicked the native Close button: the window stayed open and showed Save/Discard/Cancel. Only preview windows and ignored scratch files were used; user projects and application preferences were untouched. Captures are under `work/windows-titlebar-preview/final-native-*`, with `native-light-max-final-header.png` and `native-unsaved-prompt.png` covering the final theme refresh and close guard.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `cargo build -p daw-desktop --offline` pass. `cargo test --workspace --offline --no-fail-fast` completes with the same two pre-existing Windows path-fixture failures named in the preceding entry; all other tests pass. Logs are `work/windows-native-caption-clippy.log`, `work/windows-native-caption-build.log`, and `work/windows-native-caption-tests.log`. `git diff --check` passes.
- Snap flyouts, physical drag/resize gestures, mixed-DPI monitor transitions, and native accessibility activation remain unverified. Native input checks used this host's graphics driver; other drivers still need checks. macOS/Linux builds and audio devices were not checked for this change.

## Borderless Windows burger button — 2026-10-11

- Removed the custom header burger button's border in all widget states. Retained its hover fill, tooltip, accessibility name, and existing menu actions. The style override applies only to this button.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. `cargo test --workspace --offline --no-fail-fast` passes except for the same two existing Windows path-fixture failures recorded above. The burger menu test passes. Logs are `work/windows-burger-border-clippy.log` and `work/windows-burger-border-tests.log`.
- Built the isolated Windows preview with the production header and verified its borderless idle appearance. The capture is `work/windows-titlebar-preview/borderless-burger-header.png`. No user project or application preferences were changed. Native hover appearance and other platforms were not checked for this styling change.

## Burger toolbar below the native Windows titlebar — 2026-10-11

- Native Windows mode now uses `[burger menu] | [Timeline · Mixer]` in the toolbar below the system titlebar. Both Windows titlebar modes share the borderless burger and File/Edit/Track/View menu actions. The project title stays in the native titlebar. Playback controls remain in the next row. This supersedes the expanded menu labels in native Windows mode recorded above; Linux and macOS layouts retain their behavior.
- Updated the existing layout and menu tests for the new presentation. Automated checks cover both themes, both views, minimum/default widths, one separator, view switching without project edits, native title updates, titlebar switching, and opening File > Settings from the burger in both Windows modes.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `cargo build -p daw-desktop --offline` pass. `cargo test --workspace --offline --no-fail-fast` passes except for the same two existing Windows path-fixture failures recorded above. Logs are `work/windows-native-burger-clippy.log`, `work/windows-native-burger-build.log`, and `work/windows-native-burger-tests.log`.
- The isolated Windows preview confirms the native titlebar and requested toolbar layout. The capture is `work/windows-titlebar-preview/native-burger-toolbar.png`. Only preview windows and ignored scratch data were used. Native menu mouse interaction and macOS/Linux builds were not checked for this change.

## Shared Linux and native Windows burger toolbar — 2026-10-11

- Linux now uses the same `[burger menu] | [Timeline · Mixer]` toolbar below its native titlebar as native Windows mode. Moved the borderless burger and toolbar composition into `crates/ui/src/application_menu.rs`, shared by both platforms. Windows custom mode reuses the same burger. The native project title and the playback row remain in their existing positions. This supersedes the expanded Linux menu labels recorded above.
- Updated the shared layout and View-menu tests for both platforms. Added a shared native-toolbar input test that opens all four menu groups and File > Settings in both themes without changing project data. These tests and the Windows titlebar regression tests pass on Windows.
- On Windows, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build -p daw-desktop --offline`, and `git diff --check` pass. `cargo test --workspace --offline --no-fail-fast` passes except for the same two existing Windows path-fixture failures recorded above. Logs are `work/linux-burger-toolbar-clippy.log`, `work/linux-burger-toolbar-build.log`, and `work/linux-burger-toolbar-tests.log`.
- Linux builds and native UI checks remain unverified. This host has no installed Linux Rust target or Windows Subsystem for Linux. No platform tools were installed. macOS and audio-device checks were not run for this change.

## Shared collision handling for clip duplication — 2026-10-11

- Moves retain the existing trim, split, and deletion rules. Drag duplication and paste now use the same `Edit::OverwriteClips` transaction. A copy can trim, split, or remove its original when their ranges intersect. This supersedes the earlier overlap rejection for duplication and paste; committed clips on one track still do not overlap. Menu imports retain overlap rejection.
- Duplicate previews include affected neighbors without changing project data or playback. Mouse release commits copies and neighbor changes together; cancellation retains the original arrangement. Group copies preserve timing, track gaps, source metadata, and source audio. Invalid ranges roll back the full edit, including newly required tracks and selection changes.
- All 268 workspace tests pass, including duplicate edge trims, middle splits, full coverage, loop-phase mapping, group paste, self-overlap, preview/release agreement, focus-loss cancellation, snapping, and invalid-group rollback. `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, and `git diff --check` pass. The macOS debug bundle is rebuilt with `sh tools/package_macos.sh debug` at `target/debug/DAW.app`. Logs are `work/overlap-tests.log`, `work/overlap-clippy.log`, and `work/overlap-package.log`.
- Native mouse interaction, real-device playback, and Windows/Linux builds were not checked for this change.


## First MIDI patterns and oscillators — 2026-10-11

- Added MIDI tracks and embedded pattern assets, a MIDI piano roll in Edit, and Sine/Saw/Square/Triangle oscillator controls in Units. Patterns use the existing clip editing and collision transactions. Pattern edits create a new source reference for the edited clip; copies keep their original pattern. MIDI track instruments are separate from source content. Wrong-type placements reject atomically. New destination tracks preserve each source track's instrument, including group moves/duplicates and empty track gaps.
- Added portable `daw-unit-core` and `daw-oscillator` crates. FunDSP remains behind the shared source API. Versioned oscillator settings persist with tracks. Note schedules and source state prepare before publication. One voice per MIDI track uses latest-note replacement, stale note-off protection, and 5 ms transitions. Pause preserves state; seek/stop reset it. Muted voices continue processing. Compatible renderer replacements transfer state without callback allocation. Clip fades suppress release tails outside trimmed clips.
- Playback and offline export use sequential MIDI processing with the existing audio mixer, project processing rate, pan, mute/solo, and Master controls. MIDI ticks remain fixed through tempo and processing-rate changes. Derived clip and source frame ranges scale and stay within rounded source bounds; final overlap and overflow checks remain transactional. Save, reopen, Save As, mixed audio/MIDI WAV export, and MP3 export preserve the new state. The macOS bundle includes the new dependency license and source notices.
- Automated UI checks cover MIDI creation/actions, view switching, copy/paste, note addition/deletion, body movement with multiple pointer updates, both resize edges, preview-only drags, release coordinates, and Escape cancellation. Model tests cover source isolation, invalid input/placement rollback, repeated non-integer tempo changes, every processing rate, loops, and stretched source bounds. Engine tests cover all four waveforms at all six rates and eight block sizes, finite/nonzero output, block-size independence, live/offline agreement, pause/seek/stop, latest-note replacement, mute state transfer, trimmed gaps, and allocation counting during note events, loops, and renderer replacement.
- All 281 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. `cargo fmt --all --check`, strict workspace/all-target Clippy, CLI build, `python3 tools/check_cli.py`, changed documentation file links, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-tests.log`, `work/midi-ui.log`, `work/midi-clippy.log`, `work/midi-cli-build.log`, `work/midi-cli-check.log`, and `work/midi-package.log`.
- Native macOS checks used only the scratch project under `work/midi-preview`. Confirmed MIDI track/pattern creation, clip-header double-click into Edit, note selection/addition, waveform choices, the shared gain knob, readable sharp-note labels, and the piano-roll ruler. Save persisted nine notes with a Square instrument on one track and one note with a Sine instrument on another. Reopening the project restored both tracks and the Square instrument setting. CLI validation and WAV/MP3 export of that saved project succeed; the float32 WAV contains nonzero samples. Playback opened MacBook Pro Speakers at 48 kHz with the requested fixed 512-sample buffer, advanced the playhead, and showed nonzero track/Master meters. Pause worked. This is an output-path check, not an audio-quality or dropout measurement.
- Native drag automation did not activate a note drag. Body and edge dragging remain verified through egui input tests, with physical native mouse/trackpad checks still pending. Windows/Linux builds and native UI/device checks, other device configurations, and the full eight-voice workload with representative effects remain unverified. This first version has one voice and one oscillator per MIDI track, starts patterns at one bar, and has no pattern-length editor, MIDI file/hardware input, effects graph, or audio/beat editor.


## MIDI editor gestures and polyphonic playback — 2026-10-11

- MIDI track/pattern creation is exclusive to Timeline, including MIDI paste and MIDI track duplication. Double-click empty MIDI lane space to create and select a one-beat pattern, retaining Timeline. Existing saved source lengths remain intact. Fixed a background click that could clear the newly created selection.
- Edit reuses Timeline's ruler painting, project playhead, loop-selection interactions, snapping, and right-aligned playback-toolbar zoom control. Horizontal transforms remain separate. Pattern notes and source bounds align to the clip's project-time origin; pre-zero source content remains editable while seeking clamps at project zero. Piano keys stay pinned during horizontal scrolling. Subdivisions and note snapping follow zoom automatically; the manual selector is removed.
- Double-click deletes a note through an isolated pattern edit. Removed the extra pattern-name toolbar and instruction row. Selected-note velocity and Delete note are available from the pattern header's context menu.
- Pattern-header edges resize source content through a shared model transaction. Left-edge resizing keeps the clip start fixed and shifts retained notes; both edges remove outside notes and shorten crossing notes. Preserve other source users, compatible trim/repeat/stretch metadata, and use shared trim/split/removal placement rules for expansion. Preview source and range changes during the gesture; release commits them together. Shift bypasses snapping; Escape/focus loss cancels.
- Empty-space note drawing captures its starting pitch and ordered time range. Preview same-pitch edge trims, middle splits, and covered-note deletion without changing project data or playback. Release commits the new source once. Other pitches remain intact and can play together. Click-only creation uses the visible grid size. Drawing backwards retains its starting pitch; Escape/focus loss cancels. Capture the initial press even when press/move/release arrive in one frame, including releases outside the pattern bounds.
- MIDI tracks now render polyphonic patterns. Prepare independent oscillator slots for overlapping note ranges and their 5 ms release tails before publication. There is no fixed voice cap or callback allocation. Each note-off addresses its assigned note; seeking reconstructs every held note. Playback, export, pause, looping, mute/solo, and renderer replacement retain the shared processing path. This supersedes the one-voice MIDI-track implementation recorded above; beat-lane polyphony remains pending.
- All 293 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Added coverage for one-beat creation, creation guards, shared ruler/zoom geometry, loop selection/seeking, double-click source isolation, pattern-edge previews/releases/Shift/cancellation, automatic subdivisions, note drawing and overlap remnants, backwards drawing, focus loss, and independent polyphonic rendering/seeking. The chord test matches the sum of separately rendered pitches within floating-point tolerance. CLI exports of a scratch triad succeed in WAV and MP3; frequency projection of the float32 WAV confirms all three pitches. Save/reopen/Save As and mixed WAV/MP3 workflows now include a chord. All-waveform/rate/block-size checks and callback allocation counting pass with polyphonic schedules. Logs: `work/midi-polyphony-tests.log`, `work/midi-polyphony-engine.log`, `work/midi-drawing-focused.log`, `work/midi-polyphony-project.log`, and `work/midi-polyphony-clippy.log`.
- Strict workspace/all-target Clippy, formatting, diff checks, CLI build, and CLI smoke checks pass. The macOS debug bundle is rebuilt under `target/debug/DAW.app`. Build/smoke logs are `work/midi-polyphony-package.log`, `work/midi-polyphony-cli-build.log`, and `work/midi-polyphony-cli-check.log`.
- Native macOS checks on the scratch project confirmed empty-lane double-click creation, retained Timeline/selection, the shared Edit ruler and toolbar zoom position, the removed rows/selector/creation buttons, note creation, and Save. The saved source has 960 ticks, a 24,000-frame clip at 120 BPM/48 kHz, and a grid-sized note. Native drag automation remains unreliable; physical note drawing/resizing, pattern resizing, and polyphonic device playback need separate checks. Windows/Linux native checks, other devices, and the full effects workload remain unverified. Application preferences and user recordings were not changed.

## Track-wide MIDI editor and left-edge timing — 2026-10-11

- Edit now shows a track strip below the shared ruler and all patterns on the selected track in one piano roll. Ruler, strip, pattern borders, notes, and playhead use the same project-time transform. Selecting or editing another pattern retains zoom, horizontal scroll, and the pitch viewport. Pattern outlines distinguish contiguous clips in both themes. Trimmed offsets, stretch timing, and repeat phases determine each visible note occurrence; edits isolate the selected clip's source. Empty gaps do not create patterns.
- Left source resizing now moves the clip start and adjusts note ticks to preserve retained notes' project timing within sample-frame rounding. Extension adds space before those notes; left shortening keeps the clip end fixed. Clamp left extension at project zero and keep positive source/clip lengths. Trimmed ranges and looped arrangement lengths follow the requested duration change within source bounds. Overlap trims, splits, and removals still preview and commit through the shared model transaction on release. This supersedes the fixed-start resizing and pre-zero full-source display recorded in the previous entry.
- Below half a logical pixel per repetition, the piano roll shows pitch coverage and keeps pattern borders instead of visiting every repeat. Individual note editing needs more zoom at that density. The shared miniature-note painter also bounds repetition work. Automated coverage includes a ten-minute clip repeating a one-frame range at minimum zoom.
- All 301 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Added checks cover selected-track filtering, contiguous borders in both themes, strip/roll alignment through zoom and scroll, editing unselected clips without a viewport jump, copied-source isolation, trimmed repetitions, left-edge preview/release timing, project-start clamps, stretched source timing, neighbor trim/removal, and trimmed/looped range resizing. Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Logs: `work/midi-track-editor-tests.log` and `work/midi-track-editor-clippy.log`.
- CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-track-editor-cli-build.log`, `work/midi-track-editor-cli-check.log`, and `work/midi-track-editor-package.log`.
- Native macOS checking used only `work/midi-track-editor-preview`. Direct launch loaded its two contiguous patterns in Timeline; CLI validation also succeeds. Native automation did not complete selection/view switching reliably, so the new Edit layout and physical resize gestures remain unverified natively. An earlier file-picker attempt stayed on the opening screen during observation. The scratch app was closed. Windows/Linux native UI/device checks remain pending. No user projects, recordings, preferences, or staged changes were replaced.

## MIDI note selection and drawing modifier — 2026-10-12

- Plain empty-space dragging now selects notes intersecting a selection box on the selected track. Selection spans patterns and gaps, distinguishes clip copies that share note IDs, and selects repeated occurrences once per clip. An empty click clears selection. Selection stays in UI state and does not change source assets or mark the project modified. Delete/Backspace removes the selected notes through one batch while retaining unselected source users.
- Command-drag on macOS and Ctrl-drag on Windows/Linux draw notes in empty space; a modified click still creates a grid-sized note. This supersedes plain drawing in the earlier MIDI gesture entry. Capture gesture mode and modifiers at the initial press, including press/move/release in one frame. Modifier changes during a gesture do not switch modes. Drawing retains source isolation, snapping, Shift bypass, same-pitch overlap handling, and commit on release. Existing note movement and edge resizing remain single-note actions. Escape/focus loss cancels a selection gesture and restores the previous selection.
- All 305 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. All 21 MIDI UI tests pass, including new coverage for cross-pitch selection, backwards selection across copied patterns and gaps, selected-group deletion, retained source copies, repeated-note deduplication, empty-click clearing, cancellation, and modifier latching. Existing drawing tests now start with the platform modifier; release and cancellation behavior remain covered. Logs: `work/midi-selection-focused.log` and `work/midi-selection-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant local documentation links pass. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. Logs: `work/midi-selection-clippy.log` and `work/midi-selection-package.log`.
- Native trackpad/mouse gestures and Windows/Linux native modifier behavior were not checked in this change. Automated egui tests do not establish native input behavior. No user projects, recordings, preferences, or staged changes were replaced.

## MIDI note group movement and drag duplication — 2026-10-12

- Dragging a selected note body now moves the selected group across the selected track's patterns. Option on macOS and Ctrl on Windows/Linux change the move to duplication, matching clip duplication. The copy modifier can change during the drag; its release state decides the result. Copies use stable new note IDs and retain duration and velocity. Originals remain intact, including overlapping placements. The moved notes or copies remain selected after release. Note edges still resize one note even with the modifier held.
- Use one horizontal project-time displacement and one pitch displacement for the group. Clamp both at every selected source/pitch bound to retain spacing; differently stretched sources round their displacement to the nearest tick. The dragged note's visible subdivision controls snapping, with Shift bypass. Repeated occurrences select and copy a source note once per clip. A drag with no effective displacement creates no copies, source assets, or modified state.
- Per-clip source snapshots drive previews without changing project data or playback. Release applies changed patterns through one `Edit::Batch`; unselected clips keep their previous source. Escape/focus loss restores source data and the previous note selection. A press/move/release in one frame captures the original note position and completes the copy. Group movement supersedes the single-note movement limit in the preceding validation entry; group edge resizing remains pending.
- All 313 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. All 29 MIDI UI tests pass. New checks cover stable copy previews, retained originals/source copies, selected-group movement and duplication, source/pitch clamps, no-op releases, live modifier changes, Shift bypass, cancellation, repeated-note deduplication, differently stretched patterns, edge-resize behavior, and fast release. Logs: `work/midi-note-duplicate-focused.log` and `work/midi-note-duplicate-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-note-duplicate-clippy.log` and `work/midi-note-duplicate-package.log`.
- Native mouse/trackpad gestures and Windows/Linux native modifier behavior were not checked in this change. Automated egui tests do not establish native input behavior. No user projects, recordings, preferences, or staged changes were replaced.

## Shift note-selection toggles — 2026-10-12

- Shift-click adds or removes one note while retaining the other selected notes, including notes in different patterns that share source note IDs. Primary-clip changes retain the note group and viewport. Repeated Shift-clicks toggle selection instead of deleting a note. Double-click without Shift and secondary-click retain their delete behavior.
- Shift-drag in empty space toggles the intersecting notes against the selection at the initial press. The box captures its Shift mode at that press, keeps notes outside the box selected, and deduplicates repeated source-note occurrences before toggling. Repainting the same box does not toggle again. Empty Shift-clicks retain selection; ordinary empty clicks clear it. Escape/focus loss restores the previous selection. Selection changes do not change source assets, playback, or modified state.
- Shift remains the snapping bypass for note-body moves, edge resizes, and drawing. Groups formed by Shift-click still move together, with note changes previewed until release. Existing drag duplication remains available.
- All 317 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. All 33 MIDI UI tests pass. New checks cover cross-pattern toggles, shared note IDs, rapid repeated clicks, mixed selected/unselected box hits, retained outside notes, stable repeated previews, latched box mode, empty-click behavior, repeated-note deduplication, cancellation, and group movement with free time adjustment. Logs: `work/midi-shift-selection-focused.log` and `work/midi-shift-selection-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-shift-selection-clippy.log` and `work/midi-shift-selection-package.log`.
- Native mouse/trackpad and Windows/Linux native input checks were not run for this change. Automated egui tests do not establish native gesture behavior. No user projects, recordings, preferences, or staged changes were replaced.

## MIDI note group resizing — 2026-10-12

- Dragging either edge of a selected note now resizes the selected group, including notes in different patterns. Dragging an unselected note edge resizes only that note. Keep each opposite edge fixed and apply one snapped project-time delta to the group, rounding to source ticks for differently stretched patterns. Clamp the common delta at every pattern boundary and the one-tick minimum note length. Preserve note IDs, pitch, velocity, unselected notes, and outer clip ranges.
- Repeated occurrences resize one source note per clip. Per-clip snapshots drive all previews; release commits changed patterns through one batch. Unselected source users retain their previous pattern. Escape/focus loss restores the previous sources and selection. No-op resizes retain source IDs and modified state. Shift bypasses snapping; Option/Ctrl on note edges still resizes without duplication. This supersedes the single-note edge-resize limits in previous entries.
- All 322 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. All 38 MIDI UI tests pass. New checks cover both edges, preview/release, opposite-edge preservation, unselected notes and source copies, shortest-note clamps, pattern-boundary no-ops, differently stretched patterns, repeated occurrences, group selection retention, Shift bypass, modifier behavior, and cancellation. Logs: `work/midi-group-resize-focused.log` and `work/midi-group-resize-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-group-resize-clippy.log` and `work/midi-group-resize-package.log`.
- Native mouse/trackpad and Windows/Linux native input checks were not run for this change. Automated egui tests do not establish native gesture behavior. No user projects, recordings, preferences, or staged changes were replaced.

## Add audio track label — 2026-10-12

- Renamed the shared audio-track action to Add audio track in native and application menus. The Tracks header Plus button uses the same label for its tooltip and accessible name. Track creation behavior is unchanged. This supersedes earlier Add track label references.
- The existing menu interaction test passes with the updated label and still verifies track creation without a selection. All 322 workspace tests, strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Logs: `work/add-audio-track-focused.log`, `work/add-audio-track-tests.log`, and `work/add-audio-track-clippy.log`.
- Rebuilt `target/debug/DAW.app`; log: `work/add-audio-track-package.log`. Native menu rendering and Windows/Linux UI were not checked.

## Oscillator panel and source-track toolbar — 2026-10-12

- Units titles its source panel Oscillator. Removed the enable checkbox, Polyphonic oscillator text, and Edit selected pattern button. Waveform and Transpose share a field width, with centered transpose text. The gain field fills the remaining row after the shared knob and centers its text, including during text entry. Existing settings, parameter bounds, polyphonic processing, and saved missing-instrument state remain intact.
- Units and Edit share a track dropdown in the playback toolbar. Edit keeps its zoom control at the right. Changing the Edit track selects its earliest clip, or clears clip selection for an empty track; the track strip and piano roll follow that selection. Track selection does not edit project data. The dropdown is disabled during editor gestures and inherits toolbar dialog/job guards.
- All 323 workspace tests pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. The new egui interaction check covers toolbar placement, track changes in both views, clip context, an empty track, and unchanged project/modified state. Logs: `work/oscillator-toolbar-focused.log` and `work/oscillator-panel-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-panel-clippy.log` and `work/oscillator-panel-package.log`. Native layout and Windows/Linux input checks were not run.

## Editor view and audio-track names — 2026-10-12

- Renamed the clip-editing view from Edit to Editor through its shared label. View buttons, tooltips, native View menus, and application View menus use Editor. Current documentation uses the new name; earlier validation entries retain their historical labels. The Edit menu for clipboard actions retains its name.
- New audio tracks are named Audio N instead of Track N, using their position in project order for N. Menu creation, file imports, and moves/drops that create tracks share this default. Existing names, duplicate names, and MIDI naming remain unchanged. Updated existing UI checks for the new default.
- All 229 UI tests and all 323 workspace tests pass. Strict workspace/all-target Clippy, formatting, `git diff --check`, and documentation links pass. Built the CLI and passed `python3 tools/check_cli.py`. Logs: `work/editor-audio-names-focused.log`, `work/editor-audio-names-tests.log`, `work/editor-audio-names-clippy.log`, and `work/editor-audio-names-cli.log`.
- Rebuilt `target/debug/DAW.app`; log: `work/editor-audio-names-package.log`. Native menu rendering and Windows/Linux native UI were not checked.

## Shared unit panel headers — 2026-10-12

- Added `PanelStyle::unit` as the base panel for all source, effect, and modulator units. The Oscillator uses it now. Its header spans the inner panel width with the unit name, active application accent, and centered contrasting text. Unit and mixer headers share 24-point height and 11-point text constants. The body retains the neutral theme surface, thin border, 10-point padding, and existing controls.
- Recorded the common-panel requirement in the Units design, repository instructions, and implementation specification. Future units supply their names and controls to the same panel instead of creating separate shells.
- All 229 UI tests and 323 workspace tests pass. Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Logs: `work/unit-headers-focused.log`, `work/unit-headers-tests.log`, and `work/unit-headers-clippy.log`.
- Rebuilt `target/debug/DAW.app`; log: `work/unit-headers-package.log`. Native layout and Windows/Linux native UI were not checked.

## Unit header text and toolbar divider — 2026-10-12

- Unit headers use the mixer strip foreground rule while keeping the active accent as their background. Deep Orange and Purple now have bright text; Yellow retains dark text. This supersedes the direct black/white foreground choice in the preceding unit-header entry.
- Units shares one 1-point divider with the playback toolbar. The toolbar’s separate bottom divider is omitted for Units; the workspace line and first unit’s top border align, without an extra gap or stacked borders. The shared base-panel requirement remains in the Units design and repository instructions.
- All 324 workspace tests pass. The new egui regression check covers Deep Orange, Purple, and Yellow in both themes, centered 11-point header text, exact toolbar/header spacing, and one thin full-width divider. Existing 229 UI tests also passed before adding that check. Logs: `work/unit-header-divider-focused.log`, `work/unit-header-divider-regression.log`, and `work/unit-header-divider-tests.log`.
- Strict workspace/all-target Clippy, formatting, `git diff --check`, and relevant documentation links pass. Rebuilt `target/debug/DAW.app`; logs: `work/unit-header-divider-clippy.log` and `work/unit-header-divider-package.log`. Native layout and Windows/Linux native UI were not checked.

## Settings field alignment — 2026-10-12

- Settings and Project Settings use a shared label-column row layout. All dropdowns and the project-name field fill the remaining width, with matching left and right edges within each dialog. The application accent swatch stays beside its label.
- The egui regression check covers field-border alignment in both dialogs and both themes. All 18 settings-related tests and 325 workspace tests pass, including dropdown behavior, name selection, and Apply/Cancel transactions. Logs: `work/settings-alignment-focused.log` and `work/settings-alignment-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation links, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/settings-alignment-clippy.log` and `work/settings-alignment-package.log`. Native layout and Windows/Linux native UI were not checked.

## Empty Timeline track-list menu — 2026-10-12

- Secondary-clicking the blank track-list area below track panels and above Master opens Add audio track and Add MIDI track, including when no tracks exist. The menu uses existing guarded actions, appends/selects an empty track on item release, and preserves clip selection. Opening or dismissing it leaves project data and selections unchanged. Timeline lanes, ruler, and Master retain their existing behavior.
- Two egui regression tests cover both track types with and without existing tracks, release-only commits, selection preservation, Escape dismissal, lane/Master exclusion, and blocking during jobs. All 327 workspace tests pass. Logs: `work/empty-tracks-menu-focused.log` and `work/empty-tracks-menu-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation links, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/empty-tracks-menu-clippy.log` and `work/empty-tracks-menu-package.log`. Native pointer behavior and Windows/Linux native UI were not checked.

## Shared project-grid snapping — 2026-10-12

- Fixed MIDI pattern resizing that quantized the drag distance and retained an off-grid offset. Pattern edges now snap their absolute project position using the ruler’s shared grid, with source-tick conversion after snapping. Preview, cancellation, opposite-edge preservation, and release-only commits remain in place.
- Timeline and MIDI editing share bounded snapping and the Shift bypass helper. Note drawing, moves, and group resizes use the project ruler’s six-point radius and bar/beat/visible-subdivision priorities; movement outside that radius stays free. MIDI adapters account for source offsets, repetitions, and stretch. Click-created notes use one visible project subdivision. Bounds and zero-displacement edits retain their existing constraints. This supersedes pattern-relative note grids and drag-distance step quantization.
- All 236 UI tests and 330 workspace tests pass. New regressions cover both off-grid pattern edges, extension/shortening, two zoom levels, scrolling and stretch, plus off-grid note drawing/moving/resizing. Shared snapping tests cover bypass, bounds, radius, and pixel/frame conversion at 44.1/48/96 kHz. Existing Shift, group, source-isolation, duplication, and cancellation tests pass. Logs: `work/shared-grid-snap-ui.log` and `work/shared-grid-snap-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation links, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/shared-grid-snap-clippy.log` and `work/shared-grid-snap-package.log`. Native gestures and Windows/Linux native UI were not checked.

## Oscillator controls and shared knob base — 2026-10-12

- Oscillator controls use the confirmed compact vertical layout: Waveform, Transpose, Gain from top to bottom. Each shared row has a left-aligned label, standard knob, and equal-width centered value on the right. Waveform has four steps (Sine, Saw, Square, Triangle); Transpose has whole-semitone steps. Transpose and Gain retain editable numeric values. This supersedes the earlier dropdown/input rows and the proposed horizontal columns.
- All knob types use the `Knob`/`KnobSpec` base for mapping, painting, focus/disabled guards, fractional drag accumulation, Shift sensitivity, keyboard steps, bounds, double-click reset, tooltips, and accessibility. Existing Timeline/Mixer gain and pan controls use it too. Reset values are Sine, zero semitones, unity gain, and centered pan. Slow stepped drags accumulate across frames; modifier changes retain the current value without a jump.
- All 239 UI tests and 333 workspace tests pass. New checks cover slow stepped drags, Shift changes, discrete keyboard bounds/reset, disabled input, row label/knob/value alignment in both themes, and selected-instrument edits that retain pattern assets. Existing gain-drag, unity/center marks, double-click reset, timeline, and mixer tests pass. Logs: `work/oscillator-knobs-layout.log` and `work/oscillator-knobs-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation links, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-knobs-clippy.log` and `work/oscillator-knobs-package.log`. Native knob gestures/layout and Windows/Linux native UI were not checked.

## Waveform diagrams and continuous selector — 2026-10-12

- The waveform knob shows small vector diagrams for Sine, Saw, Square, and Triangle at four equally spaced positions around its standard face. The active diagram uses the accent color. The row reserves space for the diagrams and retains alignment with Transpose and Gain. This replaces the waveform tick marks and bounded selector from the preceding entry.
- Cyclic wrapping and optional choice diagrams use the shared knob base. Waveform dragging and arrow keys wrap continuously in both directions; Shift retains fine adjustment without a jump, release retains the choice, and double-click resets to Sine. Gain, pan, and transpose retain their bounds.
- All 241 UI tests and 335 workspace tests pass. New tests cover multiple turns in each direction, keyboard wrapping, disabled input, fine adjustment, release, and reset. The Units layout regression checks four diagrams, active highlighting, diagram bounds, and aligned label/knob/value columns in both themes. Logs: `work/waveform-knobs.log`, `work/waveform-layout.log`, and `work/waveform-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/waveform-clippy.log` and `work/waveform-package.log`. Native knob gestures/layout and Windows/Linux native UI were not checked.

## Transpose octave reference marks — 2026-10-12

- Transpose shows nine marks at −48, −36, −24, −12, 0, +12, +24, +36, and +48 semitones. Zero retains the longer, brighter reference mark. The shared knob base accepts a reference interval separate from its input step; transpose still adjusts by one semitone, with the same bounds, reset, and layout.
- The Units regression checks all nine angular positions and zero/minor styling in both themes while retaining its one-semitone input check. All 241 UI tests and 335 workspace tests pass. Logs: `work/transpose-marks-focused.log` and `work/transpose-marks-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/transpose-marks-clippy.log` and `work/transpose-marks-package.log`. Native knob appearance/gestures and Windows/Linux native UI were not checked.

## Knob zero reference labels — 2026-10-12

- Transpose displays a small “0” above its zero mark. Gain knobs support an optional “0 dB” above the unity mark; Oscillator uses it. Both labels use the shared knob base, 9-point theme text, and reserved space above the standard face. All Oscillator rows reserve 52 points and retain aligned label, knob, and value columns. Timeline/Mixer gain knobs retain their unlabeled layout and behavior.
- The Units regression checks that both reference labels are centered above their marks and stay within their reserved bounds in both themes. Existing octave marks, waveform diagrams, one-semitone input, gain edits, and row alignment checks pass. All 241 UI tests and 335 workspace tests pass. Logs: `work/knob-reference-focused.log` and `work/knob-reference-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/knob-reference-clippy.log` and `work/knob-reference-package.log`. Native knob appearance/gestures and Windows/Linux native UI were not checked.

## Transpose endpoint labels — 2026-10-12

- Transpose displays “−48” and “+48” below its lower-left and lower-right endpoint marks. These use the shared knob base's optional endpoint labels and the same 9-point theme text as the existing zero label. The labels fit within the existing 52-point footprint; parameter adjustment and row alignment remain unchanged.
- The Units regression checks both endpoint labels, placement on opposite sides, matching baselines, theme text color, and reserved bounds in both themes. Existing zero labels, octave marks, waveform diagrams, and input checks pass. All 241 UI tests and 335 workspace tests pass. Logs: `work/transpose-endpoints-focused.log` and `work/transpose-endpoints-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/transpose-endpoints-clippy.log` and `work/transpose-endpoints-package.log`. Native knob appearance/gestures and Windows/Linux native UI were not checked.

## Gain fader scale and exact silence — 2026-10-12

- Shared track and Master faders label 0, −3, −6, −10, −15, −20, −30, −40, and −∞, plus +12 dB. Unity has a longer, brighter mark. Smaller subdivisions use the same gain/position mapping as the handle; the scale gives more travel near unity and a continuous amplitude tail below −60 dB. Subdivisions and labels reduce at short heights to avoid collisions. Scale space is reserved inside each strip. This supersedes the previous linear −60 to +12 dB fader scale.
- The confirmed −∞ endpoint produces exact silence without changing mute/solo. `SILENT_GAIN_DB` is a finite project value that the existing shared gain conversion maps to zero, preserving JSON and the manifest schema. Timeline/Mixer gain fields share formatting/parsing and accept −∞, -inf, and -infinity. Master input uses the new fader limits; finite track values outside the slider range survive idle display. Shift adjusts travel ten times more slowly and can change mid-drag without jumping; release retains the gain. Double-click reset reads the pointer event within the rail because egui Slider responses sense drags rather than clicks.
- All 244 UI tests and 339 workspace tests pass. Checks cover monotonic/reversible mapping, exact silence and recovery from the floor, input aliases/invalid values, idle preservation, Shift changes, release, reset, labels/subdivisions fitting track and Master strips in both themes, numeric commit/cancel, and view switching. The workflow regression verifies save/reopen, unchanged mute/solo and source files, live renderer samples, and exact-zero float WAV export for both silent track and silent Master gain. Logs: `work/fader-scale-focused.log`, `work/fader-silence-workflow.log`, and `work/fader-scale-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Built the CLI and passed `python3 tools/check_cli.py`. Rebuilt `target/debug/DAW.app`; logs: `work/fader-scale-clippy.log`, `work/fader-scale-cli-build.log`, `work/fader-scale-cli.log`, and `work/fader-scale-package.log`. Native fader appearance/gestures, device output, and Windows/Linux native UI were not checked.

## Positive fader marks and stereo meter scales — 2026-10-12

- Shared gain faders now label +3, +6, +9, and +12 dB with 1 dB subdivisions above zero. Positive interior labels reduce at shorter heights to avoid collisions. The existing gain mapping, exact silence, fine adjustment, and reset remain in use.
- Track and Master stereo meters share a −60 to 0 dBFS scale in Timeline and Mixer, marking 0, −3, −6, −9, −12, −15, −18, −21, −24, −30, −35, −40, −45, −50, and −60. Subdivisions use 1 dB above −30 and 5 dB below it when spacing permits. Labels reduce at short heights; both endpoints fit inside the panel. Fill and ticks share the dBFS mapping, replacing the previous amplitude-proportional fill. Silence and levels at or below −60 show no fill. Actual peak tooltips, per-channel clipping, and click-to-clear remain available. This changes presentation only.
- Reserved a 24-point meter scale column in both views. Mixer strips expand from 108 to 132 points; Timeline header buttons move left to retain the fixed track column. Existing track selection, grip alignment, and control interactions pass.
- All 246 UI tests and 341 workspace tests pass. New meter checks cover dBFS positions, silence/floor/headroom, both themes, four meter heights, label bounds and spacing, subdivisions, independent clipping controls, and disabled input. The Mixer regression checks every positive fader label and subdivision plus meter labels fitting track and Master strips. Logs: `work/meter-scale-unit.log`, `work/meter-scale-focused.log`, and `work/meter-scale-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/meter-scale-clippy.log` and `work/meter-scale-package.log`. Native appearance/gestures, device output, and Windows/Linux native UI were not checked.

## Mixer gain and peak fields — 2026-10-12

- Track and Master strips center the gain field above the fader rail and a matching read-only peak field above the stereo bars. Both fields share one row; their scale columns do not affect centering. Fader and meter heights remain aligned, with the existing bottom controls and strip widths retained.
- Both fields use the shared centered numeric field style. The peak field shows the higher current L/R peak in dBFS with one decimal place, −∞ for silence, and a plus sign above zero. It follows current readings without holding a historical maximum. Text entry is disabled; monitoring does not change gain, project data, or clipping state. Gain entry retains its commit, cancel, validation, and silence behavior.
- All 248 UI tests and 343 workspace tests pass. New checks cover field and rail alignment, matching field widths and baselines, strip bounds, both themes, three window sizes, read-only interaction, left/right peak changes, silence, low levels, and positive headroom. Existing numeric gain, fader, meter, and clipping checks pass. Logs: `work/mixer-readouts-layout.log`, `work/mixer-readouts-peak.log`, and `work/mixer-readouts-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/mixer-readouts-clippy.log` and `work/mixer-readouts-package.log`. Native appearance/gestures, device output, and Windows/Linux native UI were not checked.

## Held peak readouts and click reset — 2026-10-12

- Track and Master peak fields now hold the highest L/R level until clicked, superseding the current-level behavior in the previous entry. The output monitor captures the maximum with an atomic update on each published block, retaining brief peaks between UI frames. Live meter bars and clipping indicators keep their existing behavior.
- Clicking a peak field resets its hold to −∞ on release; later output can raise it again. Reset does not change gain, project data, live levels, or clipping state. The fields retain their numeric style and do not accept text input. Disabled UI and focus loss block reset. Holds survive pause, Stop, view changes, and live plan updates; track holds follow track IDs through reordering. New output monitors and project replacement start without held peaks. Holds are not saved in project data.
- All 248 UI tests and 344 workspace tests pass. Output checks cover both channels, falling levels and silence retaining the maximum, independent track/Master reset, recovery after reset, unchanged live/clipping readings, and stable track identity. UI checks cover release-only reset, silence and headroom formatting, text input rejection, disabled controls, and focus loss. Existing field alignment, gain controls, meter scales, and renderer checks pass. Logs: `work/peak-hold-output.log`, `work/peak-hold-ui.log`, and `work/peak-hold-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/peak-hold-clippy.log` and `work/peak-hold-package.log`. Native click behavior, real-device peak capture, and Windows/Linux native UI were not checked.

## Timeline meters without scales — 2026-10-12

- Timeline track and Master meters now show the stereo bars without dB labels, tick marks, or reserved scale width. Header controls regain the space used by the scale column. Mixer retains its labeled scales and held peak fields. Both views continue to share meter fill, peak tooltips, and independent clipping controls.
- All 248 UI tests and 344 workspace tests pass. The existing scale regression checks labeled Mixer meters and unlabeled Timeline track/Master bars in both themes, including the absence of ticks and the compact bar width. Track selection, header button interaction, gain entry, peak hold, and clipping checks pass. Logs: `work/timeline-meter-scale-focused.log` and `work/timeline-meter-scale-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/timeline-meter-scale-clippy.log` and `work/timeline-meter-scale-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Mixer strips at 120 points — 2026-10-12

- Track and Master Mixer strips are now 120 points wide, replacing the previous 132-point width. Side padding is 4 points and fader/meter column spacing is 1 point. Vertical padding, control sizes, scales, field alignment, and held peak behavior remain unchanged.
- All 248 UI tests and 344 workspace tests pass. The existing field regression checks the 120-point width and verifies that controls remain inside track and Master strips in both themes at three window sizes. Scale bounds, scrolling, pinned Master, selection outlines, numeric entry, and fader interactions pass. Logs: `work/mixer-width-focused.log` and `work/mixer-width-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/mixer-width-clippy.log` and `work/mixer-width-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Mixer fields centered over marked columns — 2026-10-12

- Gain and held peak fields now center over their complete fader and stereo meter columns, including scale labels and ticks. This supersedes the previous rail-only centering. Column widths come from the shared control and scale widths. Both fields retain their 36-point width and common baseline; track and Master strips remain 120 points wide.
- All 248 UI tests and 344 workspace tests pass. The existing field regression checks centering over complete marked columns in tracks and Master, both themes, and three window sizes. Field and scale bounds, numeric gain, held peak reset, and fader interactions pass. Logs: `work/mixer-marked-alignment-focused.log` and `work/mixer-marked-alignment-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/mixer-marked-alignment-clippy.log` and `work/mixer-marked-alignment-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Readable infinity in numeric controls — 2026-10-12

- Numeric fields, held peak readouts, and gain fader labels now use a shared enlarged Ubuntu infinity fallback. Outfit digits, minus signs, line heights, field sizes, gain entry, and peak reset retain their existing behavior. Other UI fonts and menu shortcuts are unchanged.
- All 249 UI tests and 345 workspace tests pass. Glyph checks cover 8- and 11-point text at 1×, 1.5×, and 2× display scales: infinity is larger, centered within one point of the minus sign, and retains the original line height. Numbers retain identical glyphs and layout. Existing field bounds, gain editing, peak reset, and fader interaction checks pass. Fader test contexts now install the same shared fonts as the application. Logs: `work/infinity-font-focused.log`, `work/infinity-faders.log`, and `work/infinity-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/infinity-clippy.log` and `work/infinity-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Gain knob scales and Oscillator controls — 2026-10-12

- Gain knobs now span exact silence (−∞) to +12 dB, replacing their −60 dB minimum. They reuse the fader scale and continuous amplitude tail, adapted to keep 0 dB at twelve o’clock. Arrow keys retain 0.1 dB steps above −60 dB and step through the amplitude tail below it, allowing entry into and recovery from silence. Idle display, Shift adjustment, bounds, focus guards, and double-click reset remain in the shared knob base.
- The base supports optional explicit reference values. Oscillator enables the same 13 dB reference marks as the gain fader, a stronger unity tick, the existing “0 dB” label, and −∞/+12 endpoint labels. Compact gain knobs omit these optional hints. Oscillator rows now show labels and aligned knobs without the separate waveform name or transpose/gain value boxes, superseding the earlier value-column design. Tooltips retain current parameter values.
- Oscillator settings accept finite source gain down to exact silence. `SILENT_GAIN_DB` now belongs to portable `daw_unit_core` and remains available through `daw_core`. The saved state version and manifest schema remain unchanged. A workflow test confirms exact-zero live rendering and float WAV export after save/reopen, audible recovery at 0 dB, and unchanged pattern sources. Unit validation rejects non-finite values and gains outside the supported range.
- All 251 UI tests and 349 workspace tests pass. Checks cover optional markers, stronger unity presentation, knob scale round-trips, dragging and keys entering/leaving silence, unchanged adjustment with markers enabled, both themes, endpoint-label bounds, absence of the value column, and editing the selected instrument. Logs: `work/gain-knob-base.log`, `work/gain-knob-unit.log`, `work/gain-knob-silence.log`, and `work/gain-knob-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, `git diff --check`, and CLI smoke checks pass. Rebuilt `target/debug/DAW.app`; logs: `work/gain-knob-clippy.log`, `work/gain-knob-cli.log`, and `work/gain-knob-package.log`. Native appearance/gestures, real-device output, and Windows/Linux native UI were not checked.

## Oscillator without parameter-name labels — 2026-10-12

- Removed the Waveform, Transpose, and Gain labels and their reserved column from Oscillator. The three knobs retain their vertical order, aligned centers, waveform diagrams, reference ticks, and reference/endpoint labels. Parameter names and current values remain available in the shared knob tooltips and accessibility information. This supersedes the label-and-knob layout recorded above.
- All 251 UI tests and 349 workspace tests pass. The existing Oscillator regression checks the absence of parameter-name labels and value fields, retained alignment and marker bounds, both themes, and keyboard edits to the selected instrument without changing pattern sources. Logs: `work/oscillator-labels-focused.log` and `work/oscillator-labels-tests.log`.
- Strict workspace/all-target Clippy, formatting, documentation link targets, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-labels-clippy.log` and `work/oscillator-labels-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.

## Source-container architecture decision — 2026-10-12

- Recorded D30 and B26: one source container per track, reusable source units inside MIDI instruments, and independent lanes inside one beat instrument. Each container exposes one output bus before track effects and gain/pan. Immutable clip content remains separate from processing state.
- Updated the MIDI notes, implementation specification, and repository guidance to link this decision. Corrected obsolete voice guidance to retain current MIDI-track polyphony and the Beat MVP's one-voice limit per lane. Composite containers and multiple oscillator units remain planned work.
- Reviewed the documentation diff, checked 56 local links and heading anchors, and ran `git diff --check`; all pass. This change affects documentation only. No runtime tests, build, or native-device checks were run.

## Multiple oscillators inside one module — 2026-10-12

- MIDI tracks now own one oscillator instrument with a list of independent source settings. Older single-oscillator state loads as a one-item list. Validate the instrument version and every source before committing changes; empty instruments are valid and silent. Pattern sources remain unchanged by instrument edits.
- Oscillator uses one shared unit panel with horizontal source columns, each retaining the existing vertical Waveform/Transpose/Gain knob layout. The icon-only Lucide Circle Plus button to the right adds a default source on release. Right-click a column or knob to remove that source through its context menu. Removing the last source retains the add button; horizontal scrolling handles overflow.
- Each note has independent oscillator state for every source. Sum source outputs with individual transpose and gain, without normalization, before the shared note envelope/velocity and track processing. Playback, seeking, loops, release tails, and export use the same prepared runtime. Preparation allocates all source state before callback processing.
- All 352 workspace tests, including 252 UI tests, pass. Checks cover legacy migration, malformed collections, atomic validation rollback, independent controls, add/remove gestures on gaps and knob faces, both themes, empty-module recovery, immutable pattern sources, polyphonic source summing and seeking, live/offline agreement, block-size independence, callback allocation, save/reopen/Save As, rate conversion, and WAV/MP3 export. Logs: `work/multiple-oscillators-tests.log`, `work/multiple-oscillators-ui.log`, and `work/multiple-oscillators-workflow.log`.
- Strict workspace/all-target Clippy, formatting, 73 local documentation links/anchors, `git diff --check`, and CLI smoke checks pass. Rebuilt `target/debug/DAW.app`; logs: `work/multiple-oscillators-clippy.log`, `work/multiple-oscillators-cli.log`, and `work/multiple-oscillators-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked. Full instrument graph routing and Beat containers remain pending.

## Oscillator mute buttons, separators, and value labels — 2026-10-12

- Each oscillator has a centered standard 22-point M toggle below its knobs, with selected-state feedback, a tooltip, and an accessibility name. Source mute saves independently of gain and track mute; missing mute fields default to false in both legacy and collection-based instrument state. Muted sources advance with zero output. Unmuting restores their configured gain.
- Adjacent source columns have one 1-point white separator at low opacity, spanning the knob and mute rows. No separator is added next to the add button. This preserves existing column spacing and keeps the separator below panel-border contrast.
- The shared knob base supports an optional current-value formatter. Transpose and marked gain show their current values above the zero/unity position, replacing the earlier fixed “0”/“0 dB” labels. Fixed ticks and endpoint labels remain. Gain uses shared dB formatting and numeric typography, including −∞ for silence; transpose shows signed positive values. Tooltips retain parameter names and units.
- All 354 workspace tests, including 253 UI tests, pass. Checks cover independent mute/unmute, silence with all sources muted, retained gain, live/offline consistency, legacy defaults, save/reopen/export with a muted source, callback allocations, centered bottom buttons, separator placement, and current labels after edits (including silence), in both themes. The first layout check found an off-center button; the corrected layout passes. Log: `work/oscillator-mute-tests.log`.
- Strict workspace/all-target Clippy, formatting, 73 local documentation links/anchors, `git diff --check`, and CLI smoke checks pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-mute-clippy.log`, `work/oscillator-mute-cli.log`, and `work/oscillator-mute-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Oscillator separator contrast correction — 2026-10-12

- The user's light-theme screenshot showed that the translucent white separator was nearly invisible. Replaced that color with shared `theme::unit_separator`, an opaque blend halfway between the current panel and border colors. The 1-point line, column spacing, and span remain unchanged. This supersedes the white separator color recorded above.
- All 354 workspace tests pass. The existing two-theme oscillator test checks separator painting and verifies a visible contrast of at least 8 channel levels, below panel-border contrast. Strict workspace/all-target Clippy, formatting, changed documentation links/anchors, and `git diff --check` pass. Logs: `work/oscillator-separator-tests.log` and `work/oscillator-separator-clippy.log`.
- Rebuilt `target/debug/DAW.app`; log: `work/oscillator-separator-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Full-height oscillator strips with equal padding — 2026-10-12

- Oscillator applies the shared 10-point panel margin separately on all four sides of each source strip and around the add button. Removed the extra horizontal gap and outer body padding. Adjacent strips share a boundary; the subdued 1-point separator spans the full body from below the common unit header to the bottom border, including top and bottom strip padding. This supersedes the earlier knob-to-mute separator span.
- Right-clicking strip padding opens the same Remove oscillator menu as its controls. Knob alignment, vertical row spacing, mute centering, and the shared unit shell remain in use.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks cover equal padding on every strip side, shared boundaries, full-height separator endpoints, retained contrast, and removal through strip padding. Strict workspace/all-target Clippy, formatting, changed documentation links/anchors, and `git diff --check` pass. Logs: `work/oscillator-strip-tests.log` and `work/oscillator-strip-clippy.log`.
- Rebuilt `target/debug/DAW.app`; log: `work/oscillator-strip-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Oscillator containers, Power controls, and add strip — 2026-10-12

- Each oscillator and Add has its own strip container, equal height, and shared 10-point internal padding. Center the source knobs over the bottom control row. A subdued 1-point separator spans the full body between every adjacent strip, including the last oscillator and Add. This supersedes the earlier omission of a separator before Add.
- Replaced each square M toggle with a circular 22-point Lucide Power button, followed by an “Oscillator N” label. Shared theme colors match the view controls: accent fill with contrasting foreground when on, dim foreground when off. Retain independent saved source mute, gain, and track state. The add strip contains a circular Lucide Plus button centered vertically. Both button types share icon painting, interaction, tooltips, and accessibility names/state. This supersedes the earlier M and Circle Plus controls.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks cover equal padding and heights, knob alignment, named Power controls, independent on/off changes committed on release, on/off colors, centered circular Add, full-height separators including before Add, strip context menus, and empty-instrument recovery. Logs: `work/oscillator-power-ui.log` and `work/oscillator-power-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local links/anchors in changed design documents, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-power-clippy.log` and `work/oscillator-power-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Compact oscillator controls and full-strip Add button — 2026-10-12

- Reduced each oscillator Power button from 22 to 18 points and its label to 11 points. The shared circular control scales its icon with its size. Source strips still center the knobs above the bottom row and retain equal padding and height. This supersedes the previous Power button size.
- The entire add strip is now a borderless button with a centered Lucide Circle Plus icon. Hover or keyboard focus shows the standard button background without a border; moving away clears it. The strip retains equal internal padding and its full-height separator. Clicking padding, away from the icon, adds one default source on release. This supersedes the separate circular Plus button recorded above.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks cover compact Power geometry, knob alignment, full-strip hit bounds, padding clicks committed on release, hover and idle backgrounds, separators, and independent source state. Logs: `work/oscillator-compact-ui.log` and `work/oscillator-compact-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-compact-clippy.log` and `work/oscillator-compact-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Fixed oscillator strip width and centered compact Add icon — 2026-10-12

- Replaced width derived from the highest oscillator label with a fixed 120-point source-strip width, including shared padding. Adding or removing sources no longer changes strip widths. Long labels truncate with a full-name tooltip instead of growing the strip.
- Reduced the add strip's Circle Plus icon from 22 to 18 points and retained 10-point horizontal padding. The strip remains fully clickable, borderless, and highlighted on hover/focus. An explicitly centered child layout positions the image on both axes; the previous layout inherited top alignment from its parent. Earlier checks covered the clickable strip, but did not verify the painted icon position.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks now verify fixed width through add/remove and inspect the actual textured icon rectangle for its 18-point size and center position. Padding, separators, hover behavior, release commits, knob alignment, and independent source state continue to pass. Logs: `work/oscillator-fixed-width-ui.log` and `work/oscillator-fixed-width-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-fixed-width-clippy.log` and `work/oscillator-fixed-width-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Shared circular Add and Power controls — 2026-10-12

- Add now uses the same shared 18-point circular control as Power, with a Lucide Plus icon inside the circle. It is centered on both axes within the padded add container. Only the nested button adds a source; padding clicks do not. This supersedes the full-strip button and Circle Plus image recorded above. Removed the unused full-strip button helper.
- Off controls use the current theme text color for both icon and border. Off labels remain dim. On retains the view controls’ accent fill and contrasting foreground. Add uses the same inactive appearance, border, icon scale, hover feedback, tooltips, and accessibility implementation as Power. Hover/focus feedback fills the circle rather than the whole strip.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks verify matching button and painted-icon sizes, centered Add, text-colored off borders, circular hover feedback, padding-click no-ops, release commits, fixed strip width, full-height separators, and independent source state. Logs: `work/oscillator-nested-add-ui.log` and `work/oscillator-nested-add-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-nested-add-clippy.log` and `work/oscillator-nested-add-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Accent-colored Power icon and thin on-state border — 2026-10-12

- On controls now have a 1-point border and icon in the current application accent color, retaining the accent-tinted fill and contrasting label. This supersedes the earlier borderless on state and contrasting icon color. Off continues to use the theme text color for both border and icon, with a dim label. The shared circular control supplies both states.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks verify the on-state border width/color and retained fill, alongside off borders, independent source state, shared Add geometry, release commits, padding behavior, and fixed strip layout. Logs: `work/oscillator-on-border-ui.log` and `work/oscillator-on-border-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-on-border-clippy.log` and `work/oscillator-on-border-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Equal circular controls with full-strip Add interaction — 2026-10-12

- Add and Power now share one circle/icon painter. Inset the 1-point stroke so the painted outer diameter, including the border, is exactly 18 points for both controls. The centered Plus and Power icons retain the same scale. Earlier size checks compared control rectangles and icon textures; the updated check also compares painted circle radii and stroke widths.
- The whole add strip is one interaction target. Hover/focus highlights its background, and clicking anywhere, including padding away from the circle, adds exactly one source on release. The small circle remains centered within the strip and uses the shared inactive appearance. This supersedes the earlier padding-click no-op requirement. There is one response for the whole strip, so the circle cannot cause a second Add action.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks verify equal painted circle sizes, border-inclusive diameter, centered icons, shared icon scale, full-strip hover/idle feedback, padding clicks committed on release, one-source additions, fixed strip widths, separators, and independent source state. Logs: `work/oscillator-add-strip-ui.log` and `work/oscillator-add-strip-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-add-strip-clippy.log` and `work/oscillator-add-strip-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## Unlabeled centered Power buttons and 100-point oscillator strips — 2026-10-12

- Removed the visible “Oscillator N” labels and centered each Power button below its knobs. Retain the source number in its tooltip and accessible name. Changed oscillator strips from 120 to 100 points wide, including the existing shared padding. This supersedes the earlier strip-label and width requirements.
- Off controls now use the secondary text color for both icon and border, matching the former dim label. On retains its accent icon, 1-point accent border, and tinted fill. Add continues to use the same circular renderer, with whole-strip hover and click handling. This supersedes the previous off-state primary text color.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks verify horizontal Power centering, absent strip labels, secondary-colored off borders, fixed strip widths through add/remove, equal circle sizes, centered Add, whole-strip hover/padding clicks, release commits, separators, and independent source state. Logs: `work/oscillator-centered-power-ui.log` and `work/oscillator-centered-power-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-centered-power-clippy.log` and `work/oscillator-centered-power-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.

## 90-point oscillator strips — 2026-10-12

- Reduced the fixed oscillator strip width from 100 to 90 points, including the existing padding. This supersedes the previous width.
- All 354 workspace tests, including 253 UI tests, pass. Existing two-theme checks cover fixed width, centered knobs/Power, shared Add geometry, whole-strip hover/clicks, padding, separators, and independent source state. Logs: `work/oscillator-90-width-ui.log` and `work/oscillator-90-width-tests.log`.
- Strict workspace/all-target Clippy, formatting, 45 local design-document links/anchors, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`; logs: `work/oscillator-90-width-clippy.log` and `work/oscillator-90-width-package.log`. Native appearance, real-device output, and Windows/Linux native UI were not checked.


## Pitch naming and continuous oscillator controls — 2026-10-12

- Renamed the user-facing Transpose control to Pitch, including tooltip/accessibility names and the shared knob wrapper. Retained one-semitone steps, −48/+48 limits, reference marks, and the persisted `transpose` field for existing projects.
- Gain and pitch edits previously replaced prepared voices, restarting oscillator phase and note envelopes during knob changes. Compatible gain, pitch, and mute edits now transfer the active voices and retarget 5 ms amplitude/frequency ramps from their current values. Mixer/source control updates retain any existing output transition instead of starting another transition from a held sample. Waveform, source-count, and pattern changes retain prepared replacement behavior. This supersedes the earlier instrument-change replacement behavior for gain, pitch, and mute.
- All 356 workspace tests, including 253 UI tests, pass. Two new audio regressions cover all six processing rates and four waveforms: rapid gain edits match continuously running reference sources without affecting another oscillator; pitch edits match a continuously running oscillator during unfinished ramps and after reaching the −48/+48/zero targets. Gain coverage includes exact silence and mute/unmute. Extended the callback allocation check to include source gain, pitch, and mute updates. Log: `work/pitch-gain-tests.log`.
- Strict workspace/all-target Clippy, formatting, 62 local documentation links/anchors, and `git diff --check` pass. CLI build and `python3 tools/check_cli.py` pass for WAV/MP3 codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite behavior, and exit codes. Rebuilt `target/debug/DAW.app`. Logs: `work/pitch-gain-clippy.log`, `work/pitch-gain-cli-build.log`, `work/pitch-gain-cli-smoke.log`, and `work/pitch-gain-package.log`.
- Native appearance, real-device audio, and Windows/Linux native UI were not checked. The regressions verify rendered samples and allocation behavior; they do not establish a listening result on an audio device.


## ADSR Envelope unit — 2026-10-12

- Added Envelope immediately to the right of Oscillator. Both use the shared unit shell, equal total height, aligned accent headers, three control rows, and matching footer alignment. Four 90-point stage strips have equal internal padding, full-height separators, and bottom Attack/Decay/Sustain/Release labels. Panels share one full-height 1-point muted divider and one horizontal scroll area. Sustain reserves its two unused rows; timed stages use time, stepped curve-diagram, and curve-amount controls from the shared knob base. Linear disables amount without discarding it.
- Added versioned portable envelope settings and the reusable `daw-envelope` crate. Each note has an independent 0–1 ADSR signal applied to its combined oscillator output. New defaults are 5 ms Attack, 100 ms Decay, 80% Sustain, and 200 ms Release, with Linear Attack and Exponential Decay/Release. Missing envelopes in legacy flat or collection instruments preserve the previous 5 ms gate. Core rejects malformed versions, times, levels, and amounts atomically. Saving persists curve choices and retained amounts; save/reopen, Save As, processing-rate changes, and WAV/MP3 exports share the configuration.
- Live envelope changes preserve oscillator phase, current stage/progress, and current envelope level. Sustain changes ramp over 5 ms. Release starts at the current level after early note-off. Zero-duration stages advance immediately. Voice slots reserve the supported release range before publication, with clip-stretch conversion, so release edits retain schedules without callback allocation. Seek reconstructs envelope progress in project time, including stretched and trimmed clips. Existing clip/repeat boundary fades and arranged export duration still bound source releases; effect/graph tail work remains pending.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). New checks cover six-rate stage timing, exact endpoints, curve monotonicity and zero amount, early note-off, zero stages, live edits, fast reconstruction, legacy migration, invalid-edit rollback, two-theme panel/row/footer geometry and separators, disabled amount retention, independent release tails, all-rate/buffer playback/export agreement, and phase continuity. Extended callback allocation and persisted mixed-audio/oscillator export workflows. Log: `work/envelope-tests.log`.
- Strict workspace/all-target Clippy, formatting, 64 local documentation links/anchors, and `git diff --check` pass. CLI build and `python3 tools/check_cli.py` pass for codecs/bitrates, deterministic exports, missing sources, eight-track projects, overwrite handling, and exit codes. Rebuilt `target/debug/DAW.app`. Logs: `work/envelope-clippy.log`, `work/envelope-cli-build.log`, `work/envelope-cli-smoke.log`, and `work/envelope-package.log`.
- Native appearance, real-device listening, Windows/Linux native UI, and large-workload CPU/memory performance were not checked. Automated geometry and rendered-sample tests do not establish native/device behavior.


## Centered Envelope value labels — 2026-10-12

- Fixed Attack and Sustain current-value labels, which followed their reset marks around the arc. The shared knob base now centers value labels directly above the knob, independent of reset position. Tick marks and bottom stage labels retain their positions.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Extended the existing two-theme Envelope geometry check to verify centered readouts, top placement, and fit within the control row at default and changed Attack/Sustain values, including second-format times. Logs: `work/envelope-readout-focused.log` and `work/envelope-readout-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/envelope-readout-clippy.log` and `work/envelope-readout-package.log`. Native appearance and Windows/Linux native UI were not checked.


## Envelope knob endpoint labels — 2026-10-12

- Time knobs now show 0 ms and 10 s below the range endpoints. Sustain level and curve amount (strength) show 0% and 100%. All use the shared knob endpoint-label implementation, matching Pitch and Gain. Current-value labels remain centered above the knobs; Linear retains visible endpoint labels on its disabled amount control.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Extended the two-theme Envelope geometry check to verify every numeric knob's endpoint labels sit below the face, remain inside its row/strip, and do not overlap. Logs: `work/envelope-endpoint-focused.log` and `work/envelope-endpoint-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/envelope-endpoint-clippy.log` and `work/envelope-endpoint-package.log`. Native appearance and Windows/Linux native UI were not checked.


## More pronounced Envelope curve symbols — 2026-10-12

- Curve-choice diagrams now use full-strength previews in a 10-by-10-point area instead of 70% previews in a 10-by-6-point area. This makes the bends more visible while preserving the shared knob layout. Diagram size is supplied through the shared knob specification; waveform diagrams retain their existing size. This changes the symbols only; saved curve amounts and envelope processing retain their behavior.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Extended the existing two-theme Envelope geometry check to verify four diagrams remain inside each shape-knob row, Linear stays straight, and all nonlinear symbols have visible curvature in rising and falling stages. Logs: `work/envelope-symbol-focused.log` and `work/envelope-symbol-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/envelope-symbol-clippy.log` and `work/envelope-symbol-package.log`. Native appearance and Windows/Linux native UI were not checked.


## Centered Oscillator Power-label pairs — 2026-10-12

- Added an 11-point “Osc N” label to the right of each 18-point Power button with a 4-point gap. Center the pair in the fixed 90-point strip, with primary text when on and secondary text when muted. Retain the numbered tooltip/accessibility name and existing independent toggle behavior. This supersedes the earlier unlabeled, individually centered Power button requirement.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated the two-theme Oscillator check to verify pair centering, label position/size, active and muted text colors, retained strip padding, equal Add/Power painted dimensions, and release-based source toggling. Logs: `work/oscillator-label-focused.log` and `work/oscillator-label-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/oscillator-label-clippy.log` and `work/oscillator-label-package.log`. Native appearance and Windows/Linux native UI were not checked.


## Panel-border color for the Oscillator–Envelope divider — 2026-10-12

- The full-height 1-point divider between Oscillator and Envelope now uses the shared panel frame's border stroke, matching the bottom and other outer panel borders. This supersedes the earlier muted inter-panel divider color; internal oscillator/stage separators retain their lower contrast.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Extended the existing two-theme Envelope geometry check to verify the full-height divider color and both actual painted panel-border strokes match. Existing checks retain equal panel height, shared boundary, and muted internal separators. Logs: `work/unit-border-divider-focused.log` and `work/unit-border-divider-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/unit-border-divider-clippy.log` and `work/unit-border-divider-package.log`. Native appearance and Windows/Linux native UI were not checked.


## Increased Oscillator Power-label spacing — 2026-10-12

- Increased the gap between each Power button and its “Osc N” label from 4 to 8 points, using the shared toolbar spacing. The pair remains centered in the fixed 90-point strip. This supersedes the previous 4-point gap.
- All 371 workspace tests, including 255 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated the existing two-theme Oscillator layout check for the 8-point gap; pair centering, vertical alignment, internal padding, label colors, shared Add/Power geometry, and independent source toggling pass. Logs: `work/oscillator-label-gap-focused.log` and `work/oscillator-label-gap-tests.log`.
- Strict workspace/all-target Clippy, formatting, and `git diff --check` pass. Rebuilt `target/debug/DAW.app`. Logs: `work/oscillator-label-gap-clippy.log` and `work/oscillator-label-gap-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Shared Timeline and Editor MIDI clip actions — 2026-10-12

- Both clip strips now call the same gesture/context-menu handler. MIDI creation, selection, horizontal move, modifier drag duplication, copy/paste, body-edge source trim/extension, header-edge looping, modifier stretching, split/delete, and guarded track actions share project state and dispatch. Editor empty-strip double-click creates a one-beat pattern without switching views. Editor retains its own zoom/scroll and horizontal moves retain track assignments; Timeline also supports vertical placement.
- MIDI body edges crop/pad immutable source assets in both views, ignoring the current pattern length as an expansion limit. Left extension pads before retained notes without moving their project times, within sample/tick/stretch rounding. Source resize bounds and group overlap resolution are shared in core. Each selected source is prepared from the original project before resolving neighbors once. Audio source limits and shared-boundary trimming remain intact. File actions are disabled during shared clip gestures; cancellation leaves committed sources/ranges unchanged.
- This supersedes earlier Timeline-only MIDI creation/paste/track-duplication requirements and Editor pattern-header source resizing. Header edges now loop in both clip strips; body edges resize MIDI sources. The [parity contract](../instruments/midi-patterns.md#timeline-and-editor-parity-contract), specification, README, and AGENTS.md identify the common code, supported gestures, cancellation rules, and required regression checks.
- All 377 workspace tests, including 258 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Paired view input tests cover move/copy, body trims, loops, stretching, preview staging, Escape cancellation, cross-view visibility, creation, clipboard use, track duplication, and context-menu split/delete. Existing Editor tests retain off-grid/Shift snapping and focus-loss cancellation coverage. New core tests cover source isolation, no-op identity, selected-neighbor direction priority, and fixed note timing at all six rates, fractional tempos, and stretch. Log: `work/midi-parity-workspace.log`.
- Strict workspace/all-target Clippy (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), `cargo fmt --all --check`, and `git diff --check` pass. Built `daw-cli` and ran `python3 tools/check_cli.py`; WAV/MP3 smoke checks pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/midi-parity-clippy.log`, `work/midi-parity-cli-build.log`, `work/midi-parity-cli.log`, and `work/midi-parity-package.log`.
- Native mouse/trackpad appearance and gestures, live audio devices, and Windows/Linux native UI were not checked. Automated input/render tests do not establish those results.

## Equal Envelope and Oscillator shape-icon sizes — 2026-10-12

- All choice diagrams now use the shared knob renderer's 10-by-6-point size and 1.25-point stroke. Removed the Envelope's separate 10-by-10-point override and per-control size setting. Curve diagrams retain full curve strength. This supersedes the earlier taller Envelope symbols while preserving knob positions and interaction.
- All 377 workspace tests, including 258 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated the existing two-theme Envelope geometry check to compare painted curve dimensions with oscillator waveform dimensions, verify every curve stays inside its control row, and retain straight Linear and visibly bent nonlinear symbols. Logs: `work/shape-icon-size-focused.log` and `work/shape-icon-size-tests.log`.
- Strict workspace/all-target Clippy, `cargo fmt --all --check`, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/shape-icon-size-clippy.log` and `work/shape-icon-size-package.log`. Native appearance and Windows/Linux native UI were not checked.

## Standard knob-icon size — 2026-10-12

- Confirmed 10-by-6 logical points as the standard for all current and future knob choice icons. AGENTS.md and the MIDI control guide require the shared size/renderer and prohibit per-unit dimensions. The existing renderer already enforces this size with a 1.25-point stroke.
- Documentation-only update: verified the rule against `knobs.rs`, checked documentation file targets, and ran `git diff --check`. The preceding automated checks remain applicable; no runtime code changed in this update.

## Parallel instrument layers — 2026-10-12

- New MIDI tracks start with an accent Instrument panel, one empty `Layer 1 | Add source | Add envelope` row, and Add layer below it. Add source creates an oscillator; each layer reuses the existing Oscillator controls and has one optional amplitude ADSR. Layer labels and rows have thin panel-border dividers. Units scroll horizontally per layer and layers scroll vertically. Layer/envelope context-menu removal commits on release; the last layer cannot be removed. Removing an envelope retains its settings for adding it again. No LFO or Effects section is included in this scope.
- Instrument version 2 persists ordered layers with stable, unique, non-nil IDs, oscillator lists, envelope presence, and retained envelope settings. Old flat/list instruments migrate into Layer 1 with their sound/settings intact, including the legacy gate for missing envelopes. Empty sources render silence. Sources without an installed envelope use the linear 5 ms note gate. Notes reach every layer; each note owns independent oscillator/envelope state per layer. Sum layer outputs into the track's single instrument bus before velocity and track processing. This supersedes the earlier automatic oscillator/ADSR default for new MIDI tracks.
- All 381 workspace tests, including 260 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). New input checks cover both themes, the empty layout, additions on release, stable layer IDs, independent pitch controls, removal with retained envelope settings, and the final-layer guard. Core checks cover empty defaults, legacy migration, round trips, duplicate/nil IDs, invalid versions/settings, and atomic rejection. Render checks cover independently shaped layers, silent empty instruments, different block sizes, seek reconstruction for layered/stretched/trimmed notes, and callback allocation during live layered controls. Mixed audio/MIDI save/reopen, Save As, processing-rate changes, and WAV/MP3 export retain layered instruments. Existing Timeline/Editor parity checks pass. Logs: `work/instrument-ui-tests.log` and `work/instrument-layers-final-tests.log`.
- Strict workspace/all-target Clippy (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), `cargo fmt --all --check`, and `git diff --check` pass. Built `daw-cli` and ran `python3 tools/check_cli.py`; codec, repeatability, missing-source, eight-track, overwrite, and exit-code smoke checks pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/instrument-layers-final-clippy.log`, `work/instrument-layers-cli-build.log`, `work/instrument-layers-cli.log`, and `work/instrument-layers-package.log`.
- Native appearance/gestures, live audio devices, and Windows/Linux native UI were not checked. Automated input/render tests do not establish those results.

## Instrument section heading and shared dividers — 2026-10-12

- Instrument now uses a left-aligned section heading in primary text, without a background fill. Oscillator and Envelope retain their shared accent unit headers. This supersedes the filled, centered Instrument header recorded above.
- Add source appears only when that layer's source list is empty. Populated Oscillator panels retain their Plus strip; removing the final source restores Add source. Add source, Add envelope, and Add layer use flat click areas with hover/focus feedback and accessible names. Shared 1-point panel-border dividers replace individual rounded button outlines. Adjacent rows and unit panels share border coordinates instead of stacking strokes.
- All 381 workspace tests, including 260 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated two-theme input/painting checks cover conditional Add source visibility, empty-source restoration through existing removal tests, left heading alignment and primary text, retained accent unit headers, borderless add areas, 1-point dividers, shared row boundaries, additions, independent layer controls, and context-menu removals. Logs: `work/instrument-panel-style-focused.log` and `work/instrument-panel-style-tests.log`.
- Strict workspace/all-target Clippy (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), `cargo fmt --all --check`, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/instrument-panel-style-clippy.log` and `work/instrument-panel-style-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.

## Tracks-style Instrument toolbar and unused workspace — 2026-10-12

- Instrument now reuses `Toolbar::row` and the standard Lucide Plus button from the Tracks header. Its title is left-aligned with the same font and padding; the header uses the neutral panel background and stays above the vertically scrolling layers. The header Plus adds an empty layer on release. Removed the separate Add layer footer. This supersedes the plain, unfilled section-heading design above; unit headers remain accent-colored.
- Unused space beside and below layers retains the theme's workspace background (dark in the dark theme). Only layer labels, source/envelope units, and add areas paint the panel background. Add source remains conditional on an empty source list, and flat add areas retain hover/focus feedback and shared thin dividers.
- All 381 workspace tests, including 260 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated two-theme checks verify the header Plus placement and shared control size, layer addition, label alignment/rendered text color, unchanged accent unit headers, unused space free of panel fill, conditional source additions, and shared dividers. Existing independent controls, removal, Timeline/Editor parity, and rendering/persistence checks pass. Log: `work/instrument-tracks-header-tests.log`.
- Strict workspace/all-target Clippy (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), `cargo fmt --all --check`, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/instrument-tracks-header-clippy.log` and `work/instrument-tracks-header-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.

## Instrument column, layer reordering, and names — 2026-10-12

- Instrument now shares the Tracks panel's 250-point column width. Its title and Plus stay in that column; layer headers use the shared Lucide grip, 13-point name font, 7-point inset, and 6-point handle/name gap. The column keeps the panel background below its rows, while the header and unused unit space to its right use the workspace background. This supersedes the previous full-width Instrument toolbar and workspace fill below the label column. One thin divider separates the column from the unit area.
- Grip drags preview an insertion line and scroll near viewport edges without moving the fixed header. Release commits the row order. Escape, focus loss, disabled editing, and outside drops cancel; no-op drops stay clean. Double-clicking a name selects its text. Enter saves trimmed nonempty text; Escape/focus loss and track/view/project changes cancel. Empty and unchanged names stay clean. File actions and view switches are guarded during layer drags; name editing guards clip/track actions.
- Layer names and order persist with stable IDs, sources, and envelope settings. Missing names in older version-2 instruments default to their original Layer N positions; explicit malformed names fail parsing, and blank names fail core edits atomically. Renderer preparation sorts layers by stable ID so names and row order do not restart live voices or alter offline audio.
- All 386 workspace tests, including 264 UI tests, pass with `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Checks cover two-theme column geometry and backgrounds, drag release/cancellation, edge scrolling and the fixed header, rename commit/cancellation, old-manifest names, malformed/blank names, persistence through save/reopen/Save As, and sample-exact live/offline audio after renaming and reordering. Log: `work/layer-workspace-tests.log`.
- Strict workspace/all-target Clippy passes with `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`. `cargo fmt --all --check` and `git diff --check` pass. The CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-clippy.log`, `work/layer-cli-build.log`, `work/layer-cli-smoke.log`, and `work/layer-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.

## Instrument/Effects view split and Layers header — 2026-10-12

- Renamed Units to Instrument in the shared view enum, titlebar/view controls, and toolbar/native View menus. Added Effects after Instrument; all entry points use Timeline, Instrument, Effects, Editor, Mixer. Renamed the instrument column header to Layers, retaining its Plus, width, dividers, and existing layer controls.
- The shared playback-toolbar selector now lives in `track_selector.rs`. Instrument lists MIDI tracks only and shows its MIDI selection prompt for an unsupported current track. Effects lists audio/MIDI tracks and uses the separate `effects_panel.rs` view. Processing-view track switches retain clip selection and project state; Editor retains its active-clip behavior. Both editor and layer drags disable the selector. Effects currently shows an explicit unavailable-processing message or a track-selection prompt; no effect DSP or effect controls were added.
- All 388 workspace tests, including 266 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). New two-theme input tests verify MIDI-only Instrument choices, audio/MIDI Effects choices, Layers labeling, source controls absent from Effects and audio selections, unchanged clip/project state, and empty prompts. Existing menu release/state and titlebar styling tests now cover all five views. Existing instrument, layer, MIDI editor, persistence, and rendering checks pass. Logs: `work/instrument-effects-ui-tests.log` and `work/instrument-effects-workspace-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/instrument-effects-clippy.log` and `work/instrument-effects-package.log`. Native appearance/menu interactions, device playback, and Windows/Linux native UI were not checked.

## Uniform instrument layer height — 2026-10-12

- Empty layers now use the full shared unit-panel height, matching source-only, envelope-only, and combined layers. Labels and Add source/Add envelope regions fill the same height. Removed the 44-point empty-row exception; height derives from the shared controls, panel padding, header, and borders. Adding or removing units does not resize a row.
- Extended the existing two-theme geometry check to compare all four layer configurations at a narrow viewport with horizontal overflow. Updated the reorder gesture test to drop after the final row's midpoint while remaining inside the viewport. Focused layer checks pass; all 388 workspace tests, including 266 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Logs: `work/layer-height-focused.log` and `work/layer-height-workspace-tests.log`.
- Strict workspace/all-target Clippy, `cargo fmt --all --check`, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-height-clippy.log` and `work/layer-height-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Instrument empty-state cards — 2026-10-12

- Missing sources and envelopes now use the shared `empty_state.rs` component, with rounded dashed outlines, centered titles, muted descriptions, and standard Add source/Add envelope CTA buttons. Only the CTA adds a unit on release; titles, descriptions, and margins are non-actionable. This supersedes the earlier fully clickable flat add regions.
- Cards have no background fill. Their 240-point-wide regions retain the full layer height and reserve 20-point margins on all sides. Adjacent cards share one horizontal margin so the gap between dashed outlines is 20 points. Installed unit panels retain their shared thin divider and existing controls. Documented this layout in the instrument behavior/specification and repository guidance.
- All 388 workspace tests, including 266 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Updated two-theme painting/input checks cover card text, dashed outlines, transparent surfaces, equal outer/inter-card spacing, centered content, CTA-only additions, release behavior, independent layers, and unchanged row heights. Logs: `work/empty-states-focused.log` and `work/empty-states-workspace-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/empty-states-clippy.log` and `work/empty-states-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Shared unit collapse — 2026-10-12

- Double-clicking an installed unit header now collapses its body into a full-height accent strip; double-clicking the strip expands it. The strip uses the shared 24-point header dimension (26 points including the border), 11-point title, current accent, and contrast rule. The title is centered and reads bottom-to-top. Single clicks and disabled controls do not toggle collapse. Header tooltips/accessibility names identify the action, and the Envelope removal menu remains available in either state.
- `PanelStyle::unit` owns the presentation and temporary state, scoped by the enclosing track/layer and unit name. Oscillator and Envelope reuse it; future units must use the same shell. Hidden bodies do not run their UI controls. Collapse retains layer height, source/envelope settings, and audio processing, makes no project edit, and stays clean. State follows layer identity through reorder and view switches; it is not saved in instrument settings.
- All 389 workspace tests, including 267 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). New input/painting checks cover independent collapse/expand, single-click and disabled behavior, unchanged project data/dirty state, restored controls, fixed layer height, shared borders, accent/text contrast in both themes with Deep Orange/Yellow, and state retained across reordering/view switches. Existing envelope-removal checks now run expanded and collapsed, retaining release-only edits and other layers. Logs: `work/unit-collapse-toggle-tests.log`, `work/unit-collapse-menu-tests.log`, and `work/unit-collapse-workspace-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/unit-collapse-clippy.log` and `work/unit-collapse-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.


## Instrument layer mixing controls and right-aligned M/S — 2026-10-12

- Layer headers now follow the Tracks controls: grip/editable name and M/S on the first row, then compact gain/pan knobs with numeric fields. Timeline tracks and layers reuse `mix_controls.rs` for toggles, knobs, parsing, and numeric commit/cancel rules. Reserve the full name column so short and long names retain the same right-aligned M/S positions as Tracks. Existing column width, full layer height, naming, reordering, unit collapse, and empty-state layout remain in use. Layer changes use `Edit::SetInstrument`; numeric drafts are scoped by layer identity and cleared on track/view replacement or layer removal.
- Version-2 layers save `muted`, `soloed`, `gain_db`, and `pan`. Missing fields load as off/off/0 dB/center, including migrated version-1 instruments. Gain and linear amplitude must be finite; pan must be finite and bounded −1 through +1. Invalid edits remain atomic. Extended core round-trip/legacy/invalid-edit checks and project save/reopen/Save As/export checks cover these controls with independent layers.
- Layer solo applies within its instrument and shares core `mix_audible` with tracks: solo overrides mute when any peer is soloed. Layer gain and center-normalized equal-power pan follow the envelope; layers sum in stereo before the existing track gain/pan and master. This retains legacy centered-layer output and existing track panning. Sources and envelopes keep processing when muted or excluded by solo. Compatible live edits preserve voices and phase and ramp active stereo coefficients over 5 ms; idle/new-note slots start at the new controls. Unrelated control updates do not restart a layer ramp.
- All 392 workspace tests, including 268 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). New two-theme input/geometry checks cover right-aligned M/S across different names, shared knob/numeric dimensions, release-only toggles, keyboard knob changes, Enter/focus-loss commits, Escape cancellation, exact silence, rejected pan, and unchanged peer-layer/track controls. New engine checks cover stereo endpoints, gain, mute, solo precedence, peer exclusion/summing, playback/export parity, a measured 5 ms live ramp, retained phases/future-note state, and allocation-free retargeting. Existing source, envelope, layer, collapse, and track checks pass. Logs: `work/layer-controls-workspace-tests.log`, `work/layer-controls-ui-tests.log`, and `work/layer-controls-engine-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`, including the final M/S alignment fix. Logs: `work/layer-controls-clippy.log`, `work/layer-controls-cli-build.log`, `work/layer-controls-cli-smoke.log`, and `work/layer-controls-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.


## Layer header drag areas — 2026-10-12

- Layer moves can start from the grip, name, or unused space throughout the left header panel. All areas call `start_layer_drag` and reuse the existing reorder preview, viewport scrolling, release commit, and cancellation logic. This supersedes the grip-only restriction. Double-click rename remains available. M/S, gain/pan controls, and the active name editor retain their own interactions; background drags exclude controls at the pointer press origin even after the pointer leaves them.
- Extended automated egui checks cover grip/name/blank/inset/right-gutter/body starts, clean previews, release commits with stable IDs/settings, no-op moves, Escape/focus loss/disabled/outside cancellation, and scrolling from blank header space with the fixed Layers toolbar. Added control/name-editor drag checks; existing rename, release-only toggles, parameter adjustment, and unit interaction checks pass. Updated repository guidance, MIDI behavior, modular-unit documentation, and acceptance criteria to require one shared layer drag handler.
- All 393 workspace tests, including 269 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-header-drag-focused.log`, `work/layer-header-drag-workspace-tests.log`, `work/layer-header-drag-clippy.log`, and `work/layer-header-drag-package.log`. Native gestures, device playback, and Windows/Linux native UI were not checked.


## Unit power controls and collapsed header caps — 2026-10-13

- `PanelStyle::unit` now supplies a 10-point circular on/off control with a 22-point hit area, tooltip, and accessible state. It has no visible label or icon. Toggles commit on release, exclude disabled/unfocused input, and never trigger collapse, including double-clicks. Enabled headers reuse the active view-mode button background through `theme::toggle_colors`; their power circles use the full current accent. Off headers use the theme’s neutral muted surface and a hollow circle in the contrasting header foreground.
- Collapsed units retain a regular panel background and primary-text vertical title centered below a 24-point header cap containing only the power control. The header cap retains the same on/off colors as expanded headers. Double-clicking the collapsed body expands the unit; header/body context menus remain available. Width, full layer height, stable UI identity, and temporary collapse state remain unchanged. This supersedes the previous full-height accent strip. Shared unit requirements and repository guidance document the final layout/color rules for future units.
- Saved layer settings add `oscillator_enabled` and `envelope_enabled`, defaulting to true for older version-2 manifests and migrated version-1 instruments. Oscillator off silences its sources without changing their individual switches or saved parameters. Envelope off crossfades to the existing short note gate. Both ADSR and gate keep advancing so re-enabling retains note, phase, and stage progress. Live source output and envelope bypass changes ramp over 5 ms; idle voices take new states immediately. The switches remain independent of layer mute/solo and temporary collapse; controls remain editable when off. Playback and export share these states without callback allocation.
- All 396 workspace tests, including 270 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Input/painting checks cover both themes, Deep Orange/Yellow accents, expanded/collapsed visibility, active-view background parity, accent/hollow circle colors and dimensions, release-only toggles, double-click isolation, neutral collapsed bodies, centered vertical titles, disabled/focus guards, and retained peer/source/envelope settings. Added engine tests cover 44.1/48 kHz source silence, layer independence, ADSR bypass, playback/export parity after transport start fades, a measured 5 ms live crossfade/gain ramp, retained phase/stage progress, and allocation-free replacement. Core and project checks cover default migration, round-trip/save/reopen/Save As, and export with disabled units. Logs: `work/unit-power-ui.log`, `work/unit-power-engine.log`, and `work/unit-power-workspace-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug` after the final header color update. Logs: `work/unit-power-clippy.log`, `work/unit-power-cli-build.log`, `work/unit-power-cli-smoke.log`, and `work/unit-power-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.


## Off unit header border and muted text — 2026-10-13

- Off unit headers now use the main panel border color (`palette.border`) rather than the previous muted surface. Titles and hollow power circles use the standard muted text color (`palette.secondary`). The collapsed header cap uses the same border fill; its vertical body title also uses secondary text when off. On headers continue to share the active view-mode background, with full-accent power circles and their existing title colors. Updated the shared shell, theme helper, and unit guidance/specification.
- Extended existing two-theme painting checks to compare off header fills with the actual default panel border stroke and verify muted expanded/collapsed titles and power circles. All 396 workspace tests, including 270 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/unit-off-header-ui.log`, `work/unit-off-header-tests.log`, `work/unit-off-header-clippy.log`, and `work/unit-off-header-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Shared Timeline and Editor clip painting — 2026-10-13

- Both clip strips and drag previews use `clip_paint.rs::paint_clip`. Editor unselected clips now match Timeline's rounded body/header corners, divider outline, title spacing/font, track or override colors, MIDI note previews, and repeat markers. Missing-source states also use the shared renderer. Every selected clip uses the common selection outline, including secondary group selections. Editor passes provisional MIDI sources into the renderer to retain note-edit and source-resize previews. View layout, zoom, scroll, and piano-roll note gestures remain separate. The MIDI parity contract and repository instructions require future clip design changes to reuse this renderer.
- Added paired full-view painting checks in both themes, with inherited/custom clip colors, unselected/primary/group selection, repeated note previews, repeat markers, and a clip crossing the left viewport edge. All 397 workspace tests, including 271 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Existing Timeline/Editor creation, move, duplication, trim/loop/stretch, cancellation, and note-edit checks pass. Logs: `work/editor-clip-paint-parity.log` and `work/editor-clip-paint-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/editor-clip-paint-clippy.log` and `work/editor-clip-paint-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Shared Tracks and Layers control panel — 2026-10-13

- Tracks and Layers are instances of `panels.rs::ControlPanel`. The base owns their shared 250-point column width, full-height panel background, left-aligned title/right-aligned action toolbar, and continuous 1-point neutral right border. Views supply their row contents and actions. The border is painted after row fills, clip lanes, and selection outlines so it stays uniform from the header through unused space. Removed independent track-row and Master right borders to avoid stacked strokes. Updated repository guidance and instrument behavior documentation to require this shared base.
- Added full-view painting checks for Tracks and Layers in both themes and short/tall viewports. They verify the shared background/column width, title inset, header control containment, exactly one full-height border, and its paint order after row fills and selected-track outlines. All 398 workspace tests, including 272 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Existing header actions, track/layer control geometry, naming, reordering, scrolling, and selection checks pass. Logs: `work/control-panel-parity.log`, `work/control-panel-ui.log`, and `work/control-panel-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/control-panel-clippy.log` and `work/control-panel-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Track panel and horizontal timeline separator alignment — 2026-10-13

- Timeline lanes now take their vertical bounds from their laid-out track panels. The shared `ControlPanel::row_dividers` paints one horizontal separator across the complete panel/lane row at the panel border coordinate, replacing independent lane bottom strokes. Layers reuse the same divider helper. Row separators stay below clip shells and selection outlines; the shared column's final right border remains continuous. Updated repository instructions and layout requirements to retain this geometry during future changes.
- Added painting/geometry checks in both themes at 1.0, 1.25, and 2.0 pixels per point, before and after vertical scrolling. They verify matching panel/lane bounds, a single full-width row separator, no independently positioned lane bottom border, and unchanged project data. All 399 workspace tests, including 273 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Existing clip appearance parity, shared panel shell, selection, controls, scrolling, and drag/reorder checks pass. Logs: `work/track-grid-alignment.log`, `work/track-grid-ui.log`, and `work/track-grid-tests.log`.
- Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/track-grid-clippy.log` and `work/track-grid-package.log`. Native appearance/gestures and Windows/Linux native UI were not checked.


## Project-owned unit collapse state — 2026-10-13

- Unit collapse is now saved in each version-2 instrument layer as independent `oscillator_collapsed` and `envelope_collapsed` flags. Missing fields and migrated version-1 instruments default to expanded. `PanelStyle::unit` reads and updates the caller's project-owned flags instead of temporary egui data. Header double-clicks commit through the existing `Edit::SetInstrument` and mark the project modified; single clicks, power clicks, disabled controls, and focus guards keep their existing behavior. Save/reopen and Save As restore presentation, and row/view/track changes retain it. Replacement projects read their own flags. Hidden controls retain their values and keep processing audio. This supersedes the temporary-collapse behavior recorded in earlier validation entries.
- Updated two-theme collapse input/painting checks for persisted flags, modified-state changes, independent units/layers, unchanged height and parameters, disabled guards, view switches, and row moves. Added a fresh-context UI restoration test and same-ID project-replacement check. Extended core defaults/legacy loading/round-trip/malformed-field checks and actual project save/reopen/Save As/export checks with independent collapse flags. Extended live renderer inheritance and offline audio checks to change collapse during held notes; output remains exactly equal to the reference.
- All 400 workspace tests, including 274 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/saved-collapse-focused-ui.log`, `work/saved-collapse-core.log`, `work/saved-collapse-engine.log`, `work/saved-collapse-tests.log`, `work/saved-collapse-clippy.log`, `work/saved-collapse-cli-build.log`, `work/saved-collapse-cli-smoke.log`, and `work/saved-collapse-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.

## Compact instrument layers — 2026-10-13

- Added a saved `collapsed` flag per instrument layer. Older version-2 projects and migrated version-1 instruments default to expanded. A square header button after M/S uses Lucide Arrow Up From Line to collapse and Arrow Down From Line to expand. Collapsed layers use the shared 68-point Tracks row height; expanded layers retain 244 points. Compact units reuse the existing shell and footer controls while omitting knob rows. Source Power/labels, stage labels, unit power, and Oscillator Plus remain available. Empty layers keep their transparent dashed cards, shared margins, and centered add actions, with descriptions in tooltips. Independent horizontal unit collapse remains saved; when both collapse states apply, vertical titles read Osc and Env. This supersedes earlier records requiring full height for every layer regardless of presentation.
- Added egui input/geometry checks in both themes for release-only commits, exact row/panel height, unchanged unit widths, horizontal overflow, empty/source-only/envelope-only layers, footer actions, compact additions, independent saved states, fresh UI restoration, and expanding one layer without affecting others. Checked mixed-height reordering, disabled/focus/rename guards, button press exclusion from background drags, and abbreviated vertical text fitting below its power cap. Existing layout, mixing, rename, reorder, and unit-collapse checks pass. Extended model defaults/round-trip/malformed-field tests, actual save/reopen/Save As/export coverage, and live renderer inheritance/offline audio equality checks with layer collapse flags.
- All 402 workspace tests, including 276 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. CLI build and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-collapse-guards.log`, `work/layer-collapse-workspace.log`, `work/layer-collapse-clippy.log`, `work/layer-collapse-cli-build.log`, `work/layer-collapse-cli-smoke.log`, and `work/layer-collapse-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.

## Layer action button width — 2026-10-13

- Expand/collapse now uses the same `icons::button` implementation as Add layer, replacing the narrow square variant. The layer toolbar supplies the measured Add layer width for header placement. Both buttons are 30 points wide with the current shared style. Collapse stays inside the panel, with no overlap or movement of M/S. This supersedes the square button geometry in the compact-layer record above.
- Extended the existing two-theme layer checks to compare button widths in both expanded and collapsed states, retain panel containment, and exclude M/S overlap. All 402 workspace tests, including 276 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-button-width-focused.log`, `work/layer-button-width-tests.log`, `work/layer-button-width-clippy.log`, and `work/layer-button-width-package.log`. Native UI appearance and gestures were not checked.

## Layer header button spacing — 2026-10-13

- Put collapse in the same row layout as M/S instead of positioning it separately near the panel border. All three buttons now share the 6-point gap, and collapse uses the header’s 7-point top/right inset. Retained its Add layer width, two-row collapsed height, shared controls, and interaction guards. The name column reserves room for the full button group, moving M/S left to keep spacing consistent. This supersedes earlier validation of exact track M/S positions and the width change’s border placement.
- Existing geometry checks now verify equal gaps, top/right insets, Add layer width, panel containment, and unchanged gain/pan geometry in both themes and both layer states. All 402 workspace tests, including 276 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/layer-button-spacing-focused.log`, `work/layer-button-spacing-tests.log`, `work/layer-button-spacing-clippy.log`, and `work/layer-button-spacing-package.log`. Native UI appearance and gestures were not checked.

## Compact unit title suffix — 2026-10-13

- Units collapsed within collapsed layers now display the first three letters of their name followed by three periods: Osc... and Env.... `PanelStyle::unit` derives this title from the full name, sharing the rule for future units. Other presentations retain full titles; widget identity, accessible names, and saved state retain the full name. This supersedes the bare Osc/Env titles in earlier records.
- Updated the existing collapsed-unit painting check to require the exact suffixed text and verify its vertical bounds fit below the power cap. All 402 workspace tests, including 276 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/compact-unit-title-focused.log`, `work/compact-unit-title-tests.log`, `work/compact-unit-title-clippy.log`, and `work/compact-unit-title-package.log`. Native UI appearance and gestures were not checked.

## Effects workspace and bottom strip — 2026-10-13

- Replaced the unavailable-processing bar/message with a full-height, 250-point Effects catalog using the Tracks/Layers `ControlPanel`. Extracted the Editor ruler, selected-track strip, and clip geometry into `track_overview.rs` and reused them in Effects. Audio and MIDI tracks share clip painting and existing clip/ruler gestures. Effects follows the selected processing track and preserves clip selection; Editor and Effects share overview zoom/scroll independently of Timeline. A missing selection retains the catalog, ruler, and empty effects strip, with a prompt in the track row. No effect controls, project fields, or processing were added.
- The user's strip reference requires a distinct area at the bottom. The empty strip is now pinned to the bottom right, with top/bottom panel-border lines and the workspace background. It uses the same 244-point expanded unit height as Instrument through shared panel geometry, clamped to available space in short windows. The timeline scrollbar sits immediately above the bottom strip and retains the ruler/lane alignment. Its 15-point row height is shared with Timeline and Editor. The shared timeline grid and playhead extend from the track strip to the scrollbar; pinch and horizontal scrolling cover that complete timeline area. This supersedes the initial layout that treated all remaining space as the strip, and the placeholder behavior recorded for the earlier Instrument/Effects split.
- Added two-theme geometry/painting checks at large, narrow, and short sizes for full-height catalog, continuous right border, ruler/track alignment, bottom anchoring, strip height, both strip borders, and scrollbar adjacency/alignment, and grid lines reaching the scrollbar. Added selected-processing-track/missing-selection checks and ruler seek/loop/pinch and scrollbar-drag checks, including zoom over the extended grid, ignoring zoom over the lower strip and retaining the Timeline transform. Extended shared MIDI appearance, move/duplication/trim/loop/stretch/cancellation, empty-lane creation, and clipboard checks to Effects.
- All 405 workspace tests, including 279 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/effects-scrollbar-focused.log`, `work/effects-scrollbar-tests.log`, `work/effects-scrollbar-clippy.log`, and `work/effects-scrollbar-package.log`. Native appearance/gestures, device playback, and Windows/Linux native UI were not checked.

## Full-width effects lane and status separator — 2026-10-13

- Extended the bottom effects lane across the full window width, including below the catalog. The shared catalog panel now fills the upper workspace and stops at the lane's top edge. Retained the shared expanded unit height, short-window clamping, the timeline scrollbar immediately above the lane, and grid/playhead extension to the scrollbar. Removed the lane's bottom stroke; the status panel supplies the only thin separator, with no gap. This supersedes the bottom-right lane and catalog-height geometry recorded above.
- Updated both-theme geometry/painting checks at large, narrow, and short sizes to require full window width, catalog/lane adjacency, lane background across the left area, and exactly one 1-point separator at the status boundary. Existing ruler, clip, zoom, and scrollbar input checks pass.
- All 405 workspace tests, including 279 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy passes (`CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`), as do `cargo fmt --all --check` and `git diff --check`. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/effects-full-width-focused.log`, `work/effects-full-width-tests.log`, `work/effects-full-width-clippy.log`, and `work/effects-full-width-package.log`. Native appearance/gestures and device playback were not checked.

## Three-band EQ, middle shelf, and all-band silence — 2026-10-13

- Added the first serial effect for audio/MIDI tracks: double-click 3-band EQ in the shared Effects catalog to append an independent saved instance. The lane reuses the unit shell, base knobs, 90-point strips, header power/collapse, and saved track compact mode. Oscillator and EQ footer pairs share `rows::power_label` (18-point button, 8-point gap, 11-point label). Effects units omit their bottom stroke and share the status panel's one thin separator. Defaults are Low 250 Hz and High 2.5 kHz.
- The final accepted Mid design is a middle shelf with Gain and on/off only, using Low/High frequency boundaries and Q slopes. Opposing shelves set its middle plateau; Mid off uses parallel low/high-pass branches to reject that interval. Higher outer Q steepens the corresponding cut edge. Crossed boundaries retain their corresponding Q; equal boundaries leave no middle interval. This supersedes the earlier bell and Mid-Q design. Version-1 Mid frequency/Q fields remain saved for compatibility but do not affect UI or processing.
- Low/High off use high-pass/low-pass cuts. Finite filter slopes leave residual audio when all cuts combine, so the enabled unit additionally suppresses its wet output when no band remains enabled. This uses the existing 5 ms band ramps and produces exact stereo silence after the transition. Processing/history continue; re-enabling restores saved gain. Whole-unit off still bypasses the effect. Added tests cover all-band silence at six processing rates, equal/crossed boundaries, last-band on/off transitions, reset, bypass, and silence in playback, offline/block rendering, random-access rendering, and compatible live updates.
- Shared core validates saved parameter bounds and unique effect IDs atomically; old projects default to empty chains, duplicates get new effect IDs, and unknown/future effect state is preserved and bypassed with an open warning. Save/reopen, Save As, rate changes, and float WAV export checks retain effects, presentation state, sources, and playback/export parity. The engine processes aggregate clip/instrument output → serial effects → track gain/pan and audibility → Master. Compatible updates preserve history without callback allocation; seek/Stop reset it, pause retains it, and loops/gaps continue processing. Export retains its existing arranged duration.
- All 425 workspace tests, including 283 UI tests, pass: `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`. Strict Clippy passes: `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --workspace --all-targets --offline -- -D warnings`. `cargo fmt --all --check`, `git diff HEAD --check`, the offline CLI build, and `python3 tools/check_cli.py` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/eq-mid-shelf-focused.log`, `work/eq-mid-shelf-ui.log`, `work/eq-mid-shelf-engine.log`, `work/eq-mid-shelf-workspace.log`, `work/eq-mid-shelf-clippy.log`, `work/eq-mid-shelf-cli-build.log`, `work/eq-mid-shelf-cli-check.log`, and `work/eq-mid-shelf-package.log`.
- Native macOS check used a separate app copy and a new unsaved test project. Verified catalog double-click addition, Low/High defaults, Mid Gain with no Frequency/Q controls, aligned footer pairs, expanded/compact layout, continuous grid/scrollbar adjacency, and the thin status separator. Closed the test app and discarded only its generated project. The existing user app/project was not changed. Native device playback, performance targets, Windows/Linux UI, and native save/export dialogs were not checked; automated rendering/storage checks do not establish those results.

## Effects catalog item spacing and text — 2026-10-13

- Effects catalog entries now reuse `ControlPanel::catalog_item` with an 8-point frame margin on all sides and the theme's 11-point small text style. The text remains left-aligned. The frameless button uses its natural small-text height inside the padded row; it retains double-click addition, tooltip/accessibility, and editing guards. Padding belongs to the frame because egui removes internal margins from frameless buttons.
- Existing Effects interaction/layout checks pass. All 425 workspace tests pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`); strict workspace/all-target Clippy, `cargo fmt --all --check`, and `git diff --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/effect-item-focused.log`, `work/effect-item-workspace.log`, `work/effect-item-clippy.log`, and `work/effect-item-package.log`.
- Native macOS check confirmed the smaller label and visible left/top padding in an isolated app copy. The test app was closed; only generated test state was discarded. The user's open app/project was not changed. Windows/Linux native appearance was not checked. No audio behavior changed.

## Effects catalog hover and selection states — 2026-10-13

- `ControlPanel::catalog_item` now renders dim idle text from the inactive view-mode color, default text on hover/focus, and default text plus `theme::toggle_colors`' active-view fill when selected. The whole row is interactive, retaining 8-point padding, 11-point text, tooltips, and accessible selected state. Single-click selects a catalog type in UI state without a project edit; double-click still appends an effect. Selection persists across view/track changes within the app session.
- Added both-theme painting/input checks with Deep Orange/Yellow accents for idle/hover/selected colors, exact shared selected fill, retained padding/font size, single-click selection, and unchanged project contents/dirty state. Existing double-click addition, saved controls, guards, and workspace layout tests pass. All 426 workspace tests, including 284 UI tests, pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline`). Strict workspace/all-target Clippy, `cargo fmt --all --check`, and `git diff HEAD --check` pass. Rebuilt `target/debug/DAW.app` with `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`. Logs: `work/effect-catalog-state-focused.log`, `work/effect-catalog-state-workspace.log`, `work/effect-catalog-state-clippy.log`, and `work/effect-catalog-state-package.log`.
- Native macOS check in an isolated app copy confirmed dim idle text, default selected text, the selected view-mode background, and single-click selection without an installed effect. Closed the test app and discarded only its generated track/project. The user's open app/project was not changed. Hover was checked by automated input/painting tests; Windows/Linux native appearance was not checked. No audio behavior changed.

## Drive effect — 2026-10-13

Implemented the agreed Drive and Color strips in the existing serial effects lane. Drive has Volume/Gain/Tone; Color has Shape/Strength/Frequency, with Off, Mid scoop, Mid hump, and Mid hump + bass cut. The catalog shares EQ insertion, selection, dim/hover colors, and track guards. The unit shares the header, saved power/collapse, compact lane, standard knobs, 90-point strips, and bottom separator. Compact mode keeps both labels. Color Off keeps saturation active and retains the disabled Strength/Frequency values. See [Drive](../effects/drive.md) for bounds, defaults, processing, and the project-rate clipping limitation.

Checks completed:

- `CARGO_HOME="$PWD/work/cargo-home" cargo fmt --all --check` passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --workspace --all-targets -- -D warnings` passed, including the final endpoint-label adjustment.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --workspace` passed: 442 tests, including 288 UI tests. After shortening Tone endpoint labels to avoid crowding, the full 288-test UI library passed again.
- Drive DSP checks cover all six project rates, color curve polarity/depth, neutral Strength and Off, linked frequency movement, clean zero Gain, harmonics, output scaling/exact silence, neutral/dark/bright Tone, independent stereo state, 5 ms transitions, deterministic reset, unrelated in-flight ramps, and finite maximum controls/high-level input.
- Shared engine checks cover audio/MIDI processing, serial EQ/Drive order, playback/export/random-access consistency, all supported buffer sizes, compatible state preservation, bypass, and zero allocations during callback inheritance, retargeting, processing, Stop/reset, and restart.
- Core checks cover versioned round trips, invalid-edit rollback, stable settings with new IDs on track duplication, and opaque future-version preservation. Project workflow checks cover save/reopen, Save As, processing-rate changes, exact float WAV/render equality, and unchanged source recordings.
- egui input checks cover catalog single/double-click behavior, independent instances alongside EQ, track switching, modal/no-selection guards, all parameter drags, keyboard shape selection, Gain reset, retained inactive parameters, header power, and saved presentation. Dark/light layout checks cover matching EQ row/header/footer geometry and compact labels.
- CLI/desktop builds passed. `python3 tools/check_cli.py` passed its WAV/MP3 codec/bitrate, repeatability, missing-source, eight-track, overwrite, and exit-code checks.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`, including after the final endpoint-label adjustment. Existing packaged FunDSP dependency notices remain included.
- An isolated macOS test app confirmed catalog double-click insertion, Drive/Color strip labels, expanded geometry, activation of Color controls, compact labels, header bypass, and horizontal collapse. Its generated project was test-only; the test app closed. Native inspection identified crowded Dark/Bright endpoint labels; the final implementation uses compact minus/plus endpoints, checked by the UI suite. The final endpoint change was not re-inspected natively.

Native audio-device listening, sustained callback performance with a representative polyphonic workload, and Windows/Linux UI/device checks remain unverified. The clipping stage runs at the project rate without oversampling; high-frequency material at strong drive can alias. No exact pedal-circuit model or additional export-tail policy is claimed.

## Effect unit selection and chain reordering — 2026-10-13

- Installed effect units support plain-click single selection and Shift-click membership toggles. Empty lane content clears selection. Catalog, track, and clip selections remain independent. Selection is UI-only, retains stable IDs through reordering/view switches, prunes removed IDs, and clears on project replacement. Selected units and Tracks reuse `panels::selection_border`, including the theme-aware stroke, inside placement, and corner radius. Instrument units retain their existing behavior.
- Header dragging moves selected instances as a group in their existing chain order, including separated selections and reverse selection-click order. Unselected instances retain their relative order. An unselected dragged unit becomes the sole selection. The lane previews insertion and scrolls at its edges, and commits through `Edit::SetEffects` only on release. Capture the press before movement so a motion/release in one frame retains its origin. Cancellation, focus loss, modal editing, externally changed chains, and outside/no-op drops leave project contents and dirty state unchanged. Power, knobs, and double-click collapse keep their existing behavior; ordering preserves every instance's ID, settings, enable/collapse flags, and saved presentation.
- Automated egui checks cover single/group moves in both directions, compact/expanded and horizontally collapsed units, separated groups, selection border painting in both themes, body/header clicks, Shift toggles, empty-area deselection, selection across reordering/view changes, removed IDs, controls, release-only persistence, save-format round trips, no-op/cancel cases, focus/modal guards, secondary drags, fast drags, and scrolling to offscreen units. Existing shared engine checks cover order-dependent serial processing and compatible identity/state preservation.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --workspace` passed: 453 tests, including 299 UI tests. Strict workspace/all-target Clippy, `cargo fmt --all --check`, and `git diff HEAD --check` passed. `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Logs: `work/effects-reorder-focused.log`, `work/effects-interaction-workspace.log`, `work/effects-interaction-clippy.log`, and `work/effects-interaction-package.log`. After restricting empty-area clicks to the scroll content so scrollbar clicks retain selection, all 299 UI tests passed again (`work/effects-interaction-final-ui.log`); strict Clippy, formatting, and packaging also passed again.
- An isolated macOS app confirmed EQ/Drive insertion, plain-click selection with the matching accent border, and empty-lane deselection. Native drag automation left the chain unchanged, so native reordering, group movement, and Shift-click remain unverified. The automated input checks above pass. The test app was closed and its generated project was disposable; no user project was changed. Windows/Linux UI checks and audio-device listening remain unverified. No DSP algorithm changed.

## Auto-filter — 2026-10-13

Implemented [Auto-filter](../effects/auto-filter.md) as a portable `daw-auto-filter` crate with versioned shared settings, core validation, project persistence, and serial engine integration. Its four-strip UI reuses the effect shell, catalog, knobs, selected border, reordering/removal, saved collapse, and compact lane. The two middle strips have no internal divider and share one centered standard LFO Power/label pair. Cutoff and Out use the same footer label helper as Drive. Both phase controls span −180° to +180°; LFO off retains a static filter, while header power bypasses the whole unit.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --workspace` passed all 470 tests, including 302 UI tests. A subsequently added loop-wrap/history check passed separately with `cargo test --offline -p daw-engine --test effects auto_filter_loop`. No implementation changed after the full workspace run.
- DSP checks cover all four static filter responses at all six project rates, waveform bounds/polarity, exact octave sweep endpoints, signed stereo/sync phases, all 27 note durations, Hz independence from tempo, smooth rate realignment, static LFO bypass/zero depth, independent stereo histories, Dry/Wet, output gain/silence, header bypass, deterministic reset, retained in-flight ramps, resonance, and finite extreme controls.
- Shared engine checks cover audio and MIDI buses, playback/offline/block/random-access parity at all supported rates and buffer sizes, order-dependent Drive/Auto-filter processing, transport loops, and compatible live state publication. Allocation counting found zero callback allocations in inheritance, retargeting, processing, Stop/reset, seek, and restart.
- Core/storage checks cover invalid-edit rollback, versioned round trips, new instance IDs on track duplication, future opaque state preservation, save/reopen, Save As, processing-rate changes, unchanged source recordings, and exact float WAV/export-render equality.
- Automated UI checks cover independent catalog insertion, selection/editing guards, parameter drags, retained Hz/note rates, switches, collapse/persistence, both themes, expanded/compact strip geometry, aligned rows/footers, the standard LFO button/label gap, and the absence of an internal LFO divider. Existing selection/reorder and Drive/EQ tests pass.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --workspace --all-targets -- -D warnings` passed, including after the final loop test. `cargo fmt --all --check` and `git diff HEAD --check` passed. CLI build and `python3 tools/check_cli.py` passed the codec/bitrate, repeatability, missing-source, eight-track, overwrite, and exit-code checks.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Existing packaged FunDSP dependency notices remain included; no external dependency was added.
- Native macOS inspection in a separate app copy and disposable project confirmed the catalog item, coordinate double-click insertion, all default controls, four-strip expanded layout, shared LFO footer/no middle divider, independent LFO switch, and compact footer layout. The test app was closed and its generated project discarded. No user project was changed. AX double-click selected the catalog item without adding it; coordinate double-click added it successfully.

Logs are in `work/auto-filter-{dsp-model,ui,engine,workspace,loop,clippy-final,cli-build,cli-check,package}.log`. Native audio-device listening, sustained performance against the representative eight-voice callback target, Windows/Linux UI/device checks, and native parameter drag/save/export workflows remain unverified. Automated DSP, persistence, and UI checks do not establish those results. Export retains the existing arranged duration without adding a resonant tail.

## Auto-filter LFO column spacing — 2026-10-13

The LFO is one strip with two control columns. This supersedes the initial two-strip geometry recorded above. Its three horizontal gaps are equal: left inset, column gap, and right inset. The gap derives from the shared single-column control inset (19 points with the current 90-point strip and 52-point knob row). The resulting LFO strip is 161 points wide. The shared footer stays centered, with no internal divider; Cutoff/Out geometry and processing are unchanged.

The existing both-theme layout check now measures all three gaps for each control row and confirms they match the single-column inset. All three Auto-filter UI checks pass, including catalog insertion, saved state, switches, expanded/compact layout, and parameter edits. Strict workspace/all-target Clippy, formatting, and diff checks pass. The macOS debug app was rebuilt. Logs: `work/auto-filter-spacing-{ui,clippy,workspace,package}.log`. This spacing adjustment was checked through egui layout/input tests; native appearance was not re-inspected.


## Auto-filter selectors, rate bounds, and reference marks — 2026-10-13

- The LFO remains one strip with two columns. Its inner gap and outer control padding match, with one centered LFO Power/label footer and no internal divider.
- Filter Shape shows low-pass, high-pass, band-pass, and notch response diagrams. LFO Wave shows sine, triangle, rising saw, falling saw, and square diagrams. Both reuse the shared diagram selector and highlight the active choice. Repository instructions and the unit specification now require this design for every shape-selection knob, with drawings of the shapes the control represents.
- Rate mode is a bounded two-position knob: Hz at the upper left, Note at the upper right, and the pointer turns toward the selected label. Rate mode and note duration do not wrap. Note duration runs from dotted 4 bars to triplet 1/64, retains all 27 saved IDs, resets to 1/4, and has one reference mark per duration. Strength has marks at every whole octave from 0 through 6 while retaining continuous adjustment and 0.1-octave keyboard steps. Shared knob focus now retains arrow-key editing across repeated press/release events.
- Focused Auto-filter UI checks passed all six tests, covering both themes, spacing, diagram paths and active highlighting, upper-corner labels, marker counts, bounded keyboard/large-drag input, note ordering, reset, and retained rates. `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace --offline` passed all 474 tests, including 305 UI tests (`work/auto-filter-final-workspace.log`). Strict workspace/all-target Clippy, formatting, and debug macOS packaging passed. Logs: `work/auto-filter-controls-ui.log`, `work/auto-filter-final-clippy.log`, and `work/auto-filter-final-package.log`.
- Native appearance for this revision remains unverified: the isolated macOS app launched, but the computer-use capture service then failed twice with ScreenCaptureKit error −3811. The temporary app was closed and confirmed stopped. No user project was changed. Windows/Linux appearance and device checks were not repeated. DSP and saved parameter meanings are unchanged.


## Auto-filter straight-duration markers and mode highlighting — 2026-10-13

- Note rate shows only the nine straight-duration reference marks: 4 bars, 2 bars, 1 bar, 1/2, 1/4, 1/8, 1/16, 1/32, and 1/64. This supersedes the preceding 27-marker design. All 27 straight/dotted/triplet choices, their ordering, bounded input, endpoints, and saved duration IDs remain unchanged. Strength retains its whole-octave marks.
- The shared two-way selector paints its selected label and marker in the accent color and its other choice in secondary text color. Auto-filter Hz/Note labels represent choices rather than range endpoints. Updated repository instructions and unit/control specifications require that presentation.
- All six focused Auto-filter UI checks pass. They verify the nine reference marks, pointer/mark alignment for each of the 27 durations, retained dotted/triplet selection, and both Hz/Note label/marker colors in both themes. All 474 workspace tests (including 305 UI tests), formatting, strict workspace/all-target Clippy, and macOS debug packaging pass. Logs: `work/auto-filter-choice-hints-{ui,clippy,workspace,package}.log`.
- Native appearance was not rechecked after the previously recorded macOS capture error. No DSP or saved state format changed.


## Two-option selector neutral arc — 2026-10-13

- The shared bounded two-option selector keeps a neutral border-color arc between its upper-left and upper-right choices. It has no reference marks and no accent value arc. Auto-filter Hz/Note retains its knob face and pointer; only the selected label uses the accent color, while the other label remains dim. Updated repository instructions and control specifications define this presentation.
- All six focused Auto-filter UI checks pass. Painting checks verify exactly one neutral arc between the options, unchanged arc color across both modes/themes, no reference marks, selected label colors, and unchanged bounded input. All 474 workspace tests (including 305 UI tests), formatting, strict workspace/all-target Clippy, and macOS debug packaging pass. Logs: `work/auto-filter-neutral-mode-{ui,clippy,workspace,package}.log`.
- Native appearance was not rechecked; the earlier macOS capture limitation remains recorded above. No DSP or saved state format changed.

## Windows ZIP packager — 2026-10-13

- Added `tools/package_windows.ps1`, using the macOS packager as the reference. It builds the desktop with `Cargo.lock`, accepts debug (default) or release, and writes `DAW.zip` beside Cargo's reported executable. The ZIP contains `DAW/daw-desktop.exe`, the project license, and the font, icon, UI, media, oscillator, and EQ notices. It uses standard ZIP entry paths and publishes a complete archive before replacing an existing one. Temporary staging is removed on success or failure. The README records commands, contents, output paths, and the required destination-PC Microsoft C++ runtime and graphics driver.
- Both profiles built and packaged successfully on Windows PowerShell 5.1. Verified extraction and SHA-256 equality for each executable, project license, and all 37 packaged dependency notices. Archives contain no extra files, projects, recordings, CLI, or debug symbols. Outputs are `target/debug/DAW.zip` and `target/release/DAW.zip`; logs are `work/windows-package-debug.log`, `work/windows-package-release.log`, and `work/windows-package-archives.log`.
- Isolated fixtures with a mocked Cargo artifact verified the default/release profiles, custom artifact paths with spaces, archive replacement, restored caller directory, invalid-profile rejection before building, build failure, missing-notice failure, unchanged existing ZIP on failure, and staging cleanup. These checks do not replace the actual builds above. The fixture runner and log are `work/check_windows_package.ps1` and `work/windows-package-smoke.log`.
- Formatting, strict workspace/all-target Clippy, and `git diff --check` pass. `cargo test --workspace --offline --no-fail-fast` completes with three failures in unchanged UI Rust code: the two Windows path-fixture failures recorded above, plus `windows_titlebar::custom_header_layout_reserves_native_buttons_and_switches_views_without_project_edits`, where the view controls extend into the caption reservation at 900 points with a long title. All other workspace tests pass. Logs are `work/windows-package-clippy.log` and `work/windows-package-tests.log`. No Rust source or dependency changes were made for packaging.
- Packaging checks used a process-scoped RemoteSigned policy; the saved execution policy remains Restricted. Clean-PC runtime checks, installed-app launch, signing, and native UI/audio-device checks were not run for this change.

## Windows desktop GUI subsystem — 2026-10-13

- The Windows desktop executable previously used the console subsystem, so launching an extracted package created a terminal window. Added a Windows-only `windows_subsystem = "windows"` crate attribute to the desktop entry point. Both debug and release desktop builds now use the GUI subsystem. The CLI and other platforms retain their existing configuration.
- Rebuilt both packages through `tools/package_windows.ps1`. Extracted each ZIP and verified that its executable matches the corresponding build by SHA-256 and has PE subsystem value 2 (Windows GUI), instead of the previous value 3 (Windows console). MSVC `dumpbin /headers` independently confirms Windows GUI for the release executable. Outputs are `target/debug/DAW.zip` and `target/release/DAW.zip`; logs are `work/windows-gui-package-debug.log`, `work/windows-gui-package-release.log`, and `work/windows-gui-subsystem.log`.
- Formatting, strict workspace/all-target Clippy, and `git diff --check` pass. `cargo test --workspace --offline --no-fail-fast` completes with the same three existing UI failures listed in the preceding packager entry; all other workspace tests pass. Logs are `work/windows-gui-clippy.log` and `work/windows-gui-tests.log`.
- The subsystem check verifies the Windows launch configuration. A live desktop launch, native UI/audio-device checks, and macOS/Linux builds were not run for this change.

## Native CI tests and package artifacts — 2026-10-13

- Added `.github/workflows/build-and-test.yml` for pushes, pull requests, and manual dispatches. Independent Windows x64 (`windows-2025`), macOS ARM64 (`macos-15`), and Linux x64 (`ubuntu-24.04`) jobs install the pinned Rust toolchain and Python 3.13, check formatting and strict workspace/all-target Clippy, run locked workspace tests, and run the headless CLI checks. Each successful job builds a release package and uploads it for 14 days. Windows uploads a portable `.exe` with the project license and dependency notices; macOS uploads a `.dmg`; Linux uploads a `.zip`. No GitHub release is published.
- Added native macOS DMG and Linux ZIP packaging scripts. Both retain an existing artifact on failure and clean temporary staging. Linux archives preserve executable permissions. The macOS app packager now uses locked dependencies and includes the original project license. All packages retain the font, icon, UI, media, oscillator, and EQ notices. README and repository instructions describe the runners, artifact contents, packaging commands, and system-runtime requirements.
- Corrected the shared UI test fixture's Unix-only missing-source path by generating an absolute temporary path on each platform. Corrected Windows header title-width reservation to include every view-button gap and the separator width. A direct header-layout regression check now runs on all platforms, covering long titles, both themes, and 900/1280-point widths. These changes address the causes of the three Windows failures recorded above; native Windows confirmation remains pending.
- On macOS, formatting, strict locked workspace/all-target Clippy, and all 476 workspace tests pass, including the two fixture tests and the new header-layout check. The CLI build and `python3 tools/check_cli.py` pass. The debug desktop app was rebuilt. Logs: `work/ci-validation/final-clippy.log`, `final-tests.log`, `cli-build.log`, `cli.log`, and `final-app.log`.
- Actionlint 1.7.12 passes for the workflow; ShellCheck was unavailable and disabled for that invocation. YAML checks verify all three runners, triggers, the test gate, upload paths, and retention. `sh -n` passes for all shell packagers. README/AGENTS file links and `git diff --check` pass.
- The debug ARM64 macOS DMG builds successfully. Its checksum passes; a read-only mount verifies byte-for-byte app contents, executable permission, the project license, all 37 dependency notices, and the Applications link. The image was detached after inspection. Logs: `work/ci-validation/macos-dmg.log` and `macos-verify.log`. Isolated Linux fixtures with mocked Cargo/uname and real ZIP creation pass both build profiles, locked build arguments, all 37 notice hashes, executable permission, invalid-profile rejection, failure preservation/cleanup, and ARM64 naming (`work/ci-validation/check_linux_package.py`). These fixtures do not establish native Linux builds.
- The workflow has not been pushed or dispatched. Hosted runner execution, artifact downloads, current native Windows/Linux builds, macOS release packaging, and native UI/audio-device checks remain unverified. The unsigned macOS app is not notarized; Linux packages require compatible system libraries. No user project or recording was changed.

## Separate CI test and build jobs — 2026-10-13

- Replaced the combined job above with separate `test` and `build` matrix jobs. Each uses Windows x64, macOS ARM64, and Linux x64 workers. `build` depends on the complete `test` matrix; release compilation, packaging, and artifact uploads start only after all three test workers pass. Formatting, Clippy, workspace tests, and CLI checks remain in `test`. Artifact formats and 14-day retention are unchanged.
- Both jobs check out the same workflow revision and use `.github/actions/setup-build` for Python, native dependencies, libclang, the pinned Rust toolchain, and Cargo caching. Composite shell steps declare their shells explicitly. README and repository instructions describe the split.
- Actionlint 1.7.12 passes for the workflow and an expanded scratch copy that includes the shared setup steps. ShellCheck remains unavailable. YAML checks pass for job separation, identical platform matrices, the dependency gate, checkout before local-action use, explicit composite shells, and retained artifact settings. Shared Bash steps pass `bash -n`; formatting, documentation file links, and `git diff --check` pass.
- No Rust source or native packaging scripts changed in this revision, so the preceding workspace, CLI, and native package checks were not repeated. Hosted GitHub execution remains unverified; the workflow was not pushed or dispatched.

## CI log visibility and first hosted run — 2026-10-13

- Inspected [run 38467606872](https://github.com/fecabrera/daw/actions/runs/38467606872), for commit `f7cb6432eae128136bce00eb3459bfc1ecd2fd55`. The GitHub CLI returned empty step lists for all jobs, matching the blank job pages, but could retrieve the raw logs. This indicates a step-metadata/display issue; its GitHub-side cause is unverified. Downloaded the full run to `work/ci-log-diagnosis/full-run.log` and a readable copy to `full-run-readable.log`. Run metadata is in `run.json`. This supersedes the preceding entries' lack of hosted-run evidence.
- The original hosted macOS test job passed. Windows failed during Clippy compilation because Rust selected Git Bash's `C:\Program Files\Git\usr\bin\link.exe`, which cannot link MSVC object files. Linux passed compilation but failed four group-stretch UI checks at the two-second background-job helper deadline. Build was skipped because the complete test matrix did not pass. Windows' Node.js deprecation warning did not cause its linker failure.
- Shared Windows setup now selects the absolute MSVC x64 linker through `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER`, after configuring the C++ tools. The UI test helper now uses a bounded 30-second wall-clock deadline and reports the pending job on timeout. The production UI and audio behavior are unchanged.
- Test and build jobs stream command output to the console and to `work/ci-logs`. They upload separate platform/job log artifacts with `always()` and 14-day retention. Bash uses `pipefail` so `tee` does not turn failures into passes; Windows packaging retains the packager's terminating-error behavior while writing through `Tee-Object`. Runner metadata is saved before setup. Checkout and external setup-action output remain in the raw runner logs; README documents CLI retrieval when the job page has no steps.
- Added `tools/summarize_ci.py` for always-run job summaries after a successful checkout. Summaries show step outcomes and a log-artifact link, and include at most the last 30 lines/12,000 characters of the latest command log on failure. ANSI color codes are removed from that excerpt. Summaries include artifact-upload outcomes; skipped build jobs have no runner or build logs.
- Local macOS checks pass: formatting, strict locked workspace/all-target Clippy, and all 476 workspace tests, including all group-stretch checks (`work/ci-log-diagnosis/clippy.log` and `tests.log`). Actionlint passes for the workflow and an expanded setup-action copy; ShellCheck remains unavailable. Shared/workflow Bash syntax checks pass. Isolated log checks verify that stdout/stderr are retained and an exit code of 7 remains a failure. Summary fixtures cover success, failure, skipped/cancelled outcomes, missing step data, download links, and ANSI removal. Documentation file links and `git diff --check` pass.
- The new Windows linker selection, Linux timeout budget, log artifacts, and summaries still need a hosted run. No changes were committed, pushed, or dispatched, and no user project or recording was changed.

## Independent platform build gates — 2026-10-13

- The aggregate test-matrix dependency caused the successful macOS tests in run 38467606872 to have no build job: Windows and Linux failures blocked the entire build matrix. Replaced that gate with a reusable `.github/workflows/platform.yml`, called separately for each platform by the existing entry-point matrix. Each invocation has its own `test` and `build` jobs; build depends only on its local test job. Fail-fast remains disabled across platforms. This supersedes the complete-matrix gate described above.
- macOS can now start its release build, DMG packaging, and uploads after its own tests pass, regardless of Windows/Linux results. The same rule applies to each other platform. The setup action, compiler checks, test commands, native packaging, log capture, summaries, artifact formats, and retention are retained. Caller-level concurrency still cancels older runs for the same branch; the called workflow adds no concurrency group that could cancel sibling platforms.
- Actionlint passes for the caller, called workflow, and expanded shared-setup copy; ShellCheck remains unavailable. YAML checks verify all three native runners, required platform/runner inputs, separate jobs with no inner matrix, and local build dependencies. An expanded dependency-graph check with macOS success and Windows/Linux failure confirms that only the macOS build is eligible. Workflow Bash syntax, retained log uploads/summaries, documentation links, and `git diff --check` pass.
- No Rust source or packaging scripts changed in this revision, so the preceding 476-test, Clippy, and logging checks were not repeated. The new gate still needs hosted execution; no changes were committed, pushed, or dispatched.

## Compressor effect — 2026-10-13

- Added the reusable `daw-compressor` crate and `daw.compressor` version-1 settings. The Effects catalog adds independent instances to audio/MIDI tracks. The panel has Comp (Threshold, Ratio, Peak/RMS), Response (Attack, Release, Knee), and Out (Level, Dry/Wet, gain-reduction meter) in three standard strips. Peak/RMS reuses the bounded Hz/Note knob with upper-corner labels, selected accent text, a neutral arc, and no markers/wrap. Compact mode retains the footers; header power/collapse, selection, removal, and group reordering use the existing lane behavior.
- Processing uses a linked stereo feed-forward detector, private FunDSP RMS state, a continuous soft knee, and Attack/Release time constants on gain reduction in dB. Level follows the linear Dry/Wet blend. Header off passes unscaled dry audio; modes/parameters/bypass ramp over 5 ms. Compatible edits retain detector/envelope history, meter handles, and unrelated ramps by instance ID. Stop/seek reset; pauses retain DSP state; loops/gaps keep history. The meter is read-only and publishes actual pre-blend reduction through prepared atomic handles; the UI shows zero while stopped/paused. No latency, lookahead, automatic makeup, or external sidechain is added. Defaults/bounds and exact behavior are recorded in [Compressor](../effects/compressor.md).
- Seven focused DSP tests pass across all six processing rates. They cover analytic ratio/knee behavior, linked stereo, attack/release time constants, short-pulse Peak/RMS differences, bypass/dry-wet/Level endpoints, mode/parameter transitions, reset/meter behavior, and non-finite/extreme input stability (`work/compressor/dsp.log`).
- Integration checks pass for atomic invalid-setting rejection, versioned/future payload round trips, new IDs on track copies, audio/MIDI playback/export parity, every supported buffer size, random-access rendering, chain order, compatible history/meter inheritance, and allocation-free processing/reset/retargeting. Save/open, Save As, rate changes, and float WAV exports retain settings and leave source bytes unchanged (`work/compressor/integration.log`). A device-free callback check verifies actual reduction values and meter-handle identity/removal. Four egui tests cover catalog guards, all controls/order, compact/collapse/bypass, read-only monitoring, both themes, and exact Peak/RMS painting/bounds (`work/compressor/ui-meter.log`).
- All 494 workspace tests pass, as do formatting, strict locked workspace/all-target Clippy, the CLI build, and the CLI smoke checks. The CLI also validates the temporary native-preview project containing a compressor. Logs: `work/compressor/workspace.log`, `clippy.log`, `cli-build.log`, and `cli.log`. Documentation file links, shell syntax, and `git diff --check` pass.
- Rebuilt `target/debug/DAW.app` (`work/compressor/package.log`). Verified its executable hash matches the current build, executable permission, root project license, all 37 dependency notices, and no temporary executable remains. macOS packaging now publishes the executable by atomic rename so an existing DAW process retains its mapped binary during an update. Compressor reuses the existing FunDSP dependency and notices; no external dependency version changed.
- A separate native preview used the real UI/runtime with an ignored copy of the demo project and its own persistence directory. The native screenshot confirms the three strips, control order, Peak/RMS labels/arc, and meter placement. The app-control launch stalled; raising the window completed project loading. The preview remained inactive for editing, so native parameter gestures and live audio-device metering were not established by this check. The preview was closed and confirmed stopped. The user's DAW window and projects were not modified. Windows/Linux native compressor checks and the full units performance benchmark remain unverified. No changes were committed or pushed.

## Shared stereo meter resets (2026-10-13)

- Track and Master clipping/held-peak resets now share `daw_output::StereoMeter` and `meters::reset_on_click`. Each track and Master has one latched stereo clipping flag; either channel exceeding 0 dBFS marks both channels. Clicking either Timeline/Mixer level bar or the Mixer held-peak field clears both clipping indicators and resets the held peak to −∞ for that meter. Timeline and Mixer read the same state. New audio can raise the hold and latch clipping again. Current channel levels and audio/project data are unchanged. This supersedes the earlier per-channel track flags and separate held-peak reset behavior.
- Focused output/egui checks cover clipping in either channel, both bar layouts, release-based resets, disabled/focus guards, both warning caps, unchanged live levels, read-only held-peak text, and subsequent clipping. Output checks retain meter identity across track reordering and isolate Track/Master monitoring. All 496 workspace tests pass (`CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`). Formatting, strict locked workspace/all-target Clippy, and diff checks pass. The debug macOS app was rebuilt and its bundled executable matches `target/debug/daw-desktop`. Logs: `work/stereo-meter-reset-focused.log`, `work/stereo-meter-workspace.log`, `work/stereo-meter-clippy.log`, and `work/stereo-meter-package.log`. Native meter clicks, live audio-device checks, and Windows/Linux checks were not run for this change.


## Effects library selection and insertion (2026-10-13)

- Library entries now support plain-click selection, Shift-click toggles, bounded Up/Down navigation, and anchored Shift+Up/Down range selection that grows or shrinks. Enter adds selected effects in library order, once per key press; Escape clears library selection. Keyboard handling stays with library focus and respects selected-track, editing, job/dialog, view, and drag guards. Library selection stays separate from installed-unit and track/clip selection and does not mark the project modified. Double-click still appends the addressed effect.
- Library dragging shares the installed-unit insertion line, edge scrolling, cursor feedback, and release/cancellation path in `effects_reorder.rs`. Selected library groups insert in library order with fresh IDs; an unselected entry becomes the drag selection. Empty lanes and fast move/release gestures work. Preview leaves the chain unchanged, and new instances are created only for a valid release inside the lane. Escape, focus loss, blocked editing, track/view changes, chain changes, or outside drops cancel. Dragging retains library keyboard focus so Enter works after a drop.
- All 38 Effects UI tests pass. Six added tests cover navigation/range reversal and endpoints in both themes, Shift-click toggles, Enter order/independent IDs/repeat suppression, Escape without removing installed units, input guards, multi-effect drops in expanded/compact layouts, exact shared preview geometry, cancellation, empty/fast drops, and scrolling to offscreen insertion slots. Final workspace verification passes all 502 tests, including 318 UI tests, with `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`. Formatting, strict locked workspace/all-target Clippy, and diff checks pass.
- Rebuilt `target/debug/DAW.app`; its executable hash matches the current desktop build. Logs: `work/effects-library-ui.log`, `work/effects-library-followup.log`, `work/effects-library-workspace.log`, `work/effects-library-clippy.log`, and `work/effects-library-package.log`. Native keyboard/drag gestures and Windows/Linux checks were not run for this change.


## Effects pane focus and empty catalog clicks (2026-10-13)

- Primary clicks in unused catalog space, including blank header space, clear library selection through the same operation as Escape. Empty regions claim their own pointer hits so egui's interaction radius cannot activate nearby entries or units. Effect rows and the compact/expand button retain their actions; the compact button stays enabled through an inactive drag arm so it accepts a click immediately after selecting an entry.
- Catalog and installed-unit selections now clear when their pane loses focus: clicks elsewhere, keyboard focus leaving the pane, window deactivation, dialogs/disabled editing, and view changes. `selection_focus.rs` shares pane/layer ownership checks. Controls inside the pane retain normal selection and parameter behavior. Offscreen controls remain owned by the horizontal effects lane. Active drags retain their selection; completion and Escape cancellation restore the gesture's focus. This supersedes the earlier behavior that retained unit selection across view changes or catalog clicks. Selection changes do not edit effects, audio, or project data.
- Four new egui tests and updated focus expectations cover empty-region corners/edges and release timing, blank headers, compact-button interaction, input guards, direct focus surrender, window/view changes, both directions of pane focus transfer, controls within the lane, and hover without focus changes. All 42 Effects UI tests pass. Final verification passes all 506 workspace tests, including 322 UI tests (`CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`), formatting, strict locked workspace/all-target Clippy, and diff checks.
- Rebuilt `target/debug/DAW.app`; its executable hash matches the desktop build. Logs: `work/effects-selection-focus-ui.log`, `work/effects-selection-focus-workspace.log`, `work/effects-selection-focus-clippy.log`, and `work/effects-selection-focus-package.log`. Native gestures and Windows/Linux checks were not run for this change.

## OTT unit — 2026-10-13

Implemented the accepted [OTT design](../effects/ott.md) in the reusable `daw-ott` crate and the Effects catalog/lane. Low/Mid/High each use one two-column strip, fixed soft knees, Input/Output/Depth/Attack/Release, a centered band bypass footer, and a signed gain meter. Low/High frequencies default to 250 Hz/2.5 kHz; Mid inherits their boundaries. Out reuses Level, Dry/Wet, and the bounded Peak/RMS mode knob. State uses `daw.ott` version 1 and the existing chain/presentation/edit guards.

Automated coverage:

- Nine DSP tests cover flat crossover magnitude and phase-matched Dry/Wet at all six project rates, per-band upward/downward compression in both detector modes, stereo linking, band bypass with retained gains, exact silence/output-level/global bypass, signed meters/reset, attack/release time constants, Depth, soft knees/gain bounds, history/ramp retention, ordered consecutive crossover edits, and extreme/non-finite inputs.
- Shared core tests cover complete settings round-trip, independent duplicate IDs, future-version preservation, and atomic rejection of invalid band/crossover controls. Project workflow tests cover save/open, Save As, rate changes, Float32 export versus the shared renderer, and unchanged source recordings.
- Engine checks cover live/offline parity, all supported rates and block sizes, random-access rendering, compatible history, meter reset, and zero callback allocations during processing, reset, inheritance, and retargeting. Output tests verify meter ownership by instance ID and actual callback gain changes.
- egui checks cover catalog keyboard addition, both themes, equal inner/outer column gaps, aligned controls, strip containment, band/header bypass, read-only meters, compact presentation, Depth editing, bounded detection selection, and crossover clamping. Existing selection, keyboard range, focus, and drag tests include the expanded catalog.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **522 passed**, including **324 UI tests** and doc tests. Log: `work/ott-workspace-final.log`. An earlier overlapping build/test run completed runtime tests but rustdoc reported stale `daw_engine` dependency metadata (E0463); the final sequential workspace run passed and supersedes that result.
- `cargo fmt --all --check`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: passed. Log: `work/ott-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo build --locked --offline -p daw-cli` and `python3 tools/check_cli.py`: passed WAV/MP3 codecs/bitrates, repeatability, missing-source handling, eight-track projects, overwrite, and exit-code checks. Logs: `work/ott-package.log`, `work/ott-cli.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. The app bundle executable and `target/debug/daw-desktop` have SHA-256 `36b75b6834390f2a4a9b467b8a1cb1e08ce8c0de09e9852753a6b544860d51bd`.
- Documentation file links and `git diff HEAD --check`: passed. Existing FunDSP dependency/license notices are retained; no external dependency version changed.

Native mouse/keyboard gestures, listening/device behavior, CPU performance, and Windows/Linux builds were not verified in this task. Automated checks do not establish those results. The existing user DAW process and projects were not restarted or modified.

## Delete selected effect units — 2026-10-13

With Effects lane focus, Delete or Backspace removes all selected installed effects through the existing single `Edit::SetEffects` transaction. Surviving units retain order, IDs, settings, and compact state; track/clip content and selection remain unchanged. Removal clears effect selection and retains a keyboard-only pane focus target. Held-key repeats are consumed so they cannot fall through to overview clip deletion. Shift may remain held after group selection; Ctrl/Command/Alt combinations are ignored. Other panes/text editing, window loss, unavailable/disabled editing, modal work, and pointer/drag gestures remain guarded.

Two egui tests cover single, separated multiple, and complete-chain selections; Delete and Backspace; Shift-held groups; compact/collapsed combinations; survivor/clip preservation; held keys after the focused unit disappears; and focus, repeat, modifier, text, modal, disabled, view, and gesture guards. Existing Effects regression checks retain body/header/power/knob selection and drag behavior. An initial regression from querying provisional unit focus on ordinary click frames was corrected by limiting shortcut focus queries to deletion events. All affected checks pass on the final implementation.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui effects --lib`: **47 passed**, including the two new tests. Log: `work/effect-delete-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **524 passed**, including **326 UI tests** and doc tests. Log: `work/effect-delete-workspace.log`.
- `cargo fmt --all --check` and `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: passed. Log: `work/effect-delete-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/effect-delete-package.log`. The app bundle executable matches `target/debug/daw-desktop`, SHA-256 `e728153e87664381c54783a540df70c0336f8083be65970ef27d723ad4d10f44`.
- `git diff --check`: passed. CLI smoke checks were not repeated because this change is UI-only and does not change shared rendering/import/export.

Native keyboard gestures and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Timeline catalog and effects lane — 2026-10-14

Timeline now shows the shared Effects catalog to the left of Tracks and a full-width effects lane below its multi-track timeline. The grid ends at its scrollbar, directly above the lane; Master remains pinned above the lane, and the status panel supplies the single bottom separator. The existing Effects view and selected-track overview remain available. Both views edit the same track-owned chain and saved presentation settings through shared catalog/lane composition.

Three added egui tests cover panel/scrollbar/grid alignment in both themes and compact states, one status separator, read-only layout, same-frame chain switching after a track click, presentation shared between Timeline and Effects, library group drag into an empty lane, chain reordering, and group Delete without clip changes. Existing Timeline input tests now derive track/header/file-drop coordinates from the shifted viewport and use enough viewport space for their scrolling/group-placement scenarios. Media-drop tests also reject the catalog and effects lane. Effect gestures retain their originating view and cancel if it changes; Timeline controls remain disabled during an effect drag. Existing Effects, clip, track, focus, keyboard, waveform, and ruler regression checks pass.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **329 passed**. Log: `work/timeline-effects-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **527 passed**, including doc tests. Log: `work/timeline-effects-workspace.log`.
- `cargo fmt --all --check` and `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --locked --offline --workspace --all-targets -- -D warnings`: passed. Log: `work/timeline-effects-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/timeline-effects-package.log`. The app bundle executable matches `target/debug/daw-desktop`, SHA-256 `c3d49e3ed838a388775fa3a6fa472711809eb5790607ae8c4130990cfac7d6fc`.
- Documentation file links and `git diff --check`: passed. CLI smoke checks were not repeated because this change affects UI composition only; shared audio, persistence, and import/export implementations are unchanged.

Native layout/gesture checks and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Timeline Library toolbar — 2026-10-14

Timeline now has a 38-point left toolbar spanning playback to status beside both the catalog and effects lane. Its centered borderless Library button is 24×24 points, with a seven-point inset and the existing 16-point icon. The button toggles catalog visibility in UI state without changing project content or dirty state. It uses the active-view background when on, transparent fill when off, and the same neutral translucent hover overlay in both themes. The effects lane fills the width to the right of the rail; hiding the catalog expands the timeline viewport. The separate Effects view keeps its catalog.

Four added egui tests cover geometry, expanded/compact layouts, both themes, on/off/hover painting, selection cleanup, project preservation, hidden-catalog keyboard guards, retained lane actions and Effects view, and disabled/modal/active-drag guards. The existing clip trim test uses a lower zoom so its target edge remains visible in the narrower viewport.

Completed checks on the final 24×24 button:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **333 passed**. Log: `work/timeline-toolbar-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **531 passed**, including doc tests. Log: `work/timeline-toolbar-workspace.log`.
- `cargo fmt --all --check` and strict locked offline workspace/all-target Clippy: passed. Log: `work/timeline-toolbar-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/timeline-toolbar-package.log`. The bundle executable matches `target/debug/daw-desktop`, SHA-256 `b0e7dad7dc3911b9faaf5610542ecc931dcb863f0460cfa6ac524168cb45b1d5`.
- Documentation file links and `git diff --check`: passed. CLI smoke checks were not repeated for this UI-only change.

Native layout/gesture checks and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Timeline lane toggle and shared Tracks divider — 2026-10-14

Added a bottom-aligned borderless Lucide Gallery Horizontal toggle to the Timeline toolbar. It uses the same 24×24-point button, 16-point icon, seven-point inset, selected/off/hover styles, and input guards as Library. Lane visibility starts on, remains independent UI state across view changes, and does not alter installed effects, processing, presentation settings, or project dirty state. Hiding the lane returns its height to Timeline and the visible catalog, clears unit selection and pending gestures, and removes its keyboard removal/drop target. Catalog addition remains available. Effects view always shows both panes.

Removed redundant track-row and Master left borders. Their adjacent catalog or toolbar owns one continuous 1-point divider, with no gap. Track selection outlines, horizontal row separators, and Master's top separator remain.

Three new egui tests cover bottom anchoring and resizing, catalog/lane combinations, expanded/compact presentation, project preservation, hidden-lane selection/removal/drop behavior, catalog Enter addition, restored units, and separate Effects view. Divider checks cover both themes, 1×/1.5×/2× scales, pane visibility combinations, and background alignment. Existing toggle style and disabled/modal/active-drag tests now exercise both buttons.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **336 passed**. Log: `work/timeline-lane-toggle-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **534 passed**, including doc tests. Log: `work/timeline-lane-toggle-workspace.log`.
- Formatting, strict locked offline workspace/all-target Clippy, documentation links, and `git diff --check`: passed. Log: `work/timeline-lane-toggle-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/timeline-lane-toggle-package.log`. The bundle executable matches `target/debug/daw-desktop`, SHA-256 `04c648bb07816fbbdf5e89d8aed3b74c55e1503cb7fe1cb8629ca6df818bbbe4`.

CLI smoke checks were not repeated for these UI-only changes. Native layout/gesture checks and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Transport buttons and first-unit divider — 2026-10-14

Play/Pause, Stop, and Loop now share the Timeline toggles' borderless 24×24-point design with centered 16×16-point icons. Playback uses seven-point padding and horizontal gaps, aligning Play with Library while retaining the 38-point toolbar height. The Play icon remains unchanged while playing; its tooltip/accessibility name still changes to Pause, and its action still pauses. Active Play and enabled Loop use Green 500 and Yellow 500 blended into the current theme panel at 18%, respectively. View controls and semantic transport fills share `theme::toggle_fill`; the configured accent remains independent. Transparent inactive fills, the shared gray hover overlay, contrasting active icons, input guards, Stop, keyboard activation, and empty-loop rejection remain.

The first Timeline effect now shares the vertical toolbar's divider. A scoped shared-left-border tag tells `PanelStyle::unit_in_lane` to omit that duplicate stroke without changing unit geometry, other borders, controls, or selection outlines. The standalone Effects view retains its first-unit border.

Four new egui tests cover both themes, independent playing/loop states, matching semantic tint through the standard accent-toggle style, painted 16×16 icon bounds, button size/gaps/padding and toolbar height, hover/border styling, pointer and keyboard actions, disabled controls, empty-loop validation, dirty-state behavior, Play/Library alignment, and first-unit border ownership in expanded/compact/collapsed presentations. The existing UI regression suite passes on the final muted-color implementation.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **340 passed**. Log: `work/transport-buttons-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **538 passed**, including doc tests. Log: `work/transport-buttons-workspace.log`.
- Formatting, strict locked offline workspace/all-target Clippy, documentation file links, and `git diff HEAD --check`: passed. Log: `work/transport-buttons-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/transport-buttons-package.log`. The app bundle executable matches `target/debug/daw-desktop`, SHA-256 `e1f1baad898ea1925de4c9822f7aea363f28e6203b553ce1578476775424f1e8`.

CLI smoke checks were not repeated for these UI-only changes. Native appearance/gesture checks and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Stronger Loop yellow and shared view hover — 2026-10-14

Loop now blends Yellow 500 into the current panel at 35%, producing a stronger muted yellow in both themes. Play retains its 18% green tint. Existing transport paint checks verify the final dark/light Loop colors, independent of the configured accent, along with the retained geometry, icon bounds, hover, and input behavior.

View mode buttons now use the same shared `theme::TOOLBAR_HOVER_FILL` as borderless toolbar buttons for hover/press. Their selected accent background remains below the overlay. Backgrounds paint before labels, preserving text contrast, dimensions, corner radius, borderless framing, and view-switch guards. Disabled buttons do not draw the overlay. The existing titlebar/pointer-state regression test now checks exact shared hover fill, selected/unselected states, paint order, and hover exit across dark/light themes, two accents, all view modes, and both titlebar layouts.

Completed checks on the final implementation:

- Focused transport and view-style checks pass. Logs: `work/loop-yellow-focused.log` and `work/view-hover-focused.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **340 passed**. Log: `work/view-hover-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **538 passed**, including doc tests. Log: `work/view-hover-workspace.log`.
- Formatting, strict locked offline workspace/all-target Clippy, documentation file links, and `git diff HEAD --check`: passed. Log: `work/view-hover-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/view-hover-package.log`. The bundle executable matches `target/debug/daw-desktop`, SHA-256 `35e480ed0cdf4f32f5a07002e290dc18d40ddc5b4332abe86c12a89ae210cae6`.

CLI smoke checks were not repeated for these UI-only changes. Native appearance/gesture checks and Windows/Linux builds were not verified. The running user DAW and project files were not restarted or changed.

## Timeline-only effects and transport colors — 2026-10-14

Removed the dedicated Effects view from the view enum, buttons, menus, and workspace dispatch. Timeline retains the catalog/lane, independent visibility toggles, track-owned chains, unit settings, selection, keyboard actions, group removal/reorder, drag previews, and focus/gesture guards. Editor retains its selected-track overview. Three tests for the retired Effects overview were removed; surviving effect interaction tests now exercise Timeline and real view changes. The view/menu test verifies the four remaining views.

Active transport backgrounds now use Play `#338555`, Stop `#C54E58` during playback, and enabled Loop `#AF8C3E`. Loop's final gold is slightly lighter and softer than the supplied swatch. All three active icons use the standard toolbar-toggle foreground. The transport tests inspect painted backgrounds and actual SVG texture pixels in both themes while retaining size, spacing, hover, input, and disabled-state checks. This supersedes the earlier blended green/yellow fills and separate Effects workspace placement.

Completed checks on the final colors and view composition:

- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/remove-effects-view-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **337 passed**. Log: `work/remove-effects-view-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **535 passed**, including doc tests. Log: `work/remove-effects-view-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/remove-effects-view-package.log`. The bundle executable matches `target/debug/daw-desktop`, SHA-256 `c8148c7aa6e8f71558766a30d1c5a2fcd6fa1ebfb68ccbd3fdad0e57fbf9c112`.
- Documentation file links and `git diff HEAD --check`: passed.

CLI smoke checks were not repeated for these UI-only changes. Native appearance/gesture checks and Windows/Linux builds were not verified. The running DAW and user project files were not restarted or changed.

## Shared loop-range colors — 2026-10-14

Timeline and Editor now share semantic loop-range colors through `theme::ruler_loop_colors`. Enabled handles match the Loop button's `theme::LOOP_COLOR` (`#AF8C3E`). Following the requested refinement from dimmer to lighter, the enabled bar uses opaque `#C3A96E`, blending the handle gold 25% toward white, with contrasting selected bar-number text. When looping is disabled, the bar uses the shared toolbar hover fill and the handles use the inactive, unhovered toolbar foreground. Accent changes do not alter these colors. Handle geometry, clipping, and loop/seek interactions remain unchanged.

Updated existing transport paint checks to cover both handles, the range fill, and selected label contrast in both themes. Existing ruler interaction checks passed, including shared Timeline/Editor behavior; the accent-settings test now verifies semantic loop colors remain independent.

Completed checks on the final lighter background:

- Focused transport paint and ruler interaction checks: passed.
- `cargo fmt --all --check` and strict locked offline workspace/all-target Clippy: passed. Log: `work/loop-range-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **535 passed**, including **337 UI tests** and doc tests. Log: `work/loop-range-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/loop-range-package.log`. The bundle executable matches `target/debug/daw-desktop`, SHA-256 `7ef490f405258a0f7a8a31a98e32951705e5c83bb70b8a0d4b858489b342945c`.
- Documentation file links and `git diff HEAD --check`: passed.

CLI smoke checks were not repeated for this UI-only change. Native appearance and Windows/Linux builds were not verified. The running DAW and user project files were not restarted or changed.

## Loop background shade refinement — 2026-10-14

The latest supplied gold swatch resolves to dominant sRGB `#C09C2E` after its embedded profile is converted to sRGB. The enabled loop-range background now uses `#D0AC3C`: HSL lightness raised by six percentage points, with hue and saturation retained. This supersedes the previous `#C3A96E` white blend. Handles, Loop button, contrasting selected ruler labels, disabled colors, and ruler interactions retain their existing behavior.

Completed checks:

- Existing transport paint/contrast check in both themes: passed. Log: `work/loop-gold-focused.log`.
- `cargo fmt --all --check` and strict locked offline workspace/all-target Clippy: passed. Log: `work/loop-gold-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **535 passed**, including doc tests. Log: `work/loop-gold-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/loop-gold-package.log`. Bundle and build executables share SHA-256 `886cbcb2c79207891b02659fa09230aa266645f6500568fc4046e1b3bb18fb74`.
- Documentation links and `git diff HEAD --check`: passed.

Native appearance and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only color change. The running DAW and user projects were not restarted or changed.

## Click loop range to toggle — 2026-10-14

A primary click on the existing loop-range body or either handle now toggles looping on release in Timeline and Editor. The ruler and toolbar share `toggle_looping`, preserving nonempty-range validation and dirty/playback synchronization. The range bounds and playhead stay unchanged. The ruler tooltip now describes click-to-toggle. Real drags, including those with unchanged bounds or no horizontal motion, retain their existing behavior and do not toggle looping. Empty upper-band clicks remain inactive.

One new input test covers both views with horizontal scrolling: handle/body clicks, small pointer movement, activation on release, both toggle directions, endpoint/playhead preservation, dirty state, empty-space clicks, vertical drags without range movement, and a background job blocking activation between press and release. Existing ruler tests cover creation, movement, resizing, lower-band seeking, and interrupted drag restoration.

Completed checks:

- Focused click test and nine ruler-related checks: passed. Final click log: `work/loop-bar-click-focused.log`.
- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/loop-bar-click-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **536 passed**, including **338 UI tests** and doc tests. Log: `work/loop-bar-click-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/loop-bar-click-package.log`. Bundle and build executables share SHA-256 `73d5d9f6bc325674a0ef61bfe6fa4e95bf5cf3253baf088de0b63455a2c42cfe`.
- Documentation links and `git diff --check`: passed.

Native mouse/trackpad behavior and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only change. The running DAW and user projects were not restarted or changed.

## White active transport icons in light mode — 2026-10-14

Play, Stop, and Loop now use white icons on their active colored backgrounds in both themes. This supersedes the theme-dependent active transport foreground, which produced dark icons in light mode. Inactive and disabled colors, hover overlays, fills, sizes, and actions retain their behavior. The existing paint test checks actual SVG pixel colors against white for all three active icons in both themes.

Completed checks:

- Focused transport paint check: passed. Log: `work/light-transport-icons-focused.log`.
- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/light-transport-icons-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **536 passed**, including doc tests. Log: `work/light-transport-icons-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/light-transport-icons-package.log`. Bundle and build executables share SHA-256 `f8613525d210adeff6e991b74fdb9358e461af38f4bfc0cbad3ae3af4dca0149`.
- Documentation links and `git diff --check`: passed.

Native appearance and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only change. The running DAW and user projects were not restarted or changed.

## Independent Master effects lane — 2026-10-14

Master now owns saved effects and compact presentation with legacy empty/false defaults. Select Master in Timeline or Mixer to direct Timeline's shared effects lane to its independent chain; selecting a track restores that track's chain. Master selection uses the shared outline, clears track/catalog/unit selection and effect gestures, retains clips, and does not modify project content. Catalog keyboard add/drop, unit settings, compact/collapse presentation, group reorder/Delete, focus guards, and visibility use the existing handlers with a typed track/Master target. Master can receive effects without any tracks.

Core commits Master edits atomically through `Edit::SetMasterEffects`; `Project::all_effects` includes every owner for global ID validation and meter discovery. Shared `EffectChain` processing applies Master effects after summed post-fader tracks and before Master gain/output meters in live playback, export, and direct render helpers. Prepared updates retain compatible histories and meter handles without callback allocation. Stop/seek reset Master effects, settled pause preserves them, and loops retain history. Unknown Master effects bypass with retained serialized state and a load warning.

Added checks cover legacy migration, save round trips, invalid settings/duplicate ID rejection without partial edits, stereo mix placement and live/export/random-access agreement at all six project rates, allocation-free state inheritance, reset and actual compressor/OTT monitoring, save/save-as/rate changes/float export, and unknown-unit warnings/bypass. UI checks cover Master selection and chain switching in the same frame, clip preservation, independent compact state, Mixer selection, project closure, and addition to an empty project. Existing catalog drag/reorder/group-Delete and callback meter tests now exercise both track and Master owners.

Completed checks:

- Focused core, engine, UI, output meter, and project integration checks: passed.
- `cargo fmt --all --check` and strict locked offline workspace/all-target Clippy: passed. Log: `work/master-effects-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **541 passed**, including **339 UI tests** and doc tests. Log: `work/master-effects-workspace.log`.
- Locked offline CLI build and `python3 tools/check_cli.py`: passed. Logs: `work/master-effects-cli-build.log` and `work/master-effects-cli.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/master-effects-package.log`. Bundle and build executables share SHA-256 `8335369df07a46cd2bc71798f53ff1a5c32a9101899809608bb11f3088d36818`.
- Documentation links and `git diff --check`: passed.

Native selection/drag behavior, device playback, and Windows/Linux builds were not verified. The running DAW and user projects were not restarted or changed. Export still uses the arranged duration; automatic tails and beat-lane routing remain pending.

## Numbered view shortcuts and menubar bindings — 2026-10-14

Added Command+1/2/3/4 on macOS and Ctrl+1/2/3/4 on Windows/Linux for Timeline, Instrument, Editor, and Mixer. `ViewMode::shortcut` supplies both keyboard routing and menu labels. The egui View menu retains its active-view checkmark and displays shortcut text with the shared platform/font helper. The native macOS View menu registers matching Command key equivalents. Keyboard routing uses the existing view-switch guard and draft cleanup, ignores extra modifiers and repeated presses, and protects focus and active pointer gestures. The native handler owns its key equivalents and removes forwarded duplicate presses.

One new input test covers the mapping, all views on Mac/Windows/Linux contexts, project/clip/track/viewport preservation, exact modifiers, held-key repeat suppression, modal/drag guards, draft cancellation, and native-menu ownership. Existing menu label/font checks now include View shortcuts in both themes on all three OS contexts; checkmark/menu switching checks also pass.

Completed checks:

- Focused numbered-shortcut test and 17 menu-related checks: passed.
- `cargo fmt --all --check` and strict locked offline workspace/all-target Clippy: passed. Log: `work/view-shortcuts-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **542 passed**, including **340 UI tests** and doc tests. Log: `work/view-shortcuts-workspace.log`.
- After the final native keyboard focus/pointer guard refinement, strict Clippy and locked offline desktop tests passed. Desktop test log: `work/view-shortcuts-desktop.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed on the final source. Log: `work/view-shortcuts-package.log`. Bundle and build executables share SHA-256 `695fe03c577d59fea93b32cd3a2ab73cd0833740f1d4b3a93607388eb1a2ee70`.
- Documentation links and `git diff --check`: passed.

Native menubar/key-equivalent interaction and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only change. The running DAW and user projects were not restarted or changed.

## Instrument effects toolbar and shared workspace — 2026-10-14

Instrument now uses Timeline's shared effects workspace and left toolbar. The 38-point rail spans playback to status, with the same 24×24 Library/Gallery Horizontal buttons, 16-point icons, styles, and guards. The catalog sits left of Layers, and the lane sits below the layer workspace. Independent catalog/lane visibility is shared UI state across Timeline and Instrument; hiding either returns its space without project edits. Instrument targets only its selected MIDI track's existing saved chain. Layer/source/envelope composition, the MIDI selector, processing, and saved schema retain their behavior.

Catalog keyboard/multiple selection, drag/drop previews, unit controls, compact/collapse settings, group reorder/Delete, focus cleanup, and cancellation reuse existing handlers. Instrument clears stale Timeline lane geometry and file-drop targets. The shared workspace owns the single top divider; its instrument body no longer adds a duplicate divider or gap.

One new UI test covers both themes, toolbar/button dimensions, catalog/Layers/lane adjacency, independent hiding/restoring, shared visibility across views, project preservation, and rejecting audio targets. Existing catalog drag/reorder/group-Delete coverage now also exercises Instrument with a MIDI track. Three layer tests were adapted to the new optional-pane layout: provide enough height for background geometry, hide the lane for full three-layer drag targets, and exclude toolbar-button fills when locating a unit header. Layer interaction and divider checks pass.

Completed checks:

- Focused Instrument geometry, shared effects interactions, layer reordering/layout, and header/divider checks: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **341 passed**. Log: `work/instrument-effects-ui.log`.
- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/instrument-effects-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **543 passed**, including doc tests. Log: `work/instrument-effects-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/instrument-effects-package.log`. Bundle and build executables share SHA-256 `9a298a1683519d882455aed0d491ae7274bc287925b1578f17eddd7987ce02f6`.
- Documentation links and `git diff --check`: passed.

Native layout/gesture behavior and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only composition change. The running DAW and user projects were not restarted or changed.

## Standard scrollbar palette — 2026-10-14

The shared theme now makes every scrollbar use the surface-color palette already used by Timeline. This includes the effects lane, Instrument layer lanes, lists, and dialogs in both themes. Scrollbar thumbs use themed widget background colors instead of foreground colors, removing the bright foreground hover color. Existing track colors, scrollbar dimensions, floating/fading behavior, and interactions remain in place.

Completed checks on the final source:

- `cargo fmt --all --check`: passed.
- Strict locked offline workspace/all-target Clippy: passed. Log: `work/effects-scrollbar-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **543 passed**, including **341 UI tests** and doc tests. Log: `work/effects-scrollbar-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/effects-scrollbar-package.log`. Bundle and build executables share SHA-256 `744b5fb94456bba578561a6eb634d798c33e1a0362ed66878b4bd1d38c0af4cd`.
- Documentation links and `git diff --check`: passed.

Native scrollbar appearance and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only style change. The running DAW and user projects were not restarted or changed.

## Pinned effects-lane toolbar — 2026-10-14

Timeline and Instrument now reserve a 38-point toolbar at the effects lane's right edge, outside horizontal unit scrolling and drag/drop targets. Its borderless 24×24 button uses a 16-point Arrow Up From Line icon to expand the saved chain layout and Arrow Down From Line to collapse it. The catalog header is title-only; its former compact button was removed. The lane control remains available with the catalog hidden, uses the existing guarded effects edit for tracks and Master, and hides with the lane. Tooltips, accessibility, theme colors, pane focus, release-only commits, and unit settings/selection remain intact.

One new egui interaction test covers both themes, Timeline track/Master and Instrument targets, fixed toolbar/button geometry while scrolling, release-only changes, preserved effects and lane selection, catalog visibility, and modal blocking. Existing compact-layout tests now use the pinned button. Catalog empty-header checks pass after removing the header action.

Completed checks on the final source:

- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/effects-toolbar-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline -p daw-ui --lib`: **342 passed**. Log: `work/effects-toolbar-ui.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **544 passed**, including doc tests. Log: `work/effects-toolbar-workspace.log`.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/effects-toolbar-package.log`. Bundle and build executables share SHA-256 `74c9eeb691e10decf6be62bd3218d4ada31c86beaea235da952b6cf6384b8514`.
- Documentation links and `git diff --check`: passed.

Native appearance/gestures and Windows/Linux builds were not verified. CLI smoke checks were not repeated for this UI-only change. The running DAW and user projects were not restarted or changed.

## Limiter and compensated lookahead — 2026-10-14

Added the portable `daw-limiter` crate, version-1 settings/validation and future-state preservation, engine/track/Master processing, shared meter handles, catalog entry, and two-strip UI. Limit contains Input/Release/Lookahead; Out contains Ceiling, Peak/True Peak, and L/R/M/S. The thin gain-reduction rail, two-way knobs, enable/collapse, compact mode, and catalog/lane actions reuse existing controls. True Peak uses an eight-phase/64-tap reconstruction guard with a 32-frame block and 0.175 dB estimation headroom; it does not claim ITU certification.

Fixed instance latency includes the maximum 10 ms delay and reconstruction block even when bypassed. Prepared alignment delays, source read-ahead/preroll, serial Master accounting, and loop clock correction preserve arranged/export frame positions. Seeks, Stop, Pause/resume, and compatible plan replacement use the shared path. Zero-latency track additions retain the previous live pitch phase; a regression found during review was fixed before the final checks. See [Limiter](../effects/limiter.md) for processing and reconstruction limits.

Completed checks on the final source:

- Eight limiter DSP tests cover supported rates, finite/extreme input, linked sample ceilings, independent M/S behavior, bypass delay/reset, lookahead, release, quiet-signal transparency, retargeting, and an independent 16×/128-tap true-peak reconstruction of high-frequency stereo bursts.
- Engine checks cover unequal serial track/Master delays, byte-identical bypass alignment, exact live/export output, seeks, loop/MIDI timing, frame-limit arithmetic, retained live pitch phase, meters, and allocation-free processing/reset/retarget/topology changes. Model and project checks cover atomic invalid edits, future data, duplication, Save/Open/Save As, rate changes, and exact float export frame count/ceiling.
- Formatting and strict locked offline workspace/all-target Clippy: passed. Log: `work/limiter-clippy.log`.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --locked --offline --workspace`: **561 passed**, including **343 UI tests** and doc tests. Log: `work/limiter-workspace.log`.
- Locked offline CLI build and `python3 tools/check_cli.py`: passed. Logs: `work/limiter-cli-build.log`, `work/limiter-cli.log`.
- Additional temporary-project CLI validate/render check: passed with True Peak, M/S, −6 dB ceiling, and exactly 4800 stereo frames. Log: `work/limiter-cli-unit.log`. User sources and tracked fixtures were not changed.
- `CARGO_HOME="$PWD/work/cargo-home" CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug`: passed. Log: `work/limiter-package.log`. Bundle and build executables share SHA-256 `958697f248af45e9a5a024a971d3933e6486c9443cafde5a9adc6e9be3e337fd`.
- Documentation file links and `git diff --check`: passed.

Native appearance/gestures, listening quality, callback CPU/device stability, and Windows/Linux builds were not verified. The running DAW and user projects were not restarted or changed. Reconstruction checks establish the tested signals/rates; they do not establish a bound for every possible analog reconstruction.

## Experimental Timestretch engine and comparison tool — 2026-10-14

Implemented an independent BSD-2-Clause `crates/timestretch` library, a DAW-independent `crates/timestretch-cli` tool, Python comparison/benchmark wrappers, and the saved Project Settings engine selector. The two labels are **Rubber Band** and **Integrated (experimental)**. Rubber Band R3 remains the default for new and legacy projects. R2 is a standalone comparison reference.

The engine uses a linked stereo phase vocoder with shared peak phase rotations and source-wide spectral-flatness window selection. Original implementation provenance, mathematical basis, prior API/adapter exposure, dependency licenses, and the mandatory no-Rubber-Band-code rule are recorded in [PROVENANCE.md](../../crates/timestretch/PROVENANCE.md). No Rubber Band implementation source was consulted or copied for this DSP. The core dependency graph contains no DAW or Rubber Band dependency. The tool's separately licensed reference adapters call the public R2/R3 API.

Settings changes prepare complete buffers/peaks on a worker and publish them together. Failure or Cancel retains the old session. A cancelled receiver cannot accept late results. Cache entries retain their source allocation and check source identity, sample rate, reduced ratio, engine, and processing revision. Rendering and waveform lookup reject mismatched entries. Source allocations stay immutable while cached; replacing one invalidates its prepared audio.

### Completed focused checks

| Area / acceptance IDs | Evidence and scope |
| --- | --- |
| IP-01/IP-02 | Reviewed independent source/provenance and resolved dependency graphs. Extracted core/tool packages built outside the workspace; core consumer and reference-disabled tool tests passed. This is not a formal clean-room or legal-clearance certification. |
| TS-01/02/03/04/11/14/16 | Typed request errors, rational rounding/overflow, exact identity bits including signed zero, constant/silent inputs, deterministic repeat calls, finite output/headroom, configured memory-limit rejection, and a minimal external consumer passed. Release short-layout matrix covers 6 rates × 11 ratios × 14 lengths × 2 channel counts (1,848 cases), using nonconstant asymmetric inputs. OS allocator-failure injection and exhaustive platform determinism remain unverified. |
| TS-05/08 | Release tone matrix covers 6 rates × 11 ratios × 5 frequencies (330 cases). Pitch/level limits passed. This does not establish harmonic-stack or arbitrary-music quality. |
| TS-06/07/08 | Mono equivalence, dual mono, anti-phase, proportional/silent channels, channel swap, phase fixtures, and amplitude homogeneity passed at the tested 48 kHz ratios. Full cross-product stereo coverage remains open. |
| TS-09/10/12 | Independent 48 kHz burst onset/energy-width tests, start/end impulse probes, and a short tonal burst passed at the tested normal/extreme ratios. Mixed-material transient listening and exhaustive boundary/rate coverage remain open. |
| TS-13 | Immediate and synthesis cancellation plus monotonic progress passed; bounded checkpoints were inspected. The tool's timeout/cancellation supervision was exercised separately. A designated-host 250 ms latency distribution has not been measured. |
| TS-15 / TT-06 | Independent tool analyzers reject deliberate octave shifts, doubled gain, channel reversal/collapse, missing output frames, silence, NaN, and delayed events. Versioned fixture metadata must match prepared SHA-256 input. Unsupported/unprovided measurements are not reported as passes. |
| TI-01/02/03/04/05/08 | Saved/legacy/unknown identity checks, A→B→A rebuilds, source-allocation invalidation, deterministic save/reopen rendering, cancellation/failure retention, selector draft/cancel/apply/no-op, and row alignment in both themes passed. Native device playback and full edit-scenario parity remain separate checks. |
| TT-01/02/03/04/05/08/09/10 | Three-engine WAV/JSON comparison, native R2/R3 identity, shared prepared hashes, decimal/fraction equivalence, invalid options, existing destinations, literal metacharacter paths, hard-link/symlink source protection, optional-reference failures, and extracted package execution passed. Windows file identity/native execution remains unverified. |
| TT-07 | Benchmark smoke ran all three engines serially at 48 kHz and ratios 1/2, 3/2, and 8, with one warmup and five measured runs. JSON/CSV summaries completed without failures. A known timing sequence verified median/min/max/MAD aggregation. These short synthetic cases do not establish the full performance gate. |

`cargo test -p timestretch --release -- --include-ignored` passed all nine test groups, including both expanded matrices. Standard `cargo test` leaves the two expanded matrices explicitly ignored for separate release-mode execution. The initial full workspace run found a test setup error: the new selection-preservation test expected a track selection it had not established. The test now selects a track before applying settings; focused Project Settings tests passed afterward.

Additional local comparisons used audio from the user's **local Splice library**, read-only. All three engines completed 12 file/ratio cases with matching prepared-input hashes. Source hashes remained unchanged. Outputs and reports stay in ignored scratch storage; no recordings or per-file inventory were added to the repository. This is a processing smoke test, not a listening panel or permission to redistribute the material.

The final `cargo test --workspace` run passed (the two expanded engine matrices remain explicitly ignored there and passed separately in release mode). Strict workspace Clippy, formatting, documentation links, and whitespace checks passed. Later CLI signal-supervision changes passed focused tool tests. After replacing the full-length normalization buffer with a periodic hop-sized table, the expanded engine suite, project workflows, Settings tests, tool tests, strict Clippy, and macOS rebuild passed again. The DAW CLI WAV/MP3 smoke checks passed. The macOS debug app bundle was rebuilt at `target/debug/DAW.app`. No native app/device audition or Windows/Linux run was performed.

### Remaining acceptance limits

This is an initial experimental implementation, not full acceptance or Rubber Band equivalence. Keep the existing numeric targets. Mixed tonal/percussive material uses one window-size decision for the complete source and needs listening review. Peak process RSS is reported as unavailable; engine scratch accounting, the full 60-second corpus benchmark, designated-host latency measurements, three-listener evaluation, exhaustive cross-product checks, and native checks on all supported platforms remain open. Sample comparisons do not waive any failed or untested gate. Rubber Band remains the default.


## Timestretch: level and transient validation — 2026-10-15

The engine carries the phase rotation actually used by peak locking into the next frame, maintaining phase continuity when spectral peaks change. Expansion protects detected attacks at their original local speed and phase, anchored to the requested onset time; intervening regions provide the extra duration. Short broadband guards retain more decay. Occupied-band flatness prevents a band-limited cymbal/noise spectrum from selecting long windows solely because frequency margins are empty. Compression retains its linear time map. No engine gain matching, normalization, clipping, or limiter was added. See the [independent design record](../../crates/timestretch/PROVENANCE.md#phase-continuity-and-attack-preservation) for sources, thresholds, and limits. No Rubber Band implementation material was read or reused.

Completed checks:

- Final release DSP contract suite passed, including the six-rate/eleven-ratio pitch and short-layout matrices. Five new regression groups passed: moving-peak noise level retention and amplitude scaling; kick/noise/mixed-material attack shape and pre-echo; transient stereo relationships and cancellation through analysis/synthesis/draining; dense/off-grid/boundary impulses at all rates; and decay energy on short band-limited noise. Generated kick/mixed attacks retain their initial 10 ms within the stated residual threshold; the narrower broadband attack test checks its first 0.5 ms. These synthetic checks do not establish subjective cymbal quality.
- On generated stationary noise, level loss was approximately 2.0 dB at half duration and 0.6–0.7 dB at 1.5–2 times duration. Residual noise level error remains; no global gain correction conceals it.
- Read-only local Splice comparisons completed 24 file/ratio cases, with Integrated, R2, and R3. All had matching prepared inputs, exact output lengths, and finite samples. Source hashes remained unchanged.
- Across the original four local-library sources, Integrated RMS differed from R3 by -0.47 to +0.14 dB across half, 1.5, 2, and 8 times duration. Additional short percussion ranged from -0.27 to +2.16 dB relative to R3. RMS is a level diagnostic, not a quality score; protecting attacks can change average level as spacing increases.
- Twelve listening sets include separate copies of Integrated, R2, and R3. FFmpeg measured integrated LUFS with one second of appended silence for measurement only, including short sources. One disclosed constant gain per copy matches a shared target while retaining headroom. The verified copies stayed within 0.05 LU of their target and at or below -3.53 dBTP. Raw outputs remain untouched. The HTML audition page and JSON gain/measurement reports remain in ignored scratch storage, with generic case IDs and no source inventory in project documentation.
- Strict workspace Clippy, formatting, whitespace checks, reference-disabled tool tests, processing identity checks, project/cache checks, and Settings Apply checks passed. An initial workspace tool test was invalidated by concurrently building the reference-disabled executable at the same target path; the final serialized `cargo test --workspace` run passed with references enabled. The two expanded matrices remain ignored in that command and passed separately in release mode. The final DSP changes also passed the expanded release tests separately. DAW CLI WAV/MP3 smoke checks passed, and the macOS debug bundle was rebuilt. Documentation link targets were checked.

Sample timings were collected during development with other build/test activity and are not a controlled performance comparison. This is an experimental refinement, not R3 equivalence. Masked attacks, dense-event guard limits, source-wide window choice, sustained cymbal tails, and transient/level tradeoffs still need listening evaluation. The new listening copies were measured but not auditioned by the agent. Native UI/device playback, Windows/Linux validation, the full performance corpus, and formal listening acceptance remain open. Rubber Band remains the default.

## Timestretch: single current implementation — 2026-10-15

The library and comparison tool use one Integrated implementation. Requests contain only audio parameters and resource limits. Project loading accepts the current identity for each engine and rejects unsupported identities without substitution. Cache keys and reports retain processing identity; the Settings selector remains Rubber Band / Integrated (experimental). The spectral and transient algorithms are unchanged.

The user's informal listening feedback favors Integrated's fuller body on kicks, cymbals, and some synths, while R3 retains stronger attacks on fast or syncopated percussion. This is a preference report, not formal listening acceptance. Preserve this balance in future changes.

Completed checks:

- `cargo test -p timestretch -p timestretch-cli -p daw-core --release` passed. This covers current processing, absolute noise/attack/decay limits, reference comparisons, report identity, and rejection of unsupported saved identities. The two expanded DSP matrices remain separate release checks; they were not repeated for this cleanup.
- Rebuilt the release comparison tool. A generated stereo tone/noise fixture produced byte-identical float WAVs before and after cleanup at duration ratios 1/2, 3/2, and 8. Report processing identities and sample hashes also matched. No source-library audio was used for this check.
- Formatting, strict workspace Clippy, local documentation links, and whitespace checks passed.
- `cargo test --workspace` passed, including 45 project workflow tests and 345 UI tests. The two expanded DSP matrices remain explicitly ignored in this command.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` passed, including WAV/MP3 export smoke checks.
- `sh tools/package_macos.sh debug` passed and rebuilt `target/debug/DAW.app`.

Native UI/device playback and Windows/Linux checks were not run for this cleanup. The running app and user projects were not changed.

## Timeline and Editor clip height parity — 2026-10-15

The Editor track strip now derives its height from the Timeline row. Both use a 68-point clip shell with the shared 22-point header and a 46-point body. Audio and MIDI clips use the same renderer and strip bounds. The retained overview-layout calculations also use this shared height.

Extended the existing painted-clip parity check to compare header and body heights across Timeline and Editor, in both themes, with inherited/overridden colors and unselected/primary/group selection. The check failed on the previous geometry before the height change.

Completed checks:

- `cargo test -p daw-ui`: passed all 345 tests, including painted header/body height parity and existing Editor clip/note gestures.
- `cargo fmt --all --check`, strict workspace Clippy, 62 local documentation file links, and `git diff --check`: passed.
- `cargo test --workspace`: passed. The two expanded DSP matrices remain explicitly ignored in this command; no DSP changed in this layout fix.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.

Native appearance and gestures were not checked in the running app. Windows/Linux builds were not run for this layout fix.

## Instrument component catalog and selected layer — 2026-10-15

Added an Instrument-only Piano toolbar toggle below the Effects catalog toggle. Both reuse the standard 24-point borderless button and 16-point icon. Opening either catalog closes the other, and closing the active catalog leaves both hidden. Oscillator and Envelope use the same 250-point ControlPanel sidebar as Effects.

Extracted catalog painting, mouse/Shift selection, bounded arrow/range selection, Enter-without-repeat, Escape/blank-space clearing, and pane focus into `catalog.rs`. Effect entries and existing drag adapters remain separate from instrument component entries and target-layer additions. The shared blank-region hit areas retain their enabled state during library clicks; clearing remains guarded separately, preserving secondary-click focus behavior.

Instrument now keeps a stable selected-layer target in UI state and paints its label with the shared selection border. Click a layer label or content to select it. The first layer is the initial target; rename/reorder/view changes retain identity, track changes choose the first layer, removing the target falls back to the first surviving layer, and replacing the project resets it. Catalog focus does not clear the layer target.

Double-click adds a component; Enter adds selected components in catalog order through one `Edit::SetInstrument`. Oscillator appends a default source. Envelope installs the optional amplitude envelope, retaining settings and enabled/collapsed state; adding an existing envelope alone is a clean no-op. Existing empty-state Add source/Add envelope actions use the same component helper. Invalid/non-MIDI targets, modal work, text edits, drags, disabled UI, and window focus loss retain the existing add guards.

Completed checks:

- `cargo test -p daw-ui`: passed all 351 tests. Six new groups cover both-theme toggle placement and catalog exclusion, multi-selection and additions to the selected layer, repeat suppression, no-op envelope installation, double-click behavior, focus/empty-space clearing, target guards, saved-model round-trip, stable rename/reorder/view identity, removal/project replacement, and the visible selection outline. Existing Effects selection/keyboard/drag/reorder and layer gesture tests also passed.
- `cargo fmt --all --check` and strict workspace Clippy passed.
- `cargo test --workspace`: passed. The two expanded DSP matrices remain explicitly ignored in this command; no DSP changed for this feature.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- 125 local documentation file links, the new component-catalog anchor, and `git diff --check`: passed.

Native appearance/gestures and device playback were not checked in the running app. Windows/Linux builds were not run for this feature. No project format or audio-processing changes were added.

## Instrument catalog drag/drop — 2026-10-15

Instrument catalog rows now drag onto any visible layer label or body, including collapsed or empty layers. A selected row carries the selected types in catalog order; an unselected row becomes the sole dragged selection after the shared movement threshold. Press-time capture also handles a move and release delivered together. The hovered layer is outlined without changing selection or project data; valid release adds the group through the existing component helper and selects that layer. Existing-envelope drops retain all settings and stay clean when no component changes.

Catalog press/selection capture now lives in `Catalog::start_drag` and `select_dragged`, shared with the Effects catalog. `drag.rs` shares pointer checks, movement thresholds, focus restoration, and bounded horizontal/vertical edge scrolling. The layer list reuses its existing reorder scroll path for component drops. Layer/effect controls, catalog/track/view switches, and file actions are guarded while armed. No component is created before valid release. Escape, window focus loss, disabled/modal work, hidden catalog, track/view/project changes, changed instrument settings, and invalid/header/blank-space drops cancel without edits. The captured project/track/settings identity prevents a stale drop from replacing another edit.

Completed checks:

- Ten focused instrument-catalog test groups passed. Four new groups cover both themes, expanded/collapsed layers, label/body destinations, multi-selection, unselected-source dragging, clean previews, fast release, focus/selection retention, no-op Envelope drops, thirteen cancellation/invalidation contexts, and vertical scrolling to an offscreen layer.
- `cargo test -p daw-ui`: passed all 355 tests, including existing Effects library drag/reorder/scroll, catalog click/keyboard/focus, and layer gestures.
- Formatting and strict workspace Clippy passed.
- `cargo test --workspace`: passed. The two expanded DSP matrices remain explicitly ignored; this feature changes no DSP.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- 125 local documentation file links and `git diff --check`: passed.

Native pointer gestures, appearance, and device playback were not checked in the running app. Windows/Linux builds were not run for this feature. No project format or audio-processing changes were added.

## Remove Instrument empty cards — 2026-10-15

Removed the missing-source and missing-envelope cards, including their text, outlines, Add buttons, and reserved widths. This supersedes the earlier card requirements and checks. Missing components now leave workspace background; installed components begin beside the Layers column and share their existing dividers. Expanded and compact layer heights, full-width catalog drop targets, and the populated Oscillator Plus strip remain available. Components are added through the Instrument catalog. The unused card renderer was removed, and current requirements were updated.

Completed checks:

- `cargo test -p daw-ui`: all 355 tests passed. Updated checks cover blank empty layers in both themes, absent card text/outlines/buttons, unchanged expanded/compact heights, unit alignment without placeholder columns, catalog additions after removal, and retained source controls. Existing catalog keyboard, double-click, drag/drop, cancellation, and edge-scroll checks passed.
- `cargo fmt --all --check` and strict workspace Clippy passed.
- `cargo test --workspace`: passed. The two expanded DSP matrices remain explicitly ignored; this change does not alter DSP.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- 131 local documentation links and `git diff --check` passed.

Native appearance/gestures and device playback were not checked in the running app. Windows/Linux builds were not run. No project format or audio processing changed.

## Amplitude Envelope and unconnected Filter Envelope — 2026-10-15

Renamed the existing instrument Envelope to Amplitude Envelope in the catalog, installed headers, power/collapse accessibility text, and removal menu. Kept its saved field names, catalog identity, and audio behavior. Added Filter Envelope as an independent optional component per layer, with retained ADSR settings and enable/collapse state. Both roles reuse the same unit frame and envelope controls. Their control IDs include the envelope role and existing track/layer scopes. Catalog double-click, Enter, and drag/drop use the existing component addition path, with one instance of each envelope per layer.

Filter Envelope is saved and editable but unconnected. Its header tooltip explains that state. No Auto-filter API, modulation connection, sidechain, routing UI, or filter runtime was added. Installed order is Oscillator, Amplitude Envelope, Filter Envelope, omitting missing components; compact strip titles use Amp... and Fil.... Instrument version 2 has optional filter-envelope fields with defaults for older projects. Shared validation checks both retained envelopes even when absent.

Completed checks:

- `cargo test -p daw-ui`: all 359 tests passed. New groups cover both-theme unit titles, independent controls/power/collapse, aligned geometry, filter-only placement, removal/re-addition and no-op retention, saved-state restoration, compact footers/title, and catalog double-click. The existing expanded/compact, label/body, normal/fast drag matrix now carries all three component types.
- Focused core and engine MIDI suites passed (16 and 25 tests). Checks include filter-field round-trip, missing-field defaults, malformed-field rejection, transactional validation of both envelopes, and preservation of audio/live voices without callback allocations when the unconnected filter envelope changes.
- Formatting and strict workspace Clippy passed.
- `cargo test --workspace`: passed. The two expanded DSP matrices remain explicitly ignored.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- 133 local documentation links, including the envelope-components anchor, and `git diff --check` passed.

Native appearance/gestures and device playback were not checked in the running app. Windows/Linux builds were not run. Filter modulation routing remains intentionally unimplemented.

## Generic Envelope, Amplitude, and modulation — 2026-10-15

This replaces the role-specific envelope design above. Layers now contain independent generic Envelope instances and one optional Amplitude audio unit. Envelopes support Per note, Retrigger, and Legato. Amplitude and Auto-filter reuse the same destination-owned Source/Parameter/Amount strips, Power/Mod N footer, Plus strip, and removal menu. Their 18-point circular header button uses ChevronRight to reveal modulation strips on the right while keeping regular controls visible, and ChevronLeft to hide only those strips. Visibility is UI state; connections are saved project data. Full and compact heights, footer alignment, unit collapse, and header gesture boundaries remain intact.

Instrument version 3 saves stable envelope identities and explicit Amplitude connections. Legacy amplitude ADSR settings migrate into Envelope plus Amplitude with a full-depth connection. Previously unconnected filter envelopes become additional generic instances. Auto-filter accepts shared envelopes in its own MIDI track; Amplitude accepts per-note/shared sources in its instrument. Removed components retain their settings. Core rejects invalid sources, targets, amounts, identities, and incompatible modes atomically.

The engine evaluates each source once per sample and reuses cached values across destinations. Compatible edits retain live state using prepared storage. Parameter/power ramps remain allocation-free. Shared amplitude releases retain the final notes without reviving earlier notes; partial depths fade to zero instead of ending with a step. Auto-filter's portable external-offset API retains the independent internal LFO.

Completed checks:

- `cargo test --workspace`: passed, including 18 core MIDI tests, 27 engine MIDI tests, shared-mode/fan-out tests, and 359 UI tests. The two existing expanded time-stretch DSP matrices remain explicitly ignored.
- After final UI layout adjustments, `cargo test -p daw-ui` passed all 359 tests again. Coverage includes both themes, independent Envelope state, catalog additions/drop, regular-control visibility, expansion width and clean toggles, source selection/scope, Amount keyboard adjustment, Power, multiple connections, removal, aligned footers, compact height, and saved-state restoration.
- The focused portable Auto-filter external-offset check passed for all four targets, target bounds, static-control equivalence, and nonfinite inputs. Engine checks cover audible routing, overlapping notes, partial/full shared release, playback/export equality, random-access rendering, compatible updates, and callback allocation.
- Final formatting and strict workspace Clippy passed.
- The separate CLI build and `tools/check_cli.py` passed. The smoke check covers WAV/MP3 formats and bitrates, deterministic output, missing sources, eight-track projects, overwrite, and exit codes.
- `sh tools/package_macos.sh debug` passed after the final layout changes and rebuilt `target/debug/DAW.app`.
- 174 local documentation file links and `git diff --check` passed.

Native appearance, pointer gestures, listening, and device playback were not checked in the running app. Windows/Linux builds were not run. Cross-track modulation, per-note aggregation into shared effects, audio sidechains, and unrestricted graph routing remain outside this implementation.

## Compact unit selector text and modulation buttons — 2026-10-15

Closed Source and Parameter selectors in Auto-filter and Amplitude now use the shared 11-point unit font. The Envelope note-mode selector uses the same size. Both modulation expand/collapse buttons now paint the same 10-point outer diameter as the header enable circle, with the same 22-point click target. This supersedes the earlier 18-point modulation button requirement. Existing shared circle/icon painting, chevron directions, unit expansion, focus guards, and regular-control visibility remain in use.

Formatting, strict workspace Clippy, and the workspace tests passed. The two existing expanded time-stretch DSP matrices remain ignored. After the final Envelope/button adjustments, all 359 UI tests passed again, and the macOS debug bundle was rebuilt with `sh tools/package_macos.sh debug`. `git diff --check` passed. No DSP or project-format behavior changed. Native appearance/pointer checks and Windows/Linux builds were not run for this change.

## Filled modulation header buttons — 2026-10-15

Auto-filter and Amplitude now use filled, borderless circles for their modulation chevrons. Closed uses the theme's secondary color; expanded uses the accent color. The chevron uses the shared contrasting foreground color. Hover/focus paints a circular halo. Both buttons retain the 10-point outer diameter, 22-point click target, and existing expansion behavior. A reusable filled variant shares icon painting and accessibility with the existing circular controls.

All 359 UI tests, formatting, strict workspace Clippy, and workspace tests passed. The two existing expanded time-stretch DSP matrices remain ignored. The macOS debug app was rebuilt. `git diff --check` passed. Native appearance and pointer checks were not run; no audio behavior or project format changed.

## Modulation visibility dot — 2026-10-15

This supersedes the chevron and permanently filled-button designs above. Auto-filter and Amplitude now use an icon-free dot to toggle modulation visibility. It shares the enable control's painter, 10-point outer diameter, 22-point click target, filled/expanded and hollow/collapsed states, disabled dimming, and circular hover/focus halo. The modulation dot uses the theme's secondary color to distinguish it from audio enable. Tooltips and accessible state identify Expand/Collapse modulation controls. The temporary filled-icon variant was removed; other circular icon controls retain their existing style.

All 359 UI tests, formatting, strict workspace Clippy, and the workspace suite passed. The two existing expanded time-stretch DSP matrices remain ignored. The macOS debug app was rebuilt, and `git diff --check` passed. Native appearance/pointer checks were not run. No audio behavior or project format changed.

## Saved modulation visibility and parameter indicators — 2026-10-15

This supersedes the earlier UI-only modulation expansion state. Amplitude and Auto-filter save `modulation_open` per unit through the normal instrument/effect edits. Toggling visibility marks the project modified. Older settings default closed without a version bump; malformed explicit values fail deserialization. Removed/re-added units retain this field. Fresh windows and replacement projects use the saved values rather than egui temporary state.

Assigned Amplitude and Auto-filter parameter knobs now share status dots, source/amount tooltips, click-to-open controls, possible-range arcs, and live processed-value dots. Filled status means active modulation; hollow means assigned but inactive. Connections to the same source combine before range calculation. The base pointer, accent arc, and numeric readout retain their original value. Frequency includes the internal LFO and left/right processed cutoff. Amplitude shows the highest active voice gain before velocity, layer mix, and voice summing. The range describes held-note values; live markers also follow gates and release. Indicators fit the existing knob rows without changing unit dimensions.

Telemetry reads the actual DSP state, including parameter ramps, external smoothing, bounds, and bypass/gate behavior. The output layer publishes block snapshots through prepared atomics. No UI envelope clock, callback allocation, locking, or project edits are used for live display. Values outside the knob scale remain available in hover text. Live dots appear during playback.

Focused automated checks passed for both themes, independent visibility per unit, project replacement, save/reopen/Save As and legacy defaults, all four Auto-filter marker clicks and Amplitude clicks, preserved base values, disabled/drag guards, correlated-route cancellation, signed/clamped ranges, inactive assignments, stereo LFO values, note-driven telemetry, deterministic audio, pause/Stop, stable meter identities, and allocation-free telemetry reads. The portable Auto-filter test checks smoothed values and the rate-dependent cutoff bound. These checks use egui input/shape tests and device-free render/output tests.

Final checks passed: `cargo fmt --all --check`, strict workspace Clippy (`cargo clippy --workspace --all-targets -- -D warnings`), `cargo test --workspace`, `sh tools/package_macos.sh debug`, `cargo build -p daw-cli`, and `python3 tools/check_cli.py`, with the workspace Cargo cache selected. All 364 UI tests passed. The two existing expanded time-stretch matrices remain ignored. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. `git diff --check` passed. Native appearance, device playback, and Windows/Linux builds were not checked for this change.


## Lane separators during clip trim previews — 2026-10-15

Shortening a clip erased its lane separators under the original clip bounds. The preview mask restored only a bottom line at the lane edge, while normal Timeline rows use an inset top separator and an inset bottom separator only on the final row. The mask now calls the same `ControlPanel` separator painter, with the same row ownership and coordinates, clipped to the erased region. It restores separators before painting preview clips and removes the misplaced bottom line. Editor's overview border behavior stays unchanged. This shared mask also serves resize, stretch, move-overwrite, and file-drop neighbor previews.

The new egui regression failed on the original code at an interior lane's top separator, then passed after the fix. It covers audio/MIDI, left/right shortening, first/interior/final rows, both themes, and 1×/1.25×/2× display scales. It verifies that the separator is painted after the erasing mask in the newly exposed gap and that previewing leaves project data unchanged.

`cargo fmt --all --check`, strict workspace Clippy, and `cargo test --workspace` passed. All 365 UI tests passed; the two existing expanded time-stretch matrices remain ignored. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. `git diff --check` passed. Native visual checks and Windows/Linux builds were not run. This UI-only change did not require CLI smoke or device playback checks.

## Drive envelope modulation — 2026-10-15

Drive now shares destination-owned modulation controls, saved visibility, status/range/live indicators, shared-source validation, and route smoothing with Auto-filter and Amplitude. Targets are Volume (±24 dB), Gain/Tone/Color strength (±1), and Color frequency (±3 octaves). Color shape stays discrete. Sources must be Retrigger/Legato envelopes from the same MIDI instrument. Audio tracks and Master cannot reference instrument envelopes. Color modulation continues processing with Color Off; it affects audio when a color shape is enabled.

Drive version 1 gains optional `modulations` and `modulation_open` fields, defaulting to empty/closed for older projects. Save/reopen, Save As, and track duplication preserve route identities, targets, signed amounts, source references, and open state; duplicated effects receive new instance IDs. Shared route access also guards envelope note-mode changes. Core rejects invalid targets, amounts, duplicate route IDs, and out-of-scope/per-note sources atomically.

The portable processor receives five resolved offsets and smooths them over 1 ms before bounds. Base controls, source enable, and route amount/power retain their existing 5 ms smoothing. Volume retains exact base silence and bounds combined offsets before gain conversion. Effective values come from the processor and use the existing prepared atomic snapshots. Shared effect routing evaluates each cached source without a second envelope clock. Stable route/effect IDs retain compatible state without callback allocation. The UI reuses the existing strips and knob decoration; regular controls, dimensions, power, collapse, and header gestures retain their behavior.

Automated coverage includes six-rate static-equivalence and neutral/extreme-offset DSP checks; both shared note modes; playback/export/random-access parity; live retargeting and telemetry reads without allocation; transactional scope/target/amount validation; save/reopen/Save As, duplication, and legacy/default parsing; and both-theme header/Plus/marker interactions with stable base values and geometry. The output snapshot test now includes Drive. Native appearance, device listening, and Windows/Linux builds require separate checks.

Completed checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build -p daw-cli`, and `python3 tools/check_cli.py` passed with the workspace Cargo cache selected. After the final Volume indicator bounds/silence adjustment, strict Clippy and the complete UI suite passed again: 367 tests, including the new range case. `sh tools/package_macos.sh debug` rebuilt the final macOS app. The two existing expanded time-stretch matrices remain ignored. `git diff --check` passed. Native visual/listening checks and Windows/Linux builds were not run.

## Shared modulation protocol — 2026-10-15

Envelope now implements the portable modulation source contract and publishes through the common normalized output and enable ramp. Amplitude, Auto-filter, and Drive use one typed destination entry point, scope check, route evaluator, and saved-route validation contract. Auto-filter and Drive reuse one offset smoother. Amplitude gate/release ownership moved into the shared resolver while preserving full-depth ADSR arithmetic. Source discovery, source labels, range indicators, and reference validation use generic provider metadata. Descriptors expose modulation input/output capabilities separately from audio generation.

The Rust source identity is `ModulationSourceId { layer, unit }`. Existing JSON still writes `envelope` for the unit ID and also accepts `unit` on read. Saved connections, defaults, modulation visibility, and retained component settings remain compatible. The [shared contract](../modulation/contract.md) documents required integration steps for future providers and destinations; AGENTS.md requires this shared path.

New automated coverage checks an independent bipolar provider and typed destination, signed fan-out without a second source tick, unsupported targets, local/shared scope enforcement, source/route enable ramps, invalid runtime values, neutral missing sources, saved ID compatibility, retained provider discovery, bipolar amplitude bounds, and metadata-driven UI range indicators. A portable integration test feeds one cached Envelope output to all three destinations at every project rate, compares their output exactly with the existing low-level APIs, and checks that processing allocates nothing.

Completed checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including 368 UI tests, 30 engine MIDI tests, and seven unit-core tests. Existing save/open, migration, release ownership, live/export/seek, compatible replacement, and callback allocation checks passed. The two expanded time-stretch acceptance matrices remain explicitly ignored.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- Local documentation file links and staged/unstaged whitespace checks passed.

Native visual/gesture checks, device playback, listening comparisons, and Windows/Linux builds were not run. No additional provider type was added to the DAW; the independent provider used to test the contract is a test fixture.

## Stop and return to origin — 2026-10-15

The transport keeps its two-step action. Square Stop returns to the playback start, including when paused. After that return is consumed, the same button shows Lucide Chevron Left with Return to origin as its tooltip and accessible name. This action seeks to frame zero and is disabled when fully stopped there. A stopped project positioned away from zero without an audio output also offers the return action. Disabled clicks do not change the playhead or mark the project modified.

The renderer exposes whether a playback-start return remains pending. Output publishes that flag with its existing atomic transport status; the UI uses it rather than guessing from position or maintaining another playback-start cache. The original Stop and Seek commands retain their processing behavior. Shared button dimensions, spacing, colors, guards, and all four views remain in use. No saved fields were added.

Focused egui checks passed for both themes, paused/playing/stopped/origin states, pointer activation and disabled clicks, preserved button geometry and active colors, device-free return in every view, and clean repeated clicks at zero. The resampled callback test verifies that pending-return status survives pause, seek, and compatible renderer replacement, clears on Stop, and stays clear after the seek to zero.

Completed checks with the workspace Cargo cache selected: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (including 370 UI tests), `cargo build -p daw-cli`, `python3 tools/check_cli.py`, and `sh tools/package_macos.sh debug` all passed. The macOS debug bundle is rebuilt at `target/debug/DAW.app`. The two expanded time-stretch acceptance matrices remain explicitly ignored. Local documentation file links and whitespace checks passed.

Native visual/pointer checks, device playback, and Windows/Linux builds were not run for this change.

## Return-to-origin icon — 2026-10-15

The return-to-origin button now uses Lucide Chevron First. This supersedes the Chevron Left icon above. The two-step Stop/return action, disabled state at zero, tooltip, accessibility name, dimensions, and colors remain unchanged.

Formatting, strict workspace Clippy, and workspace tests passed, including all 370 UI tests. The two expanded time-stretch acceptance matrices remain explicitly ignored. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app` with Chevron First. Whitespace checks passed. Native appearance and Windows/Linux builds were not checked for this icon-only change.

## LFO component and continuous modulation — 2026-10-16

Instrument now adds independent LFO instances from its existing component catalog. LFOs use the shared source interface, publication, metadata, validation, destination routes, controls, and indicators. Amplitude, Auto-filter, and Drive consume one cached shared value per instance. The portable BSD-2-Clause `daw-lfo` crate has only `unit-core` as a dependency. Auto-filter shares its waveform, rate, clock, and parameter widgets with this component while keeping its internal stereo behavior.

The shared source context supplies frame, tempo, and prepared note/legato origins. Dynamic metadata supplies polarity and Contour/Continuous behavior. The amplitude resolver multiplies normalized continuous gain by the existing contour result. Continuous values cannot own a release or revive a voice. Prepared note tables cover overlap, adjacent notes, gaps, trims, stretch, and repeat phase without expanding repeats. Compatible updates swap prepared timing/storage and preserve live state, including adding the first LFO during playback. Analytical periodic one-pole smoothing permits deterministic direct access without replaying a long control history.

Automated checks cover all six processing rates, all five waveforms, both polarities, every note duration, phase, frequency/smoothing bounds, periodic/direct-access output, and live clock correction. Integration checks cover fan-out to all three destinations, audible modulation, contour composition and release lifetime, playback/export/block parity, deterministic random access, pause, loop phase after the existing boundary crossfade/effect history settles, source power/settings changes, and allocation-free processing, seek, and compatible replacement. Save/reopen, Save As, duplication, sample-rate changes, legacy empty lists, malformed settings, and source identity checks pass. Egui tests cover catalog double-click/Enter/drag actions, shared control heights/power/compact states in both themes, and continuous amplitude range indicators.

Completed checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 645 passed, zero failed, two ignored. This includes 374 UI tests, 32 engine MIDI tests, five portable LFO tests, and nine unit-core tests. The two existing expanded time-stretch matrices remain explicitly ignored.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- Local documentation file targets and `git diff --check`: passed.

Native visual/gesture checks, device playback/listening, and Windows/Linux builds were not run. Per-note LFOs and broader graph routing remain outside the accepted scope. See [LFO controls and timing](../instruments/lfo.md) and [the shared modulation contract](../modulation/contract.md).

## Instrument Main bus — 2026-10-16

Instrument now has one fixed Main bus below the vertical layer viewport and above the effects lane. Main receives the stereo layer sum, applies its own gain/pan before track effects, and hosts shared Envelope/LFO sources. It starts compact at 68 points and can expand to the shared 244-point unit height. Mix controls, component panels, catalog additions/drops, source metadata, routing, power/collapse state, and source publication reuse the existing code. Main cannot be removed or reordered. Regular layers retain their own scroll/reorder viewport.

Main settings save separately from regular layers in instrument version 3. Older instruments receive an empty neutral bus with a deterministic owner ID chosen to avoid existing layer IDs. Reopening the same legacy data produces the same migration. Null/malformed Main data, owner/source collisions, per-note bus envelopes, invalid controls, and invalid references fail validation. Save/reopen, Save As, duplication, and sample-rate changes preserve settings and connections.

New automated checks cover bottom pinning while regular layers scroll, compact/expanded heights, shared gain/pan input, selection through components, both themes, catalog keyboard additions and Main drops, unsupported audio-component no-ops, Main source labels/scope, six-rate stereo multi-layer gain/pan, pre-effect placement verified against nonlinear Drive processing, shared source fan-out to all current destinations, note-release behavior, playback/export parity, and compatible live edits without callback allocation. Existing UI gesture tests now reserve enough visible space or use edge scrolling to reach rows above Main. Native visual/gesture behavior and device listening remain separate checks.

Completed checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 653 passed, zero failed, two ignored. This includes 376 UI tests, 35 engine MIDI tests, 49 project workflow tests, and 11 unit-core tests. The two existing expanded time-stretch matrices remain explicitly ignored.
- After the final UI-only owner-selection adjustment, strict workspace Clippy and all 376 UI tests passed again.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed and rebuilt `target/debug/DAW.app`.
- Local documentation file targets and `git diff --check`: passed.

Native UI checks, device playback/listening, and Windows/Linux builds were not run. See [Instrument Main bus](../instruments/main.md) for controls, processing order, persistence, and scope.

## Main mix control placement — 2026-10-16

Main's header inherited the surrounding horizontal layout. Its name/button row consumed the panel width, so the gain/pan row appeared in the component area to its right. The header now uses the same explicit top-down layout as regular layer headers. Name and collapse remain on the first row; gain/pan knobs and numeric fields stay on the second row inside the left panel in compact and expanded states.

The existing both-theme Main test now checks containment of every mix control inside the label panel and vertical separation below the header at both heights. The new containment check failed before the fix and passed afterward. Audio processing and saved settings are unchanged.

For this layout fix, `cargo fmt --all --check`, strict workspace Clippy, and `cargo test --workspace` passed with the workspace Cargo cache selected: 653 passed, zero failed, and the two existing expanded time-stretch matrices ignored. All 376 UI tests passed. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Whitespace and local documentation target checks passed. Native visual/pointer checks and Windows/Linux builds were not run. CLI checks were not repeated for this UI-only change.

## Per-oscillator pitch modulation — 2026-10-16

Each oscillator now has a saved stable UUID and a separate Pitch target through the shared modulation interface and controls. Amounts use signed fractional semitones. Local Envelope contours remain independent for overlapping notes; shared Envelope/LFO values, including Main providers, fan out through the existing cached output path. The portable oscillator applies common additive smoothing without changing base transpose or restarting phase. The shared target resolver, instance validation, route preparation, and route-history retarget helper also preserve existing Amplitude/Auto-filter/Drive behavior. Removing an oscillator removes only its routes.

The Oscillator header reveals shared modulation strips beside its regular controls. Tests exercise target selection, markers, click-to-open, and saved expansion in both themes. Save/reopen, Save As, track duplication, and sample-rate changes retain IDs and routing. Legacy lists receive repeatable unique IDs; invalid IDs, targets, amounts, and source references fail validation. Legacy UI fixtures now create distinct oscillator IDs instead of cloning one instance into multiple positions.

New engine/DSP tests cover measured quarter-semitone shifts, neutral audio, signed cancellation, target isolation, Nyquist silence/recovery, overlapping-note pitch extrema, Main LFO fan-out, live/export/block/random-access parity, loop behavior, pause/stop, and identity-based live retargeting without callback allocation. A new random-access test exposed an existing MIDI-only rendering bug: without effects, `RenderPlan::render` used the audio-only sample path and returned silence. MIDI plans now use the same stateful renderer for random access with or without effects.

Checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 662 passed, zero failed, two ignored. Includes 377 UI tests, 38 engine MIDI tests, 50 project workflow tests, 20 core MIDI tests, and 13 unit-core tests. The existing expanded time-stretch acceptance matrices remain ignored.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Local documentation file targets and `git diff --check`: passed.

Native UI/pointer checks, device playback/listening, and Windows/Linux builds were not run. See [Oscillator pitch modulation](../modulation/oscillator-pitch.md) for controls, bounds, persistence, and API details.

## Pitch modulation amount steps — 2026-10-16

The pitch modulation amount knob now reuses `knobs::pitch_spec`, including one-semitone input steps, octave reference marks, ±48 bounds, and shared Shift-drag sensitivity. Other modulation amounts keep their existing controls. Saved fractional amounts remain intact until the user adjusts the knob; model validation and audio processing are unchanged. The controls are documented in [Oscillator pitch modulation](../modulation/oscillator-pitch.md).

Formatting, strict workspace Clippy, `cargo test --workspace` (662 passed, zero failed, two existing ignored tests, including all 377 UI tests), and `sh tools/package_macos.sh debug` passed with the workspace Cargo cache selected. The macOS app bundle was rebuilt. Native UI/device checks and Windows/Linux builds were not run. CLI smoke checks were not repeated for this UI-only change.

## Destination-matched modulation amount controls — 2026-10-16

Regular controls and modulation amount controls now share destination definitions in `parameter_knobs.rs`. Amplitude, Auto-filter, and Drive read those definitions directly; oscillator pitch uses the same existing Pitch specification. The shared base knob's amount adapter retains native scale/keyboard functions, reference marks, and mouse/Shift sensitivity while preserving signed amount domains. Amount readouts now show percentages, Q, dB, octaves, or semitones. Route meanings, finite bounds, neutral-zero reset, saved fields/values, and audio processing are unchanged. See [the mappings](../modulation/contract.md#destination-matched-amount-knobs).

Three new automated tests cover all 11 targets: monotonic signed round trips and exact endpoints; native-domain keyboard and pointer-travel equivalence; and actual egui input for idle fractional values, keyboard changes, disabled guards, reset, and accumulated Shift drags. The existing Amplitude strip test now selects its percentage readout and checks the native gain step converted to envelope depth.

Checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 665 passed, zero failed, two existing ignored time-stretch matrices. All 380 UI tests passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Local documentation file targets and `git diff --check`: passed.

Native UI/pointer checks, device playback/listening, and Windows/Linux builds were not run. CLI smoke checks were not repeated for this UI-only change.

## Component moves, copies, and strip reordering — 2026-10-16

Instrument component headers now move installed components between compatible layers. Individual oscillator strips also move, copy, and reorder through insertion positions. The core transfer command preserves settings and remaps provider references, oscillator targets, and route IDs as one transaction. Legacy provider-ID collisions do not redirect existing destination connections. Main accepts shared Envelope/LFO components and rejects audio components and Per note envelopes; occupied Amplitude stages reject transfers.

The existing platform copy modifier now duplicates installed effects and selected effect groups in chain order. Originals remain in place, copies receive fresh effect/route IDs, and copied destinations keep source assignments. Modulation strips on Amplitude, Auto-filter, Drive, and Oscillator use one shared reorder gesture. Effects, oscillator sources, and routes share insertion positions and preview-line painting. The component catalog and installed components share the same drag lifecycle and layer targets. See [component editing](../editing/components.md) for gestures, settings, and destination rules.

Layer and Main selection outlines meet all four edges without an inset and paint after shared neutral dividers and row fills, so no side of the stroke is covered. Main component shells omit their top and bottom edges. Main owns one top separator on the preceding layer's bottom pixel and shares the adjacent lane/status bottom separator. The fixed Layers toolbar separator paints after scrolling content. New both-theme tests cover outline geometry, bottom-edge ownership with compact/expanded/collapsed units and the effects lane shown/hidden, and separator paint order after vertical scrolling.

New core and egui tests cover component moves/copies, source/target identity remapping, legacy collisions, Main, occupied destinations, source and route insertion, dynamic copy modifiers, fast releases, group chain order, both themes, compact/expanded rows, collapsed effects, control isolation, no-op drops, snapshot changes, hidden owners, jobs/dialogs, focus loss, Escape, and outside drops. A project workflow test moves two oscillators and their LFO source, reorders sources and pitch connections, saves/reopens with Save As, and verifies identical rendered and float-exported samples.

Checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 688 passed, zero failed, two existing expanded time-stretch matrices ignored. Includes 394 UI tests and 51 project workflow tests. After the final flush-outline paint-order update, `cargo test -p daw-ui --lib` also passed all 394 UI tests.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Local documentation targets and `git diff --check`: passed.

Native UI/pointer, audio-device playback/listening, and Windows/Linux build checks were not run. Automated egui checks do not establish native gesture behavior.

## Main header fill and Timeline selection edges — 2026-10-16

Main unit headers now fill the one-point frame space left by their omitted top border. The shared shell extends only the painted background; header controls, hit areas, title positions, and unit heights retain their existing geometry. A new regression test failed on the gap before the fix and passed afterward for Envelope/LFO, both themes, compact/expanded Main, collapsed/expanded units, and unit power on/off.

Timeline track and Master selection outlines now use the same final control-row paint pass as Instrument Layer/Main. Row fills and the neutral column divider paint first, then the full two-point inside selection stroke paints at the original row bounds. Track selection and Master interactions retain their existing guards. The shared-border ordering test failed before the fix; it and a new both-theme track/Master regression test pass afterward. Audio processing and saved project data are unchanged.

Checks with the workspace Cargo cache selected:

- `cargo fmt --all --check` and strict workspace Clippy: passed.
- `cargo test --workspace`: 690 passed, zero failed, two existing expanded time-stretch matrices ignored. Includes 396 UI tests; the complete UI suite also passed separately.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Local documentation targets and `git diff --check`: passed.

Native UI/pointer, device playback, and Windows/Linux builds were not run. CLI smoke checks were not repeated for these UI-only changes.


## Instrument layer expand/collapse icons — 2026-10-16

Regular layer and Main expand/collapse icons now use the same small text size as the M/S labels. The shared icon button reserves its original layout space, retaining button dimensions, spacing, and hit areas. Main uses an up arrow to expand above its pinned position and a down arrow to collapse. Regular layer directions stay unchanged. See [Instrument Main](../instruments/main.md#layout-and-interaction).

The 14 affected layer tests and all 396 UI tests passed. The existing layout tests caught a size-dependent button spacing change during development; the shared button now reserves the original icon space and those tests pass. Formatting, strict workspace Clippy, and the macOS debug app rebuild passed. Native UI/pointer, device playback, and Windows/Linux checks were not run. CLI smoke checks were not repeated for this UI-only change.

`cargo test --workspace` passed: 690 tests passed, zero failed, and two existing expanded time-stretch matrices remained ignored.


## LFO Depth and Rate modulation — 2026-10-16

LFO now uses the shared modulation source and destination interfaces. Its third strip adds Depth; the header dot opens the existing Source/Parameter/Amount/Power strips. Depth uses ±100% offsets and Rate uses ±6-octave offsets in both Hz and Note modes. Shared knob definitions supply the native controls, amount profiles, assignment markers, range arcs, and processed-value telemetry. State and incoming routes save with the instance, including Main. Missing saved fields retain the previous 100% Depth, empty routing, and closed controls.

Preparation orders control providers before consumers through the portable dependency helper. Cycles, self-references, missing providers, and voice-local inputs fail validation; disabled/zero/retained connections also count as edges. Each LFO resolves the shared typed inputs and publishes once per sample. The common output publisher applies a normalized source contribution weight: 0% Depth leaves Amplitude and additive destinations neutral. Rate offsets integrate frequency changes without phase jumps. Direct seeks reset accumulated Rate phase at their destination rather than reconstructing its prior history in the callback; uninterrupted playback/export and random-range rendering from frame zero retain the history. See [controls, timing limits, and setup](../instruments/lfo.md#depth-and-rate-modulation).

New portable tests cover shared scope and resolution, all six rates, true frequency integration, both rate modes and bounds, neutral routing, Depth contribution semantics for both polarities, invalid weights, source metadata, graph order, cycle rejection, malformed state, and legacy defaults. Core tests cover rollback, incoming route copies, and provider moves into LFO consumers. Engine tests cover live/offline/block/random-range parity, loop repeats, provider order independent of layout, repeatable direct seeks, processed telemetry, pause, compatible edits, and zero callback allocations. Both-theme egui tests cover expanded controls, assignment-marker gestures, Depth keyboard changes, neutral connection creation, and unchanged normal-control geometry. The existing amount tests now cover 13 targets. The project workflow includes Depth, both input targets, and open state through save/reopen, Save As, track duplication, and sample-rate changes.

Formatting, strict workspace Clippy, the CLI build/smoke checks, the macOS debug app rebuild, local documentation targets, and `git diff --check` passed. Native UI/pointer, audio-device playback/listening, and Windows/Linux checks were not run.

`cargo test --workspace` passed: 698 passed, zero failed, and 2 existing expanded time-stretch matrices ignored. This includes all 397 UI tests and 51 project workflow tests.


## Disabled transport Stop icon — 2026-10-16

The transport button shows Square/Stop whenever it is disabled, including timeline origin and UI guards. Chevron First/Return to origin appears only for an enabled return action. The two-step Stop/return behavior, hit area, disabled clicks, and keyboard actions remain unchanged.

All six transport UI tests, formatting, strict workspace Clippy, the macOS debug app rebuild, and `git diff --check` passed. Native visual/device and Windows/Linux checks were not run. CLI smoke checks were not repeated for this UI-only change.

`cargo test --workspace` passed: 698 passed, zero failed, and 2 existing expanded time-stretch matrices ignored, including all 397 UI tests.


## Envelope note-relative timing and first-column controls — 2026-10-16

Envelope now saves Time/Note timing and independent Attack/Decay note fractions. Known note durations follow the stretched processing clock; Release retains milliseconds. Unknown note lengths fall back to retained Time values. Shared Retrigger uses each triggering note's length; Legato retains the first held note's length. Timing and the compact Mode dropdown share one leading column, above each other before ADSR. Unit height remains unchanged. See [Envelope requirements](../instruments/midi-patterns.md#envelope-unit).

Checks with the workspace Cargo cache selected:

- Formatting and strict workspace Clippy: passed.
- `cargo test --workspace`: 706 passed, zero failed, two existing expanded time-stretch matrices ignored. The final first-column layout passed a separate rerun of all 398 UI tests after the layout correction.
- New checks cover stage endpoints across processing rates and note lengths, fixed Release, unknown-duration fallback, live progress retention, reconstruction through stretch/trim/repeat, shared overlap and live provider addition, saved defaults/malformed fields, atomic invalid edits, and playback/export/block/random-range parity without callback allocations.
- First-column Timing keyboard input, Mode dropdown selection, retained millisecond/percentage values, serialization, both themes, and existing shared row geometry: passed. The reconstruction fixture also passes project validation.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app` after the final layout change.
- Changed documentation links and `git diff --check`: passed.

Native UI/device playback and Windows/Linux builds were not run. Automated egui checks do not establish native pointer or device behavior.


## Modulation selector labels — 2026-10-16

Expanded modulation strips now show Source and Destination above their respective dropdowns through the shared `modulation_panel.rs` row helper. Captions use the shared 11-point secondary theme text. Control rows keep their existing height, and modulation footers remain aligned with the regular controls. Compact strips retain their footer-only layout. Oscillator, Amplitude, Auto-filter, Drive, and LFO reuse these labels. Envelope Mode reuses the same labeled row, secondary caption color, spacing, and dropdown width. See [shared modulation controls](../modulation/contract.md#repeated-destinations).

Checks with the workspace Cargo cache selected:

- `cargo test --workspace`: 706 passed, zero failed, two existing expanded time-stretch matrices ignored.
- All 398 existing UI tests also passed after the final Envelope Mode style change, including expanded modulation controls, footer alignment, selection, reordering, and both-theme unit geometry.
- Formatting, strict workspace Clippy, and `git diff --check`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.

Native UI, audio-device playback, and Windows/Linux builds were not run. CLI smoke checks were not repeated for this UI-only change.

## Multiple Macro strips — 2026-10-16

Macro adds one component on regular instrument layers and Main, containing independent stable-ID strips. Each strip has one 0–100% knob, a shared circular Power control, and a Macro N label. The Plus strip and catalog additions add controls. Each strip can receive shared modulation through the existing targeted connection controls and fan out to several destinations. Module/strip power, presence, values, routes, panel state, and IDs save with the project. See [Macro requirements and limits](../instruments/macro.md).

The portable `daw-macro` crate uses the shared source/destination interfaces, offset smoother, and base-value ramp. LFO and Macro share the host's continuous-provider sampling, publication, dependency ordering, and retargeting code. Graph checks and source pickers filter edges by individual target ID. Component moves/merges/copies reuse the existing drag transactions and update consumers, targets, and copied identities.

Checks with the workspace Cargo cache selected:

- `cargo test --workspace`: 718 passed, zero failed, 2 existing expanded time-stretch matrices ignored.
- Portable Macro checks passed at all six processing rates: exact manual ramp completion, bounds, neutral processing, target isolation, signed cancellation, nonfinite inputs, reset, and rejection of voice-local inputs.
- Model/core checks passed: missing-field migration, malformed/null rejection, independent and retained strips, targeted dependency ordering, Macro/LFO feedback and scope rejection, atomic invalid edits, collision-safe moves, destination merges, and fresh copied IDs/targets.
- Integrated Macro rendering checks passed: Macro → several destinations and Macro → LFO → Macro dependencies, live/export/block/random-range parity, strip-order independence, neutral unassigned/off modules, note lifetime, pause/seek, telemetry, compatible live value/power/route edits, and callback allocation checks.
- Save/reopen, Save As, track duplication, and processing-rate changes passed with retained Macro state and references.
- All 401 UI tests also passed after the final layout/selector review, including new multi-strip catalog addition on regular layers and Main, shared Power/name pairs, one knob per strip, Plus, compact/collapsed geometry, expanded route controls, per-target cycle filtering, Macro module drag moves/copies in both themes and row heights, and an empty component with separate header controls.
- Formatting, strict workspace Clippy, changed documentation links, and `git diff --check`: passed.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.

Native UI/device playback and Windows/Linux builds were not run. Automated egui checks do not establish native gesture or device behavior. Direct seeks reset Macro offset smoothing without reconstructing its history; upstream LFO Rate history retains its existing documented limit. Individual Macro strip transfers and custom strip names are outside this change.

## Macro removal menu — 2026-10-16

Macro strips now register click input for their context menu. The menu also attaches to the Value knob and Power button, which own their pointer events. Right-click any of these areas and select Remove macro. Removal retains the existing saved-state and restore behavior.

The new egui regression failed before the fix because Remove macro did not appear. After the fix, it passes on regular layers and Main, in expanded and compact layouts, using strip padding, the knob or label, and Power. Opening the menu and pressing its item retain the original instrument; releasing the item removes only the target strip. Formatting, strict workspace Clippy, and `git diff --check` passed.

`cargo test --workspace` passed: 719 tests, zero failures, and two existing expanded time-stretch matrices ignored, including all 402 UI tests. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Commands used the workspace Cargo cache. Native UI/device behavior and Windows/Linux builds were not checked. CLI smoke checks were not repeated for this UI-only fix.

## Macro strip names — 2026-10-17

Double-click a Macro name to edit it on a regular layer or Main, including compact views. Enter commits a trimmed, nonempty name through the existing instrument edit. Escape, loss of focus, view/track/project changes, and unavailable controls cancel drafts. Names save with each stable strip ID and appear in shared modulation discovery and Value destinations. Long names retain the fixed strip width and expose their full text in tooltips. This supersedes the earlier restriction on custom Macro names.

Focused checks passed with the workspace Cargo cache: double-click/select-all, Enter, blank/unchanged names, cancellation, muted strip power/value/route preservation, both themes and row sizes, and long-name geometry while editing and after commit. Model checks cover legacy missing-name defaults, Unicode names, metadata identity, round-trip serialization, and invalid blank/control-character names. Component transfer and project workflows retain names through moves, copies, save/reopen, Save As, duplication, and processing-rate changes. A live rename retains exact audio and state without callback allocations.

Formatting, strict workspace Clippy, documentation link targets, and `git diff --check` passed. `cargo test --workspace` passed: 721 tests, zero failures, and two existing expanded time-stretch matrices ignored, including all 403 UI tests.

`cargo build -p daw-cli` and `python3 tools/check_cli.py` passed. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Native UI/device behavior and Windows/Linux builds were not checked. Automated egui input checks do not establish native pointer behavior.

## Registered component drops on Main — 2026-10-17

An empty Main row rejected Macro catalog drops because the drag handler kept an older Envelope/LFO type allowlist. The regression failed before the fix and passed after it. Main accepts Envelope, LFO, and Macro drops across its label and body in compact and expanded rows. Oscillator and Amplitude remain regular-layer components.

Catalog registrations now contain the layer installation function and optional Main installation function. Catalog additions, drop eligibility, and unsupported-target hints use this one definition. New registrations automatically gain catalog drop support on their registered destinations, without changes to the drag handler. Preview inspects the registration without constructing units or assigning IDs; existing release-only commits, core installed-component transfer eligibility, and cancellation remain shared. See [registration requirements](../editing/components.md#registering-instrument-components).

Focused egui checks passed for every registered component in both themes, both Main row sizes, label/body targets, fast motion-and-release, clean previews, unsupported-only drops, and mixed selections on populated Main. A synthetic new type ID passes through the same registration checks and addition functions, with zero installation calls during preview; unregistered IDs, invalid owners, and absent Main installers stay rejected. Existing instrument catalog, cancellation, selection, and Main layout checks also passed.

Checks with the workspace Cargo cache passed: formatting, strict workspace Clippy, `cargo test --workspace` (723 tests, zero failures, two existing expanded time-stretch matrices ignored, including all 405 UI tests), documentation link targets, and `git diff --check`. `sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`. Native UI/device behavior and Windows/Linux builds were not checked. CLI smoke checks were not repeated for this UI-only change.

## Component selection and saved unit order — 2026-10-17

Component units now support single-click selection with the same outline, click detection, and focus helpers as installed effects. Header drags reorder units within regular layers or Main, with audio sources pinned first. Installed transfers between lanes, including empty Main, use the effects insertion-line renderer instead of a row outline. The drop position determines the saved unit position; existing Oscillator and Macro merge rules remain in effect. Selection stays outside project data.

Checks completed:

- `cargo test --workspace` passes: 732 tests pass and 2 existing expanded time-stretch acceptance matrices remain ignored.

- `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass. Repository-relative documentation link targets exist.
- The full UI suite passes 410 tests; the focused component run passes 23. New input/paint checks cover both themes, both row heights, expanded/collapsed units, same-lane insertion, source pinning, fast release, clean no-op/cancelled previews, horizontal edge scrolling isolated to the hovered regular layer or Main, selection/focus clearing, cross-lane moves/copies, empty destinations, and selection of the resulting unit. Existing catalog, oscillator-strip, modulation-strip, and transfer tests remain passing.
- Portable layout tests cover legacy defaults, save/deserialize round trips, retained hidden-unit positions, new-unit placement, stale/duplicate/unknown identities, individual-strip rejection, Main restrictions, and source-first validation. Core tests cover transactional reorder/placement rejection, legacy provider-ID collisions, and transfer cleanup.
- Project workflow checks preserve layer/Main order through save/reopen, Save As, track duplication, and sample-rate changes. The engine check retains sample-exact offline/live output, held-note state, and zero callback allocations after a layout update.
- The CLI build and `python3 tools/check_cli.py` pass. WAV/MP3 formats, bitrates, deterministic output, missing sources, eight-track projects, overwrite protection, and exit codes remain covered.
- `sh tools/package_macos.sh debug` succeeds and rebuilds `target/debug/DAW.app`. The running app was not relaunched.

The saved optional `component_order` field retains instrument version 3. Missing fields use canonical legacy layout; malformed lists and invalid insertion positions fail without changing the project. Presentation order does not reorder provider/source vectors or change DSP topology. The earlier restriction against arbitrary component-unit presentation order is superseded; cross-track transfers and component multi-selection remain outside this change.

Native mouse/trackpad, display-scale, audible device, Windows, and Linux checks were not performed. Automated egui and renderer tests do not establish those native behaviors.

## Component group selection, moves, and copies — 2026-10-17

Shift-click toggles component selection within one lane. Selected headers move or copy the group in display order within regular layers or Main, or to a compatible destination lane. Audio sources stay first. Oscillator and Macro units retain their existing merge rules; one incompatible member rejects the complete group. A drag from an unselected header acts on that unit alone. Copies retain originals and source assignments, use fresh IDs, and select the resulting group.

Completed checks:

- `cargo test --workspace` passes: 739 tests pass; 2 existing expanded time-stretch acceptance matrices remain ignored.

- The full UI suite passes 414 tests. New pointer/paint tests cover both themes, compact/expanded rows, collapsed units, Shift toggling and replacement, lane boundaries, multiple outlines, selection order independent of click order, source-containing groups, group insertion previews, fast release, dynamic copy modifiers, moves/copies to empty Main and regular layers, clean no-op/cancelled/invalid drops, occupied Amplitude rejection, and unselected-header drags. Existing effect selection/group drags and single-component/strip gestures remain passing.
- Core group tests cover saved presentation order without processing-vector changes, source pinning, fresh copy and route IDs, resolved legacy collisions, source/Macro merges, complete reference remapping on moves, retained source assignments on copies, duplicate/empty group rejection, invalid positions, no-op detection, and preparation without publication. Failed preparation leaves the original project unchanged.
- The existing project workflow now includes a Main group copy before save/reopen, Save As, track duplication, and sample-rate changes; it passes. The existing live/export layout test now uses a prepared group reorder and passes sample-exact output, held-note continuity, and zero callback-allocation checks.
- `cargo fmt --all --check`, strict workspace Clippy, and `git diff --check` pass. The CLI build and `python3 tools/check_cli.py` pass.
- `sh tools/package_macos.sh debug` succeeds and rebuilds `target/debug/DAW.app`. The running app was not relaunched.

Effects and components share `unit_selection::select`, lane-order group capture, and `drag::group_insertion`, in addition to existing focus, outline, pointer, insertion-line, scroll, and cancellation primitives. Core prepares one validated Batch edit through the existing member transfer/remapping rules and shared `unit-core` placement. Selection stays in UI state; the existing component-order fields save the result without a format change. The earlier component multi-selection limit is superseded. Selection across lanes, cross-track transfers, and a second Amplitude stage remain outside this change.

Native mouse/trackpad, display-scale, audible device, Windows, and Linux checks were not performed. Automated input/render tests do not establish those native behaviors.

## Selected component keyboard removal — 2026-10-17

Delete/Backspace removes the selected component group from its focused regular-layer or Main lane through one `Edit::SetInstrument`. Effects and components share the same deletion-key handler and pane guards. Removal retains surviving units and track/clip state. The lane keeps keyboard focus when it becomes empty, and repeat or empty-selection presses cannot delete clips. Text editors retain their deletion keys.

Focused checks pass for both keys, themes, row heights, expanded/collapsed units, partial/complete selections, settings/route retention, oscillator source/pitch-route cleanup, selected-clip protection, and clean repeats. Guard checks cover extra modifiers, pointer/drag gestures, disabled/modal UI, window/pane focus loss, view changes, and active naming. A real Macro name editor accepts Delete/Backspace and commits its replacement text without removing the component. The existing effect Power-click selection regression and effect deletion checks pass after extraction of the shared handler.

Portable owner settings reject invalid, duplicate, and individual-strip groups before mutation. Removed provider/stage settings and unit positions survive serialization and restoration. Oscillator removal discards sources and pitch routes; all other retained component settings stay intact. The existing project fields and instrument version remain unchanged.

Completed checks with the workspace Cargo cache:

- `cargo fmt --all --check`, strict workspace Clippy, and `cargo test --workspace` pass. All 743 tests pass, including 417 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored.
- The CLI build and `python3 tools/check_cli.py` pass, covering WAV/MP3 output, bitrates, repeatability, missing sources, eight-track projects, overwrite protection, and exit codes.
- `sh tools/package_macos.sh debug` succeeds and rebuilds `target/debug/DAW.app`. The running app was not relaunched.
- Repository-relative documentation targets exist, and `git diff --check` passes.

Native mouse/keyboard, display-scale, audible device, Windows, and Linux checks were not performed. Automated egui and renderer tests do not establish those native behaviors.

## Three-band EQ output monitors — 2026-10-17

Each Low/Mid/High strip now has a four-point horizontal rail below the shared unit header. The rails reuse Limiter geometry/painting, the common −60 to 0 dBFS mapping and peak text, and OTT's prepared three-band atomic handle path. Track and Master instances share the same integration. Unit height, knobs, footers, audio, and saved fields remain unchanged. Compact lanes and horizontally collapsed units hide the rails; inactive playback displays empty rails.

Independent portable analysis filters measure the final EQ output, with a combined stereo peak, instant attack, and 300 ms release. Analysis uses two-pole low-pass/high-pass filters at Q = 1/√2; Mid cascades both. The outer frequency controls set the ordered boundaries. Their finite slopes overlap near the boundaries; these readings are filtered band-level estimates. Gain/cuts affect the measured signal, and unit bypass shows dry output. Stop/seek reset monitoring, while compatible updates retain its history and stable handles.

Focused checks pass:

- Portable tests cover frequency isolation, higher-channel peak detection, gain response, bypass, all-band silence, reset, and sample-exact unchanged audio at all six processing rates. A focused analysis check covers moved/crossed/equal boundaries, peak decay, silence, and nonfinite monitor input.
- Renderer tests cover track and Master monitoring, sample-exact compatible presentation updates, pause after the existing output fade, seek/Stop reset, and zero callback allocations. The existing EQ update test also runs with monitoring attached through parameter retargeting and transport changes.
- Output tests cover EQ levels and existing OTT gain changes on tracks and Master. Handles retain identity through reordering, stay independent for new instances, and are pruned on removal. Stop clears the handles.
- Both-theme egui checks cover per-strip geometry, knob clearance/alignment, half-scale mapping, silence, headroom/error color, compact/collapsed visibility, and read-only track/Master clicks without saved changes or a dirty project.

`cargo test --workspace --offline` passes: 748 tests pass, including 419 UI tests; two existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, the CLI build, and `python3 tools/check_cli.py` pass. The CLI checks cover WAV/MP3 formats and bitrates, deterministic output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Repository-relative documentation link targets and `git diff --check` pass. Commands used the default Cargo cache; the repository-local cache was absent.

Native mouse/trackpad, display-scale, audible device/callback timing, Windows, and Linux checks were not performed. Automated egui and renderer checks do not establish those native behaviors.

## Three-band EQ modulation — 2026-10-17

EQ now consumes the shared portable modulation API for Low/Mid/High Gain and Low/High Frequency/Q. Its settings and DSP declare the same typed parameters and shared scope. It reuses cached Envelope/LFO/Macro publication, route validation/resolution, the common 1 ms offset smoother and 5 ms route ramps, identity-based retargeting, modulation strips, destination-matched amount knobs, indicators, and prepared atomic snapshots. Seven processed readings fit the extended common snapshot; existing five-reading accessors remain adapters. Source lookup and route summing remain in `unit-core`.

Base values, band cuts, whole-unit bypass, output-level rails, and unit geometry retain their behavior. Monitor analysis follows the effective outer frequency boundaries. Optional empty `modulations` and closed `modulation_open` defaults retain EQ state version 1 and legacy unmodulated behavior. See [EQ modulation](../effects/equalizer.md#modulation--2026-10-17), the [shared contract](../modulation/contract.md#three-band-eq-destination), and the [acceptance requirements](../architecture/iteration-1.md#three-band-eq-modulation--2026-10-17).

Focused checks pass:

- Portable EQ checks cover all seven typed inputs, neutral sample-exact audio, signed cancellation, per-note exclusion, all six processing rates, smoothed values, bounds, nonfinite standalone offsets, reset, presentation-only retargeting, bypass, and all-band cuts. The shared-protocol test fans one cached source out to EQ and the existing destinations at every project rate without callback allocations.
- Renderer checks cover shared Envelope, LFO, and Macro providers on Main; overlapping notes; live/export/block/random-range parity; repeatable loop playback; presentation and route reorder/amount/power updates; full seven-value readings; and allocation-free processing, compatible replacement, and transport actions.
- Core checks cover legacy missing fields, malformed/null fields, saved round trips, every amount unit, unsupported parameters, duplicate/nil IDs, foreign/missing/per-note sources, assigned sources on audio/Master, invalid-edit rollback, and provider moves to Main. Shared LFO/Macro metadata follows the same validation path.
- Project workflow checks cover save/reopen, Save As, track duplication, and processing-rate changes while retaining EQ routes, presentation, and source identities.
- Both-theme UI checks cover the header dot, unassigned zero-amount Plus additions, all seven parameter marker actions, regular control/footer geometry, compact connection footers, persistence, and existing output-level rails. The common amount-control matrix includes all EQ targets and checks native-domain keyboard/pointer behavior, Shift sensitivity, reset, idle fractional values, and disabled guards. Effect group-copy checks now include EQ routes and verify fresh route IDs in compact/expanded, collapsed, and track/Master contexts.
- Output snapshot checks publish all seven EQ values without advancing DSP, preserve stable handles across updates, and prune removed destinations. Existing five-value destination accessors retain their readings.

`cargo test --workspace --offline` passes: 756 tests pass, including 421 UI tests; two existing expanded time-stretch acceptance matrices remain ignored. Formatting, strict workspace Clippy, the CLI build, and `python3 tools/check_cli.py` pass. CLI smoke checks cover WAV/MP3 formats/bitrates, deterministic output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands used the default Cargo cache. Repository-relative documentation link targets and `git diff --check` pass.

Native pointer/trackpad, display-scale, audible modulation/callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.

## Compressor modulation — 2026-10-17

Compressor now consumes the shared portable modulation API for Threshold, Ratio, Attack, Release, Knee, Level, and Dry/Wet. Settings and DSP declare the same typed parameters and shared scope. Level and Dry/Wet reuse existing targets. Cached Envelope/LFO/Macro publication, route validation/resolution, the common 1 ms offset smoother and 5 ms route ramps, identity-based retargeting, connection strips, destination-matched amount knobs, indicators, and seven-value snapshots remain shared code.

The unit retains its base slope/log-time/gain ramps, detector and reduction history, stereo linking, soft knee, meter meaning, dry bypass, and zero-route arithmetic. Peak/RMS remains discrete. Optional empty `modulations` and closed `modulation_open` defaults retain Compressor state version 1. See [Compressor modulation](../effects/compressor.md#modulation--2026-10-17), the [shared contract](../modulation/contract.md#compressor-destination), and the [acceptance requirements](../architecture/iteration-1.md#compressor-modulation--2026-10-17).

Focused checks pass:

- Portable checks cover all seven typed inputs, neutral sample-exact audio, signed cancellation, per-note exclusion, smoothed values, bounds, nonfinite standalone offsets, exact base silence, reset, presentation retargeting, dry bypass, and actual gain reduction at all six processing rates. A timing check verifies modulated attack/release response. The existing shared-protocol test includes Compressor in cached-source fan-out and callback allocation checks.
- The common renderer matrix now covers EQ and Compressor with shared Envelope, LFO, and Macro providers on Main, overlapping notes, live/export/block/random-range parity, repeatable loops, compatible presentation/route reorder/amount/power changes, full snapshots, and allocation-free processing and replacement.
- Shared core tests cover both effects' legacy defaults, malformed/null fields, saved round trips, every amount unit, unsupported parameters, invalid route IDs/targets, foreign/missing/per-note providers, audio/Master source restrictions, invalid-edit rollback, and provider moves to Main. Project workflow checks cover save/reopen, Save As, track duplication, and processing-rate changes.
- Both-theme UI checks cover the shared header toggle, unassigned zero-amount Plus additions, all seven marker actions without base changes, saved visibility, normal geometry, and compact/collapsed controls and meter visibility. The common amount-knob matrix checks native scales, keyboard/pointer gestures, Shift sensitivity, neutral reset, idle fractional values, and disabled guards. Effect group-copy checks include Compressor settings and fresh route IDs across track/Master and compact/collapsed contexts.
- Output checks publish all seven processed values without advancing DSP, preserve stable handles across updates, and prune removed destinations. The common Level indicator now preserves exact base silence and uses the same summed-offset limit as its destinations.

`cargo test --workspace --offline` passes: 761 tests pass, including 423 UI tests; two existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build --locked -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. CLI smoke checks cover WAV/MP3 formats/bitrates, deterministic output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands used the default Cargo cache. Repository-relative documentation link targets and `git diff --check` pass.

Native pointer/trackpad, display-scale, audible modulation/callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.


## Sampler source — 2026-10-17

Sampler is a regular-layer Instrument source, mutually exclusive with the Oscillator component. It reuses catalog registration, source ordering, component edits/transfers, unit shells, clip mapping/header painting, mono/stereo waveform peaks, loop-selection geometry/calculation, marked Gain, fixed mode knobs, worker preparation, MIDI scheduling, Amplitude, layer/Main mixing, and prepared atomic position snapshots. Root note remains metadata. See [Sampler](../instruments/sampler.md) and the [acceptance requirements](../architecture/iteration-1.md#sampler-source--2026-10-17).

The preview reuses the standard musical grid/ruler painters, scaled by the selected region output/input ratio in Time/Bars; snapping maps back to input frames through that ratio. Root note and sharp form one joined input group; the group and conditional duration share one horizontal row without visible field labels; tooltips and accessible names remain. The controls use a standard 90-point strip with shared padding/divider and a centered Sample footer. The fixed Off/Time/Bars selector uses the shared neutral arc and measured label spacing. The filename appears once in the clip header. Bars stores positive whole bars, with stable 1-bar drag steps and a minimum of 1. Mode/load initialization and saved fractional-bar prototype values round to the nearest whole bar. Root note reuses the standard drag input with shared piano-roll naming/parsing; semitone drags update the sharp toggle. Drag previews commit on release; cancelled text cannot commit on a later focus-loss frame. Trigger changes apply to new notes and retain existing voice behavior.

Focused automated checks pass:

- Portable checks cover stereo playback, 5 ms boundaries/gain/power fades, silence, seeking, source immutability, note naming/sharp validation, whole-bar initialization/migration, duration bounds, legacy defaults, and source exclusivity at all six processing rates.
- Core checks cover source conflicts, Main exclusion, copies with fresh IDs, source ordering, removal, track duplication, and invalid-edit rollback.
- Project workflows cover file loading, missing/error paths, retained clip trim/repeat/stretch mapping, unchanged source clips, region/duration/root/trigger/presentation persistence, save/reopen, Save As, all processing rates, and tempo-following Bars.
- Renderer checks cover One-shot/Gate, no pitch response, stereo channels, live/export/block/random-range parity, transport, compatible Gain/power/root/presentation changes, actual positions, and zero callback allocations. Trigger changes preserve held/released notes without cutting or reviving them. Adding an empty Sampler layer preserves existing oscillator audio and track panning sample-exactly.
- Both-theme egui checks cover catalog/source conflicts, fixed mode/trigger knobs and non-overlapping labels, standard strip width/divider/alignment/centered bottom footer, local grid/ruler alignment in Off/Time/one-bar/two-bar modes at three tempos, output/input clock mapping and snapping with trimmed regions at all six rates, joined root/sharp geometry, matched 22-point duration/group heights, fixed 160-point duration width and centered text across units/value lengths in display/text-entry states, and retained interactions, horizontal input geometry/hidden labels, filename placement, natural note/sharp text entry, root-note dragging/accidental crossings/bounds/cancellation, whole-bar stepping/minimum, compact/collapsed controls, file/clip drops, release-only region changes, cancellation, and protection against late worker publication.

`cargo test --workspace --offline` passes: 778 tests pass, including 430 UI tests; two existing expanded time-stretch acceptance matrices remain ignored. After the final preview/input layout changes, `cargo test -p daw-ui --offline` passes all 433 UI tests, including local-grid/ruler, duration-clock, and fixed-width/centered duration-input checks. This final run includes the existing Timeline/Editor ruler, grid, and selection regressions.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build --locked -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. CLI smoke checks cover WAV/MP3 formats/bitrates, repeatable output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands used the default Cargo cache. Repository-relative documentation link targets and `git diff --check` pass.

Native pointer/trackpad, display-scale, audible device/callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.


## Sampler input groups — 2026-10-17

Root note, sharp, and duration use the shared 11-point numeric font. Duration now has a numeric drag/text segment and a separate read-only bars/ms label under one outline, with an internal divider. Both segments center their text. The group retains its 160-point outer width, 40-point unit segment, and 22-point height. The unit stays visible during text entry. Root note and duration reuse the same group frame and focus/hover outline painter.

Both-theme egui checks pass for small font sizes, joined geometry, individual segment centering, short/long values in both modes, fixed dimensions during text entry, and clean unit-label clicks. Existing root-note typing/dragging, sharp toggling, whole-bar stepping, grid, drop, selection, and cancellation checks pass.

`cargo test -p daw-ui --offline` passes all 433 UI tests. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, documentation link targets, and `git diff --check` pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Native pointer/display-scale, audio-device, Windows, and Linux checks were not performed.


## Sampler mode dropdowns — 2026-10-17

The duration group's unit segment is now an Off/Time (ms)/Bars dropdown. This supersedes the separate stretch-mode knob and the read-only unit label recorded above. The closed selector shows Off/ms/bars. The joined duration control is now 120 points wide, including a 60-point mode segment. Its numeric value stays centered; unusually long values scale below the shared 11-point font to fit. Off retains an available dropdown and a disabled original-duration millisecond readout, including 0 before loading.

One-shot/Gate is now a 76-point dropdown in the same row as root note and duration. Both dropdowns reuse one selector helper. The row uses 4-point gaps and matched 22-point heights within the preview width. Gain is the only knob in the standard control strip; two reserved rows retain the centered Sample footer and full-height divider. The Trigger dropdown supersedes the fixed two-way knob recorded above. Trigger behavior and saved values remain unchanged.

Both-theme egui checks cover both selectors, actual stretch preparation, retained playback buffers for Trigger changes, clean current-choice selection/cancellation, disabled Off editing, row alignment, matched heights, fixed duration width/text centering during display/editing, whole-bar dragging, and the standard strip/footer. Existing root-note, grid, file/clip drop, selection, worker cancellation, and compact/collapsed checks pass.

`cargo test --workspace --offline` passes: 781 tests pass, including all 433 UI tests; two existing expanded time-stretch acceptance matrices remain ignored.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, repository-relative documentation link targets, and `git diff --check` pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Native pointer/display-scale, audio-device, Windows, and Linux checks were not performed.


## Sampler Gain modulation — 2026-10-17

Sampler Gain now consumes the shared portable modulation API in PerNote scope. It accepts local Per note Envelopes and shared Envelope/LFO/Macro values from its MIDI instrument, including Main. The typed `SamplerGain` parameter uses signed ±24 dB amounts. Source publication/discovery, route validation/resolution, 5 ms source/route ramps, the common 1 ms offset smoother, destination-matched amount knobs, connection strips, indicators, and identity-based retargeting remain shared code. Sampler owns final gain conversion and bounds, exact base silence, and its existing gain/power/boundary ramps. It publishes no control source and adds no other modulation targets. See [Sampler Gain modulation](../instruments/sampler.md#gain-modulation--2026-10-17), the [shared contract](../modulation/contract.md#sampler-gain-destination), and the [acceptance requirements](../architecture/iteration-1.md#sampler-gain-modulation--2026-10-17).

Focused automated checks pass:

- Portable checks cover local/shared inputs, signed cancellation, neutral sample-exact stereo audio, 1 ms smoothing, final gain bounds, exact silence/power, reset, nonfinite offsets, and source immutability at all six processing rates. Settings checks cover legacy empty/closed defaults, saved round trips, null fields, invalid parameters/targets/amounts/IDs, and duplicate route IDs.
- Core checks cover atomic failure for missing/foreign providers, fresh route IDs on Sampler copies, retained source assignments, provider remapping in every Sampler consumer, and destination moves. Project workflows retain modulation and visibility through save/reopen, Save As, and every processing-rate change.
- Renderer checks cover overlapping notes with Per note/Retrigger/Legato Envelopes and Main LFO/Macro providers, One-shot/Gate, processed gain extrema, linked stereo, and live/export/block/random-access parity at all six rates. Compatible route additions/reordering/power/removal and presentation edits retain source positions and route/offset history without callback allocations. The existing portable-protocol fan-out check now includes Sampler and cached Envelope publication.
- Both-theme egui checks cover the shared header dot, unassigned zero-amount additions, Per note source selection, Gain marker actions without base changes, saved visibility, and compact/collapsed controls. The shared amount-knob matrix now includes Sampler Gain for native scales, pointer/keyboard/Shift behavior, reset, and disabled guards. The shared range-marker matrix includes Gain bounds and exact silence.
- Output checks include Sampler in prepared snapshot publication without advancing DSP, stable handle retention, unavailable readings, and removal. Position remains in slot 0; active voice gain extrema use slots 1/2.

`cargo test --workspace --offline` passes: 787 tests pass, including all 434 UI tests; two existing expanded time-stretch acceptance matrices remain ignored.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build --locked -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. CLI smoke checks cover WAV/MP3 formats/bitrates, repeatable output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands used the default Cargo cache. Repository-relative documentation link targets and `git diff --check` pass.

Native pointer/trackpad, display-scale, audible device/callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.


## Limiter modulation — 2026-10-17

Limiter now consumes the shared portable modulation API for Input, Release, Lookahead, and Ceiling in Shared scope. It uses cached Envelope/LFO/Macro publication, metadata/source discovery, typed inputs, common route validation/resolution and 5 ms ramps, the shared 1 ms offset smoother, destination-owned strips, destination-matched amount knobs, indicators, and identity-based route retargeting. Mode/Routing remain discrete and gain reduction remains read-only. State version 1 adds empty `modulations` and closed `modulation_open` defaults. See [Limiter modulation](../effects/limiter.md#modulation--2026-10-17), the [shared contract](../modulation/contract.md#limiter-destination), and the [acceptance requirements](../architecture/iteration-1.md#limiter-modulation--2026-10-17).

Focused automated checks pass:

- Portable checks cover every typed input, local-source exclusion, signed cancellation, neutral sample-exact audio, offset smoothing, final bounds, nonfinite offsets/extreme input, reset, exact delayed bypass, and unchanged latency at all six rates. The existing Peak/True Peak, L/R/M/S, release, transient, reconstruction, and history checks pass. The shared portable-protocol fan-out/allocation matrix now includes Limiter.
- The core/model matrix includes Limiter for legacy defaults/null fields, round trips, unsupported/targeted routes, invalid amounts/IDs, duplicate route IDs, invalid-edit rollback, foreign/missing/per-note providers, audio/Master source restrictions, and provider moves to Main. Storage workflows cover save/reopen, Save As, track duplication, and processing-rate changes.
- The shared renderer matrix includes Limiter at all six rates with Main Envelope/LFO/Macro providers. It covers live/export/block/random-access/loop parity, changed audio with active routes, full snapshots, compatible route reorder/amount/power/presentation edits, preserved histories, and allocation-free processing/replacement. Existing limiter latency/transport/meter regressions also pass.
- Both-theme egui checks cover the common header dot, zero/unassigned Plus additions, four Input/Release/Lookahead/Ceiling marker actions without base changes, saved visibility, regular strip/footer heights, unchanged rail width, and compact/collapsed geometry. The shared amount and range-marker matrices include all four destinations. The amount adapter now enforces a bipolar zero origin when reusing an already symmetric native range, including Limiter Input; native controls retain their design and steps.
- Output checks include all four processed readings, NaN unused slots, prepared atomic publication without advancing DSP, stable meter handles, and removal.

An additional estimator-limit check used 48 kHz, a 2048-frame 0.47-cycle-per-sample stereo burst with channels `[x, −0.25x]` and peak amplitude 3, True Peak M/S, Input +12 dB, Release 50 ms, Lookahead 3 ms, and Ceiling −6 dB. Controls settled on silence for 50 ms before the burst. Independent 16×/128-tap reconstruction measured 0.51345796 against the 0.50118723 ceiling: about 2.45%, or 0.210 dB, above it. Fixed settings produce the identical reconstructed peak. This exceeds the 1.2% tolerance used by earlier tested stimuli; it records an existing estimator limit rather than a modulation regression. The added regression checks sample-peak protection and parity with fixed settings for this burst. Earlier reconstruction checks retain their original tolerance and pass. The guard algorithm and fixed latency are unchanged; no exact analog bound or certification is claimed.

`cargo test --workspace --offline` passes: 790 tests pass, including all 435 UI tests; two existing expanded time-stretch acceptance matrices remain ignored.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build --locked -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. CLI smoke checks cover WAV/MP3 formats/bitrates, repeatable output, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands used the default Cargo cache. Repository-relative documentation link targets and `git diff --check` pass.

Native pointer/trackpad, display-scale, audible modulation/callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.


## Fresh destination tracks for clip copies — 2026-10-17

Clip drag duplication and group paste now use the standard audio/MIDI track constructors for new destination rows. New MIDI tracks start with one empty instrument layer instead of inheriting the source instrument. This supersedes the earlier instrument-copy behavior for clip copies. Ordinary moves still carry the MIDI instrument; explicit track duplication still copies track settings without clips. Existing destination settings, copied clip metadata, shared source assets, group timing, and track gaps remain intact. New tracks and clips publish together after valid release or paste. See the [clip-copy track rules](../instruments/midi-patterns.md#new-tracks-for-clip-copies).

`cargo test -p daw-ui --offline` passes all 437 UI tests. Added egui input checks cover audio/MIDI copies into new track space, standard track defaults, fresh clip IDs, retained source clips, selection, clean previews, release-only publication, and Escape cancellation. Group regressions cover out-of-order destination rows with track gaps, fresh MIDI instruments for copies, preserved MIDI instruments for moves, and group paste that keeps existing destination instruments. Existing invalid-copy rollback, overlap placement, metadata, clipboard guards, and explicit track-duplication checks pass.

`cargo test --workspace --offline` passes: 792 tests pass; 2 existing expanded time-stretch acceptance matrices remain ignored.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Repository-relative link targets in the changed documentation and `git diff --check` pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched.

Native pointer/trackpad, audio-device, Windows, and Linux checks were not performed. Automated egui checks do not establish native gesture behavior.


## Documentation audiences and topic folders — 2026-10-17

User guides now live in topic folders under `docs/`. They describe tasks, controls, observable behavior, defaults, and practical limits. Full implementation requirements, architecture, data formats, dependency findings, acceptance plans, and this historical validation record now live in topic folders under `specs/`. Effects use `effects/`; views use `views/`; instruments, modulation, editing, and audio have their own folders. Global architecture, acceptance plans, and validation have separate technical folders. Both trees have indexes. See the [user guide](../../docs/README.md), [specification index](../README.md), and [repository documentation rules](../../AGENTS.md#documentation-and-dependency-licenses).

The migration preserves the full text of all 19 former technical documents from `docs/` and the two existing specifications, apart from updated navigation and audience notes. A scripted comparison verified that preservation, including the time-stretch independent implementation rule and all earlier validation evidence. User guides were rewritten for their audience; practical audio limitations remain documented. Incoming Markdown links, heading anchors, README navigation, repository instruction references, and the time-stretch provenance reference use the new paths.

Automated checks pass for local Markdown file targets and heading anchors across all tracked Markdown files and the new documentation trees. A targeted scan and content review found no API, schema, callback, widget-geometry, or engineering-test material in user guides. The standalone command-line tool guide remains linked as user help. Documented CLI examples match the existing command interface. `git diff --check` passes.

This change affects documentation only. No application code, dependencies, or runtime behavior changed. Builds, audio tests, native UI, device, and platform checks were not run for this documentation change.


## OTT modulation — 2026-10-17

OTT now consumes 19 continuous targets in Shared scope through the portable source/destination API: Low/High crossover; Low/Mid/High Depth, Input, Output, Attack, and Release; Level; and Dry/Wet. It accepts shared Envelope/LFO/Macro sources from the same MIDI instrument, including Main. Peak/RMS and power remain discrete; gain-change monitoring stays read-only. Cached source publication, metadata/discovery, typed route validation/resolution, source/route ramps, the common offset smoother, parameter-derived Amount controls, connection strips, indicators, prepared snapshots, and identity-based retargeting remain shared. See [OTT's implementation contract](../effects/ott.md#modulation--2026-10-17), [acceptance requirements](../architecture/iteration-1.md#ott-modulation--2026-10-17), and the [user guide](../../docs/effects/ott.md#modulation).

Focused automated checks pass:

- Portable tests cover all typed parameters with distinct band amounts at all six rates, shared/local scope, signed cancellation and neutral sample-exact audio, smoothing, comparison with fixed controls, parameter bounds and nonfinite offsets, simultaneous crossover sweeps during base edits, exact reset/bypass/base silence, linked stereo, and modulated Depth readings independent of Dry/Wet and Level. Existing phase matching, Peak/RMS, timing, bypass, curve, and history checks pass.
- Shared core matrices include OTT for legacy empty/closed defaults, round trips, explicit null rejection, invalid IDs/amounts/parameters/targets, atomic rollback, missing/foreign/per-note source rejection, audio/Master restrictions, and provider moves. Project storage matrices include save/reopen, Save As, track duplication, and all six processing rates.
- Shared renderer matrices include OTT with Envelope/LFO/Macro providers at all six rates: live/export/block/random-access/loop parity, changed audio under modulation, full readings, compatible route edits/reorder/power changes, history retention, and allocation-free callback processing/replacement. The portable-protocol fan-out/allocation matrix also includes OTT without adding a source clock or consumer lookup.
- Both-theme egui checks cover the shared header dot, unassigned zero-amount additions, all 19 parameter marker actions without base edits, saved visibility, compact/collapsed controls, unchanged footer/monitor geometry, and effect copying with fresh route IDs and preserved assignments. The effect-copy fixture refreshes geometry after replacing its narrower unit with OTT. Native Amount/keyboard/Shift/reset/disabled and range-marker matrices include all fixed-band parameters.
- Output checks cover all 19 actual readings, publication without advancing DSP, retained handle identity, removal, legacy five-reading access, and NaN padding for destinations with fewer readings. The shared snapshot capacity is 19.

`cargo test --workspace --offline` passes: 796 tests pass, including all 438 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Repository-relative documentation file targets and heading anchors, the user-guide audience scan, and `git diff --check` pass.

`cargo build --locked -p daw-cli --offline` and `python3 tools/check_cli.py` pass. CLI smoke covers WAV/MP3 codecs and bitrates, repeatability, missing sources, eight-track projects, overwrite protection, and exit codes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not relaunched. Commands use the default Cargo cache.

Native pointer/trackpad, display-scale, audible modulation/device callback timing, Windows, and Linux checks were not performed. Automated egui, DSP, and renderer checks do not establish those native behaviors.

## Instrument catalog category folders — 2026-10-17

The Instrument components catalog now uses Sources (Oscillator, Sampler), Modulators (Envelope, LFO, Macro), and Processors (Amplitude). Category membership comes from the existing component registration. Optional folders, visible-row keyboard navigation, press-time drag capture, and scrolling reuse the shared catalog and `ControlPanel` rows. Folder expansion is UI state and leaves project data unchanged.

Automated egui checks cover both themes, folder/component order, release-time collapse/expand without component drag capture, hidden selection pruning, retained selections in other folders, Enter repeat protection, folder keyboard actions, bounded movement, Shift ranges, component addition, cross-folder drag order, and catalog/view-switch retention. Short-viewport checks cover scrolling below the fixed header, selection retention when its focused row moves offscreen, and independent Effects/Instrument scroll positions. Disabled and modal UI cannot toggle folders. The focused catalog command, `cargo test -p daw-ui --offline instrument_catalog -- --nocapture`, passes all 17 tests.

`cargo test --workspace --offline` passes: 801 tests pass, including all 443 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. The existing Effects Escape regression confirms that arrow keys do not restart an empty entry selection. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` pass. The bundle is rebuilt at `target/debug/DAW.app`; the app was not launched. Repository-relative documentation file targets and heading anchors pass: 470 links across 53 Markdown files. The changed user guide contains operating instructions; implementation and validation details remain in `specs/`. `git diff --check` passes.

Native pointer/trackpad, display-scale, accessibility-client, Windows, and Linux checks were not performed. Automated egui checks do not establish those native behaviors. This change does not alter audio processing or project serialization.

## Shared layer dividers — 2026-10-17

Regular-layer spacing now applies to every component type instead of checking only Oscillator, Sampler, Envelope, and Amplitude. This fixes the adjacent divider pixels beside LFO-only and Macro-only layers. Regular layers and Main reuse `SHARED_LEFT_BORDER_TAG`; the shared unit shell honors left-edge ownership independently of top/bottom-edge ownership. Frame dimensions and the other unit edges remain in place.

The new egui regression covers every registered component as the first unit on each supported owner: 72 combinations of component/owner, theme, compact/expanded row, and collapsed/expanded unit. It checks the shared column coordinate, one visible neutral stroke, unchanged project data, and a clean modified flag. The existing Oscillator/Envelope geometry test checks the three retained unit edges instead of requiring duplicate complete border rectangles. Both focused tests pass.

`cargo test --workspace --offline` passes: 802 tests pass, including all 444 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Local Markdown file/heading links and `git diff --check` pass. The user guide describes the controls column; exact spacing, border ownership, and acceptance remain in the component-editing specification.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched.

Native appearance, pointer/trackpad, display-scale, Windows, and Linux checks were not performed. Automated shape/geometry checks do not establish native rasterization or device behavior.

## Effects catalog category folders — 2026-10-17

The Effects catalog now groups 3-band EQ and Auto-filter under EQ & Filters; Compressor, OTT, and Limiter under Dynamics; and Drive under Distortion. Effect registration owns category membership and construction. The adapter reuses the Instrument catalog's `Catalog::with_folders`, `ControlPanel` folder/entry painting, visible-row navigation, selection/focus, hidden-selection pruning, scrolling, and press-time drag capture. Enter and valid group drops share effect construction in visible folder order. Folder browsing and cancelled previews do not edit the project.

`cargo test -p daw-ui --offline` passes all 448 UI tests. Four new egui regressions cover both themes and Timeline track, Timeline Master, and Instrument targets: folder/entry order, keyboard collapse/expand, hidden selection pruning with other folders retained, hide/show retention, cross-folder Enter and drag addition order, release-only commit, fresh effect IDs, modal/focus/disabled/drag guards, short-height scrolling, fixed headers, and retained offscreen selection. Existing keyboard, blank-space, row-painting, and library-drag tests now use visible folder order; installed-chain move/copy/delete and all other UI checks pass.

`cargo test --workspace --offline` passes: 806 tests pass, including all 448 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Repository-relative Markdown file/heading links pass: 475 links across 53 files. User guides document folder controls and locations; registration, construction, order, and acceptance remain in `specs/` and contributor instructions in `AGENTS.md`. `git diff --check` passes.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched.

Native pointer/trackpad, display-scale, accessibility-client, Windows, and Linux checks were not performed. Automated egui checks do not establish native rendering or input behavior. This change does not alter audio processing or project serialization.

## Collapsed catalog defaults — 2026-10-17

`Catalog::with_folders` now initializes every category as collapsed. Instrument components and Effects use this same default without adapter-specific overrides. This supersedes the initial expanded defaults recorded in the earlier category entries. User expansion choices remain independent UI state for each catalog and survive catalog/view switches during the application session.

A fresh-application egui regression covers both themes and initial Timeline/Instrument views. It checks hidden children in both catalogs, mouse expansion, keyboard opening of a collapsed folder, untouched categories remaining collapsed, independent expansion choices across catalog switches, hide/show retention, no drag capture, unchanged project data, and a clean modified flag. `cargo test -p daw-ui --offline both_catalogs_start_collapsed -- --nocapture` passes. Existing control fixtures now explicitly expand folders through a shared test-only helper when they need visible entries; startup tests use the actual default.

`cargo test --workspace --offline` passes: 807 tests pass, including all 449 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Local Markdown file/heading links and `git diff --check` pass. Current user guides, component/editing specifications, architecture summaries, and repository instructions now state the collapsed default. Historical validation remains intact.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched.

Native appearance, pointer/trackpad, display-scale, accessibility-client, Windows, and Linux checks were not performed. Automated egui checks do not establish native rendering or input behavior. This change does not alter audio processing or project serialization.

## Disabled EQ band meter color — 2026-10-17

The EQ passes each band's enabled state to the shared `band_level_rail` renderer. Disabled bands use the theme's secondary fill, including readings above 0 dBFS. Enabled bands retain accent and overload colors. Meter scale, readings, layout, tooltips, and audio processing remain unchanged. The user guide and implementation specification describe the color rule separately.

The existing meter-layout regression now checks each band disabled independently, with the other bands enabled, plus all bands enabled. It covers both themes, silent/normal/overload readings, compact and collapsed visibility, rail dimensions, control alignment, and unchanged saved settings. The focused meter test passes. `cargo test --workspace --offline` passes: 807 tests pass, including all 449 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Local Markdown links pass: 475 links across 53 files. `git diff --check` passes. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched.

Native appearance, display-scale, Windows, and Linux checks were not performed. Automated egui shape checks do not establish native rasterization.

## Shared effect meter bypass lifecycle — 2026-10-17

EQ, Compressor, Limiter, and OTT now use `unit-core::UnitMeter` for scalar and band-meter publication. Whole-unit off clears the attached readings once and suppresses later writes until re-enabled. Handles remain owned until processor retirement. EQ resets and suspends its independent analysis; audio processing, detector histories, existing bypass ramps, and latency retain their contracts. Every effect panel uses `meters::unit_reading` to hide queued nonzero readings while its saved power state is off. This supersedes EQ dry-output monitoring during bypass and the earlier rule that whole-unit bypass did not affect its rails. Individual EQ band cuts retain their secondary-grey meter fill while the whole unit is enabled.

The shared storage regression covers initially disabled attachment, clearing, suppressed writes, resumption, scalar/array storage, and retained handle owners. The output regression covers all four effects on tracks and Master, repeated off/on changes through compatible prepared renderer updates, immediate clearing, and sentinel values that prove no writes occur during bypass. The panel regression supplies stale nonzero readings to all four effects in both themes and checks empty disabled rails against enabled fills. The existing EQ layout test also covers whole-unit power with independent band states, silent/normal/overload readings, and compact/collapsed geometry. Focused portable-unit, output, and UI checks pass.

`cargo test --workspace --offline` passes: 810 tests pass, including all 450 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `git diff --check` and the staged-file whitespace check pass.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. `cargo build -p daw-cli --offline` and `python3 tools/check_cli.py` pass, including deterministic WAV/MP3 export and existing error/overwrite checks. Local Markdown links pass: 481 links across 53 files. User guides, implementation specifications, acceptance requirements, and repository instructions document the shared rule for current and future effect meters.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched. Native appearance, display-scale, audio-device, Windows, and Linux checks were not performed. Automated shape and callback checks do not establish native rendering or device timing.

## Disabled OTT band meter color — 2026-10-17

OTT now passes each band's enabled state to `meters::gain_change`. Its signed gain-change monitor and EQ's level monitor reuse `band_color`: a disabled band uses the theme's secondary fill for either boost or reduction. Enabled bands retain their existing colors. Meter values, signed scale, center marker, geometry, tooltips, and audio processing remain unchanged. Whole-unit off still clears and suspends all meters through the shared lifecycle.

`cargo test -p daw-ui --offline band_meter_colors_follow_each_band_power` passes. The new panel regression covers each disabled band independently and all bands enabled, both themes, signed boost/reduction and zero readings, retained fill width/direction, unchanged saved settings, and empty rails when the whole unit is off.

`cargo test --workspace --offline` passes: 811 tests pass, including all 451 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. Existing EQ band colors and shared whole-unit meter bypass regressions pass. `git diff --check` passes.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Local Markdown links pass: 481 links across 53 files. The OTT user guide describes the disabled-band color; implementation details remain in the OTT and shared meter specifications.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched. Native appearance, display-scale, Windows, and Linux checks were not performed. Automated shape checks do not establish native rasterization.

## Direction Mixer — 2026-10-18

Direction Mixer is available under the new Stereo folder in the shared Effects catalog for Timeline track/Master and Instrument targets. The portable `daw-direction-mixer` crate owns M/S input decoding, stereo direction/width, 5 ms base/mode/bypass ramps, finite bounds, and the shared typed Direction/Spread modulation destinations. Shared host registration covers validation/serialization, source scope and remapping, cached route processing, identity-based replacement, and prepared parameter snapshots. The panel reuses the standard unit shell, angle and two-way knobs, destination-based Spread/Amount controls, modulation strips, and indicators. It adds no audio meter or latency. User instructions and implementation requirements are separate topic documents, with updated indexes, catalog descriptions, README feature list, and repository ownership/rules.

Portable DSP checks cover all six rates: exact default unity, M/S decode, width 0/1/2, cardinal direction positions, dry bypass, silent/finite output, typed shared/local scope, effective parameter bounds, offset smoothing/reset, and compatible ramp retention. The engine's existing shared-provider test now includes Direction Mixer at every rate with Envelope, LFO, and Macro sources: live/export/block/random-access/loop parity, signed cancellation, route reordering, snapshots, and allocation-free callback sampling/replacement/transport checks pass. A separate track/Master test compares M/S decoding against independently predecoded source audio at every rate and passes.

Model and workflow matrices now include Direction Mixer routes and non-default M/S/direction/spread settings for source validation/moves, invalid-edit rollback, omitted/null modulation fields, save/open/Save As, track duplication, and processing-rate changes. A unit-core check retains unknown future version payloads as missing effects. Output snapshot discovery/binding covers the two native processed values. The shared amount-knob matrix includes both new native destinations.

Two new egui interaction tests cover both themes and track/Master/Instrument additions, Input/Direction/Spread edits, power, compact mode, removal, Main Macro route markers, and shared connection opening. Existing catalog tests include the fourth folder and seventh effect; keyboard and empty-space checks use the extended visible row order. Explicit separated clicks keep selection tests independent of double-click timing. All 453 UI tests pass.

`cargo test --workspace --offline` passes: 818 tests pass, including all 453 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `git diff --check` passes.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. `cargo build -p daw-cli --offline` and `python3 tools/check_cli.py` pass, including WAV/MP3 codec/bitrate, deterministic export, missing-source, overwrite, multi-track, and exit-code checks. A separate temporary demo project with a track M/S Direction Mixer and Master L/R Direction Mixer validates and renders float32 WAV through the headless CLI. Local Markdown links pass: 492 links across 55 files.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched. Native appearance, pointer/trackpad, display-scale, audio-device timing, Windows, and Linux checks were not performed. Automated egui and callback checks do not establish native rendering or device behavior.

## Direction Mixer knob orientation — 2026-10-18

Direction now uses the same left/right convention as its shared knob: negative angles pan the center left and positive angles pan it right. The portable matrix changes only the Mid panning coefficients; Side spread, 0° unity, ±180° exchange, M/S decoding, base/modulation values, bounds, and smoothing retain their behavior. This supersedes the initial positive-left sign convention. The user guide, matrix specification, and repository instructions record the corrected rule, including the deliberate sign difference from the reference processor.

`cargo test -p daw-direction-mixer --offline` passes all four portable tests. The new regression covers −98°, −90°, −45°, +45°, +90°, and +98° at all six project rates, with both L/R and M/S input and both base and modulated Direction. It checks the louder output channel against the knob's sign. Existing cardinal tests now expect negative-left/positive-right and still check unity, mono/spread, M/S decoding, channel exchange, and bypass.

`cargo test --workspace --offline` passes: 819 tests pass, including all 453 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, local Markdown links, and tracked/staged whitespace checks pass. `cargo build -p daw-cli --offline` and `python3 tools/check_cli.py` pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilt `target/debug/DAW.app`; the app was not launched.

Native listening, appearance, pointer/trackpad, audio-device timing, Windows, and Linux checks were not performed. Automated channel-value checks do not establish native device behavior.

## Direction Mixer L/R Spread investigation — 2026-10-18

A new engine regression uses separate 440 Hz left and 731 Hz right signals at all six rates. It checks Spread 0/1/2 at Direction 0° in track and Master chains against independently calculated Mid/Side output, both offline and after compatible live updates. Both Direction Mixer engine tests pass with `cargo test -p daw-engine --offline direction_mixer`. This test was added after the workspace run above. The existing shared-panel interaction test also passes after adding an L/R, Direction 0° Spread gesture for both themes and all three view targets. `cargo fmt --all --check`, strict workspace Clippy, 492 local Markdown links, and tracked/staged whitespace checks pass.

The investigation has not reproduced the reported lack of Spread on L/R audio; no Spread DSP change is claimed. The guide now explains how to check stereo content and the reduction of Side near ±90° Direction. Native listening and the user's source remain unverified.

Follow-up source check: a user-supplied 48 kHz stereo PCM24 loop contains 708,923 frames with L/R correlation 0.998214 and Side RMS 30.485 dB below Mid RMS. The file has a small stereo difference rather than identical channels. Three temporary CLI projects render the complete source at Direction 0°, L/R input, and Spread 0/1/2 to float32 WAV. Side RMS is respectively 0, 0.00512446, and 0.01024893, while Mid RMS remains 0.17124230 after the standard clip fades. The largest sample difference from independently calculated Mid/Side output is below 3e−8. This confirms that Spread removes/retains/doubles the small stereo difference in this source. Its dominant center content explains a subtle audible change; no DSP fault was found. This supersedes the source-unverified limit above. The original recording remains unchanged, and the source path, private manifests, and rendered media stay outside the repository. Native listening/device checks remain unperformed. The guide now covers nearly identical channels as well as exact mono.

Reference comparison remains open: the user reports clear differences at Spread 0, 1, and 2 with the same source in Logic Pro. The preceding measurements verify this implementation's linear Side multiplier only; they do not establish that this mapping meets the intended reference behavior. The earlier conclusion that the source alone explains the reported difference is withdrawn. Apple's [Direction Mixer guide](https://support.apple.com/en-mide/guide/logicpro/lgcef240d9d7/10.7/mac/11.0) distinguishes L/R and M/S Spread behavior and specifies Side-only output at 2 in M/S mode, which the current implementation does not provide. It does not specify an exact L/R gain curve. Matched reference output is required before claiming equivalence or selecting a replacement curve. No Spread behavior change is made during this investigation.

## Direction Mixer reference Spread correction — 2026-10-18

Six user-supplied stereo PCM24 AIFF bounces contain 768,000 frames at 48 kHz, labeled Spread 0, 1, 1.25, 1.5, 1.75, and 2. The user confirms L/R input, Direction 0°, and an exact 2.00 upper endpoint readout. Relative to the unity bounce, the measured Mid gains are 1, 1, 0.81800005, 0.64999997, 0.48199999, and 0.29999998; Side gains are 0 and approximately 1 at every other supplied setting. Widening therefore reduces Mid rather than doubling Side. This supersedes the original L/R linear Side multiplier and the earlier source-only explanation.

The reference has a short regular transient component that does not follow the Mid attenuation. The measurements exclude the first 20 ms of each half-second interval; outside those windows, fitted Mid RMS residuals are below 5.6e−8 and Side RMS residuals below 9e−9. The transient's origin is not verified, and a whole-file null match is not claimed. The three upper interior bounces establish distinct anchors rather than an assumed straight line between 1 and 2. This implementation uses continuous linear interpolation through those measured anchors; unmeasured reference settings remain unverified. Private audio, decoded buffers, temporary manifests, and machine paths are not repository fixtures.

The portable L/R processor now narrows by reducing Side below unity and widens by reducing Mid above unity. Base and shared modulation offsets pass through the same final gain curve. Encoded M/S retains its existing direct Side-level behavior, with no reference equivalence claim. The shared control, amount mapping, typed routes, smoothing, snapshots, and saved fields retain their design. The user guide describes center attenuation and the possible level reduction; the effect specification records the gain table, equations, interpolation, and measurement limits.

All six portable tests pass, including new base/modulated reference gain checks at all six rates, mono/anti-phase/nearly mono input, upper/lower interpolation, retained M/S behavior, and smooth live changes across unity and input modes. Both focused engine Direction Mixer tests pass; L/R checks cover all supplied anchors on track and Master chains and compatible live updates after the 5 ms ramp.

A temporary CLI project uses the unity reference bounce as source audio and renders all six supplied Spread settings to float32 WAV. After excluding the transient windows and the standard final clip fade, each comparison contains 737,040 frames. Spread 0 and 1 match exactly; the other four settings have sample RMS errors below 5.7e−8 and peak errors below 1.8e−7 against the supplied reference bounces. This verifies the implemented gains against reference audio through the shared project/render/export path; it does not establish the source of the excluded transient component or behavior at other Direction settings.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. Local Markdown links pass: 493 links across 55 files. Tracked/staged whitespace checks pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the app was not launched. Native listening, device timing, pointer/display appearance, Windows, and Linux checks were not performed.

`cargo test --workspace --offline` passes: 822 tests pass, including all 453 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored. The catalog naming change below was applied after this run and receives a separate full UI check.

## Imaging catalog category — 2026-10-18

The Effects catalog now places Direction Mixer under Imaging, superseding the earlier Stereo folder name. Registration metadata supplies the name through the shared categorized catalog; the fourth-folder position, effect registration identity, initially collapsed state, shared navigation, selection, and addition behavior retain their design. User guides and implementation specifications use Imaging. The unit's Stereo strip footer remains its audio-role label. The existing Direction Mixer interaction regression now checks that Imaging is painted in both themes for track, Master, and Instrument targets; the focused test passes.

After the rename, `cargo test -p daw-ui --offline` passes all 453 UI tests. `cargo fmt --all --check`, strict workspace Clippy, 493 local Markdown links, and tracked/staged whitespace checks pass. The macOS debug bundle is rebuilt with the new category name and is not launched. Native appearance, pointer/display-scale, Windows, and Linux checks remain unperformed.

## Stereo Spread — 2026-10-18

Stereo Spread is registered after Direction Mixer under Imaging for Timeline track/Master and Instrument effect lanes. Its three standard strips expose signed Lower/Upper Intensity, logarithmic Lower/Upper Frequency, and whole-band Order. Final defaults are both intensities 0%, Lower 20 Hz, Upper 20 kHz, and Order 8. Shared controls supply reset, focus, Shift sensitivity, power/collapse, selection, dividers, and modulation connections. The new portable crate uses private FunDSP filter banks, alternating generated Side, independent shared base ramps, the typed source/destination contract, and prepared effective-value snapshots. It adds no audio meter or alignment latency.

Six portable tests pass across all six project rates: exact neutral/dry bypass, mono widening and mono fold-down, out-of-range attenuation, endpoint intensities, alternating midrange channel emphasis, 2/8/12-band behavior, positive/negative and mixed signs, base/modulated agreement, exclusion of voice-local sources, crossed frequency bounds, rapid Order changes, reset, and finite extreme inputs. The engine regression covers track/Master/live/offline/block/random-access parity plus allocation-free compatible Order/frequency changes and transport. The existing shared-provider matrix now includes all four destinations with Envelope, LFO, and Macro sources at every rate; signed cancellation, route reordering, loop/seek/block/live/export behavior, snapshots, and callback allocation checks pass.

Core matrices include Stereo Spread for same-instrument source rules, per-note rejection, provider moves/remapping, invalid-edit rollback, optional/null modulation fields, and round-trip defaults. The new unit-core test checks finite ranges, frequency ordering, integer Order bounds, and future missing-effect payload retention. Storage workflows include non-default values, routes, visibility/collapse, Save As/reopen, duplication, and rate changes. Output discovery/binding includes all four native values and unused snapshot slots. All four destinations join the common amount-knob bounds/reset/Shift/keyboard/pointer matrix.

Two egui regressions cover both themes: catalog addition on track/Master/Instrument, five control edits and default resets, fixed equal strip geometry, power, compact mode, removal, Main Macro markers, and connection opening. Existing catalog fixtures now include the eighth entry in bounded keyboard ranges and measure empty space below its row. This corrects the initial empty-space fixture failure caused by its former last-row assumption; catalog behavior is unchanged.

The spectral construction and signed intensity response are independently specified from the public band-distribution description and current user requirements. No exact match to Logic's unpublished response is claimed without reference measurements. User documentation and implementation specifications remain separate, with updated indexes, feature/catalog lists, architecture, shared modulation contract, and repository ownership rules. No private media or machine paths are repository fixtures.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. An additional temporary CLI fixture validates and renders Stereo Spread on track and Master chains: default zero intensities and disabled active settings produce sample data identical to the dry baseline, while signed active settings change the output. Local Markdown checks pass for 505 links across 57 files. Tracked/staged whitespace checks pass.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the app was not launched. Native listening, appearance, pointer/display-scale, audio-device timing, Windows, and Linux checks were not performed. Automated DSP and egui checks do not establish those native results.

`cargo test --workspace --offline` passes: 832 tests pass, including all 455 UI tests. The 2 existing expanded time-stretch acceptance matrices remain ignored.

## Bitcrusher — 2026-10-18

Bitcrusher is registered under Distortion after Drive, using shared catalog metadata, selection, and installation behavior. Its two standard strips expose Drive, whole-step Resolution/Downsampling, Level, and Dry/Wet. Defaults are 0 dB Drive, 24 bits, 1×, 0 dB Level, and 100% wet. The portable crate uses symmetric endpoint-inclusive quantization with an explicit digital-silence exception, a shared stereo capture clock with separate held channels, hard clipping, common base ramps, and typed shared modulation. It adds no audio meter, external dependency, alignment latency, or export extension.

Seven portable tests cover every project rate, all 24 bit depths, and all 40 fixed hold ratios. Checks include known low-bit levels, symmetric polarity, silence, clipping/error bounds, intentional aliasing, stereo capture timing, Drive-before-clip and Level-after-blend, exact dry mix/bypass, base silence, whole modulated steps, signed bounds, fixed/modulated agreement, all five typed destinations, shared/local scope, nonfinite-neutral controls, compatible clock retention, reset, and extreme finite headroom. Final review replaced subtractive blends with weighted sums so large dry inputs cannot cancel a clipped wet result; an explicit full-scale-headroom regression passes.

The engine test covers mono/stereo track and Master chains at every rate, live/offline/block/random-access parity, capture history through presentation updates, changed Resolution/Downsampling/Drive snapshots, seek/stop, and allocation-free renderer inheritance/processing/transport. The shared-provider matrix now includes all five Bitcrusher destinations with Envelope, LFO, and Macro, signed cancellation, route reorder, live changes, block/seek/loop/export parity, snapshots, and callback allocation checks. Core/storage matrices cover invalid-edit rollback, same-instrument scope, Per note rejection, provider moves, optional/null modulation fields, save/reopen/Save As, duplication, and processing-rate changes. A new model test covers parameter bounds, fractional saved-step rejection, round-trip defaults, and opaque future versions. Output discovery/binding checks five native readings and unused NaNs.

Both new egui tests pass for both themes, including catalog addition on track/Master/Instrument, all five edits/resets, whole keyboard/pointer steps, equal strip geometry, power, compact mode, deletion, Main Macro markers, and shared connection opening. All 457 UI tests pass. The wider amount matrix exposed the shared adapter's early return before discrete-step quantization; the base knob now applies inherited steps after signed amount conversion. Existing fractional saved amounts remain unchanged while idle. The catalog scroll fixture now checks the final visible entry after scrolling to the bottom, rather than assuming Drive remains within that viewport. Shared navigation fixtures include the new Distortion row.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. A temporary CLI fixture validates and renders default/active track and Master settings, checks finite output and exact dry-mix/bypass sample data against the baseline, and rejects fractional saved Resolution/Downsampling values. Local Markdown checks pass for 517 links across 59 files; tracked/staged whitespace checks pass. No private media or machine paths are repository fixtures.

`CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the app was not launched. Native listening, appearance, pointer/display-scale, audio-device timing, Windows, and Linux checks were not performed. Automated DSP and egui tests do not establish those native results. The quantizer and transition behavior are independently specified; exact Logic equivalence is not claimed.

`cargo test --workspace --offline` passes: 843 tests pass, including all 457 UI tests. The 2 existing expanded time-stretch acceptance matrices remain ignored.

## Performance audit — 2026-10-18

The [performance audit](performance-audit-2026-10-18.md) records 16 prioritized findings, source evidence, concrete remediations, proposed acceptance checks, and native verification gaps. The [raw measurements](performance-audit-2026-10-18.json) contain all 217 completed cases, distributions, allocation counters, workload facts, and hardware/compiler/commit hashes. Application behavior and dependencies are unchanged. The standalone [runner](../../tools/performance_audit.py) and [harness](../../tools/performance_audit.rs) use generated fixtures and an ignored build manifest; no private media, machine paths, or native recordings are fixtures.

`python3 tools/performance_audit.py --suite all --runs 7 --output work/performance-audit/final.json` completes on an Apple M5 with 16 GB RAM, macOS 27.0.1, pinned Rust 1.97.0, and a release build at baseline `430e05f024815c78f3b1bde476feecb681474bbc`. The eight-saw-voice EQ/Compressor workload remains below half the nominal renderer duration in all 48 supported rate/buffer combinations; its worst measured p95 utilization is 14.55%. Dense clip playback, Timeline CPU painting, transport pre-roll/event reconstruction, route/history lookup, and snapshot publication expose scaling costs. The report distinguishes measured budget comparisons from code-based resource risks and proposed optimizations.

`cargo fmt --all --check`, `rustfmt --check --edition 2024 tools/performance_audit.rs`, strict workspace Clippy offline, and strict standalone release Clippy offline pass. The standalone command is `cargo clippy --release --offline --manifest-path work/performance-audit/Cargo.toml --target-dir target -- -D warnings` after the runner creates its manifest. Python syntax parsing and the runner's `--help` check pass. `cargo test --workspace --offline` passes: 843 tests, including all 457 UI tests; the 2 existing expanded time-stretch acceptance matrices remain ignored. Local Markdown links pass: 561 links across 60 files. Recorded source/lockfile hashes, JSON completion/sample consistency, and changed-file whitespace checks pass.

No native app is launched or relaunched. No device callback/underrun, GPU presentation, idle power, native gesture/display-scale, long-session memory/hover stress, cold-storage, Windows, or Linux performance check is performed. Headless CPU measurements do not close native real-time acceptance. The audit reads the DAW's own adapters and calls Rubber Band through the existing public adapter; it does not inspect Rubber Band implementation source or change the independent timestretch engine.

## Performance remediation — 2026-10-18

The [remediation report](performance-remediation-2026-10-18.md) maps changes to all 16 audit findings, compares the [follow-up measurements](performance-remediation-2026-10-18.json) with the preserved baseline, and records conditional work and native acceptance gaps. The [performance specification](../architecture/performance.md) defines prepared scheduling, indexed modulation and MIDI restart, worker transport preparation, bounded output activation, UI culling/repaint behavior, shared audio admission/cache ownership, and diagnostics. User guides describe delayed seek publication and large-file preparation limits without exposing internal APIs.

`cargo test --workspace --offline` passes 856 tests; the 2 existing expanded time-stretch matrices remain ignored. After the final route-matching and import-path changes, focused engine/unit-core/output checks pass, followed by `cargo test -p daw-unit-core -p daw-output -p daw-project -p daw-ui --offline`: 559 tests pass, including all 457 UI tests. These focused counts overlap the workspace run. New regressions cover indexed restart parity, collision-safe route history, transport request races and queue capacity, allocation-free prepared activation, audio admission/cancellation, cache invalidation, and persistent offline rendering.

`cargo fmt --all --check`, `rustfmt --check --edition 2024 tools/performance_audit.rs`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and strict standalone release Clippy pass. The standalone command is `cargo clippy --release --offline --manifest-path work/performance-audit/Cargo.toml --target-dir target -- -D warnings`. `cargo build -p daw-cli --offline` and `python3 tools/check_cli.py` pass. `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the app is not launched.

`python3 tools/performance_audit.py --suite all --runs 7 --compare specs/validation/performance-audit-2026-10-18.json --output specs/validation/performance-remediation-2026-10-18.json` completes all 221 cases with 217 baseline matches. A separate `--suite dsp --runs 21` run completes 49 cases and writes [DSP confirmation measurements](performance-remediation-dsp-2026-10-18.json). Both use the normal release build path and record the final source-tree hash. Dense 192 kHz playback falls from 6.295 to 0.042 ms per 512 frames, and 128-track Timeline CPU painting/tessellation falls from 50.810 to 4.191 ms. The report also records small-workload regressions and higher preparation allocation costs. Timings are local headless observations; no native deadline or audible-dropout result is inferred.

Python syntax, runner help, JSON completion/sample consistency, matching source hashes, 579 local Markdown links across 62 files, and tracked/staged whitespace checks pass. The supplementary Limiter timing is slower than baseline despite a faster primary run with the same source hash; the report retains this unresolved variation.

No native listening, device callback/xrun, GPU/presentation, physical device failure, idle power, long-source/parallel-job memory stress, Windows, or Linux check is performed. Initial device opening remains on the caller thread. Conditional indexing/caching proposals and hardware acceptance remain open as listed in the report. The original audit JSON remains unchanged. No private media, machine-specific paths, or generated build outputs are added as fixtures.

## Timestretch: offline pitch changes — 2026-10-18

The portable library now accepts constant fractional pitch offsets from −24 to +24 semitones through `Pitch` and `Request::with_pitch`. `PitchRequest` composes the existing request with the validated offset. Duration remains independent and retains its exact rational frame count. Nonzero pitch reuses linked-stereo spectral processing, then applies an independently written shared windowed-sinc resampler. It adds no dependency, thread, file, device, or DAW requirement. The original stretch path and revision 2 remain unchanged; zero pitch delegates to that path. The optional pitch path has revision 1, which is included with the offset in tool processing identities. Project identities, saved data, DAW adapters, and Sampler pitch behavior do not change.

The standalone `process`/`compare` commands and both Python wrappers expose `--pitch-semitones`, default 0. All three engines receive the same factor; reports and benchmark summaries record the offset. Known-sine analysis uses the requested shifted frequency, including phase projections, and rejects an expected frequency at or above Nyquist. R2/R3 remain public-API references. The independent equations, filter/table choices, and research source are recorded in [provenance](../../crates/timestretch/PROVENANCE.md#offline-pitch-composition) and the [specification](../audio/timestretch.md#offline-pitch-processing). No Rubber Band implementation source, coefficient table, or test material was inspected or reused.

Completed checks against the [pitch extension gate](../acceptance/timestretch.md#offline-pitch-extension-gate):

- TP-01/02: invalid/nonfinite offsets, defaults, signed-zero identity, explicit-zero versus original processing, exact rational lengths, short/empty/DC/silent results at all six rates, invalid layout/rate, and resource errors pass. Selected nonconstant short buffers also retain finite nonzero output.
- TP-03: all six rates pass duration-1 tones at −24, −12, −0.5, +7, +12, and +24 semitones for 440 Hz; 55/1,000/5,000 Hz cover ±24 semitones at every rate. At 48 kHz, duration ratios 1/8, 1/2, 3/2, 2, and 8 combine with −24, −12, +7, +12, and +24 semitones. Fixtures extend to at least 20 measured cycles in the middle half. Frequency/level limits are 5 cents/0.5 dB ordinary and 15 cents/1.5 dB extreme. Combined private staging reaches approximately 1/32 and 32 without changing the public duration range.
- TP-04/05: proportional, anti-phase, silent-channel, mono-equivalent, swapped, and unrelated stereo pass. Same-tone phase, repeatability, unclipped headroom, amplitude scaling, very large finite input, and short nonconstant stability pass. An independently generated 18 kHz stopband tone is suppressed by more than 70 dB relative to a 2 kHz passband tone in the isolated +12 semitone/downsample-by-two path.
- TP-06: progress stays monotonic; cancellation at validation, spectral analysis/synthesis, table setup, and resampling checkpoints returns no output. Small configured budgets fail before allocation/publication. These checks and bounded loops establish checkpoint behavior, not the separate physical 250 ms cancellation wall-time gate under load.
- TP-07: commands pass pitch/duration/report checks, invalid offsets return exit 2, correct known-tone shifts pass while deliberately unshifted controls fail, and explicit zero produces the same WAV bytes as the default. A −7.5 semitone comparison completes all three engines with matching input/pitch identities. Both wrappers run from an unrelated temporary directory with a prebuilt binary. The benchmark wrapper completes six generated-fixture smoke cases (two durations × three engines, +12 semitones, one run, no warmup); invalid NaN pitch returns 2 before destination creation. Source bytes remain unchanged. These runs check orchestration, not performance acceptance.
- TP-08: a copied/extracted portable crate builds and runs through a tiny external Rust consumer for −12, +7.5, and +12 semitones without repository runtime paths. Its resolved dependency graph contains only the library, RustFFT, and permissive Rust support crates. The reference-disabled tool passes its tests in a separate target directory; unavailable references still fail comparison explicitly. No DAW or Rubber Band dependency enters the portable engine.

`cargo test -p timestretch -p timestretch-cli --release --offline` passes 25 tests, including the API doctest; two existing expanded matrices remain ignored in this command. `cargo test -p timestretch --release --offline -- --include-ignored` passes all 23 portable tests, including both expanded matrices and eight new pitch test groups. These counts overlap. `cargo test -p timestretch-cli --no-default-features --release --offline --target-dir target/timestretch-no-reference` passes all four tool tests. Reference-enabled and disabled builds use separate target paths to avoid executable replacement during comparisons.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets --offline -- -D warnings` pass. Both Python scripts parse successfully. `cargo build -p timestretch-cli --release --offline` passes, and `process --help` lists the new option, range, and zero default. All 583 local Markdown links across 62 files and changed-file whitespace checks pass. Only the portable engine/tool and documentation change, so DAW workspace tests, DAW CLI smoke checks, and app packaging are not repeated under the standalone acceptance workflow.

This remains experimental offline processing. Formants shift with pitch. The finite interpolation filter has a transition band, can reduce upper-frequency content, and can ring near boundaries. Large combined upward pitch/duration requests need larger intermediate buffers and can exceed the configured memory limit. No native app is launched. Full-corpus listening/performance, controlled cancellation timing, long-source peak memory, and Windows/Linux verification remain open. No DAW pitch control, live pitch automation, or stable-quality/reference-equivalence claim is made.

## Sampler note pitch and full MIDI range — 2026-10-18

Sampler now plays each MIDI note relative to its saved root note through the selected project engine, Integrated or Rubber Band R3. Every MIDI note/root pair is supported: offsets span −127..=127 semitones. This supersedes the previous record's ±24 portable API range and its statement that Sampler pitch behavior is unchanged. Saved root notes now control playback, so non-root notes in existing projects can sound different. Duration stays fixed for each selected Off/Time/Bars setting. Source recordings remain unchanged.

The [Sampler contract](../instruments/sampler.md#offline-note-pitch--2026-10-18) defines worker-prepared pitch banks, identity-based reuse, atomic edit publication and callback-safe note selection. Root audio and the required distinct note offsets share existing media preparation and memory admission. Root/note/content/duration/engine/rate edits prepare before publication; failure or cancellation retains the previous session and playback. Pasted and moved MIDI clips use the same path. Gain modulation, trigger behavior and voice processing remain shared.

The [full-range composition](../audio/timestretch.md#full-range-pitch-composition) splits wide offsets into passes of at most two octaves, applying duration once. Upward processing raises frequency before spectral analysis; downward processing applies the remainder first. These orders preserve low MIDI fundamentals in the tested endpoint fixtures. The pitch revision advances to 2; the duration revision and the numerical path within ±24 remain unchanged. Both native adapters use the shared pass schedule through the public API. Standalone reports expose the pass sequence. The [provenance record](../../crates/timestretch/PROVENANCE.md#full-range-pitch-composition--2026-10-18) records the independent mathematical design and failed initial ordering experiments. No Rubber Band implementation source, tests, tables or pseudocode were inspected or reused.

Completed focused checks:

- `cargo test -p daw-project --test sampler_pitch --offline` passes all three tests. Both engines cover root identity, measured ±12/±24 and wider note offsets, full MIDI endpoints, linked stereo, duration modes, engine changes, all six processing rates, note/root invalidation, cache reuse, cancellation and invalid-revision rollback, live/offline/range rendering, float WAV export, save/reopen and unchanged source bytes. Endpoint native tests establish finite exact-length preparation without clamping; they do not establish native pitch quality at inaudible frequencies.
- `cargo test -p timestretch --release --offline -- --include-ignored` passes all 26 portable tests, including the two expanded acceptance matrices. New wide-shift tone fixtures cover ±36, ±96 and ±127 at 48 kHz with enough measured cycles, at most 15 cents frequency error and 3 dB level error. Six-rate wide-shift checks cover fractional offsets, exact duration, anti-phase stereo, finite output, memory estimates and monotonic cancellation. The existing ≤24-semitone frequency/level limits remain unchanged. This is targeted synthetic evidence, not full-corpus acceptance.
- `cargo test -p timestretch-cli --test commands --offline` passes all four command tests. The full-range comparisons complete Integrated, R2 and R3 at ±127 semitones with matching prepared input hashes, exact durations and six reported passes. `cargo test -p timestretch-cli --no-default-features --offline` passes all five tests; missing references remain explicit errors. `cargo tree -p timestretch --offline` confirms that the portable engine still depends only on RustFFT and its permissive Rust support crates. Both Python wrappers parse successfully.

`cargo test --workspace --offline` passes 876 tests, including all 458 UI tests. The normal workspace command leaves the two expanded portable matrices ignored; both pass in the release command above. Engine tests cover overlapping pitched voices, all six rates, live/export parity, compatible retargeting, and allocation-free note starts, seek, pause, play and Stop. The new egui regression checks deferred note/root publication and pitch preparation before pasted clips enter another Sampler track. Existing modulation, source, trigger, component and editing tests remain green.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, `cargo build -p daw-cli --offline`, and `python3 tools/check_cli.py` pass. The reference-enabled standalone binary is rebuilt after the reference-disabled check. All 588 local Markdown links across 62 files and tracked/staged whitespace checks pass. `CARGO_NET_OFFLINE=true sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the app is not launched.

Full-corpus listening, native root-note/job interaction, physical-device behavior, controlled cancellation timing, long-source/parallel-job memory stress, Windows and Linux checks remain open. Large shifts can lose out-of-band content and accumulate processing artifacts. Formants are not preserved. Resource admission covers known buffers and scratch, not native-library process RSS. Automated DSP, rendering and egui checks do not establish native or perceptual acceptance.

## Select All in installed-unit lanes — 2026-10-18

Command+A on macOS or Ctrl+A on Windows/Linux selects all installed units in the focused regular-layer, Main, track-effects, or Master-effects lane. Selection includes units outside the horizontal viewport. Effects and components share `unit_selection::shortcuts` and `unit_selection::select_all`. Empty lane space retains keyboard focus, and text editors retain their own Select All behavior. Selection remains outside project data and audio processing.

Focused checks pass for both themes, compact/expanded rows, collapsed/expanded units, track/Master/Instrument effects targets, regular layers/Main, scrolled units, empty selections, empty-lane focus followed by component addition, unchanged project/track/clip state, and actual Macro text replacement after Select All. The shared effects shortcut guard check covers pane/window focus, catalog/other-pane focus, modal/disabled UI, active pointer/drag gestures, held repeats, extra modifiers, view changes, and name editing. Existing deletion guard coverage remains in that same check.

Completed checks with the workspace Cargo cache:

- `cargo fmt --all --check` and strict workspace Clippy pass.
- `cargo test --workspace` passes: 880 tests pass, including all 462 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`. The running app was not relaunched.
- Repository-relative documentation targets exist, and `git diff --check` passes.

This UI-only change does not alter project data or audio processing. CLI smoke checks were not repeated. Native keyboard/trackpad, display-scale, device, Windows, and Linux checks were not performed. Automated egui input tests do not establish native shortcut delivery.

## Select All after component-lane blank-space clicks — 2026-10-18

A reported follow-up exposed a gap in the preceding validation: it tested a lane with no units and a cleared selection that retained unit focus, but did not test blank space in a populated component lane. A new input test failed on that path before the fix. The zero-area keyboard target lost focus when egui re-read its response after the off-target click.

Component and effects lanes now reuse `unit_selection::keyboard_focus` for a pane-wide, non-interactive keyboard target. Blank-space clicks explicitly request lane focus. Component shortcut dispatch also resolves the clicked owner before handling a shortcut on the same release frame. Child controls retain clicks, drags, and text input.

The regression test passes in both themes, on regular layers/Main, with compact/expanded rows, with intervening idle frames, and with Select All on the same frame as mouse release. It starts with no unit selection, checks retained pane focus, selects the complete installed group, and verifies unchanged project data. The full UI suite passes all 463 tests, including existing control, scrolling, selection, group drag/copy, deletion, text editing, and cancellation checks. This supersedes the preceding incomplete blank-space coverage claim.

Completed checks with the workspace Cargo cache:

- `cargo fmt --all --check` and strict workspace Clippy pass.
- `cargo test --workspace` passes: 881 tests pass, including all 463 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`. The running app was not relaunched.
- Repository-relative documentation targets exist, and `git diff --check` passes.

Native keyboard/trackpad, display-scale, device, Windows, and Linux checks were not performed. The headless regression establishes the reproduced focus fix, not native shortcut delivery. CLI smoke checks were not repeated for this UI-only correction.

## Regular Save without workspace dimming — 2026-10-18

Regular Save previously used the blocking job receiver, disabling and dimming the workspace for the write and publishing the saved session over live state afterward. It now writes a snapshot through an independent receiver and keeps the workspace enabled. Completion clears dirty state only when project identity and change revision match. Later edits, seeks, and worker-published changes stay unsaved; current transport, prepared audio, and UI state are not replaced. File operations and window close remain guarded; named Save, Save As, and save-before-continuation retain blocking behavior.

The three new save checks pass. Exact workspace shape/clip comparisons are unchanged before/during a held save in Timeline, Instrument, Editor, and Mixer in both themes. Effect/clip selections remain intact, file actions stay disabled, and close requests are cancelled until completion. A real temporary-folder save writes its captured snapshot while later edits/transport remain current and dirty; the next save persists them and clears dirty state. Worker-prepared modified sessions also advance the revision. Failure, worker disconnection, and stale project results retain current project data. Existing named-save and save-before-close success/cancellation/failure checks remain passing. The full UI suite passes 466 tests.

Completed checks with the workspace Cargo cache:

- `cargo fmt --all --check` and strict workspace Clippy pass.
- `cargo test --workspace` passes: 884 tests pass, including all 466 UI tests; 2 existing expanded time-stretch acceptance matrices remain ignored.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`. The running app was not restarted.
- Repository-relative documentation targets exist, and `git diff --check` passes.

Native save/display, keyboard/trackpad, device, Windows, and Linux checks were not performed. Exact headless workspace paint checks establish the removed disabled-state transition; they do not establish native compositor behavior. CLI smoke checks were not repeated because the shared persistence/render/export APIs are unchanged.


## Amplitude strip width — 2026-10-18

Amplitude now uses the shared 90-point unit strip width instead of its previous 178-point width. The Level strip retains this width when modulation opens and when the layer is compact. The existing egui interaction check now verifies closed width and Level alignment with open modulation in both themes and both layer layouts. Connection editing and footer alignment remain covered.

Completed checks with the workspace Cargo cache:

- The focused Amplitude interaction check passes.
- `cargo fmt --all --check`, strict workspace Clippy, and `cargo test --workspace` pass. The two existing expanded time-stretch acceptance matrices remain ignored.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`. The running app was not restarted.
- `git diff --check` passes.

Native display-scale, keyboard/trackpad, device, Windows, and Linux checks were not performed. CLI smoke checks were not repeated for this UI-only layout change.

## Inline layer effects and transfers — 2026-10-18

Regular MIDI layers save independent effect chains after Amplitude and before layer gain/pan. Main saves an inline chain after the layer sum and before Main gain/pan. Effects appear after components in the same scrolling row and follow its compact state. The selected layer receives catalog additions; Main receives additions in its pinned row; clicking the bottom lane targets track effects. Library drops also target empty layer rows while the bottom lane is hidden. Shared effect panels, modulation controls, selection, keyboard handling, previews, and scrolling serve both layouts.

Core checks cover legacy empty-chain defaults, save-state round trips, discovery, global effect IDs, invalid-owner/settings/scope rejection, track duplication, and provider-reference remapping. Group transfers preserve chain order and move IDs, create fresh effect/route IDs for copies, and reject invalid or incompatible transfers without changing the project. Transfers between regular layers, Main, and track effects publish atomically.

Engine checks compare every registered processor between layer and track placement, verify pre-gain processing and parallel isolation, align limiter latency, and reproduce seeks. Live updates preserve modulation snapshots and prepared storage. Reordering layer rows preserves exact audio. Moving a filter between regular layers, Main, and track effects retains its ringing history on silent input with no callback allocation; compatible processor histories follow globally unique effect IDs. Latency placement changes require worker pre-roll.

Project checks preserve chains, routes, missing-unit settings/warnings, and rendered audio through Save As/reopen, and export WAV audio. All 472 headless UI tests pass. New input checks cover inline geometry and power in both themes, drag duplication, deletion, empty-layer drops with the bottom lane hidden, and selected-group moves/copies in both directions, including fast releases, Main destinations, and transfers in both directions between the track effects lane and instrument buses. Shape checks also verify one full-height shared divider from Macro to Drive on Main, Amplitude to EQ on regular layers, and between adjacent inline effects, in both themes and compact/collapsed states. Blank-lane component Select All and existing effect-library edge scrolling remain covered.

Executed checks:

- `cargo fmt --all --check` and workspace Clippy with `-D warnings` pass.
- `cargo test --workspace` passes; two existing expanded time-stretch matrices remain ignored. After the final divider-only change, all 472 UI tests pass again.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`, including the divider fix. The running app was not restarted.
- `git diff --check` and cached diff checks pass. Changed documentation links resolve.

Native UI, display-scale, device, Windows, and Linux checks were not performed.

## Envelope modulation inputs — 2026-10-18

Envelope accepts Time Attack/Decay/Release, Sustain, three curve strengths, and Note Attack/Decay fractions through the shared destination contract. It retains its Contour output. Shared and local providers use one prepared dependency graph and common input/publication helper, with no callback source lookup or graph traversal. The common destination entry point honors prepared instance scope. Existing settings migrate to empty/closed connections.

Portable checks cover exact neutral samples, signed cancellation, effective bounds, Note Attack modulation from zero, actual curve changes, stage continuity at all six rates, release completion, and runtime rejection of local inputs by shared Envelopes. Model/core checks cover save-state migration, mixed provider cycles, scope validation, copied route IDs, and provider-reference remapping. Engine checks cover all three modes, Envelope/LFO/Macro ordering, block parity, native worker seek history, cancellation, live route retargeting, snapshots, and zero callback allocations. Project checks retain routes and rendered audio through Save As/reopen. Headless input checks exercise the shared strips, source filtering, saved open state, and marker opening in both themes.

Native worker seeks replay active Envelope input history with cancellation at buffer boundaries. Direct low-level seeks, hidden trimmed-note history, and loop jumps retain existing reconstruction limits; they do not synthesize earlier input modulation history. Documentation records these limits.

Executed checks:

- `cargo fmt --all --check` and workspace Clippy with `-D warnings` pass.
- `cargo test --workspace` passes: 910 tests, no failures, and two existing expanded time-stretch matrices ignored. This includes all 473 UI tests.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`. The running app was not restarted.
- Changed documentation links resolve; working and cached diff checks pass.

Native UI, display-scale, device, Windows, and Linux checks were not performed.

## Oscillator Gain modulation — 2026-10-19

Each stable oscillator target now accepts Gain as well as Pitch through the shared modulation destination contract. Gain uses signed ±24 dB amounts, cached per-note/shared source values, existing route ramps, the common additive smoother, and portable gain conversion. It retains host gain/mute ramps, exact base silence, and the +12 dB processed-gain ceiling. The UI reuses targeted strips, Gain-derived amount controls, markers, possible-range arcs, and processed active-voice extrema. Existing projects keep their settings and default to no Gain connections.

Portable checks cover all six processing rates, local/shared input, neutral audio, target isolation, signed cancellation, exact silence, ceiling bounds, and reset. Engine checks cover live/export/random-access parity, polyphonic extrema, stable-ID reordering, route disabling, retained history, and zero callback allocations. Storage checks retain Pitch/Gain connections and open controls through save/reopen, Save As, track duplication, and rate changes. Headless UI checks cover target selection and marker opening in both themes.

Executed checks:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo test --workspace` passes: 912 tests, no failures, and two existing expanded time-stretch matrices ignored. All 473 UI tests pass.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the running app was not restarted.
- Changed documentation links resolve; working and cached diff checks pass.

Native UI, display-scale, audio-device, Windows, and Linux checks were not performed. Existing note/seek reconstruction limits remain unchanged.

## Envelope time multiplier display — 2026-10-19

Attack/Decay/Release Time modulation amounts display multipliers in their shared knob readouts, tooltips, endpoints, and assignment-marker tooltips. Zero displays ×1; saved −6..+6 offsets display ×1/64..×64. Note fractions retain percentage displays. Source weighting, saved settings, DSP, smoothing, limits, and amount interaction mapping are unchanged.

The regression check compares the existing shared time adapter's positions, keyboard results, drag sensitivity, endpoints, and reset value while checking multiplier formatting. Marker checks retain actual duration ranges and verify multiplier tooltips. Headless panel checks verify neutral and endpoint labels for regular/Main Envelopes in both themes.

Executed checks:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo test --workspace` passes: 913 tests, no failures, and two existing expanded time-stretch matrices ignored. All 474 UI tests pass.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the running app was not restarted.
- Changed documentation links resolve; working and cached diff checks pass.

Native UI, display-scale, Windows, and Linux checks were not performed. This presentation change does not change the audio processing path; no new device check or CLI smoke run was performed.

## Saturator — 2026-10-19

Implemented the accepted five-control Saturator through the existing effect, modulation, catalog, control, selection, drag/copy, and persistence paths. It supports tracks, Master, regular layers, and Main. Independent curves and a separate post-clip stage run through fixed 4× FIR oversampling; existing host alignment and priming compensate its constant 32-frame latency, including dry and bypass.

Portable tests cover distinct/symmetric/continuous curves, small-signal gain, bass harmonic behavior, clip ceilings, processed-only Level, exact delayed dry/bypass, all six processing rates, stereo isolation, gain silence, finite extreme input, modulation scope/bounds, reset, and rapid selector changes without gain dips. Engine tests cover live/export/block/random-access parity, Layer-vs-Track placement, Main sources, all provider types, compatible history retention, snapshot values, and zero callback allocations. Core/project tests cover atomic rejection, unknown future payloads, route identities/scopes, Save As/reopen, duplication/rate changes, and transfers through Main/layer/track chains. Headless UI checks cover dropdowns, shared knobs, modulation expansion, selection/deletion, catalog navigation, and group moves/copies in both themes.

Executed checks:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo test --workspace` passes: 923 tests, no failures, and two existing expanded time-stretch matrices ignored. All 475 UI tests pass.
- `cargo test -p daw-saturator --release -- --nocapture --test-threads=1` passes all seven DSP/quality tests.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass.
- `sh tools/package_macos.sh debug` rebuilds `target/debug/DAW.app`; the running app was not restarted.
- Changed local documentation links resolve; working and cached diff checks pass.

The representative normalized high-frequency test (input at 0.21875 × project rate, Hard Curve, +18 dB Drive, No Clip) measures the folded third harmonic at 0.34375 × project rate. Oversampling reduces it by approximately 49.4 dB against the same curve at project rate at all six rates. This measures one alias component, not universal alias suppression. A low-frequency harmonic test also verifies lower relative fifth/seventh/ninth harmonic energy for Bass Shaper than Analog Clip.

Release wall-time measurements for one stereo unit on the local macOS ARM64 development host:

| Project rate | Approximate fraction of one core |
| --- | --- |
| 44.1 kHz | 1.26% |
| 48 kHz | 1.34% |
| 88.2 kHz | 2.50% |
| 96 kHz | 2.74% |
| 176.4 kHz | 5.05% |
| 192 kHz | 5.46% |

These are isolated processor wall-time/audio-duration ratios for 0.1 seconds of input, not device deadline guarantees or a full-session benchmark. The CPU model was unavailable in the sandbox. FIR transition-band rolloff near project Nyquist is documented in the user guide. Native UI, display-scale, listening/audio-device, Windows, and Linux checks were not performed.

## Project undo and redo — 2026-10-19

Implemented [session history](../editing/undo-redo.md), superseding the original MVP exclusion. History restores project metadata and shared prepared audio together, groups pointer edits, retains stable IDs, preserves source references through Save As, and limits undo to 100 entries. Undo/redo uses shared Edit actions and keyboard shortcuts, including native macOS menu items. Text editing retains its own keys; a focused knob does not block project undo.

Completed targeted checks:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test -p daw-project -p daw-ui`: pass. Covers history grouping/no-ops/branching, history bounds, save checkpoints, import/stretch/rate restoration with removed source files, source identity after Save As, and 480 UI tests.
- `cargo test -p daw-ui history`: five interaction tests pass after the final live-transport refresh change. Covers Command/Ctrl shortcuts, exact Shift handling, repeat suppression, native-action ownership, text focus, an actual focused-knob drag, menu clicks, save/undo races, job guards, and reset on New.
- Documentation links resolve; working-tree whitespace checks pass.

Logs: `work/undo-final-clippy.log`, `work/undo-final-affected-tests.log`, and `work/undo-final-shortcut-tests.log`. Prepared audio and history remain outside the audio callback. Native menu clicks, device playback during undo, and Windows/Linux desktop behavior have not been checked interactively.

Workspace verification: `cargo test --workspace` passes 930 tests with two existing extended acceptance matrices ignored (`work/undo-workspace-tests.log`). The subsequent affected-crate run includes all 480 UI tests, including the additional focused-knob history regression. The final shortcut tests and Clippy pass after the last live-transport change.

Final verification after the worker/gesture boundary fix:

- `cargo test -p daw-ui`: all 481 tests pass, including a completed worker publication followed by a new pointer gesture in the same frame (`work/undo-final-ui-tests.log`). These remain separate undo steps.
- `cargo fmt --all --check` and workspace Clippy with warnings denied: pass.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: pass (`work/undo-cli-build.log`, `work/undo-cli.log`).
- `sh tools/package_macos.sh debug`: pass; rebuilt `target/debug/DAW.app` from the final sources (`work/undo-final-package.log`). The running app was not restarted.

## Transport shortcut focus — 2026-10-19

Space now reaches Play/Pause before focused controls or catalog folders handle it. Focused text editors retain Space for typing. Held-key repeats do not toggle playback again; Command/Ctrl/Alt combinations, window focus, disabled UI, dialogs, and blocking jobs retain their guards. Catalog folders use Enter to toggle expansion.

- `cargo test -p daw-ui`: 483 tests pass. New tests cover focused-button priority, text input, held-key repeats, modifier handling, and window focus. Updated catalog tests check that Space retains folder state and Enter toggles it.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- Local links in changed documentation resolve; `git diff --check` passes.

Native keyboard and audio-device behavior has not been checked interactively. Automated shortcut tests do not open an audio device.

Final workspace verification: `cargo test --workspace` passes 934 tests with two existing extended acceptance matrices ignored (`work/transport-workspace-tests.log`). The macOS debug package build passes (`work/transport-package.log`); `target/debug/DAW.app` was rebuilt without restarting the running app. UI and Clippy logs are `work/transport-ui-tests.log` and `work/transport-clippy.log`.

## Release transport delay investigation — 2026-10-19

A private local project with three tracks, modulated envelopes, and 17 effects reproduces a release-build transport preparation delay. At 48 kHz/512 frames, reconstruction at frame 156,160 takes about 366 ms; rendering 100 ms takes about 12.5 ms. Plan cloning takes about 15 microseconds and renderer construction about 1.5 ms. Envelope destination history forces replay from frame zero through the full processing graph. Incompatible live note edits also pause for this worker reconstruction. This is preparation latency, not proof of a steady callback overrun.

The initial development-build comparison was not representative of the reported release issue. Its profile change was removed. A sampled Saturator buffer-access change did not materially improve the project benchmark and was also removed. No build profiles, DSP coefficients, or project data were changed.

Pending Pause now sends a bounded, revision-checked callback command as well as preparing the final paused state. It stops active audio without waiting for reconstruction or falsely acknowledging the pending plan. A regression test covers pending pause, stable playhead during the pause fade, later prepared-state activation, and rejection of stale pause commands after resume.

The shared modulation entry point skips route resolution for empty connection lists while retaining destination processing and smoothing. Parameter lookup helpers can inline across crate boundaries. Tests compare note-gate behavior, including signed zero and nonfinite input, and verify smoothing after route removal. A subsequent release measurement remains about 348 ms to prepare and 12.2 ms per 100 ms of audio. This small timing difference does not establish a substantial speedup or resolve seek/note-edit preparation latency.

A headless release UI probe loaded the same project and painted 100 idle frames in each view: Timeline 79 ms, Instrument 33 ms, Editor 6 ms, Mixer 10 ms. It observed no automatic project edits. This probe excludes live device output, GPU presentation, and native input handling; it does not establish that the reported native interface delay is fixed.

A second release probe used Core Audio with a silent, in-memory copy of the same project. Opening the device took 91 ms. During playback, seek took 15 microseconds, a note edit took 176 microseconds, and the slowest subsequent UI frame took 0.66 ms. The 467 audio callbacks recorded no budget overruns; the slowest callback took 4.49 ms. This probe exercises the real output and UI update paths but excludes native window input and GPU presentation. It does not reproduce or resolve the reported visible freeze.

Private data was read only. Temporary probes and logs remain under ignored `work/808-performance/`, `work/808-release-*.log`, `work/808-release-sample.txt`, `work/808-ui-release.log`, and `work/808-playback-release*.log`. Exact arbitrary-position DSP checkpoints remain outside the implemented rendering architecture.

The extended all-view probe included tessellation but received zero audio callbacks despite successful device initialization. It is not evidence of live playback or loop-boundary behavior. It recorded a 112 ms maximum frame; no trace was captured to attribute that delay. The profiler request timed out during automatic permission review after the test process had finished.

Final checks:

- `cargo fmt --all --check` and `git diff --check`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass (`work/808-final-clippy.log`).
- `cargo test --workspace`: 936 passed, two existing extended acceptance matrices ignored (`work/808-final-workspace-tests.log`).
- `cargo test --release -p daw-output -p daw-unit-core`: 50 passed (`work/808-final-release-tests.log`).
- CLI build and `python3 tools/check_cli.py`: pass (`work/808-final-cli-build.log`, `work/808-final-cli-check.log`).
- macOS release and debug packages: pass; both bundles rebuilt with the original profile settings (`work/808-final-release-package.log`, `work/808-final-debug-package.log`).

The private-project probes are temporary investigation code, removed from the repository test sources. Windows/Linux device behavior, native window responsiveness, and the remaining seek/note-edit delay are not resolved by these changes.

### Native playback interaction follow-up

The report affects every view. A separate project copy was opened in the native macOS release app, with Master gain set to silence. The original project was not modified. Playback ran through the loop while the ruler was clicked repeatedly; four new notes were then added in the MIDI editor using Command-click. The test session was closed without saving its edits.

Temporary native frame timing separates menu dispatch, UI update, and menu refresh. During this run, Timeline UI updates remained below 5 ms and Editor UI updates remained below 3.2 ms, including seeks and note additions. Median steady Timeline updates were about 2.9 ms; Editor updates were about 0.57 ms. The first Play update took 94 ms, consistent with the earlier device-open measurement. These are application update times, not end-to-end input-to-display measurements. Native menu refresh was small; no audio-status spin was found in the captured stacks.

The visible playhead pause on a note edit is reproduced. The output callback suspends the old transport when the replacement MIDI runtime cannot inherit modulated envelope history, then waits for worker replay. A seek displays its requested position while that preparation runs. This explains a visible pause across views without requiring a long UI update. It does not rule out other intermittent native presentation stalls.

A native Timeline stack sample also found repeated waveform extrema scans as the largest UI CPU contributor. An Instrument sample instead shows widget construction, accessibility updates, and tessellation. Neither result alone establishes the cause of a long interaction stall. No speculative waveform or menu changes were retained.

Native traces and timing batches are in ignored `work/808-native-playing-sample.txt`, `work/808-native-instrument-sample.txt`, and `work/808-native-frames-17148-*.csv`. Temporary desktop instrumentation was removed, formatting and diff checks passed, and the release bundle was rebuilt from the normal sources. The existing pending-Pause fix remains; seek/note-edit history reconstruction and synchronous device opening remain unresolved.

### Continuous playback during preparation

This change supersedes the suspension behavior described above. Incompatible note and latency edits now keep the old renderer playing. Worker preparation reconstructs history and advances the shared live renderer to a future project-sample clock. Activation swaps at that exact clock. Late results retry without pausing or rewinding. A pending playing seek displays the active audio position until the prepared seek activates.

Focused output regressions pass. They cover forward/backward seeks, uninterrupted note-edit preparation, exact output after the activation fade for an LFO-modulated envelope across loop boundaries, latency changes with 44.1/48/96 kHz device conversion, full retirement queues, missed activation deadlines, stale revisions, and Pause/Resume. A thread-local allocator check confirms no allocation or deallocation during tested callback operations. The shared renderer retains modulation resolution, smoothing, and note scheduling; no DSP approximation was added.

The rebuilt macOS release app opened a silent disposable copy of the reported project at 48 kHz/512 frames. Ruler seeks were exercised in Timeline and Editor. Five notes were added during looped playback. The notes appeared, playback remained active, and Pause/Resume worked after a pending edit. The session was closed without saving. This native check does not measure end-to-end input latency or audible hardware dropouts. The original project was not modified.

History reconstruction still delays when some edits become audible; playback continues during that delay. Initial audio-device opening remains synchronous. Windows/Linux devices and sustained overload behavior remain unverified.

Validation commands and results:

- `cargo fmt --all --check`, `git diff --check`, and changed guide/spec link checks: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass (`work/808-continuous-clippy.log`).
- `cargo test --workspace`: 940 passed; two existing extended acceptance matrices ignored (`work/808-continuous-workspace-tests.log`).
- `cargo test --release -p daw-output -p daw-unit-core`: 54 passed (`work/808-continuous-release-tests.log`).
- `sh tools/package_macos.sh release`: pass; `target/release/DAW.app` rebuilt and used for the native check (`work/808-continuous-package.log`).
- CLI build and `python3 tools/check_cli.py`: pass (`work/808-continuous-cli-build.log`, `work/808-continuous-cli-check.log`).

### Seek preparation cache follow-up

The continuous-playback fix still waited for full history reconstruction before a ruler seek activated. `TransportCache` now reuses exact, worker-owned DSP snapshots, with bounded checkpoint count and cancellable prefetch. Shared audio buffers retain their original ownership. New project plans invalidate checkpoints, and cloned processors detach meter publication handles.

The private-project release probe compared five positions between checkpoints. Cached preparation took about 7–29 ms; full reconstruction at the same positions took about 191–1,128 ms. At each position, the next 1,024 stereo output samples matched exactly. Positions close to checkpoint boundaries took about 2 ms. These are preparation timings, not native end-to-end input latency. Cold preparation and initial prefetch still take time. Probe logs are `work/808-seek-cache-probe.log` and `work/808-seek-cache-offset-probe.log`.

A process-level memory comparison recorded about 87 MB maximum resident size with warmed checkpoints and 75 MB without checkpoints for this project. This includes decoded audio, manifests, and probe renderers; it is not a general cache-size limit. Logs are `work/808-seek-cache-memory.log` and `work/808-seek-cache-memory-baseline.log`. The temporary probe source was moved to ignored `work/808-performance/seek_cache_probe.rs`.

The final macOS release build was checked with the same silent disposable project copy. Three Timeline ruler seeks and two Editor ruler seeks retained active playback. Three MIDI notes were added during playback; Pause during an edit and Resume both worked. The session was closed without saving. These observations confirm native interaction and transport state, not end-to-end latency or audible hardware dropout measurements.

Final cache validation:

- `cargo fmt --all --check`, `git diff --check`, and changed documentation links: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass (`work/808-cached-clippy.log`).
- `cargo test --workspace`: 945 passed; two existing extended acceptance matrices ignored (`work/808-cached-workspace-tests.log`).
- `cargo test --release -p daw-output -p daw-unit-core`: 57 passed (`work/808-cached-release-tests.log`).
- `sh tools/package_macos.sh release`: pass; the rebuilt release bundle was used for the native check (`work/808-cached-package.log`).
- CLI build and `python3 tools/check_cli.py`: pass (`work/808-cached-cli-build.log`, `work/808-cached-cli-check.log`).

New regressions cover exact cached/uncached history with modulation and latency effects, backward seeks, cancellation, transport-mode changes, project-plan invalidation, bounded checkpoint storage, detached meter handles, and independent snapshots for every effect processor. Callback allocation/deallocation checks remain enabled in the output regressions.

### Cold seek optimization

The follow-up scope is the first uncached seek; note-edit scheduling remains unchanged. A fresh release profile of the reported project identified Saturator FIR traversal and EQ processing among the main history-reconstruction costs (`work/808-first-seek-sample.txt`). Mirrored FIR history removes per-tap wrapping. EQ retains converted parameters and filter configuration until its base values or smoothed offsets change. ThinLTO and one code-generation unit optimize the release build across shared DSP crates. Debug profile settings remain unchanged.

Focused release regressions pass for exact scalar-ring/FIR parity through wraparound, resets, and nonlinear changes, and for cached/uncached EQ parameter processing through modulation, base edits, bypass, and reset at 44.1/96/192 kHz. Tests compare sample bits and effective EQ values. All filters, ramps, and shared modulation smoothing continue to run.

The cold-seek probe constructs a new transport cache for each request and compares four positions with three trials per position. Each trial checks 4,096 stereo samples against ordinary full-history reconstruction. Baseline and optimized runs also record sample fingerprints. Project data is read from a frozen disposable copy; original media remains read-only. The probe source and binaries remain in ignored `work/`, outside the delivered source tree. Measurements cover worker preparation, not native input-to-speaker latency. Cold history reconstruction remains proportional to project complexity and requested position; no approximate seek or persisted cache was introduced.

The rebuilt native macOS release app opened the silent disposable playback copy at 48 kHz/512 frames. Playback, forward/backward ruler seeks, Pause, and Resume worked. The app was closed without saving. This confirms interaction and active transport state, not audible dropout measurements; Windows/Linux device checks remain unverified.

- `cargo fmt --all --check`, `git diff --check`, and changed documentation links: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass (`work/808-cold-clippy.log`).
- `cargo test --release -p daw-output -p daw-unit-core -p daw-saturator -p daw-equalizer`: 77 passed (`work/808-cold-release-tests.log`).
- `sh tools/package_macos.sh release`: pass; rebuilt with the saved ThinLTO release profile (`work/808-cold-package.log`).
- `cargo test --workspace`: 947 passed; two existing extended acceptance matrices ignored (`work/808-cold-workspace-tests.log`).
- CLI build and `python3 tools/check_cli.py`: pass (`work/808-cold-cli-build.log`, `work/808-cold-cli-check.log`).

The final serial comparison ran after builds and tests completed. Median preparation times across three cold trials per position were:

| Seek frame (48 kHz) | Previous release | Optimized release | Reduction |
| --- | --- | --- | --- |
| 73,923 | 179.7 ms | 153.7 ms | 14.5% |
| 156,160 | 372.8 ms | 319.8 ms | 14.2% |
| 310,423 | 745.1 ms | 639.3 ms | 14.2% |
| 447,723 | 1,061.5 ms | 924.7 ms | 12.9% |

All baseline/optimized output fingerprints matched. Logs are `work/808-cold-final-baseline.log` and `work/808-cold-final-optimized.log`; this final comparison supersedes the preliminary timing estimates. The initial disposable timing copy had an invalid MIDI source path and failed validation before rendering. That copy was corrected; original project data was not changed. The reported measurements use the corrected copy.


## Effect latency clock propagation — 2026-10-19

This resolves the two clock omissions in [architecture review F01](../architecture/review-2026-10-19/findings.md#f01-effect-clock-latency-is-incomplete). Every runtime effect forwards its shared `EffectUnit::latency_frames` report, and serial chains use the existing loop-aware clock helper after each nonzero delay. Main uses the same helper for parallel-layer latency. Bypassed processors retain their fixed delay. Zero-latency processors leave the clock unchanged.

Three new engine regression tests cover:

- Saturator → Auto-filter → Limiter → Auto-filter → Saturator → Auto-filter against direct processors with an independent signed delayed-clock reference. All six project rates, enabled/bypassed latency effects, frame-zero clamping, nonzero loop origins, and loops shorter than processor latency are included.
- Serial track placement versus the same Auto-filter on Master, with active Saturator and bypassed Limiter upstream. Check offline blocks, explicit offline reconstruction, live playback, looping, and seeks near the loop start.
- Auto-filter on Main versus track placement after unequal parallel Saturator/Limiter layer delays. Check active/bypassed delays, offline output, reconstruction, live loop wraps, and seeks near the loop start at all six rates.

The serial-chain and Main tests both failed on the original code and passed after the fix. The references contain nonzero audio; the internal Auto-filter LFO remains active. Existing shared-source modulation routing and cached provider values are unchanged. Delaying those values is a separate design concern, not established by these clock tests.

Native listening, physical device behavior, and Windows/Linux execution were not checked for this change. The corrected internal LFO phase can change rendered audio in affected saved chains; no saved settings or format migration is required.

Executed for F01 before the F05/F06 follow-up:

- `cargo test -p daw-engine effect_clock` and `cargo test -p daw-engine delayed_effect_clock`: passed.
- `cargo fmt --all --check` and strict workspace Clippy: passed.
- `cargo test --workspace`: passed; the two extended time-stretch acceptance tests remain ignored.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.
- `git diff --check` and changed-document relative file-link checks: passed.

## Exact seek preparation and memory limits — 2026-10-19

Scope: F05/F06 follow-ups to the [architecture review](../architecture/review-2026-10-19/README.md), following F01 above. The accepted behavior preserves exact processing history and permits memory-driven removal of older undo states with a visible notice.

Implemented checks cover:

- Device-independent idle checkpoints survive handoff to playback and new plan allocations that differ only in requested playhead. Warm preparation replays fewer frames and matches full-history samples. Changed processing, prepared allocations and rates reject reuse.
- Idle work stops at a buffer boundary, retains completed checkpoints, resumes to arrangement end, and releases its cursor. Checkpoint count/byte accounting and optional release remain bounded.
- Renderer estimates cover measured clone allocations for all installed effect types and modulated voices in the fixture. Shared PCM is reported separately. Existing callback allocation/deallocation, cancellation, meter isolation, live-update, loop, seek and retirement regressions remain in the workspace suite.
- History byte pressure removes oldest states, preserves current/pending/saved snapshots and working undo/redo, counts shared audio once, and releases exclusive PCM after eviction. The UI reports removals once without changing the project.
- Output diagnostics record request-to-activation timing and reject an oversized memory lease. This does not establish native peak RSS or a maximum seek delay.

Completed on macOS with the pinned toolchain and `CARGO_HOME="$PWD/work/cargo-home"`:

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test -p daw-output` — 24 passed during focused development, including callback allocation/deallocation guards and idle preparation parity.
- `cargo test -p daw-project --lib history::` — 4 passed.
- `cargo test -p daw-ui history::memory_tests` — passed after the final notice repaint change.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` — passed; WAV/MP3 formats, deterministic output, missing sources, eight-track projects, overwrite handling and exit codes.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` after the final UI change.
- Relative documentation file links — 313 checked, no missing targets. `git diff --check` — passed.

`cargo test --workspace` — **958 passed, 0 failed, 2 ignored**. The two ignored tests are the existing extended time-stretch acceptance matrices. Native device response time, CPU-saturated activation percentiles, concurrent-workload peak RSS, and Windows/Linux execution remain unverified. No user project or recording was changed.

## Seek cache profiles — 2026-10-19

Implemented Light, Balanced (default), and Responsive in application Settings through the shared engine cache policy and output preparation worker. Profiles control checkpoint bytes/count/spacing, edit quiet time, work/wait slices, and playback gating. Required requests keep priority and exact processing history. Pointer gestures and blocking jobs suppress optional construction and generation; callback budget overruns add a one-second optional-work backoff. Profile updates preserve valid snapshots and cannot change the required-request revision.

Automated regression coverage:

- Settings dropdown selects all profiles; application serialization round-trips them. Missing profile fields and unknown profile names default to Balanced while preserving known preferences.
- Cache downsizing keeps valid snapshots within count/byte policy; denser profiles retain snapshots and restart optional coverage. Large renderer estimates reduce available slots.
- Deterministic scheduling tests cover edit/release quiet periods, Light playback gating, and overrun backoff/extension.
- All profiles match full-history modulated Saturator output, with callback allocation/deallocation guards. Required requests complete despite optional editing/playback gates and a profile change.
- Optional renderer construction waits for gesture release, then checkpoint preparation resumes. Existing first-seek handoff, cancellation, live edits, loops, meter isolation, and exact cache reuse remain covered.

Completed with the pinned toolchain and local dependency cache:

- `cargo test -p daw-output` — 28 passed.
- `cargo test -p daw-engine --lib transport_cache::` — 5 passed.
- `cargo test -p daw-ui -p daw-engine -p daw-output cache_profile` — Settings and exact-history profile regressions passed during development.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app`.

- `cargo fmt --all --check` — passed.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` — passed.
- Relative documentation file links — 169 checked, no missing targets. `git diff --check` — passed.

`cargo test --workspace` — **965 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices. Native Settings interaction, device-specific seek timings, battery/thermal effects, peak RSS under load, and Windows/Linux execution remain unverified. Profile values are resource policies, not measured CPU ceilings or seek-time guarantees.

## Parallel seek cache preparation — 2026-10-19

Balanced and Responsive now prepare eligible independent tracks with two persistent workers. Light and small or resource-limited workloads retain serial preparation. Track processing, modulation, effects and Master processing reuse the live/offline DSP stages. Project-order summation preserves exact samples. Only complete track-and-Master states become checkpoints; cancellation discards partial cursors, and worker failure reconstructs through the serial path.

The [validation and benchmark report](parallel-cache-preparation-2026-10-19.md) records the design, release comparisons and limits. New regressions cover exact history across rates and buffer sizes, loops, layered/Main/Master effects, latency, mixed audio/Sampler tracks, cancellation, serial fallback, worker failure and callback allocation/deallocation guards.

Completed with the pinned toolchain and local dependency cache:

- `cargo test --workspace` — **971 passed, 0 failed, 2 ignored**. The two ignored tests are the existing extended time-stretch acceptance matrices.
- `cargo test -p daw-engine --test parallel_cache --release` — four passed.
- `cargo fmt --all --check` and strict workspace Clippy — passed.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` — passed.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app`.

Native device latency/underruns, battery/thermal behavior, and Windows/Linux execution remain unverified. Headless release measurements do not establish a maximum native seek delay.

Changed-document relative file links — 155 checked, no missing targets. `git diff --check` — passed.

## Split clips at the playhead — 2026-10-19

`Project::split_clips` splits every requested clip that strictly contains the playhead and returns the new clip IDs. The original clip keeps its ID and ends at the playhead. A missing clip or an invalid result leaves the project unchanged. Clips that do not contain the frame stay unchanged.

Audio uses `Edit::Split` and keeps the source recording, name, color, loop phase, and stretch. MIDI notes follow the application setting `MidiNoteSplit`. Split into two divides a crossing note and keeps the original note ID on the first clip. Preserve first keeps that note on the first clip and does not continue it. Non-looping MIDI writes a new pattern only for a piece whose notes changed, so other clips that share the original pattern stay unchanged. A looping MIDI clip with a crossing occurrence becomes two unlooped patterns. The Edit command, Command+T on macOS or Ctrl+T on Windows/Linux, and the clip context menu share that operation. The command is available in Timeline and Editor and is disabled when no selected clip contains the playhead. Settings stores the MIDI choice on this computer; a missing or unknown value means Split into two.

Completed with the pinned toolchain:

- `cargo test -p daw-core --lib split::` — 4 passed.
- `cargo test -p daw-ui --lib -- clip_split midi_note_split menu_shortcuts midi_clip_context_menus` — 5 passed. These cover command enablement, a group split, the Edit menu with Preserve first, the Settings dropdown, shortcut labels, and the Timeline/Editor context menu.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace` — **978 passed, 0 failed, 2 ignored**. The two ignored tests are the existing extended time-stretch acceptance matrices.

Relative links in the edited documents — 212 checked, no missing targets. `git diff --check` — passed.

Native macOS menu Command+T, Windows/Linux Ctrl+T, device playback of a split, and the Settings dropdown in a running app remain unverified. Automated egui tests do not establish native menu or pointer behavior.

## Time-range selection — 2026-10-19

Command-drag on macOS, or Ctrl-drag on Windows and Linux, selects one time span across the lanes it touches. The span can include partial clips, several clips, and silence. The pointer uses the same crosshair as creating a loop range while that gesture is available and while the span is being drawn. Dragging the span moves its contents. Option-drag on macOS, or Ctrl-drag that starts inside the span on Windows and Linux, duplicates it. Copy keeps the span start as the paste anchor, so leading silence and gaps stay. Delete removes only that time. A span that contains only silence clears the highlight. Failure leaves the project unchanged.

Audio and MIDI pieces use `Clip::retained_range`. MIDI crops the source window and leaves the shared pattern unchanged. Timeline and Editor share the gesture.

Completed with the pinned toolchain and `CARGO_HOME="$PWD/work/cargo-home"`:

- `cargo test -p daw-core --lib portion` — 3 passed.
- `cargo test -p daw-ui --lib clip_portion` — 2 passed. These cover a range across two clips and silence, copy, move, delete, Option/Ctrl duplication, the Editor gesture, and the crosshair during the drag.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace` — **983 passed, 0 failed, 2 ignored**. The two ignored tests are the existing extended time-stretch acceptance matrices.

Relative links in the edited documents were checked, with no missing targets. `git diff --check` — passed.

Native pointer cursors, Windows/Linux Ctrl disambiguation, and device playback of a moved range remain unverified. Automated egui tests do not establish native pointer behavior.

## Clip settings — 2026-10-20

Timeline and Instrument share a Clip settings column with the Effects catalog and the Instrument components catalog. Opening one closes the others. All three can be closed. The panel edits the primary selected clip through `Edit::SetClipName`, `Edit::SetClipGain`, and `Edit::SetClipTranspose`.

Labels and values use the 11-point catalog text and are left aligned. Source is read-only and uses the secondary text color. Gain shows a dB suffix, for example `0.0 dB`. Transpose shows a semitones suffix, for example `0 semitones`. Gain and transpose drag vertically; Shift makes the step ten times finer. Double-click opens a text field. Enter commits. Escape or focus loss restores the previous value. Audio and MIDI playback apply clip gain before track or Main gain. MIDI transpose shifts note-on pitch and clamps to 0–127. Audio transpose is saved and validated. Playback does not pitch-shift the recording.

Completed with the pinned toolchain and `CARGO_HOME="$PWD/work/cargo-home"`:

- `cargo test -p daw-ui --lib clip_settings` — 1 passed. It covers catalog exclusivity, the suffixed labels, 11-point text, the dimmed source, drag steps, typed edits, and rejection of invalid text.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace` — **987 passed, 0 failed, 2 ignored**. The two ignored tests are the existing extended time-stretch acceptance matrices. This run includes clip-setting persistence, audio gain, and MIDI transpose/gain playback.

Relative links in the edited documents — 415 checked, no missing targets. `git diff --check` — passed.

Native pointer drags, device playback of clip gain and MIDI transpose, and Windows/Linux execution remain unverified. Automated egui tests do not establish native pointer behavior.

## Clip settings spacing — 2026-10-20

Clip settings uses `ControlPanel::padded_body` for its content inset and available width. Form rows and catalog rows share one height calculation. Vertical padding belongs to each row, removing the extra gap between the header and the first form label. The empty-selection prompt uses the same geometry. Existing field alignment and editing behavior remain shared.

The painted-layout regression compares the four form label positions with the four collapsed Effects catalog folder rows, relative to their headers. It also checks the left inset and empty-selection prompt. Timeline passes in Light and Dark; comparisons allow half a logical point for text rounding. A separate regression verifies that Instrument has no toggle or panel, leaving Timeline closes the panel and discards a draft for Instrument/Editor/Mixer, and returning retains clip selection without a project edit. This supersedes the shared Timeline/Instrument visibility described in the original Clip settings entry above.

The initial workspace run caught a catalog scroll-bounds regression from raw fractional font metrics. The shared helper now uses the catalog's pixel-rounded galley measurement; the existing scrolling regression passes without changes to its assertions.

Completed checks:

- `cargo test -p daw-ui clip_settings` — two passed for the spacing change before the Timeline-only follow-up. Final availability checks are recorded below.
- `cargo test -p daw-ui` — **493 passed** after the Timeline-only follow-up and row-rounding correction, including all three Clip settings regressions.
- Final `cargo test --workspace` — **989 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching the app.

Native display-scale inspection and Windows/Linux execution remain unverified. Automated painted-layout checks do not establish native rendering behavior.

Relative file links in the affected guides, specifications and repository instructions — 279 checked, no missing targets. `git diff --check` — passed.

## Audio clip transpose — 2026-10-20

Audio clips now apply their saved constant transpose through the selected Rubber Band R3 or Integrated engine. This supersedes the earlier inactive audio-transpose limitation. The media `ClipAudioKey` and common lookup serve Session and RenderPlan; preparation delegates to the existing `PreparedStretch::with_pitch_cancellable` used by Sampler. No portable DSP algorithm or processing revision changed, and no Rubber Band implementation source was inspected or reused.

The clip cache distinguishes asset, reduced duration ratio and semitone offset. Prepared entries additionally validate source allocation, processing rate, selected engine/revision and pitch revision. Matching requests share PCM/peaks; stale or absent transformed entries cannot use the unshifted original. Processing starts from the original decoded source, retaining clip duration, source coordinates, repeats, gain and IDs. Unstretched zero pitch restores the original allocation. Existing saved nonzero values now affect playback; missing values remain zero.

The existing Sampler edit worker transaction is generalized to `audio_edits::prepare_audio_edit` and `Job::PreparedEdit`. Clip pitch and Sampler requests use the same atomic publication path. Audio pitch drags preview until release; MIDI drag behavior remains unchanged. Failure, cancellation and discarded receivers cannot publish partial project/audio state. Callback playback only reads prepared buffers.

Focused automated checks completed:

- `cargo test -p daw-project --test audio_transpose` — four passed. Both engines, mono/linked stereo, ±12-semitone tone checks, fixed and combined stretch duration, live seek/offline/direct rendering, same/different-pitch cache reuse, source/engine/rate invalidation, source-file safety, save/reopen, Float32 export, undo/redo, trim/repeat coordinates, cancellation/failure and finite ±48-semitone endpoints are covered. Final workspace execution also checks repeated pitch edits against the original source.
- `cargo test -p daw-ui audio_clip_transpose` — two passed. These cover release-only preparation, atomic worker publication, retained playhead/selection, undo/redo buffer reuse, Escape cancellation, rejected late worker results and failed preparation without a project edit.
- `cargo test -p daw-output prepared_audio_clip_transpose` — passed for both engines. Prepared plan activation and sample processing pass the allocation/deallocation guard and match the reference after the existing transition ramp.
- `cargo test --workspace` — **996 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices.
- `cargo fmt --all --check` and strict workspace Clippy — passed.
- CLI build and `python3 tools/check_cli.py` — passed, including WAV/MP3 codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite handling and exit codes.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching the app.

Relative file links in the changed user guides and specifications — 124 checked, no missing targets. `git diff --check` — passed.

Native listening, physical-device behavior, native pointer gestures and Windows/Linux execution remain unverified. Synthetic signal and headless callback tests do not establish broad sound-quality or device-latency acceptance.

## Group Clip settings — 2026-10-20

Clip settings now reads the full clip selection through the shared snapshot helper. Name and Source are disabled and show “multiple values” whenever more than one clip is selected, including matching names/sources. Gain and Transpose show a common value or “multiple values” and set one absolute value across the selection. Dragging starts from the primary value and reuses the existing steps, Shift sensitivity, parsers and bounds.

The existing typed clip edits are collected into one `Edit::Batch`, excluding unchanged clips, and submitted through the shared UI edit path. Group gain and MIDI-only transpose keep live updates and one gesture undo step. Any selection containing audio previews transpose until release; its complete audio/MIDI batch uses the existing atomic preparation worker and shared cache. Failed preparation retains every clip and the previous audio. Field drafts, audio previews and pointer identity include selection membership so an old interaction cannot affect a replacement group. This supersedes the original primary-only Clip settings scope.

Completed checks:

- `cargo test -p daw-ui clip_settings` — **11 passed**, including six new group regressions. Coverage includes mixed/common values, disabled name/source editing, cross-track gain and transpose, unchanged-primary commits, unselected-clip retention, drag steps and clamping, single-step undo/redo, MIDI-only live transpose, both audio engines, shared prepared buffers, mixed audio/MIDI publication, failed preparation and selection-change cancellation.
- `cargo test -p daw-ui` — **501 passed** after the final pointer-identity guard, including all Clip settings regressions.
- `cargo test --workspace` — **1002 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching the app.

Relative file links in the affected guide and specification — four checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback and Windows/Linux execution remain unverified. Automated egui interaction tests do not establish native input or display behavior.

### Selection message correction

Name and Source now show the exact text “Multiple clips selected” for group selections. This supersedes their “multiple values” text above. Gain and Transpose show the exact text “Multiple values” when their numeric values differ. This supersedes the lowercase label above. The existing painted-text regression verifies both messages and the common-value case.

For this text-only correction, `cargo test -p daw-ui clip_settings` — 11 passed; `cargo fmt --all --check` and strict workspace Clippy — passed. `sh tools/package_macos.sh release` — passed; rebuilt the app without relaunching it. `git diff --check` — passed. The full workspace result above is from the group-edit implementation before this wording correction.

## Relative Clip settings drags — 2026-10-20

Gain and Transpose drags now apply a common offset to selected clips, preserving differences in dB and semitones. This supersedes the earlier absolute group-drag behavior; typed commits retain their absolute behavior. Both paths use the shared gain/transpose edit builders, one `Edit::Batch`, and the existing UI edit and audio preparation paths. A common bound stops group movement before individual clipping can change the differences; reversing direction resumes at the applied value. Silent clips retain their gain when selected with audible clips. All-silent selections retain the existing upward-drag behavior.

Audio transpose previews the bounded group offset until release, retaining “Multiple values” for differing clips. Release prepares every resulting pitch together; a return to zero offset creates no job or edit. Gain and MIDI-only transpose keep live updates and one gesture undo step. No project schema, processor or cache identity changed.

Completed checks:

- `cargo test -p daw-ui clip_settings` — **14 passed**. Updated regressions verify relative gain and MIDI/audio transpose, Shift steps, unselected-clip retention, one-step undo/redo and distinct prepared buffers for distinct resulting pitches across both engines. Three new regressions cover common gain/transpose bounds, reversal without overshoot, mixed/all-silent gain, mixed-value preview labels and zero-offset audio release. Existing absolute text commits, selection guards and failed/cancelled preparation also pass.
- `cargo test --workspace` — **1005 passed, 0 failed, 2 ignored** before the live-feedback follow-up below. The ignored cases are the existing extended time-stretch acceptance matrices.
- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching it.

Relative file links in the affected guide and specification — four checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback and Windows/Linux execution remain unverified. Automated egui tests do not establish native input or display behavior.

### Live drag feedback

Mixed fields now show the signed cumulative applied change beside “Multiple values” during a drag. Gain displays two decimal places for fine steps; Transpose displays whole semitones. The shared `DragChange` state follows the field and selection, records bounded changes only, and clears on release or cancellation. It also covers staged audio previews without publishing the project early.

Final `cargo test -p daw-ui` — **505 passed**, including all 15 Clip settings regressions. Painted-label checks verify positive/negative fine gain, live MIDI transpose, both audio engines, bounded changes without overshoot, release cleanup and the full readout fitting the control width. Final formatting, strict workspace Clippy and the macOS release rebuild — passed. This UI follow-up did not repeat the full workspace run recorded above. Native/device/platform limits remain as stated above.

### Signed-only group drag readout

Group drags now show only the signed applied change, such as `+1.00 dB` or `−2 semitones`, through the same `DragChange` formatter and shared numeric field controls. This replaces the prefixed readout above. It applies to matching and differing starting values; release restores the shared value or “Multiple values”. Single-clip fields retain their normal numeric display.

`cargo test -p daw-ui` — **505 passed**, including all 15 Clip settings tests. Existing painted-label regressions now require the signed-only text; the group readout test also covers matching gain/transpose values and their idle restoration. Formatting, strict workspace Clippy, the macOS release rebuild, and `git diff --check` — passed. This display refinement did not repeat the full workspace run. Native/device/platform limits remain as stated above.

## Compressor peak reduction marker — 2026-10-20

Compressor now holds the maximum sample-level gain reduction in shared `unit-core::PeakMeter` storage through `UnitMeter`. The existing output binding path retains handles by effect ID across track, Master, regular-layer and Main chains. A 1-point secondary-color line marks the held value on the existing 0–24 dB rail. Primary click resets only the hold; current readings, compression and project settings stay unchanged. Active processing can immediately capture another peak. Bypass clears both readings and suspends publication. Holds survive pause, stop, seek and compatible renderer replacement, but are not saved.

Binding a prepared or seek-reconstructed processor initializes only the current cell. It cannot raise the hold from unplayed processing history. Snapshot clones detach handles. Peak storage normalizes invalid readings and negative zero before its atomic maximum update. Legacy scalar attachment APIs retain control-thread adapters. Audio processing and the project format are unchanged.

Completed checks:

- `cargo test --workspace` — **1009 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices. This run started before the final prepared-attachment and negative-zero safeguards; the final affected-crate run below covers those changes.
- Final `cargo test -p daw-unit-core -p daw-compressor -p daw-output -p daw-ui` — **584 passed, 0 failed**. Coverage includes short peaks between refreshes, independent reset, detached snapshots, prepared attachment, invalid readings, bypass clearing/resume, stable-ID retention and independent track/Master/layer/Main holds. Compressor audio matches an unmonitored reference at all six project rates. Existing callback allocation/deallocation guards cover track, Master and layer processing with peak monitoring attached.
- The UI regression checks both themes, exact secondary-color marker geometry, display saturation above 24 dB, primary-click reset, disabled/focus/secondary-click guards, bypass clearing and no parameter-change response.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` — passed on the final source.
- CLI build and `python3 tools/check_cli.py` — passed, including WAV/MP3 codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite handling and exit codes.
- `sh tools/package_macos.sh release` — passed on the final source; rebuilt `target/release/DAW.app` without relaunching the app.

Relative file links in the changed guide, Compressor specification and architecture document — 75 checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback/listening and Windows/Linux execution remain unverified. Automated painting and headless callback tests do not establish native-device or platform acceptance.

## Limiter peak reduction marker — 2026-10-20

Limiter now uses the same `PeakMeter`/`UnitMeter` storage and stable-ID output binding as Compressor. Both UI meters share peak painting, reset guards, tooltip and accessibility through `meters::GainReduction`. Limiter retains its 4-point rail and control layout. Its 1-point secondary-color line holds the maximum reduction until primary-click reset or bypass. Peak capture continues while hidden; pause, stop, seek and compatible edits retain the hold. Reset changes no settings or current reading, and active processing can immediately set another peak. Prepared attachment and detached snapshots keep background reconstruction out of the hold. No audio, latency or project-format change is introduced.

Focused automated checks completed:

- `cargo test -p daw-limiter held_reduction -- --nocapture` — passed. All six project rates, Peak/True Peak and L/R/M/S combinations match an unmonitored audio reference and latency. The test covers sample-level maxima, recovery, independent reset, compatible retarget/reset retention, bypass clearing/suspension and re-enable.
- `cargo test -p daw-ui meters::tests` — **7 passed**. Shared tests cover both rail layouts and both themes, secondary-color geometry, display saturation, click reset, unchanged current readings, disabled/focus/secondary-click guards and no parameter-change response. A Limiter panel test confirms click wiring, unchanged settings, and off-state clearing in compact/collapsed views.
- `cargo test -p daw-output gain_reduction_meter_tests` — **2 passed**, extended to both Compressor and Limiter. Track, Master, regular-layer and Main bindings retain independent holds and stable handles. Callback sample processing passes the existing allocation/deallocation guard. Stop retains held readings; Limiter's primed current cell remains separate from transport-masked display state.
- `cargo test --workspace` — **1012 passed, 0 failed, 2 ignored**. The ignored cases are the existing extended time-stretch acceptance matrices.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` — passed. CLI build and `python3 tools/check_cli.py` — passed, covering codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite handling and exit codes. `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching it.

Relative file links in the changed guide, Limiter specification and architecture documents — 135 checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback/listening and Windows/Linux execution remain unverified. Automated painting and headless callback tests do not establish native-device or platform acceptance.

## OTT band peak reduction markers — 2026-10-20

Each OTT band now holds its greatest reduction independently and shows a 1-point secondary-color line on the reduction side of its existing signed rail. Clicking a rail resets only that band's hold. Boost cannot raise a reduction hold. Current gain values, settings, signed fills, center markers, disabled-band colors and layout remain unchanged. Holds survive hidden/compact/collapsed presentation, individual band bypass, pause, stop, seek and compatible updates; whole-unit bypass clears and suspends all three. They are temporary monitoring state and are not saved.

Shared `PeakHold` supplies capture/reset for scalar `PeakMeter` and signed `BandMeter`; both remain behind `UnitMeter`. Band storage retains the current atomic array for legacy attachments and EQ. Existing stable-ID discovery and track/Master/layer/Main binding, unbound preparation, detached snapshots and callback retirement remain shared. UI views reuse the same reset guards, peak painter, tooltip and accessibility helpers. No audio arithmetic, latency, settings schema or project format changes.

Focused checks completed:

- `cargo test -p daw-unit-core meter::tests` — **4 passed**, covering scalar and signed band hold capture, independent reset, boost exclusion, invalid/negative-zero readings, prepared attachment, detached snapshots and bypass suspension/resume.
- `cargo test -p daw-ott band_reduction_holds` — passed. At all six project rates in Peak/RMS, monitored samples equal an unmonitored reference. Captured peaks match maxima over every processed reading. Independent reset, DSP reset/compatible retarget retention and bypass clearing/resume are covered.
- `cargo test -p daw-ui band_peak_lines` — passed. Both themes and each band cover line color/position, display saturation, signed current retention, reset isolation, unchanged settings, disabled-band operation and whole-unit clearing in compact/collapsed presentation.
- `cargo test -p daw-output meter` — **8 passed**. Existing callback heap guards now cover band monitoring on track, Master, layer and Main paths. Tests cover stable handle reuse, independent reset, removal/copy identity, stop retention and whole-unit clearing/suspension, alongside EQ, Compressor and Limiter regressions.
- `cargo test --workspace` — **1015 passed, 0 failed, 2 ignored** on the final source, including all 508 UI tests. The ignored cases are the existing extended time-stretch acceptance matrices.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` — passed on the final source. CLI build and `python3 tools/check_cli.py` — passed, including codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite handling and exit codes. `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching it.

Relative file links in the changed guide, OTT specification and architecture documents — 134 checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback/listening and Windows/Linux execution remain unverified. Automated painting and headless callback tests do not establish native-device or platform acceptance.

## OTT band peak boost markers — 2026-10-20

Each OTT band now holds peak boost as well as peak reduction. This extends the reduction-only markers above: a rail click now resets both holds for that band and retains every other band's holds. Both lines use the shared 1-point secondary-color painter, with reduction left of center and boost right of center. Each display side saturates at 24 dB; tooltips retain actual held values. Current readings, signed fills, band colors, layout and settings stay unchanged. Holds retain the existing hidden/compact/collapsed, band bypass and transport behavior. Whole-unit off clears all six holds and suspends capture.

`BandMeter` composes the existing `PeakHold` for both polarities behind `UnitMeter`. Publication records each sign into only its corresponding hold. Preparation initializes current cells without raising either hold, and DSP snapshots stay detached. UI reset and off-state masking use the same helpers for both holds. Existing output discovery, binding, stable IDs, legacy adapters and callback retirement stay unchanged. No audio arithmetic, latency, saved settings or project format changes.

Focused checks completed:

- `cargo test -p daw-unit-core meter::tests` — **4 passed**. Extended signed-band tests cover both maxima, polarity isolation, independent reset, invalid/zero readings, detached snapshots, prepared attachment, bypass suspension and resume.
- `cargo test -p daw-ott band_peak_holds` — passed. Loud, silent and quiet signals at all six rates in Peak/RMS produce identical monitored/unmonitored samples. Both held values match maxima over processed readings. Paired band reset, retarget/reset retention and bypass clearing are covered.
- `cargo test -p daw-ui band_peak_lines` — passed. Each band in both themes checks both lines, secondary color, position and endpoint saturation, paired reset with other-band isolation, unchanged current/settings, disabled-band operation and compact/collapsed off-state clearing. Disabled UI, window focus loss and secondary clicks cannot reset holds; a valid reset removes both lines.
- `cargo test -p daw-output meter` — **8 passed**. Extended callback tests capture boost, retain other bands/current/reduction when resetting a boost hold, retain holds through Stop and preserve stable bindings. Shared callback heap guards and bypass clearing/suspension cover both hold arrays, alongside EQ, Compressor and Limiter regressions.
- `cargo test --workspace` — **1015 passed, 0 failed, 2 ignored** on the final source, including all 508 UI tests. The ignored cases are the existing extended time-stretch acceptance matrices.

`cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` — passed. CLI build and `python3 tools/check_cli.py` — passed, including codecs/bitrates, repeatability, missing sources, eight-track projects, overwrite handling and exit codes. `sh tools/package_macos.sh release` — passed; rebuilt `target/release/DAW.app` without relaunching it.

Relative file links in the changed guide, OTT specification and architecture documents — 134 checked, no missing targets. `git diff --check` — passed.

Native pointer behavior, physical-device playback/listening and Windows/Linux execution remain unverified. Automated painting and headless callback tests do not establish native-device or platform acceptance.

## Track group buses — 2026-10-20

Implemented [nested track group buses](../editing/track-groups.md) with stable parent IDs, atomic hierarchy edits, collapse, shared effects/mix controls/meters, and Timeline track membership drops. Group/Ungroup use Command+G / Command+Shift+G (Ctrl equivalents on Windows/Linux) through the same guarded actions as the menu. Group creation, movement, ungroup, deletion and empty-track duplication retain the existing undo and source ownership behavior.

Regression coverage includes hierarchy/cycle rejection and transaction rollback, cross-parent grouping, solo ancestor paths, nested sum/gain/pan, nonlinear shared processing, child/group/Master latency alignment and clocks, exact audio bus cold seeks, serial/two-worker history parity under loops, collapse cache reuse, callback allocation-free retargeting and group peaks, save/reopen/render parity, unchanged source recordings, keyboard focus/repeats, pointer release/cancellation, and shared effects targets/transfers. The existing Track menu test viewport now includes its additional group commands. Ordinary track copies retain their existing owner-scoped modulation route identities.

Completed checks:

- Focused core, engine, project workflow, group UI, and track shortcut regressions pass. The project workflow run passes all 60 tests, including modulation identity preservation during ordinary duplication.
- `cargo fmt --all --check`, `git diff --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass with the configured local Cargo cache.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass. CLI smoke coverage now includes validation and byte-identical unity-group export against the flat mix.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release` rebuilds `target/release/DAW.app` successfully.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --workspace` passes: 1034 tests pass, no failures, and 2 pre-existing full timestretch acceptance matrices remain ignored.

These checks run on macOS. No native gesture, menu key-equivalent, audio-device continuity, Windows, or Linux check was performed for this change. Group controls and shortcuts have automated egui input coverage; that does not establish native device or platform behavior.

## Nested track group panels and header dragging — 2026-10-20

Timeline child panels now sit inside the group's container with a shared-padding left inset. The complete control panel moves, including the name, mix controls, and meters. Group selection outlines span visible descendants; child and group right/bottom edges coincide. The workspace supplies the root left border, so top-level groups do not paint an adjacent second stroke. Nested panels use the shared left-divider painter. Timeline lanes retain their fixed x-position.

Drop scope follows the inset header position. A child reordered at its bottom edge retains its parent; group append previews use the full container bottom. Offscreen placeholders retain panel inset and subtree outline geometry without changing project data.

Track and group headers omit the visible grip. The name fills the recovered space and retains the existing name/blank-header drag handler, grab cursor, selection, rename, previews, edge scrolling, release-only commit, and cancellation. Group collapse and mix controls keep their separate interactions. Instrument layer grips are unchanged.

- `cargo test -p daw-ui --lib`: all 518 tests pass. Added geometry/input coverage for dark/light themes, 1×/1.25×/2× scale, nested and collapsed groups, shared bottom/right boundaries, absence of a duplicate root left stroke, offscreen headers/virtual children, and before/after drop scope. Header tests cover name alignment, track reorder/cancellation, renaming, unchanged mix-control positions, and moving complete group subtrees from the name or blank area.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, and `git diff --check` pass with the configured local Cargo cache.
- `cargo test --workspace`: 1038 tests pass, no failures, and 2 pre-existing extended timestretch acceptance cases remain ignored.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` successfully with the final header layout.
- Relative documentation links: 96 checked, no missing targets.

No native gesture or platform/device check was performed for this visual change. Audio processing and saved project data are unchanged.

## Group and Master header alignment — 2026-10-20

Group and Master labels now align left in Timeline and Mixer through the shared `rows::left_aligned` helper. Group rename fields retain left alignment. Header rows retain their full height so lower controls do not move. Group disclosures use a borderless control with the same chevron painter, icon size, and idle/hover colors as catalog folders. Their existing hit area, tooltip, expanded-state accessibility, collapse edits, and separation from header dragging remain intact. Catalog folder layout and interaction stay unchanged.

- `cargo test -p daw-ui --lib track_groups`: all 11 tests pass. Added dark/light, Timeline/Mixer checks for title positions, borderless disclosure geometry, both collapse states, release behavior, and retained drag isolation. Existing group movement, nesting, and shortcut checks pass.
- `cargo test -p daw-ui --lib`: all 519 tests pass on the final layout, including retained Master gain-control positions and existing catalog folder navigation/interaction checks.
- Formatting, strict workspace Clippy, and diff checks pass. Relative documentation links: 72 checked, no missing targets.
- `cargo test --workspace`: 1039 tests pass, no failures, and 2 pre-existing extended timestretch acceptance cases remain ignored.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` successfully with the final header alignment and fixed row heights.

Native pointer and platform/device checks were not performed. Audio processing and saved project data are unchanged.

## Audio, MIDI, and Master header icons — 2026-10-20

Audio, MIDI, and Master headers now show Lucide Audio Lines, Keyboard Music, and Boom Box in Timeline and Mixer. They reuse the group disclosure's 28×22-point slot, 16-point artwork painter, secondary idle color, hover/focus color, and 6-point name gap. All track names and rename fields align left after that slot. Master reuses the same icon and label row in both views, while its panel and gain controls retain their positions. Timeline audio/MIDI icons use the shared guarded track drag handler, including release-only commit and cancellation; group chevrons retain their separate collapse action. Mixer icons identify track type without adding a new reorder gesture. The shared stereo-panel layout now reserves its full content width, keeping Master's meters at the same right inset as track and group meters even when its header is short.

Icon regressions inspect the actual rendered textures and bounds in both themes and both views, at 1× and 1.5× scale, including nested audio/MIDI tracks and pinned Master headers. Input regressions cover both type icons, uncommitted previews, moves, and Escape cancellation. Meter-right-edge assertions failed for Master before the shared width fix and now pass for all four header kinds in both themes and at both scales. Existing header tests now target the shifted name text and blank name area; Mute/Solo positions and clip selection remain covered. No project data, audio processing, or dependencies changed.

- `cargo test -p daw-ui --lib track_and_master_icons`: passed on the final geometry, including Boom Box on Master.
- `cargo test -p daw-ui --lib`: all 521 tests pass with the Master icon, track-icon drag/cancellation, renaming, mix controls, selection, and catalog regressions.
- `cargo test --workspace`: 1041 tests pass, no failures, and 2 pre-existing extended timestretch acceptance cases remain ignored. This final rerun includes the shared meter-width fix and all icon/alignment regressions.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass. Relative documentation links: 99 checked, no missing targets.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` successfully with all three icons and the shared meter-width fix.

Native pointer and Windows/Linux checks were not performed. Automated egui checks do not establish native gesture behavior.


## Reject clip drops onto track-group buses — 2026-10-20

Clip move previews mark group-bus destinations invalid before release. The shared preview painter shows the forbidden cursor before Move/Copy feedback. The shared release path rejects the complete incoming set if any destination is invalid, without a project edit, preparation job, selection change, undo entry, or error dialog. Audio and MIDI clips use the same path. Valid child-track drops and error reporting for unrelated preparation/model failures retain their behavior.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test -p daw-ui --lib track_groups`: all 16 tests pass. Added pointer regressions cover audio/MIDI moves and copies onto expanded/collapsed buses, multi-track selections where a secondary clip targets a bus, forbidden-cursor precedence over Copy, and recovery to a valid child track within the same drag.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass with the configured local Cargo cache.
- `cargo test --workspace`: all 1044 tests pass, including 524 UI tests. Two pre-existing extended timestretch acceptance cases remain ignored. The existing failed-placement regression still reports unrelated model/preparation errors and retains the original tracks.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` successfully.
- Relative links in the four changed documentation files: 69 checked, no missing targets.

Native pointer behavior and Windows/Linux checks were not performed. Automated egui input checks verify cursor output and edit behavior; they do not establish native cursor rendering. Audio processing and saved project data are unchanged.


## Audio-file drag rejection on group buses — 2026-10-20

External file hover now uses the existing rejected `file_target` result to show the forbidden cursor. Release continues through the same target lookup: no target means no import, selection change, undo entry, or error dialog. Other unsupported file targets receive the same feedback; existing Sampler targeting stays separate.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test -p daw-ui --lib track_groups`: all 18 tests pass. New input tests cover group headers and lanes, expanded/collapsed buses, loading/ready previews, direct drops, unchanged project/selection/history, no import job or error dialog, and returning to an audio track while retaining the prepared audio buffer.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass with the configured local Cargo cache.

- `cargo test --workspace`: 1046 tests pass, including all 526 UI tests; no failures. Two pre-existing extended timestretch acceptance cases remain ignored. Existing file import, snapping, overwrite, failed-import, and Sampler tests pass.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` successfully.
- Relative documentation links: 69 checked, no missing targets.

Native operating-system file-drag cursor rendering and Windows/Linux checks were not performed. Automated egui checks cover cursor output and drop behavior. Audio processing and saved project data are unchanged.


## Audio-file drag rejection on MIDI tracks — 2026-10-20

The shared `file_target` lookup and forbidden-cursor feedback already reject MIDI headers and lanes. Added explicit regression coverage and documentation for that existing behavior. The tests reuse the group-bus rejection/recovery assertions and file-frame helper; production code is unchanged.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test -p daw-ui --lib audio_file_`: all 4 tests pass. Cases cover root/grouped MIDI tracks with an existing pattern and selection, headers/lanes, direct drops, prepared previews, clearing an earlier valid audio target, unchanged project/selection/history, no import job or error dialog, and returning to an audio track with the same prepared buffer. Existing expanded/collapsed group-bus cases use the same assertions and pass.

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass with the configured local Cargo cache.
- `cargo test --workspace`: 1048 tests pass, including all 528 UI tests; no failures. Two pre-existing extended timestretch acceptance cases remain ignored.
- Relative documentation links: 68 checked, no missing targets.

Native operating-system cursor rendering and Windows/Linux checks were not performed. The existing release app already contains the shared behavior; this test/documentation change does not alter the runtime.


## Existing clip drops onto incompatible track types — 2026-10-20

The earlier MIDI file-drop tests cover external files. Existing clip moves/copies had a separate gap: resolved previews showed an invalid destination, but release used raw previews that rejected only group buses. A new input regression reproduced `Clip source type does not match track type` on release before the fix.

Raw move previews now use core `TrackKind::accepts_asset`, also used by project validation. The existing invalid-preview guard rejects the entire move/copy before editing or preparation, without an error dialog. Other structural and preparation failures retain their reporting paths. External-file behavior stays covered by the shared file-drop tests.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test -p daw-core placement`: all 6 matching tests pass, including atomic rejection of mixed valid/incompatible placements, moves/copies, both source types, and existing invalid MIDI placement coverage.

- `cargo test -p daw-ui --lib track_groups`: all 24 tests pass. New cases cover both audio/MIDI directions, moves/copies, root/grouped tracks, a secondary incompatible group member, recovery to a valid track, and retained missing-asset diagnostics. The first test failed before the fix with the exact reported dialog.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass with the configured local Cargo cache.

- `cargo test --workspace`: 1053 tests pass, including all 532 UI tests; no failures. Two pre-existing extended timestretch acceptance cases remain ignored. Existing file import, clip overwrite, new-track placement, cancellation, and failed-preparation checks pass.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` with the production fix.
- Relative documentation links: 68 checked, no missing targets.

Native pointer behavior, operating-system cursor rendering, and Windows/Linux checks were not performed. Automated input tests do not establish native gesture behavior.


## Clip header drop targeting — 2026-10-20

Expanded the source-type rejection regression to track headers. It failed before the fix because clip drags returned the source track when the pointer left the timeline lane, producing `Grabbing` instead of `NotAllowed` over an incompatible header.

Clip moves and external file drops now share `placement::drop_bounds` and `clip_drop_track`. Headers and timeline lanes identify the same destination row, so the existing core source-type policy and invalid-preview release guard reject incompatible header drops without a dialog. Valid headers use the existing placement transaction and copy identity rules. File drops keep their existing snapping, import, and unsupported-target behavior.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test -p daw-ui --lib track_groups`: all 25 tests pass. Expanded coverage includes both audio/MIDI directions, header/lane targets, moves/copies, grouped tracks, expanded/collapsed buses, recovery from invalid headers to valid lanes, and accepted compatible audio/MIDI header drops.

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`: pass with the configured local Cargo cache.
- `cargo test --workspace`: 1054 tests pass, including all 533 UI tests; no failures. Two pre-existing extended timestretch acceptance cases remain ignored. Existing file-drop, clip overwrite, group selection, cancellation, new-track placement, and Sampler checks pass.
- `sh tools/package_macos.sh release`: rebuilt `target/release/DAW.app` with the shared header/lane target lookup.
- Relative documentation links: 68 checked, no missing targets.

Native pointer behavior, operating-system cursor rendering, and Windows/Linux checks were not performed. Automated egui checks verify cursor output and project edits.


## Integrated bass analysis and attack detection — 2026-10-21

Integrated processing revision 3 reuses the full-source linked power analysis, phase vocoder, overlap schedule, guarded expansion map, and pitch composition. Broadband material retains the short window. Tonal sources with at least half their non-DC spectral power at/below 250 Hz select a nominal 320 ms power-of-two window. The shared attack detector uses 80 preceding approximately 1 ms observations, with populated-count startup averaging; it retains the recent-rise threshold and 30 ms refractory period. No normalization, clipping, channel summing, separate pitch processor, or new dependency was added. See [algorithm and migration](../audio/timestretch.md#bass-aware-analysis--revision-3) and [independent provenance](../../crates/timestretch/PROVENANCE.md#bass-analysis-and-attack-detection--2026-10-21).

Saved Integrated revision-2 projects explicitly upgrade on open before preparation. A warning reports the possible sound change. Complete clip and Sampler preparation uses the new engine identity through existing workers/cache checks. Opening leaves the manifest and recordings unchanged; Save persists revision 3. Other unsupported identities still fail. Rubber Band remains the default and its processing is unchanged.

Completed checks with the workspace Cargo cache selected:

- New generated bass regressions cover the 51.9130872/65.40639133 Hz dyad, second/third harmonics, all six rates, ratio 17/20 with +6 semitones, 5/4 with −6, and 3/2 with zero pitch. Each partial meets 5 cents and 0.5 dB without level matching. Spectral-estimator controls reject incorrect pitch/level assumptions. The same regression against an extracted pre-change engine fails at 80.51 cents on the first shifted partial; the failure remains in ignored scratch logs.
- Changing bass chords pass at 48/96 kHz, with +6 semitones at 17/20 and zero pitch at 3/2. Stationary mixtures generate only their initial attack at every rate, including amplitude scaling and anti-phase. Dense impulses remain detected.
- Bass with noise-like drum bursts passes at 48/96 kHz for 3/2, 2, and +6 semitones at 17/20: independent first-difference energy quantiles remain within 5 ms of mapped attacks and within a 20 ms energy width. Existing kick, noise, mixed-percussion, boundary, stereo, and phase tests also pass. Proportional/silent/dual-mono/anti-phase channels, unrelated-channel swaps, gain homogeneity, exact duration, monotonic cancellation, and memory admission are covered.
- `cargo test -p timestretch --release -- --include-ignored` passes, including the 330-case six-rate tone matrix and 1,848-case short-layout matrix. The later changing-chord regression passes separately; the final `cargo test -p timestretch --release --test bass` passes all six bass tests.
- `cargo test --workspace --release` passes: 1,062 passed, zero failed, two existing expanded matrices ignored there and passed separately above. The changing-chord test was added afterward and passed in the final focused bass suite. An earlier unoptimized workspace run was stopped after the overlap choice changed; no complete unoptimized pass is claimed.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, staged/unstaged whitespace checks, and changed-document local link targets pass. The project workflow test verifies revision-2 upgrade, rebuilt pitched audio matching fresh revision-3 preparation, unchanged source/manifest on open, persisted new revision on Save, and rejection of other unsupported revisions.
- An extracted external consumer processes pitched anti-phase bass with only the portable engine dependency. Extracted reference-disabled tool tests pass; dependency inspection contains only RustFFT and its permissive dependencies, with no DAW/native/Rubber Band dependency in the engine graph. Initial external Cargo attempts could not reach the registry; the successful runs use the cached dependencies with offline mode.
- The maximum stereo spectral-array estimate was checked against the native planner at width 65,536: 6,094,920 bytes, including 65,536 complex scratch values and the worst hop-sized weight table. This fits the existing 8 MiB FFT allowance; planner internals, output/intermediate audio, transient regions, and allocator overhead retain their documented separate accounting/limits.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass after the final DSP change. `sh tools/package_macos.sh release` rebuilds `target/release/DAW.app`; its executable hash matches the final desktop binary and executable permissions are retained. The running app/session was not replaced or relaunched.

A read-only local bass-heavy excerpt was processed at +6 semitones, ratio 17/20, stereo 48 kHz. Original-file hashes remained unchanged. The output has the exact expected 921,601 frames; raw outputs/reports remain in ignored `work/bass-transpose-review/`, with no private media in Git. Diagnostic repeated measurements use serial engine processes, rotated order, one warmup and five measured runs per engine: median engine time is 0.555 s for Integrated revision 2, 0.649 s for revision 3, and 1.245 s for the public-API R3 reference. Median child-process CPU times are 0.648/0.742/1.338 s respectively. All runs share the same prepared-input hash and retain deterministic per-engine output hashes. Builds are release on macOS/aarch64; other validation was active, CPU model/RAM and process RSS are unavailable in the tool report. These are local diagnostic costs, not an idle-host/full-corpus performance gate or a reference-quality claim.

No Rubber Band implementation source, tests, comments, tables, or pseudocode were inspected or reused. Listening evaluation, native device playback/Settings warning interaction, Windows/Linux builds, full corpus performance, and multi-listener acceptance remain open. Source-wide window selection, masked/dense attacks, and cymbal tails retain their documented limitations.


## Integrated bass-heavy drum compression regression — 2026-10-21

A zero-transpose 96→100 BPM drum report exposed a revision-3 regression. The local source has approximately 83.5% of analyzed non-DC power below 250 Hz and global weighted flatness about 0.039, so revision 3 selected its 16,384-frame bass window at 48 kHz instead of the prior 2,048-frame window. Compression does not run the expansion-only guarded time map. Earlier mixed-drum checks exercised expansion or upward-pitch staging and did not establish this zero-pitch compression case. This entry supersedes revision 3's unqualified bass-window selection and current processing identity.

Revision 4 reuses the existing power bins and flatness helper to inspect three upper bands. Significant broadband activity vetoes only the long bass window; global broadband selection and normal tonal processing retain precedence/behavior. The shared 80-block detector, stereo phase decisions, overlap schedule, pitch filter, resource checks, and exact length are unchanged. Opening saved Integrated revisions 2 or 3 now explicitly upgrades to 4 and rebuilds preparation with a warning, leaving the saved manifest unchanged until Save. See [revision-4 contract](../audio/timestretch.md#drum-aware-bass-selection--revision-4) and [provenance](../../crates/timestretch/PROVENANCE.md#drum-aware-bass-selection--2026-10-21).

Completed checks with the workspace Cargo cache selected:

- Independently generated chirped kicks with weak seeded-noise hats reproduce the compression regression. At 48 kHz and ratio 24/25, pre-kick energy over 100 ms relative to the following 100 ms is −16.44 dB with the archived revision-3 processor and −33.78 dB with revision 4. The new calibrated −30 dB gate rejects revision 3. All six rates retain onset error within 5 ms; silent and anti-phase stereo pass. Other rates use the onset gate, not the 48 kHz energy ceiling: the existing normal-window algorithm itself gives roughly −20 to −28 dB pre-echo at the other rates for this phase/grid fixture. An initial proposed universal −30 dB threshold was therefore replaced with this explicit pairwise 48 kHz regression and the existing six-rate timing criterion; no established acceptance threshold was weakened.
- `cargo test -p timestretch --release -- --include-ignored`: 35 passed, including all seven bass/drum regressions, the 330-case tone matrix, the 1,848-case short-layout matrix, stereo, pitch, cancellation, and memory admission checks. The archived revision-3 copy fails the new drum energy gate as expected.
- `cargo test --workspace --release`: 1,064 passed, zero failed, two existing expanded matrices ignored there and passed separately above. The updated project workflow verifies automatic upgrade from both revisions 2 and 3, unchanged recordings/manifests on open, rebuilt audio matching fresh preparation, persisted revision 4 after Save, and rejection of unsupported identities.
- Formatting, strict workspace Clippy, staged/unstaged whitespace checks, and changed-document link targets pass. The extracted portable consumer and reference-disabled standalone tool tests pass with revision 4. No dependency was added; the engine remains independent of DAW/native/Rubber Band dependencies.
- Final DAW CLI build and WAV/MP3 smoke checks pass. `sh tools/package_macos.sh release` rebuilds the app; its executable hash matches the current desktop build. The running app was not relaunched.

Read-only local renders verify exact prepared-input identity, source hashes, and output length. At zero pitch and ratio 24/25, the new drum output is sample-identical to the pre-change revision-2 raw output (460,800 frames). The earlier +6-semitone, ratio-17/20 bass example remains sample-identical to revision 3 (921,601 frames). This verifies retention of those exact behaviors, not subjective quality beyond them. Recordings, raw WAVs, and reports remain in ignored `work/drum-stretch-review/`; no private fixture is committed. Separate single-loop audition files apply the same −6 dB attenuation to fresh raw revision-2/3/4 and native R3 renders for playback headroom; raw files remain unchanged. Float headroom is preserved in the engine, with no new limiter or normalization.

Native listening/device playback, Settings-warning interaction, Windows/Linux builds, and full-corpus/multi-listener acceptance remain open. Source-wide selection can trade bass resolution for attack localization in mixtures; compression/transient behavior beyond the calibrated case and general reference-quality parity remain unproven. No Rubber Band implementation source, tests, tables, comments, or pseudocode were inspected or reused.

## Timeline effect transfers onto track rows — 2026-10-21

Installed effects from the bottom lane now append to visible audio, MIDI, and Group track rows. The UI reuses effect activation, ordered selection capture, copy modifiers, scrolling, selection borders, and core atomic transfers. Read-only eligibility and final project validation share modulation source-scope checks. Hover preserves both chains; valid release selects and focuses the destination effects. Foreign modulation providers reject the complete group with a forbidden cursor and no error dialog. See [Timeline effect drops](../editing/components.md#timeline-effect-drops).

Completed checks with the workspace Cargo cache selected:

- `cargo test -p daw-core effect_transfer` passes all three transfer tests. Coverage includes Master-to-track moves/copies, empty audio/MIDI/Group chains, insertion order, stable move identities, fresh effect/route copy IDs, compatible layer providers, foreign providers, invalid groups, and atomic rejection.
- `cargo test -p daw-ui --lib timeline_effect_drops` passes all five new interaction tests. The matrix covers both themes, expanded/collapsed units, compact/regular lanes, track headers and clip lanes, grouped child ownership, vertical edge scrolling, group order, copy modifiers, unchanged preview state, destination selection/focus, undo/redo, Master sources, fast motion/release, clean same-track no-ops, invalid modulation, and cancellation for Escape, focus loss, outside releases, dialogs, disabled UI, view changes, and changed source order.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo test --workspace --release` passes: 1,070 passed, zero failed, two existing extended timestretch matrices ignored. No stretch processor changed in this task.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass. `sh tools/package_macos.sh release` rebuilds `target/release/DAW.app`; its executable hash matches `target/release/daw-desktop`. The running app/session was not relaunched. Changed-document local link targets and staged/unstaged whitespace checks pass.

Native mouse/modifier gestures, live device playback during transfers, and Windows/Linux builds remain unverified. Automated egui input tests do not establish native gesture behavior.

## Timeline effect drops onto Master — 2026-10-21

The fixed Master block now accepts installed effect moves and copies through the same Timeline drop resolver, preview, and atomic transfer path as track rows. The existing Master selection helper selects the destination before the lane refresh restores transferred-unit selection and keyboard focus. Core routing and processing are unchanged. Master rejects effects with assigned instrument modulation sources through the existing eligibility check, showing a forbidden cursor without a dialog. This extends the preceding track-row validation; see [Timeline effect drops](../editing/components.md#timeline-effect-drops).

Completed checks with the workspace Cargo cache selected:

- `cargo test -p daw-ui --lib timeline_` passes all 40 selected interaction tests. New coverage includes ordered group moves/copies onto empty and populated Master chains, both themes, compact/regular lanes, expanded/collapsed units, unchanged preview state and source clips, stable move IDs, fresh copy IDs, release-time copy modifiers, destination selection/focus, undo/redo, same-Master reorder/copy/no-op, fast release, Escape, and rejection of instrument modulation without an error dialog.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- `cargo test --workspace --release` passes: 1,072 passed, zero failed, two existing extended timestretch matrices ignored. No audio processor, project format, or CLI behavior changed.
- `sh tools/package_macos.sh release` rebuilds `target/release/DAW.app`; its executable hash matches `target/release/daw-desktop`. The running app/session was not relaunched. Changed-document local link targets and staged/unstaged whitespace checks pass.

Native mouse/modifier gestures, device playback during transfers, and Windows/Linux builds remain unverified. The automated egui tests do not establish native gesture behavior.

## Instrument layer icons and Output name — 2026-10-21

Regular layers replace their visible grip with Lucide Audio Waveform through the same `icons::header_icon` used by Timeline tracks. The 28×22-point slot, 16-point artwork, colors, 7-point inset, and 6-point name gap are shared. Icon, name, and background starts keep the existing guarded layer reorder path; mix controls and name editors retain their own gestures. The pinned bus uses Keyboard Music in the same icon slot and now displays Output. Header, tooltips, accessibility, and shared Envelope/LFO/Macro source prefixes use the new name. `InstrumentBusSettings::DISPLAY_NAME` supplies the header and provider metadata; saved `main`, stable IDs, routing, and processing remain unchanged. User guides use Output consistently, with implementation alias and compatibility documented in [Output bus](../instruments/main.md).

Completed checks with the workspace Cargo cache selected:

- `cargo test -p daw-ui --lib layer_`: 18 passed. Existing matrices cover both themes, shared header/name/control geometry, icon/name/blank/inset/gutter/body reorder starts, release-only commits, stable layer state, scrolling, no-ops, cancellation, rename and control guards, compact layers, and unit interactions. The new Output check covers compact/expanded icon/name alignment, visible Output text, icon selection, a fixed bus, unchanged settings/identity, and clean UI-only changes.
- `cargo test -p daw-unit-core instrument_bus`: two passed. Discovery reports Output while shared scope, identity validation, and legacy `main` loading/defaults retain their contracts. Existing UI source-picker and saved-route tests now expect the Output prefix.
- The first workspace run exposed an existing edge-scroll test race: it chose a destination from the previous frame before the queued scroll applied. The test now leaves the edge, settles the pending scroll, and releases inside a row. Product drag behavior is unchanged. The focused release regression passes after this correction.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass after the final changes.
- Final `cargo test --workspace --release` passes with the Keyboard Music Output icon: 1,073 passed, zero failed, two existing extended timestretch matrices ignored. Saved-route, picker, compatibility, and audio pipeline regressions pass; no DSP behavior changed.
- Final `sh tools/package_macos.sh release` rebuilds `target/release/DAW.app`; its executable hash matches `target/release/daw-desktop`. The running app/session was not relaunched. Changed-document local link targets and staged/unstaged whitespace checks pass.

Native visual/drag checks, device playback, and Windows/Linux builds remain unverified. Automated egui input tests do not establish native gesture behavior.

## Instrument layer drag duplication — 2026-10-21

Option on macOS or Ctrl on Windows/Linux duplicates a regular layer at the existing insertion preview. Release selects move/copy and commits through the existing instrument edit/history path. Shared drag press/threshold, focus, copy modifier, edge scrolling, and cancellation primitives cover icon, name, and background starts, including fast release. Output is excluded. `InstrumentLayerSettings::duplicate` preserves all retained settings and assigns new owned IDs, remapping internal sources, targeted oscillator/Macro routes, and saved provider order. External providers, outside consumers, original layers, and immutable Sampler inputs stay intact. Effect copies and component-transfer consumer remapping reuse the existing effect copy contract and shared layer route iterator. Prepared audio edits retain the selected copied owner through worker publication.

Completed checks with the workspace Cargo cache selected:

- `cargo test -p daw-unit-core instrument_copy`: two passed. Tests verify the complete serialized settings after identity normalization, fresh owned/route identities, local/external reference behavior, valid provider graph, retained components, component order, effects, saved-data round-trip, Sampler input sharing, and separate oscillator/provider namespaces when old UUIDs coincide.
- `cargo test -p daw-ui --lib layer_`: 21 passed. Coverage includes both themes, compact/expanded rows, icon/name/blank starts, unchanged previews, fast releases, modifier changes, copy insertion/order, selected copied owner, preserved clips, atomic undo/redo, same-position copies, cancellation, invalid Output/outside drops, changed layer order, disabled/focus guards, and existing rename/control/reorder behavior. The Sampler case holds a busy frame and verifies preparation before publication plus copied-owner selection after completion.
- `cargo test -p daw-project --test workflow duplicated_layers_preserve`: passed for oscillator and pitched Sampler layers. An isolated copy renders sample-identically to its original with internal Amplitude, Macro, gain, and effect modulation; two copies render twice the single-layer signal within 1e-6. Save/reopen retains copied routes and rendering. Source WAV bytes, assets, and original settings remain intact.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and final `cargo test --workspace --release` pass: 1,079 passed, zero failed, two existing extended timestretch matrices ignored. Existing component-transfer and audio allocation/history tests pass with the shared route visitor.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass. `sh tools/package_macos.sh release` rebuilds `target/release/DAW.app`; its executable hash matches `target/release/daw-desktop`. The running app/session was not relaunched. Changed-document local link targets and staged/unstaged whitespace checks pass.

Native Option/Ctrl drags, live device playback during duplication, and Windows/Linux builds remain unverified. Automated egui tests do not establish native gesture behavior.

## Instrument layer group selection and dragging — 2026-10-21

Shift-click toggles regular-layer membership; plain header clicks select one layer. Every selected header uses the existing row outline. Output stays exclusive and pinned. Layer drags capture selection in saved order through `unit_selection::drag_group`, use `drag::group_insertion` for moves/no-op detection, and retain shared thresholds, preview, copy modifier, scrolling, release-only publication, and cancellation. Copies share owned-identity remapping with single-layer copies and retarget links between all copied owners. External inputs/consumers and immutable Sampler inputs stay unchanged. Prepared audio publication restores the primary owner and whole selected group.

Completed focused checks:

- `cargo test -p daw-unit-core instrument_copy --lib`: 3 passed. Covers full retained settings, fresh IDs, oscillator/provider namespaces, cross-layer sources with equal provider UUIDs, external sources/consumers, empty groups, and serialization.
- `cargo test -p daw-ui --lib layer_`: 23 passed. Covers both themes, selection outlines and toggles, repeated Shift-clicks on names without Rename, control exclusions, Output exclusivity, track target reset, adjacent/sparse groups, saved order rather than click order, group move/copy, unselected drag starts, fast releases, clean no-op moves, cancellation/outside/Output drops, undo/redo, existing header controls/scrolling, and complete Sampler group preparation/selection publication.
- `cargo test -p daw-project --release --test workflow copied_layer_groups`: passed. Confirms row moves retain rendered audio, copied groups double the modulated signal, isolated copies preserve the reference audio, and save/reopen retains the copied graph.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, changed-document links, and `git diff --check`: passed.
- `cargo test --workspace --release`: 1,083 passed, 0 failed; the 2 existing extended timestretch matrix checks remain ignored.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py`: passed. Smoke checks cover WAV/MP3 codecs/bitrates, repeatability, missing sources, eight-track projects, group buses, overwrite, and exit codes.
- `sh tools/package_macos.sh release`: rebuilt the unsigned `target/release/DAW.app`.

Native mouse/trackpad gestures, device playback during these gestures, and Windows/Linux checks remain unverified. Automated egui input tests do not establish native gesture behavior.

## Project-specific independent review skill — 2026-10-21

Added `.agents/skills/daw-review` for committed DAW branch and PR reviews. The skill asks for reviewer count, accepts optional per-reviewer models/focus, and runs independent fresh-context reviewers followed by a separate adjudicator. Reviewers cannot exchange messages, conclusions, or scratch artifacts. The adjudicator receives their reports through the coordinator and rechecks claims and suggestions without contacting them. This is workflow isolation over shared filesystem tools, not an adversarial sandbox.

The rubric covers documentation completeness/coverage/style, implementation against current specs, security, performance, shared-code reuse/migration, dependency vulnerabilities, and license/distribution obligations. DAW-specific checks include callback restrictions, exact history and cache reuse, worker publication, sample/frame and edit invariants, stable/copied modulation identities, shared UI controls/drag behavior, playback/export parity, and Integrated timestretch independence. Missing current advisory access or other required evidence remains limited coverage rather than a fabricated vulnerability or a clean PASS.

The standard-library Python helper pins merge-base/head snapshots without switching branches or including uncommitted changes. It validates each reviewer, requires exactly one disposition for each raw finding, rejects missing/mislabeled reviewers and mixed revisions, and counts only accepted findings after deduplication. The final report has type, per-category/severity counts/status, verdict, grouped/ranked findings with suggestions/evidence, declared coverage/checks, and model information with an AI disclaimer. Critical/major findings or incomplete required coverage fail by default. Raw reports and adjudication decisions remain in the audit artifact. Existing final artifacts cannot be overwritten.

Completed checks:

- Skill-creator `quick_validate.py`: passed.
- `python3 .agents/skills/daw-review/scripts/test_review.py`: 12 passed. Covers severity/category counts, merge/reclassification/removal, threshold/coverage gates, missing/duplicate/unaccounted findings, revisions, JSON duplicate keys, configured reviewers, output preservation, source snapshot immutability, dirty-tree exclusion, tracked-output rejection, and archive path/link safety.
- Independent forward test on a temporary, dependency-free Rust workspace: two fresh-context reviewers each reproduced an introduced relative-gain defect. A separate fresh adjudicator independently reproduced it and verified the correction in its own scratch copy. Its decision merged both raw findings into one major code finding; the report correctly showed docs PASS, code FAIL, deps PASS, and total FAIL. Both pinned snapshots were byte-compared against Git objects after all agents finished and remained unchanged. This test did not review the actual DAW branch.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --release`: 1,083 passed, 0 failed; the 2 existing extended timestretch matrix checks remain ignored.
- Skill/reference/document links and `git diff --check`: passed.

No production PR provider/fetch integration, live vulnerability/license audit, or Windows/Linux Python execution was exercised. Provider resolution/fetching and subagent launches belong to the coordinating skill/tool workflow; the Python helper does not supply an agent runtime. No desktop runtime changed, so no app rebuild or native/device test was needed.

## Review category labels and scope gates — 2026-10-21

Updated the DAW review skill to display Documentation, Code Quality, Dependencies and Total while retaining stable machine IDs in JSON. Code Quality is SKIPPED when the pinned scope has no source, script, test or behavior-affecting configuration changes. Dependencies is SKIPPED when dependency inputs are unchanged. Documentation remains active. Skipped categories have zero findings, contribute neither PASS nor FAIL, and are excluded from Total counts/verdict. Incomplete required coverage continues to fail active categories only.

Preparation records both scope gates before agents start and uses a no-renames inventory to retain removed/renamed inputs. Parsed Cargo fields distinguish dependency/code behavior from package descriptions, comments and unrelated metadata. Feature/build/target inputs, native/vendor resources and licensing/packaging inputs keep relevant reviews active conservatively. Unknown scope or malformed manifests cannot justify skipping. Agent prompts explicitly omit skipped audits and source-test execution while permitting source context needed to verify Documentation claims. Validators reject findings in skipped categories and reject skipped coverage for active categories.

Completed checks:

- `python3 .agents/skills/daw-review/scripts/test_review.py`: 21 passed. Added docs-only and dependency-only gating, active-category failures, legacy incomplete coverage exclusion, skipped-coverage/finding validation, source/script/config inputs, manifest metadata versus feature changes, removed/renamed inputs, friendly labels and zero-count SKIPPED output. Existing counting, decision, identity, revision, snapshot safety and output-preservation checks remain covered.
- Skill-creator `quick_validate.py`, skill/document local link checks and `git diff HEAD --check`: passed.
- `cargo fmt --all --check`, strict workspace Clippy and `cargo test --workspace --release`: passed; 1,083 tests passed, 0 failed and the 2 existing extended timestretch matrices remain ignored. No Rust runtime changed.
- Re-rendered the prior `main` review from the same pinned revisions and raw reviewer/adjudicator evidence under the new policy. Code Quality remains active with its one minor finding; unchanged Dependencies is SKIPPED. Documentation and Code Quality pass, and Total is PASS. Original report/audit artifacts remain unchanged. This is a policy revision, not a new agent review; the selection-loss finding remains unfixed.

This supersedes only the prior review's dependency-coverage FAIL policy for unchanged dependency inputs. It does not assert that skipped dependencies or code have passed an audit. No app rebuild or native/device check was needed for these harness/documentation changes.

## Layer copy selection retention — 2026-10-21

Fixes review finding F001. Instrument layer copies keep the original primary layer and selected group while Sampler audio prepares. The shared prepared-edit result selects copied IDs only when their session publishes. Failure or cancellation retains the original selection and prepared audio; synchronous copies retain release-time selection.

Automated egui regression coverage exercises single-layer and two-layer copies, an intervening busy Instrument frame, successful publication and undo, an injected worker error, and cancellation. Failure/cancellation checks retain project tracks, selection, clean state and the original prepared sample allocations.

- `cargo fmt --all --check`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked -p daw-ui --release layer_duplication_stages_selection --lib`: passed (six cases).
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace --release`: passed; 1,083 tests passed, 0 failed, 2 existing extended timestretch tests ignored.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.
- Changed guide/specification local link targets exist; `git diff --check` passed.

Native drag gestures, live device playback, and Windows/Linux execution were not checked for this fix. Automated egui checks do not establish native behavior.

## Timeline Mute/Solo shortcut focus — 2026-10-21

Timeline M/S uses text-edit focus rather than the general keyboard-focus guard. The shared edit shortcut environment still guards jobs/dialogs, active gestures, window focus and name drafts. Other views and track/clip actions retain their previous focus policy. Track shortcut dispatch checks availability per action before consuming a key.

Automated egui coverage focuses a non-text control or text field in Timeline and Mixer. It checks selected-group mute/solo, retained control focus, text entry, untouched unselected tracks, dirty state, and unchanged Rename availability. Existing shortcut tests cover mixed values, held repeats, modifiers, text drafts, jobs/dialogs, disabled UI, drags and window focus.

- `cargo fmt --all --check` and `git diff --check`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked -p daw-ui --release track_shortcuts --lib`: 20 passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace --release`: 1,084 passed, 0 failed, 2 existing extended timestretch tests ignored.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.
- Changed guide/specification local link targets exist.

Native menu/key delivery, live device playback and Windows/Linux execution were not checked. Automated egui input tests do not establish native platform behavior.

## Instrument layer mix shortcuts and Escape — 2026-10-21

Instrument M/S uses the existing text-aware action guard and native/egui action dispatch. Regular layer selections toggle each member's own state through one instrument edit. Output or an empty layer selection targets only the displayed track, retaining other selected tracks. Escape clears layer UI selection without editing the project. The previous text widget ID prevents egui's frame-start Escape focus release from also clearing layers.

Automated egui coverage checks single/group and Output/no-selection targets, mixed layer states, untouched tracks, undo/redo, focused non-text controls, typed M/S, Escape leaving text input before clearing layers, retained empty selection and drag cancellation precedence. The first focused regression exposed the early text-focus release; it passed after retaining the previous text widget identity.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked -p daw-ui --release instrument_mix_shortcuts --lib`: 2 passed, covering the six target/focus cases.
- `cargo fmt --all --check`, changed local documentation link targets and `git diff --check`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace --release`: 1,087 passed, 0 failed, 2 existing extended timestretch tests ignored. Includes the additional layer-drag Escape regression and existing catalog, selection and rename coverage.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.

Native key/menu delivery, live audio devices and Windows/Linux execution were not checked. Automated egui tests do not establish native platform behavior.

## Timeline Escape selection priority — 2026-10-21

Timeline Escape clears clips and clip time ranges first while retaining tracks/Master. A later press clears tracks/Master when no clip selection remains. Project data, playhead, loop endpoints, playback and history remain unchanged. Timeline and Instrument share Escape observation, selection-environment guards and previous-text-focus retention. Existing track actions use the shared environment without changing their focus policy.

Regression cases cover clip/range/both selection with tracks or Master, held Escape, non-text control focus, text editing, and staged deselection. Existing context-menu dismissal and fast effect-drag cancellation tests exposed precedence conflicts; frame-start availability capture and the popup guard keep those actions ahead of deselection. Idle Timeline range clearing now has one shared owner; Editor range handling and gesture cancellation stay on their existing path.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace --release`: 1,089 passed, 0 failed, 2 existing extended timestretch tests ignored. Includes the two new Timeline regressions, shared Instrument selection tests and existing menu/effect-drag cancellation coverage.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check`, `git diff --check` and changed local documentation link targets: passed.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.

Native key/menu delivery, live device playback and Windows/Linux execution were not checked. Automated egui tests do not establish native platform behavior.

## Native mix and armed-layer Escape guards — 2026-10-21

Fixes F001 and F002 from the `f37981d` review. Shared track-action execution rejects M/S while a pointer button is held, including native dispatch, without disabling menu press tracking. After release, focused non-text controls and menu activation still work. Shared Escape availability includes an armed layer gesture before activation, while the existing active-only drag predicate remains unchanged for other consumers.

Regression coverage holds real Timeline and Instrument knobs, calls the public native-dispatch entry for Mute and Solo, verifies no project mutation, then verifies execution after release. Fast move/release plus Escape checks adjacent and sparse selected layer groups, primary/group retention, no edit on cancellation, the subsequent layer M/S target, and normal idle Escape deselection. Existing menu release, active-drag, focus and selection tests also run.

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked -p daw-ui --release track_shortcuts --lib`: 27 passed on the final fixtures.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace --release`: 1,091 passed, 0 failed, 2 existing extended timestretch tests ignored.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed on the final sources.
- `cargo fmt --all --check`, `git diff --check` and changed local documentation link targets: passed.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh release`: passed; rebuilt `target/release/DAW.app`.

Native AppKit key/menu delivery, native pointer timing, live device playback and Windows/Linux execution were not checked. Automated egui input and public-dispatch tests do not establish native platform behavior.

## Stem separation backend evaluation — 2026-10-21

Evaluated pinned demucs-rs and Charon revisions for a possible offline clip operation. This adds research documentation and [machine-readable local measurements](stem-separation-2026-10-21.json), not an application feature or dependency. See the [evaluation and integration proposal](../audio/stem-separation-evaluation.md) for model hashes, method, licensing evidence and remaining gates.

- Built both upstream release evaluators with locked dependencies in ignored scratch storage; also built the demucs-rs CPU feature. Two scratch-only changes isolate Charon's workspace and redirect demucs-rs model-cache reads. No inference math was changed.
- Recorded 24 local invocations: five reference excerpts and three long-workload repeats for Metal/Charon CPU, first GPU setup, silence/rate probes, and three demucs-rs CPU controls. All five scored excerpts were finite on both primary paths. Current demucs-rs Metal returned non-finite audio in its first setup run, all three long-workload repeats and the 48 kHz probe despite successful exits. Charon CPU and the CPU controls returned finite audio.
- For the 68.034-second concatenation, Charon CPU completed in 11.17/11.09/11.18 seconds; the single valid demucs-rs CPU control took 152.02 seconds. Failed Metal runs are not ranked as successful throughput. Memory evidence records RSS and physical footprint separately.
- Checked exact frames/stereo, non-finite samples, reference-stem SDR, digital silence and rate behavior. demucs-rs returned one extra frame at 48 kHz; Charon returned model-rate audio. Local model permissions remain unresolved despite permissive library licenses and third-party model labels.
- Documentation/data checks: JSON parsed with finite metric values; all 24 records have four stereo stems and consistent expected frame metadata; reported medians/timings checked against recorded values; local document links and `git diff --check` passed. No DAW source, dependency, audio fixture, project or model artifact changed.

Limits: five AAC-derived research previews and an artificial concatenation, one Mac, no blind listening or local PyTorch parity check, no optimized Charon CoreML export, no Windows/Linux or live playback-contention tests, no complete transitive-license/security audit. Upstream measurements are attributed and kept separate from local runs. DAW workspace tests and packaging were not rerun for this documentation-only change.

## Charon CoreML follow-up — 2026-10-21

Completed the previously unmeasured CoreML path using the same hashed audio and pinned Charon revision. See the [CoreML benchmark](../audio/stem-separation-coreml-evaluation.md) and [follow-up measurements](stem-separation-coreml-2026-10-21.json). The original 24-run baseline JSON is unchanged.

- Exported the official HTDemucs checkpoint through Charon's checked CoreML-target exporter. The split/PyTorch and ONNX-CPU/PyTorch maximum differences passed its 1e-3 gate. Recorded checkpoint/export hashes, structure and Python/native runtime versions. Built release `separate` and `profile` examples with `--features coreml` in scratch storage.
- Completed five excerpt and three long one-shot runs each on CoreML and CPU using the same split export, plus three reused-model separations each for original CPU, same-export CPU and CoreML. All outputs passed finite/stereo/frame checks. Added separate empty-cache, silence, 48 kHz and provider-audit probes; timing tables exclude instrumentation/metric work.
- Audited actual ORT assignment: one CoreML partition, all 1,982 optimized model nodes supported, no CPU-provider model-kernel events in the recorded profile. CoreML is configured CPUAndGPU; host transforms remain CPU work. This is not a hardware-level GPU execution trace.
- Median 68.034-second process time: 2.82 s CoreML versus 11.17 s recorded original CPU and 6.85 s same-export CPU. Reused-model separation medians: 1.98 / 8.13 / 6.29 s respectively. Excerpt SDR changes stayed below 0.0005 dB. Peak RSS, physical footprint and disk cache usage are reported separately.
- Retained startup evidence: first empty-cache model load/compile 25.26 s; another fresh process took 25.97 s despite cached files. Warm results do not establish universal startup latency. The CoreML cache occupied about 1.67 GiB locally; same-export CPU fallback used more memory than the original low-memory CPU path.
- Documentation/data checks: original baseline hash retained; source/reference hashes and reconstructed long input checked; 18 new one-shot records and nine resident separations validated; per-stem comparisons, derived summary values, JSON, local links and whitespace checked. App source, dependencies, fixtures and models remain unchanged in tracked scope. DAW workspace tests and packaging were not rerun for this documentation-only follow-up.

Remaining limits: one host, five AAC-derived excerpts, artificial long workload, no blind listening or live playback/UI contention, no broader device/platform or signed-package checks, no hardware-level GPU trace, no dedicated CPU-target split export measurement. Model-use/redistribution terms remain unresolved. This follow-up closes the earlier lack of local CoreML measurements; it does not grant release acceptance.

## Stem separation — 2026-10-21

Implemented [stem separation](../audio/stem-separation.md) with local model preferences, CoreML on macOS, a separate low-memory CPU fallback, streaming output through the shared WAV encoder, worker-side import, atomic group publication, and a disabled original clip. Added shared export/separation progress reporting and project-owned generated-source persistence.

Completed checks:

- `cargo test --workspace --locked`: passed, including documentation tests. The project, UI separation, and disabled-clip tests were also rerun after the final persistence/test changes. Existing opt-in acceptance tests remained ignored.

- `cargo check --workspace --all-targets`: passed on macOS ARM64.
- `cargo fmt --all --check` and `git diff --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- Affected library tests: engine, project, separation, and UI passed. Project regressions cover loop input and gain, exact aligned durations, original-source preservation, grouping, unique folders, cancellation/failure/discard cleanup, Save As, and Undo/Redo after Save As. Additional focused tests cover disabled audio/MIDI scheduling and history invalidation, preference persistence, and guarded failure reports without a second error dialog.
- `cargo test -p daw-project --locked`: passed, including storage, export, transpose, and Sampler preparation workflows after the generated-source/history changes.
- `cargo build -p daw-cli --locked` and `python3 tools/check_cli.py`: passed. The smoke check covers codecs, deterministic export, missing sources, multi-track projects, group buses, overwrite protection, and exit codes without graphics/audio devices.
- Opt-in release test `model_smoke::local_models_separate_and_import`: passed with the local CPU model, local CoreML split model, and an intentionally missing CoreML model to force CPU fallback. Each run used the same local stereo test WAV and verified four finite, duration-matched, aligned stems plus saved-project reopening. The fallback run retained the CoreML error as a warning. No weights or test recordings were added to the repository.
- `sh tools/package_macos.sh release`: passed. `otool -L` reports system libraries/frameworks, including CoreML; no external ONNX Runtime dylib is required. Every separation notice is present in the bundle. Linux CI setup includes the OpenSSL headers needed for build-time native TLS downloads; Linux and Windows packaging includes the same notices.
- Local Markdown link checks: passed for changed documents and new feature guides/specifications.

Native checks: model controls and persisted filenames were inspected in the macOS Settings dialog. A generated test project then completed separation through Timeline’s clip context menu and status dialog. Its four tracks were visibly aligned with the dimmed original; the original menu offered Enable clip. Native Undo/Redo removed/restored the group. Regular Save persisted the disabled original, all four aligned frame ranges, retained clip gain, and project-relative stem files. The app was returned to an empty project afterward. The local CoreML model test exercised the actual native provider configuration. These checks do not establish Windows/Linux runtime behavior, Intel Mac support, device underrun rates during separation, or quality across arbitrary material. Cold model loading and inference calls remain non-interruptible; cancellation takes effect at the next shared checkpoint. A native out-of-memory process abort is outside the in-process recovery guarantee. Full model-dependent acceptance is opt-in; the normal test suite does not distribute or require weights.

### Status report progress width — 2026-10-21

The shared export/separation report progress bar now fills the available content width, keeping the dialog's existing horizontal padding. Focused export-status and separation UI tests passed, as did formatting, strict workspace Clippy, and whitespace checks. The macOS release bundle was rebuilt. This change does not alter audio processing; the full audio suite was not repeated for this layout adjustment.

### Disabled clip palettes — 2026-10-21

Disabled clips now use an opaque dimmed version of their resolved palette instead of reducing painter opacity. Track-color inheritance and clip overrides retain their hue; the header, channel fill, waveform/MIDI notes, and text share the dimmed palette. The selection outline keeps its normal color and width. Opaque fills prevent timeline grid lines from showing through disabled clips.

The new painting regression passed for inherited and overridden colors in dark and light themes, including opaque fills, waveform/text colors, and full-strength selection outlines. All 557 UI library tests passed. Strict workspace Clippy, formatting, and whitespace checks passed. The macOS release bundle was rebuilt; no audio-processing behavior changed.

### Greyed disabled clip palettes — 2026-10-21

Supersedes the disabled-palette color treatment above. Disabled clips now desaturate their resolved body toward grey with a small retained color tint. They reuse the standard clip palette builder for readable text, waveforms, note colors, and related fills, instead of fading foregrounds toward the panel. Opaque fills and normal selection outlines remain.

The painting regression now checks reduced color saturation and foreground contrast in both themes. All 557 UI tests, strict workspace Clippy, formatting, and whitespace checks passed. The macOS release app was rebuilt. No audio behavior changed.

### F001: Generated stem paths in retained history — 2026-10-21

Shared save completion now retargets generated asset paths by stable ID in every retained history snapshot, including pending grouped edits. Saving with separation undone no longer erases the destination paths needed by Redo. Snapshot memory accounting and the existing byte limit include the updated paths; the saved-state baseline remains independent.

Completed automated checks:

- `cargo test --offline --locked -p daw-project`: 79 passed, 0 failed, 1 local-model test ignored. The new synthetic regressions cover the UI's record-then-mark-saved sequence, Save As → undo separation → Save → redo, deletion of the first project folder before the final Save/reopen, and save completion with newer grouped edits. Existing history memory limits, dirty-state handling, storage, export, transpose, and Sampler tests passed.
- `cargo clippy --offline --locked -p daw-project --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.

Tests used generated audio and the existing deterministic separation fixture. No model inference, native UI/device checks, release packaging, or Windows/Linux execution was repeated for this history-only fix.

### PR #2 F001–F004: Stem backend and preparation fixes — 2026-10-22

- F001: Intel macOS omits the unsupported Charon/ONNX backend. Shared target availability disables model controls and the clip action with an explanation, and the project API rejects unsupported requests before preparation. Added target-specific API/UI tests and an Intel cross-check to macOS ARM64 CI.
- F002: Decoder admission, resampling and stem preparation share checked ceiling frame arithmetic. Preparation validates the exact model-to-project count and trims its known rounding surplus. Synthetic regressions cover all six project rates, five frame-count residues, exact durations/stereo, and truncated-output rejection.
- F003: Both post-inference decode/resample/peak passes use the shared cancellable decoder. Deterministic tests cancel inside each decoder and verify unpublished-file cleanup and unchanged project state.
- F004: Pin native runtime 1.31.0 through upstream `ort-sys` revision `f2a5f73c9dd9ff3682303481cc792bf4669e3e94`; its distribution table retains per-target SHA-256 hashes. Updated runtime notices. A separate-process synthetic Loop regression verifies the linked version and the exact `INVALID_GRAPH` rejection for upstream #32609.

Native smoke testing caught a compatibility issue: ort's automatic efficiency policy selected CoreML even for a CPU-only model under runtime 1.31. The small [Charon source patch](../../vendor/charon-audio/DAW-PATCH.md) resets that policy before explicit providers are registered. The retained upstream processing and low-memory settings are unchanged. A weights-free native identity-model test covers stream stereo/duration and failed-CoreML-to-CPU fallback.

Completed checks:

- `cargo test --offline --locked --workspace`: 1,106 passed, 0 failed, 3 opt-in tests ignored before the final provider-policy patch. After that patch, affected media/project/separation suites passed; the final separation suite passed all three tests, including malformed-model rejection and native streaming/fallback.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed on final code.
- Intel macOS desktop and CLI cross-builds passed. Final `cargo check --offline --locked --workspace --all-targets --target x86_64-apple-darwin` passed using the installed rustup target; the Intel dependency graph contains neither Charon nor ort-sys. No Intel hardware execution is claimed.
- Opt-in project smoke tests passed with the existing local CPU model, the existing local split CoreML model, and an intentionally missing CoreML path. Each test used generated stereo WAV input, imported four aligned finite stems, saved, and reopened the project. The CoreML run had no fallback warning; the intentional failure retained the CoreML warning and completed through CPU.
- Formatting, whitespace, and the headless CLI smoke check passed.

The initial sandboxed model runs could not create native compiler working directories. Unrestricted generated-input checks then exposed the provider-policy issue above; the final CPU, CoreML and fallback runs passed after its correction. No private recording or pretrained weights were added to source. Windows/Linux runtime execution, Intel hardware execution, native UI/device checks, release packaging, broad separation quality measurements, and a complete native dependency audit were not repeated.

### Static CoreML release linkage — 2026-10-22

The native runtime update exposed a thin-LTO release link failure: CoreML availability checks referenced `__isPlatformVersionAtLeast`, but Rust's native link omitted Apple Clang's compiler runtime. `daw-separation/build.rs` now locates `libclang_rt.osx.a` through the selected Xcode/Command Line Tools `xcrun` and publishes its static linkage to consumers. Only Apple Silicon macOS runs this step; application crates do not duplicate it.

The macOS release package rebuilt successfully through `sh tools/package_macos.sh release`. All three separation tests passed in release mode, including separate-process malformed-model rejection and native stream/fallback. Release CLI linking, focused Clippy, formatting and whitespace checks also passed. No native UI/device or older-macOS compatibility check is claimed.
### Stem Save As path assertion on Windows — 2026-10-22

The stem Save As/reopen regression now checks the `stems` directory with `Path::starts_with` instead of a literal `stems/` string prefix. Saved relative paths use native separators, including backslashes on Windows. Save and import behavior are unchanged.

The focused `separation_preserves_original_aligns_group_and_reopens_after_save_as` test passed on macOS. `cargo fmt --all --check` and `git diff --check` passed. Native Windows execution remains for CI; it was not run locally.

### PR #2 F001: Preserve Redo during stem Save As — 2026-10-22

Blocking save completion now calls the existing shared `SessionHistory::mark_saved` through the UI history helper before recording the returned session. Generated source relocation is normalized before structural comparison, so a same-name Save As retains Redo and adds no Undo entry. A real project-name change still records an edit. Regular-save snapshot handling and bounded history retargeting remain shared and unchanged.

Completed automated checks:

- Added a UI regression using real generated WAV import, `save_named`, the save worker and `poll`. It failed before the fix at “relocation alone must preserve Redo” and passed afterward. Both same-name and renamed Save As cases verify history, saved-state tracking, destination retention and copied-source availability after removal of the original project folder.
- `cargo test --offline --locked -p daw-project -p daw-ui`: 639 passed, 0 failed, 1 opt-in local-model test ignored. Existing generated-source Save As/Undo/Save/Redo, grouped newer-edit, memory-limit, save-failure and UI lifecycle coverage passed. Updated the existing project regression to use the corrected save-completion order.
- `cargo clippy --offline --locked -p daw-ui -p daw-project --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.

Tests used generated media in temporary directories. Native UI/device interactions, Windows/Linux execution and release packaging were not repeated. This correction supersedes the earlier record-before-mark-saved completion order recorded in the stem persistence validation.

### Unsaved-project stem storage — 2026-10-22

Stem separation now supports unsaved projects. The existing preparation pipeline reserves an exclusively created temporary root and retains private shared owners in session/worker/history clones. Unsaved generated assets keep absolute source paths without setting a project folder. First Save reuses generated-asset copying and shared history retargeting, then releases temporary owners only when their source references have moved or been discarded. Failed saves retain the original temporary sources. The menu and action share the model/idle/platform readiness check. The completed report displays the actual output directory.

Completed automated checks:

- `cargo test --offline --locked -p daw-project -p daw-ui`: 86 project and 558 UI tests passed; 1 opt-in local-model test ignored. New project regressions cover unsaved separation, session/worker ownership, Undo/Redo and history eviction, first-save relocation and reopen, save while separation is undone, failed first save and retry, cancellation, backend failure and discarded-result cleanup. Existing saved-project/history/rate/import behavior passed.
- `cargo test --offline --locked -p daw-ui separation --lib`: 3 passed, including the additional shared readiness regression for saved/unsaved folders, missing models and blocking jobs. The failure/status guard test now starts from an unsaved project.
- `cargo test --offline --locked -p daw-project --lib same_name_jobs_use_distinct_directories`: passed after extending the collision check to saved and unsaved projects.
- `cargo clippy --offline --locked -p daw-project -p daw-ui --all-targets -- -D warnings`: passed.
- `cargo build --offline --locked -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.

Tests used generated WAV fixtures and temporary directories. No user media or model weights were used. Native UI/device interaction, Windows/Linux execution, real-model separation and release packaging were not repeated. Temporary storage has no crash-recovery or stale-directory cleanup service. This behavior supersedes the earlier saved-project prerequisite.

### Platform-neutral separation loading message — 2026-10-22

Replaced the shared loading text with “Preparing audio and loading model…” so CPU jobs on Windows, Linux, and macOS do not show a CoreML compilation note. Backend selection is unchanged. Updated the user guide and separation specification.

Completed automated checks on Windows:

- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo build --offline --locked --release -p daw-desktop`: passed; rebuilt `target/release/daw-desktop.exe`.
- `cargo test --offline --locked --workspace`: failed in the unchanged project test `separation::tests::failed_first_save_retains_temporary_stems_and_the_unsaved_session`, at `assert!(next.save(&blocked).is_err())`. The test expected saving to fail with a directory at `project.json`, but the save succeeded on this Windows run. The suite stopped before UI tests; this run does not establish a passing workspace test suite.
- `cargo test --offline --locked -p daw-ui separation --lib`: all 3 separation UI tests passed.
- `git diff --check`: passed.

Native UI/device interaction, real-model inference, macOS/Linux execution, and release packaging were not repeated for this text change.

### Stem-separation native dependency and packaging audit — 2026-10-22

Completed the native closure audit for the checksum-selected macOS arm64, Windows x64/arm64 and Linux x64/arm64 runtime distributions. Intel macOS omits this backend. See the [audit report](../audio/stem-native-dependencies.md) and [machine-readable inventory](stem-native-dependencies-2026-10-22.json). The scope covers the new stem-separation native graph and affected packaging obligations, not a fresh vulnerability audit of all pre-existing media/system libraries.

Completed checks:

- Downloaded all five selected archives and verified their SHA-256 against pinned `ort-sys` distribution entries. Embedded runtime build strings agree on source commit `402979ac1bef89dcd55add41eaa7277c407599eb`. Listed native archive members and reconciled the source-derived closure: 20 top-level C/C++ source projects plus protobuf’s embedded UTF-8 validator, platform DirectML and Apple compiler runtime, and external system runtime families. Both DirectML DLL hashes match the original Microsoft.AI.DirectML 1.15.4 NuGet payloads.
- Queried OSV at 20 exact source commits and primary repository advisories for every source component. Recorded dispositions for 20 ONNX/protobuf advisories, plus NVD keyword and Apple security-content checks. Confirmed affected ONNX 1.22.0 under GHSA-xrch-8vh7-h656; native `MathOpTwoIntegers(..., int, int)` and ORT schema-inference calls corroborate the component exposure. This audit does not upgrade or patch the dependency, and does not establish memory corruption or execution of a malicious model.
- Read pinned source licenses, exact DirectML runtime/source terms, installed Apple LLVM acknowledgment, current/legacy compiler-rt terms, MPL source-availability requirements and Apache/GPL compatibility guidance. Added missing native/source notices, compiler-runtime notices, immutable source locations and GPLv3 text beside existing media GPLv2 text. Existing notices and original-code BSD licensing remain intact. Distributors still must satisfy corresponding-source obligations for exact combined executables; no source-release offering was created or certified.
- Downloaded and statically inspected completed Windows/Linux CI packages at `9fcbae5`; runtime/dependency inputs match audit revision `58a5d98`. Read PE imports and ELF dependencies/version requirements without running foreign executables. Inspected the local macOS release executable's system links and linked availability symbol. Verified macOS native archive `LC_BUILD_VERSION` 13.4 and Linux package symbol floors GLIBC_2.39/GLIBCXX_3.4.31.
- `sh tools/package_macos.sh debug`: passed after notice additions. Compared every packaged separation/media notice byte-for-byte against source. All three packagers include those notice directories (static script check). Windows/Linux packagers were not executed locally.
- Inventory JSON counts/source-commit/hash checks, Markdown local-link checks, `sh -n tools/package_macos.sh tools/package_linux.sh`, `cargo fmt --all --check`, and tracked/new-file whitespace checks passed.

No DSP/application implementation or dependency version changed. No private media, weights, credentials, native binaries or caches were added to tracked files. No clean-machine Windows/Linux/Intel launch, arm64 foreign package execution, subjective separation test or new model inference was performed. An inventory and advisory audit does not certify an untested deployment: the ONNX advisory, matching-source release obligations and oldest-supported-OS smoke tests remain explicit release actions. The earlier review artifacts are retained unchanged; this audit is new evidence, not a rewritten PASS verdict.

### PR #2 major save findings: ordinary Save and directory destinations — 2026-10-22

Ordinary Save now returns the worker's successfully written session. Completion merges generated-source locations into the live session by stable asset ID and marks that exact written checkpoint saved. Live state and retained history use shared `Session::retarget_generated_sources`; newer edits, pending gestures, transport, prepared audio and revision guards stay intact. Temporary owners are pruned after their paths move. This prevents repeated stem copies and stale original-folder references after saving with separation undone and then redoing it.

Shared file replacement now rejects an existing non-file destination before moving it to a Windows backup. A directory at `project.json` retains its name and contents. The failed-first-save regression checks preserved contents, unchanged session state, temporary-source retention, staged-copy and manifest-temp cleanup, and successful retry. This correction supersedes the failing Windows first-save result recorded under “Platform-neutral separation loading message”; that historical result remains unchanged above.

Completed automated checks on Windows with the pinned toolchain:

- The new UI regression failed before the implementation fix at the live/written generated-path comparison. It now covers unsaved first Save and same-name Save As with a generated clip undone, Redo, ordinary Save during a newer grouped edit and seek, shared prepared audio, Undo/Redo after removing original sources, repeated saves without extra stem directories, and successful reopen. All four regular-save UI tests passed.
- The new project regression uses deterministic separation output and verifies live/current/pending/Undo/Redo source paths and temporary-directory release. All 13 focused separation tests passed, including the strengthened first-save failure regression.
- `cargo test --offline --locked --workspace`: 1,121 passed, 0 failed, 3 opt-in acceptance/model tests ignored. This includes all 565 UI tests, the project save/history workflows, shared export checks and documentation tests.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo build --offline --locked -p daw-cli`: passed. `tools/check_cli.py` passed using Python 3.12, covering WAV/MP3 output, repeatability, missing sources, track/group rendering, overwrite protection and exit codes.
- `cargo fmt --all --check` and `git diff --check`: passed.

Tests used generated media in temporary directories. Native UI/device interaction, real HTDemucs/CoreML inference, macOS/Linux execution and release packaging were not repeated. The two minor review findings and dependency coverage limits are outside these two major fixes; this record does not change the pinned review artifacts or certify a new review verdict.

### PR #2 F003: failed Save As cleanup — 2026-10-22

Failed `Session::save_new` now removes its empty `stems` root before removing the exclusively created project folder. Cleanup uses non-recursive directory removal, as with the existing `assets` cleanup, so unrelated contents remain intact. This releases the project name after a generated-source copy fails and permits retry after restoring the missing source.

Completed automated checks on Windows with the pinned toolchain:

- Added `failed_generated_save_as_releases_the_name_and_allows_retry` in the project workflow suite. It failed before the fix because the `Retry` project folder remained. It now passes after staging one stem and failing on a missing second stem. The test verifies unchanged live state, save destination and original manifest, retained source/sibling files, removal of the failed destination, successful same-name retry after source restoration, and reopen with matching prepared samples and no warnings.
- `cargo test --offline --locked -p daw-project --test workflow`: all 64 tests passed.
- `cargo test --offline --locked --workspace`: 1,122 passed, 0 failed, 3 opt-in acceptance/model tests ignored.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo build --offline --locked -p daw-cli` and `tools/check_cli.py` using Python 3.12: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.

Tests used generated WAV fixtures in temporary directories. Native UI/device interaction, real HTDemucs/CoreML inference, macOS/Linux execution and release packaging were not repeated. No dependency version changed; the separate ONNX advisory and dependency coverage limits remain outside this fix. The pinned review artifacts remain unchanged.

### PR #2 F001 and F004: cancellation and stem folder names — 2026-10-22

These corrections address F001 and F004 from the five-reviewer run pinned to `6ac9634`.

Status-bar Cancel now sets the separation cancellation flag and retains the job receiver. Both Cancel controls wait for the worker response before the report enters its cancelled state. Closing that report restores editing and file actions. Other preparation jobs retain their existing late-result rejection.

Stem directory reservation now bounds the complete basename to 255 UTF-8 bytes, including a collision suffix. It truncates only at character boundaries and shares the folder-name byte limit with project-name validation. Separation and generated-source copies use the same reservation helper. The user guide and specification describe both corrections.

Completed automated checks on macOS arm64 with the pinned 1.97.0 toolchain:

- The new egui cancellation regression failed before the fix because status-bar Cancel removed the receiver. It now passes for both Cancel buttons, with an explicit cancellation response and with worker disconnection. It verifies that Escape cannot dismiss a running report, Close restores editing/file actions after acknowledgement, and project state remains unchanged.
- The new folder-reservation regression failed before the fix because a 252-byte name grew beyond 255 bytes after numbering. It now passes for ordinary names, 252/255-byte multibyte names, longer four-byte-character names, and one- and two-digit collision suffixes. Existing files remain intact.
- A second project regression passes for repeated separation followed by Save As into an existing stem-folder collision. It verifies bounded names, retained files, copied generated sources and successful reopen without warnings.
- `cargo fmt --all --check` and `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 561 UI tests. Opt-in real-model and release acceptance tests remained ignored.
- `cargo build --offline --locked -p daw-cli` and `python3 tools/check_cli.py`: passed. CLI checks cover WAV/MP3 codecs and bitrates, repeatability, missing sources, track/group rendering, overwrite protection and exit codes.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- `git diff --check`: passed.

The first workspace test build used `CARGO_NET_OFFLINE=true`, which made `ort-sys` skip native runtime linkage. Removing that environment flag and retaining Cargo's `--offline` option allowed the cached runtime to link; the complete workspace rerun passed. This required no dependency or implementation change.

Tests used generated media and temporary directories. Native UI/device interaction, real HTDemucs/CoreML inference and Windows/Linux execution were not performed. The filename byte limit was checked on macOS; native Linux filesystem execution remains unverified. Scope is F001/F004; the pinned review artifacts remain unchanged.

## 2026-10-22 — Validated model selection and optional CPU fallback

Model selection now loads the requested CPU or CoreML backend and runs a short, nonzero stereo probe on the existing worker. The production output adapter checks the four ordered stem names, stereo channels, 44.1 kHz rate, finite samples and exact output duration without writing stem files. Either validated model enables separation on its supported platform. CoreML takes priority when both models pass; a separate CPU fallback is optional. Failure without a fallback retains the project and reports the error.

A candidate path is accepted and persisted only after successful validation. Failed replacement retains a different accepted model; a failed recheck of the same path clears it. Cancellation retains the previous model, keeps the worker receiver until acknowledgement and rejects late success. Saved paths are revalidated at startup. Superseded workers cannot overwrite newer settings. Validation state is not serialized into settings or projects.

Focused automated checks passed on macOS arm64:

- Native CPU validation accepts the generated four-stem fixture and rejects missing paths and the split-model input contract. Native CoreML loading/inference with an isolated temporary compilation cache accepts the generated split fixture and rejects the CPU-model contract. These fixtures contain no pretrained weights.
- Backend tests cover CoreML-only success, preference with both models, optional CPU retry, failure without fallback, cancellation without retry, and invalid output metadata, channels, lengths and non-finite samples.
- UI tests cover delayed acceptance, rejected replacement, clearing each model independently, startup rejection, superseded results, and status-bar cancellation that retains the previous model and project.

Completed repository checks with the pinned Rust 1.97.0 toolchain:

- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed, including a final rerun after the same-path rejection change.
- `cargo test --offline --locked --workspace`: passed. The workspace build included 565 UI tests. After the final same-path rejection change, all nine focused separation UI tests and the complete 566-test UI suite passed.
- `cargo build --offline --locked -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- `git diff --check`: passed.

The initial Clippy run identified an unnecessary unwrap in validation scheduling. It was removed before the successful reruns. Checks used the local Cargo cache and Cargo's `--offline` option. No dependency pins changed.

Native UI/device gestures, production HTDemucs weights and Windows/Linux execution were not tested. Native CPU/CoreML checks used synthetic fixtures, so they establish loading and output-contract behavior rather than separation quality or GPU coverage. Existing opt-in real-model and release acceptance tests remain ignored. Startup and model-replacement behavior were checked through automated UI state and worker tests.

## 2026-10-22 — Application Settings categories

Application Settings now groups existing controls into expanded Appearance, Playback, MIDI and Stem Separation sections. Appearance contains Titlebar where supported, Theme and Accent color; Playback contains Seek cache; MIDI contains Split MIDI notes; Stem Separation contains the existing model controls and platform notice. Bold body-text headings and standard separators retain the shared dialog, common label column and Close action. No settings schema or control behavior changed. User-guide navigation and the implementation specification describe the categories.

`cargo test --offline --locked -p daw-ui --lib` passed all 566 existing UI tests. Coverage includes theme/accent/titlebar controls, seek-cache and MIDI dropdowns, popup dismissal, settings guards, model validation and cancellation. No new layout-only tests were added.

Repository checks on macOS arm64 with the pinned toolchain:

- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 566 UI tests. Existing opt-in real-model and release acceptance tests remain ignored.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Documentation link targets and `git diff --check`: passed.

The change only rearranges application UI controls, so CLI smoke checks were not repeated. Native UI interaction and Windows/Linux execution were not performed; automated egui tests do not establish native window behavior.

## 2026-10-22 — Separation build warning on Windows and Linux

Windows and Linux CI rejected `run_models` because its runner parameter was declared mutable while only the macOS CoreML branch borrowed it mutably. The function now takes an immutable parameter and creates its mutable binding only in the macOS configuration. CPU dispatch, CoreML preference, fallback and cancellation behavior are unchanged.

A temporary host-side compile harness extracts the production function and substitutes a feature flag for its macOS predicate, with minimal backend stubs. With warnings denied, the original non-macOS branch reproduces the reported `unused_mut` error; the corrected non-macOS and macOS branches compile and pass dispatch assertions. This checks the conditional Rust function, not a native Windows/Linux dependency build.

The seven native separation tests passed on macOS arm64, including CPU/CoreML inference fixtures, preference/fallback, cancellation and output-contract rejection. Native Windows/Linux builds remain to be verified by CI.

Completed with the pinned Rust toolchain on macOS arm64:

- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 566 UI tests. Existing opt-in real-model and release acceptance tests remain ignored.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- `git diff --check`: passed.

This correction changes conditional binding mutability only. Native UI/device checks and CLI smoke checks were not repeated.

## 2026-10-22 — Clip menu and grouped context actions

The native macOS menu bar and Windows/Linux application menus now place Clip between Edit and Track. Shared `ClipMenuAction` groups define Import clip/Add MIDI pattern; Split at playhead/Stem-separation; Edit clip/Disable clip/Delete clip, with separators. Edit retains Copy/Paste and no longer lists Split. File retains its existing Import WAV alias. The shared clip context menu reuses the last two groups and retains selected-note Velocity/Delete note controls in a separate MIDI-only group, as confirmed by the user.

Creation reuses the existing import flow and transactional MIDI pattern insertion on the selected MIDI track at the playhead. Menu-bar edit/mute/separation target the primary selected clip; context actions target the clicked clip. Context split/delete retain selected-group behavior when the clicked clip belongs to the selection. Native and egui dispatch share readiness, view/kind, focus/text, gesture and job/dialog checks. Enable/Disable and singular/group deletion labels follow current state. Model validation and separation publication retain their existing worker guards. Specifications, contributor guidance and user guides now use Clip > Split at playhead.

Completed focused checks on macOS arm64:

- Five focused clip-menu tests passed: the four new menu tests plus the updated MIDI split menu regression. Coverage includes ordered groups and separators, removal of Split from Edit, primary/clicked targets, selected-group deletion, pattern creation at the playhead, validated-model availability, view restrictions and blocked dispatch.
- `cargo test --offline --locked -p daw-ui --lib`: all 570 UI tests passed. Existing tests cover actual right-click actions in Timeline/Editor, retained MIDI note controls, Copy/Paste, native-action guards and simulated macOS/Windows/Linux shortcut labels. Application-menu tests now include Clip.
- `cargo check --offline --locked -p daw-desktop` and `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- A temporary main-thread AppKit harness instantiated the production native menu and verified menu order, separators, the split key equivalent and consumption of its forwarded event, dynamic labels, selector dispatch for pattern creation/mute/split/group deletion, and rejection while Settings is open. It passed outside the sandbox after matching the application-menu initialization normally supplied by winit. The first harness setup omitted that prerequisite; it was corrected without a production-code change.

Final repository checks with the pinned Rust toolchain:

- `cargo fmt --all --check`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 570 UI tests. Existing opt-in real-model and release acceptance tests remain ignored.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Changed documentation link targets and `git diff --check`: passed.

Native Windows/Linux execution and manual menu/keyboard interaction were not performed. The native macOS harness checked AppKit objects and selector dispatch rather than human input. CLI smoke checks were not repeated because shared import/render/export behavior and CLI code are unchanged.

## 2026-10-22 — Import clip owns the import menu entry

Removed Import WAV from the native macOS and shared egui File menus. Clip > Import clip retains the existing Command+Shift+I/Ctrl+Shift+I shortcut and shared import dispatch. Removed the obsolete import-group separator and updated the current specifications and user guide. Internal `FileAction::Import` remains available to the shared import flow and shortcut handling. Native File items keep their original action tags by filtering after enumeration.

The 30 focused menu tests passed. Existing shortcut expectations now place import only in Clip; the grouped-menu regression also checks that File omits the import label. The temporary AppKit harness passed additional assertions for File import-item/key removal, Clip import key/modifiers, and Save's unchanged action tag. Its existing native menu dispatch and guard checks also passed. `cargo fmt --all --check` and workspace Clippy with warnings denied passed.

Final repository checks with the pinned Rust toolchain on macOS arm64:

- `cargo fmt --all --check` and `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 570 UI tests. Existing opt-in real-model and release acceptance tests remain ignored.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Changed documentation link targets and `git diff HEAD --check`: passed.

Native Windows/Linux execution and manual menu/keyboard interaction were not performed. CLI smoke checks were not repeated because shared import/render/export behavior and CLI code are unchanged.

## 2026-10-22 — Track menu action groups

Reordered the native macOS and shared egui Track menus into Add audio track/Add MIDI track; Rename track/Duplicate track; Group selected tracks/Ungroup tracks; Mute/Solo; Delete track. Track context menus now use Rename/Duplicate; Group/Ungroup; Delete. Separators mark each group. Both menu adapters reuse the shared group boundaries. Existing action guards, shortcuts and dynamic selection labels remain in place. User guidance and the track-group specification describe the new order.

Completed focused checks on macOS arm64:

- `cargo test --offline --locked -p daw-ui --lib track`: all 112 selected tests passed, including track shortcuts, selection, context-menu dispatch and guards.
- A temporary AppKit harness instantiated the production native menu and verified the exact Track action order, four separators and each native action tag against the shared dispatch order. Existing Clip/File order, shortcut and guarded selector-dispatch assertions also passed. This harness ran outside the sandbox with the application-menu initialization normally supplied by winit.
- `cargo fmt --all --check` and `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- Changed documentation link targets: passed.

The native menu harness passed with Mute/Solo after Group/Ungroup. The existing rendered-menu test initially failed on its old Duplicate/Mute ordering; its expectation now follows the new order. All 570 UI tests then passed with `cargo test --offline --locked -p daw-ui --lib`. Workspace Clippy with warnings denied and formatting checks also passed again after that correction.

Final repository checks with the pinned Rust toolchain on macOS arm64:

- `cargo test --offline --locked --workspace`: passed, including all 570 UI tests. Existing opt-in real-model and release acceptance tests remain ignored.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Changed documentation link targets and `git diff HEAD --check`: passed.

Native Windows/Linux execution and manual menu/keyboard interaction were not performed. The AppKit harness checks native menu objects and action tags rather than human input. CLI smoke checks were not repeated because shared import/render/export behavior and CLI code are unchanged.

## 2026-10-23 — MP3 and AAC clip imports

Added shared MP3 and AAC-LC decoding alongside strict WAV decoding. AAC supports
ADTS `.aac` and ISO MP4 `.m4a`/`.mp4` sources. Menu import, file-hover/drop, Sampler,
project reopen and processing-rate changes use the generalized weak decode cache.
Compressed source metadata uses `mp3`/`aac` with zero PCM bit depth; core validation
accepts these combinations without changing existing WAV metadata. Decoding,
resampling, peaks, memory admission and cancellation remain on worker paths.
Unsupported codecs/channel counts, invalid samples and detected truncated sources
fail before session publication. MP3 gapless metadata is honored; AAC may retain
container/encoder padding. Updated user guides, implementation requirements,
locked dependency features and bundled Symphonia MPL/source notices.

Focused checks on macOS arm64 with the pinned toolchain:

- Three `daw-media` compressed-import integration tests passed. Synthetic mono and
  stereo MP3, AAC and M4A fixtures cover all six project rates, frame conversion,
  finite samples, mono duplication, independent stereo, waveform preparation,
  weak cache reuse, MP3 encoder delay/padding, cancellation before/during decode,
  unsupported rates, misleading extensions, corrupt input, unsupported 5.1 AAC,
  and truncated MP3/MP4 data.
- The project workflow regression passed import/save/reopen, original source
  metadata and PCM preservation, processing-rate changes, WAV export, failed
  import rollback and rejection of compressed metadata with a PCM bit depth.
- The egui input regression passed worker import and hover/drop publication for
  MP3, AAC and M4A. Existing WAV import behavior remains covered by the suite.
- `cargo fmt --all --check` and
  `cargo clippy --offline --locked --workspace --all-targets -- -D warnings` passed.
- Changed documentation link targets and `git diff HEAD --check` passed.

AAC export was added to the same change after the import checks started. It uses
pinned MIT-licensed `oxideav-aac` 0.1.7 with locked `oxideav-core` 0.1.41, through
a bounded streaming worker adapter. Desktop and CLI expose AAC-LC/ADTS `.aac`,
independent 128/192/256/320 kbps target selection (192 default), and the actual
output sample rate. Higher project rates convert to 48 kHz through streaming
Rubato. The existing session export path retains publication, progress, warnings,
overwrite/source protection and failure cleanup. AAC priming/padding and the
absence of M4A export are documented. MIT/source notices use existing packaging.

Completed AAC export checks:

- An isolated dependency evaluation encoded a synthetic one-second stereo tone;
  FFmpeg decoded the ADTS without errors to 49,152 finite stereo frames with
  approximately 0.2504 peak and 0.1381 RMS. FFmpeg is a development check only.
- Two media regressions passed all six project rates and four target bitrates,
  independent Symphonia decoding, finite stereo samples and channel energy,
  deterministic bytes across 512/137-frame writes, expected ADTS padding,
  empty/one-frame/boundary tails, silence, clipping and non-finite rejection.
- The project AAC regression passed 96-to-48 kHz export, progress, source and
  overwrite protection, retained destination bytes on rejection, cleanup and
  identical output across 32/4096-frame renderer blocks.
- The export dialog regression passed AAC selection, bitrate changes, retained
  MP3/WAV settings and existing action behavior. CLI argument checks passed AAC
  extension inference, defaults, target rates and rejection of incompatible
  codecs, extensions and bitrates.
- Workspace Clippy with warnings denied passed after AAC export integration.

Final validation:

- The full `cargo test --offline --locked --workspace` run passed its test binaries,
  including the unchanged long-running DSP suites. Its media documentation
  compilation failed because AAC export and its dependency were added after that
  run started. With the final dependency graph,
  `cargo test --offline --locked --workspace --doc` passed.
- Rechecked all affected crates after AAC export with
  `cargo test --offline --locked -p daw-core -p daw-media -p daw-project -p daw-ui -p daw-cli`:
  passed, including all 571 UI tests, both AAC export tests, all three compressed
  import tests and all 66 project workflow tests. Existing opt-in tests remain
  ignored. The final export-dialog label check also passed.
- Final workspace formatting and Clippy with warnings denied passed.
- `cargo build --offline --locked -p daw-cli` and `python3 tools/check_cli.py` passed.
  Smoke coverage now includes short AAC exports at all four target rates, AAC-LC
  ADTS headers, repeatability, overwrite handling and invalid argument rejection,
  alongside existing WAV/MP3, missing-source and project checks.
- `sh tools/package_macos.sh debug` passed; rebuilt `target/debug/DAW.app`.
- Packaged Symphonia and OxideAV notices, changed documentation link targets and
  `git diff HEAD --check` passed.

Native Windows/Linux builds, manual file-dialog/drop interaction and listening
checks were not performed. Automated egui input and independent decode checks do
not establish native platform or perceptual audio quality.

## 2026-10-23 — Filter effect without an internal LFO

Added Filter under EQ & Filters with the Auto-filter Cutoff/Out controls in two
90-point strips. The entire middle LFO section is omitted. Reused the panel,
knobs/diagrams, modulation controls/indicators, power, compact/collapse, selection,
catalog insertion and chain editing paths. Filter is available on audio/MIDI
tracks, Master, regular instrument layers and Output. Version-1 `daw.filter`
state contains no LFO controls and uses existing source/route validation,
duplication, unknown-version preservation and transactional edits.

The portable `daw-filter` crate now owns the stereo SVF, response mixing,
parameter and bypass ramps, external modulation smoothing and telemetry.
Auto-filter composes that processor with its existing LFO and clock. No external
dependency was added or upgraded; FunDSP remains behind the shared filter API.
Filter implements Shared-scope Frequency/Resonance/Level/Dry-Wet destinations
through the existing typed modulation resolver. Engine registration, route-state
retargeting, memory accounting and prepared fingerprints include the new type.

Completed focused checks on macOS arm64 with the pinned Rust toolchain:

- An isolated harness compiled the pre-change Auto-filter source and compared it
  with the composed processor. Audio and effective-parameter arrays matched
  exactly over all six rates, four shapes, five waveforms, live parameter/power
  changes, reset and clock jumps.
- All 12 Auto-filter tests passed, including the new static Filter parity test
  across rates/shapes, external offsets, retarget and reset. Existing response,
  smoothing, LFO and finite-output tests remain in place. Private implementation
  assertions now observe public processed values and presentation-update parity.
- Core Filter checks passed saved state/routes, fresh duplicate effect/route IDs,
  unknown future versions, unsupported modulation parameters and invalid setting
  rollback. Invalid edits retain the original complete chain.
- Engine Filter/Auto-filter history tests passed processing, reset, seek and
  compatible retarget with zero callback allocations. The extended shared-provider
  matrix passed Filter with Envelope/LFO/Macro sources at all project rates,
  live/export/block/seek/loop parity, telemetry, route reorder and live updates.
- The project regression passed track/Master/layer/Output persistence, save/open,
  rate changes, fresh track-copy effect identity, float WAV export/render parity
  and unchanged original source bytes.
- All 572 UI tests passed. New coverage checks both-theme catalog creation,
  Cutoff/Out geometry and controls, omission of LFO controls, power and compact
  mode. Group addition/drop tests include Filter on all existing target types.
  Keyboard expectations include the new entry; lower catalog entries are scrolled
  into view rather than assumed to fit in the previous viewport height.
- Workspace Clippy with warnings denied and formatting passed. Changed
  documentation link targets and `git diff --check` passed.

Final repository checks:

- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --offline --locked --workspace`: passed, including all 572 UI tests
  and the new Filter coverage. Existing opt-in real-model/release tests remain
  ignored. Logs are under `work/filter-unit-20261023`.
- `cargo build --offline --locked -p daw-cli` and `python3 tools/check_cli.py`: passed.
- `sh tools/package_macos.sh debug`: passed; rebuilt `target/debug/DAW.app`.
- Final changed documentation link targets and `git diff HEAD --check`: passed.

Manual native interaction, listening/device checks and native Windows/Linux
execution were not performed. Automated egui geometry/input and exact DSP parity
checks do not establish those platform or perceptual results.

## Granulizer instrument component — 2026-10-23

Added the Granulizer source through the shared sample-input component, with a portable `daw-granulizer` DSP crate. Grain/Scan/Random/Out strips reuse the common sample shell, knobs, window diagrams, modulation connections and component editing. Sampler and Granulizer share file/clip input, region preparation and source exclusions. Granular sources prepare only the root buffer; conversion drops unused offline pitches while retaining matching PCM. Version-2 sample settings protect granular projects from older readers; ordinary version-1 Sampler projects remain compatible.

Completed automated checks:

- `cargo fmt --all --check`, strict `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass with the pinned toolchain and locked dependencies. Commands used the local `work/cargo-home` cache and offline mode where supported. Final affected component, UI, engine and project regressions were also rerun after the source-version, boundary, sample-buffer and control-order adjustments.
- Portable settings round-trip, legacy Sampler defaults, future/invalid version rejection, parameter bounds and window endpoints pass. All six project rates cover stereo relationships, scan/pitch independence, reverse/freeze, windows and finite output. Deterministic grain sequences, bounded analytical advance, fractional spacing, source ends, One-shot tails, parameter edits, nonfinite modulation and fixed grain capacity pass. A regression prevents fractional forward scans from sticking at the source end.
- Shared MIDI tests cover Gain and all eight granular destinations through Per note/Retrigger/Legato Envelope, LFO and Macro sources. Live/offline and prepared-seek results agree. Compatible edits retain grain and route history. Allocation counters remain zero during rendering, snapshots, parameter updates and renderer inheritance. Gate and One-shot rendering agree across all six processing rates.
- The project regressions cover selected-region preparation, root-only banks, PCM reuse, copy identities, Main rejection, all processing-rate conversions, save/reopen, float-WAV export, failed/cancelled preparation retention and unchanged recordings. Switching from ordinary Sampler removes unused pitch variants while retaining the root allocation.
- All 573 egui UI tests pass, including Granulizer catalog installation, source exclusions and Grain/Scan/Random/Out controls in both themes. These are automated interactions, not native-window checks.
- `cargo build -p daw-cli` and `python3 tools/check_cli.py` pass. The smoke script covers WAV/MP3/AAC export, bitrate choices, repeatability, missing recordings, groups, overwrite handling and exit codes.
- `sh tools/package_macos.sh debug` builds the updated unsigned `target/debug/DAW.app`. Formatting, changed Markdown link targets and `git diff --check` pass. Local command logs remain in ignored `work/granulizer/`.

Native visual inspection was attempted but the Mac was locked. Native picker/drag/device/listening checks, Windows/Linux builds and hardware performance measurements were not run. Linear grain interpolation can alias at large upward pitch shifts; this implementation does not establish listening-quality acceptance or measured real-time CPU limits. No external dependency or third-party source was added.

## Granulizer window diagram bounds — 2026-10-23

Corrected the Grain window paths from −1..+1 to the shared −0.5..+0.5 coordinate bounds. The existing knob renderer now draws them at the required 10-by-6-point size with its standard 1.25-point stroke and active accent color. Grain shapes and audio behavior are unchanged.

The new bounds/shape regression and all 574 UI library tests pass. Formatting, strict UI Clippy, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/granulizer/icon-*.log`. Native visual and other-platform checks were not run for this correction.

## Ruler label background contrast — 2026-10-23

The shared Timeline/Editor/sample ruler now colors text by the background under the glyphs instead of tick-beat membership. Dark-mode text outside the loop keeps the theme's light text color, including labels after a frame-rounded loop end. Labels crossing a range edge are clipped into inside/outside color regions. Enabled-loop contrast and disabled-selection styling remain theme-aware.

All 575 UI tests pass, including both themes, enabled/disabled loops, two tempos, frame-rounded ends and partial-label intersections. Formatting, strict UI Clippy, `git diff --check` and the debug macOS bundle rebuild pass. Logs are in ignored `work/ruler-color-*.log`. Native visual and Windows/Linux checks were not run for this correction.

## MIDI editor track header and scale controls — 2026-10-23

Editor now has a full-height options panel and a track-header dropdown beside the clip strip. The header reuses track mute/solo, gain/pan inputs and stereo meters. Root defaults to Off; Scale and Snap to scale remain disabled until a root is selected. Scale selection dims out-of-scale key labels/rows and note fills without retuning source notes. The Scale heading retains the regular text size; its field labels, dropdowns and checkbox use the shared Small style.

Snapping applies only to newly drawn notes and nonzero pitch moves. Group moves snap the anchor and retain intervals. Timing-only moves and edge resizes keep pitch; Shift bypasses snapping. Root Off bypasses pitch constraints and restores ordinary colors while retaining the chosen scale and snap option. These options remain application-session UI state.

Completed checks:

- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked -p daw-ui --lib`: all 582 UI tests pass. New coverage checks scale/root bounds, nearest pitches, group moves/copies, Shift bypass, timing-only moves, resizing, cancellation, unchanged sources, Root Off controls, track selection/mixing, panel alignment, both themes, note/key dimming, and compact control text.
- `CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace`: passes, including doctests. Existing ignored acceptance/performance tests remain ignored. The final UI suite above also passes after the later color and text-size changes.
- `CARGO_HOME="$PWD/work/cargo-home" cargo clippy --offline --locked --workspace --all-targets -- -D warnings`: passes.
- `CARGO_HOME="$PWD/work/cargo-home" cargo fmt --all --check` and `git diff --check`: pass.
- `CARGO_HOME="$PWD/work/cargo-home" sh tools/package_macos.sh debug`: passes; rebuilt `target/debug/DAW.app`.

Logs are in ignored `work/editor-*.log`. These are automated checks and a bundle build. Native UI, device playback, and Windows/Linux checks were not completed for this change.

## Editor panel section headings — 2026-10-23

Both Editor panel sections now use the shared `ControlPanel::header` and bottom row-divider pattern. The lower section is MIDI options for MIDI tracks and Clip options for audio tracks. Audio options stay empty; returning to a MIDI track restores its scale controls without changing their choices or project data. These section names supersede the earlier Scale heading.

`CARGO_HOME="$PWD/work/cargo-home" cargo test --offline --locked --workspace` passes, including all 583 UI tests and doctests; existing ignored tests remain ignored. New coverage checks both themes, full-width title separators, MIDI/audio switching, absence of audio scale controls, retained MIDI choices, and clean project state. Strict workspace Clippy, formatting, `git diff --check`, and the macOS debug bundle rebuild also pass. Logs are in ignored `work/editor-sections-*.log`. Native UI and Windows/Linux checks were not run for this refinement.

## Persisted MIDI track options — 2026-10-23

Root, Scale, and Snap to scale now belong to `Track::midi_options`, using core `MidiOptions`/`MidiScale` definitions and `Edit::SetMidiOptions`. This supersedes the earlier application-session storage. The editor reads the current track for controls, dimming, and note input. Option changes preserve track/clip selection and enter project history and save revisions without preparing audio or requesting a renderer update. Duplication copies options; older manifests default to Root Off, Major, and snapping off. Invalid roots and non-MIDI option targets reject atomically. Root Off retains the saved scale and snap preference.

Completed checks:

- `cargo test --offline --locked --workspace` passes, including doctests; existing ignored tests remain ignored. The final UI suite was also rerun after the selection-preservation correction.
- All 584 UI tests pass, including per-track choice isolation, changed/clean history state, Undo/Redo, unchanged note content/selection, drawing with different track scales, and no audio preparation or playback synchronization on an option edit.
- Three core persistence/transaction tests pass: all scale variants round-trip, missing fields default, audio defaults remain omitted, invalid roots/targets/batches reject without mutation, and track copies retain independent options.
- The project workflow test passes for save/reopen, Save As, legacy loading, and rejection of an invalid saved root.
- Strict workspace Clippy, formatting, and `git diff --check` pass.
- The macOS debug bundle rebuild and CLI smoke checks pass.

Logs are in ignored `work/midi-options-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Editor audio clip options — 2026-10-23

Audio tracks now display the shared Timeline Name, Source, Gain, and Transpose fields under Editor’s Clip options header. This supersedes the earlier empty audio section. Both views use `clip_settings_body` for selection, group edits, validation, history, and release-only audio transpose preparation. An empty selection shows the standard prompt. Switching views or entering MIDI options clears pending text and audio previews.

Completed checks:

- All 588 UI tests pass. Four new egui regressions cover both-theme field geometry and the empty-selection prompt, name/group Gain edits with Undo/Redo, release-only audio Transpose publication with source preservation, and draft/preview cancellation on track or view changes.
- `cargo test --offline --locked --workspace` passes, including doctests. Existing opt-in acceptance/performance tests remain ignored.
- Strict workspace Clippy, formatting, and `git diff --check` pass.
- The macOS debug bundle rebuild passes and updates `target/debug/DAW.app`.

Logs are in ignored `work/editor-clip-options-*.log`. These are automated checks and a bundle build. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Selected audio waveform in Editor — 2026-10-23

Editor replaces its audio placeholder with an enlarged view of the primary selected clip. The new layout reuses the shared clip painter, prepared audio lookup, waveform peaks, overview clock, grid, playhead, zoom, scroll, and scrollbar. Clip gestures remain in the overview strip. The user guide and roadmap distinguish this waveform display from future direct waveform editing.

All 591 UI tests pass. Three new automated regressions cover mono/stereo peaks, both themes, primary selection, viewport retention, zoom and playhead alignment, trim offsets, loop phases, prepared pitch/stretch audio and fallback previews, missing sources, and empty selections. Viewing the waveform leaves project data unchanged and starts no preparation jobs.

`cargo test --offline --locked --workspace` passes, including doctests; existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace Clippy, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/audio-editor-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Independent clip editor viewport — 2026-10-23

The lower waveform/piano-roll area now has independent zoom, scroll, ruler, and scrollbar state. Selecting a clip in the overview fits its content width. Fit clip restores fitting, and window resizing retains it until a manual zoom or pan. Its button uses the user-supplied SVG geometry with theme tint, a standard 16-point icon/24-point button, tooltip, and accessible name. Both audio and MIDI show other clips on the selected track when zoomed out. Note-driven selection retains the working viewport and cross-clip note groups. Overview loop gestures and playback-toolbar zoom keep their original overview coordinates.

All 597 UI tests pass. Six new regressions cover both audio/MIDI fitting and window resize, independent pinch/swipe/zoom controls, zoomed-out neighboring audio clips and MIDI patterns, note-driven viewport retention, lower-ruler seeking, and persistent manual panning from a fitted view. A distant fractional project origin checks that unchanged scrollbar rounding cannot disable fitting. Updated waveform and MIDI gesture checks preserve source/loop/stretch mapping, project-grid snapping, group editing, and source isolation with separate viewports. Seeking retains the existing saved-navigation behavior; zoom, scroll, and Fit clip stay clean.

`cargo test --offline --locked --workspace` passes, including doctests; existing opt-in acceptance/performance tests remain ignored. The final 597-test UI suite was rerun after the manual-pan and SVG-button refinements. Formatting, strict workspace Clippy, `git diff --check`, and the final debug macOS bundle rebuild pass. The embedded SVG parses as XML. Logs are in ignored `work/editor-viewport-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Audio waveform selection and Fit clip placement — 2026-10-23

Clicking a visible lower-editor audio waveform selects its clip through the shared editor selection helper. Selection updates the outline and Clip options while retaining content and overview zoom/scroll. The content zoom range accommodates an existing close-up when the new clip is much longer. Selection uses clipped hit bounds and the shared focus/modal/gesture guards; pointer drags do not move or select clips. Fit clip now sits immediately to the left of the lower zoom controls.

All 599 UI tests pass. Two new regressions cover both themes, waveform display, clip/selection and options updates, unchanged viewport transforms/project data, extreme clip-duration differences, unfocused input, and pointer drags. `cargo test --offline --locked --workspace` passes, including doctests; existing opt-in acceptance/performance tests remain ignored. The final UI suite was rerun after adding the options-panel repaint request. Strict workspace Clippy, formatting, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/editor-waveform-selection-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Audio editor waveform context menu — 2026-10-23

Right-clicking a lower-editor waveform reuses Timeline’s `clip_context_menu`, including its groups, target resolution, action guards, and dispatch. Opening leaves clip selection and both viewport transforms unchanged. The owning response remains active while its popup is open; other popup owners continue to block waveform selection.

All 601 UI tests pass. Two new regressions cover both themes, menu persistence across frames, action order, clicked-target Disable behavior without changing the primary selection, unchanged zoom/scroll, clean Escape dismissal, and empty-space exclusion. `cargo test --offline --locked --workspace` passes, including doctests; existing opt-in acceptance/performance tests remain ignored. Strict workspace Clippy, formatting, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/editor-waveform-menu-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Fit all clips in Timeline and track overview — 2026-10-23

Timeline and the Editor track overview now have Fit all clips immediately left of the playback toolbar zoom controls. Both reuse the editor's SVG fit icon. Timeline fits the earliest clip start through the latest end across all tracks; overview fits the displayed track. The one-shot request resolves against the current layout width after overview track initialization. Exact fitting supports ranges outside the usual zoom limits, retains distant fractional origins through idle scrollbar frames, and allows pinch zoom back toward the normal range without a jump. Selection, lower-editor zoom/scroll, project data, and history remain unchanged.

All 604 UI tests pass. Three new regressions cover audio/MIDI track scope versus project scope, viewport and selection preservation, clean project state, empty arrangements, blocked settings, changed layout widths, extreme clip lengths, distant origins, idle-frame retention, and pinch behavior after fitting. `cargo test --offline --locked --workspace` passes, including doctests; existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace Clippy, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/fit-all-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Empty Editor and lower MIDI pattern selection — 2026-10-23

Removed the no-track placeholder label below the Editor ruler. The lower MIDI editor now has a pinned pattern-header row using the shared clip header painter, track/clip colors, muted colors, and clip-name styling. Pattern outlines span the visible note viewport instead of the fixed clip-strip height at an offscreen canvas origin. Selected patterns use the shared selection outline; other patterns use the theme divider. Header clicks select the pattern without refitting or changing either viewport and retain focus, modal, popup, and active-gesture guards. Note editing stays in the piano roll.

The workspace test run, including doctests, passes; existing opt-in acceptance/performance tests remain ignored. After the header changes, all 606 UI tests pass, including two new regressions for both-theme outlines, pinned header geometry during vertical scrolling, selection without zoom/scroll or project changes, and focus/settings guards. The existing contiguous-pattern rendering regression now checks the visible full-height outlines and selected styling. The two new header tests were rerun after the final gesture-guard refinement. Formatting, strict workspace Clippy, `git diff --check`, and the final debug macOS bundle rebuild pass. Logs are in ignored `work/editor-placeholder-*.log` and `work/midi-pattern-headers-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for these changes.

## Lower MIDI pattern gestures and context menus — 2026-10-23

Lower pattern headers and side borders now reuse `clip_lane_response` for selection, group moves/copies, header-edge looping, body-edge trimming, modifier stretching, and the shared clip context menu with MIDI note controls. This supersedes the selection-only lower headers. Drag ownership keeps release handling in the originating pane; the origin pane's scale supplies frame deltas and snapping for both overview and lower previews. Ownership clears after commit or cancellation. Lower gestures leave fitted mode and retain both viewports. Candidate headers, outlines, notes, and affected neighbors use the shared raw preview calculations and core project transactions. Publication remains release-only, including worker preparation for stretching.

All 610 UI tests pass. Four new regressions cover both-edge move/trim/loop/stretch previews and commits, header and body modifier stretching, matching overview previews at a different zoom, unchanged project state during previews, viewport retention after worker completion, ownership cleanup, Escape cancellation, group snapping and Shift bypass, persistent header/border context menus, note controls, and Disable dispatch. A border context menu over a full-length note must not delete that note. The existing low-zoom note-creation test caught overlapping edge hit areas; capping each side at one quarter of the pattern width keeps an editable interior. The final full UI suite passes after that fix and ownership cleanup.

`cargo test --offline --locked --workspace` passes, including all 610 UI tests and doctests; existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace Clippy, `git diff --check`, and the final debug macOS bundle rebuild pass. Logs are in ignored `work/midi-pattern-gestures-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.

## Audio editor clip gestures — 2026-10-23

The lower audio editor now reuses `clip_lane_response` for header move/copy, waveform-edge trimming, header-edge looping, modifier stretching, and the existing clip context menu. `editor_viewport.rs` owns the shared audio/MIDI candidate-preview and viewport-retention helpers. Waveform responses keep a stable drag sense across frames; waveform-interior drags do not start clip gestures or change selection. Every selected clip has the shared selection outline. Right-edge shared-boundary detection uses the active pane's zoom rather than Timeline's zoom. Linked-boundary previews paint both clips, including the surviving neighbor when the primary clip disappears; a resulting empty selection retains its manual viewport until an explicit overview selection fits another clip.

All 614 UI tests pass. Four new regressions cover both-edge move/trim/loop/stretch previews and commits, header/body modifier stretching, actual stretch preparation and prepared-buffer publication, unchanged original PCM ownership, source offsets at shared boundaries, both-clip previews, removal of the primary clip, retained zoom/scroll across idle empty-selection frames, later overview refitting, and Escape/focus-loss cancellation. Existing audio waveform selection, source mapping, prepared pitch/stretch display, context-menu persistence, and MIDI gesture tests remain passing.

`cargo test --offline --locked --workspace` passes, including all 614 UI tests and doctests; existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace Clippy, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/audio-editor-gestures-*.log`. Native UI, device playback, and Windows/Linux checks were not performed for this change.


## Editor separators and floating timeline scrollbars — 2026-10-23

The lower Editor toolbar continues the options title’s top and bottom separators through the shared panel border painters. The overview and track header end at the same height, with no scrollbar spacer. Timeline, the overview, and the lower audio/MIDI editor inherit the effects lane’s standard floating scrollbar style and default visibility. Scrollbars overlay their content instead of allocating a row or reducing the editing area; their interactions and painting follow the canvas. Independent viewport clocks and existing clip gestures remain unchanged.

The expanded section-header regression checks audio/MIDI track switches, both themes, and two window sizes. A new scrollbar regression checks fully visible hovered handles in both Editor panes and Timeline, including a theme width override, full-height content, and the absence of an overview-to-toolbar gap. Existing layout and drag tests now require Timeline content to reach the viewport bottom and allow clip placement in the former reserved bottom strip.

The workspace test suite and doctests passed during this change, with existing opt-in acceptance/performance tests ignored. After the final overlay and border changes, the complete affected UI suite passed again: 615 tests. Final `cargo fmt --all --check`, strict workspace/all-target Clippy, and the debug macOS bundle rebuild passed. Cargo used the local cache with offline/locked options. Logs are in ignored `work/editor-alignment-*.log` and `work/editor-scrollbar-test.log`. Native UI, device playback, and Windows/Linux checks were not performed.


## Editor border alignment at fractional display scales — 2026-10-23

The Editor track header, options title, and lower toolbar now use the shared `rows::fixed` allocation. Their outer row geometry no longer expands with rounded text or frame sizes. This keeps the options-title and toolbar bottom borders aligned at fractional display scales without adding spacing or reducing the editing canvas. Existing shared panel border painters remain in use.

The section-header regression now covers 100%, 125%, 150%, and 200% display scales, both themes, audio/MIDI track switches, and two window sizes. It allows the viewport to settle after a scale/size change before checking the painted boundaries. All 615 UI tests and the complete workspace suite, including doctests, pass. Existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace/all-target Clippy, `git diff --check`, and the debug macOS bundle rebuild pass. Logs are in ignored `work/editor-border-*.log`. Native UI, audio-device, and Windows/Linux checks were not performed.


## Timeline chord catalog — 2026-10-23

Timeline now has a Chords catalog tab with 12 note folders, a root-note filter, shared catalog selection and keyboard navigation, and 12 root-position chord types. Double-click and Enter insert ordinary one-bar MIDI patterns at the playhead on the selected MIDI track. Dragging previews incoming notes and overwritten neighbors through the shared clip painter and placement geometry, then commits on release to an existing MIDI track or one new MIDI track below the arrangement. Track-header drops use frame zero. Invalid targets, Escape, focus loss, blocked editing, and preparation failures retain the original project. Required Sampler pitches prepare through the shared worker publication path.

Core regressions cover all 144 presets at all six processing rates with fractional tempo, note duration and identity, source preservation during overwrite, invalid roots/targets, empty selections, overflow, and atomic failure. Nine new UI regressions cover note filtering without dirtying the project, visible-order group selection and placement, keyboard/Shift navigation, double-click/Enter behavior, preview versus release, overwrite and Undo/Redo, new-track creation, invalid drops, cancellation, Shift snapping, fast motion/release, header placement while scrolled, catalog-tab exclusivity, and successful/failed Sampler preparation. The project workflow regression saves and reopens embedded chord notes and IDs, then verifies identical audible float32 WAV exports.

All 624 UI tests and the complete workspace test suite, including doctests, pass. Existing opt-in acceptance/performance tests remain ignored. Formatting, strict workspace/all-target Clippy, documentation links, staged/unstaged whitespace checks, and the debug macOS bundle rebuild pass. Cargo used the local offline cache and locked dependencies. Logs are in ignored `work/chords-*.log`. Native UI, audio-device, and Windows/Linux checks were not performed.


## Chord catalog audition — 2026-10-23

Primary clicks and keyboard selection now audition the focused chord through the selected MIDI track. The isolated worker render reuses instrument layers, Sampler pitch preparation, group routing, effects, and Master processing. It leaves arrangement content, saved prepared pitches, history, and transport unchanged. Previews have a one-bar gate capped at eight seconds and one second of release/effect tail. Latest-selection cancellation, stale-result rejection, local mute, unrelated solo isolation, and Stop availability at the timeline origin are covered. The callback mixes bounded project-rate buffers before device conversion and returns buffer owners through a bounded retirement queue with five-millisecond cancellation fades.

Focused project tests passed for audible instrument rendering, ancestor-bus gain, mute, unrelated solo, unchanged session data, invalid targets, cancellation, and slow-tempo bounds. Sampler regression coverage confirms audible required-pitch preparation without publishing preview notes or prepared variants. Final affected checks passed all 627 UI tests and 34 output tests. Callback allocation instrumentation covers replacement, cancellation, queue saturation, natural completion, retirement, and preview mixing without advancing stopped transport. UI tests cover primary clicks, keyboard movement, invalid track selections, drag exclusion, worker coalescing, stale results, and Stop cancellation.

The complete workspace suite and doctests passed, with existing opt-in acceptance/performance tests ignored. The affected UI/output suites were rerun after the final Stop and repaint changes and passed. Final formatting, strict workspace/all-target Clippy, changed-document links, whitespace checks, and the debug macOS bundle rebuild passed. Cargo used the local offline cache and locked dependencies. Logs are in ignored `work/chord-audition-*.log`. Native UI, audio-device playback, and Windows/Linux checks were not performed.


## Catalog toolbar order and Instrument components icon — 2026-10-23

Timeline now orders its catalog buttons as Clip settings, Effects, and Chords. Instrument orders Instrument components before Effects and uses Lucide KeyboardMusic for the components button. Widget creation follows the visible order for keyboard focus. Existing button identities, selection behavior, and the bottom effects-lane toggle remain intact. User guides and the component specification describe the new layout.

Updated existing layout assertions cover the new spacing and order in Timeline and Instrument, including both themes. All 627 UI tests pass with the final icon and button order. Final formatting, strict workspace/all-target Clippy, and the debug macOS bundle rebuild pass.

The complete workspace suite and doctests passed, with existing opt-in acceptance/performance tests ignored. Changed-document links and staged/unstaged whitespace checks passed. Cargo used the local offline cache and locked dependencies. Final UI results are in ignored `work/catalog-order-ui-final.log`; workspace, Clippy, test-build, and packaging logs use the `work/catalog-order-` prefix. Native UI and Windows/Linux checks were not performed.


## Saturator curve separation — 2026-10-23

Soft Sine now uses a wider sine arc, Bass Shaper a broader rational knee, Medium Curve an earlier cubic plateau, and Hard Curve an actual hard knee at ±1.2. Analog Clip retains its default response. All controls, setting identities/version, oversampling, fixed latency, dry/bypass alignment, modulation, and selector crossfades remain unchanged. Existing non-default modes use the new tuning and can change sound/level.

All 10 Saturator tests pass, including new gain-matched processed-triad separation checks at all six project rates and harmonic checks for Soft Sine/Medium Curve and Hard Curve. Existing symmetry, continuity, unity small-signal gain, Bass Shaper upper-bass harmonics, alias suppression, latency, channel isolation, clipping, routing, modulation, and rapid-retarget checks pass. Formatting, strict workspace/all-target Clippy, the CLI build and smoke check, changed-document links, whitespace checks, and the debug macOS bundle rebuild pass.

An ignored CLI probe uses the supplied clean chord at +10 dB Drive, No Clip, 100% wet, and −10 dB Level before encoding. It accounts for the original mono center-pan gain, compares the stationary middle region with a least-squares gain match, and creates a separate −18 dBFS RMS comparison. All processed exports retain headroom. Soft Sine versus Medium Curve differs by approximately 11.5% RMS after gain matching, versus 0.4% in the supplied original recordings; all retuned pairs differ by at least 3.9%. These are waveform measurements, not perceptual listening results. Private media and probe manifests/outputs remain in ignored `work/`; no recordings or machine-specific source paths enter tracked files. Logs use the ignored `work/saturator-retune-` prefix.

The complete workspace suite and doctests passed, including shared Saturator live/export, compatible-update, modulation, latency, and callback-allocation coverage. Existing opt-in acceptance/performance tests remain ignored. Cargo used the local offline cache and locked dependencies. Native listening/UI, audio-device playback, and Windows/Linux checks were not performed.


## Oscillator unit output Level — 2026-10-23

Oscillator now ends its control body with a standard 90-point Out column after Add. Level uses the shared gain knob, spans exact silence through +12 dB, defaults/resets to 0 dB, and saves per layer. The shared MIDI runtime scales the independent oscillator sum before Amplitude and layer effects. It reuses the five-millisecond linear ramp and preserves phase, note/envelope state, source settings, and callback allocation behavior. Legacy layers without the new optional saved field retain unity output. Unit moves/copies into empty destinations carry Level with the shell; existing destination units keep their own Level.

Focused model checks passed for serialization, missing-field migration, finite/range validation, and core component moves/copies. The new engine regression passed all six processing rates, checking multiple summed sources, offline scaling, compatible live changes, phase continuity across attenuation/silence/unmute, exact silence, and zero callback allocations. The UI regression checks last-column geometry, equal strip dimensions, editing without changing individual sources, Undo/Redo, project serialization, and compact presentation in both themes. All 628 UI tests passed; the existing per-source knob-layout assertion now scopes its checks to the source strip. Formatting, strict workspace/all-target Clippy, documentation links, CLI build/smoke checks, whitespace checks, and the debug macOS bundle rebuild passed. Logs use ignored `work/oscillator-level-*.log`.

The complete workspace suite and doctests passed, including all 628 UI tests and the new model, transfer, and engine regressions. Existing opt-in acceptance/performance tests remain ignored. Cargo used the local offline cache and locked dependencies. Native UI, audio-device playback, and Windows/Linux checks were not performed.

## Oscillator Level modulation — 2026-10-23

Added the unit Level destination after the per-note oscillator sum, using shared typed routing, scope checks, offset smoothing, amount controls, markers, and prepared telemetry. Unit routes omit the oscillator target UUID; shared mixed-target validation rejects invalid ownership and duplicate route IDs. Whole-unit shell transfers and layer duplication preserve the appropriate Level connections.

Completed automated checks on macOS:

- Formatting and strict workspace Clippy passed.
- Locked, offline workspace tests passed: 1,236 passed and three configured tests ignored, including all 628 UI tests.
- Portable Level tests cover exact silence, +12 dB bounds, cancellation, wrong targets, invalid base controls, and all six processing rates. Engine tests cover per-note Envelope/shared Output Macro sources, two-source sums, telemetry, playback/export, random-access export, live seeking against an equivalent base Level, compatible route disabling, and callback allocation checks.
- Settings/core/UI tests cover mixed-target validation, save/reopen, fresh copied route IDs, individual-source removal, whole-unit versus source transfers, Level destination selection and marker click-to-open in dark/light themes. Output tests cover prepared telemetry handles and identity retention.
- CLI build and smoke check passed. The macOS debug app was rebuilt through the native packaging script.

Logs: `work/oscillator-level-mod-{clippy,workspace,engine,ui,portable,cli-build,cli-smoke,package}.log`. Native UI listening/device tests and Windows/Linux checks were not run. Live seeks retain the existing fresh-oscillator-phase behavior; random-access export retains sequential render parity.

## Drive v2 — 2026-10-23

Added a separate `daw.drive-v2` effect while retaining Drive and its saved settings. The final layout is Drive: Soft/Hard two-way Clip mode, Gain, Tone; Color: three-way Shape, Strength, Frequency plus shared Power footer; Output: Input Gain, Output Level, Dry/Wet. Input Gain feeds both paths; Color precedes distortion; Output Level follows the mix. The portable processor composes the existing Drive kernel and uses the shared modulation contract for seven continuous controls.

Completed automated checks on macOS:

- Formatting and strict workspace Clippy passed.
- Locked, offline workspace tests passed: 1,243 passed and three configured tests ignored. The complete 629-test UI suite also passed after the final control reorder.
- All 12 portable Drive tests passed. New cases cover exact Soft parity against Drive at all six rates, all three Color shapes with Color on/off, silence/−12/0/+12 dB output levels, dry input-gain retention, output gain after blending, whole-unit bypass, hard clipping, mode transitions/reset, shared-source scope, all modulation inputs, bounds and base silence.
- Renderer integration covers track/Master placement, playback/export/block/random-access parity, processed snapshots and compatible retargeting without callback allocation. Settings tests cover distinct type identity, save/reopen, copied route IDs, invalid controls/routes and opaque future versions.
- UI tests cover catalog addition, three 90-point strips, aligned controls, Color power, gain editing, bounded Soft/Hard keyboard selection, Undo/Redo and project round trips in dark/light themes. Existing catalog tests now include the new row and scroll before offscreen clicks.
- CLI build and smoke check passed. The macOS debug app was rebuilt through `tools/package_macos.sh debug` with the final layout.

Logs: `work/drive-v2-{portable,settings,focused,ui-all,clippy,workspace,cli-build,cli-smoke,package}.log`. Native UI/device listening and Windows/Linux checks were not run. Drive v2 retains Drive's project-rate clipping without oversampling; Clip selects the distortion curve rather than a final output limiter.

## Oscillator output Pitch — 2026-10-23

Added a common Pitch knob below Level in the Oscillator Out strip. It transposes all sources together, retains source phase and tuning intervals, and accepts untargeted Pitch routes through the shared modulation contract. The saved layer setting defaults to zero for older projects.

Completed automated checks on macOS:

- Formatting and strict workspace Clippy passed.
- Locked, offline workspace tests passed: 1247 passed and 3 configured tests ignored, including all 629 UI tests.
- Portable tests cover local/shared routes, base/offset smoothing, compatible retargeting, reset, wrong targets, invalid bounds, and all six processing rates.
- Renderer tests cover two-source transpose equivalence, neutral output, compatible live edits, callback allocation checks, local Envelope/shared Output Macro sources alongside source Pitch routes, processed telemetry, live/export parity, random-access export, live seeking, and route disabling.
- Settings/core tests cover save/reopen, missing-field defaults, bounds, duplication with fresh route/source IDs, source removal, and whole-unit versus individual-source transfers.
- All nine focused Oscillator UI tests passed. They cover the Out strip geometry, independent Pitch editing, Undo/Redo, save/reopen, modulation destination selection, markers, and compact presentation in dark/light themes.
- CLI build and smoke check passed. The macOS debug app was rebuilt through `tools/package_macos.sh debug`.

Logs: `work/oscillator-output-pitch-{check,engine,ui,clippy,workspace,cli-build,cli-smoke,package}.log`. Native UI/device listening and Windows/Linux checks were not run. Live seeks retain the existing fresh-oscillator-phase behavior. Output Pitch applies a common source-frequency ratio; it does not resample the summed PCM signal.
