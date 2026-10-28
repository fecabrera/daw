# Audio devices and project audio quality

Application Settings > Playback owns machine-local input/output device identities and concrete defaults for new projects. Each project owns its processing sample rate, output sample format and processing buffer size. Playback and chord/note audition use the current project's quality. Offline export retains its own encoding choices.

## New-project defaults and persistence

- `core::AudioQuality` stores a concrete sample rate, output sample format and buffer size. Initial defaults are 48 kHz, 32-bit float and 512 samples. The dialog offers no Automatic choices and does not depend on connected devices.
- Copy application defaults when creating the initial blank project, using New, or returning to a blank project after Close. Opening a saved project retains its saved quality. Changing defaults does not modify, rescale, stop or recreate the current project or output stream.
- Project Settings exposes sample rate, output bit depth and buffer size alongside name and stretch engine. The existing worker prepares processing changes before atomic publication. Format-only changes also recreate output; undo/redo restores the project format and retires the stream. Name-only and unchanged edits retain their existing paths.
- Project manifests persist `output_sample_format` with the existing rate and buffer fields. A missing legacy format defaults to F32; a missing legacy buffer remains 512. These fallbacks are independent of application preferences. Internal DSP stays floating point; this format requests an output stream type and does not choose export encoding or source recording precision.
- Application storage keeps `audio_quality` separately from `audio` device identities. Migrate explicit legacy optional `audio.sample_rate_hz`, `audio.sample_format` and `audio.buffer_size_frames` into new-project defaults. Missing/null automatic values receive concrete defaults; invalid default rates or buffers recover to the standard quality. Device identities and unrelated preferences remain intact.

## Device controls and stream construction

Output device and input device each offer System default and discovered devices from the default CPAL host. Discovery runs on a named control worker and returns one result through a bounded channel. Refresh does not replace the accepted stream or quality defaults. Input selection is reserved for future recording: it does not open capture, request microphone access, record or monitor input.

CPAL 0.16 exposes names but no portable persistent device identifiers. Store name and occurrence within each direction to distinguish duplicate names. Identity depends on enumeration order among equal names. Device changes/renames can require reselection. System default resolves at stream creation; an open stream does not move automatically.

Stream construction receives only the device identities from app preferences. Resolve quality from `RenderPlan.project`, including when probing an output device replacement. Require the project's advertised output sample format; report unsupported formats with a Project Settings/device-selection error rather than silently substituting a bit depth. Distinguish signed/unsigned integer and float labels. Driver mixing/conversion can still use a different physical hardware format.

Prefer the project processing rate. If hardware requires another supported rate, retain the preallocated output converter without changing project timing or export rate. Request the project processing buffer size, clamp the hardware request to advertised limits, and retain the existing host-default retry for rejected fixed buffers. Hardware adaptation never rewrites the project's saved quality.

## Apply, cancellation and information text

Device selectors publish changes after validation. An output device change probes a replacement using the current project's quality before publishing the new device identity; failure retains the previous device/output and reports an error. Success stops playback and retains the playhead. Input-only and unchanged selections do not open a stream or edit the project.

The Audio quality defaults dialog edits a separate draft. Apply validates and saves only future-project defaults. Cancel/Escape discards the draft; Escape closes an open dropdown first. Missing devices and unsupported current-device combinations do not block future defaults. Project Settings remains the place to change an existing project.

Informational advice below audio quality, Playback and Project Settings controls uses `rows::advice`: Small text in the theme's secondary grey, with wrapping. Separation model summary/description rows reuse this helper. Keep error/progress messages and field-value readouts separate from advice styling.

No new work is added to the audio callback. Discovery, validation, configuration and device opening happen outside it. Rendering, prepared project audio, resampling and channel conversion remain shared. CLI export does not discover devices or read application preferences.

## Windows enumerator lifetime

CPAL 0.16 keeps a process-global `OnceLock` device enumerator but initializes COM on the first calling thread as an STA. If a temporary discovery worker owns that STA, its exit can invalidate the cached interface pointer. Subsequent enumeration can terminate the process with `0xc0000005` instead of returning a Rust error. This affects ordinary Settings refresh as well as tests. The same cache/lifetime issue is documented [upstream](https://github.com/RustAudio/cpal/issues/1302); the pinned 0.16 source has that pattern.

`output::wasapi::prepare` gates both device discovery and output-device selection before their first CPAL host call. A one-time fresh initialization thread explicitly joins the MTA with `CoInitializeEx`, retains it with `CoIncrementMTAUsage`, then seeds CPAL's enumerator in that MTA. Keep one successful MTA usage reference for the same process lifetime as CPAL's static cache. Balance the usage reference on initialization failure/unwind; balance the thread-local initialization with `CoUninitialize` when that worker exits. Do not release it when the short initialization/discovery thread exits. This uses Microsoft's documented [MTA lifetime API](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coincrementmtausage) and leaves the window thread's apartment unchanged. Return worker/COM failures through the existing output/discovery errors. Initialization and waiting occur only on control/worker threads; the audio callback is unchanged. Other platforms retain their existing path.

Reuse the existing `windows-sys` 0.52 binding as a target-specific dependency, with its version shared in the workspace and its license/source notices included by the application packagers. No CPAL or registry package upgrade is required.

UI unit tests use a per-instance injected discovery function. Their default returns an empty synthetic catalog; UI tests must not enumerate real hardware or initialize native audio. Test success/failure/retry through the same bounded worker/result path. Keep real Windows lifetime coverage in an isolated output subprocess: discover/select devices on successive short-lived threads, join each, then repeat. Also query from a simulated window STA. Capture its apartment type and qualifier before audio initialization, verify the type is `APTTYPE_STA` or `APTTYPE_MAINSTA`, then compare the complete pair after each discovery worker exits and after window-thread discovery. Windows distinguishes the main STA (`APTTYPE_MAINSTA`, value 3) from other STAs (`APTTYPE_STA`, value 0); do not hardcode 0 as the post-discovery expectation. See [Microsoft's apartment types](https://learn.microsoft.com/en-us/windows/win32/api/objidlbase/ne-objidlbase-apttype). Missing hardware and driver errors are valid results; native faults and worker panics fail the test. The subprocess keeps a fault from killing the entire test executable.

## Validation

Cover concrete defaults, new/startup projects, saved-project precedence, legacy application/project migration, preference and manifest round trips, invalid values, no-op/cancel/defaults Apply, and format-only apply/undo/redo. Verify device replacements use current project quality and preserve the previous stream/preferences on rejection. Test unsupported output formats and device-rate conversion without hardware. Check Small secondary-grey advice alongside model info in the rendered UI. Run workspace and CLI checks. Record native hardware switching, format negotiation, disconnect/reconnect and Windows/Linux checks separately; automated checks do not establish those results.
