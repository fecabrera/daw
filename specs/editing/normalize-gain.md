# Clip gain normalization

## Accepted behavior

Expose **Normalize gain** in the shared Clip/context menu in Timeline and Editor.
Use the existing menu target rules: a selected target uses all selected clips; an
unselected context target uses only that clip. Enable only for an all-audio
selection. Retain the shared job, modal, focus, text-editing and gesture guards.
The native menu uses the same action metadata and dispatcher.

The standard application dialog contains a canonical dropdown for Sample peak,
True peak and Integrated loudness, a finite numeric Target in −70..=0, and an
Allow gain increase checkbox (default on). Peak modes default to −1 dBFS/dBTP;
loudness defaults to −23 LUFS. Changing mode resets its target to that default.
Disable Normalize for invalid input. Cancel/Escape closes without edits.

On Normalize, close the dialog and run one worker. Report frame-weighted progress
as a percentage in the status bar. Show Cancel there, retain the receiver until
acknowledgement, and reject late cancelled results. Do not open processing,
completion or cancellation dialogs. Use the standard error dialog on failure.

Measure each clip separately, with one linked gain for both channels, over
`0..length_frames` mapped by `Clip::source_frame` into `Session::audio_for_clip`.
This includes trimmed offsets, repeat bases/phases, prepared stretch and transpose.
Exclude existing clip Gain, boundary fades, mute, instrument/track effects, track
mix and Master processing. Do not read surrounding source audio. Stream frames
through `FrameReader`; do not materialize or clone complete PCM.

Set each finite result to `target − measured` dB. When increases are disabled,
cap that result at the existing clip gain. Leave exact silence unchanged. Missing
prepared audio, invalid source ranges, nonfinite samples, undefined loudness or
out-of-range results fail the full group. Clip gain validation and shared clip
settings controls accept `SILENT_GAIN_DB..=CLIP_MAX_GAIN_DB` (+60 dB), superseding
the prior +12 dB clip limit. Track, unit and Master gain bounds are unchanged.

Publish all changed gains through one `Edit::Batch(SetClipGain…)` after analysis
and core validation succeed. Keep the live model, history and playback unchanged
until publication. A no-op leaves the project clean. Undo restores all gains as
one operation. Do not modify or create source recordings or prepared audio.

## Measurement and ownership

`loudness::OfflineAnalyzer` is a synchronous bounded-memory worker API. It reuses
the Loudness Meter's K-weighting, true-peak reconstruction and integrated gated
energy distribution. No worker thread or telemetry queue is created by this API.
Sample peak scans both channel magnitudes. True peak uses eight reconstruction
phases and flushes 64 zero frames after input to include its delayed tail; padding
never enters integrated loudness. This is the existing estimate, without a new
broadcast-conformance claim.

Integrated loudness uses fully populated 400 ms stereo K-weighted power windows
at 100 ms hops, the −70 LUFS absolute gate, and a −10 LU relative gate. The bounded
0.01 LU distribution retains exact power sums as in the live meter. Reject
non-silent clips shorter than 400 ms and clips with no above-gate windows. Silent
clips remain unchanged in every mode. Loudness gain does not apply a peak limiter.
The references remain [EBU loudness guidance](https://tech.ebu.ch/loudness) and the
[Loudness Meter measurement contract](../effects/loudness-meter.md).

`project::normalize` owns options, progress/cancellation and source traversal.
It returns validated gain edits without mutating its Session. `ui::normalize`
owns the options draft and worker lifecycle; `clip_menu` supplies targets. Use
atomic frame counters and cancel state with the existing job channel. No new DSP
or I/O runs in painting, input handlers or the audio callback.

## Validation

Cover independent gains for shared-source trims, both channels, loop phase,
prepared transformed audio, streamed PCM, source immutability, save/open,
attenuation-only behavior, silence, invalid targets/ranges, gain bounds and atomic
failure. Check loudness calibration at all project rates and true-peak tail flush.
Exercise the actual options dialog, context targeting, status progress/cancel,
blocked project actions, no completion dialog, error routing, and grouped Undo.
Record automated results separately from native gesture and platform checks.
