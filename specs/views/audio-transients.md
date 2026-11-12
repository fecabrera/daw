# Audio transient indicators in Editor

Status: initial visual implementation, 2026-11-08. Detector behavior is specified
in [Audio analysis](../audio/analysis.md). This feature adds presentation and
optional worker analysis; it does not change stretch detection or playback.

## Controls and presentation

Audio tracks show **Show transients** in the lower Editor toolbar, above its
ruler. It defaults off at application startup and remains view state during the
running application. It is not a project edit or saved analysis asset. MIDI/beat
editors do not show the control. Toggling, analysis completion and cache changes
must not mark the project dirty, add history or change selection/transport.

When enabled, paint thin translucent vertical lines from the bottom of each
clip's 22-point name header to the clip bottom. Include the 8-point fade-handle
strip above the waveform. Clip to the visible clip/viewport; keep the header text
clear. Paint before fade curves/controls and selection outlines. Lines span both
stereo lanes and have no pointer hit areas. Only the lower audio Editor shows
indicators; Timeline and the Editor overview do not. Existing fade, stretch,
trim, selection and context-menu gestures retain their ownership.
Use the dark palette's beat-grid color in every theme at 45% opacity, above the active clip grid's 10%
opacity. Preserve marker positions, line width and clip background colors.

Use one line per visible logical pixel at most. Query sorted onsets by binary
search rather than enumerating every repeated occurrence. Reuse
waveforms::source_windows for exclusive clip ends, source offsets, trim bounds,
loop phase and wrap windows. Prepared audio positions are already in transformed
source coordinates. While painting a provisional/unprepared stretch, map cached
original onset points with ClipStretch::scale, consistent with waveform source
mapping. Do not analyze every pointer-movement preview.

## Analysis and data ownership

The UI requests analysis only for visible lower-editor clips when enabled and
not dragging, running a foreground file job or waiting for required audio.
Prefer Session::audio_for_clip, falling back to original audio exactly as the
waveform painter does. The media adapter calls the existing Combined detector
on the complete prepared mono/linked-stereo PCM. Analyze before clip gain,
fades, channel enable/swap and polarity settings; these markers describe source
attacks, not an effects-processed output or mixed crossfade contribution.

crates/media owns the storage-independent adapter. Read in 8,192-frame chunks
through AudioData::read_frames on a worker, then analyze one contiguous input.
Do not reset detector history at chunk boundaries. Resident and disk-backed
inputs must produce identical marker positions. Keep the normal six supported
processing rates. Cancellation is checked before allocation/reading and during
analysis; invalid samples/layout and resource/I/O failures return errors.

Admission reserves complete interleaved input, the analysis estimate and 128 KiB
for read buffers. Reject requests above 128 MiB of additional known storage
before reading. Use the existing shared AudioReservation budget too. This is a
scratch/input limit, not process RSS; the caller's shared source, FFT planner
internals and allocator overhead retain their existing exclusions. Oversized
sources remain playable/editable with an analysis-unavailable status. A streaming
detector or sensitivity controls are outside this initial feature.

## Optional worker and cache lifecycle

editor_transients::State owns at most one worker/result channel per app instance.
Use a bounded one-result channel and a cancellation token. Poll without waiting;
request repaint on completion and periodically while waiting. Required audio and
foreground preparation take priority: cancel optional analysis when such jobs
start. Leaving Editor, changing to MIDI/beat content, disabling the toggle or
dropping the state also cancels. Retain the pending receiver until completion,
so repeated toggles cannot start overlapping workers.

Keep a 32-entry LRU result/error cache in memory. Source identity uses immutable
resident/disk allocation identity, processing rate and channel layout. Weak
references prevent identity reuse without retaining full PCM. Prepared buffers
for different stretch/pitch/engine revisions have different owners, so they
cannot reuse stale markers. Identical source owners shared by clips reuse one
result. No source hashes or filesystem reads run in UI painting. The detector
configuration is fixed; future configurable presets must join cache identity.

A project change cancels the job and clears entries. Generation/cancellation
checks discard late results. Failed analysis is cached to prevent automatic
retry loops. Show a spinner while working and a small unavailable label with an
error tooltip for cached failures. Toggling off/on clears failures for retry.
Cache eviction permits later reanalysis. Optional analysis errors do not stop
playback or replace prepared audio. Original media and project manifests remain
unchanged; no marker editing, snapping, splitting or warping is implemented.

## Verification

Automated checks cover resident/disk parity and source preservation, invalid
input, cancellation, memory admission, trim/loop/exclusive-end/preview mapping,
weak cache ownership, replacement invalidation, stale completion rejection,
worker completion/reuse, toggle/dirty-state behavior, lower-editor-only painting
and exact header-bottom/clip-bottom line geometry. Run shared waveform/fade and
Editor interaction tests after mapping or painting changes. Native screenshot
review, device behavior and Windows/Linux execution are separate checks.
