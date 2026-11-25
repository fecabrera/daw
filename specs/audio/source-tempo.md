# Audio-source tempo detection

## Source and presentation

Clip Settings and the lower Timeline audio editor's shared Clip options show
**Source tempo** below **Source**. The read-only value is a representative
pulse estimate in beats per minute (BPM), rounded to the nearest BPM. Show one
estimate for audio clips that share an asset. Show **Multiple sources** for
different assets or a mixed selection. Omit the row for empty and non-audio
selections. Retain the separate Source row and all existing edits.

Use the original prepared PCM for a recording, import, stem, or Bounce asset.
The main file of a Bounce already includes its captured processing. Exclude
auxiliary routing files. A compound asset uses the main recipe on its own
clock, including authored gaps, member processing and internal transitions.
Outer clip edits, track settings, project tempo and timeline placement do not
participate. No analysis data enters the project file or history.

## Worker input and limits

File analysis reads available frames at the prepared rate in 8,192-frame blocks.
Shortened files retain the preparation warning. Compound preparation publishes
an immutable source-only catalog beside the final PCM and transform maps. It
retains reverse rendered-owner metadata for baked loop edges. Copy only the
required source and transform handles. The catalog checks an inspection cap of
65,536 items and reserves from a shared 8 MiB metadata budget across retained
catalog owners. Admission failure leaves ordinary preparation usable.

Compound analysis checks all unmuted recipe members before sampling. Known
missing audio, shortened required ranges, missing transforms, invalid samples
and worker-local read errors cannot produce a number. Apply a limit of 4,096
expanded members, 16 disk readers, 128 million conservative schedule visits and
128 MiB of disk reads per job. Optional refill reservations do not evict other
prepared audio to gain admission. Keep file blocks and detector scratch within
a 1 MiB reservation. All reads and analysis run on a worker.

## Detector

The production source tempo detector uses the Ellis-based estimator for
both file and compound sources. The headless beat-tracking comparison
also runs [offline OBTAIN](offline-obtain.md).

Analyze up to 12 seconds per window. Read the whole source in one to three
adjacent windows up to 36 seconds. Above 36 seconds, take three 12-second
windows from the start, middle and end, with up to 50 ms of unscored warmup
per window. Untested positions can contain tempo changes.

Accumulate independent channel powers in three bands in 10 ms bins. Derive
novelty from positive energy changes against a local floor. Separate detected
onsets by at least 50 ms. Use normalized autocorrelation and a bounded phase
grid search between 45 and 240 BPM. Require at least four onsets and three
adjacent occupied beat slots. Score both the full and accent-weighted onset
sets. When the competitive rates are all related by half or double tempo,
keep the strongest candidate in each supported octave. Show the candidate
closest to 100 BPM, with the lower rate winning an exact tie. Other competing
rates give **Ambiguous tempo**. Rates just outside the supported range can
veto an alias.

Require one supported estimate for a single window and at least two for
multiple windows. Take the median period when the decisive estimates agree
within 3%. If there is half/double ambiguity within or across windows, select
the supported rate closest to 100 BPM and require the periods to agree within
3% after aligning their octaves. Other conflicts and unresolved ambiguous
windows veto the number. Weak windows abstain. Rhythm without a stable pulse
may show **Tempo unavailable**.

When the ordinary detector is weak, a complete source up to 36 seconds can
suggest a second candidate. Test whether the source length spans 4, 8, 16, 32
or 64 beats at a representative tempo from 80 to under 160 BPM. Check the
audio for aligned eighth- or sixteenth-note attacks and repeated intervals
of one, two or four beats. The candidate must align better than nearby tempos.
Require support from one window for a short source and at least two for a
longer one. Ordinary estimates can support the candidate at half or double
tempo. A conflicting estimate or an ambiguous window blocks the fallback.
A window with a clear out-of-range pulse contributes no support. Duration alone
cannot produce a BPM.

If the combined onset and duration result is weak, check signed changes in
the three band levels. Keep up to 1,201 level bins for each of at most three
weak windows. Smooth across three bins, compare separated levels, and suppress
quiet and flat bands before measuring normalized recurrence. Test periods
from 23 through 136 bins, with longer lags as support for a two-beat repeat.
Compare at most eight peaks and eight separated change landmarks per half.
Fit a shared beat or eighth-note phase across both halves and check nearby
and octave rivals. A complete source can also use its existing duration
candidate when a near-aligned one- or two-beat repeat is present in both
halves, the change phase agrees, and no stronger incompatible rate is present.
Keep the combined onset and duration result when it gives a number or reports
ambiguity. A decisive range veto blocks recovery. For multiple windows, require
two agreeing votes, including at least one recovery vote, and keep the 3% raw
period consensus and sample flag. The additional pass uses the existing reads,
cancellation path and 1 MiB admission.

If the result is still weak, check changes in a finer spectrum. During the
same scored reads, average frames into stereo PCM at 2,205 or 2,400 Hz.
Retain up to 12 seconds per weak window. Take a 128-point Hann-windowed FFT
every 10 ms and measure changes across bins 3 through 56. Use both channels so
opposite channel polarity does not cancel a pulse. Require spectral landmarks
in both halves, a shared eighth- or sixteenth-note phase, and repeated changes
at that tempo. Reject nearby competing tempos and honor the earlier range
vetoes. The existing complete-loop duration hint can provide a candidate;
without it, the spectral changes must supply their own rate. Keep the earlier
numeric and ambiguous results and use the same multi-window consensus, worker,
memory admission, and cancellation path.

## Lifetime and status

Keep one active or retiring worker and up to 32 recent outcomes in desktop
memory. Use weak prepared owners, actual PCM framing and an open-workspace epoch
for identity. Cancel work when demand or source identity changes, the inspector
closes, or required preparation takes precedence. Commit a result only for
the current matching source. The row distinguishes waiting for audio, analyzing,
missing audio, unavailable or ambiguous tempo, and analysis failure. A cached
failure can retry once on revisit. A known backing failure invalidates a
previous numeric result.

See [Clip Settings](../editing/clip-settings.md) and
[compound source evaluation](../editing/audio-merge.md#prepared-source-evaluation).

See [offline OBTAIN](offline-obtain.md) for the separate experimental beat tracker.
