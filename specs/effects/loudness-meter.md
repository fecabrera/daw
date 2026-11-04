# Loudness Meter

Implemented stereo analyzer with a shared built-in/standalone body and rolling
history. Public unit ID: `daw.loudness-meter`; settings and plugin state version 1.
The checked product manifest freezes separate CLAP and VST3 identities. No new
third-party dependency or source import is required.

## Measurement contract

- All six project rates are supported. Each channel has independent double-precision
  K-weighting: the BS.1770 pre-filter and RLB high-pass prototypes transformed at
  the processing rate. Sum channel powers after filtering; do not sum channels.
- Momentary and Short term are rectangular 400 ms and three-second windows, with
  zero-padded startup. Loudness is `−0.691 + 10 log10(power)`.
- Max M/S update on a 10 ms grid. A bounded observation is emitted every 100 ms.
  Integrated gates fully populated momentary windows: absolute −70 LUFS and
  relative −10 LU against the absolute-gated mean power.
- LRA uses fully populated three-second windows at one-second intervals, absolute
  −70 LUFS and relative −20 LU. Its bounds are the 10th and 95th percentiles.
- Two fixed 14,001-bin distributions cover −100 through +40 LUFS in 0.01 LU steps.
  Counts and exact energy sums remain bounded in memory; gating and percentile
  boundaries have bin quantization. Out-of-range upper values mark the result
  incomplete. No arbitrary duration cutoff or recording buffer is required.
- True peak uses a normalized 64-tap Hann-windowed sinc reconstruction bank. HQ
  evaluates eight phases; ECO evaluates four phases from the same bank. Include
  input sample peaks. Analysis reconstruction lags by about 32 samples and does
  not delay audio. This estimate has not passed broadcast conformance testing.
- Current TP is the maximum over the latest 100 ms observation. PSR is that peak
  in dBTP minus current S; PLR is the maximum TP minus I. Clamp both ratios to zero
  at the lower bound. These PSR semantics do not clone the reference product's
  overlapping peak-envelope algorithm.
- History retains 180 M/S points, one per second, oldest first. The display scale
  is −60 through 0 LUFS with a target reference line. Results outside this display
  scale remain available in numeric readings. No dialogue classifier or
  multichannel profile is included.

The primary measurement references are [ITU-R BS.1770-5](https://www.itu.int/rec/R-REC-BS.1770-5-202311-I/)
and [EBU Tech 3341/3342](https://tech.ebu.ch/loudness). The reviewed external Youlean
analysis informed the feature inventory; its source code, private tables, timing
quirks and dialogue classifier are not imported. Numerical identity with that
product is not an acceptance claim.

## Ownership and callback behavior

`unit-core` owns versioned settings and fixed atomic telemetry. `loudness` owns
K-weighting, prepared energy windows, true-peak reconstruction and the analysis
worker. The audio function returns the original samples without multiplication,
filtering or delay, including signed zero and nonfinite bit patterns. Nonfinite
input contributes silence to analysis and marks it incomplete. Format boundaries
retain their existing nonfinite-buffer rejection policy.

The callback produces fixed observations through a 512-slot `rtrb` queue. It does
not allocate, lock, wait, dispatch work or release final audio owners. Queue-full
sets an epoch-tagged incomplete flag visible even if the final observation is
lost. Worker-start failure uses the same indication. The worker owns integrated
gating, LRA and history reduction. It publishes through `UnitMeter` and a bounded
atomic snapshot. Readers make at most three attempts. Writers try once; a
contending publication is skipped. A snapshot generation masks results racing
with Reset or header power. Worker retirement sets a stop flag without joining;
queue and statistics cleanup happens on the worker after processor retirement.

Prepared instances allocate outside audio processing. Renderer checkpoints detach
publication and do not start analysis workers; binding a live handle prepares its
worker before activation. Compatible same-ID updates
retain the previous analyzer and its worker through existing effect inheritance.
Target changes retain measurement; quality, clock and power changes reset it.
The output rail uses `LevelMonitor<3>` and the existing output handle bindings.
Analysis handles are prepared and retained by effect UUID, including instrument
bus traversal. Disabled readings are masked through `meters::unit_reading`.

## Transport and actions

Transport mode starts a new epoch at a sample-clock discontinuity or the first
sample after processor reset. Pause and Stop retain published summaries. Next Play
after Stop resets; resume after Pause continues. Loop wraps reset. Continuous
ignores clock discontinuities and processor resets but still obeys power and Reset.
Seek reconstruction, transport-cache replay (serial and parallel), offline range
skips, latency priming and synthetic pause/stop fades are excluded from analysis.

Plugin adapters pass playback state separately from their optional frame clock on every block. A supplied stopped
transport suspends Transport analysis; Continuous measures those blocks. Without
host transport information, or while playing without a sample position, the processor's local clock advances. Missing context clears any prior stopped suspension. Hosts may not
distinguish Pause from Stop; a changed playback position or host processor reset
starts the next pass. Native host behavior remains an acceptance check.

Hold and Reset are transient atomic commands, outside effect settings, project
history and automation. Reset advances the epoch. Hold freezes windows, elapsed
count, distributions and history while audio passes unchanged. Queued observations
from before Hold may finish publication. Resume excludes the held interval.

## Shared presentation and formats

The fixed 720-point body has Options, Loudness, Maxima, Range, Dynamics, History
and Out. Ordinary strips are 90 points; History is 180. Reserve the full common row height even for short readouts and buttons, so all
footers align at the bottom. Use the common three-row
height, margins, dividers, small dropdowns, footer labels, fonts and output rail.
History metric selection lives in egui view state. Settings persist only version,
power, collapse, target (Off/−14/−23), quality and clock mode.

Built-in and standalone editors call `unit-ui::loudness::show`. CLAP shares prepared
atomics. VST3 copies 388 scalar attributes on its existing control-thread telemetry
connection: 13 standard meter slots plus 14 analysis scalars, 360 graph values and the transient Hold state.
Analysis values use a +1000 wire offset and zero for negative infinity to fit the
existing finite, nonnegative attribute contract. Reset masks 256/512/1024 carry
analysis Reset, Hold and Resume. The plugin's only parameters are Bypass, Target,
TP quality and Clock, IDs 100–103. Audio latency and tail are zero in both formats.
Packaging discovers the checked manifest; the CI matrix includes both formats on
all three native platforms.

See [validation history](../validation/README.md) for executed checks and remaining
native/device/platform and standards-conformance limits.
