# Plate Reverb and Ambient Reverb

Status: both built-in units and independent CLAP/VST3 development products are implemented. Native host/device and reference-equivalence acceptance remain open.

User guides: [Plate Reverb](../../docs/effects/plate-reverb.md) and [Ambient Reverb](../../docs/effects/ambient-reverb.md).

## Accepted presentation

Both products use shared `unit-ui` bodies. The DAW owns the normal unit shell,
selection, collapse, drag handling, and modulation connection strips. Native
plugin editors embed the same headerless body. Use standard 90-point strips,
three 52-point control rows, shared padding, dividers, dropdowns, footers and Out
meter. Derive height from `layout::unit_control_height`.

Plate's High-pass and Low-pass knobs reuse Drive v2's shared frequency formatter:
whole Hz below 1 kHz and one decimal place in kHz at or above 1 kHz, for both
readouts and value editing. Show standard range labels below each knob: `20` / `1k`
for High-pass and `500` / `15k` for Low-pass. Keep this in the shared body for DAW
and standalone editor parity.

| Plate row | Reverb | Filters | Character | Out |
| --- | --- | --- | --- | --- |
| Top | Pre-delay | High-pass | Drive | Width |
| Middle | Decay | Low-pass | Drive power + label | Level |
| Bottom | Normal/Gated | Color | ECO/HQ | Dry/Wet |

| Ambient row | Reverb | Texture | Out |
| --- | --- | --- | --- |
| Top | Decay | Tone | Width |
| Middle | Bloom | Modulation | Level |
| Bottom | ECO/HQ | High-pass power + label | Dry/Wet |

Gate selects a short algorithm, not a level-triggered gate. Disable the Quality
control while gated, retaining its saved choice. Drive power disables only the
Drive amount control. Confirm Color's bright/dark mapping before freezing labels.

Drive and High-pass power use `rows::power_label`, centered in the existing
control row. Keep their host parameter identities and response bindings unchanged.

## Processing ownership

`daw-reverb-core` owns original portable mathematical primitives. Product crates
own the two processing chains. `unit-core` owns versioned settings and
typed shared modulation. Backend types must stay private.

Plate: wet LP/HP/Color filters, stereo preparation, tank, wet width, curved mix,
optional amplifier, Level. Ambient: wet HP, diffusion, tank, wet width, curved mix,
Level, transformer. The Ambient detector consumes the previous sample's shaper
output. Preserve a continuously generated kernel stream; a fixed impulse response
does not define the analyzed primitive.

Preparation must allocate FFT plans, scratch space, rings, mode banks and meters
off audio. Callback processing and parameter changes must not allocate, block,
compile processing code, retire final owners, or clear large buffers in one step.
Compatible updates retain histories. Mode transitions need a bounded, preallocated
transition mechanism. Use deterministic versioned seeds and processing-clock
events instead of Windows timers. Keep base/bypass ramps at 5 ms and modulation
offsets at the shared 1 ms duration. Document departures from reference smoothing.

DAW Stop/seek reset and reconstruct history through the worker path. Pause retains
state; loops retain tails. External host suspension is a separate lifecycle event.

## Parameters and persistence

Plate Decay is 0.3–15 s, default 3 s; pre-delay is 0–300 ms, default zero.
High-pass is 20–1000 Hz, default 20 Hz; low-pass is 500–15000 Hz, default 15000 Hz.
Drive is −12..+12 dB, default zero and disabled. Ambient Decay is 1–60 s, default
7.5 s. Bloom and Tone default to 0.5; Modulation defaults to the captured 0.4975.
Both default to ECO, 100% width, 50% mix, and unity Level. Level uses the common
exact-silence..+12 dB scale, deliberately extending the reference attenuation bound.

Persist physical values, stable effect IDs, discrete modes, collapse, routes and
route visibility. Validate finite bounds atomically and preserve future state as
a missing effect. All continuous destinations use shared routing and smoothing;
discrete modes remain unmodulated.

## Standalone delivery

