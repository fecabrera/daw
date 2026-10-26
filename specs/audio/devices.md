# Audio device preferences

Application Settings > Playback exposes input/output device selectors. Its separate Audio quality defaults dialog controls machine-local output rate, sample format and buffer preferences. Project rate/buffer size and offline export stay under their existing project/export controls. Device stream conversion does not change project timing, prepared media, or saved processing settings.

## Controls and persistence

- Output device and input device each offer System default and discovered devices from the default CPAL host. Discovery runs on a named control worker and returns one result through a bounded channel. The UI polls it without blocking. Refresh re-enumerates devices without replacing the accepted stream or quality defaults.
- Input selection is reserved for future recording. It persists but does not open a capture stream, request microphone access, record, or monitor input.
- Output sample rate offers supported combinations of the six project-rate choices. Automatic prefers the project rate and otherwise selects a supported device rate for the existing output converter.
- Output bit depth selects the advertised stream sample format, distinguishing signed/unsigned integer and float formats. Do not label a float stream as integer PCM. Driver mixing/conversion can use a different physical hardware format; this control does not set export bit depth or change the internal float DSP.
- Output buffer size offers supported power-of-two requests from 32 through 4096 samples. Automatic prefers the project buffer, clamps to the advertised range and retains the existing default-buffer retry. An explicit size is never silently clamped or retried as a different size. Unknown hardware buffer limits require a successful stream-open probe.
- Persist preferences in application storage with backward-compatible defaults. They do not enter project manifests, undo history, or plugin state. Default preferences preserve automatic device/rate/format/buffer selection.

CPAL 0.16 exposes names but no portable persistent device identifiers. Store name and occurrence within each direction to distinguish duplicate names. Identity depends on enumeration order among equal names. Device changes/renames can require reselection. System default follows the host default on stream creation; an already-open stream does not move automatically.

## Apply and failure

The main Settings selectors publish device changes immediately after validation. An output change retains the existing quality defaults and probes a replacement before publishing the device choice; failure retains accepted preferences/output and displays the error in Settings. Input changes only save the preference.

The separate Audio quality defaults dialog edits a draft of rate, format and buffer size, without device selectors. Cancel/Escape discards it; Escape closes a dropdown first. Return to application Settings after Apply or Cancel. Disable device and quality editing during device discovery. Show unavailable saved devices in the main selectors. Block quality Apply for a missing output or unsupported format/rate/buffer combination. An unavailable reserved input does not block output quality changes.

Apply probes an output replacement on the control thread before publishing it or saving preferences. Failure retains the prior stream/preferences/project and shows the error in the dialog. Success replaces the stream, stops playback, retains the playhead and keeps the project clean. No-op and input-only changes do not rebuild output. Closing the main Settings window retains accepted device choices. Playback and chord/note audition use the accepted output preferences when constructing a stream. Device failure retains the existing error path and never silently substitutes a named device.

No new work is added to the audio callback. Discovery, allocation, configuration and device opening happen outside it. The original rendering, prepared project audio, resampling and channel conversion remain shared. CLI export does not discover devices or read application preferences.

## Validation

Test automatic preference selection, explicit format/rate/buffer requests and unsupported combinations without hardware. Test preference round trips and legacy defaults, missing devices, failed stream publication, input-only/no-op changes, draft cancellation and popup Escape behavior. Run workspace regression checks. Record native hardware switching, driver format negotiation, disconnect/reconnect and Windows/Linux checks separately; unit tests do not establish those results.
