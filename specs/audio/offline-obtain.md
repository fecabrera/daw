# Experimental offline OBTAIN beat tracking

The offline tracker analyzes one immutable prepared stereo source. It returns
beat frames and local tempos in the source clock. It also reports a stable
source BPM or a weak, ambiguous, variable or out-of-range outcome. A successful
abstention is distinct from media, admission, resource and cancellation errors.
Standard offline OBTAIN includes targeted tempo selection and percussion
ambiguity recovery. Its entry point is a headless file comparison. Clip
Settings uses the [production Ellis-based source tempo detector](source-tempo.md).

## Source clock and limits

Analyze the whole prepared source through 36 seconds. For longer sources,
analyze independent 12-second regions at the start, middle and end. Reuse the
source tempo window geometry. Read up to 100 ms of real context at region edges
through the same block reader. Retain overlapping context without rereading it.
Zero pad at physical file boundaries. Report zero-based integer source frames,
the prepared sample rate, half-open analyzed and tracked ranges, and whether
each reported beat is supported by a nearby onset or inferred within a gap.
Keep beats ordered, unique and within analyzed regions.

Use the existing six processing sample rates, an 8,192-frame prepared read
block and an optional 8 MiB analysis reservation in the media adapter. Bound
each source to 3,700 grid values, 7,400 channel-equivalent transforms, 256
million counted scoring visits and 192 output beats. Check cancellation before
reads, in feature and search batches, and before returning a result. The
actual FFT packs two stereo channels into one complex transform. Source
read and nonfinite-input failures abort the operation.

The comparison accepts regular files up to 256 MiB encoded. The probe, WAV
validation and Opus timing pass share the same opened file and a 512 MiB
cumulative read cap. Cap demux work at 100,000 returned packets, both PCM
clocks at 600 seconds, and the combined native and converted stereo PCM
payload at 2 GiB. Enforce duration and PCM bounds before reservation growth
or disk writes. The comparison holds one prepared owner and runs both
detectors against it. File identity comes from the open handle. Check
file length and modification time at the end of preparation.

## Offline detector

- Generate onset strength on a 100 Hz clock. Use centered Hann windows of
  about 23.2 ms, FFTs up to 8,192 points, linked stereo magnitudes and a
  20 kHz ceiling. Apply a 0.0002 magnitude floor and log compression.
  Sum positive changes between consecutive spectra. Assign each change
  to the midpoint of the frame centers. Smooth with a centered 15-tap
  Hamming-windowed sinc at 7 Hz. Retain raw flux and RMS for alignment
  and gap checks.
- Examine tempo contexts up to six seconds at half-second steps. For each,
  use centered overlap-normalized autocorrelation, two- and four-period
  enhancement, and a phase grid with separate audio landmarks. Search
  45–240 BPM with guard rates beyond its edges. Consider up to ten
  autocorrelation peaks and 30 expanded candidates, including half and
  double variants and the complete-loop duration hint. Require at least
  three supported grid slots and two consecutive intervals. Dynamic
  programming penalizes period changes and allows unsupported contexts.
  Separate runs across unsupported gaps, silence, and incompatible
  jumps. Repeated weaker offbeat accents can favor the slower grid.
- Accumulate beat strength from the current onset strength and the best
  log-Gaussian-weighted predecessor. The weights are 0.25 for the onset
  and 0.75 for the predecessor. Use predecessor delays from half to twice
  the local period, width parameter 4, and quarter-bin period keys. Reset
  each run and region independently.
- Compare a path through two-sided cumulative-strength peaks with
  phase-aligned pulse paths. Include directly supported edge landmarks.
  Check interval consistency and penalize skips. Withhold a phase tie
  when two comparable paths cover the same span at distinct phases.
  Refine supported timestamps against the raw spectral flux. Infer at
  most one beat between each bracketed pair of anchors when the
  cumulative evidence and local activity support it. Avoid padding or
  quiet-gap extrapolation.

Summarize the median supported interval for a stable run. Use compatible
run estimates for one source BPM. Withhold the BPM when stable periods
differ by more than 3% across runs or when a supported run's end-to-end
tempo path shifts by at least about 6%. On sampled sources require two
regions with a usable run and consistent estimates. Report a credible
alternate octave as a local BPM when it yields its own supported path.
The nearest-100 BPM projection is a separate output based on supported
octaves. Keep the main path and its reported beat frames at the raw
metrical level.

These choices are an independently written offline interpretation of the
[four published OBTAIN stages](https://arxiv.org/pdf/1704.02216v2).
The original paper describes a causal history, online beat deadlines and
peak correction. The offline implementation uses centered contexts,
backtracking and bounded phase comparisons.

## Targeted tempo selection

The standard tracker can replace a steady run on a complete source with one
active span and one supported tempo contour. A candidate near the
duration-derived tempo must be present in at least 80% of three or more
contexts. The candidate must belong to a different tempo family. The
comparison uses one steady rate for each path over the same active span.

Switch only when both paths decode and the challenger has more directly
supported adjacent intervals, at least 55% slot coverage, and a clear
gain in slot coverage and overall onset-evidence score. If the challenger
qualifies, return its decoded tempo and frames. Keep inferred beats out
of the evidence score. Keep the original result if either path cannot be
verified.

## Experimental path verification

The headless comparison can add an opt-in third detector. It has the same
input, source clock and output limits as current OBTAIN. It reuses ordered
phase seeds and landmark weights without changing their ranking.

- Sum positive compressed FFT changes in six physical frequency bands on
  the same 100 Hz clock. Bound local normalization and require raw flux
  and RMS before a band landmark can vote. A weak, guarded, or contested
  broadband path and an independently supported duration cue can trigger
  a secondary search. Merge eligible candidates into the 30-candidate pool.
- Link tempo alternatives across adjacent contexts. Decode at most three
  contours per dispute, including a credible octave and an unrelated
  contour or guard. Decode guards at their actual period and check range
  before publishing. Score contours over the same physical attacks using
  supported slots, adjacent intervals, explained onset weight and phase
  residual. An inferred beat adds no evidence. Require at least three
  matched attacks and a chain of two directly supported intervals.
- A near-complete slower grid can win when its attacks are distinctly
  stronger than the faster grid's subdivisions. Withhold unresolved rate
  and phase disputes. Check the median supported interval against the local
  contour before rounding frames. The nearest-100 projection uses only a
  decoded compatible octave.

## Percussion ambiguity comparison

The tracker attempts recovery when its initial tempo contour is ambiguous.
Recovery requires one active span
on a complete source, repeated short attacks, and a duration-derived tempo
family. It verifies the leader and up to two alternatives over the same
span on the existing broadband onset evidence. The winner must have direct
beat support and a clear score advantage over unrelated decoded contenders.
A range guard or unresolved dispute keeps the original result. A recovered
run reports its own decoded beat frames on the prepared source clock. The
headless comparison can also run targeted selection before recovery.

## Entry point and validation

See the [headless comparison guide](../../docs/development/offline-obtain.md).
The command has file scope. A compound source requires the project's prepared
recipe and a separately admitted analysis-only reader. The comparison does
not create project fields or change playback, editing, or export.

Evaluate BPM against the current detector on the same prepared frames.
Report coverage, raw and half/double-adjusted error and preparation and
analysis times. Evaluate beat frames separately with one-to-one matching
to source-clock annotations, with analyzed-region and full-source recall.
Keep generated controls and independently annotated music separate from
tempo-labeled loops.
