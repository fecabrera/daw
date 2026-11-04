# Shimmer Reverb

Status: built-in processing and independent CLAP/VST3 development products are
implemented. See the [user guide](../../docs/effects/shimmer-reverb.md) and
[validation record](../validation/README.md#shimmer-reverb--2026-11-04).

## Scope and provenance

The user-supplied Solaris recovered technical specification, version 3 dated
2026-11-04, informed the feature set. Its late diffuser used different delay
results in feedback and feedforward and could grow to overflow. This processor
uses an original bounded network. It does not embed extracted instructions,
decompiled functions, runtime tables, plugin binaries or GUI assets. Exposure to
the supplied behavioral and mathematical report is explicit; no clean-room or
sample-equivalence claim is made. Reference presets and original public IDs are
not imported.

Accepted differences from the reference are immediate dry audio, rate-scaled
storage, continuous host controls without the 1/128 quantizer, zero motion at zero
Depth, time-based smoothing, warm history during bypass, a normalized vowel
model, capped pitch regeneration and a contractive late tank. Warm uses a
rate-aware one-pole at 5 kHz. The reference's fixed-coefficient biquad is not used.

## Ownership and delivery

| Owner | Responsibility |
| --- | --- |
| `unit-core::shimmer_reverb` | Version 1 settings, `daw.shimmer-reverb`, interval and note definitions, validation and typed modulation |
| `shimmer-reverb` | Portable prepared DSP, bounds, ramps, telemetry, reset and compatible retargeting |
| `unit-ui::shimmer_reverb` | One headerless body, controls, response bounds, footer and output meter placement |
| `ui::shimmer_reverb_panel` | Standard DAW shell and destination-owned modulation strips |
| `engine::effects` | Shared runtime construction, processing, inheritance, snapshots and meter attachment |
| `shimmer-reverb-plugin` / `shimmer-reverb-editor` | Typed host state, mappings, manifest validation and editor gestures |
| `plugins/shimmer-reverb{,-clap,-vst3}` | Checked manifest and independent exports through existing narrow format sources |

Reuse `reverb-core::Delay` for the stereo pre-delay. Other internal delay reads
use prepared scalar rings. Do not extend the Plate/Ambient processor with product
flags. There is no new third-party dependency or copied native implementation.

Catalog placement is Reverb. Track, group, Master, layer, Main and beat buses use
the existing effect chain and transactions. Selection, collapse, copy/reorder,
Delete, undo/redo and editing guards stay in their shared hosts. Compatible
updates preserve audio history and route identity. Processing history preparation
includes this unit for stop/seek/cache reconstruction; pause and loops retain
tails. Rate changes prepare new storage before publication.

## Shared presentation

Use five 90-point strips and one 161-point two-column Shimmer strip: 611 points
total. Use `unit_columns`, its 19-point equal gaps and one centered Shimmer footer.
There is no divider between the two Shimmer columns. Use the normal full-height
outer dividers, margins, 52-point control rows and shared control height. Both
formats embed exactly the same body. The DAW adds its normal `unit_in_lane` shell.

| Row | Reverb | Vowel | Shimmer column 1 | Shimmer column 2 | Motion | Damping | Out |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Top | Size | Position | Pitch 1 | Pitch 2 | Rate | Low Damp | Level |
| Middle | Pre-delay time/note | Amount | Amount | Amount | Depth | High Damp | Empty |
| Bottom | Time/Sync dropdown | Empty | Empty | Empty | Empty | Warm Off/On dropdown | Dry/Wet |
| Footer | Reverb | Vowel | Shimmer, centered across both columns | | Motion | Damping | Out |

Compact mode retains the footer order. Use standard percentage/gain controls,
bounded stepped pitch knobs, canonical small dropdowns and control capture.
Pre-delay replaces its time knob with a musical division dropdown in Sync mode.
Retain both fields. Its tooltip states the eight-second limit. Put the standard
current/held output rail in Out. `UnitMeter`/`LevelMonitor` owns clearing and
publication; `meters::unit_reading` masks stale readings while bypassed. Meter
reset and appearance are not audio parameters.

## Saved parameters

All scalar controls must be finite. Settings version is 1. Unknown future effect
state remains a missing effect with its payload retained. Keep effect and route
IDs stable; duplication gets fresh IDs through existing helpers.

| Parameter | Saved bounds | Default | Plugin ID |
| --- | --- | --- | --- |
| Bypass | 0/1, inverse of `enabled` | 0 | 100 |
| Size | 0–1 | 0.75 | 101 |
| Pre-delay | 0–1000 ms | 0 | 102 |
| Sync | false/true | false | 103 |
| Note | index 0–15 | 6, dotted 1/16 | 104 |
| Vowel Position | 0–1 | 0 | 105 |
| Vowel Amount | 0–1 | 0 | 106 |
| Pitch 1 | index −15…15; host 0…30 | 0; host 15 | 107 |
| Amount 1 | 0–1 | 0 | 108 |
| Pitch 2 | index −15…15; host 0…30 | 0; host 15 | 109 |
| Amount 2 | 0–1 | 0 | 110 |
| Rate | 0–1 | 0.390625 | 111 |
| Depth | 0–1 | 0.429688 | 112 |
| Low Damp / High Damp | 0–1 | 0 / 0 | 113 / 114 |
| Warm | false/true | false | 115 |
| Level | silence sentinel…+12 dB | 0 dB | 116 |
| Dry/Wet | 0–1 | 0.5 | 117 |

Interval index k maps to `sign(k)*max(abs(k)/4,abs(k)-3)` semitones. Index zero
mutes the voice's feedback and late injection through a 5 ms ramp. The two voices
are fixed slots; variable-strip reordering does not apply. Shared note beats and
labels define sixteen whole/dotted/triplet divisions, from 1/64 to dotted 1/2.
Synced milliseconds are `min(60000*beats/tempo,8000)`. Ignore invalid tempo and use
the existing host fallback. Local DSP calls use 120 BPM if their tempo is invalid.

The manifest freezes distinct CLAP/processor/controller identities. Version 1
plugin state uses `DAWSHIM1`, the common bounded envelope and 18 f64 parameter
values in ID order. Reject foreign magic, future versions, invalid values,
truncation and trailing bytes atomically. Identity and default-state fixtures
freeze the contract. Format wrappers advertise stereo f32, six processing rates,
zero alignment latency and a fixed 120-second tail budget. Both receive host
transport for synced pre-delay. Editor presentation shows the selected note;
it does not imply that the editor has live tempo telemetry.

## DSP and stability

Processing is stereo pre-delay → fixed early allpasses → vowel blend → fixed
early allpasses → two regenerated pitch voices → late tank → Warm blend → curved
dry/wet mix → Level → global bypass blend. Dry audio remains immediate.

The fixed early Schroeder allpasses read q before writing `v=x−0.5q` and output
`0.5v+q`. Both branches use the same current read, so the recovered one-frame
mismatch is absent. Ring limits scale with rate.

The vowel uses two rotating resonators per channel. Each has a fixed radius
`r=exp(−2π45/Fs)<1`, a position-dependent unit-norm rotation and excitation
`(1−r)*input`. Rotation updates every 32 samples. A convex average of the real
outputs blends with input. Varying the rotation cannot increase its state norm;
there is no resonant feedback gain above one.

Each pitch voice uses a 600 ms ring, two linearly interpolated read heads and
complementary nonnegative cosine windows. The phase step is `(1−ratio)/window`.
Ratio follows the stepped interval through a 5 ms ramp. A convex 10 kHz one-pole
filters the read. Feedback is `0.65*(1−(1−amount)^2)`, at most 0.65. Its write is
half the early input plus that bounded return. Thus regeneration remains bounded
under pitch changes. Equal pitches halve combined injection through a 5 ms ramp;
amount also attenuates injection by `1−0.25*max(amount1,amount2)`.

The late tank has eight independent scalar rings. Tap seconds are
`[.0437,.0571,.0713,.0899,.1093,.1319,.1571,.1811]*(.3+Size)` plus at most ±2 ms
of motion. Reserve 300 ms per line. Reads are convex linear interpolation, so
moving delays do not amplify the historical supremum. Scattering uses two signed
half-weight taps per row; each row's absolute sum is one. The feedback gain is
`0.25+0.67*Size`, at most 0.92. Excitation is 0.18 times the channel input.

High damping uses a convex one-pole from 18 kHz down to 800 Hz. Low damping blends
the low-passed signal with half its difference from a convex 400 Hz low-pass
state. Both branches are bounded by the historical input supremum. This bound
survives coefficient changes and moving taps; stability does not rely on motion,
Warm, a limiter or output nonfinite recovery. The wet readout is a signed sum
with total absolute weight one per channel. This deliberately lossy network is
an original sound design and has no reference-null-test requirement.

Motion frequency is 0.1–1.8 Hz internally; its Rate presentation stays normalized.
Depth zero is exactly no motion. Update motion targets and damping coefficients
every 32 samples, with interpolated motion offsets between updates. Warm's
history remains active while off. Mix gains are `dry=1−m²` and
`wet=.9*(1−(m−1)²)`. Level follows the mix and is bounded to +12 dB after shared
modulation. Exact base silence remains silent. Nonfinite input is treated as
silence; use f64 state and finite f32 output bounds for extreme input headroom.

Apply fixed +15 dB wet makeup after Warm and before the existing mix. It is
outside the pitch and tank feedback loops, so their contraction bounds and tail
shape remain unchanged. Output peak bounds must include the 5.623413251903491
multiplier; this is not a limiter. No detector, new state, saved parameter or host
ID is introduced. Existing saved settings use the new wet level. Reuse the
generated fixtures and calibration procedure in [reverbs](reverbs.md#calibrated-wet-makeup),
including full regeneration and short/long Size headroom checks.

Prepare all rings and meter handles before processing. Retarget, reset, sampling
and modulation must not allocate, lock, perform I/O or release final owners.
Resets invalidate rings in constant time. Base controls, power, ratio and mode
transitions use 5 ms ramps; additive offsets use the shared 1 ms smoother.
Pre-delay tap travel also ramps over 5 ms. Initial/reset timing uses the first
host tempo without ramping from a stale read head. Reset after an unprocessed
retarget must match construction with the new parameters. Musical delay changes can bend pitch.
Continue all histories during silence and bypass. Host suspension is separate.

The fixed 120-second tail declaration is a conservative processing budget, not
an audibility guarantee. DAW export retains its ten-second cap and truncation
notice. Automatic host sleep must not discard an active tail.

## Acceptance

Cover six-rate dry/bypass/silence/reset and meter behavior, signed pitch-tone
measurements, synced delay bounds, arbitrary parameter/tempo changes, extreme
input, continuous modulation on track/layer/Main, callback heap operations,
prepared seeks, compatible inheritance, project save/open and capped exports.
Run the ignored 120-second frozen/moving tail stress explicitly in release.

Editor checks compare complete meshes in Dark/Light/Mist at 1×/1.5×/2×, at defaults
and two changed snapshots. Check both pre-delay modes, response containment,
balanced gestures and every discrete selector. ABI tests cover every parameter,
choices, irregular/in-place buffers, dense automation, active recall and state
corruption. Manifest discovery and CI include both formats on all three platforms.
Native host/editor, listening, device and deadline acceptance remain separate.