Each product needs its own manifest, typed plugin model and editor, CLAP and VST3
targets over shared format sources. Freeze distinct public identities and explicit
parameter IDs before release. Plate has 13 parameters including Bypass; Ambient
has 10. Appearance and meters remain outside audio parameter state. Reuse bounded
state publication, real widget gesture capture, native child windows, packaging
and platform CI. Both hosts call the same body renderers.

Report zero alignment latency while dry input remains immediate. Musical wet
onset is not dry-path alignment delay. Add shared tail reporting rather than
inheriting existing plugins' four-second default. Derive conservative maximum
tails from full Decay/Bloom testing. DAW export retains the accepted ten-second
tail cap with a truncation notice unless separately changed.

## Implementation and provenance

The primitive crate implements the three-point rational control map, product mix
curves, wet M/S width, rounded 0.7071 diffusion junction, four-lane deterministic
noise, stereo fractional delay and generated prime-table Bloom mapping. Prime
values are generated independently rather than copied from a runtime table.
The runtime adds a private 1024-frame-hop, 2048-point radix-two FFT, evolving noise partitions, coupled diffusion, an orthogonal sixteen-line late feedback tank, stereo preparation, filtering, smoothing and output models. It allocates all storage and twiddles before processing. The feedback tank derives attenuation from each delay and the requested T60. Kernel updates advance during silence and bypass. Reset invalidates delay/history storage and selects prepared initial kernel partitions; it does not allocate or rebuild FFT kernels on audio.

Bloom prime-delay targets refresh every 128 samples. Both diffusion and late
feedback lines crossfade between stationary read taps over 50 ms with a cubic
smoothstep. Requests during a fade replace only the pending target, preserving
continuity. Do not interpolate the delay address itself or restart an active
fade on each drag update. Reset selects the current target without a stale fade.
The transition uses fixed arrays and preserves state during compatible retargets.

These are original algorithm-family adaptations, not ports of captured functions. Partition scheduling, excitation, coefficient generation, diffusion and amplifier behavior are not sample-identical reconstructions. No complete reference-plugin null test has passed, and reference emulation is not claimed.

The model appends modulation slots 95–104 without changing published identities. Bodies reuse shared parameter/amount controls and response capture. The plugin models freeze separate class IDs, product magic and parameter/default-state fixtures. Both products use the existing CLAP/VST3 sources through concrete dependency aliases.

The shared modulation input storage includes all 105 destination slots. Reading
an unconnected reverb destination must return zero. Playback preparation tests
cover both products, restart, and 48 kHz project audio at a 44.1 kHz device rate.
These tests guard against a destination index panic that stopped playback.

The release timing probe at 192 kHz with 32-frame blocks measured roughly
22–24% average single-core utilization, but FFT bursts took about 500 microseconds
against a 167-microsecond block period. This configuration does not meet the
deadline in that probe. Small-buffer deadline acceptance requires further work;
average CPU use alone is not sufficient. Prepared storage was about 22.8 MB per
unit at that rate. See the validation record for the exact command and limits.

Enabled Plate reports a 45-second conservative tail budget (two seconds when Gated); Ambient reports 180 seconds. Plugin host declarations use fixed product maxima so automation cannot shorten an active host tail. DAW export sums enabled budgets conservatively across buses, appends at most ten seconds and warns on truncation. These are processing budgets rather than measured audibility guarantees.

The supplied reports describe epicPLATE mkII and epicCLOUDS. The integration
addendum supersedes short-settle mode comparisons: steady Gate overrides Quality;
Ambient's short-settle wet output can remain dimmed. Reference comparisons must
record message settling, silent warmup, sample rate and initial state. No native
plugin code or decompiled functions are included in this crate.

## Acceptance

Automated tests cover model bounds and future-state retention; primitive reference anchors; six-rate wet output, reset and bypass; portable/plugin equality; dense automation and callback heap checks; shared body geometry and tessellation parity; track/Master history inheritance and seek reconstruction; MIDI layer/Main modulation; catalog operations; persistence and capped-tail export. Record actual command results separately in [validation](../validation/README.md).

Full reference comparisons, native external-host GUI attachment, listening, and device deadline acceptance remain separate gates. No build or automated input test establishes those results.

Paired rendering tests must cover all shared themes and scales. Listening, native
UI/device tests and external host acceptance remain separate from headless checks.
